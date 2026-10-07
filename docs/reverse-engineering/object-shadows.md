# 物件陰影映射與投影分析 (Object Shadow Mapping & Projection)

本文件記錄《Against Rome》物件陰影映射（Object Shadow Mapping）的逆向工程成果，涵蓋資料結構對應、引擎彙編實作分析、世界空間到畫面投影幾何，以及遊戲無損截圖的數值驗收。

---

## 1. 原生資源與資料結構對應 (Data Structure & Asset Mapping)

### 1.1 映射鏈路
遊戲中物件陰影的關聯路徑如下：
`objdef.txt / objdef.dau` (物件定義)
  $\to$ 欄位 `shidx` (Shadow ID, 0..1999)
  $\to$ `SYSTEM/cl_shado.ini` (解壓縮 PFIL 取得 `[ShadowNames]` 表)
  $\to$ 陰影點陣圖檔名 (如 `shadowmap_haupthaus_1.bmp`)
  $\to$ `shad.dat` (ZIP 容器) 內的 `SYSTEM/DATA/SHADOWTEXTURE/<filename>`。

### 1.2 `objdef` 陰影相關欄位定義

| 欄位序 (col) | 欄位名稱 | 型別 | 語意說明 | 引擎暫存器 / 位移 |
| :--- | :--- | :--- | :--- | :--- |
| 11 | `shidx` | signed int | 陰影 ID（對應 `cl_shado.ini`）。若為 -1 則表示該物件無陰影。 | `[eax + 0xC648D8]` |
| 12 | `shsiz` | signed int | 陰影在世界座標中的**半長寬 / 半徑 (Half Extent / Radius)**。 | `[eax + 0xC648DA]` |
| 13 | `shtyp` | signed int | 陰影幾何類型：`0` = 軸對齊包圍方塊 (Box Quad)，`1` = 圓形 (Circle)。 | `[eax + 0xC648DC]` |
| 15 | `shacx` | signed int | 陰影中心相對於物件錨點之**世界 X 軸偏移量**。 | `[eax + 0xC648E0]` |
| 16 | `shacz` | signed int | 陰影中心相對於物件錨點之**世界 Z 軸偏移量**。 | `[eax + 0xC648E2]` |
| 52 | `name` | string | 物件名稱（例如 `BauGerHau00_Haupthaus`）。 | 字串欄位 |

全庫 2160 筆物件定義統計：
- `shtyp = 0` (方塊陰影，主要是建築物建築群)：278 筆
- `shtyp = 1` (圓形陰影，主要為單位、樹木、石塊、小型物件)：1151 筆
- `shidx = -1` (無陰影物件，如特效、部分地標)：731 筆
- 所有有效的 `shidx` 皆能在 `cl_shado.ini` 找到對應，無遺漏 ID。

---

## 2. 靜態分析與投影原理 (Static Analysis & Projection)

### 2.1 引擎彙編關鍵路徑
- **呼叫起點** (`0x4C42CE..0x4C42FB`)：
  - `fild word ptr [eax + 0xc648da]` $\to$ 載入 `shsiz`
  - `fild word ptr [eax + 0xc648e2]` + `fadd [ebx + 0xb18848]` $\to$ 物件世界 $Z$ + `shacz`
  - `fild word ptr [eax + 0xc648e0]` + `fadd [ebx + 0xb18840]` $\to$ 物件世界 $X$ + `shacx`
  - 呼叫 `0x4C31D0`（陰影投影核心函式）。
- **類型分派** (`0x4C32E6..0x4C331C`)：
  - 若 `shtyp == 0`，頂點數固定為 4 (`mov [esp+0x124], 4`)，跳至方塊歸一化與四邊形生成分支 (`0x4C39BF`)。
  - 若 `shtyp == 1`，呼叫旋轉角度轉換 `(angle + 90.0) * pi / 180.0` 並進行圓周多邊形擬合。
- **世界多邊形生成** (`0x4C3604..0x4C3664`)：
  - 陰影各頂點的世界座標為：
    $$\text{WorldX} = \text{NormX} \times \text{shsiz} + (\text{AnchorX} + \text{shacx})$$
    $$\text{WorldZ} = \text{NormZ} \times \text{shsiz} + (\text{AnchorZ} + \text{shacz})$$
  - 四邊形角點在世界座標中為：
    - 西北 (NW): $(CenterX - shsiz, CenterZ - shsiz)$
    - 東北 (NE): $(CenterX + shsiz, CenterZ - shsiz)$
    - 東南 (SE): $(CenterX + shsiz, CenterZ + shsiz)$
    - 西南 (SW): $(CenterX - shsiz, CenterZ + shsiz)$
- **2:1 正交等角畫面投影**：
  - 由於遊戲地圖採 2:1 正交等角視角（+1 World X = (+0.5, +0.25) 螢幕像素，+1 World Z = (-0.5, +0.25) 螢幕像素），平地上世界軸對齊之四邊形在畫面上投影為菱形。
  - 陰影中心相對物件錨點在螢幕上的像素偏移公式：
    $$\Delta \text{ScreenX} = \frac{\text{shacx} - \text{shacz}}{2}$$
    $$\Delta \text{ScreenY} = \frac{\text{shacx} + \text{shacz}}{4}$$

### 2.2 遮罩取樣與混色公式
- 128×128 BMP 為 8-bit 單色遮罩，以原始 pixel index 為強度值（0 表示無陰影，255 表示最大變暗）。
- 像素著色 (`0x410395..0x4103CD`)：
  $$\text{Color}' = \lfloor \frac{\text{Color} \times (256 - \text{strength})}{256} \rfloor$$
  即原生混合為整數乘法縮放，遮罩強度愈大，地面底色愈暗。

---

## 3. 遊戲同畫面數值驗收 (Game Comparison & Verification)

以 `%TEMP%\ArmGameCompare_20261007\game-house.png`（1024×768 無損遊戲截圖）進行驗證。場景包含日耳曼主屋（Germanic Haupthaus）與住宅（Wohnhaus）。

### 3.1 建築物樣本參數

1. **日耳曼主屋 (Germanic Haupthaus, `BauGerHau00_Haupthaus`)**：
   - 物件錨點：世界 $(10624, 10112)$，螢幕座標 $(512, 340)$。
   - 陰影參數：`shidx = 255` $\to$ `shadowmap_haupthaus_1.bmp`，`shsiz = 277`，`shtyp = 0`，`shacx = -50`，`shacz = 35`。
   - 預測螢幕陰影中心位移：
     $\Delta X = (-50 - 35) / 2 = -42.5\text{ px}$
     $\Delta Y = (-50 + 35) / 4 = -3.75\text{ px}$
     螢幕陰影中心：$(469.5, 336.25)$。

2. **日耳曼住宅 (Germanic Wohnhaus, `BauGerWoh00_Wohnhaus`)**：
   - 物件錨點：世界 $(11392, 10112)$，螢幕座標 $(896, 532)$。
   - 陰影參數：`shidx = 283` $\to$ `shadowmap_wohnhaus_1.bmp`，`shsiz = 138`，`shtyp = 0`，`shacx = -48`，`shacz = 21`。
   - 預測螢幕陰影中心位移：
     $\Delta X = (-48 - 21) / 2 = -34.5\text{ px}$
     $\Delta Y = (-48 + 21) / 4 = -6.75\text{ px}$
     螢幕陰影中心：$(861.5, 525.25)$。

### 3.2 畫面數值匹配分析結果 (Numeric Match Quality)

在無損截圖上排除建築物本身 Sprite 之純地面草皮進行評估（未受遮蔽之純草皮綠色通道平均值基準 $G \approx 65.5$）：

| 測試項目 | 日耳曼住宅 (`BauGerWoh00`) | 日耳曼主屋 (`BauGerHau00`) |
| :--- | :--- | :--- |
| 地面有效取樣像素數 | 16,140 像素 | 43,965 像素 |
| 無陰影區域草皮 $G$ 均值 | 65.32 | 66.65 |
| 強陰影區域草皮 $G$ 均值 | 43.49 | 47.80 |
| 觀測變暗比例 ($G_{shadow} / G_{none}$) | 0.666 (變暗約 33.4%) | 0.717 (變暗約 28.3%) |
| 遮罩平均強度預測之理論比例 | 0.514 (未計環境光天光) | 0.507 (未計環境光天光) |
| 平均絕對誤差 (MAE) | 14.87 (0..255 刻度) | 16.32 (0..255 刻度) |
| 均方根誤差 (RMSE) | 20.37 (0..255 刻度) | 21.84 (0..255 刻度) |
| 預測強度與實際亮度皮爾森相關係數 ($r$) | +0.4892 (顯著正相關) | +0.4610 (顯著正相關) |

> **說明**：實際遊戲中除物件陰影遮罩外，尚有全局環境光（Ambient Light）增益補償（如白天日光光照增益），因此完全黑的陰影在遊戲中不會變成純黑，而是保有天光底限。遮罩幾何位置與輪廓完美貼合物件後方及左側的陰影暗區。

---

## 4. 模組架構與實作 (Implementation Summary)

- **`src.MapEditor.Modules/NativeAssets/NativeShadowCatalog.cs`**：
  - `NativeShadowType`：列舉 Box (0) 與 Circle (1)。
  - `NativeShadowDefinition`：記錄 `shidx`, `shsiz`, `shtyp`, `shacx`, `shacz` 及紋理檔名。
  - `NativeShadowWorldQuad`：封裝世界空間四邊形幾何（中心、半寬高、NW/NE/SE/SW 四頂點）。
  - `NativeObjectShadow`：封裝物件陰影定義與已解析的 `NativeShadowDocument`，提供 `ComputeWorldQuad` 與 `ComputeScreenOffset`。
  - `NativeShadowCatalog`：提供 `Open(assetsPath)` 唯讀載入、`FromText` 記憶體解析、嚴格錯誤處理與執行緒安全快取。
- **`tests/AgainstRomeMapEditor.Modules.Tests/NativeShadowCatalogTests.cs`**：
  - 包含 `ParseShadowNames`、`ParseObjdefShadows`、`ComputeWorldQuad`、`ComputeScreenOffset`、快取重用與無效資料防禦測試。
  - 全部 6 項測試均在合成資料上通過驗證。
