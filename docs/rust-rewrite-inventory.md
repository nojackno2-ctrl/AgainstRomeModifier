# Against Rome 專案全程式 Rust 改寫對照與未完成清單 (Rust Rewrite Inventory)

> 文件路徑：`docs/rust-rewrite-inventory.md`  
> 生成時間：2026-10-15  
> 總目標：依據專案規範與使用者指示，將原 C# 解決方案（`src.Core`、`src.Shared`、`src.MapEditor`、`src.MapEditor.Modules`、`src.Modifier`、`src.SaveManager`）及 C++ 追蹤工具（`native/argm-trace`）**全數改寫為純 Rust 實作**。建置與執行完全不再依賴 .NET / MSBuild 與 C++ 工具鏈。

---

## 1. 狀態定義與證據標準 (Status Definitions & Evidence Standards)

| 狀態 | 定義 | 判定條件與證據標準 |
|---|---|---|
| **Ported (已移植)** | 功能已由純 Rust 完全替代，具備同等或超越的規格，且有自動化測試或 Differential 驗證。 | 存在對應 `native/*` 或 `apps/*` 原始碼；單元測試或 C# Parity 測試通過；無遺留的依賴缺口。 |
| **Partial (部分移植)** | 核心邏輯或部分子模組已有 Rust 實作，但缺少部分關聯功能、邊界防護、GUI 接線或完整往返驗證。 | 存在部分 Rust 模組／檔案，但仍有未支援的資料池、UI 缺口、尚未合併的 review 修正或待修補的邊界條件。 |
| **Missing (尚未移植)** | 尚未建立任何 Rust 替代實作，功能目前仍僅存在於 C# 原始碼或 C++ 專案中。 | 無任何對應的 Rust crate、模組或檔案。 |

---

## 2. 專案／命名空間／主要類別完整對照清單 (Detailed Inventory Mapping)

### 2.1 `src.Core` (核心補丁、服務與特徵系統)

| 命名空間 / 模組 | 主要類別 (Major Classes) | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|---|
| **AgainstRomeModifier** (Root) | `Localization`<br>`TroopConfig` | **Partial**<br>**Missing** | `apps/against-rome-editor/src/app.rs`<br>*(無)* | 桌面 app 內有基礎中文 UI 標籤，但無完整的語系切換與 `AppSettings` 配置。<br>`TroopConfig` 兵種資料規格尚未移植。 |
| **AgainstRomeModifier.Bci** | `BciPattern`<br>`BciScriptFile` | **Partial** | `native/against-rome-scenario/src/bci.rs` | Rust `BciImage` 已能解析與組裝 BCI 二進位指令碼，但 high-level pattern 比對與通用 script patcher 尚未完整。 |
| **AgainstRomeModifier.EndlessAi** | `BciLiteralPatch`<br>`CustomPatches` (P4, P6)<br>`EndlessAiModule`<br>`EndlessAiOrchestrator`<br>`IEndlessPatch`<br>`PatchState` | **Missing** | *(無)* | 無盡模式 AI 指令碼熱補丁（如撤退截止時間、迴圈延遲等修復）尚未移植至 Rust。 |
| **AgainstRomeModifier.Core.Features** | `FeatureRegistry`<br>`FeatureDetector`<br>`FeatureKey`<br>`FeatureCategory`<br>`DetectContext`<br>`PatchContext`<br>`PatchProfile`<br>`IFeatureModule` | **Missing** | *(無)* | 可擴充特徵偵測與設定檔登錄框架尚未移植。現有 Rust 修改器僅硬編碼三項補丁。 |
| **AgainstRomeModifier.Core.Features.Bci** | `BciFeatureDetector`<br>`CiviProduce20Feature`<br>`FoodHealingFeature`<br>`RetiredDefaultSpecialArrowsMigration` | **Missing** | *(無)* | BCI 特徵偵測與遊戲邏輯注入補丁（食物治療、平民生產上限）尚未移植。 |
| **AgainstRomeModifier.Core.Features.Exe** | `ExeFeatureDetector`<br>`ExeFeaturePatcher` | **Partial** | `native/against-rome-modifier/src/lib.rs` | 支援 `FOCUS`、`ROMAN_ENDLESS`、`UNIT_RECRUIT_20` 三項 offset 補丁；缺少通用 EXE 特徵識別與擴充機制。 |
| **AgainstRomeModifier.Core.Features.Ini** | `ClScriptFeatureDetector`<br>`IniFeaturePatcher`<br>`RessFeatureDetector` | **Missing** | *(無)* | INI 設定檔補丁（如 `cl_script.ini`、`ress.ini` 資源平衡）尚未移植。 |
| **AgainstRomeModifier.Core.Features.Install** | `ArgmTraceFeature`<br>`DgVoodooConfigProfile`<br>`DgVoodooFeature`<br>`InstallFeatureDetector`<br>`LanguagePackFeature` | **Missing** | *(無)* | 遊戲安裝目錄偵測、dgVoodoo2 與語系包部署特徵尚未移植。 |
| **AgainstRomeModifier.Core.Features.Map** | `MaxPopulationFeature`<br>`TeamDatFeatureDetector` | **Missing** | *(無)* | 地圖人口上限與 `team.dat` 特徵偵測尚未移植。 |
| **AgainstRomeModifier.Core.Features.Objdef** | `ObjdefFeatureDetector`<br>`ObjdefFeaturePatcher` | **Missing** | *(無)* | `objdef.ini` 物件定義修補與解析尚未移植。 |
| **AgainstRomeModifier.Core.Patches** | `ExePatchModel`<br>`VerifiedBinaryWriter`<br>`ClScriptPatcher`<br>`EparaPatcher`<br>`ObjdefPatcher`<br>`PartgeoPatcher`<br>`PatchText`<br>`RessPatcher`<br>`TeamDatPatcher` | **Ported**<br>**Ported**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing** | `native/against-rome-modifier/src/lib.rs`<br>`native/against-rome-modifier/src/lib.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)* | `Patch`、`State`、驗證比對後寫入及唯讀保護已移植並通過 15 tests。<br>其餘文字與二進位檔案專用 Patcher 尚未移植。 |
| **AgainstRomeModifier.Core.Services** | `BackupManager`<br>`SaveBackupManifestCodec`<br>`SaveBackupService`<br>`GameDirectoryLocator`<br>`ILogger`<br>`OriginalFileValidator`<br>`PatchEngine`<br>`BackupAutoHealer`<br>`BackupValidator`<br>`BackupSourceLoader`<br>`CleanEparaBaseline`<br>`CustomMapRestoreService`<br>`EndlessSaveAiRepairService`<br>`GameDirectoryBackupLoader`<br>`PatchFileComposer`<br>`PatchFileContributors`<br>`PatchOperationRunner`<br>`PatchRestoreService`<br>`TgaDecoder`<br>`TroopPresetCodec`<br>`UnitBaselineCatalog`<br>`UnitStatParser`<br>`UnitStatsEditorService`<br>`UnitStatsProjectionService` | **Ported**<br>**Ported**<br>**Ported**<br>**Partial**<br>**Partial**<br>**Partial**<br>**Partial**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Ported**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Ported**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing** | native/against-rome-saves/src/lib.rs<br>native/against-rome-saves/src/lib.rs<br>native/against-rome-saves/src/lib.rs<br>native/against-rome-modifier/src/main.rs<br>apps/against-rome-editor/src/app.rs<br>native/against-rome-modifier/src/lib.rs<br>native/against-rome-modifier/src/lib.rs<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>native/against-rome-modifier/src/custom_map_restore.rs; native/against-rome-modifier/src/cli.rs<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>native/against-rome-modifier/src/tga.rs; native/against-rome-modifier/tests/tga_decoder_migration_edges.rs<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)* | 存檔 ZIP 備份、Manifest 序列化與原子還原已完全移植（8 tests）。<br>路徑驗證與安全寫入已移植。<br>缺存檔 AI 死結修復、兵種屬性編輯器及備份自動修復；TGA 已移植並通過 C# oracle 邊界測試。<br>Rust 完整還原自製地圖刪除已接入 CLI 交易，含 manifest/暫存路徑預檢與失敗回滾（4 tests）。<br>TGA 解碼新增 C# oracle 邊界測試（5 tests passed）。 |
| **AgainstRomeModifier.Core.UI** | `WinFormsTheme` | **Ported** | `apps/against-rome-editor/src/main.rs` | 原 WinForms 深色主題已由 egui 暗色主題與字型設定替代。 |

---

### 2.2 `src.Shared` (共用資料庫、存檔與地圖抽象層)

| 命名空間 / 模組 | 主要類別 (Major Classes) | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|---|
| **AgainstRomeModifier** (Root) | `FileRollbackScope`<br>`GameLZSS` | **Ported**<br>**Ported** | `native/against-rome-placement/src/lib.rs`<br>`native/against-rome-data/src/lzss.rs` | 暫存 staging、唯讀防護與 rollback 機制已移植。<br>嚴格 PFIL/LZSS 解壓縮與壓縮核心已完全移植。 |
| **AgainstRomeModifier.Core.Services** | `JsonDefaults`<br>`SafeFileWriter` | **Ported**<br>**Ported** | `native/against-rome-saves/src/lib.rs`<br>`apps/against-rome-editor/src/storage.rs` | serde / serde_json 與安全寫入（同檔案系統暫存再原子替換）已普及於所有 Rust crate。 |
| **AgainstRomeModifier.Maps** | `BlankMapContent`<br>`BlankMapStarterKit` | **Partial** | `apps/against-rome-editor/src/app.rs` | 支援 257x257 與 193x193 空白地圖記憶體配置，但初始物件樣板（StarterKit）尚未移植。 |
| **AgainstRomeModifier.Maps** | `CustomMapAccess`<br>`CustomMapManifest`<br>`EndlessMapCatalog`<br>`EndlessMapCloner`<br>`EndlessMapDeleter`<br>`GameMapCatalog` | **Missing** | *(無)* | 地圖目錄列舉、自訂地圖 Manifest 管理、無盡地圖複製與刪除尚未由 Rust 實作。 |
| **AgainstRomeModifier.Maps** | `LevelObjectStore`<br>`LevelObjectStore.Families`<br>`LevelObjectStore.VisualState` | **Partial** | `native/against-rome-placement/src/lib.rs`<br>`apps/against-rome-editor/src/placement_ui.rs` | Rust 實作了 11-18 資料池的解析、UID 重映射、parent/child/light/anim 平移與複製（37 tests）；但 particle/garrison family 仍略過，且缺 visual state cache。 |
| **AgainstRomeModifier.Maps** | `MapDesignValidator` | **Missing** | *(無)* | 地圖設計規則驗證器（水域孤島、生成點邊界等）尚未移植。 |
| **AgainstRomeModifier.Maps** | `MapTextDocuments` | **Partial** | `apps/against-rome-editor/src/map_settings.rs`<br>`native/against-rome-textures/src/document.rs` | `boden.ini` 與 `boden.txt` 已支援，其餘地圖文字檔仍待統一整合。 |
| **AgainstRomeModifier.Maps** | `NativeAnimationVisualState`<br>`NativeMarkerTypeTable`<br>`NativeObjectDefinitions`<br>`NativePoolDiagnostics` | **Missing** | *(無)* | 物件視覺動態、標記型別對照表與原生池深層診斷尚未移植。 |
| **AgainstRomeModifier.Maps** | `NativeLightDefinitions`<br>`NativeLightPool`<br>`RustLightPool` | **Ported** | `native/against-rome-data/src/pool.rs`<br>`native/against-rome-data/src/bin/ar-light-json.rs` | 光源池嚴格解碼、定長 payload、唯讀快照完全移植（`light_pool.rs` 測試）。 |
| **AgainstRomeModifier.Maps** | `NativeMapLoadContract` | **Partial** | `apps/against-rome-editor/src/storage.rs` | 地圖資料夾安全載入協定已移植於桌面 app，但缺少完整非必要檔案容錯清單。 |
| **AgainstRomeModifier.Maps** | `NativeParticlePool`<br>`RustParticlePool` | **Ported** | `native/against-rome-data/src/particle.rs` | 粒子發射器資料池與結構解析完全移植。 |
| **AgainstRomeModifier.Maps** | `RustParticleLightProjection` | **Ported** | `native/against-rome-data/src/projection.rs` | 光源投射與粒子光影計算已完全移植至純 Rust。 |
| **AgainstRomeModifier.Maps** | `SdlObjectCatalog`<br>`SdlSceneCatalog`<br>`SdlSceneEditService` | **Missing** | *(無)* | SDL 場景定義檔解析與編輯服務尚未移植。 |
| **AgainstRomeModifier.Scripting** | `BciImage`<br>`ScenarioEventCompiler`<br>`ScenarioEvents`<br>`ScenarioObjectIdentity`<br>`ScenarioLevelObjects`<br>`LevelScriptInjector` | **Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Partial** | `native/against-rome-scenario/src/bci.rs`<br>`native/against-rome-scenario/src/compiler.rs`<br>`native/against-rome-scenario/src/model.rs`<br>`native/against-rome-scenario/src/lib.rs`<br>`native/against-rome-scenario/src/model.rs`<br>`native/against-rome-scenario/src/compiler.rs` | BCI 解析器、事件編譯器（勝負、條件、動作、波次）、UUID 物件識別已完成移植（13 tests）；腳本注入器已移植 bytecode wait-hook 插入，但跨 pool 完整寫回仍屬 Partial。 |

---

### 2.3 `src.MapEditor.Modules` (地圖編輯器功能模組)

| 子模組 (Sub-module) | 主要類別 (Major Classes) | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|---|
| **AI** (5 files) | `AiArchetypeModels`<br>`CampaignMissionCompiler`<br>`CampaignMissionModels`<br>`FactionAiSession`<br>`WaypointModels` | **Missing** | *(無)* | 戰役任務目標編譯、陣營 AI 姿態與路徑點巡航規劃尚未移植。 |
| **Atmosphere** (6 files) | `BodenIniData`<br>`BodenIniSerializer`<br>`AtmosphereEditSession`<br>`AtmosphereLightingBridge`<br>`AtmosphereProfile`<br>`WeatherPresets` | **Partial**<br>**Missing** | `apps/against-rome-editor/src/map_settings.rs`<br>*(無)* | `boden.ini` 的水位、Heightmapstep 已能解析。<br>天氣預設、動態光影橋接與大氣情境 session 尚未移植。 |
| **Cinematics** (8 files) | `MapStartCamera`<br>`MapStartCameraInjector`<br>`CameraFlightPreviewer`<br>`CameraTrackSplinePlanner`<br>`CameraWaypoint`<br>`CinematicBciCompiler`<br>`CutsceneModels`<br>`CutsceneSequenceCatalog` | **Ported**<br>**Ported**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing** | `native/against-rome-scenario/src/model.rs`<br>`native/against-rome-scenario/src/compiler.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)* | 起始鏡頭參數與 BCI 注入已完全移植並通過測試。<br>過場動畫序列、攝影機樣條軌道與飛行預覽尚未移植。 |
| **Diagnostics** (2 files) | `CliffDiagnostics`<br>`MapDiagnostics` | **Missing** | *(無)* | 懸崖坡度診斷與地圖健全度診斷尚未移植。 |
| **Diff** (4 files) | `MapDiffEngine`<br>`MapDiffTypes`<br>`MapVisualDiffOverlay`<br>`SelectiveRollbackController` | **Missing** | *(無)* | 地圖差異比對引擎、視覺化 Heatmap 與選擇性圖層回滾控制器尚未移植。 |
| **Events** (9 files) | `ScenarioEventSession`<br>`EventGraph`<br>`EventGraphConverter`<br>`EventGraphValidator`<br>`GraphEdge`<br>`GraphEnums`<br>`GraphNode`<br>`GraphNodes`<br>`GraphPort` | **Partial**<br>**Missing** | `apps/against-rome-editor/src/scenario_ui.rs`<br>*(無)* | `scenario_ui.rs` 提供了結構化清單編輯與預覽。<br>事件節點圖（Node Graph）視覺化畫布與連接埠驗證器尚未移植。 |
| **Fortification** (6 files) | `FortificationEditSessionCoordinator`<br>`FortificationPassabilitySync`<br>`FortificationTypes`<br>`PalisadeRunPlanner`<br>`WallStrokePlanner`<br>`WallTileCatalog` | **Missing** | *(無)* | 柵欄與城牆筆刷規劃器、磁磚目錄及通行網格同步尚未移植。 |
| **Localization** (5 files) | `BriefingSaveTransaction`<br>`BriefingVariableCatalog`<br>`MultiLanguageTextStudio`<br>`PutScriptAst`<br>`PutScriptSyntaxValidator` | **Missing** | *(無)* | 任務簡報多語系工作室、變數目錄及 `.put` 指令碼語法解析器尚未移植。 |
| **NativeAssets** (34 files) | `NativeAlrDocument`<br>`NativeAlrIndexedFrame`<br>`RustAlrFrameDecoder*`<br>`NativeParticleRenderDefinition`<br>`RustParticleBaseDefinitions*`<br>`NativeLightSource`<br>`MapLightingBitmap`<br>`MapLightingModel`<br>`MapLightingShadowMap`<br>`NativeAptDocument`<br>`NativeAptAttachmentGeometry`<br>`NativeAptLightPoints`<br>`NativeObjectLightingCatalog`<br>`NativePaletteColor`<br>`NativePreparedParticlePalette`<br>`NativeShadowCatalog`<br>`NativeShadowDocument`<br>`NativeSpriteAnimation`<br>`NativeSpriteAtlas`<br>`NativeSpriteCatalog`<br>`SceneLightingContext` | **Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Partial**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing** | `native/against-rome-data/src/alr_metadata.rs`<br>`native/against-rome-data/src/alr_scanlines.rs`<br>`native/against-rome-data/src/alr_scanline_ffi.rs`<br>`native/against-rome-data/src/particle_definitions.rs`<br>`native/against-rome-data/src/particle_frames.rs`<br>`native/against-rome-data/src/pool.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)* | ALR 影格解碼、掃描線解壓、粒子定義與影格提取已移植（17 tests）。<br>光照基本計算已移植。<br>缺 APT 附件幾何、陰影檔（.shd）、精靈圖集（SpriteAtlas）與日夜光照模型。 |
| **Nature** (6 files) | `BiomeEcologyProfile`<br>`BiomeType`<br>`EcologyNoise`<br>`FloraScatterEngine`<br>`NatureEditSession`<br>`NatureScatter` | **Missing** | *(無)* | 生態群系分佈、植物噪聲分佈散佈引擎與自然編輯工作階段尚未移植。 |
| **Objectives** (5 files) | `BciObjectiveCompiler`<br>`ObjectiveDependencyGraph`<br>`ObjectiveModels`<br>`ObjectiveRuleCatalog`<br>`ObjectiveSandbox` | **Missing** | *(無)* | 戰役目標模型、依賴關聯圖、規則目錄及 BCI 編譯器尚未移植。 |
| **Packaging** (5 files) | `MapBundleContract`<br>`MapExportPreflightChecker`<br>`MapPackageManifest`<br>`MinimapThumbnailRenderer`<br>`ModBundleExporter` | **Missing** | *(無)* | 地圖包匯出前置檢查、模組打包器、縮圖渲染器與安裝原則尚未移植。 |
| **Pathfinding** (6 files) | `FormationWidthValidator`<br>`NavMeshConnectivityAnalyzer`<br>`NavMeshPassabilityGrid`<br>`NavMeshRepairContracts`<br>`RoadGapDetector`<br>`RoadPathHealer` | **Ported** | `native/against-rome-navigation/src/lib.rs`<br>`apps/against-rome-editor/src/navigation.rs` | 通行網格分析、連通分量檢查、陣型寬度瓶頸偵測、道路斷裂偵測與修復建議已完全移植（12 tests + egui 診斷介面）。 |
| **Persistence** (1 file) | `ScenarioSavePreflight` | **Missing** | *(無)* | 劇本儲存前置檢查合約尚未移植。 |
| **Placement** (6 files) | `PlacementEditSession`<br>`PlacementEditSession.Movement`<br>`FormationLayout`<br>`FormationLayoutDefinition`<br>`LayoutTypeResolver`<br>`MapLayoutPreset` | **Partial**<br>**Partial**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing** | `native/against-rome-placement/src/lib.rs`<br>`apps/against-rome-editor/src/placement_ui.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)* | 物件平移、複製、刪除、復原重做與預防污染已移植。<br>陣型預設排列（矩形、環形、錐形）及佈局解析器尚未移植。 |
| **Profiling** (3 files) | `MapBudgetLimits`<br>`MapBudgetProfiler`<br>`MapBudgetReport` | **Missing** | *(無)* | 地圖資源預算限制（物件上限、光源密度、繪圖調用）分析器尚未移植。 |
| **Rendering** (3 files) | `DynamicAtlasPacker`<br>`IsometricDepthSorter`<br>`IsometricFrustumCuller` | **Missing** | *(無)* | 視錐剔除、等角深度排序與動態圖集封裝尚未移植。（依據範圍規範，不需追求像素級渲染，但視窗預覽仍需基本排序）。 |
| **Scripting** (22 files) | `CommandRegistry`<br>`EditorCommandParser`<br>`MacroScriptRunner`<br>`IEditorCommand`<br>指令集 (`AlignGrid`, `Brush`, `Diagnose`, `Elevate`, `Flatten`, `HealNavMesh`, `ReplaceTexture`, `Scatter`, `SpawnRing`, `SystemCommands`, `TeamCommands`) | **Missing** | *(無)* | 編輯器巨集指令直譯器、批次自動化指令集尚未移植。 |
| **Settlement** (5 files) | `MultiplayerFairnessBalancer`<br>`ResourceClusterPlanner`<br>`SettlementGeneratorEngine`<br>`SettlementModels`<br>`SettlementSiteEvaluator` | **Missing** | *(無)* | 聚落自動生成引擎、資源群聚規劃、多人公平性平衡器尚未移植。 |
| **Soundscape** (6 files) | `SoundscapeBinaryStorage`<br>`SoundscapeEditorModule`<br>`SoundscapeMixEvaluator`<br>`SoundscapeZoneCatalog`<br>`SoundscapeZoneModels`<br>`SoundscapeZonePlanner` | **Missing** | *(無)* | 環境音效區域規劃、二進位儲存與混音評估器尚未移植。 |
| **Terrain** (21 files) | `EditorBrushExecutor`<br>`TerrainHeightEditSession`<br>`TerrainEditHistory`<br>`TerrainRampTool`<br>`TerrainBlendAuthoringMap`<br>`TerrainBlendEditSession`<br>`TerrainRegionPlanner`<br>`CliffEdgeDetector`<br>`CliffFacePlanner`<br>`CliffTileCatalog`<br>`HydraulicErosionSimulator`<br>`INativeTerrainMaterialResolver`<br>`RiverFlowPlanner`<br>`RiverTileCatalog`<br>`RiverTypes`<br>`RoadStrokePlanner`<br>`RoadTileCatalog`<br>`TerrainHeightWorkBuffer`<br>`TerrainSculptFilter`<br>`TerrainSculptPipeline`<br>`ThermalErosionSimulator` | **Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Ported**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Partial**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Missing**<br>**Partial**<br>**Partial**<br>**Partial**<br>**Missing** | `native/against-rome-editor/src/terrain.rs`<br>`apps/against-rome-editor/src/terrain.rs`<br>`native/against-rome-editor/src/terrain.rs`<br>`native/against-rome-editor/src/terrain.rs`<br>`native/against-rome-textures/src/authoring_map.rs`<br>`native/against-rome-textures/src/session.rs`<br>`native/against-rome-textures/src/region.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>`native/against-rome-textures/src/resolver.rs`<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>*(無)*<br>`native/against-rome-editor/src/terrain.rs`<br>`native/against-rome-editor/src/terrain.rs`<br>`native/against-rome-editor/src/terrain.rs`<br>*(無)* | 地形高度核心通過 24 ramps / 22 snapshots C# differential 完全一致。<br>材質混合已實作過渡橋接與區域規劃。<br>懸崖、河流、道路筆刷以及水蝕/熱蝕模擬器尚未移植。 |
| **WildLair** (5 files) | `NeutralLairCatalog`<br>`NeutralLairModels`<br>`ResourceRegenerationModels`<br>`ResourceRegenerationPlanner`<br>`WildLairScenarioEventBinder` | **Missing** | *(無)* | 中立巢穴、資源重生排程與劇本事件綁定器尚未移植。 |
| **Root** | `IEditorModule` | **Missing** | *(無)* | C# 模組化介面尚未建立 Rust trait 等價物。 |

---

### 2.4 `src.MapEditor` (地圖編輯器主程式與視窗互動)

| 檔案 / 類別 | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|
| `MapEditorForm.cs` (含 33 個 partials: Terrain, Placement, Events, Diagnostics, etc.) | **Partial** | `apps/against-rome-editor/src/app.rs`<br>`apps/against-rome-editor/src/storage.rs`<br>`apps/against-rome-editor/src/terrain.rs`<br>`apps/against-rome-editor/src/navigation.rs`<br>`apps/against-rome-editor/src/placement_ui.rs`<br>`apps/against-rome-editor/src/scenario_ui.rs`<br>`apps/against-rome-editor/src/textures_ui.rs`<br>`apps/against-rome-editor/src/toolbox.rs` | 原 WinForms 主視窗已改由 egui/eframe 實作桌面入口。<br>已接通 2D 地形高度筆刷、阻擋編輯、縮放平移、全域關閉安全保護、存檔工具列，以及放置、導航、劇本與最新完成的材質面板（`textures_ui.rs`）。<br>缺少 3D 視角、撤銷樹預覽與進階圖層。 |
| `Map3DViewControl.cs`<br>`Map3DViewControl.Particles.cs`<br>`SceneGeometryBuffers.cs`<br>`SceneObjectRenderer.cs`<br>`SceneParticleRenderer.cs`<br>`SceneObjectPositioning.cs`<br>`TerrainMeshBuilder.cs`<br>`TerrainRayPicker.cs` | **Missing** | *(無)* | 3D 視角控制項、Direct3D/OpenGL 頂點緩衝區建構器、射線點擊拾取與粒子渲染器尚未移植。<br>*(註：依據專案範圍規則，優先專注於設計師放置與 2D 操作，3D 保留為後續擴充)*。 |
| `FloorMaterialCatalog.cs`<br>`FloorTextureAtlas.cs`<br>`FloorTextureLibrary.cs` | **Ported** | `native/against-rome-textures/src/resolver.rs`<br>`native/against-rome-textures/src/floortex.rs`<br>`apps/against-rome-editor/src/textures_ui.rs` | 地表材質對照、floortex.dat ZIP 讀取與圖集解析已在純 Rust 實作並接入桌面 UI。 |
| `BitmapPixels.cs` | **Ported** | `apps/against-rome-editor/src/bmp.rs` | 24-bit/32-bit BMP 解析、寫入與記憶體像素指標操作已由純 Rust 安全實作。 |
| `BlankMapBuilder.cs` | **Partial** | `apps/against-rome-editor/src/app.rs` | 空白地圖記憶體建構已有快捷按鈕，但實體目錄範本複製尚未完整。 |
| `EditorCamera.cs`<br>`MinimapNavigator.cs` | **Missing** | *(無)* | 視角座標平移縮放由 egui 內建處理，但獨立 Minimap 導航縮圖與相機軌道尚未移植。 |
| `SpriteThumbnailCache.cs`<br>`SceneCombinedSpritePlan.cs` | **Missing** | *(無)* | 精靈縮圖快取與繪製計畫尚未移植。 |
| 專用對話框類別：<br>`AiMapPlanningDialog.cs`<br>`CampaignWaveDialog.cs`<br>`LayoutApplyDialog.cs`<br>`MapSelectionForm.cs`<br>`ObjectiveStudioDialog.cs`<br>`PlacementBatchEditDialog.cs`<br>`ScenarioConditionDialog.cs`<br>`ScenarioEventDialog.cs`<br>`SettlementGeneratorDialog.cs`<br>`StartCameraDialog.cs`<br>`StarterKitDialog.cs`<br>`TerrainRegionDialog.cs`<br>`WildLairDialog.cs` | **Missing**<br>*(部分被取代)* | `apps/against-rome-editor/src/scenario_ui.rs`<br>`apps/against-rome-editor/src/placement_ui.rs`<br>`apps/against-rome-editor/src/textures_ui.rs` | 原獨立 WinForms Modal 對話框，部分已內嵌於 egui 側欄（如條件/動作直接於 `scenario_ui` 清單編輯，材質選擇於 `textures_ui` 編輯）；但 AI 規劃、戰役波次、聚落生成與巢穴設定等專用視窗尚未移植。 |
| `EditorConsoleControl.cs` | **Missing** | *(無)* | 即時控制台輸入視窗尚未移植。 |
| `Events/EventGraphCanvasControl.cs` | **Missing** | *(無)* | 事件節點圖形畫布控制項尚未移植。 |
| `AiEditorOperator.cs`<br>`AiMapEditScope.cs`<br>`AiMapPlan.cs`<br>`AiMapPlanPreview.cs`<br>`MultiAiMapPlanner.cs`<br>`OllamaMapPlanner.cs` | **Missing** | *(無)* | 本地 LLM（Ollama）與多代理協同地圖規劃工具尚未移植至 Rust。 |

---

### 2.5 `src.Modifier` (遊戲修改器主程式與 CLI)

| 檔案 / 類別 | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|
| `Program.cs` | **Partial** | `native/against-rome-modifier/src/main.rs` | Rust CLI 入口點已具備 `inspect` 與 `apply` 子命令。 |
| `Cli/CliCommands.cs`<br>`Cli/CliModels.cs`<br>`Cli/CliOptions.cs`<br>`Cli/CliRunner.cs`<br>`Cli/ConsoleHelper.cs` | **Partial** | `native/against-rome-modifier/src/main.rs` | 支援基本命令列參數解析、Program Files 目錄寫入防護；但缺少完整的 JSON 輸出格式、自動遊戲路徑偵測與各項模組參數開關。 |
| `UI/LauncherForm.cs` | **Ported** | `apps/against-rome-editor/src/app.rs` | 整合啟動器已併入 Rust 編輯器工具列中（"Modifier / Saves"）。 |
| `UI/ModifierForm.cs` (含 Data, Layout, Localization, Patches, Presets) | **Partial** | `apps/against-rome-editor/src/toolbox.rs` | egui `ToolboxUi` 已實作針對三項核心補丁（焦點、羅馬無盡、徵兵20）的檢視、套用、復原與安全路徑檢查；但缺兵種數值微調、多語系與 INI/DAT 設定檔編輯。 |
| `UI/RestoreAllOptionsDialog.cs` | **Missing** | *(無)* | 全數復原選項對話框尚未移植。 |
| `UI/TechDocForm.cs` | **Missing** | *(無)* | 技術文件內建瀏覽器尚未移植。 |
| `UI/TroopPresetForm.cs` | **Missing** | *(無)* | 兵種預設值修改對話框尚未移植。 |
| `UI/UIElements.cs` | **Ported** | `apps/against-rome-editor/src/toolbox.rs` | 自訂控制項已由 egui 標準控制項替代。 |

---

### 2.6 `src.SaveManager` (存檔管理器)

| 檔案 / 類別 | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|
| `SaveManagerForm.cs` | **Ported** | `native/against-rome-saves/src/lib.rs`<br>`apps/against-rome-editor/src/toolbox.rs`<br>`native/against-rome-saves/src/main.rs` | 存檔掃描、ZIP 封裝、Manifest 解析、還原與暫存回滾已由 Rust crate 完全支援，並提供 CLI 與桌面 GUI 操作介面。 |

---

### 2.7 `native/argm-trace` (C++ 執行期飛行紀錄器)

| 檔案 / 元件 | 狀態 | Rust 對應位置 (Rust Equivalent) | 實證與說明 (Evidence & Notes) |
|---|---|---|---|
| `config.cpp`, `config.h` | **Missing** | *(無)* | INI 配置檔解析尚未由 Rust 實作。 |
| `detour.cpp`, `detour.h` | **Missing** | *(無)* | 32-bit x86 inline-hook 引擎尚未由 Rust 實作。 |
| `dllmain.cpp` | **Missing** | *(無)* | `DllMain` 進入點與 PE 指紋比對閘門尚未由 Rust 實作。 |
| `lde.h` | **Missing** | *(無)* | 32-bit x86 指令長度解碼器 (Length Disassembly Engine) 尚未由 Rust 實作。 |
| `proxy_winmm.cpp`, `proxy_winmm.h`<br>`winmm_exports.inc`, `argm_trace.def` | **Missing** | *(無)* | Win32 `winmm.dll` 匯出轉發函式尚未建立 Rust `cdylib`。 |
| `targets.cpp`, `targets.h` | **Missing** | *(無)* | 遊戲關鍵函式簽章與位址定義尚未轉移至 Rust。 |
| `tracelog.cpp`, `tracelog.h` | **Missing** | *(無)* | 非同步/執行緒安全高效能檔案記錄器尚未由 Rust 實作。 |

---

## 3. 現有 Rust 程式碼生態總覽 (Current Rust Architecture)

目前 Rust 程式碼劃分為多個獨立 crate，均配置於 `Cargo.toml` 工作區中：

```
D:\Github\AgainstRomeModifier
├── native/
│   ├── against-rome-data/        # [Ported] 原生二進位池唯讀解析 (Light, Particle, ALR, LZSS)
│   ├── against-rome-editor/      # [Ported] 地形高度場、筆刷、斜坡與歷程核心
│   ├── against-rome-modifier/    # [Partial] EXE 二進位補丁與原子交易核心、CLI
│   ├── against-rome-navigation/  # [Ported] 通行網格分析、道路斷裂診斷與修復建議
│   ├── against-rome-placement/   # [Partial] 11-18 物件資料池解析、平移/複製/刪除、CLI
│   ├── against-rome-saves/       # [Ported] 存檔 ZIP 備份、Manifest 與還原交易、CLI
│   ├── against-rome-scenario/    # [Partial] BCI 腳本編譯、事件/相機/波次、CLI
│   ├── against-rome-textures/    # [Ported] boden.txt、floortex.dat 混合編寫與區域規劃、CLI
│   └── argm-trace/               # [Missing in Rust] (現有為 C++ MSVC winmm.dll 專案)
└── apps/
    └── against-rome-editor/      # [Ported/Integration] 整合型 egui/eframe 桌面地圖編輯器 (50 tests 通過)
```

---

## 4. 優先缺失工作包劃分（獨立檔案所有權） (Prioritized Disjoint Work Packages)

為了讓後續多代理（如 Codex 與 Agy）能平行進行開發而不發生 git 衝突與檔案所有權重疊，依模組邊界劃分以下 **9 個具備排他所有權（Exclusive File Ownership）的獨立工作包**：

### 矩陣概覽 (Overview Matrix)

| 工作包 ID | 模組名稱 | 優先級 | 獨佔路徑 (Exclusive Paths) | 替代之 C# / C++ 範圍 | 核心目標與交付驗證 |
|---|---|---|---|---|---|
| **WP-01** | **物件放置全資料池與安全性** | **P1 (最高優先)** | `native/against-rome-placement/**` | `LevelObjectStore*.cs`<br>`PlacementEditSession*.cs`<br>`NativeMarkerTypeTable.cs` | 完成唯讀權限防護與 rollback 驗證；補齊粒子、駐軍物件池；實作旋轉、陣營與屬性修改。 |
| **WP-02** | **劇本與戰役目標編譯安全** | **P1 (最高優先)** | `native/against-rome-scenario/**` | `CinematicBciCompiler.cs`<br>`BciObjectiveCompiler.cs`<br>`ObjectiveModels.cs` | 修復 BCI NUL 驗證與 alias catalog 邊界；移植戰役目標編譯器與相機樣條軌道插值。 |
| **WP-03** | **修改器全特徵與設定檔補丁** | **P1 (最高優先)** | `native/against-rome-modifier/**` | `src.Core/Core/Features/**`<br>`src.Core/Core/Patches/**`<br>`UnitStat*.cs`, `TroopConfig.cs` | 建立擴充性特徵登錄器；移植 INI/DAT/BCI 補丁（資源、食物、人口）；實作兵種屬性編輯核心與 CLI。 |
| **WP-04** | **存檔 AI 死結修復與健全度** | **P2** | `native/against-rome-saves/**` | `EndlessSaveAiRepairService.cs`<br>`BackupAutoHealer.cs`<br>`BackupValidator.cs` | 於 Rust 實作無盡模式存檔 AI 狀態機修復（party 死結釋放）；實作備份檔案完整性自動修復。 |
| **WP-05** | **生態自然與聚落公平性引擎** | **P2** | `native/against-rome-environment/**` *(新 crate)* | `src.MapEditor.Modules/Nature/**`<br>`src.MapEditor.Modules/WildLair/**`<br>`src.MapEditor.Modules/Settlement/**` | 移植植被噪聲散佈（FloraScatter）、中立巢穴重生排程、聚落自動佈局與多人平衡評估。 |
| **WP-06** | **地形高級筆刷與防禦工事** | **P2** | `native/against-rome-landscape/**` *(新 crate)* | `src.MapEditor.Modules/Terrain/Cliff*`<br>`src.MapEditor.Modules/Terrain/River*`<br>`src.MapEditor.Modules/Fortification/**` | 移植懸崖輪廓偵測與自動貼面、河流自動尋路銜接、柵欄與城牆筆刷規劃器。 |
| **WP-07** | **地圖打包、差異比對與預算** | **P3** | `native/against-rome-packaging/**` *(新 crate)* | `src.MapEditor.Modules/Packaging/**`<br>`src.MapEditor.Modules/Diff/**`<br>`src.MapEditor.Modules/Profiling/**` | 移植地圖套件導出與預檢、視覺化地圖差異比對（MapDiff）與地圖資源負載預算分析。 |
| **WP-08** | **純 Rust 執行期追蹤動態庫** | **P3** | `native/against-rome-trace/**` *(新 crate)* | `native/argm-trace/**` (全 C++ 程式碼) | 建立 32-bit Win32 `cdylib`，匯出 `winmm.dll` 代理函式，以純 Rust 實作指令解碼器與 inline-hook。 |
| **WP-09** | **事件節點畫布與巨集直譯器** | **P3** | `apps/against-rome-editor/src/event_graph/**`<br>`apps/against-rome-editor/src/macro/**`<br>`apps/against-rome-editor/src/console.rs` | `src.MapEditor.Modules/Events/Graph/**`<br>`src.MapEditor.Modules/Scripting/**`<br>`EditorConsoleControl.cs` | 於 egui 實作節點圖編輯畫布（Node Graph）；移植命令列指令直譯器與批次巨集指令執行器。 |

---

## 5. 詳細工作包規格 (Work Package Specifications)

### WP-01: 物件放置全資料池與安全性 (P1)
- **檔案所有權**：`native/against-rome-placement/**`
- **相依 crate**：`native/against-rome-data`
- **具體任務**：
  1. 重驗唯讀 `position.dat` preflight 拒絕機制與外部目標 rollback 不破壞原檔案安全規則。
  2. 擴充至 particle/emitter family 與 garrisoned units 的安全平移與複製。
  3. 實作旋轉（Rotation）、所屬陣營（Team）、初始生命值等屬性修改，並擴充 CLI `ar-placement edit`。
- **驗證指標**：`cargo test --manifest-path native/against-rome-placement/Cargo.toml` 全綠，新增的 safety 與 family 測試通過。

### WP-02: 劇本與戰役目標編譯安全 (P1)
- **檔案所有權**：`native/against-rome-scenario/**`
- **相依 crate**：`native/against-rome-data`
- **具體任務**：
  1. 修復 review 指出的 6 項缺口（NUL 驗證、fastpath 3-byte 異常拒絕、alias catalog 錯誤傳遞、active_bindings 驗證）。
  2. 移植 `src.MapEditor.Modules/Objectives`（`ObjectiveModels`, `BciObjectiveCompiler`），將戰役勝利/失敗條件編譯入 BCI。
  3. 移植相機樣條軌道插值計算（`CameraTrackSplinePlanner`）。
- **驗證指標**：BCI differential oracle 測試通過，且 retail ENDL_000/TUTOR_01 位元組一致。

### WP-03: 修改器全特徵與設定檔補丁 (P1)
- **檔案所有權**：`native/against-rome-modifier/**`
- **相依 crate**：純 Rust 核心
- **具體任務**：
  1. 將三項固定補丁架構重構為可擴充特徵登錄系統（對齊 C# `FeatureRegistry`）。
  2. 實作文字設定檔 Patcher：`cl_script.ini`（解鎖限制）、`ress.ini`（資源平衡）、`team.dat`（人口上限）。
  3. 實作二進位兵種設定解析器（`UnitStatParser`、`TroopConfig`），支援讀取與匯出兵種數值。
- **驗證指標**：針對現有已知的 clean epara 與 modified epara 進行 Parity 測試。

### WP-04: 存檔 AI 死結修復與健全度 (P2)
- **檔案所有權**：`native/against-rome-saves/**`
- **相依 crate**：`native/against-rome-data`
- **具體任務**：
  1. 移植 `EndlessSaveAiRepairService.cs`：讀取存檔中的 AI party 狀態陣列，識別死結/不增援旗標並重置。
  2. 實作備份自動修復與校驗合約（`BackupAutoHealer.cs`、`BackupValidator.cs`）。
- **驗證指標**：單元測試驗證損毀/死結的模擬存檔能被正確修復，且正常存檔 SHA256 不變。

### WP-05: 生態自然與聚落公平性引擎 (P2)
- **檔案所有權**：`native/against-rome-environment/**`
- **相依 crate**：`native/against-rome-editor`
- **具體任務**：
  1. 建立新 crate `against-rome-environment`。
  2. 移植生態植被散佈演算法（`FloraScatterEngine`, `EcologyNoise`, `BiomeEcologyProfile`）。
  3. 移植中立巢穴規劃器（`NeutralLairCatalog`, `NeutralLairModels`）與資源重生計時器。
  4. 移植聚落自動規劃器與對稱性/公平性評分器（`SettlementGeneratorEngine`, `MultiplayerFairnessBalancer`）。
- **驗證指標**：演算法單元測試，比對相同隨機種子下與 C# 產生之生成點座標分佈。

### WP-06: 地形高級筆刷與防禦工事 (P2)
- **檔案所有權**：`native/against-rome-landscape/**`
- **相依 crate**：`native/against-rome-editor`, `native/against-rome-textures`, `native/against-rome-navigation`
- **具體任務**：
  1. 建立新 crate `against-rome-landscape`。
  2. 移植懸崖輪廓偵測與自動懸崖面拼貼器（`CliffEdgeDetector`, `CliffFacePlanner`, `CliffTileCatalog`）。
  3. 移植河流與道路筆刷樣條規劃器（`RiverFlowPlanner`, `RoadStrokePlanner`）。
  4. 移植城牆與柵欄筆刷規劃器，並與 `PassabilityGrid` 阻擋網格同步。
- **驗證指標**：輸入已知測試高程圖，產生的懸崖與河流磁磚排列符合規則，無拓撲孤島。

### WP-07: 地圖打包、差異比對與預算 (P3)
- **檔案所有權**：`native/against-rome-packaging/**`
- **相依 crate**：`native/against-rome-data`, `native/against-rome-saves`
- **具體任務**：
  1. 建立新 crate `against-rome-packaging`。
  2. 移植地圖套件打包與導出前置檢查（`MapPackageManifest`, `MapExportPreflightChecker`）。
  3. 移植地圖版本差異分析器（`MapDiffEngine`, `SelectiveRollbackController`）。
  4. 移植地圖效能預算評估器（`MapBudgetProfiler`）。
- **驗證指標**：打包之 ZIP 檔案格式與 C# ModBundle 一致，差異比對輸出結構符合預期。

### WP-08: 純 Rust 執行期追蹤動態庫 (P3)
- **檔案所有權**：`native/against-rome-trace/**`
- **相依 crate**：`windows-sys` 或純 Win32 FFI bindings
- **具體任務**：
  1. 建立新 crate `against-rome-trace`，`crate-type = ["cdylib"]`，以 `i686-pc-windows-msvc` 為編譯標靶。
  2. 實作 `winmm.dll` 匯出轉發（透過 `#[no_mangle]` 匯出所有 180+ 個 winmm 原始符號）。
  3. 移植 32-bit x86 指令長度解碼器 (LDE) 與安全 prologue detour 機制。
  4. 實作 PE 指紋辨識、簽章檢查與非同步飛行記錄寫入器。
- **驗證指標**：以 32-bit MSVC 組建通過，匯出表與原 C++ `winmm.dll` 完全一致，detour 測試通過。

### WP-09: 事件節點畫布與巨集直譯器 (P3)
- **檔案所有權**：
  - `apps/against-rome-editor/src/event_graph/**`
  - `apps/against-rome-editor/src/macro/**`
  - `apps/against-rome-editor/src/console.rs`
- **相依 crate**：`apps/against-rome-editor`, `native/against-rome-scenario`
- **具體任務**：
  1. 在 egui 中實作互動式節點圖形畫布（Node Graph），視覺化編輯條件、事件與動作連接埠。
  2. 實作巨集指令直譯器（`EditorCommandParser`, `MacroScriptRunner`），支援文字批次指令操作地圖。
  3. 整合至編輯器主介面底部控制台。
- **驗證指標**：UI 互動測試，指令解析單元測試。

---

## 6. 結論與執行指導方針 (Conclusion & Execution Guidelines)

1. **嚴守檔案所有權**：任何代理開始執行特定任務前，必須僅在指派的工作包路徑內進行寫入，不得擅自修改其他代理所負責的模組或全域共用配置。
2. **優先修復核心安全與功能阻礙**：
   - 第一優先（P1）為 **WP-01**（Placement 唯讀/Rollback 防護與其餘 family）、**WP-02**（Scenario NUL/Catalog 修正與 Objectives）、**WP-03**（Modifier 特徵登錄器與 INI/DAT 補丁）。
   - 其餘模組（WP-04 ~ WP-09）可由獨立代理並行開發。
3. **持續更新交接**：任一工作包達成具體里程碑或遇到未預期障礙時，立即按規範更新 `AI_HANDOFF.md`，維持跨代理無縫協作。
