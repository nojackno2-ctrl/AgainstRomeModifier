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

## 2026-10-18 陰影投影資料與遮蔽查詢（`src/shadow/`，新增）
- 移植 C# `NativeShadowCatalog` / `NativeShadowDocument` / `MapLightingShadowMap`（純資料與數學，無 GPU）：
  `ShadowCatalog`（`from_text`/`open`、objdef `shidx/shsiz/shtyp/shacx/shacz` + `cl_shado.ini` + `shad.dat` ZIP、大小寫不分、解碼結果快取含失敗）、
  `ObjectShadow::{compute_world_quad, compute_screen_offset}`、`ShadowWorldQuad`、`ShadowDocument`/`ShadowFrame`/`darken_argb`、
  `LightingShadowMap::{is_occluded, occlusion_at, sample_occlusion, shadow_factor}`。新增依賴 `against-rome-data`(PFIL)、`encoding_rs`(CP1251)、`zip =0.6.6`。
- 與 C# 的刻意差異：錯誤以 `ShadowError`（InvalidData/NotSupported/OutOfRange/Io）回傳；`open` 缺檔回 `Ok(None)`、壞檔回 `Err`；PFIL 用 data crate 的嚴格 LZSS（C# 版容忍截斷）；ZIP 單檔上限 16 MiB；`parse_objdef_shadows` 回傳依檔案順序的 `Vec`。
- 固定 C# oracle：`tests/shadow_tests.rs` 內的 hash/字串常數由未改動的 C# 原始碼在 net8.0 console 程式產生（原始碼存為 `tests/oracle/*.txt`，需自行放到無 `Directory.Build.targets` 的暫存目錄執行）。
- 測試：`cargo test --release --locked --offline --manifest-path native/against-rome-environment/Cargo.toml` → 6+9+9+9+24 passed（新增 `shadow_tests` 24 項）。
- 未驗證：未用零售 `shad.dat`/`objdef.dau`/`shadows.dat` 實檔驗證（僅合成資料）；`shadows.dat` 的 PFIL 解壓入口（呼叫端負責）；未接 scene/app。

## 2026-10-18 自然地景物件調色盤與筆刷工作流（`src/nature/palette.rs`，新增）
- 移植 C# `src.MapEditor/MapEditorForm.Nature.cs` 之設計師地景分類、地區過濾、物種混種與筆刷操作：
  - `nature_region(name)`：提取地景名稱地區代碼（如 `LanGerNad00` → `"Ger"`）。
  - `nature_category(name)` / `NatureCategory`：以代碼判定分類（`Tree`, `Grass`, `Bush`, `Water`, `Rock`, `Other`）及多語系字串。
  - `NatureOperation`：`Plant`（種植）與 `Remove`（移除）操作定義。
  - `map_nature_regions`：統計既有地圖地景物件，以 15% 門檻計算地圖主要地區（混合地區地圖支援）。
  - `NaturePaletteFilter`：支援分類過濾、主要地區過濾（受 `show_other_regions` 控制）與不分大小寫名稱排序。
  - `NatureSpeciesMixer`：在啟用 `mix_species` 時，嚴格依同地區、同分類抽取候選清單（避免日耳曼混入義大利地景）。
  - `NatureBrushRemover`：以世界半徑圓形規劃移除，精確保護已連結（`linked`）與非地景物件，並排除已標記移除之 slot。
  - `NatureBrushPainter`：規劃筆刷單格/多格種植、高度雙線性取樣與旋轉角度，可直接串接 `NatureEditSession` 交易歷史。
- 測試：新增 `tests/nature_palette_tests.rs` 7 項測試全數通過（全 crate 6+9+7+9+9+24 = 64 passed，0 failed）。

## 2026-10-18 自然調色盤/筆刷 C# 對拍驗證與 Parity 缺口修復（`src/nature/palette.rs`）
- 發現並修復的核心 Parity 缺口：
  1. **PRNG 推進序列對齊（Critical）**：
     在 C# `MapEditorForm.Nature.cs:272` 中，`species = _natureMix.Checked ? ... : [type]`；無論是否啟用物種混合，每個種植點都無條件評估 `_natureRandom.Next(species.Length)`（未混合時長度為 1，調用 `Next(1)` 推進 PRNG），隨後才調用 `NextDouble()` 計算旋轉角度 `rotation`。
     原 Rust 實作在 `!mix_species` 時直接 early return，跳過了 `random.next_i32(1)`，導致 `rotation` 消耗了錯誤的隨機數，且後續所有筆刷點擊的 PRNG 狀態完全錯位脫軌。修復後在未混合時如實調用 `random.next_i32(1)`，使得旋轉角度與後續種植點之 PRNG 序列與 C# 100% 精準對拍。
  2. **`dominant_regions` 大小寫不敏感過濾（Compatibility）**：
     C# `MapNatureRegions` 回傳之 Set 採用 `StringComparer.OrdinalIgnoreCase`。原 Rust 實作將項目地區轉為大寫後於 `BTreeSet<String>` 中比對，若外部傳入之 `dominant_regions` 包含混合或小寫字串（如 `"ger"` / `"Ger"`）會錯誤全部排除。修復為比對時透過 `eq_ignore_ascii_case` 支援任意大小寫地區代碼。
  3. **物種候選清單確定性排序**：
     `NatureSpeciesMixer::matching_species` 候選集強制按名稱大小寫不敏感排序，確保在多物種候選時，PRNG 索引抽選結果與 C# `_natureTypes.Items` ListBox 排序確定對齊。
  4. **UTF-8 切片安全性**：
     `nature_region` 與 `nature_category` 增加 `is_char_boundary` 檢查，防範多字節 Unicode / emoji 外部字串切片 panic。
  5. **筆刷移除 Tile Helper**：
     `NatureBrushRemover::plan_removal_tile`，精確遵循 C# `PaintNatureTile` 移除公式（中心偏移 `(tile + 0.5) * world`、半徑 `(brush_size / 2 + 0.26) * world`）。
- C# Oracle 對拍測試（`tests/oracle/nature_oracle*` & `tests/nature_palette_tests.rs`）：
  - 新增 `test_csharp_oracle_single_and_large_brush_placement_parity`：驗證單格筆刷（brush=1）與大筆刷（brush=5）在確定性種子下，種植物件數量、名稱、座標 (x, y, z) 及 `rotation` 與 C# Oracle 完全吻合（浮點誤差 < 1e-3）。
  - 新增 `test_dominant_regions_case_insensitivity_and_multiregion`、`test_nature_region_and_category_utf8_boundary_safety`、`test_nature_brush_remover_plan_removal_tile`。
- 完整測試：`cargo test --release --locked --offline --manifest-path native/against-rome-environment/Cargo.toml` → 6+9+11+9+9+24 = 68 passed，0 failed。
- 如實標註無法涵蓋項目：
  1. WinForms UI 佈局與元件事件生命週期（如 `ComboBox` 選項切換、多語系切換文字 `Loc.CurrentLanguage`）。
  2. DirectX / SDL 3D View 即時點選繪製與渲染管線（由前端 app / scene crate 處理）。
  3. 零售地圖磁碟真實 `objects.dat` 讀寫儲存之整合（此層為純計算與資料模型，實際存檔依賴 placement / packaging）。

