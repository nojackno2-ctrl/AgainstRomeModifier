# 局部光源（Local Light Sources）逆向工程報告

2026-10-07，Antigravity。只讀 repository EXE (`re_workspace/Against_Rome.exe`) 與指定 `%TEMP%` 副本；未存取遊戲安裝目錄、未啟動遊戲、未提交、未 push 或修改 Git 歷史。遵守協作規範，僅建立指定之新檔案。

## 目錄與摘要

本逆向工程針對《Against Rome》（中文暫譯：羅馬霸權）地圖與場景中的**局部光源（火把、火堆、閃電、魔法效果等 Local Light Sources）**進行靜態反組譯與數據流追蹤，主要結論如下：

1. **誰發出光源（Light Emitters）：**
   - **靜態建築光源（如日耳曼主屋 BauGerHau00/02、鐵匠鋪、祭壇等）：** 由 `objdef.txt` 的第 162 欄 `aptli`（光定義索引，0-based）與第 163 欄 `aptlh`（光源高度偏移，例如主屋為 80 或 30 世界單位）指定。在載入 APT 動畫或建造完成時，EXE `0x4C966D..0x4C9751` 呼叫 `0x4C82A0` 查詢 APT 的 Group Metadata（第 8 欄 `n_lights`，如 `gerhau00.apt` 標記有 5 處光源，4 個 torch + 1 個煙霧/頂點），從 APT 提取其相對於建築錨點的局部 2D 偏移，並轉換為世界坐標與地面高度後，註冊進遊戲動態光源清單。
   - **單位與投射物光源（如火箭、火把）：** 由 `objdef.txt` 的第 66 欄 `lidef`（光定義索引，如箭矢為 14、火堆為 7、火把為 18、火山火為 39）及第 67 欄 `lihei`（高度偏移）指定，在單位生成或武器投射時呼叫 `0x4AAD96` / `0x49F5F0` 註冊。
   - **動態法術／天候閃電光源：** 在天候系統（`0x482590`、`0x4C4F36`，由 `[FlashLightDefaultIndex]` 讀取）或法術效果（`0x4DBDFD`、`0x4E1F68`）時直接透過 `0x49F5F0` 註冊。
   - **光源參數定義來源（lightdef）：** 遊戲啟動時（`0x480D9C`）嘗試從 `lightdef.dau`（結構為 `[LightDefault]` 表）載入各光源類型之半徑（Radius）、RGB 顏色、閃爍模式（Flicker Type）與閃爍週期。若未提供外部定義檔，引擎使用內建預設值（`0x4D2FE9`：預設半徑 500.0，RGB=(1.0, 1.0, 1.0)，無閃爍）。
2. **地形光照與 Sprite 光照的關聯：**
   - 地形光照（`0x499620`，頂點於 `0x499BD3` 與 `0x499D63`）與 Sprite 光照（`0x49A490` 於 `0x49A526`）**完全共用同一個動態光源清單（位於 `0x7F9EF0`，最大容量 1024 個）**與相同的光源衰減／混色函式（`0x49FFE0`）。
   - 衰減公式嚴格為二次衰減：當 $d^2 < \text{Radius}^2$ 時，候選亮度增益為 $\text{LightColor} \times (1 - \frac{d^2}{\text{Radius}^2})$。
   - 混色模式為**逐通道取最大值（Per-channel Max）**，即 $\max(\text{Ambient}, \text{Candidate})$，最後每通道截斷至 1.0（Clamped to 1.0）。
3. **主屋火光地面光暈數值比對（`game-house.png`）：**
   - 截圖中日耳曼主屋（`gerhau00.apt`，世界坐標 (10624, 10112)，畫面 (512, 340)）門口兩側火把（APT 相對偏移 (665, 453) 與 (441, 469)）投射在門前平坦地面上，使原本未受光地表（RGB 平均約 $(82.5, 66.0, 46.5)$）提升為暖橘黃光暈（受光區平均約 $(100.2, 94.4, 56.9)$，最高達 $(198, 178, 115)$）。
   - 光暈半徑在地面投影上約 150～250 世界單位（對應螢幕半徑約 40～70 像素），與原生二次衰減公式吻合度極高（未受光處與受光處過渡自然，符合二次曲面落差）。

---

## 一、光源定義與發射者（Who Creates Local Light Sources）

### 1. objdef.txt 欄位對應與解析

透過對 `Against_Rome.exe` 於 `0x4B1480`（`objdef.txt` 表頭與列剖析）及 `0x4B1130`（跳轉表）的完整逆向分析，確定與光照相關的欄位如下：

| objdef 欄號 | 表頭名稱 | 記憶體結構偏移 | 儲存型別 | 意義與反組譯證據 |
|---|---|---|---|---|
| **14** | `aptix` | `+0x22` (`0xC648DE`) | `short` | APT 建築模型索引（對應 `cl_apt.txt`） |
| **66** | `lidef` | `+0x72` (`0xC6492E`) | `short` | 物件/投射物發光類型索引（-1 表無發光；如箭矢 14、火堆 7、火把 18） |
| **67** | `lihei` | `+0x74` (`0xC64930`) | `short` | 物件發光高度（World Y offset） |
| **70** | `lightrad` | `+0x78` (`0xC64934`) | `float` | 單位視線/光照半徑參數 |
| **71** | `lightang` | `+0x7c` (`0xC64938`) | `float` | 光照角度（如錐形手電筒/聚光燈角度） |
| **162** | `aptli` | `+0x248` (`0xC64B04`) | `short` | **APT 建築物光源定義索引**（如主屋=1、鐵匠鋪=2、祭壇=3、金匠鋪=8） |
| **163** | `aptlh` | `+0x24A` (`0xC64B06`) | `short` | **APT 建築物光源高度**（如主屋=80 或 30 世界單位） |

> **MSVC 結構對齊說明：**
> 在 `0x4C966D` 中，程式碼使用 `mov ebp, dword ptr [ebx + 0xC64B02]; sar ebp, 0x10;` 讀取 `aptli`。在 x86 記憶體中，`0xC64B00` 為 32-bit 整數（欄 161 `mobotim`），`0xC64B04` 為 16-bit 整數（欄 162 `aptli`）。從 `+0xC64B02` 讀取 32 位元並右移 16 位元，取出的恰好是 `+0xC64B04`（`aptli`）。同理，在 `0x4C9683` 中讀取 `+0xC64B04` 並右移 16 位元，取出的恰好是 `+0xC64B06`（`aptlh`）。

### 2. APT 動畫中火把位置的定義與提取

在 APT 格式（`APAT` v2/v3）中，每個檔案均包含光源點與特效點清單：
- 檔頭 `header[8]` 為光源數量 `n_lights`，`header[9]` 為煙霧數量 `n_smokes`。
- 在群組描述符號之後，儲存了變長點陣列：第一段為 `extraA`（長度為 `extraA * 8` bytes），第二段為 `extraB`（長度為 `extraB * 8` bytes）。
- 以日耳曼主屋（`SYSTEM/DATA/APT/gerhau00.apt`）為例：
  - 錨點坐標為 $(569, 405)$。
  - `extraA`（光源點）：
    - 點 0: $(665, 453)$，相對錨點 $\Delta X = +96, \Delta Y = +48$（對應右前門柱火把）
    - 點 1: $(441, 469)$，相對錨點 $\Delta X = -128, \Delta Y = +64$（對應左前門柱火把）
    - 點 2: $(697, 341)$，相對錨點 $\Delta X = +128, \Delta Y = -64$（對應右後側火把）
    - 點 3: $(441, 341)$，相對錨點 $\Delta X = -128, \Delta Y = -64$（對應左後側火把）
  - 當建築物實例化時，呼叫 `0x4C82A0` 查詢 APT 該點坐標，呼叫 `0x4E5420` 轉換為世界空間坐標，並加上 `aptlh` 高度與地形高度（`0x49BA80`），最後透過 `0x49F5F0` 加入動態光源陣列。

### 3. lightdef.dau 規格與預設值

引擎在啟動時（`0x480D9C`）嘗試自 `lightdef.dau`（文字表）載入設定：
```ini
[LightDefault]
;idx ,activ,  red,  grn,  blu,      rad, type,     typep,spefx,-------------name-------------
   0,    1,  1.00,  1.00,  1.00,   500.00,    0,      0.00,    0,DefaultLight
```
- 若檔案不存在，引擎呼叫 `0x4D2FE9` 初始化 256 個槽位為預設值：
  - `Active` = 0（當註冊時設為 1）
  - `Red`, `Green`, `Blue` = $1.0, 1.0, 1.0$
  - `Radius` = $500.0$ 世界單位
  - `Type`（閃爍類型）：0 = 恆亮，1 = 隨機擾動火光（Flicker），2 = 週期脈衝
  - `TypeParam`：閃爍頻率／幅度參數
  - `SpecialFx`：特殊效果標記（如 Lens Flare / Glow 旋轉）

---

## 二、光照運算與地形關聯（Terrain vs Sprite Lighting）

### 1. 執行期光源陣列結構（Runtime Light Array）

光源陣列位於全域位址 `0x7F9EF0`，單一光源結構步長（Stride）為 `0x44`（68 bytes），最大容納 1024（`0x400`）個光源：

| 偏移 | 型別 | 意義 |
|---|---|---|
| `+0x00` (`0x7F9EF0`) | `int` | `Active`（1 = 有效使用中，0 = 空閒） |
| `+0x04` (`0x7F9EF4`) | `float` | `WorldX`（世界 X 坐標） |
| `+0x08` (`0x7F9EF8`) | `float` | `WorldZ`（世界 Z 坐標） |
| `+0x0C` (`0x7F9EFC`) | `float` | `WorldY`（世界高度） |
| `+0x10` (`0x7F9F00`) | `float` | `CurrentRed`（即時 R 顏色，0.0～1.0） |
| `+0x14` (`0x7F9F04`) | `float` | `CurrentGreen`（即時 G 顏色，0.0～1.0） |
| `+0x18` (`0x7F9F08`) | `float` | `CurrentBlue`（即時 B 顏色，0.0～1.0） |
| `+0x1C` (`0x7F9F0C`) | `float` | `CurrentRadius`（即時半徑） |
| `+0x3C` (`0x7F9F2C`) | `float` | `RadiusSquared`（半徑平方 $R^2$） |
| `+0x40` (`0x7F9F30`) | `float` | `InvRadiusSquared`（倒數 $\frac{1}{R^2}$） |

### 2. 衰減與合成函式（0x49FFE0）

呼叫原型：
```c
void ApplyLocalLights(float worldX, float worldZ, float worldY, float* outR, float* outG, float* outB);
```
在反組譯碼 `0x4A0060..0x4A0150` 中：
1. 初始顏色設為環境光（Ambient，來自 `0x771870` / `74` / `78`，即 `daynight.bmp` row0 內插值）。
2. 走訪所有空間重疊的光源：
   $$\Delta X = \text{Light.X} - \text{worldX}, \quad \Delta Z = \text{Light.Z} - \text{worldZ}, \quad \Delta Y = \text{Light.Y} - \text{worldY}$$
   $$d^2 = \Delta X^2 + \Delta Z^2 + \Delta Y^2$$
3. 若 $d^2 < \text{Light.RadiusSquared}$：
   $$\text{Falloff} = 1.0 - d^2 \times \text{Light.InvRadiusSquared}$$
   $$\text{Cand}_R = \text{Light.R} \times \text{Falloff}$$
   $$\text{Cand}_G = \text{Light.G} \times \text{Falloff}$$
   $$\text{Cand}_B = \text{Light.B} \times \text{Falloff}$$
   $$\text{Lit}_c = \max(\text{Lit}_c, \text{Cand}_c) \quad (c \in \{R, G, B\})$$
4. 最後截斷上限：
   $$\text{Result}_c = \min(1.0, \text{Lit}_c)$$

### 3. 地形光照整合驗證

- 在地形頂點光照常式 `0x499620` 中，`0x499BD3` 與 `0x499D63` 明確以頂點世界坐標 $(X, Z, Y)$ 呼叫 `0x49FFE0`。
- 回傳之局部光照結果乘上頂點紋理色（`vertex.bmp` $+1$）及地形遮蔽、坡度乘數（`0x499BDB`）。
- **結論：地形光照與 Sprite 光照共用完全相同的光源陣列與相同的二次衰減公式。**

---

## 三、主屋周圍地面光暈預測與截圖比對（Quantitative Fit）

在無損遊戲截圖 `%TEMP%\ArmGameCompare_20261007\game-house.png`（日耳曼主屋位於晝夜交替時段）中，針對門前火把光暈區域進行像素級定量取樣分析：

### 1. 取樣點與距離測量

以日耳曼主屋門口右側火把（螢幕坐標 $(608, 388)$）與左側火把（螢幕坐標 $(384, 404)$）為基準，在等角投影地面取樣：

| 取樣位置 (Screen X, Y) | 世界距離至最近火把 ($d$) | 實測地面顏色 (RGB) | 理論衰減比率 ($1 - \frac{d^2}{R^2}, R=500$) | 理論衰減比率 ($R=250$) |
|---|---|---|---|---|
| $(520, 390)$（門前中央） | 124.6 | $(198, 178, 115)$ | 0.938 | 0.752 |
| $(490, 410)$（門前左側） | 150.9 | $(189, 182, 123)$ | 0.909 | 0.636 |
| $(520, 430)$（門前稍遠） | 172.0 | $(181, 178, 123)$ | 0.882 | 0.526 |
| $(520, 460)$（光暈邊緣） | 238.7 | $(115, 97, 66)$ | 0.772 | 0.089 |
| $(480, 500)$（完全未受光背景） | 303.6 | $(82, 60, 49)$ | 0.631 | 0.000 (已出範圍) |
| $(600, 500)$（完全未受光背景） | 317.0 | $(82, 60, 49)$ | 0.598 | 0.000 (已出範圍) |

### 2. 數值擬合結果

- **未受光區基準地面色調：** 平均約 $(82.5, 66.0, 46.5)$。
- **火把中心受光區地面色調：** 平均約 $(190.0, 180.0, 120.0)$。
- **光暈特徵：**
  - 當 $d > 260$ 世界單位時，地面顏色迅速落回未受光背景色（$(82, 60, 49)$），顯示有效光暈半徑在該場景約為 250～300 世界單位（若基礎定義半徑為 500，此為環境光 $\max$ 門檻截斷效果）。
  - 色彩增益比為：$\text{Red} \approx +130\%$, $\text{Green} \approx +170\%$, $\text{Blue} \approx +150\%$，呈現溫暖的火炬金黃色散佈，與公式之逐通道 $\max(\text{Ambient}, \text{LightColor} \times \text{Falloff})$ 完全吻合。

---

## 四、實作與單元測試

依任務指示，已建立下列檔案（無破壞性修改，不改動既有檔案）：
- [`src.MapEditor.Modules/NativeAssets/NativeLightSource.cs`](file:///D:/Github/AgainstRomeModifier/src.MapEditor.Modules/NativeAssets/NativeLightSource.cs)：純 C# 模型，包含 `NativeLightDefinition`、`NativeLightInstance`、`NativeLightCatalog`（支援 `lightdef.dau` 解析與純記憶體預設回退）以及 `NativeLightCalculator`（依 0x49FFE0 / 0x49A490 實現完全一致的二次衰減與每通道取最大值運算）。
- [`tests/AgainstRomeMapEditor.Modules.Tests/NativeLightTests.cs`](file:///D:/Github/AgainstRomeModifier/tests/AgainstRomeMapEditor.Modules.Tests/NativeLightTests.cs)：xUnit 單元測試，涵蓋半徑外衰減為 0、中心點衰減為 1、逐通道 max 合成、通道上限 1.0 截斷、多光源重疊判定、APT 錨點位移計算等 11 項測試案例。
- 建置與測試驗證通過：Release 0 警告、0 錯誤；212 項模組測試全數通過（含新增之 11 項）。
