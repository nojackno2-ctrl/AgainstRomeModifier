# Native Against Rome Campaign Crate (AI, Objectives, Cinematics)

## 概述
本 crate（`native/against-rome-campaign`）為 `src.MapEditor.Modules` 中三大核心模組的純 Rust 移植：
1. **AI 戰役企劃與路徑規劃 (AI)**
   - `AiArchetypeModels` (`AiArchetypeProfile`, `AiArchetypeCatalog`, `EngagementRule`, `TacticalPosture`, `TargetPriority`, `UnitPreferenceWeight`)：定義四種標準戰術原型（游牧突襲者、防禦堡壘、日耳曼聚落、野蠻人前哨）與驗證規格。
   - `WaypointModels` (`WaypointNode`, `WaypointPath`, `WaypointMovementMode`, `WaypointStance`, `WaypointPathPlanner`)：路徑點定義、幾何距離累加、Loop / PingPong / OneWay 動態循徑與轉向計算。
   - `CampaignMissionModels` (`CampaignObjective`, `CampaignObjectiveType`, `WaveSquadDefinition`, `WaveAttackDefinition`, `ReinforcementDefinition`, `CampaignMissionPlan`)：戰役企劃、波次進攻、定時增援之資料模型與結構驗證。
   - `CampaignMissionCompiler`：將高階戰役企劃 8 階段降階編譯為合法原版 `ScenarioEvent` 與 BCI 執行序，支援別名校驗、名稱轉義、Windows-1251 編碼驗證。
   - `FactionAiSession`：企劃編輯工作階段，追蹤髒標記（is_dirty）、維護 Baseline 快照、提供任務企劃增刪改查。

2. **戰役任務目標系統 (Objectives)**
   - `ObjectiveModels` (`ObjectiveKind`, `ObjectiveCategory`, `ObjectiveState`, `DependencyRelation`, `CampaignResult`, `ObjectiveAreaBounds`, `ObjectiveReward`, `ObjectiveRuleParameters`, `ObjectiveDependency`, `ObjectiveDefinition`)：多重目標類型、區域界限判斷（矩形與圓形外接）、完成獎勵與條件模型。
   - `ObjectiveRuleCatalog` (`ObjectiveRuleDescriptor`, `ObjectiveRuleCatalog`)：8 大規則樣板工廠與編譯准入驗證閘門（嚴格排除未實證之複雜目標）。
   - `ObjectiveDependencyGraph`：DAG 依賴圖模型、DFS 環路偵測、Kahn 拓撲排序、動態狀態傳遞 (`propagate_states`)、勝負狀態評估 (`evaluate_campaign_result`)。
   - `ObjectiveSandboxSession`：沙盒模擬工作階段，支援時間步進、物件陣亡/移動/剩餘部隊數模擬、離線推演與日誌記錄。
   - `BciObjectiveCompiler`：將目標依賴圖安全編譯為標準 ScenarioEvents 與 BCI 位元碼，支援 PFIL 壓縮打包與無損 JSON 序列化/反序列化。

3. **運鏡與過場動畫 (Cinematics)**
   - `MapStartCamera` (`StartCameraPrimitive`, `MapStartCamera`)：開局相機座標焦點、縮放級距 (0..9)、JSON 往返與邊界檢查。
   - `CameraTrackSplinePlanner` (`CameraWaypoint`, `CameraPose`, `CameraEasingType`, `CameraTrackSplinePlanner`)：向心 Catmull-Rom (α=0.5) 樣條曲線計算、連續 Yaw 角度展開（Unwrapping）、Easing 曲線加減速。
   - `CutsceneModels` (`CutsceneUnitOrderKind`, `CutsceneFxEventKind`, `SubtitleKeyframe`, `UnitOrderKeyframe`, `CutsceneFxKeyframe`, `CutsceneSequence`)：過場動畫關鍵影格與序列模型。
   - `CutsceneSequenceCatalog`：64 序列上限管理、CRUD 操作、Baseline 髒標記追蹤與序列化。
   - `CameraFlightPreviewer` (`CameraPreviewState`, `CameraFlightPreviewer`)：即時試飛播放器、時間推進、字幕與 FX 觸發、相機初始姿態還原。
   - `CinematicBciCompiler`：底層相機 ABI (`s_lgcSetEnginePos`, `s_lgcSetEngineZoom`, `s_centerToPos`) 位元碼生成、訊息彈窗 (`s_showTextBox`) bytecode 編譯、字幕轉 ScenarioEvents 及預覽文字規劃。
   - `MapStartCameraInjector`：開局相機 Shim 注入器，將開局運鏡注入至地圖 `ak_level.bci` 並自動維護 `ak_level.arm_original` 備份與 PFIL 壓縮。

## 依賴關係
- `against-rome-scenario = { path = "../against-rome-scenario" }`（路徑依賴，未修改 scenario 原始碼）
- `against-rome-data = { path = "../against-rome-data" }`
- `serde`, `serde_json`, `uuid`, `encoding_rs`

## 驗證狀態
- 本地 Rust 工具鏈：`re_workspace/rust-runtime/cargo`
- 測試命令：
  ```powershell
  $env:CARGO_HOME = 'D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
  $env:RUSTUP_HOME = 'D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
  $env:PATH = "$env:CARGO_HOME\bin;" + $env:PATH
  cargo test --manifest-path native/against-rome-campaign/Cargo.toml --target-dir re_workspace/target-against-rome-campaign --offline
  ```
- 測試結果：**15 passed, 0 failed, 0 ignored**：
  - `tests/ai_tests.rs`: 6 passed, 0 failed
  - `tests/cinematics_tests.rs`: 5 passed, 0 failed
  - `tests/objectives_tests.rs`: 4 passed, 0 failed
