# 動態天氣與大氣環境配置系統設計規格書 (Dynamic Weather & Atmosphere Lighting Designer)

**文件版本**：1.0  
**日期**：2026-10-08  
**模組命名空間**：`AgainstRomeMapEditor.Modules.Atmosphere`  
**適用架構**：.NET 8.0 / C# 12 / OpenGL 3.3 Core (Map3DViewControl)  

---

## 摘要 (Executive Summary)

本文件詳細規範《反抗羅馬》(Against Rome) 地圖編輯器之「動態天氣與大氣環境配置系統 (Dynamic Weather & Atmosphere Lighting Designer)」。
原版遊戲引擎在載入地圖時（`LDFLOORINI` 函式 `0x481de0`），透過 `boden.ini` 定義海平面、水面物理波紋凹凸、閃電機率、晝夜時間、大氣光影網格及粒子特效；配合 `daynight.bmp` 24 時段環境光、建築 APT 局部光源與 `shadows.dat` / `skydens.dat` 衍生快取，構成了原版遊戲的環境光照表現。

目前地圖編輯器僅提供基礎時刻滾輪（`GameHour`）與分散的純數值欄位，缺乏完整的天候預設模型、3D 視圖大氣霧氣深度模擬、雷電爆閃預覽以及與地圖儲存交易（`MapEditorSaveTransaction`）的強型別雙向連動。

本系統實作了以下核心架構：
1. **`BodenIniData` & `BodenIniSerializer`**：強型別資料模型與雙向無損序列化解析器，100% 相容原版遊戲檔案規範，並支援保留未知區段與註解。
2. **`AtmosphereProfile` & `WeatherPresets`**：定義大氣配置資料結構，並提供五大經典情境（**晴朗豔陽**、**雷鳴暴風雨**、**暮色日落**、**濃霧迷漫**、**高山霜雪**）。
3. **`AtmosphereLightingBridge`**：3D 視圖即時著色管線連動橋接器，將大氣迷霧、天色散射、動態水波與雷電爆閃即時注入 `Map3DViewControl` 之 GLSL 著色器。
4. **`AtmosphereEditSession`**：實作 `IEditorModule<AtmosphereProfile>` 統一編輯介面，支援髒狀態追蹤、快照還原與交易式儲存。
5. **儲存交易與快取失效整合**：無縫整合 `FileRollbackScope`，並在環境關鍵參數變更時精確失效相關 `.dat` 快取檔案。

---

## 系統架構總覽 (System Architecture)

```
+---------------------------------------------------------------------------------------+
|                                    MapEditorForm (UI)                                 |
|  +---------------------------------------------------------------------------------+  |
|  | Weather Presets Dropdown | Time-of-Day Slider | Fog Controls | Water Dynamics   |  |
|  +---------------------------------------------------------------------------------+  |
+-------------------------------------------+-------------------------------------------+
                                            |
                         [UI Actions / Preset Selection]
                                            v
+---------------------------------------------------------------------------------------+
|               AtmosphereEditSession : IEditorModule<AtmosphereProfile>                |
|  - Tracks IsDirty, Baseline Snapshot, Undo/Redo                                       |
|  - Holds current AtmosphereProfile                                                    |
+---------------------+---------------------------------------------+-------------------+
                      |                                             |
            [3D View Realtime Tick]                            [Save Map]
                      v                                             v
+-------------------------------------------+   +---------------------------------------+
|         AtmosphereLightingBridge          |   |       MapEditorSaveTransaction        |
|  - Animation Time & Flash Simulation      |   |  - Preflight Validation               |
|  - Computes AtmosphereShaderUniforms      |   |  - FileRollbackScope (Transaction)    |
|  - Evaluates Day/Night + Fog + Weather    |   |  - BodenIniSerializer.UpdateDocument  |
+---------------------+---------------------+   |  - Invalidate Height/Sky Caches       |
                      |                         +-------------------+-------------------+
            [GLSL Uniforms Injection]                               |
                      v                                             v
+-------------------------------------------+   +---------------------------------------+
|             Map3DViewControl              |   |               boden.ini               |
|  - TerrainFragmentShader (Fog + Lighting) |   |  - 100% Native Game Compatible        |
|  - SpriteFragmentShader (Fog + Tint)      |   |  - Preserves Comments & Unknown Keys  |
|  - Water Shader (Dynamic Waves & Ripple)  |   |  - Formatted in InvariantCulture      |
+-------------------------------------------+   +---------------------------------------+
```

---

## 一、原版 `boden.ini` 完整逆向參數規範與相容性原則

依據逆向工程分析（針對 `Against_Rome.exe` 於 `0x481de0` 之剖析常式），`boden.ini` 採 INI 區段行配合次行純數值的結構。原版全部參數及其語意規範如下：

### 1. 核心參數清單

| 區段標籤 (Section) | 資料型別 | 引擎有效範圍 | 預設值 | 引擎語意與反組譯特徵 |
| :--- | :--- | :--- | :--- | :--- |
| `[Waterlevel]` | `float` | $\ge 0$ | `120` | 全地圖基準海平面高度。全域存放於 `0x77187c`。低於此高度之地形視為水下。 |
| `[Heightmapstep]` | `float` | $> 0$ | `4` | 高度圖步進乘數。全域存放於 `0x771880`（setter 為 `0x498d70`）。$64 \times 4 + 1 = 257$ 頂點。 |
| `[WaterBumpAmplitude]` | `int` | $0..1024$ | `256` | 水面法線凹凸波紋幅度。數值越大水面起伏越劇烈。 |
| `[WaterBumpFrequency]` | `int` | $1..16$ | `4` | 水面波紋頻率。決定水波密集程度。 |
| `[WaterWarpShift]` | `int` | $10..18$ | `14` | 水面反射折射擾動位移。$10$ 為最高擾動，$18$ 為最低平靜位移。 |
| `[RainDropsOnWater]` | `int` (bool) | `0` 或 `1` | `0` | 水面雨滴漣漪效果開關。開啟時水面產生雨滴擴散同心圓。 |
| `[WaterColor]` | `string` | 6 碼 Hex | `0xffdfbf` | 水體調色色碼，格式為 `0xbbggrr`（Little-Endian BGR）。 |
| `[WasserTexturName]` | `string` | 字串 | `wassAW` | 水面貼圖基底名稱（對應材質庫中的 `wassAW00.bmp` 等）。 |
| `[CausticTexturName]` | `string` | 字串 | `caustA` | 水底焦散光貼圖基底名稱。 |
| `[SkyTexturName]` | `string` | 字串 | `sky` | 天空穹頂貼圖前綴。 |
| `[FlashPropability]` | `int` | $0..1000$ | `8` | **注意原版拼寫為 Propability（含 p）**。每秒閃電觸發機率係數。 |
| `[DayStartTime]` | `float` | $0..24$ | `6` | 白天日出起始時刻（小時）。載入全域 `0x771C50`。 |
| `[DayEndTime]` | `float` | $0..24$ | `20` | 夜晚日落開始時刻（小時）。 |
| `[ShadowMeshMode]` | `int` | `0` 或 `1` | `0` | 陰影網格太陽運算模式。`0`=太陽僅上下俯仰移動，`1`=太陽東南西三向完整軌跡。 |
| `[ShadowMeshAccuracy]`| `int` | $1..10$ | `6` | 陰影網格解析度。 |
| `[ShadowMeshXZsize]` | `int` | 整數 | `16000` | 地形陰影網格 XZ 邊界長度。 |
| `[ShadowMeshYsize]` | `int` | 整數 | `5000` | 地形陰影網格 Y 軸高度範圍。 |
| `[FlashLightDefaultIndex]` | `int` | 索引 | `-1` | 閃電產生時所引用的 `lightdef.dau` 光源索引。 |
| `[SnowAlrIndex]` | `int` | 索引 | `-1` | 飄雪天候所引用的 ALR 粒子素材索引（`-1` 表關閉飄雪）。 |
| `[HandleSkyDensMap]` | `int` | `0` 或 `1` | `1` | 引擎是否載入並維護 `skydens.dat` 快取。 |
| `[HandleShadowMeshes]`| `int` | `0` 或 `1` | `1` | 引擎是否載入並維護 `shadows.dat` 快取。 |

### 2. 相容性與回寫原則
- **保留註解與未知區段**：原版地圖包含德文開發註解（如 `;0..1024, 256=default`）及未完全公開的擴充區段（如 `[FlashObjectDefaultIndex]`、`[HagelShadowIndex]` 等）。編輯器回寫時**絕對不可丟棄**未經修改的內容。
- **文化獨立性 (Culture Invariant)**：所有浮點數及整數寫入時一律使用 `CultureInfo.InvariantCulture`，避免德語/法語語系 Windows 將小數點寫為逗點 `,` 導致遊戲引擎載入當機。
- **大小寫嚴格維持**：特別是 `FlashPropability` 鍵名，若修正為英文正確的 `FlashProbability` 將導致原版遊戲無法識別閃電設定。

---

## 二、`AtmosphereProfile` 與五大經典天氣預設架構

為了使設計師能夠快速為地圖營造獨特的氛圍，《反抗羅馬》大氣系統建立了標準的 `AtmosphereProfile` 資料結構，並封裝了五種經典天氣情境：

### 1. 核心資料模型 (`AtmosphereProfile`)

`AtmosphereProfile` 整合了原生引擎參數（`BodenIniData`）與現代著色器預覽參數：
- **`Boden`**：包含上述全部原生 `boden.ini` 欄位。
- **`FogEnabled`** / **`FogStartDistance`** / **`FogEndDistance`** / **`FogDensity`** / **`FogColor`**：距離與大氣霧氣參數。
- **`SkyScatteringColor`**：天色瑞利/米氏散射加權。
- **`AmbientTint`** / **`Exposure`**：環境光乘數與視圖整體曝光。
- **`Precipitation`** / **`PrecipitationIntensity`**：降水型態（無 / 雨 / 雪 / 冰雹）與強度。
- **`WindSpeed`** / **`WindDirection`**：風速與風向（驅動波紋相位與粒子位移）。

### 2. 五大經典天氣預設參數對照表

| 預設 ID | 名稱 | 視覺氛圍 | 水體參數 (Amp/Freq/Warp/Color) | 霧氣參數 (Start/End/Density/Color) | 天候與光照特性 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `clear_sky` | **晴朗豔陽** | 晴空萬里，暖陽微風 | `140` / `3` / `14`<br>`0xffdfbf` (蔚藍透亮) | `60` 格 / `180` 格 / `0.20`<br>`RGB(0.72, 0.82, 0.95)` | 無降水，曝光 $1.0$，微幅空氣透視遠霾。適合標準戰役。 |
| `storm` | **雷鳴暴風雨** | 烏雲蔽日，風雨交加 | `720` / `9` / `10`<br>`0x5a5040` (暗沉墨青) | `12` 格 / `60` 格 / `0.85`<br>`RGB(0.22, 0.26, 0.28)` | 豪雨 ($0.95$)，水面雨滴漣漪，閃電機率 $30$，爆閃光源啟動。 |
| `sunset_dusk`| **暮色日落** | 殘陽斜照，沉靜晚霞 | `240` / `4` / `13`<br>`0x406090` (深琥珀紅) | `30` 格 / `110` 格 / `0.50`<br>`RGB(0.85, 0.45, 0.30)` | 暖金紅天色散射，長影斜照（ShadowMeshMode=1），大氣泛橘紅。 |
| `dense_fog` | **濃霧迷漫** | 沼澤迷霧，神秘幽閉 | `80` / `2` / `15`<br>`0x788a80` (迷霧灰綠) | `4` 格 / `38` 格 / `0.96`<br>`RGB(0.68, 0.73, 0.76)` | 近距離迷霧遮蔽（4格起），水波平緩，適合林地與伏擊地圖。 |
| `mountain_snow`| **高山霜雪** | 冰川晶藍，冷冽肅殺 | `60` / `2` / `16`<br>`0xf0d8a0` (冰川碧藍) | `25` 格 / `95` 格 / `0.48`<br>`RGB(0.82, 0.88, 0.98)` | 飄雪天候（SnowAlrIndex=1），冷藍色散增益，高反光曝光 $1.12$。 |

---

## 三、`BodenIniSerializer` 雙向序列化架構

`BodenIniSerializer` 提供了強型別的高效能解析與雙向回寫能力：

### 1. 解析演算法 (Parsing Algorithm)
```csharp
// 利用多行正規表示式精確匹配區段名與其正下方數值行
var matches = Regex.Matches(iniText, @"(?im)^\s*\[(?<section>[^\]\r\n]+)\][^\r\n]*(?:\r?\n)(?<value>[^\r\n]*)");
```
- 區分已知欄位與未知保留區段：
  - 若 `section` 存在於 `KnownKeys` 白名單中，解析數值並寫入 `BodenIniData` 對應屬性。
  - 若 `section` 不在白名單中，整段純文字存入 `data.PreservedSections` 字典。
- 註解安全隔離：數值行中的行尾註解（以 `;` 起始）在賦值前自動過濾，但保留在原始文檔中。

### 2. 雙向文檔更新 (In-Place Document Update)
針對既存地圖之 `BodenIniDocument`，直接調用 `SetValue(key, formattedValue)`。由於底層 `BodenIniDocument` 採用正則定位值行切片替換（Slice Replacement），因此：
- 原始排列順序不變。
- 註解與排版空白 100% 逐字保留。
- 僅值行的字元被替換為合規新字串。

### 3. 合規範圍檢驗 (Validation Preflight)
在寫入硬碟前，`BodenIniSerializer.Validate` 實施嚴格邊界檢查：
- `WaterBumpAmplitude`: $0 \le x \le 1024$
- `WaterBumpFrequency`: $1 \le x \le 16$
- `WaterWarpShift`: $8 \le x \le 20$
- `FlashPropability`: $0 \le x \le 1000$
- `DayStartTime` / `DayEndTime`: $0 \le x \le 24$
- `WaterColor`: 必須為 6 碼合規十六進位 BGR 格式
- `Heightmapstep`: 必須 $> 0$

---

## 四、3D 視圖環境色與光照管線連動（`AtmosphereLightingBridge`）

為實現「所見即所得」的編輯體驗，`AtmosphereLightingBridge` 連接了 `AtmosphereProfile`、`SceneLightingContext` 與 `Map3DViewControl` 的 OpenGL 著色器。

### 1. 執行期動態 Uniform 結構 (`AtmosphereShaderUniforms`)

```csharp
public sealed class AtmosphereShaderUniforms
{
    public Vector3 EffectiveAmbient { get; init; }  // 綜合時段+大氣+雷電之全域光
    public Vector3 FogColor { get; init; }          // 霧氣色彩
    public Vector4 FogParams { get; init; }         // (StartDist, EndDist, Density, Enabled)
    public Vector3 SkyScattering { get; init; }     // 天色散射係數
    public Vector3 SunColor { get; init; }          // 主方向光色
    public Vector3 SunDirection { get; init; }      // 主方向光向量
    public Vector4 WaterDynamics { get; init; }     // (BumpAmp, BumpFreq, WarpShift, RainDrops)
    public float FlashIntensity { get; init; }      // 當前瞬間電光增益 (0..1)
    public float SimulationTime { get; init; }      // 模擬時間（秒）
}
```

### 2. 時鐘更新與雷電動態模擬 (Flash Simulation)

編輯器畫面以 30~60 FPS 渲染時，`Update(deltaTimeSeconds, profile)` 負責推進：
1. **累積時間**：`SimulationTime += deltaTime`，供水波紋與雨滴漣漪計算相位。
2. **電光消退**：若當前有閃電，以高衰減率（$\Delta t \times 6.5$）迅速指數消退，模擬真實電光猝滅（持續時間約 0.15 秒）。
3. **泊松雷鳴機率觸發**：
   - 當 `FlashPropability > 0` 時，計算平均間隔 $T_{avg} = \max(0.5, 1000 / \text{FlashPropability})$。
   - 隨機計時器歸零時觸發電光：`FlashIntensity = 0.8 + rand * 0.2`。
   - 瞬間提升 `EffectiveAmbient`，使全地圖地形、單位與水面產生強烈白光爆閃效果。

### 3. GLSL 著色器整合規範

#### A. 通用標頭程式碼 (`GlslAtmosphereHeader`)
```glsl
// 大氣與天氣系統動態 Uniforms (由 AtmosphereLightingBridge 提供)
uniform int uAtmosphereFogEnabled;
uniform vec4 uAtmosphereFogParams; // x: startDistance, y: endDistance, z: density, w: unused
uniform vec3 uAtmosphereFogColor;
uniform vec3 uAtmosphereSkyScattering;
uniform float uAtmosphereFlashIntensity;
uniform vec4 uAtmosphereWaterDynamics; // x: bumpAmp, y: bumpFreq, z: warpShift, w: rainDrops
uniform float uAtmosphereTime;

// 空氣透視與距離迷霧混合函式
vec3 applyAtmosphereFog(vec3 litColor, float viewDist) {
    if (uAtmosphereFogEnabled == 0) return litColor;
    float factor = clamp((viewDist - uAtmosphereFogParams.x) / max(0.001, uAtmosphereFogParams.y - uAtmosphereFogParams.x), 0.0, 1.0);
    factor = factor * uAtmosphereFogParams.z;
    return mix(litColor, uAtmosphereFogColor, factor);
}

// 閃電瞬時強光增益
vec3 applyAtmosphereFlash(vec3 litColor) {
    return litColor + vec3(uAtmosphereFlashIntensity * 0.7);
}
```

#### B. 地形著色器整合 (`TerrainFragmentShader`)
```glsl
void main() {
    // 既有光照計算：貼圖 * vertex.bmp * 局部光源
    float l = max(0.28, dot(normalize(N), normalize(uLight)));
    vec3 color = texture(uAtlas, UV).rgb * l;
    color *= texture(uVertexLight, uv).rgb;
    color *= gameLight(worldPos);

    // 大氣與雷電連動擴充
    color = applyAtmosphereFlash(color);
    float dist = length(worldPos - uCameraPosition);
    color = applyAtmosphereFog(color, dist);

    c = vec4(color, 1.0);
}
```

#### C. 水面動態波紋 Shader 整合 (`GlslWaterDynamics`)
利用正弦波與柏林擾動擬合，依據 `uAtmosphereWaterDynamics` 動態改變水面頂點法線與反射 UV：
```glsl
vec2 computeWaterRippleOffset(vec2 worldPos, float time, float amplitude, float frequency) {
    float phase = time * (frequency * 0.4);
    float wave1 = sin(worldPos.x * 0.15 + phase) * cos(worldPos.y * 0.15 + phase * 0.8);
    float wave2 = sin(worldPos.x * 0.3 - phase * 1.2) * cos(worldPos.y * 0.25 + phase * 1.1);
    float normAmp = amplitude / 1024.0 * 0.08;
    return vec2(wave1 + wave2 * 0.5, wave2 + wave1 * 0.5) * normAmp;
}
```

---

## 五、地圖儲存交易（`MapEditorSaveTransaction`）與快取失效整合

環境大氣系統嚴格遵守專案既有的交易回滾架構，確保編輯器儲存失敗時絕不產生半損壞檔案。

### 1. 交易生命週期 (Transactional Lifecycle)

```
[UI Trigger: SaveMap]
        |
        v
[Phase 1: Preflight 檢查]
  - 呼叫 BodenIniSerializer.Validate(profile.Boden, out var errors)
  - 檢查水面高度、波紋、閃電、顏色 Hex 合法性
  - 檢查地形診斷面板（MapDiagnostics）嚴重錯誤
  - 若有阻斷性錯誤：終止儲存，不觸碰硬碟任何檔案
        |
        v
[Phase 2: 啟動 FileRollbackScope]
  using var rollback = new FileRollbackScope();
  - 追蹤既有檔案狀態
        |
        v
[Phase 3: 原子化寫入 boden.ini]
  var iniDoc = BodenIniDocument.Load(Path.Combine(mapDir, "boden.ini"));
  BodenIniSerializer.UpdateDocument(iniDoc, profile.Boden);
  iniDoc.Save(rollback);
        |
        v
[Phase 4: 環境衍生快取智慧失效 (Cache Invalidation)]
  - 比對 baseline 與當前 profile：
    若 (ShadowMeshMode 變更 || Skydensspread 變更 || Waterlevel 變更)
    則調用 TerrainLayerFiles.InvalidateHeightCaches(mapDir, rollback);
    --> 安全刪除 skydens.dat, shadows.dat, visible.dat, cliprect.dat
        |
        v
[Phase 5: 交易提交與狀態重設]
  - 其他檔案（briefing.put, DATA/*.dat, SDL 等）全數寫入成功
  - rollback.Dispose() 正常離開（不觸發回滾）
  - atmosphereSession.AcceptChanges()：標記 IsDirty = false
```

### 2. 失敗回滾保證 (Rollback Guarantee)
若在儲存過程中的任何後續步驟（例如 DATA 物件儲存上限超標、SDL 格式異常或磁碟空間不足）擲出例外狀況：
1. `FileRollbackScope` 捕捉例外，立即將 `boden.ini` 原樣回滾至儲存前的位元組狀態。
2. 已刪除的 `.dat` 快取檔案從備份中完全復原。
3. `AtmosphereEditSession` 未執行 `AcceptChanges()`，記憶體中的未儲存髒標記（`IsDirty = true`）完整保留，設計師可修正錯誤後重新儲存。

---

## 六、模組化架構（`IEditorModule`）與 UI 規劃

遵循專案架構規範，純邏輯與 UI 完全解耦：

### 1. 模組類別職責
- **`src.MapEditor.Modules/Atmosphere/`**（純 .NET 8.0，無 WinForms 依賴）：
  - `BodenIniData`：強型別資料載體。
  - `BodenIniSerializer`：雙向序列化與合規性驗證。
  - `AtmosphereProfile`：完整天候配置實體。
  - `WeatherPresets`：五大經典預設工廠與字典。
  - `AtmosphereLightingBridge`：動態 uniforms 計算、電光模擬與著色器代碼產生。
  - `AtmosphereEditSession`：狀態管理（實作 `IEditorModule<AtmosphereProfile>`）。

### 2. UI 面板設計規劃 (`MapEditorForm.Atmosphere.cs`)
在主編輯器表單右側的檢查器分頁（Inspector Tabs）中，新增或升級「環境與大氣 (Environment & Atmosphere)」面板：
- **天氣預設下拉選單 (Preset ComboBox)**：
  - 清單包含：`晴朗豔陽`、`雷鳴暴風雨`、`暮色日落`、`濃霧迷漫`、`高山霜雪`、`自訂環境`。
  - 切換預設時，自動平滑更新水面波紋、水色、迷霧距離與光照色調，並保留使用者設定之地表海平面高度。
- **水體控制群組 (Water Controls)**：
  - 水面高度 (`NumericUpDown`)、水體色彩（色盤選擇按鈕 + Hex 預覽）。
  - 波紋幅度 ($0..1024$)、頻率 ($1..16$)、扭曲位移滑桿。
  - 水面雨滴漣漪（核取方塊）。
- **天候與大氣迷霧群組 (Weather & Atmosphere Fog)**：
  - 啟用迷霧（核取方塊）。
  - 霧氣起始與終止距離拉桿（$0..200$ 格）。
  - 霧氣濃度拉桿（$0.0..1.0$）與霧氣色彩選擇器。
  - 閃電機率（$0..1000$ 滑桿）與「⚡ 觸發閃電預覽」測試按鈕。
- **晝夜時段 (Time-of-Day)**：
  - 晝夜起訖時間滑桿（$0..24$ 時）。
  - 太陽軌跡模式（上下 / 東南西）。

---

## 七、驗證方案與自動化測試

本系統之核心類別已於 `tests/AgainstRomeMapEditor.Modules.Tests/AtmosphereTests.cs` 完成完整單元測試覆蓋：

| 測試案例 | 驗證項目 | 預期結果 |
| :--- | :--- | :--- |
| `BodenIniData_DefaultValues_PassValidation` | 預設物件合規性驗證 | 0 錯誤，完整通過邊界檢查。 |
| `BodenIniData_WaterRgbConversion_RoundTripsAccurately` | 水色 `0xbbggrr` 與 RGB 雙向轉換 | 數值無損雙向轉換，紅藍綠通道位元正確。 |
| `BodenIniSerializer_ParseText_ParsesKnownAndPreservesUnknownSections` | 文字解析、數值讀取與未知區段保留 | 正確解析所有標準鍵，自訂區段原樣保留並能重新輸出。 |
| `BodenIniSerializer_Validate_CatchesInvalidValues` | 各參數超界防護 (Amp, Freq, Warp, Flash, Color, DayNight) | 邊界外數值皆正確被阻擋並回報對應錯誤訊息。 |
| `WeatherPresets_AllFivePresets_AreDistinctAndValid` | 五大經典預設集完整性與合規性 | 5 套預設 ID 唯一、名稱完整、參數皆通過嚴格驗證。 |
| `WeatherPresets_TryGetPreset_FindsCaseInsensitive` | 預設識別碼大小寫不敏感查詢與 Fallback | 正確命中目標預設，未知 ID 降級至 ClearSky。 |
| `AtmosphereLightingBridge_ComputeUniforms_CalculatesEffectiveAmbientAndFog` | 著色器 Uniforms 數值與環境色調結合 | 正確結合日夜環境色、AmbientTint、曝光增益與迷霧參數。 |
| `AtmosphereLightingBridge_ManualFlashAndDecay_WorksExpectedly` | 雷電瞬時強光與影格時間指數衰減 | 手動觸發瞬間提高環境光，在 0.2 秒內平滑消退歸零。 |
| `AtmosphereEditSession_IEditorModule_TracksDirtyBaselineAndReset` | `IEditorModule` 髒狀態、變更提交與還原 | 修改屬性標記 Dirty，AcceptChanges 歸零，Reset 正確復原。 |
| `AtmosphereEditSession_CaptureAndLoad_PerformsDeepCloning` | 快照深層複製隔離性 | 修改 captured 複本絕不影響 session 內部狀態。 |

---

## 結論與交接指引

本設計方案已具備完整的 C# 核心程式碼與嚴格的單元測試，所有模組均已編譯並驗證無誤：
- 核心程式碼位於 `src.MapEditor.Modules/Atmosphere/`。
- 單元測試位於 `tests/AgainstRomeMapEditor.Modules.Tests/AtmosphereTests.cs`。
- 遵循 `AGENTS.md` 規範：未存取遊戲安裝目錄、未修改其他代理之並行檔案、不自行執行 Git Commit。
- 接續工作可直接參考本文件之 GLSL 規格將 `AtmosphereLightingBridge` 接入 `Map3DViewControl.cs` 與 `MapEditorForm`。
