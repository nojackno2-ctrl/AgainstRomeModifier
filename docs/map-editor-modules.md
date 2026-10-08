# 地圖編輯器模組架構與平行開發 (Map Editor Modules and Parallel Development)

地圖編輯器的核心邏輯與狀態管理已全面自 WinForms 介面、OpenGL 著色器以及遊戲安裝目錄中抽離。`src.MapEditor.Modules` 目標為標準 `net8.0`，僅參考 `src.Shared` 專案；其可在編輯器主視窗 (`src.MapEditor`) 調整時獨立建置與測試。

---

## 1. 模組邊界架構 (Implemented Boundary)

所有核心狀態模組均實作 `IEditorModule<TSnapshot>` 介面，規範標準生命週期：
- `Load`：載入並初始化模組狀態。
- `Capture`：擷取當前狀態的獨立快照。
- `IsDirty`：判斷是否相對於已儲存基準線產生變更。
- `AcceptChanges`：僅在宿主的地圖儲存交易完全成功後呼叫，更新儲存基準線。
- `Reset`：捨棄待儲存變更，還原至基準線。

模組本身**不開啟實體檔案、不彈出對話框、不跨模組直接呼叫，亦不直接存取渲染器狀態**。一切協調與交易控制均由宿主表單 (`MapEditorForm`) 統一調度。

### 獨立單元測試驗證

無需啟動 Windows UI 或遊戲，即可透過命令列執行全套純模組測試：

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet test tests/AgainstRomeMapEditor.Modules.Tests -c Release
```

---

## 2. 模組狀態與實作矩陣 (Module Status Matrix)

| 模組領域 | 命名空間路徑 | 核心職責與規劃器 | WinForms UI 接入狀態 | 存檔／實機管線狀態 |
| :--- | :--- | :--- | :--- | :--- |
| **Terrain** | `AgainstRomeMapEditor`（目錄 `Terrain/`） | 高度場、地表紋理、河流規劃 (`RiverFlowPlanner`)、懸崖規劃 (`CliffFacePlanner`)、圖塊印章、自動選路 | 已接入（材質工具列、區域工具對話框、印章連續筆刷） | 透過 `TerrainBlendEditSession` 與高度層交易寫入地圖 |
| **Settlement** | `Modules.Settlement` | 基地選址 (`SettlementSiteEvaluator`)、資源叢集 (`ResourceClusterPlanner`)、多邊對稱平衡 (`MultiplayerFairnessBalancer`)、生成引擎 (`SettlementGeneratorEngine`) | 已接入（配置選單「一鍵生成對戰基地…」對話框） | 透過 `PlacementEditSession` 與 `NatureEditSession` 寫入 DATA |
| **AI** | `Modules.AI` | 戰略原型清單 (`AiArchetypeCatalog`)、任務波次編譯器 (`CampaignMissionCompiler`) | 部分接入（配置選單「AI 戰役波次企劃…」對話框） | 透過 `ScenarioEventSession` 降階編譯為事件注入關卡腳本 |
| **Events** | `Modules.Events` | 事件狀態管理 (`ScenarioEventSession`)、節點圖資料模型 (`Graph`)、編譯驗證器 | 已接入（右側事件分頁：清單檢視與視覺化事件節點圖） | 注入 `SCRIPT/ak_level.bci` 主程式迴圈 |
| **Fortification**| `Modules.Fortification` | 柵欄規劃器 (`PalisadeRunPlanner`)、石牆規劃器 (`WallStrokePlanner`)、工事範本目錄 | 已接入（區域工具對話框「城牆柵欄折線」） | 生成真實城牆物件寫入 DATA |
| **Atmosphere** | `Modules.Atmosphere` | 天氣預設目錄 (`WeatherPresets`)、INI 雙向序列化 (`BodenIniSerializer`) | 已接入（主工具列「天候」下拉選單） | 雙向寫入 `boden.ini`（對齊 33 個真實 INI 鍵） |
| **Pathfinding** | `Modules.Pathfinding` | 通行網格分析 (`NavMeshConnectivityAnalyzer`)、道路缺口偵測 (`RoadGapDetector`)、道路修復器 (`RoadPathHealer`) | 已接入（地圖檢查分頁連通性分析、巨集控制台 `/heal-navmesh`） | 檢測 collision 與高度層，修復時寫入印章與清除碰撞像素 |
| **Profiling** | `Modules.Profiling` | 地圖資源硬限制檢測 (`MapBudgetLimits`)、預算分析器 (`MapBudgetProfiler`) | 已接入（地圖檢查分頁預算分析報告） | 唯讀分析地圖 DATA 物件數與記憶體指標 |
| **Nature** | `Modules.Nature` | 自然景觀編輯 (`NatureEditSession`)、生態植被散播 (`FloraScatterEngine`)、生態圈配置 | 已接入（自然工具列、區域工具對話框「矩形生態植被散播」） | 寫入 `DATA/objects.dat` 等景觀槽位 |
| **Packaging** | `Modules.Packaging` | 模組包 Manifest 1.1 規格、匯出前檢查 (`MapExportPreflightChecker`)、ZIP 匯出器 (`ModBundleExporter`) | 已接入（頂端命令工具列「匯出模組…」按鈕） | 打包輸出包含縮圖、manifest 與地圖檔之獨立 `.zip` |
| **Scripting** | `Modules.Scripting` | 編輯指令解析器 (`EditorCommandParser`)、空間座標轉換、指令執行器 | 已接入（右側檢查器分頁「控制台」） | 提供 `/elevate`、`/replace-texture`、`/spawn-ring`、`/undo` 等命令 |
| **NativeAssets** | `Modules.NativeAssets` | 原始素材解碼（ALR 8-bit 動畫掃描線、APT 菱形圖塊） | 渲染層取用（3D/2D 檢視器直接讀取唯讀圖框） | 唯讀解碼遊戲安裝目錄之 `alr.dat`、`apt.dat` |
| **Soundscape** | `Modules.Soundscape` | 環境音效區域規劃 (`SoundscapeZonePlanner`)、SNDZ 草稿序列化 | **未接入 UI** | **未接入存檔**（原生 `DATA/sound.dat` 匯出已阻斷停用） |
| **Cinematics** | `Modules.Cinematics` | 航點規劃 (`CameraTrackSplinePlanner`)、ABI 呼叫片段編碼 (`CinematicBciCompiler`) | **未接入 UI** | **未接入存檔**（標記為 `IsExperimental = true; IsWiredToLevelScript = false;`） |
| **Objectives** | `Modules.Objectives` | 目標依賴圖、測試沙盒 (`ObjectiveSandboxSession`)、保守編譯器 (`BciObjectiveCompiler`) | 已接入（配置選單「任務目標設計…」對話框） | 經 `ScenarioEventSession` 降階為 `ScenarioEvent` 並隨地圖存檔（沙盒進階規則於編譯時拒絕；遊戲內行為待驗證） |
| **WildLair** | `Modules.WildLair` | 巢穴定義目錄 (`NeutralLairCatalog`)、事件適配器 (`WildLairScenarioEventBinder`) | 已接入（配置選單「野外巢穴守衛波次…」對話框） | 經 `ScenarioEventSession` 將步兵定時波次降階並隨地圖存檔（無原生 den 機制，野獸為 team 8 物件，不支援；遊戲內行為待驗證） |

---

## 3. 核心模組細節與接線

### 3.1 地形與材質 (Terrain)
- **圖塊印章與地區篩選**：
  - `_stampMode`：支援原版圖塊印章連續筆刷。
  - `_stampOtherRegions`：自動讀取目前地圖紋理或地景物件地區（GER=L2、HUN=L3、BRI=L4/04、ITA/ROM=L5/05/06、KAR=L13/15）進行過濾；預設隱藏不相容地區圖塊，輸入關鍵字搜尋或勾選方塊時顯示全部圖塊。
  - `_autoRoad`：勾選「自動選路」後，利用 `RoadTileCatalog` 與 `PaintRoadPath` 在拖曳筆畫中動態挑選原版直路、轉角、三通、十字路口與端點片。遇到無法連接的拓撲時顯示警告，單步原子 Undo。
- **懸崖岩壁規劃器 (`CliffFacePlanner`)**：
  - 由 `CliffEdgeDetector` 讀取 257×257 頂點高程場偵測陡坡與坡向。
  - `CliffTileCatalog.BuildRealNames` 嚴格僅採用經原版圖塊像素分析驗證之真實岩壁圖塊（北坡 `Fels_AA_008`、東坡 `Fels_AA_004`、南坡 `Fels_AA_002`、西坡 `Fels_AA_006`），嚴格精確朝向匹配，禁止猜測名稱回退（避免出現「Error: File not found」錯誤圖塊）；目前 16 個轉角朝向會安全略過並給出提示。
  - 套用時於 4×4 碰撞網格寫入阻擋像素（每圖格 16 阻擋像素，防止部隊翻越），並自動嘗試銜接坡腳碎石過渡（碎石邊界缺失時發出警告但不阻擋套用）。支援雙層 session（紋理 + 碰撞）單步原子 Undo/Redo。
- **水系河流規劃器 (`RiverFlowPlanner`)**：
  - 在矩形範圍內自動計算流向，將河床下挖至水位以下，並以 3 頂點平滑岸坡自然銜接兩岸。

### 3.2 聚落與對戰基地生成 (Settlement)
- `SettlementGeneratorEngine` 整合 `SettlementSiteEvaluator`（地形平坦與通行評估）、`ResourceClusterPlanner`（木材林地與採石場）以及 `MultiplayerFairnessBalancer`（2 人中心對稱、3–8 人旋轉對稱平衡分布）。
- 對照實機別名：四族真實主屋與核心建築（日耳曼 `BauGerHau00_Haupthaus`，匈人 `SCHLA` 屠宰場等）；野生動物以 `FigTieEbe00` 中立 team -1 單兵生成。
- 宿主接線：配置選單「一鍵生成對戰基地…」(`SettlementGeneratorDialog`)，跨層雙 session（放置物件 + 自然物件）單步原子 Undo/Redo 連動。

### 3.3 戰略原型與戰役任務企劃 (AI)
- `AiArchetypeCatalog` 與 `AiArchetypeProfile` 定義四大原型（日耳曼戰團、羅馬防線、匈人騎兵、塞爾特突襲），兵種均採用遊戲實機存在之真實別名。
- `CampaignMissionCompiler` 將戰役波次企劃編譯為 `ScenarioEvent`。
- 宿主接線：配置選單「AI 戰役波次企劃…」(`CampaignWaveDialog`、`MapEditorForm.Campaign.cs`)。
- **關鍵限制與合約**：原型僅用於選擇兵種組合；目前**僅支援定時生成部隊**；勢力 AI 設定、移動/巡邏路徑 (`AssignedPathId`)、指定攻擊目標物件 (`TargetObjectId`) 與部隊相對延遲等尚未由 ScenarioEvent ABI 支援，編譯器明確報錯拒絕而非靜默忽略。合併時呼叫 `ScenarioEventSession.AddRange` 支援單步 Undo/Redo。

### 3.4 劇情事件與觸發器 (Events)
- `ScenarioEventSession` 擁有事件指令、256 筆上限防護、條件/動作集合與儲存基準線。
- 宿主接線：右側「事件」分頁提供清單檢視與自繪雙緩衝 `EventGraphCanvasControl` 節點圖檢視；驗證後原子替換，支援單步 Undo/Redo。

---

## 4. 實驗性與未接入模組說明 (Experimental & Unwired Modules)

以下四個模組目前僅存在於 `src.MapEditor.Modules` 架構中或具備原型測試，**尚未接入編輯器 WinForms UI 或地圖存檔流程**：

### 4.1 環境音效 (Soundscape - `src.MapEditor.Modules/Soundscape`)
- **逆向分析結論**：經對照原版遊戲地圖檔案，發現原生地圖 DATA 目錄下根本沒有 `sound.dat`；遊戲原生聲音來自 `sfxenv/sfxobj/sfxexp.dau` 與物件 action 觸發。因缺乏原版環境音效表 (sound catalog) 與空間音效播放合約，`SoundscapeBinaryStorage` 已全面阻擋虛構的 `sound.dat` 原生檔案匯出與腳本注入（呼叫拋出 `NotSupportedException`）。
- **目前現狀**：僅保留編輯器內部的 SNDZ 草稿格式與記憶體預覽；**未接入主程式 UI 與地圖存檔流程**。

### 4.2 運鏡導演 (Cinematics - `src.MapEditor.Modules/Cinematics`)
- **逆向分析結論**：已於 EXE 中確認底層原生原語 `s_lgcSetEnginePos v(ddd)` 與 `s_lgcSetEngineZoom v(d)`（原生縮放夾限 0..9），且 `CinematicBciCompiler` 已實作底層 bytecode 呼叫片段編碼。但航點預覽距離換算、Pitch/Yaw 原生參數對應、相機控制權接管/釋放、黑邊效果及腳本時間軸排程仍無原生證據。
- **目前現狀**：程式碼常數宣告 `public const bool IsExperimental = true; public const bool IsWiredToLevelScript = false;`；**未接入主程式 UI 與關卡腳本存檔**。

### 4.3 戰役任務目標 (Objectives - `src.MapEditor.Modules/Objectives`)
- **逆向分析結論**：`BciObjectiveCompiler` 嚴格保守降階至既有的 `ScenarioEvent`（僅支援單一主要目標、目標物件死亡/移除 `ObjectDeadOrRemoved`、開局生存倒數、護送至矩形區域、保護物件失敗等）。沙盒 `ObjectiveSandboxSession` 支援的依賴圖 (Prerequisites)、多主線目標、隊伍全殲、持續佔領 (King of the Hill)、擊殺/資源計數在編譯匯出時均會被嚴格阻擋並回報錯誤。
- **目前現狀**：已接入編輯器 WinForms UI（配置選單「任務目標設計…」），編譯成功之事件合併至 `ScenarioEventSession` 並隨地圖存檔；遊戲內行為尚未驗證。

### 4.4 野外巢穴與生物 (Wild Lairs - `src.MapEditor.Modules/WildLair`)
- **逆向分析結論**：逆向分析確認原版引擎無野外巢穴 (den) 或定期再生 (spawn trigger) 機制；野生動物在 DATA 中為中立（隊伍 8）單一物件（`FigTie` 類別，如 `ALL_WOL00`, `ALL_BAE00`, `ALL_EBE00`, `ALL_RAU00`）。`WildLairScenarioEventBinder` 僅能將特定步兵的定時波次降階至 `ScenarioEvent`；動物單兵生成（`SpawnUnit` 不支援隊伍 8）、動態守衛、死亡重生、資源/榮譽獎勵等原生無支援而直接拋出例外。
- **目前現狀**：已接入編輯器 WinForms UI（配置選單「野外巢穴守衛波次…」），所產生之守衛波次事件合併至 `ScenarioEventSession` 並隨地圖存檔；遊戲內行為尚未驗證。

---

## 5. 平行開發檔案所有權與邊界 (File Boundaries for Simultaneous Work)

當多位 AI 代理人或開發者並行工作時，應遵照以下檔案所有權劃分，避免衝突：

| 領域 | 該任務專有檔案 | 跨任務共用介面／邊界 |
| :--- | :--- | :--- |
| **事件狀態** | `src.MapEditor.Modules/Events/*` 及對應測試 | 共用事件 DTO 與 `IEditorModule` |
| **事件 UI** | `MapEditorForm.Events.cs`、`MapEditorForm.EventGraph.cs`、`EventGraphCanvasControl.cs`、對話框 | `ScenarioEventSession` 指令與快照 |
| **戰役波次** | `CampaignWaveDialog.cs`、`MapEditorForm.Campaign.cs`、`CampaignMissionCompiler.cs` | `ScenarioEventSession.AddRange`、`AiArchetypeCatalog` |
| **地形與印章** | 地形 session、`RiverFlowPlanner.cs`、`CliffFacePlanner.cs`、`RoadTileCatalog.cs` | 高度/紋理快照；渲染器更新經由宿主 |
| **聚落與配置** | `SettlementGenerator*.cs`、`MapEditorForm.Layouts.cs`、`ResourceClusterPlanner.cs` | 放置與自然 session；單步 Undo/Redo 連動 |
| **自然物件** | `NatureEditSession`、`FloraScatterEngine.cs`、自然適配器 | 持久 ID 與自然景觀快照 |
| **放置物件** | `PlacementEditSession`、放置適配器 | 持久 ID 與放置快照 |
| **渲染呈現** | `MapCanvasControl`、`Map3DViewControl`、著色器與網格類別 | 唯讀場景與地形資料輸入 |
| **存檔與整合** | 永續性適配器、專案參考、方案檔 | 單一回滾交易；成功後提交基準線 |

### 跨代理協同規則
1. 進行平行修改前，務必先閱讀 `AGENTS.md` 與 `AI_HANDOFF.md`。
2. 在 `AI_HANDOFF.md` 登記自己任務擁有的檔案與預期介面變更。
3. 勿在其他代理人正在修改的檔案上並行編輯；未經授權切勿重置或丟棄他人工作。
4. 完成修改後，務必執行 `dotnet build` 與 `dotnet test` 驗證，並將結果記錄於 `AI_HANDOFF.md`。

---

## 6. 驗證現況與界限說明 (Verification Limits)

- **編譯與單元測試**：全方案建置 0 錯誤；`AgainstRomeMapEditor.Modules.Tests` 與 `AgainstRomeModifier.Tests` 均全數通過。
- **未宣稱遊戲內驗證**：本輪開發未存取遊戲安裝目錄，未啟動遊戲實機。所有新實作與修正均以原始碼分析、測試套件為準；任何實機畫面表現、尋路連通、戰鬥行為與平衡性均未宣稱在遊戲內已驗證。
