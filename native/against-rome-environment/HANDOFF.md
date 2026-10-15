# WP-05 `against-rome-environment` Handoff

## 概覽與任務定義
- **工作包 ID**：WP-05（生態自然與聚落公平性引擎，P2）
- **獨佔路徑**：`native/against-rome-environment/**`
- **相依 crate**：`native/against-rome-editor`
- **目標範圍**：
  1. `Nature`: `EcologyNoise`, `BiomeType`, `VegetationLayer`, `NatureDensity`, `NatureScatter`, `BiomeEcologyProfile`, `FloraScatterEngine`, `NatureEditSession`
  2. `WildLair`: `NeutralLairModels`, `NativeResourceModels`, `NeutralLairCatalog`, `ResourceRegenerationPlanner`, `WildLairScenarioEventBinder`
  3. `Settlement`: `SettlementModels`, `SettlementTribalPresets`, `SettlementSiteEvaluator`, `ResourceClusterPlanner`, `MultiplayerFairnessBalancer`, `SettlementGeneratorEngine`, `MapLayoutPreset`
- **狀態**：完成 (Completed)

## 完成進度記錄
- [x] 建立 `native/against-rome-environment/Cargo.toml`
- [x] 實作 `noise.rs`（梯度柏林與多八度分形雜訊，對齊 C# `EcologyNoise`）與 `random.rs`（確定性 PRNG，對齊 .NET 8 `System.Random` xoshiro256**）
- [x] 實作 `nature` 模組：
  - `biome.rs`: `BiomeType`, `VegetationLayer`, `BiomeSpeciesEntry`, `ResolvedSpecies`, `ResolvedEcologyRoster`, `BiomeEcologyProfile`（4 種生態圈預設、名稱解析分類、前綴與降級匹配）
  - `scatter.rs`: `NatureDensity`, `NatureScatter`（筆刷機率、最小間距、泊松圓形取樣）
  - `engine.rs`: `FloraScatterEngine`, `FloraScatterContext`, `FloraScatterParameters`, `FloraScatterReport`, `FloraScatterResult`, `SpatialClearanceGrid`（地形高度插值、坡度計算、水岸與懸崖適配、多八度森林矩陣決策）
  - `session.rs`: `NatureEditSession`, `NatureAddition`, `NatureSnapshot`（單元植被與批次植被、Undo/Redo、邊界安全檢查、快照還原）
- [x] 實作 `wild_lair` 模組：
  - `models.rs`: `LairCategory`, `LairDifficultyTier`, `LairGuardUnit`, `LairWaveSpawnRule`, `LairLootReward`, `NeutralLairDefinition`, `PlacedNeutralLair`, `NativeResourceKind`, `NativeResourceDefinition`, `TimedResourceReplacement`
  - `catalog.rs`: `NeutralLairCatalog`（真實 4 種野生動物權威定義、別名過濾、威脅評級守衛、規則驗證）
  - `planner.rs`: `ResourceRegenerationPlanner`（54+ 種已知原生資源定義表、定時替換規劃、編輯器範本解析放置）
  - `event_binder.rs`: `WildLairScenarioEventBinder`, `ScenarioEvent`, `ScenarioAction`, `ScenarioCondition`（波次生成定時事件綁定、核心結構消亡條件、無效機制/非部隊別名防護拒絕、事件會話同步與清除）
- [x] 實作 `settlement` 模組：
  - `models.rs`: `SettlementTribe`, `SymmetryMode`, `BuildingBlueprint`, `PlacedBuildingPlan`, `NatureClusterItem`, `PlacedResourceItem`, `SettlementSiteEvaluationResult`, `PlayerSettlementLayout`, `MultiplayerDistributionResult`, `FairnessScoreReport`
  - `presets.rs`: `SettlementTribalPresets`（Germanic, Roman, Celtic, Hun 4 大部族主屋與 6 棟核心建築藍圖、林木/採石場調色盤、野生動物）
  - `evaluator.rs`: `SettlementSiteEvaluator`（水體安全掃描、平坦度得分、腹地擴展度、走廊間隙與主屋/核心建築無碰撞排布）
  - `cluster_planner.rs`: `ResourceClusterPlanner`（森林群聚柏林雜訊與泊松間隔採樣、岩壁坡腳採石場錨點、平原野生動物群）
  - `balancer.rs`: `MultiplayerFairnessBalancer`（2 人中心點對稱、3-8 人環狀旋轉對稱、非對稱地圖拓撲等距 Lloyd's 鬆弛、資源方差與距離公平性評估）
  - `layout_preset.rs`: `MapLayoutPreset`, `MapLayoutEntry`, `MapLayoutKind`, `MapLayoutPresets`（JSON 序列化/反序列化、格式與邊界嚴格驗證、Nature 佈局轉換）
  - `engine.rs`: `SettlementGeneratorEngine`（一鍵導出 Placement/Nature 佈局與可攜式 JSON）
- [x] 完整測試覆蓋（33 項測試全部通過，0 失敗，0 略過）：
  - `src/lib.rs`: 6 tests pass
  - `tests/flora_scatter_tests.rs`: 9 tests pass
  - `tests/neutral_lair_tests.rs`: 9 tests pass
  - `tests/settlement_tests.rs`: 9 tests pass

## 驗證指令與結果
- **指令**：
  ```powershell
  $env:CARGO_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo"
  $env:RUSTUP_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup"
  $env:PATH="$env:CARGO_HOME\bin;$env:PATH"
  cargo test --manifest-path native/against-rome-environment/Cargo.toml --target-dir re_workspace/target-against-rome-environment
  ```
- **測試結果**：
  `test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s`
