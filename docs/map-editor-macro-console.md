# 地圖編輯器巨集控制台與座標空間規格書 (Map Editor Macro Console & Coordinate Contract)

本文件定義《反抗羅馬》(Against Rome) 地圖編輯器巨集控制台指令、底層編輯工作階段 (Sessions) 以及尋路導航網格 (NavMesh/Pathfinding) 系統的統一坐標空間規格與單位換算合約。

---

## 1. 核心坐標空間矩陣 (Coordinate Space Contracts)

《反抗羅馬》引擎內部的地圖由四個不同解析度的網格層與連續世界空間共同構成。標準地圖尺寸為 64×64 圖塊 (Tiles)，世界空間範圍為 0.0 至 16384.0 單位。

| 空間名稱 | 網格解析度 | 單元跨度 (World Units) | 坐標範圍 (Index) | 主要儲存來源 / 用途 |
|---|---|---|---|---|
| **世界連續空間 (World Space)** | 連續浮點數 | 1.0 (最小單位) | $X, Z \in [0.0, 16384.0]$<br>$Y \in [0.0, 1024.0]$ | 物件放置 (SDL Objects)、自然植被 (Nature Additions)、相機位置、物理判定 |
| **圖塊紋理網格 (Tile Grid)** | $64 \times 64$ | 256.0 單位 / 圖塊 | $X, Z \in [0, 63]$ | `floor.dat` / `_texturesDocument`，地面材質圖塊 (Gras, Erde, WEG, Fels) |
| **高程頂點網格 (Vertex Grid)** | $257 \times 257$ | 64.0 單位 / 頂點間距 | $X, Z \in [0, 256]$ | `boden.bmp` 綠色通道 (Byte 0..255)，$(64 \times 4) + 1 = 257$ 頂點 |
| **碰撞通行網格 (Collision Grid)** | $256 \times 256$ | 64.0 單位 / 像素 | $X, Z \in [0, 255]$ | `collision.bmp` (Byte 0 = 可通行, 255 = 障礙物)，每個圖塊對應 $4 \times 4$ 像素 |

### 空間換算公式
1. **世界空間 $\leftrightarrow$ 圖塊網格 (Tile Space)**
   $$\text{TileX} = \lfloor \text{WorldX} / 256.0 \rfloor, \quad \text{TileZ} = \lfloor \text{WorldZ} / 256.0 \rfloor$$
   $$\text{WorldX} = (\text{TileX} + 0.5) \times 256.0, \quad \text{WorldZ} = (\text{TileZ} + 0.5) \times 256.0 \quad (\text{中心點})$$

2. **圖塊網格 $\leftrightarrow$ 碰撞像素 (Collision Pixel Space)**
   $$\text{CollisionPixelX} = \text{TileX} \times 4, \quad \text{CollisionPixelZ} = \text{TileZ} \times 4$$
   每個圖塊覆蓋的碰撞像素範圍為：
   $$px \in [\text{TileX} \times 4, \text{TileX} \times 4 + 3], \quad pz \in [\text{TileZ} \times 4, \text{TileZ} \times 4 + 3]$$

3. **高程場 (Height) 位元組 $\leftrightarrow$ 世界高度 (World Y)**
   $$\text{World Y} = \text{HeightByte} \times \text{HeightmapStep} \quad (\text{預設 } \text{HeightmapStep} = 4.0)$$
   $$\text{HeightByte} = \text{clamp}\left(\lfloor \text{World Y} / \text{HeightmapStep} \rceil, 0, 255\right)$$
   > **注意**：`boden.ini` 中的 `Waterlevel` 鍵值儲存的是**世界高度值**（例如 40.0），而非未縮放之位元組。

---

## 2. 巨集控制台指令座標合約 (Macro Command Contracts)

巨集控制台 (`EditorConsoleControl`) 與腳本執行器 (`MacroScriptRunner`) 提供全套自動化指令，各指令嚴格遵循下述輸入單位與坐標空間規範：

### 2.1 高程類指令 (Terrain Elevation)

#### `/elevate`
- **分類**：Terrain
- **語法**：`/elevate rect <x1> <z1> <x2> <z2> <delta>` 或 `/elevate circle <cx> <cz> <radius> <delta>`
- **坐標空間**：**高程頂點空間 (Vertex Grid: 0..256)**
- **高度單位**：高程位元組增量 $\Delta h \in [-255, 255]$
- **說明**：直接在 `TerrainHeightEditSession` 上批量遞增或遞減指定矩形或圓形範圍內的頂點高度位元組。支援單步撤銷 (`/undo`)。

#### `/flatten`
- **分類**：Terrain
- **語法**：`/flatten rect <x1> <z1> <x2> <z2> [height]` 或 `/flatten circle <cx> <cz> <radius> [height]`
- **坐標空間**：**高程頂點空間 (Vertex Grid: 0..256)**
- **高度單位**：目標高程位元組 $h \in [0, 255]$（省略時取中心點現存取樣高度）
- **說明**：將區域內所有頂點的高程平整為指定位元組高度。

### 2.2 材質與地景類指令 (Textures & Nature)

#### `/replace-texture` (別名: `/swap-tex`, `/subst-tex`)
- **分類**：Terrain
- **語法**：`/replace-texture <oldTexture> <newTexture> [--rect <x1,z1,x2,z2>]`
- **坐標空間**：**圖塊網格空間 (Tile Grid: 0..63)**
- **說明**：在全圖或指定圖塊矩形範圍內批量替換底層材質圖塊。每行跨度由 `blendSession.TileDimension` (64) 精確驅動，更新後同步刷新 2D 畫布與 3D 視圖。

#### `/scatter` (別名: `/plant`, `/populate`)
- **分類**：Nature / Placement
- **語法**：`/scatter <template> <count> <rect> [--spacing <N>] [--seed <N>]`
- **坐標空間**：**世界空間 (World Space: 0..16384)**
- **矩形格式**：`x1,z1,x2,z2`（例如 `1000,1000,3000,3000` 為世界坐標範圍）
- **間距單位**：世界單位（預設 48.0）
- **高程處理**：自動透過 `context.GetHeightAtWorld(px, pz)` 取樣地表頂點並乘以 `HeightStep`，賦予物件貼地的世界高度 $Y$。

### 2.3 放置與隊伍類指令 (Placement & Team)

#### `/spawn-ring` (別名: `/ring`, `/circle-spawn`)
- **分類**：Placement
- **語法**：`/spawn-ring <template> <centerX> <centerZ> <radius> <count> [--team <teamId>] [--angle <outward|inward|tangent|fixed>]`
- **坐標空間**：**世界空間 (World Space: 0..16384)**
- **半徑單位**：世界單位（如 256.0 相當於 1 個圖塊寬度）
- **說明**：圍繞世界中心點等間距擺放建築或部隊，自動貼地並計算旋轉朝向角度。

#### `/align-grid` (別名: `/snap-grid`, `/grid`)
- **分類**：Placement
- **語法**：`/align-grid [step=32] [--selected]`
- **單位**：網格吸附步長（世界單位，如 32.0 或 64.0）
- **說明**：將物件世界坐標 $(X, Z)$ 吸附至指定步長的倍數，並自動重算貼地高度 $Y$。

#### `/set-team` (別名: `/change-team`, `/assign-team`)
- **分類**：Placement
- **語法**：`/set-team <teamId> [--selected] [--rect <x1,z1,x2,z2>]`
- **矩形坐標**：**世界空間 (World Space: 0..16384)**
- **隊伍範圍**：$-1 \sim 15$

#### `/select-team`
- **分類**：Placement
- **語法**：`/select-team <teamId> [--filter <figure|building|all>]`

### 2.4 通行性與診斷類指令 (Pathfinding & Diagnostics)

#### `/heal-navmesh` (別名: `/heal-roads`, `/fix-navmesh`, `/repair-roads`)
- **分類**：Pathfinding
- **語法**：`/heal-navmesh [--mode <gaps|bridges|all>] [--max-gap <1|2>]`
- **坐標空間**：**圖塊網格空間 (Tile Grid: 0..63)** 偵測道路斷點；自動映射至 **碰撞像素空間 (0..255)** 完整清除 $4 \times 4$ 通行障礙。
- **說明**：掃描道路網中 1~2 格的微小拓撲間隙，補齊相應風格圖塊（Standard, Roman, Dirt），並將該圖塊對應的 16 個碰撞像素全數設為可通行 (0)。

#### `/diagnose` (別名: `/check`, `/audit`, `/lint`)
- **分類**：Diagnostics
- **語法**：`/diagnose [--severity <all|error|warning>]`
- **說明**：執行跨空間健康檢查（物件世界邊界、碰撞重疊、事件目標 GUID、孤立陸地、道路缺口與方陣通道瓶頸）。

---

## 3. NavMesh 與尋路修復系統架構合約

```mermaid
flowchart TD
    subgraph TileSpace["圖塊空間 (64x64 Tiles)"]
        RoadTextures["道路紋理 (H_WEG, V_WEG, ROM...)"]
        RoadGapDetector["RoadGapDetector\n偵測 1~2 格道路缺口"]
    end

    subgraph CollisionSpace["碰撞空間 (256x256 Pixels)"]
        CollisionBmp["collision.bmp 阻擋層"]
        BodenHeights["boden.bmp 高度場 (257x257)"]
        PassabilityGrid["NavMeshPassabilityGrid\n(Size = 256, TileScale = 64)"]
        Connectivity["NavMeshConnectivityAnalyzer\n4-向 BFS Flood Fill 孤立分量"]
    end

    subgraph WorldSpace["世界連續空間 (16384x16384)"]
        Troops["部隊與建築 (Spawn.X, Spawn.Z)"]
        Corridors["FormationWidthValidator\n1~20人方陣通道淨寬評估"]
    end

    RoadTextures --> RoadGapDetector
    CollisionBmp --> PassabilityGrid
    BodenHeights --> PassabilityGrid
    PassabilityGrid -.->|IsTileBlocked / IsTileSubmerged\n(Scale = 4.0)| RoadGapDetector
    RoadGapDetector --> Actions["NavMeshRepairAction 清單\n(TileX, TileZ 屬於 0..63)"]

    Troops --> PassabilityGrid
    PassabilityGrid --> Connectivity
    Connectivity --> Actions

    Actions --> Healer["RoadPathHealer.ApplyRepair"]
    Healer -->|StampTexture| BlendSession["TerrainBlendEditSession (Tile 0..63)"]
    Healer -->|PaintCollision 4x4 Pixels| HeightSession["TerrainHeightEditSession (Pixel 0..255)"]
```

### 3.1 跨層比例與查詢規格
- **圖塊到碰撞像素步長**：
  $$\text{step} = \frac{\text{CollisionSize}}{\text{TileDimension}} = \frac{256}{64} = 4$$
- **圖塊碰撞阻擋檢查** (`NavMeshPassabilityGrid.IsTileBlockedByCollision`)：
  檢查該圖塊對應的 $4 \times 4$ 像素區間內是否存在任何 `collision != 0` 的像素。
- **圖塊水下判定** (`NavMeshPassabilityGrid.IsTileSubmerged`)：
  取圖塊中心點 $(\text{TileX} + 0.5, \text{TileZ} + 0.5)$ 對應之頂點高度，比對是否低於 `Waterlevel`。
- **原子化修復與復原** (`RoadPathHealer.ApplyRepair`)：
  - 鋪路：調用 `blendSession.StampTexture(TileX, TileZ, texture)`。
  - 破障：將該圖塊對應的全部 16 個碰撞像素 $(\text{TileX} \times 4 + px, \text{TileZ} \times 4 + pz)$ 清除為 0。
  - 撤銷：兩者均納入 Session 歷史堆疊，執行 `/undo` 時紋理與 16 個碰撞像素一併原子回滾。

---

## 4. 控制台上下文初始化規範 (Host Integration)

宿主表單 (`MapEditorForm.Console.cs`) 在每次執行指令前觸發 `RefreshConsoleContext()`，必須確保填入當前地圖的精確規格：

```csharp
int tileDim = _texturesDocument?.Dimension ?? _terrainBlendSession?.TileDimension ?? 64;
context.MapTileDimension = tileDim;               // 正確設為 64 (非預設 256)
context.WorldDimension = tileDim * 256f;          // 正確設為 16384.0f
context.HeightStep = _heightMapStep;              // 通常為 4.0f
context.WaterLevel = (float)_waterLevel.Value;    // 世界高度值
```

此規範確保巨集指令在任何自訂解析度的地圖上皆能維持正確的座標對齊與邊界約束。
