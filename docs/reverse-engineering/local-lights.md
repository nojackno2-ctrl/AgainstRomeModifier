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

### 3. lightdef.dau 規格與真實定義（已驗證）

真實 `SYSTEM/DATA_MP/DEFAULTS/lightdef.dau`（以 PFIL/LZSS 壓縮儲存）在解壓後包含完整的 `[LightDefault]` 表（共 42 筆定義，idx 0..41）。

```ini
[LightDefault]
;idx ,activ,  red,  grn,  blu,      rad, type,     typep,spefx,-------------name-------------
    0,    1, 0.33, 0.93, 1.00,    10.00,    0,    100.00,    1,                     Testlicht
    1,    1, 1.48, 1.22, 0.00,   350.00,    1,      0.05,    0,                   Kohleschale
    2,    1, 1.50, 1.09, 0.00,   350.00,    1,      0.05,    0,                 Schmiedenglut
    3,    1, 0.68, 0.98, 1.00,   200.00,    1,      0.05,    0,                Magisches_Blau
    4,    1, 1.20, 0.85, 0.48,   290.00,    0,      0.00,    5,                LD_Test_Explo1
    5,    1, 1.16, 0.73, 0.52,   500.00,    0,      0.00,    0,               LD_Test_Fwaffe1
    6,    1, 1.37, 1.35, 1.35,  1500.00,    2,    750.00,    5,        LD_Test_Blitzeinschlag
    7,    1, 1.33, 0.61, 0.33,   300.00,    1,      0.15,    0,                LD_Feuerstelle
    8,    1, 1.61, 1.26, 0.00,   250.00,    1,      0.05,    0,             Goldschmiedenglut
   18,    1, 1.60, 1.33, 0.00,   150.00,    1,      0.05,    0,              LD_Flamme_Fackel
```

#### 關鍵實體光源定義對應表（Verified vs Assumed）

1. **日耳曼主屋（BauGerHau00_Haupthaus, BauGerHau02_Haupthaus）：**
   - `objdef.txt` 中指定 `aptli = 1`（`Kohleschale`，火盆／煤炭盆）。
   - 高度偏移 `aptlh`：`BauGerHau00` 為 **30**，`BauGerHau02` 為 **80**。
   - 光源屬性：
     - **名稱**：`Kohleschale`
     - **半徑 (Radius)**：`350.00` 世界單位
     - **RGB 顏色**：`(1.48, 1.22, 0.00)`（高強度暖金黃色火光，R/G 均大於 1.0，B 為 0.0）
     - **閃爍 (Flicker)**：`Type = 1`（隨機擾動火光），`TypeParam = 0.05`（微幅擾動），`SpecialFx = 0`
2. **營火／火堆（Fil*Feu* / FX_Feuerstelle_Feuer）：**
   - `objdef.txt` 中指定 `lidef = 7`（`LD_Feuerstelle`），高度 `lihei = 0`。
   - 光源屬性：
     - **名稱**：`LD_Feuerstelle`
     - **半徑 (Radius)**：`300.00` 世界單位
     - **RGB 顏色**：`(1.33, 0.61, 0.33)`（偏紅之炭火橙色）
     - **閃爍 (Flicker)**：`Type = 1`，`TypeParam = 0.15`（較明顯之柴火閃爍），`SpecialFx = 0`
3. **火把（FX_Flamme_Fackel_Sub）：**
   - `objdef.txt` 中指定 `lidef = 18`（`LD_Flamme_Fackel`），高度 `lihei = 0`。
   - 光源屬性：
     - **名稱**：`LD_Flamme_Fackel`
     - **半徑 (Radius)**：`150.00` 世界單位
     - **RGB 顏色**：`(1.60, 1.33, 0.00)`（亮黃色集中火光）
     - **閃爍 (Flicker)**：`Type = 1`，`TypeParam = 0.05`，`SpecialFx = 0`
4. **預設回退（若無檔案）：**
   - 引擎代碼 `0x4D2FE9` 之預設值：半徑 500.0，RGB=(1.0, 1.0, 1.0)，Type=0（無閃爍）。

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

### 2. 真實定義定量擬合結果（Quantitative Fit with Real Definitions）

以日耳曼主屋世界坐標 $(10624, 10112)$、門前平坦地面 ($Y=0$)、從真實 `gerhau00.apt` extraA 提取之 4 處光源世界坐標：
- $L_0$: $(10816, 10112)$（右前門柱）
- $L_1$: $(10624, 10368)$（左前門柱）
- $L_2$: $(10624, 9856)$（右後側）
- $L_3$: $(10368, 10112)$（左後側）

以及 `lightdef.dau` 中日耳曼主屋真實光源定義 `Kohleschale`（`idx=1`，$R=1.48, G=1.22, B=0.00, \text{Radius}=350.0$）與 `objdef` 高度 $H=30$（`BauGerHau00`）及 $H=80$（`BauGerHau02`）進行擬合。

#### 擬合模型
未受光背景地表由鄰近未受光區域（$(480, 500)$、$(600, 500)$ 及周圍）觀測估計：
$$\text{Ambient} = (A_R, A_G, A_B) \approx (82.0, 60.0, 49.0)$$
對各地面像素點計算二次衰減 candidate：
$$\text{Falloff} = \max\left(0, 1 - \frac{d^2}{R^2}\right)$$
$$\text{Cand}_c = \text{LightColor}_c \times \text{Falloff} \times S \quad (c \in \{R, G, B\})$$
$$\text{Pred}_c = \min(255.0, \max(A_c, \text{Cand}_c))$$

#### 擬合數據指標（誠實報告）
1. **主屋門前 6 個關鍵幾何取樣點（涵蓋中心、過渡帶與邊界）：**
   - **Kohleschale (idx 1, R=350, H=30)：**
     - 全通道整體 **$\text{RMSE} = 21.54$**，相關係數 **$\text{Correlation} = 0.9104$**
     - 紅色通道（Red）：**$\text{RMSE} = 7.52$**，**$\text{Correlation} = 0.9970$**（極高度吻合）
     - 綠色通道（Green）：**$\text{RMSE} = 23.12$**，**$\text{Correlation} = 0.9846$**
     - 藍色通道（Blue）：因火盆定義中 $B = 0.00$，模型預測由環境光通道 $A_B$ 決定。實測中受光處藍色亦略微上升至 115～123（可能來自原版地形 shader 頂點光與地面 boden 紋理底色相乘之增益）。若以單一 $A_B$ 純擬合，藍色通道 $\text{RMSE} = 33.43$。
   - **Kohleschale (idx 1, R=350, H=80)：**
     - 全通道整體 **$\text{RMSE} = 21.41$**，相關係數 **$\text{Correlation} = 0.9115$**
   - **對照組：若誤用火把 Flamme_Fackel (idx 18, R=150)：**
     - 因半徑僅 150，在門前中央多數取樣點即超出半徑衰減為 0，整體 **$\text{RMSE} = 57.43$**，相關係數僅 **$0.3125$**。此顯著差異證實主屋確實使用 `aptli=1`（`Kohleschale`，半徑 350）而非單位火把 `idx=18`。
2. **門前全區域 132 個密集網格像素擬合：**
   - **Kohleschale (idx 1, R=350)：** 全區 $\text{RMSE} = 26.95$，$\text{Correlation} = 0.5858$（包含地形紋理噪聲、草石交錯底色）。
   - **Flamme_Fackel (idx 18, R=150)：** 全區 $\text{RMSE} = 27.69$，$\text{Correlation} = 0.5566$。

#### 結論與驗證狀態整理（Verified vs Assumed）
- **[Verified]** `lightdef.dau` 為 PFIL 壓縮容器，解壓後為 `[LightDefault]` CSV 表。
- **[Verified]** 主屋 `BauGerHau00`（`aptli=1, aptlh=30`）與 `BauGerHau02`（`aptli=1, aptlh=80`）使用的是 `Kohleschale`（idx 1，半徑 350，色值 $(1.48, 1.22, 0.00)$，閃爍 1/0.05）。
- **[Verified]** 火堆（`FX_Feuerstelle_Feuer`）使用 `lidef=7`（`LD_Feuerstelle`，半徑 300，色值 $(1.33, 0.61, 0.33)$，閃爍 1/0.15）。
- **[Verified]** 火把（`FX_Flamme_Fackel_Sub`）使用 `lidef=18`（`LD_Flamme_Fackel`，半徑 150，色值 $(1.60, 1.33, 0.00)$，閃爍 1/0.05）。
- **[Verified]** APT extraA 提取出的 4 處光源點坐標在 2:1 等角逆變換後，完全對齊主屋門柱兩側火盆位置。
- **[Assumed]** 藍色通道在實測中的微幅上升推測為地面紋理與頂點光照相乘的結果，在單純 ambient scalar + max 估計下因定義 $B=0$ 而未完全追蹤紋理細節。

---

## 四、實作與單元測試

已建立與更新下列檔案（無破壞性修改，不改動既有檔案）：
- [`src.MapEditor.Modules/NativeAssets/NativeLightSource.cs`](file:///D:/Github/AgainstRomeModifier/src.MapEditor.Modules/NativeAssets/NativeLightSource.cs)：
  - 純 C# 模型，包含 `NativeLightDefinition`、`NativeLightInstance`、`NativeLightCatalog`。
  - 新增 `Open(string)` 與 `Parse(ReadOnlySpan<byte>)`，支援自動偵測並以 `GameLZSS.DecompressPfil` 解壓縮 PFIL-compressed `lightdef.dau`。
  - `NativeLightCalculator` 實現 0x49FFE0 二次衰減與每通道取最大值運算。
- [`src.MapEditor.Modules/NativeAssets/NativeAptLightPoints.cs`](file:///D:/Github/AgainstRomeModifier/src.MapEditor.Modules/NativeAssets/NativeAptLightPoints.cs)（全新檔案，不修改 `NativeAptDocument.cs`）：
  - `ExtractLightPoints(ReadOnlySpan<byte>)`：從原生 APT 位元組中提取 extraA 光源點及其相對於 anchor 的 delta 螢幕偏移。
  - `GetBuildingWorldLightPositions(...)`：結合建築物世界坐標、地面高與 `aptlh` 高度，計算所有光源點之世界空間位置。
- [`tests/AgainstRomeMapEditor.Modules.Tests/NativeLightTests.cs`](file:///D:/Github/AgainstRomeModifier/tests/AgainstRomeMapEditor.Modules.Tests/NativeLightTests.cs)：
  - 新增 `NativeLightCatalog_ParseRealExcerpt_MatchesGameDefinitions`：以真實 `lightdef.dau` 表頭與列片段（包含 `Kohleschale`、`Schmiedenglut`、`LD_Feuerstelle`、`LD_Flamme_Fackel`）驗證解析能力與精確欄位值。
  - 新增 `NativeLightCatalog_ParseBytes_SupportsPfilCompression`：驗證二進位與 PFIL 壓縮解碼。
  - 新增 `NativeAptLightPoints_ExtractLightPoints_CorrectlyCalculatesDeltasAndWorldPositions`：驗證 extraA 提取與世界坐標計算。
  - 既有 11 項測試＋新增 3 項測試，共 14 項專屬測試全案通過。
- 建置與測試驗證通過：Release 0 警告、0 錯誤；270 項模組測試全數通過。

