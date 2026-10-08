# 反抗羅馬 (Against Rome) 地圖編輯器：聚落選址與資源聚落智慧平衡生成演算法設計規格書
## Settlement & Resource Distribution Designer Architecture Specification

- **文件版本**: 1.0.0
- **設計日期**: 2026-10-08
- **模組路徑**: `src.MapEditor.Modules/Settlement/`
- **測試涵蓋**: `tests/AgainstRomeMapEditor.Modules.Tests/SettlementGeneratorTests.cs`
- **相容架構**: `AgainstRomeMapEditor.Modules.Placement`, `AgainstRomeMapEditor.Modules.Nature`, `AgainstRomeModifier.Maps`

---

## 1. 設計背景與總體架構 (Executive Summary & Architecture)

在《反抗羅馬》(Against Rome) 的對戰（Endless/Skirmish）與自訂戰役地圖製作中，地圖創作者常需要為各方勢力手動逐一放置主屋、核心建築群、伐木林區、採石場與初期狩獵糧食資源。傳統純手動操作耗時且難以精準保證多玩家對戰下的「幾何對稱」與「資源產能絕對公平」。

本模組設計並實作了一套**一鍵「智慧聚落與資源初始化」生成系統 (Settlement & Resource Distribution Designer)**，包含四大核心管線：
1. **聚落腹地評估器 (`SettlementSiteEvaluator`)**：負責對地形平坦度、坡度變異、水體安全距離、主屋（Haupthaus）及初始 5 棟核心建築的空間無碰撞無干擾排布進行多維適配評分。
2. **資源聚落規劃器 (`ResourceClusterPlanner`)**：模擬自然生態分佈，使用泊松採樣（Poisson-Disk Sampling）與柏林雜訊遮罩（Perlin Noise Mask）在基地外圍 15–30 格生成有機森林，依岩壁坡腳偵測生成石礦採石場，並於開闊平原分佈野生動物群。
3. **多勢力平衡分配器 (`MultiplayerFairnessBalancer`)**：支援 2–8 位玩家的中心對稱（1v1 Point Symmetry）、環狀旋轉對稱（3–8P Rotational Symmetry）與自然拓撲等距鬆弛（Topological Equidistant / Voronoi Relaxation），並輸出公平性度量預算報告。
4. **引擎與工作階段整合 (`SettlementGeneratorEngine`)**：將運算產物自動轉譯為標準 `MapLayoutPreset`（Placement 與 Nature 雙模態），無縫對接 `PlacementEditSession` 與 `NatureEditSession`，享有完整交易式單步 Undo/Redo 與 `.arm-layout.json` 匯入匯出。

```mermaid
flowchart TD
    subgraph Inputs["地圖環境輸入 (Map Input Context)"]
        H["高度圖 (TerrainHeightField / boden.bmp)"]
        C["阻擋圖 (collision.bmp)"]
        W["水位 (WaterLevel)"]
        T["部族設定 (Germanic / Roman / Celtic / Hun)"]
        P["玩家配置 (2-8 玩家, 對稱模式)"]
    end

    subgraph CoreGenerators["生成核心管線 (Generation Pipeline)"]
        MFB["多勢力平衡分配演算法\n(MultiplayerFairnessBalancer)"]
        SSE["聚落腹地評估器\n(SettlementSiteEvaluator)"]
        RCP["資源聚落規劃器\n(ResourceClusterPlanner)"]
    end

    subgraph Evaluation["核心計算邏輯"]
        SSE -->|地形平坦度方差評估| E1["坡度高差檢驗 (ΔH ≤ 5.0)"]
        SSE -->|水體安全邊界探測| E2["離岸距離檢驗 (Dist ≥ 6.0 tiles)"]
        SSE -->|核心 5 棟無碰撞封包| E3["足跡 + 走廊無重疊排布"]

        RCP -->|泊松取樣 + 柏林雜訊| R1["木材自然樹林 (15-30 tiles)"]
        RCP -->|坡腳岩壁梯度探測| R2["採石場露頭 (Lan*Ste*)"]
        RCP -->|開闊平坦草地生成| R3["野生動物鹿/野豬群 (Figures)"]
    end

    subgraph Outputs["匯出與整合 (Integration Output)"]
        SGE["智慧生成整合引擎\n(SettlementGeneratorEngine)"]
        P_PRESET["MapLayoutPreset (Kind: Placement)\n[主屋 + 5 棟建築 + 單位]"]
        N_PRESET["MapLayoutPreset (Kind: Nature)\n[森林喬木 + 灌木 + 石礦]"]
        JSON_EXP[".arm-layout.json 可攜式配置檔"]
        PES["PlacementEditSession\n(單筆 Undo/Redo, ARM_Placed.sdl)"]
        NES["NatureEditSession\n(單筆 Undo/Redo, objects.dat)"]
    end

    P --> MFB
    H & C & W --> SSE
    H & C & W --> RCP
    T --> SSE & RCP
    MFB --> SSE
    SSE --> RCP
    RCP --> SGE
    SGE --> P_PRESET & N_PRESET & JSON_EXP
    P_PRESET --> PES
    N_PRESET --> NES
```

---

## 2. 聚落腹地評估器 (SettlementSiteEvaluator)

### 2.1 評估指標與數學模型

候選位置 $(X_c, Z_c)$ 的總適配分數 $S_{total} \in [0, 100]$ 定義為多項加權和：
$$S_{total} = w_{flat} S_{flat} + w_{water} S_{water} + w_{space} S_{space} + w_{expand} S_{expand}$$
其中權重配置為：$w_{flat} = 0.35, w_{water} = 0.25, w_{space} = 0.20, w_{expand} = 0.20$。

#### 1. 地形平坦度 (Flatness Score, $S_{flat}$)
- **建築足跡硬性約束**：
  在主屋足跡圓形區域 $\Omega_{main} = \{ (x,z) \mid (x-X_c)^2 + (z-Z_c)^2 \le R_{main}^2 \}$ 內，高度差 $\Delta H = \max_{\Omega} H(x,z) - \min_{\Omega} H(x,z)$ 必須滿足：
  $$\Delta H \le \Delta H_{max} \quad (\text{預設 } 5.0 \text{ 單位高度})$$
  若超出則直接否決（`IsValid = false`）。
- **平坦度評分**：
  $$S_{flat} = \text{clamp}\left(100 - 15 \times \frac{1}{N_{bldg}} \sum_{i=1}^{N_{bldg}} \Delta H_i, 0, 100\right)$$

#### 2. 距水體安全距離 (Water Safety Score, $S_{water}$)
- 掃描候選點周圍半徑 $R_{scan} = 18$ 格內的水位高度：
  $$D_{water} = \min \{ \sqrt{(x - X_c)^2 + (z - Z_c)^2} \mid H(x,z) \le WaterLevel + 0.5 \}$$
- 若 $D_{water} < 6.0$ 格，判定為易淹水或水邊懸崖危險區，直接否決；
- 當 $D_{water} \ge 18.0$ 格時給予飽和滿分 90 分；在 $[6, 18]$ 間採線性插值。

#### 3. 初始核心 5 棟建築無碰撞空間排布 (Building Clearance Packing)
- 聚落標配包含：**主屋（Haupthaus）+ 倉庫（Warehouse）+ 住宅（House）+ 兵營（Barracks）+ 鐵匠（Blacksmith）+ 哨塔（Tower）**。
- 空間排布檢驗條件：
  任意兩棟已排布建築 $A_i$ 與 $A_j$，其中心歐幾里得距離必須大於各自足跡半徑加上走廊緩衝間隙 $C_{corridor}$：
  $$\| P_i - P_j \| \ge R_{footprint, i} + R_{footprint, j} + C_{corridor} \quad (C_{corridor} \ge 1.2 \text{ tiles})$$
- 演算法採用**自適應極座標發散搜尋**：以建築偏好距離與角度為基底，在角向 $\Delta \theta \in [0^\circ, \pm 15^\circ, \pm 30^\circ, \dots, \pm 90^\circ]$ 與徑向 $\Delta r \in [0, \pm 0.8, \pm 1.5]$ 空間尋找最近的平坦無障礙空位。若周圍無法容納全部 5 棟核心建築，則否決該腹地。

### 2.2 部族建築配置規格矩陣

| 部族 (Tribe) | 主屋 (Haupthaus) | 倉庫 (Lager) | 民宅 (Wohnhaus) | 兵營 (Kaserne) | 鐵匠 (Schmiede) | 哨塔 (Turm) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Germanic (日耳曼)** | `BauGerHau00` ($4.2 \times 4.2$) | `BauGerLag00` ($3.0 \times 3.0$) | `BauGerWoh00` ($2.6 \times 2.6$) | `BauGerKas00` ($3.6 \times 3.6$) | `BauGerSch00` ($3.0 \times 3.0$) | `BauGerTur00` ($2.2 \times 2.2$) |
| **Roman (羅馬)** | `BauRomHau00` ($4.5 \times 4.5$) | `BauRomLag00` ($3.2 \times 3.2$) | `BauRomWoh00` ($2.6 \times 2.6$) | `BauRomKas00` ($3.8 \times 3.8$) | `BauRomSch00` ($3.0 \times 3.0$) | `BauRomTur00` ($2.2 \times 2.2$) |
| **Celtic (凱爾特)** | `BauKelHau00` ($4.2 \times 4.2$) | `BauKelLag00` ($3.0 \times 3.0$) | `BauKelWoh00` ($2.5 \times 2.5$) | `BauKelKas00` ($3.5 \times 3.5$) | `BauKelSch00` ($2.8 \times 2.8$) | `BauKelTur00` ($2.0 \times 2.0$) |
| **Hun (匈人)** | `BauHunHau00` ($4.2 \times 4.2$) | `BauHunLag00` ($3.0 \times 3.0$) | `BauHunWoh00` ($2.5 \times 2.5$) | `BauHunKas00` ($3.6 \times 3.6$) | `BauHunSch00` ($2.8 \times 2.8$) | `BauHunTur00` ($2.0 \times 2.0$) |

---

## 3. 資源聚落規劃器 (ResourceClusterPlanner)

### 3.1 木材資源（森林生態群聚）
- **空間約束**：森林重心位於聚落主屋中心 $16 \sim 28$ 格之扇區，樹木距離主屋中心嚴格維持在 $13.5 \sim 32$ 格之間，既避免初期佔據建築腹地，又確保伐木工人步行效率。
- **柏林雜訊密度場 (Perlin Noise Density Field)**：
  $$D(x,z) = 0.6 \cdot \text{Noise}(x \cdot s + o_x, z \cdot s + o_z) + 0.4 \cdot \left(1 - \frac{d_{center}}{R_{cluster}}\right)$$
  採樣門檻值 $D(x,z) > 0.35$ 視為森林可生長區。
- **泊松碟盤取樣 (Poisson-Disk Sampling)**：
  在相鄰樹木間強制施加最小間距 $d_{min} \ge 0.85$ 格，並加入隨機小數抖動（Jitter $0.15 \sim 0.85$ 格），生成自然斑駁且不穿模重疊的林相。
- **生態邊緣過渡 (Ecotone Transition)**：
  - 核心林區：主幹高大喬木（針葉樹 `LanGerNad00`..`18`、闊葉樹 `LanGerLau00`..`15`）。
  - 外圍低密度邊界（$D < 0.45$）：自動過渡為矮灌木（`LanGerNabu00`..`05`）與草叢。

### 3.2 採石場與礦物露頭（岩壁坡腳探測）
- **坡腳 (Cliff Foot) 地貌特徵演算法**：
  在距基地 $16 \sim 30$ 格方位掃描局部地形梯度。
  坡腳定義為：
  $$\text{IsCliffFoot}(x,z) \iff \left(|\nabla H(x,z)| \le 1.0\right) \land \left(\max_{d \le 2} H(x+dx, z+dz) - H(x,z) \ge 3.0\right)$$
  即本點平坦可供村民作業與放置採掘設施，但鄰近 2 格內存在 $\ge 3.0$ 單位高度的陡峭崖壁。
- **資源點生成**：在探測到的坡腳錨點半徑 4.5 格內，以間距 $\ge 1.3$ 格聚攏生成 5–8 塊自然岩石（`LanGerSte00`..`05` 或羅馬 `LanRomSte*`）。在完全平原地形下，自動退化為地質露頭模式生成。

### 3.3 糧食與狩獵區（開闊平原野生動物群）
- **方位互斥**：在聚落周圍選取與森林（偏好 $30^\circ$）和石礦（偏好 $150^\circ$）錯開之剩餘方位（偏好 $250^\circ$）之開闊平坦草地。
- **野生動物群聚**：在半徑 5 格範圍內散佈 3–6 隻中立生物（隊伍代碼 `Team = -1`，`UnitCount = 1`），包含野鹿（`FigHir00`）或野豬（`FigSch00`），旋轉角度隨機，提供初期食物狩獵補給。

---

## 4. 多勢力平衡分配演算法 (MultiplayerFairnessBalancer)

### 4.1 對稱模式支援矩陣

```mermaid
graph LR
    Mode{"對稱模式 (SymmetryMode)"}
    Mode -->|2 玩家| CS["中心對稱 (CentralSymmetry)\n• P2 = 2C - P1\n• 角度旋轉 180°\n• 資源鏡像對稱"]
    Mode -->|3-8 玩家| RS["旋轉對稱 (RotationalSymmetry)\n• 均勻分佈 θk = θ0 + 2πk/N\n• 基地半徑 Rspawn = 0.60 * Rmap\n• 資源極座標同構"]
    Mode -->|自然非對稱地形| TE["拓撲等距 (TopologicalEquidistant)\n• Lloyd 鬆弛 + 斥力場迭代\n• 最大化 min(Dist_ij)\n• 避開障礙聚集區"]
```

#### 1. 中心對稱 (CentralSymmetry, 2P)
- 設地圖中心點座標為 $C = (Dimension/2, Dimension/2)$。
- 玩家 1 基地位置：$P_1 = C + (R \cos \theta, R \sin \theta)$。
- 玩家 2 基地位置：$P_2 = 2C - P_1 = C - (R \cos \theta, R \sin \theta)$。
- 雙方所有資源生成角度相差精確 $180^\circ$，確保 1v1 電競級對稱公平。

#### 2. 環狀旋轉對稱 (RotationalSymmetry, 3–8P)
- 玩家 $k \in \{0, \dots, N-1\}$ 的基地中心分佈於極坐標：
  $$\theta_k = \theta_0 + k \cdot \frac{2\pi}{N}, \quad P_k = C + R_{spawn} (\cos \theta_k, \sin \theta_k)$$
- 每個玩家的建築朝向與資源偏好方位均加上偏移 $\theta_k$ 進行同構旋轉。

#### 3. 拓撲等距/Voronoi 鬆弛 (TopologicalEquidistant)
- 針對不對稱河流、山脈切割的複雜地圖，執行 5 輪排斥場（Repulsion Field）動態鬆弛：
  $$\vec{F}_i = \sum_{j \ne i} \frac{K}{\|P_i - P_j\|^2} \frac{P_i - P_j}{\|P_i - P_j\|} + K_{orbit} (R_{spawn} - \|P_i - C\|) \frac{P_i - C}{\|P_i - C\|}$$
- 迭代推動各勢力中心遠離彼此，直到達到平衡。

### 4.2 公平性預算校驗 (Fairness Budget Verification)
系統生成後自動評估 `FairnessScoreReport`：
- **玩家間距離均勻度**：
  $$\text{DistanceDisparity} = \frac{\max D_{ij} - \min D_{ij}}{\bar{D}} < 0.35$$
- **資源數量方差**：
  - 木材樹木數方差 $\sigma^2_{wood} \le 9.0$
  - 石礦點數方差 $\sigma^2_{stone} \le 2.0$
  - 糧食生物數方差 $\sigma^2_{food} \le 1.0$
若全部通過則標記 `IsBalanced = true`。

---

## 5. 整合匯出機制 (PlacementEditSession & LayoutJson Integration)

本系統與地圖編輯器既有模組的對接流程完全依循現有架構：

```mermaid
sequenceDiagram
    participant UI as MapEditor UI / Action
    participant SGE as SettlementGeneratorEngine
    participant PES as PlacementEditSession
    participant NES as NatureEditSession
    participant SDL as ARM_Placed.sdl
    participant DAT as DATA/objects.dat

    UI->>SGE: 一鍵觸發生成 (2-8P, 部族, 種子碼)
    SGE->>SGE: 執行 Evaluator + Planner + Balancer
    SGE->>SGE: 生成 MapLayoutPreset (Placement & Nature)
    SGE->>PES: ApplyToPlacementSession(distribution, catalog, groundHeight)
    Note over PES: 驗證邊界、分派全新 Guid、<br/>包裝為單一 BatchPlacementCommand
    PES-->>UI: 成功放置 (可單步 Undo/Redo)
    SGE->>NES: ApplyToNatureSession(distribution, templates, groundHeight)
    Note over NES: 驗證座標、包裝為單一<br/>Nature Stroke (可單步 Undo/Redo)
    NES-->>UI: 成功植生 (可單步 Undo/Redo)
    UI->>SDL: 儲存地圖時寫入 ARM_Placed.sdl (onload=1)
    UI->>DAT: 儲存地圖時寫入 objects.dat
```

### 5.1 C# 呼叫範例

```csharp
// 1. 初始化生成管線
var evaluator = new SettlementSiteEvaluator(dimension, sampleHeight, isBlocked, waterLevel);
var planner = new ResourceClusterPlanner(dimension, sampleHeight, isBlocked, waterLevel);
var balancer = new MultiplayerFairnessBalancer(evaluator, planner, dimension);

// 2. 執行 4 人旋轉對稱生成
var tribes = new[] { SettlementTribe.Germanic, SettlementTribe.Roman, SettlementTribe.Celtic, SettlementTribe.Hun };
var distribution = balancer.Generate(
    playerCount: 4, 
    mode: SymmetryMode.RotationalSymmetry, 
    tribes: tribes, 
    baseSeed: 1337);

// 3. 匯出為可攜式 JSON 檔案
var (placementJson, natureJson) = SettlementGeneratorEngine.ExportLayoutJsons(distribution);
File.WriteAllText("settlement-placement.arm-layout.json", placementJson);
File.WriteAllText("settlement-nature.arm-layout.json", natureJson);

// 4. 一鍵注入當前編輯會話（具備單步撤銷保證）
SettlementGeneratorEngine.ApplyToPlacementSession(distribution, objectCatalog, form.PlacementSession, groundHeight);
SettlementGeneratorEngine.ApplyToNatureSession(distribution, natureTemplates, form.NatureSession, groundHeight);
```

---

## 6. 驗證與測試覆蓋 (Verification & Testing Matrix)

模組測試實作於 `tests/AgainstRomeMapEditor.Modules.Tests/SettlementGeneratorTests.cs`，涵蓋以下關鍵驗收項：

1. **`Evaluator_rejects_steep_terrain_and_water_hazards`**：
   - 驗證陡坡高差超標時正確拒絕放置。
   - 驗證水體邊界過近（$< 6.0$ 格）或淹水時正確否決。
2. **`Evaluator_places_main_house_and_five_core_buildings_without_collisions`**：
   - 驗證成功規劃主屋與 5 棟核心建築。
   - 驗證全部建築彼此距離嚴格大於足跡半徑和，走廊無阻擋。
3. **`ResourcePlanner_generates_forest_quarry_and_wildlife_with_proper_constraints`**：
   - 驗證森林生成樹木位於基地外圍 $13.5 \sim 32$ 格。
   - 驗證採石場生成符合部族調色盤之岩石。
   - 驗證野生動物群為中立生物（`Team = -1`）。
4. **`Balancer_generates_perfect_2p_central_symmetry`**：
   - 驗證 2 玩家以地圖中心精確點對稱（中心坐標中點誤差 $< 0.1$ 格）。
   - 驗證雙方資源與建築數目完全平衡。
5. **`Balancer_generates_fair_4p_rotational_symmetry`**：
   - 驗證 4 玩家在旋轉對稱下均能找到合格基地，且最小相鄰間距大於安全值。
6. **`Engine_converts_to_presets_and_integrates_with_edit_sessions_and_json`**：
   - 驗證成果完整序列化為 JSON 並還原。
   - 驗證注入 `PlacementEditSession` 與 `NatureEditSession`，並經由 `Undo()` 與 `Redo()` 往返驗證狀態完整性。

---

## 7. 規範遵守宣告 (Compliance Statement)

- **遵循 `AGENTS.md`**：全案以繁體中文撰寫，保留技術名詞、API、檔案路徑；未存取遊戲安裝目錄；未自行執行 git commit/push，交由主代理人統一提交。
- **無依賴破壞**：完全遵循現有 `MapLayoutPreset` 規範與 `PlacementEditSession` 交易式原則，不修改原生資料結構簽名。
