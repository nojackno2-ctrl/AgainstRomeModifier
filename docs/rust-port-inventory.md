> **歷史文件（Historical）**：本文件撰寫於 Rust 遷移期間；其中引用的 C#／C++ 路徑（`src.*`、`tests/`、`.slnx`、`native/argm-trace` 等）已不存在。需要時可用 `git show 8d49cb3:<path>` 取回。最終結論請見 [docs/rust-rewrite.md](rust-rewrite.md)。

## 目前狀態快照（2026-10-21）

- 目前 Git tracked source audit：`.cs`、`.csproj`、`.sln`、`.slnx`、`.cpp`、`.c`、`.py` 均為 **0**；`.rs` 為 **571**、Ghidra Java scripts 為 **20**。C ABI header 是 Rust FFI 的介面宣告，不含 C 實作。
- `cargo metadata --locked --offline --no-deps --format-version 1` 回報 workspace **33 packages**；repo tracked 有 **34 個 Cargo.toml**（根 workspace manifest 加 33 個 package manifests），`against-rome-trace` 也已是 workspace member。現行 PowerShell 檔是 Rust build/test/release orchestration，不是 .NET build pipeline。
- 下方表格與統計保留遷移期間的歷史來源路徑、數量和計畫，**不是目前檔案清單或剩餘產品程式碼清單**。曾列出的 .NET oracle、舊 publisher 與 C++ 專案已從目前 tree 退役；C# oracle 輸出以 Rust fixtures 保留。完整 parity crosswalk、ignored/retail gate 與 GUI／遊戲驗收仍各自有未完成項目，見 [rust-rewrite.md](rust-rewrite.md) 與 `AI_HANDOFF.md`。

# AgainstRomeModifier → Rust 全程式改寫遷移盤點清單 (Port Inventory)

> 本文件為依據目前 Repository 真實檔案、專案檔（.csproj / .slnx / CMakeLists.txt / Cargo.toml）與程式碼符號建立的逐項遷移盤點清單。
> 建立原則：嚴格區分「測試通過（Unit/Integration Passed）」、「忽略閘門（Ignored Gate / Oracle）」與「實機尚未驗證（In-Game / Hardware Unverified）」；不以測試存在或部分通過視為全功能移植完成。所有未獲實機證實項目均標記為待核對或未驗證。

---

## 一、盤點說明與專案計數 (Overview & Project Counts)

### 1.1 盤點統計總覽 (Project Counts)

本儲存庫實際掃描（排除 `bin/`、`obj/`、`target/`、`.git/` 與本機工作區 `re_workspace/`）之專案檔計數如下：

| 專案類型 | 數量 | 說明 |
|---|---|---|
| **C# / .NET 解決方案檔 (.slnx)** | **1** | `AgainstRomeModifier.slnx`（新式 XML 方案格式） |
| **C# / .NET 核心與應用程式庫專案 (.csproj)** | **6** | `src.Core`, `src.MapEditor`, `src.MapEditor.Modules`, `src.Modifier`, `src.SaveManager`, `src.Shared` |
| **C# / .NET 方案測試專案 (.csproj)** | **2** | `tests/AgainstRomeModifier.Tests`, `tests/AgainstRomeMapEditor.Modules.Tests` |
| **C# / .NET 獨立工具與逆向探針 (.csproj)** | **10** | `tools/ArmMapPackage`, `TEMP/StarterFullMapProbe`, `tools/re/alr-probe`, `tools/re/alr-scanline-differential`, `tools/re/anim-probe`, `tools/re/apt-probe`, `tools/re/formation-probe`, `tools/re/lighting-probe`, `tools/re/lighting-probe/tests`, `tools/re/shad-probe` |
| **C# / .NET Oracle / Golden 差分專案 (.csproj)** | **18** | 分布於 Rust native crates 下之差分比對測試專案，包括 Campaign Unicode/camera、APT/ALR、editor、landscape、modifier、navigation、placement、save manifest/repair、scenario 與 textures oracles |
| **.NET 專案總數 (.csproj 計)** | **36** | 上述 6 + 2 + 10 + 18 = 36 個專案檔（含 `TEMP/StarterFullMapProbe`；排除 `bin/`、`obj/`、`target/`、`.git/` 與 `re_workspace/`） |
| **原生 C++ 專案 (CMakeLists.txt)** | **1** | `native/argm-trace/CMakeLists.txt`（Win32 x86 原生 Hooking / Proxy DLL `winmm.dll`） |
| **Rust 桌面應用程式 (Cargo.toml)** | **1** | `apps/against-rome-editor`（eframe/egui 原生桌面地圖編輯器與整合工具箱） |
| **Rust 原生核心庫 / CLI / cdylib (Cargo.toml)** | **17** | `against-rome-ai-planner`, `against-rome-ambience`, `against-rome-campaign`, `against-rome-data`, `against-rome-editor`, `against-rome-environment`, `against-rome-landscape`, `against-rome-maps`, `against-rome-modifier`, `against-rome-navigation`, `against-rome-packaging`, `against-rome-placement`, `against-rome-saves`, `against-rome-scenario`, `against-rome-scene`, `against-rome-textures`, `against-rome-trace` |
| **Rust 逆向探針工具 (Cargo.toml)** | **13** | 5 個既有工具，加上 `tools/re/alr-probe-rs`、`tools/re/alr-scanline-differential-rs`、`tools/re/apt-probe-rs`、`tools/re/anim-probe-rs`、`tools/re/formation-probe-rs`、`tools/re/lighting-probe-rs`、`tools/re/roads-golden-rs` 與 `tools/re/shad-probe-rs`（保留對應 C# oracle） |
| **Rust Package / Crate 總數** | **31** | 1 (app) + 17 (native crates，含 workspace 外 trace) + 13 (RE tools) = 31 個 Cargo 專案；根 workspace 有 30 個 package，另有獨立 trace package |

### 1.2 狀態判定準則 (Status Criteria)

- **已移植 (Ported)**：Rust 端已具備等價核心資料結構、演算法或功能模組，且已通過單元/整合測試（不涉及實體遊戲載入或 GPU 渲染時）。
- **部分 (Partial)**：Rust 端已有對應模組或部分演算法，但缺少關鍵 API 分支、資料持久化、GUI 介面接線或錯誤處理。
- **未移植 (Unported)**：Rust 端尚未有任何對應代碼、型別或獨立二進位/UI 入口。
- **未驗證 (Unverified)**：代碼已存在或合成測試已通過，但涉及真實 GPU 硬體渲染、真實遊戲引擎執行期掛鉤、零售版實機地圖往返，或目前為 ignored gate / live differential，尚未在實機上證實。
- **待核對 (To Verify)**：原始行為或 Rust 映射邊界無法由當前 repo 靜態分析百分之百確認者。

---

## 二、專案清單與對象矩陣 (Project-Level Matrix)

| 來源專案路徑 / 檔案 | 語言 / 框架 | Rust 對應路徑 / 套件 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|---|
| `AgainstRomeModifier.slnx` | .NET Solution | 根目錄 `Cargo.toml` virtual workspace (29 packages)；`native/against-rome-trace` 另有獨立 manifest | 已建 Rust workspace / .NET source solution 保留 | Workspace 包含桌面 app、16 個 native crates 與 12 個 RE probe；trace crate 排除在 workspace 外但可獨立建置。全 repo 共 30 個 Rust package。舊欄位「根目錄缺 Cargo workspace」已過時 | 維護根 workspace 與獨立 trace crate 的 release gates；保留 .NET solution 僅供未遷移實作/差分驗證 | `cargo metadata --no-deps --format-version 1` |
| `src.Core/AgainstRome.Core.csproj` | C# .NET 8.0 Win Library | `native/against-rome-modifier`, `native/against-rome-saves` | 已移植 / 未驗證 | 核心邏輯已移至 Rust；未在純乾淨 Windows 無 .NET 環境驗收 | 補齊純 Rust CLI 對真實安裝遊戲目錄之無依賴修補測試 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml` |
| `src.MapEditor/AgainstRomeMapEditor.csproj` | C# .NET 8.0 WinForms | `apps/against-rome-editor` (`against-rome-editor` standalone binary) | 功能已移植 / 渲染部分 / 實機未驗證 | Rust 有獨立桌面入口與 14 個 studio 面板；3D 檢視以 CPU draw list + egui 繪製，與原版 OpenTK OpenGL 4 硬體 pipeline 不同；Windows 視窗/GPU/零售圖尚未驗收 | 完成 Windows 桌面操作與效能驗收，再依實際編輯需求處理 3D GPU rendering gap | `cargo test --release --locked --offline -p against-rome-editor-app` |
| `src.MapEditor.Modules/AgainstRome.MapEditor.Modules.csproj` | C# .NET 8.0 Library | 14 個 native crates (`landscape`, `maps`, `placement`, `scenario`, `campaign`, `environment` 等) | 已移植 / 部分 | 絕大部分演算法已拆為純 Rust 模組；部分 .NET 差分測試仍為 ignored | 清理 ignored 差分測試，落實演算法精準逐位元一致 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -Release` |
| `src.Modifier/AgainstRomeModifier.csproj` | C# .NET 8.0 WinForms Exe | `apps/against-rome-editor/src/bin/against-rome-modifier-gui.rs`、`modifier_ui.rs` & `native/against-rome-modifier` CLI | 功能已移植 / 發行與視窗未驗 | 獨立 Modifier GUI、編輯器嵌入分頁、CLI 與 TechDoc 已存在；portable 發行/乾淨機器/原生視窗驗收仍待完成 | 驗證獨立 GUI binary 發行與 Windows UI 工作流 | `cargo test --release --locked --offline -p against-rome-editor-app` |
| `src.SaveManager/AgainstRomeSaveManager.csproj` | C# .NET 8.0 WinForms Exe | `apps/against-rome-editor/src/bin/against-rome-save-manager.rs`, `save-manager.rs`, `toolbox.rs` & `native/against-rome-saves` CLI | 主要工作流已移植 / 發行與視窗未驗 | Rust UI 已具備存檔與備份雙列表、詳細/預覽、建立備份、刪除、交易式還原、無盡 AI 修復及中英文切換；逐項 app tests **14 passed**。原 C# 表單同樣沒有十六進位檢視，舊缺口說法已更正 | 在 Windows 實機操作獨立 GUI，核對備份目錄、列表和 restore/repair 工作流，再驗收 portable 發行 | `cargo test --release --locked --offline -p against-rome-editor-app --lib save_manager_ui` |
| `src.Shared/AgainstRome.Shared.csproj` | C# .NET 8.0 Library | `native/against-rome-maps`, `native/against-rome-data`, `native/against-rome-scenario` | 已移植 | 核心資料結構、LZSS、地圖池、SDL 場景已在 Rust 實作 | 補齊 SDL 物件與真圖全欄位 roundtrip 檢核 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml` |
| `tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj` | C# xUnit | 對應 native crates 內之 tests 與 oracles | 已移植 | C# 測試涵蓋 1294 項；Rust 各 native crate 共有獨立測試對應 | 建立逐項測試用例覆蓋率對照表 | `dotnet test tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj -c Release` |
| `tests/AgainstRomeMapEditor.Modules.Tests/AgainstRomeMapEditor.Modules.Tests.csproj` | C# xUnit | 對應 native crates 內之 tests | 已移植 | C# 測試涵蓋 1152 項；Rust 已移植其主要測試 | 維護 Rust 端對齊測試與差分測試 | `dotnet test tests/AgainstRomeMapEditor.Modules.Tests/AgainstRomeMapEditor.Modules.Tests.csproj -c Release` |
| `tools/ArmMapPackage/ArmMapPackage.csproj` | C# .NET 8.0 Console Exe | `native/against-rome-packaging` & `packaging_ui.rs` | 已移植 / 部分 (clean Windows 未驗收) | Rust CLI 支援 install/export/--force；release suite 50 項通過；六目標 portable stage 和 SHA256/provenance 核對通過 | 在無 .NET 的 clean Windows 驗收六個 exe | `cargo test --release --locked --offline --manifest-path native/against-rome-packaging/Cargo.toml --target-dir re_workspace/parent-verify-packaging` |
| `TEMP/StarterFullMapProbe/probe.csproj` | C# Console Probe | `native/against-rome-maps` (`starter_kit.rs`) | 已移植 | 原為一次性範本探針，邏輯已固化於 maps starter kit | 無需額外工作（保留作為參考） | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml starter_kit` |
| `tools/re/alr-probe/alr-probe.csproj` | C# Console Probe | `native/against-rome-data` (`alr.rs`) | 已移植 | 原生 ALR 格式已完整於 Rust 資料庫中支援 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml alr` |
| `tools/re/alr-scanline-differential/alr-scanline-differential.csproj` | C# Differential Tool | `native/against-rome-data` (`alr_scanlines.rs`), `tools/re/alr-scanline-differential-rs` (independent frozen decoder) | 已移植 / 差分通過 | 獨立 Rust frozen parser 對 68 個粒子資產、11,345 個影格與 14,996 次 palette 解碼比對，0 failures 且來源未變 | 保留 C# oracle 與全資產唯讀差分閘門 | `cargo test --release --locked --offline -p alr-scanline-differential` |
| `tools/re/anim-probe/anim-probe.csproj` | C# Console Probe | `native/against-rome-textures` (`sprite.rs`) | 已移植 | 精靈動畫探針邏輯已進入 textures 與 scene crate | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-textures/Cargo.toml sprite` |
| `tools/re/apt-probe/apt-probe.csproj` | C# Console Probe | `native/against-rome-data` (`apt.rs`, `apt_geometry.rs`) | 已移植 | APT 幾何文件結構已於 Rust 資料層解析 | 驗證複雜 APT 頂點補間 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml apt` |
| `tools/re/formation-probe/formation-probe.csproj` | C# Console Probe | `native/against-rome-navigation` (`formation.rs`) | 已移植 | 陣型矩陣邏輯已在 Rust 移植且具備 golden 比對 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-navigation/Cargo.toml formation` |
| `tools/re/lighting-probe/lighting-probe.csproj` | C# Console Probe | `tools/re/lighting-probe-rs` plus `native/against-rome-data` (`AptDocument`) and `native/against-rome-environment` (`LightingShadowMap`) | 已移植 / 差分通過 | Rust port reproduces both read-only APT extraction and stdin validation modes; 20 synthetic tests pass, retail APT sample pixels and validation output match C# | Keep C# oracle and read-only sample differential gate | `cargo test --release --locked --offline -p lighting-probe` |
| `tools/re/lighting-probe/tests/lighting-tests.csproj` | C# xUnit Test | `native/against-rome-scene` (`lighting_tests.rs`) | 已移植 | 光照演算法測試已移入 scene 測試集 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-scene/Cargo.toml lighting` |
| `tools/re/shad-probe/shad-probe.csproj` | C# Console Probe | `native/against-rome-environment` (`src/shadow/*`) | 已移植 | 陰影 SHAD 檔案解析已由 Rust shadow catalog 替代 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-environment/Cargo.toml shadow` |
| `native/against-rome-campaign/tests/csharp_oracle/CampaignUnicodeOracle.csproj` | C# Oracle (.NET 10) | `native/against-rome-campaign/tests/dotnet_unicode_parity_tests.rs` | 測試通過 / 依賴本機 | 需本機 .NET 10 SDK 方能動態重產 Unicode 映射表 | 現已包含快取表，確保無 .NET 也能離線驗證 | `cargo test --release --locked --manifest-path native/against-rome-campaign/Cargo.toml dotnet_unicode` |
| `native/against-rome-data/tests/oracle/AlrOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-data/tests/alr_differential.rs` | 測試通過 / 依賴本機 | 需 C# 執行時期環境進行差分比對 | 持續保留為差分驗證閘門 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml alr_differential -- --ignored` |
| `native/against-rome-editor/tests/oracle/Oracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-editor/tests/differential.rs` | 測試通過 / Ignored Gate | 差分測試目前為 ignored gate，需顯式傳遞 `--ignored` 執行 | 納入可選 CI 差分驗證 | `cargo test --release --locked --manifest-path native/against-rome-editor/Cargo.toml --test differential -- --ignored` |
| `native/against-rome-landscape/tests/erosion_oracle/ErosionOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-landscape/tests/erosion_oracle.rs` | 測試通過 | 94 thermal + 114 hydraulic 案例已全數通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml erosion_oracle` |
| `native/against-rome-landscape/tests/oracle/RoadOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-landscape/tests/road_differential.rs` | 測試通過 | 道路鋪設差分比對已通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml road_differential` |
| `native/against-rome-landscape/tests/ramp_oracle/RampOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-landscape/tests/ramp_oracle.rs` | 測試通過 | 240 斜坡案例比對已全數通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml ramp_oracle` |
| `native/against-rome-modifier/tests/oracle/ModifierServicesOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-modifier/tests/services_differential.rs` | 測試通過 / Ignored Gate | 包含已忽略之差分閘門 | 驗證無盡 AI 修復與檔案修補差分 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml --test services_differential -- --ignored` |
| `native/against-rome-modifier/tests/oracle_cl_script/ClScriptDetectorOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-modifier/tests/cl_script_differential.rs` | 測試通過 | 58 個 ClScript 檢測案例已全數通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml cl_script` |
| `native/against-rome-navigation/tools/csharp-golden/FormationGolden.csproj` | C# Golden Tool | `native/against-rome-navigation/tests/formation_differential.rs` | 測試通過 | 陣型演算法黃金資料已固化 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-navigation/Cargo.toml formation` |
| `native/against-rome-navigation/tools/csharp-golden-roads/RoadsGolden.csproj` | C# Golden Tool | `tools/re/roads-golden-rs` + `native/against-rome-navigation/tests/connectivity_roads_differential.rs` | 測試通過 | 道路連通度黃金資料已固化；Rust 產生器輸出與 fixture 位元組完全相同 | 保留 C# oracle 作差分參考 | `cargo test -p roads-golden --locked --offline` |
| `native/against-rome-placement/tests/oracle/PlacementOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-placement/src/lib.rs` (`tests`) | Release 與 8 個 Ignored Gates 通過 | 44 常規測試與 8 個顯式 .NET/retail gates 全部通過；74 張 retail maps 唯讀掃描可載入，TUTOR_01 move/clone/delete 與 C# 逐位元一致 | 在 clean Windows 恢復完整 gate 自動化，並維護 unsupported family 關係診斷 | `cargo test --release --locked --offline --manifest-path native/against-rome-placement/Cargo.toml --target-dir re_workspace/rust-placement-target` |
| `native/against-rome-saves/tests/oracle/Oracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-saves/src/ai_repair/tests.rs` | 測試通過 / Ignored Gate | 19 個 AI 修復差分案例需加 `--ignored` 執行 | 保留為差分驗證常規管線 | `cargo test --release --locked --manifest-path native/against-rome-saves/Cargo.toml ai_repair::tests::csharp_synthetic_differential -- --ignored` |
| `native/against-rome-scenario/tests/event_graph_oracle/EventGraphOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-scenario/tests/event_graph_tests.rs` | 測試通過 | 事件圖結構與轉換差分已通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-scenario/Cargo.toml event_graph` |
| `native/against-rome-scenario/tests/oracle/Oracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-scenario/tests/compiler.rs` | 測試通過 / Ignored Gate | 包含 3 項 ignored 差分測試 | 補強 BCI 編譯器合成差分覆蓋率 | `cargo test --release --locked --manifest-path native/against-rome-scenario/Cargo.toml --test compiler csharp_synthetic_differential -- --ignored` |
| `native/against-rome-textures/tests/csharp_sprite_oracle/SpriteCatalogOracle.csproj` | C# Oracle (.NET 8.0) | `native/against-rome-textures/tests/catalog_csharp_differential.rs` | 測試通過 | 精靈目錄與圖集差分已通過 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-textures/Cargo.toml catalog` |
| `native/argm-trace/CMakeLists.txt` | C++ 17 MSVC Win32 | `native/against-rome-trace/Cargo.toml` | 已移植 / 未驗證 | Rust 已具備靜態 CRT i686 Win32 `winmm.dll`，但未在真實遊戲中掛載執行 | 在測試地圖與受控環境驗收 Rust `winmm.dll` 執行期掛鉤 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -TraceOnly` |

---

## 三、功能分類詳細對照盤點 (Detailed Feature & Symbol Inventory)

### 3.1 核心修補與逆向引擎 (`src.Core` → `against-rome-modifier`, `against-rome-saves`, `against-rome-data`)

| 來源路徑 / 類別符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `Core/Localization.cs` | `native/against-rome-ambience/src/localization.rs`, `native/against-rome-modifier/src/features.rs` | 已移植 | WinForms 多語系資源轉為 Rust 靜態字串與列舉 | 統一跨 crate 本地化詞彙介面 | `cargo test --release --locked --manifest-path native/against-rome-ambience/Cargo.toml localization` |
| `Core/TroopConfig.cs` | `native/against-rome-modifier/src/troops.rs`, `unit_stats.rs` | 已移植 | 兵種數值設定、欄位驗證（HP, Dmg, VW, AW, Sight, Relt 等）完全對齊 | 保持與遊戲二進位檔位移一致 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml unit_stats` |
| `Core/Bci/BciPattern.cs`, `BciScriptFile.cs` | `native/against-rome-modifier/src/bci_features.rs`, `native/against-rome-scenario/src/bci.rs` | 已移植 | BCI 指令字節樣式匹配與腳本反組譯已完成 | 補強長指令容錯分支 | `cargo test --release --locked --manifest-path native/against-rome-scenario/Cargo.toml bci` |
| `Core/EndlessAi/EndlessAiOrchestrator.cs`, `BciLiteralPatch.cs`, `CustomPatches.cs`, `PatchState.cs` | `native/against-rome-modifier/src/endless_ai.rs`, `native/against-rome-saves/src/ai_repair.rs` | 已移植 / 測試通過 | 19 項無盡 AI 修復邏輯、BciPatch 替換與驗證已於 Rust 實現 | 持續確認新發現之遊戲劇本 edge cases | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml endless_ai` |
| `Core/Features/FeatureDetector.cs`, `FeatureRegistry.cs`, `IFeatureModule.cs` | `native/against-rome-modifier/src/features.rs`, `detectors.rs` | 已移植 | 特徵檢測註冊表與各分類檢測器完全遷移 | 保持 stale flags 檢測與清除機制 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml detectors` |
| `Core/Features/Bci/BciFeatureDetector.cs`, `CiviProduce20Feature.cs`, `FoodHealingFeature.cs`, `RetiredDefaultSpecialArrowsMigration.cs` | `native/against-rome-modifier/src/bci_features.rs` | 已移植 | 平民生產力、食物治癒、特殊箭矢遷移修補皆已移植 | 針對損壞 BCI 樣式之退回機制防護 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml bci_features` |
| `Core/Features/Exe/ExeFeatureDetector.cs`, `ExeFeaturePatcher.cs` | `native/against-rome-modifier/src/exe_features.rs`, `exe_patches.rs` | 已移植 | EXE 位元組修補與特徵比對已完成 | 確認多版本 Against_Rome.exe 雜湊識別相容性 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml exe_features` |
| `Core/Features/Ini/ClScriptFeatureDetector.cs`, `IniFeaturePatcher.cs`, `RessFeatureDetector.cs` | `native/against-rome-modifier/src/cl_script.rs`, `ress.rs` | 已移植 | INI 腳本解析、資源生產速率與 ClScript 補丁器已全數通過 58 項案例 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml cl_script` |
| `Core/Features/Install/ArgmTraceFeature.cs`, `DgVoodooConfigProfile.cs`, `DgVoodooFeature.cs`, `InstallFeatureDetector.cs`, `LanguagePackFeature.cs` | `native/against-rome-modifier/src/install.rs` | 已移植 / 未驗證 | 外部元件（dgVoodoo2, winmm trace, 語言包）部署與狀態探測已移植；未在真實遊戲目錄直接驗收 | 在受控虛擬機驗證安裝偵測路徑 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml install` |
| `Core/Features/Map/MaxPopulationFeature.cs`, `TeamDatFeatureDetector.cs` | `native/against-rome-modifier/src/team_dat.rs`, `native/against-rome-maps/src/sdl_scene.rs` | 已移植 | 最大人口修補與 Team.dat 隊伍特徵檢測已對齊 | 驗證非標準隊伍定義地圖之容錯 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml team_dat` |
| `Core/Features/Objdef/ObjdefFeatureDetector.cs`, `ObjdefFeaturePatcher.cs` | `native/against-rome-modifier/src/objdef.rs` | 已移植 | Objdef 物件定義檔修補與檢測已移植 | 驗證自訂物件欄位位移 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml objdef` |
| `Core/Patches/ClScriptPatcher.cs` | `native/against-rome-modifier/src/cl_script.rs` | 已移植 | INI 設定鍵值置換與註解保留已移植 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml cl_script` |
| `Core/Patches/EparaPatcher.cs` | `native/against-rome-modifier/src/epara.rs` | 已移植 | 單位平衡參數二進位修補已移植 | 驗證非官方 MOD 衍生版 epara 格式 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml epara` |
| `Core/Patches/PartgeoPatcher.cs` | `native/against-rome-modifier/src/partgeo.rs` | 已移植 | 粒子幾何設定修補已移植 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml partgeo` |
| `Core/Patches/PatchText.cs` | `native/against-rome-modifier/src/patch_text.rs` | 已移植 | 遊戲文本差異比對與文字修補已移植 | 驗證 UTF-8 / ANSI 混合編碼地圖文本 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml patch_text` |
| `Core/Patches/RessPatcher.cs` | `native/against-rome-modifier/src/ress.rs` | 已移植 | 資源收益設定檔案修補已移植 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml ress` |
| `Core/Patches/TeamDatPatcher.cs` | `native/against-rome-modifier/src/team_dat.rs` | 已移植 | 隊伍二進位設定修補已移植 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml team_dat` |
| `Core/Patches/VerifiedBinaryWriter.cs` | `native/against-rome-modifier/src/services.rs` | 已移植 | 原子性寫入與寫入後二進位 SHA-256 回讀驗證已實作 | 擴展至全專案磁碟寫入規範 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml services` |
| `Core/Services/BackupManager.cs`, `BackupAutoHealer.cs`, `BackupValidator.cs`, `OriginalFileValidator.cs` | `native/against-rome-modifier/src/services.rs`, `native/against-rome-saves/src/backup_health.rs` | 已移植 | 備份驗證、自癒機制、原始檔 SHA 比對與備份管理已移植 | 針對損壞 zip 備份之自動降級流程 | `cargo test --release --locked --manifest-path native/against-rome-saves/Cargo.toml backup_health` |
| `Core/Services/EndlessSaveAiRepairService.cs` | `native/against-rome-saves/src/ai_repair.rs`, `apps/against-rome-editor/src/toolbox.rs` | 已移植 / 測試通過 | 存檔 AI 修復流程已於 Rust CLI 及 Editor Toolbox 接線（含預覽、確認、備份） | 針對真實受損存檔之邊界容錯 | `cargo test --release --locked --manifest-path native/against-rome-saves/Cargo.toml ai_repair` |
| `Core/Services/GameDirectoryLocator.cs` | `native/against-rome-modifier/src/install.rs` | 已移植 | 註冊表、預設路徑與自訂遊戲目錄探測已移植 | 測試 64 位元 Windows 虛擬化註冊表節點 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml install` |
| `Core/Services/PatchEngine.cs`, `PatchFileComposer.cs`, `PatchOperationRunner.cs`, `PatchRestoreService.cs` | `native/against-rome-modifier/src/services.rs` | 已移植 | 交易式修補規劃、執行、復原與衝突解決已實作 | 強化部分失敗時的原子回滾日誌 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml services` |
| `Core/Services/SaveBackupService.cs`, `SaveBackupManifestCodec.cs` | `native/against-rome-saves/src/lib.rs` | 已移植 | 存檔備份清單序列化、打包還原已於 saves crate 實作 | 驗收包含非英文字元存檔名稱路徑 | `cargo test --release --locked --manifest-path native/against-rome-saves/Cargo.toml` |
| `Core/Services/TgaDecoder.cs` | `native/against-rome-modifier/src/tga.rs` | 已移植 | 24/32 位元未壓縮與 RLE TGA 影像解碼已完成 | 驗證調色盤格式 TGA 錯誤處理 | `cargo test --release --locked --manifest-path native/against-rome-modifier/Cargo.toml tga` |
| `Core/Services/TroopPresetCodec.cs`, `UnitStatParser.cs`, `UnitStatsEditorService.cs` | `native/against-rome-modifier/src/unit_stats.rs`, `apps/against-rome-editor/src/modifier_ui.rs` | 已移植 | `.artroop` 預設編解碼、6/9 欄位草稿編輯已於 CLI 與 Editor UI 完成接線 | 整合檔案選擇器對話框（現為手動路徑） | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml modifier_ui` |
| `Core/UI/WinFormsTheme.cs` | `apps/against-rome-editor/src/app.rs` | 未移植 / 架構替代 | WinForms 專用主題輔助函式；Rust 改用 egui native Style 與字型系統 | 無需直接移植（以 egui 樣式取代） | 無需驗證命令 |

### 3.2 共用與底層資料管線 (`src.Shared` → `against-rome-maps`, `against-rome-data`, `against-rome-scenario`)

| 來源路徑 / 類別符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `Core/FileRollbackScope.cs` | `native/against-rome-maps/src/rollback.rs`, `native/against-rome-packaging/src/diff/rollback.rs` | 已移植 | RAII 樣式檔案修改復原範圍（失敗時自動還原檔案）已在 Rust 實作 | 擴充跨目錄批次交易還原防護 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml rollback` |
| `Core/GameLZSS.cs` | `native/against-rome-data/src/lzss.rs`, `native/against-rome-modifier/src/lzss.rs`, `native/against-rome-textures/src/lzss.rs` | 已移植 | 遊戲專屬 LZSS 演算法已移植且通過逐位元差分 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml lzss` |
| `Maps/BlankMapContent.cs`, `BlankMapStarterKit.cs` | `native/against-rome-maps/src/blank_content.rs`, `starter_kit.rs`, `blank_builder.rs` | 已移植（來源/執行期驗收待補） | 空白地圖內容與主屋 linked family 跨來源匯入、Spawn→DATA candidate transaction、14000 容量預算及失敗原子性已實作；placement 與 maps 套件測試通過，C# 11-pool move/clone/delete oracle bitwise parity 通過 | 補齊實機 TUTOR_01 map 的唯讀來源驗收與 GUI/game acceptance | `cargo test -p against-rome-placement --locked --offline`; `cargo test -p against-rome-maps --locked --offline` |
| `Maps/CustomMapAccess.cs`, `CustomMapManifest.cs` | `native/against-rome-maps/src/custom_map.rs`, `preflight.rs` | 已移植 | 自訂地圖檔案路徑探測、清單解析與唯讀安全性已實作 | 強化損壞自訂地圖目錄之報錯回報 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml custom_map` |
| `Maps/EndlessMapCatalog.cs`, `EndlessMapCloner.cs`, `EndlessMapDeleter.cs`, `GameMapCatalog.cs` | `native/against-rome-maps/src/catalog.rs`, `cloner.rs`, `deleter.rs` | 已移植 | 地圖目錄掃描、無盡地圖安全複製（重新配置 ID）與安全刪除已實作 | 驗證安裝目錄存取權限受限時之平穩失敗 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml catalog` |
| `Maps/LevelObjectStore.cs`, `LevelObjectStore.Families.cs`, `LevelObjectStore.VisualState.cs` | `native/against-rome-maps/src/sdl_objects.rs`, `native_pool.rs`, `native/against-rome-placement/src/lib.rs` | 部分移植 | 核心儲存、同圖 family clone/translate/remove、依賴閉包、視覺狀態與跨來源 family candidate import 有 Rust 實作；family pool allocation/reference remap、capacity/UID guards 和 Scenario Spawn→DATA transaction 已通過 synthetic tests；仍需以零售來源地圖覆蓋更廣 pool 形態 | 擴充零售地圖唯讀差分與 GUI/game acceptance | `cargo test -p against-rome-placement --locked --offline`; `cargo test -p against-rome-maps --locked --offline` |
| `Maps/MapDesignValidator.cs` | `native/against-rome-maps/src/design_validator.rs` | 已移植 | 地圖設計規則驗證（出生點、資源距離、可行走區閉環）已移植 | 支援設計師自訂驗證規則開關 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml design_validator` |
| `Maps/MapTextDocuments.cs` | `native/against-rome-maps/src/text_docs.rs`, `text_encoding.rs` | 已移植 | 地圖文字檔（.txt / .ini / .bci 註解）之 Windows-1252 / UTF-8 編碼安全往返 | 驗證德法西義特殊字母編碼 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml text_docs` |
| `Maps/NativeAnimationVisualState.cs`, `NativeLagerPool.cs`, `NativeLightDefinitions.cs`, `NativeLightPool.cs`, `NativeMarkerTypeTable.cs`, `NativeObjectDefinitions.cs`, `NativeParticlePool.cs`, `NativePoolDiagnostics.cs` | `native/against-rome-maps/src/native_marker.rs`, `native_objdef.rs`, `native_pool.rs`, `native/against-rome-data/src/pool.rs`, `particle_definitions.rs` | 已移植 | 5 大原始資料池（Lager, Light, Particle, Object, Marker）已於 Rust 完全解碼並支援差分校驗 | 補強未記錄之極端未知型別旗標檢測 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml native_pool` |
| `Maps/RustLightPool.cs`, `RustParticlePool.cs`, `RustParticleLightProjection.cs` | `native/against-rome-data/src/ffi.rs`, `include/against_rome_data.h` | 已移植 (FFI) / 原生已內置 | 原為 C# P/Invoke 呼叫 Rust `against_rome_data.dll`；在純 Rust 地圖編輯器中直接以 Rust 結構存取 | 逐步淘汰 C# FFI 封裝層 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml ffi` |
| `Maps/SdlObjectCatalog.cs`, `SdlSceneCatalog.cs`, `SdlSceneEditService.cs` | `native/against-rome-maps/src/sdl_objects.rs`, `sdl_scene.rs` | 已移植 | SDL 場景樹解析、物件新增/修改/刪除與場景保存已實作 | 驗收大型 1024x1024 地圖萬級物件之編輯流暢度 | `cargo test --release --locked --manifest-path native/against-rome-maps/Cargo.toml sdl_scene` |
| `Scripting/BciImage.cs`, `LevelScriptInjector.cs`, `ScenarioEventCompiler.cs`, `ScenarioEvents.cs`, `ScenarioLevelObjects.cs`, `ScenarioObjectIdentity.cs` | `native/against-rome-scenario/src/bci.rs`, `compiler.rs`, `model.rs`, `bindings.rs` | 已移植 / 差分通過 | C# 已實作條件/動作及 MoveDestination 已與 Rust compiler/model 對齊；對照未發現可編譯事件缺口。原生 `s_searchTeamUnits` / `s_searchTeamFigures` 契約在 C# 亦不支援，不能當作 Rust 移植缺口 | 保留兩項 synthetic .NET differential gates；後續僅在隔離測試地圖驗證遊戲內注入 | `cargo test --release --locked --offline --manifest-path native/against-rome-scenario/Cargo.toml` |

### 3.3 地圖編輯器核心模組 (`src.MapEditor.Modules` → `against-rome-*`)

| 來源路徑 / 類別符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `Terrain/TerrainHeightEditSession.cs`, `TerrainHeightWorkBuffer.cs`, `TerrainEditHistory.cs`, `EditorBrushExecutor.cs`, `TerrainSculptPipeline.cs`, `TerrainSculptFilter.cs` | `native/against-rome-editor/src/terrain.rs`, `apps/against-rome-editor/src/terrain.rs`, `native/against-rome-landscape/src/erosion.rs`, `sculpt.rs` | 部分移植 / 核心 filters differential 驗證 | Terrace、SharpenRidge、CarveValley 核心演算法已移植；C# differential 551 cases、32,048 samples bit-exact。尚需核對 session-facing Execute/apply wrappers、Elevate/Depress 及是否有生產呼叫路徑 | 完成 pipeline/session wrapper parity 審查並補對應 tests | `cargo test -p against-rome-landscape --locked --offline`; `cargo test -p against-rome-landscape --release --locked --offline --test sculpt_differential -- --ignored` |
| `Terrain/TerrainRampTool.cs` | `native/against-rome-landscape/src/ramp.rs` | 已移植 / 測試通過 | 240 項斜坡案例完全對齊 C# 產出 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml ramp` |
| `Terrain/HydraulicErosionSimulator.cs`, `ThermalErosionSimulator.cs` | `native/against-rome-landscape/src/erosion.rs` | 已移植 / 測試通過 | 208 項熱力與水力侵蝕案例完全對齊 C# 產出 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml erosion` |
| `Terrain/TerrainBlendAuthoringMap.cs`, `TerrainBlendEditSession.cs`, `TerrainRegionPlanner.cs`, `INativeTerrainMaterialResolver.cs` | `native/against-rome-textures/src/authoring_map.rs`, `floortex.rs`, `resolver.rs`, `session.rs` | 已移植 | 地表材質混合、區域規劃與地表材質目錄解析已完成 | 驗收大面積材質自動羽化演算 | `cargo test --release --locked --manifest-path native/against-rome-textures/Cargo.toml authoring_map` |
| `Terrain/CliffEdgeDetector.cs`, `CliffFacePlanner.cs`, `CliffTileCatalog.cs` | `native/against-rome-landscape/src/cliff.rs` | 已移植 | 懸崖邊緣自動檢測、落差幾何計算與磚塊排布已移植 | 針對極端交角懸崖之自動過渡處理 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml cliff` |
| `Terrain/RiverFlowPlanner.cs`, `RiverTileCatalog.cs`, `RiverTypes.cs` | `native/against-rome-landscape/src/river.rs` | 已移植 | 河道流向路徑計算、水深與河磚擺放已移植 | 支援多支流匯流處平滑銜接 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml river` |
| `Terrain/RoadStrokePlanner.cs`, `RoadTileCatalog.cs` | `native/against-rome-landscape/src/road.rs` | 已移植 / 測試通過 | 道路連續筆刷鋪設、交叉路口轉角匹配已移植 | 無需額外工作 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml road` |
| `Pathfinding/NavMeshConnectivityAnalyzer.cs`, `NavMeshPassabilityGrid.cs`, `NavMeshRepairContracts.cs`, `RoadGapDetector.cs`, `RoadPathHealer.cs`, `FormationWidthValidator.cs` | `native/against-rome-navigation/src/lib.rs`, `formation.rs`, `repair.rs` | 已移植 | 導航網格連通性分析、斷路修復、部隊隊形寬度通行驗證已移植 | 整合大型地圖局部導航網格快取 | `cargo test --release --locked --manifest-path native/against-rome-navigation/Cargo.toml` |
| `Placement/FormationLayout.cs`, `FormationLayoutDefinition.cs`, `LayoutTypeResolver.cs`, `MapLayoutPreset.cs`, `PlacementEditSession.cs`, `PlacementEditSession.Movement.cs` | `native/against-rome-placement/src/lib.rs` | 已移植 / 測試通過 | 單位編隊陣型、預設版面套用、物件位移與族群關聯同步已實作 | 支援陣型即時旋轉拖曳吸附 | `cargo test --release --locked --manifest-path native/against-rome-placement/Cargo.toml` |
| `Settlement/SettlementGeneratorEngine.cs`, `MultiplayerFairnessBalancer.cs`, `ResourceClusterPlanner.cs`, `SettlementModels.cs`, `SettlementSiteEvaluator.cs` | `native/against-rome-environment/src/settlement/*` | 已移植 | 多人對戰公平基地生成、資源點評估與分配已完全移植 | 針對複雜崎嶇地勢的基地可用地判定補強 | `cargo test --release --locked --manifest-path native/against-rome-environment/Cargo.toml settlement` |
| `WildLair/NeutralLairCatalog.cs`, `NeutralLairModels.cs`, `ResourceRegenerationModels.cs`, `ResourceRegenerationPlanner.cs`, `WildLairScenarioEventBinder.cs` | `native/against-rome-environment/src/wild_lair/*` | 已移植 | 中立野獸巢穴、資源再生週期規劃與劇本事件綁定已完成 | 驗證巢穴觸發器與劇本事件的即時連動 | `cargo test --release --locked --manifest-path native/against-rome-environment/Cargo.toml wild_lair` |
| `Nature/BiomeEcologyProfile.cs`, `BiomeType.cs`, `EcologyNoise.cs`, `FloraScatterEngine.cs`, `NatureEditSession.cs`, `NatureScatter.cs` | `native/against-rome-environment/src/nature/*` | 已移植 | 生態群系噪聲生成、植被散射、多地區調色盤過濾與筆刷移除防護已實作 | 驗證大規模植被批量生成的記憶體配置 | `cargo test --release --locked --manifest-path native/against-rome-environment/Cargo.toml nature` |
| `Fortification/FortificationEditSessionCoordinator.cs`, `FortificationPassabilitySync.cs`, `FortificationTypes.cs`, `PalisadeRunPlanner.cs`, `WallStrokePlanner.cs`, `WallTileCatalog.cs` | `native/against-rome-landscape/src/fortification.rs` | 已移植 | 城牆與木柵欄連續路徑鋪設、城門插入與通行性網格自動更新已實作 | 補強雙層城牆交會處的通行阻擋判定 | `cargo test --release --locked --manifest-path native/against-rome-landscape/Cargo.toml fortification` |
| `Atmosphere/AtmosphereEditSession.cs`, `AtmosphereLightingBridge.cs`, `AtmosphereProfile.cs`, `BodenIniData.cs`, `BodenIniSerializer.cs`, `WeatherPresets.cs` | `native/against-rome-ambience/src/atmosphere.rs` | 已移植 | 天候預設（晴天、雨天、霧天等）、`boden.ini` 序列化與光照橋接已完成 | 驗證非標準天候參數的平滑插值 | `cargo test --release --locked --manifest-path native/against-rome-ambience/Cargo.toml atmosphere` |
| `Soundscape/SoundscapeBinaryStorage.cs`, `SoundscapeEditorModule.cs`, `SoundscapeMixEvaluator.cs`, `SoundscapeZoneCatalog.cs`, `SoundscapeZoneModels.cs`, `SoundscapeZonePlanner.cs` | `native/against-rome-ambience/src/soundscape.rs` | 已移植 | 二進位音效區塊規劃、觸發區域權重混音評估已移植 | 實體遊戲中載入音效區塊之相容性驗證 | `cargo test --release --locked --manifest-path native/against-rome-ambience/Cargo.toml soundscape` |
| `AI/AiArchetypeModels.cs`, `CampaignMissionCompiler.cs`, `CampaignMissionModels.cs`, `FactionAiSession.cs`, `WaypointModels.cs` | `native/against-rome-campaign/src/ai/*` | 已移植 | 戰役任務 AI 原型、路徑點規劃、派系 AI 會話與任務腳本編譯已移植 | 針對複雜巡邏迴圈路徑點之邏輯審查 | `cargo test --release --locked --manifest-path native/against-rome-campaign/Cargo.toml ai` |
| `Cinematics/CameraFlightPreviewer.cs`, `CameraTrackSplinePlanner.cs`, `CameraWaypoint.cs`, `CinematicBciCompiler.cs`, `CutsceneModels.cs`, `CutsceneSequenceCatalog.cs`, `MapStartCamera.cs`, `MapStartCameraInjector.cs` | `native/against-rome-campaign/src/cinematics/*` | 已移植 | 鏡頭運鏡樣條插值、過場動畫序列目錄、開場鏡頭注入與 BCI 編譯已移植 | 驗證非均勻時間樣條曲線之運鏡流暢性 | `cargo test --release --locked --manifest-path native/against-rome-campaign/Cargo.toml cinematics` |
| `Objectives/BciObjectiveCompiler.cs`, `ObjectiveDependencyGraph.cs`, `ObjectiveModels.cs`, `ObjectiveRuleCatalog.cs`, `ObjectiveSandbox.cs` | `native/against-rome-campaign/src/objectives/*` | 已移植 | 任務目標相依圖、勝敗條件規則編譯與沙盒模擬已移植 | 補強隱藏目標與多階段目標之觸發檢驗 | `cargo test --release --locked --manifest-path native/against-rome-campaign/Cargo.toml objectives` |
| `Events/ScenarioEventSession.cs`, `Events/Graph/EventGraph.cs`, `EventGraphConverter.cs`, `EventGraphValidator.cs`, `GraphEdge.cs`, `GraphEnums.cs`, `GraphNode.cs`, `GraphNodes.cs`, `GraphPort.cs` | `native/against-rome-scenario/src/graph.rs`, `apps/against-rome-editor/src/event_graph/*` | 已移植 | 事件節點圖資料模型、循環依賴驗證、編譯轉換與 egui 畫布 UI 已實作 | 補強自訂巨集節點的摺疊與封裝 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml event_graph` |
| `Localization/BriefingSaveTransaction.cs`, `BriefingVariableCatalog.cs`, `MultiLanguageTextStudio.cs`, `PutScriptAst.cs`, `PutScriptSyntaxValidator.cs` | `native/against-rome-ambience/src/localization/*`, `app_localization.rs`, `app_tables.rs` | 已移植 | 多語系任務簡報文字工作台、變數置換與 put 腳本語法檢驗已移植 | 驗證罕見特殊控制字元（如顏色碼、分頁符） | `cargo test --release --locked --manifest-path native/against-rome-ambience/Cargo.toml localization` |
| `NativeAssets/MapLightingBitmap.cs`, `MapLightingModel.cs`, `MapLightingShadowMap.cs`, `NativeAlrDocument.cs`, `NativeAlrIndexedFrame.cs`, `NativeAptAttachmentGeometry.cs`, `NativeAptDocument.cs`, `NativeAptLightPoints.cs`, `NativeLightSource.cs`, `NativeObjectLightingCatalog.cs`, `NativePaletteColor.cs`, `NativePreparedParticlePalette.cs`, `NativeShadowCatalog.cs`, `NativeShadowDocument.cs`, `NativeSpriteAnimation.cs`, `NativeSpriteAtlas.cs`, `NativeSpriteCatalog.cs`, `SceneLightingContext.cs` | `native/against-rome-data/src/*`, `native/against-rome-textures/src/*`, `native/against-rome-environment/src/shadow/*`, `native/against-rome-scene/src/lighting.rs`, `atlas.rs`, `scene_object.rs` | 已移植 / 測試通過 | 原始遊戲專屬資產（ALR 圖素、APT 幾何、精靈圖集、陰影 SHAD、頂點光照）完全對齊解碼 | 驗證包含未知負值位移之損壞 APT 模型容錯 | `cargo test --release --locked --manifest-path native/against-rome-data/Cargo.toml` |
| `Packaging/MapBundleContract.cs`, `MapExportPreflightChecker.cs`, `MapPackageManifest.cs`, `MinimapThumbnailRenderer.cs`, `ModBundleExporter.cs` | `native/against-rome-packaging/src/*` | 已移植 | 地圖套件合約、匯出前檢查、ZIP 打包與小地圖縮圖產生器已完成 | 補齊獨立打包 CLI 二進位程式 | `cargo test --release --locked --manifest-path native/against-rome-packaging/Cargo.toml` |
| `Profiling/MapBudgetLimits.cs`, `MapBudgetProfiler.cs`, `MapBudgetReport.cs` | `native/against-rome-packaging/src/profiler/*` | 已移植 | 地圖物件上限、記憶體預算評估與報告產出已移植 | 支援自訂硬體設定檔之預算警戒線 | `cargo test --release --locked --manifest-path native/against-rome-packaging/Cargo.toml profiler` |
| `Diff/MapDiffEngine.cs`, `MapDiffTypes.cs`, `MapVisualDiffOverlay.cs`, `SelectiveRollbackController.cs` | `native/against-rome-packaging/src/diff/*` | 已移植 | 地圖版本差異比對引擎、視覺化差異圖層與選擇性回滾控制器已實作 | 支援以 UI 點選方式進行局部圖元回滾 | `cargo test --release --locked --manifest-path native/against-rome-packaging/Cargo.toml diff` |
| `Scripting/Commands/*`, `Scripting/Execution/*`, `Scripting/Models/*`, `Scripting/Parsing/*` | `apps/against-rome-editor/src/macro_cmd/*` | 已移植 | 10+ 巨集指令解析（筆刷、抬升、壓平、導航修復、散射、替換材質等）與執行器已移植 | 支援指令碼文字檔案批次載入與回放 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml macro_cmd` |

### 3.4 桌面 App 入口與介面盤點 (Desktop App Entrypoints)

| 來源路徑 / UI 類別 | Rust 對應路徑 / 模組 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `src.Modifier/Program.cs` | `native/against-rome-modifier/src/main.rs` (CLI), `apps/against-rome-editor/src/bin/against-rome-modifier-gui.rs` (獨立 GUI), `apps/against-rome-editor/src/modifier_ui.rs` (內嵌分頁) | 已移植 (功能) / 部分 (發行整合) | GUI/CLI 已分別命名為 `against-rome-modifier-gui.exe` 與 `against-rome-modifier.exe`，app Release suite 219 項全通過；publisher 合併驗證尚進行中 | 完成多 binary 發行包 WhatIf、hash/provenance 與乾淨環境驗收 | `cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated` |
| `src.Modifier/UI/LauncherForm.cs` | `apps/against-rome-editor/src/bin/against-rome-launcher.rs`, `apps/against-rome-editor/src/app.rs` (內嵌分頁模式) | 已移植 / 部分 (視窗驗收未做) | Launcher 依 child CLI contract 路由參數；父端 19 項 launcher tests 與 headless `--smoke-test` 均通過。尚無 GPU 視窗驗收 | 在 portable staging/桌面環境驗收整合啟動 | `cargo run --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated --bin against-rome-launcher -- --smoke-test` |
| `src.Modifier/UI/ModifierForm.cs` (含 Data, Layout, Localization, Patches, Presets) | `apps/against-rome-editor/src/modifier_ui.rs`, `apps/against-rome-editor/src/bin/against-rome-modifier-gui.rs` | 已移植 | 功能已嵌入編輯器 `Modifier` 分頁並有獨立 GUI binary（特徵狀態、預覽、套用、備份復原）；app Release suite 219 項全通過 | 後續提升原生檔案挑選器；確認實際 Windows 視窗操作 | `cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated` |
| `src.Modifier/UI/TroopPresetForm.cs` | `apps/against-rome-editor/src/modifier_ui.rs` | 已移植 / 原生視窗待驗 | CustomUnitStats 6 欄草稿編輯、BOM/UTF-8 `.artroop` 匯入匯出與 rfd open/save dialog 已接線；fake-picker tests 驗證取消/草稿安全 | 在 Windows 桌面實際操作 native dialog | `cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-modifier-dialog-check --lib modifier_ui` |
| `src.Modifier/UI/TechDocForm.cs` | `apps/against-rome-editor/src/tech_doc_ui.rs` (`TechDocUi`), `apps/against-rome-editor/src/app.rs` | 已移植 | 無功能缺口。以 `include_str!` 內嵌 `TechDoc.md` / `TechDoc_EN.md`，支援繁體中文與英文即時切換、唯讀捲動閱讀，並已整合於編輯器頂部選單「Tech Doc」按鈕 | 保持文件內容同步 | `cargo test --lib test_tech_doc_app_integration --manifest-path apps/against-rome-editor/Cargo.toml` |
| `src.Modifier/UI/RestoreAllOptionsDialog.cs` | `apps/against-rome-editor/src/modifier_ui.rs` | 已移植 | 全部還原與備份選擇對話框已整合於 Modifier 面板 | 無需額外工作 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml modifier_ui` |
| `src.SaveManager/UI/SaveManagerForm.cs` | `apps/against-rome-editor/src/save_manager_ui.rs`, `src/bin/save-manager.rs`, `src/bin/against-rome-save-manager.rs`, `apps/against-rome-editor/src/toolbox.rs` (`SaveService`) | 主要工作流已移植 / 發行與視窗待驗 | 雙列表、存檔詳情/預覽、備份、刪除、交易式還原、無盡 AI 修復及中英文介面皆有 Rust 對應；`save_manager_ui` 14 tests 通過。C# 原版也沒有 Hex/位移搜尋；portable 與原生視窗尚未驗收 | 在 Windows 實機驗收獨立 GUI，再核對 portable bundle 與 launcher 路由 | `cargo test --release --locked --offline -p against-rome-editor-app --lib save_manager_ui` |
| `src.MapEditor/MapEditorForm.cs` (全 26 個 partial 檔案) | `apps/against-rome-editor/src/app.rs` (egui 桌面主介面與 studio 面板), `apps/against-rome-editor/src/cli.rs` | 核心工作流已移植 / 渲染與實機未驗 | Rust 已有獨立桌面入口、CLI、Landscape/Erosion/Ramp、Scenario+Textures Safe Save、Placement 多選批次編輯；完整 app suite 267 library tests 通過。3D renderer 使用 CPU draw list 而非 C# OpenTK GPU 管線，Windows 視窗與零售圖未驗 | 完成 Windows 桌面工作流與 portable release 驗收；按實際需要補 3D/GPU gap | `cargo test --release --locked --offline -p against-rome-editor-app` |
| `src.MapEditor/MapEditorForm.TerrainRamp.cs` | `apps/against-rome-editor/src/app.rs` (`RampParameters`, two-point selection) + `native/against-rome-editor/src/terrain.rs` | 核心行為已接線 / 2D 限制 | Rust 2D canvas 可點兩點套用 ramp，寬度/邊緣衰減可調、單步 undo；3D view 尚不支援 ramp picking。app integration test 驗證 undo roundtrip | 將 ramp picking 接入 3D viewport ray-pick action，並在 Windows 視窗驗收 | `cargo test --release --locked --offline -p against-rome-editor-app --lib two_point_ramp_ui` |
| `src.MapEditor/MapSelectionForm.cs` | `apps/against-rome-editor/src/maps_ui.rs` | 已移植 / 原生視窗待驗 | 地圖目錄與空白圖頁面已有 rfd 遊戲資料夾選擇器；取消/無效路徑維持目前選擇；8 maps_ui tests 與完整 app suite 230 項通過 | 在 Windows 桌面實際操作 native folder dialog | `cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-modifier-dialog-check --lib maps_ui` |
| `src.MapEditor/Events/EventGraphCanvasControl.cs` | `apps/against-rome-editor/src/event_graph/ui.rs` | 已移植 | GDI+ 自繪節點畫布已改寫為 egui 向量畫布（支援節點拖曳、連線、端口吸附、縮放平移） | 補強多選節點框選與批量對齊 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml event_graph` |
| `src.MapEditor/AiMapPlanningDialog.cs`, `CampaignWaveDialog.cs`, `LayoutApplyDialog.cs`, `ObjectiveStudioDialog.cs`, `PlacementBatchEditDialog.cs`, `ScenarioConditionDialog.cs`, `ScenarioEventDialog.cs`, `SettlementGeneratorDialog.cs`, `TerrainRegionDialog.cs`, `WildLairDialog.cs` | `apps/against-rome-editor/src/*_ui.rs` | 已移植 | 既有 10 個 WinForms 獨立彈跳對話框已完全改寫為 egui 視窗/面板嵌入編輯器 | 增加面板展開/收合狀態記憶 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml` |
| `tools/ArmMapPackage/Program.cs` | `native/against-rome-packaging/src/main.rs` (CLI), `apps/against-rome-editor/src/packaging_ui.rs` (UI) | 已移植 / 部分 (clean Windows 未驗收) | Rust CLI 對照 C# 語法、輸出與錯誤碼，50 項測試通過；六目標 portable staging 已完成，所有 exe hash/length 與 provenance 相符 | 在無 .NET 的 clean Windows 驗收 portable bundle | `cargo test --release --locked --offline --manifest-path native/against-rome-packaging/Cargo.toml --target-dir re_workspace/parent-verify-packaging` |

### 3.5 GPU、視窗與渲染管線盤點 (GPU / Window / Rendering Pipeline)

| 來源項目 / 類別符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `src.MapEditor/Map3DViewControl.cs` (OpenTK GLControl, OpenGL 4, GLSL shaders) | `apps/against-rome-editor/src/viewport_3d.rs`, `native/against-rome-scene` | 部分 / 未驗證 | C# 原版使用 OpenGL 4 硬體管線（GLSL 著色器、VAO/VBO/EBO、材質快取）；Rust 現使用純 CPU 算圖清單並交由 egui 網格繪製，無原生硬體著色器支援 | 評估於 egui 內嵌入自訂 wgpu / glow 著色器渲染管道 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml viewport_3d` |
| GLSL 著色器 (`_terrainProgram`, `_colorProgram`, `_shadowProgram`, `_spriteProgram`) | `native/against-rome-scene/src/draw_list.rs`, `lighting.rs` | 部分 / 未驗證 | 原生 GLSL 動態頂點光照與陰影投射在 Rust 轉為 CPU 端軟體計算；效能與高階光影效果有落差 | 評估是否需要還原 GLSL 硬體著色器（目前編輯器以效能足夠為優先） | 待核對 |
| 粒子渲染 (`Map3DViewControl.Particles.cs`, `SceneParticleRenderer.cs`) | `native/against-rome-scene/src/draw_list.rs` | 部分 / 未驗證 | 原生 OpenGL 粒子混合模式與精靈縮放；Rust 目前僅支援基礎粒子繪製合約 | 補強粒子動態混色與旋轉幾何生成 | `cargo test --release --locked --manifest-path native/against-rome-scene/Cargo.toml` |
| 2D 地圖畫布 (`MapCanvasControl.cs`) | `apps/against-rome-editor/src/app.rs` (`egui::Painter`) | 已移植 | GDI+ 2D 地圖繪製已完全以 egui 畫布實現（高度、通行度、混合圖層） | 支援小地圖浮動多圖層同步預覽 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml` |
| 視窗系統與跨平台介面 | `eframe` / `egui` (x86_64-pc-windows-msvc) | 已移植 / 未驗證 | 已擺脫 Windows Forms，具備現代 DPI 自適應與微軟 CJK 字型載入；尚未於無 GPU 驅動環境實測 | 執行乾淨無預裝環境之 headless 渲染煙霧驗證 | `cargo run --release --manifest-path apps/against-rome-editor/Cargo.toml -- --smoke-test` |

### 3.6 原生 C++ DLL 與掛鉤注入盤點 (Native C++ DLL & In-Game Hooking)

| 來源項目 / 檔案符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `native/argm-trace/CMakeLists.txt` (Win32 x86 MSVC C++ 專案) | `native/against-rome-trace/Cargo.toml` (`[lib] crate-type = ["cdylib"]`) | 已移植 / 未驗證 | Rust 已具備完整替代專案，建置產出 32 位元 `winmm.dll`，但未在真實安裝遊戲中長期穩定掛載驗證 | 在虛擬環境受控測試地圖驗收 Rust `winmm.dll` 與遊戲行程互動 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -TraceOnly` |
| `native/argm-trace/src/detour.cpp`, `lde.h` | `native/against-rome-trace/src/detour.rs`, `lde.rs` | 已移植 | 指令長度解碼 (LDE) 與 Win32 Inline Hook 躍遷代碼生成已移植為純 Rust | 驗證保護分頁切換時多執行緒競爭極端狀況 | `cargo test --release --locked --manifest-path native/against-rome-trace/Cargo.toml detour` |
| `native/argm-trace/src/proxy_winmm.cpp`, `argm_trace.def` | `native/against-rome-trace/src/runtime.rs`, `pe.rs` | 已移植 | WINMM.dll 原生 180+ 匯出函式轉發與攔截已實作，且通過 dumpbin 無動態 CRT 依賴稽核 | 無需額外代碼工作 | `cargo run --manifest-path native/against-rome-trace/Cargo.toml --example audit_exports` |
| `native/argm-trace/src/tracelog.cpp`, `config.cpp` | `native/against-rome-trace/src/tracelog.rs`, `config.rs` | 已移植 | 追蹤記錄器、環形緩衝區與 `argm_trace.ini` 設定讀取已移植 | 驗證硬碟滿載或無寫入權限時之安全忽略 | `cargo test --release --locked --manifest-path native/against-rome-trace/Cargo.toml tracelog` |
| `tools/build-native-data.ps1` (`native/against-rome-data` DLL) | `native/against-rome-data` | 已移植 | 原生資料 FFI DLL (`against_rome_data.dll`) 已具備完整建置與驗證腳本 | 當 C# 全面退場後，此 C-ABI DLL 可轉為純內部 Rust 依賴 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-native-data.ps1` |

### 3.7 Windows 發行、安裝、部署與升級盤點 (Windows Distribution, Packaging, Install, Upgrade)

| 來源項目 / 檔案符號 | Rust 對應路徑 / 模組符號 | 判斷 | 具體缺口 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| `tools/publish.ps1` (.NET win-x64 單檔發行發布) | `tools/publish-rust.ps1` | 已移植 / 部分 | Rust 已具備可重現的綠色便攜目錄發布腳本（打包主程式、dgVoodoo2、winmm.dll、provenance.json）；但尚未有自動化 ZIP 壓縮輸出 | 在 `publish-rust.ps1` 加入可選的 ZIP 壓縮與校驗和產出 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/publish-rust.ps1 -WhatIf` |
| Windows 安裝程式 (Installer: InnoSetup / MSI / WiX) | 無 | 未移植 | 現有 C# 與 Rust 皆僅採可攜版（Portable Folder / Zip）；尚無註冊安裝程式、開始功能表捷徑與解除安裝機制 | 評估是否需要編寫 InnoSetup 腳本製作安裝執行檔 | 待核對 |
| 檔案關聯註冊 (`.arm`, `.artroop`, `.bci`) | 無 | 未移植 | 尚未在 Windows 註冊表建立地圖包與兵種預設檔之副檔名雙擊關聯 | 於 CLI 或安裝腳本加入 `--register-associations` 旗標 | 待核對 |
| 程式版本升級與自我修復 | 無 | 未移植 | 目前無自動更新、檢查新版本或損壞資產線上比對功能 | 實作基礎的版本比對與更新指引提示 | 待核對 |
| 依賴性與執行時期環境 | `tools/Test-ReleaseArtifacts.ps1` | 部分 / 未驗證 | 過去針對 .NET 8.0 Runtime 驗證；Rust 無需任何 .NET 依賴，但需驗收在乾淨 Windows 10/11 缺少 VC++ Redist 之機器上執行 | 在乾淨 Windows Sandbox 中執行便攜版編輯器驗證 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/Test-ReleaseArtifacts.ps1` |

### 3.8 實際遊戲相容性與驗收邊界 (Retail Game Compatibility & Verification Gates)

| 相容性領域 | 測試機制 / 測試檔案 | 判斷 | 具體缺口與未驗證邊界 | 下一個可獨立交付工作 | 可執行驗證命令 |
|---|---|---|---|---|---|
| **純單元與整合測試** | 各 crate 之 `tests/*.rs` (17 個 native crates + app) | 測試通過 | 僅證明演算法與合成資料符合預期，不代表在零售版遊戲中能正確呈現或無崩潰 | 維持本地快速迴圈 CI | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -Release -IncludeDesktop` |
| **Ignored 差分閘門 (Live Differential)** | `test-rust-rewrite.ps1 -Differential` | 測試通過 / Ignored Gate | 包含 editor terrain, placement, scenario 差分；平時列為 ignored，需本機 .NET 環境啟動 | 納入定時差分回歸，逐項消滅殘留差異 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -Differential` |
| **零售遊戲地圖往返 (Retail Maps)** | `native/against-rome-textures/tests/installed_maps_tests.rs` | 測試通過 / 依賴遊戲 | 需存取實際安裝遊戲圖資以驗證真實 PFIL / LZSS 往返；未安裝遊戲機器會被 ignored | 在授權受控機器上執行真圖往返驗收 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -Retail` |
| **實機遊戲載入與遊玩 (In-Game Acceptance)** | 無自動化腳本 (需手動或自動啟動 `Against_Rome.exe`) | 實機尚未驗證 | 由 Rust 編輯器所生成或儲存之地圖（例如 `MAPS/ENDL_005`），尚未在真實遊戲中實機讀取、建造、行軍、觸發勝利並通關 | 依 AGENTS.md 授權規範在受控測試地圖進行遊戲實測 | 手動執行遊戲驗收並記錄於 AI_HANDOFF.md |
| **動態追蹤 DLL 掛載** | `tools/test-rust-rewrite.ps1 -TraceWin32` | 測試通過 / 實機未驗證 | DLL 匯出與靜態 CRT 稽核通過，但未在真實遊戲執行期攔截 Direct3D 呼叫與計時 | 配合測試地圖測試 winmm.dll 啟動記錄檔產出 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -TraceWin32` |

---

## 四、未映射功能與結構缺口彙總 (Unmapped Functionality Summary)

經由對照全部原始碼與專案檔案，以下為 Rust 目前完全尚未映射（Unported）或僅部分映射（Partial）的結構性缺口清單：

### 4.1 獨立二進位應用程式缺口 (Binary Entrypoint Gaps)
1. **Rust Modifier GUI 與 CLI 分立 (`AgainstRomeModifier.exe`)**：
   - C# 原版 `AgainstRomeModifier.exe` 是單一進入點，同時具備 WinForms GUI（`ModifierForm.cs`）與命令列參數分派（`Program.cs`）。
   - Rust 端現況：
     - CLI 進入點：位於 `native/against-rome-modifier/src/main.rs`，預設產出檔名為 `against-rome-modifier.exe`（無依賴原生命令列修補工具）。
     - 獨立 GUI 進入點：`apps/against-rome-editor/src/bin/against-rome-modifier-gui.rs`，Cargo 目標為 `against-rome-modifier-gui.exe`；CLI 則維持 `native/against-rome-modifier/src/main.rs` 的 `against-rome-modifier.exe`。
     - 內嵌模式：已內嵌於地圖編輯器的 `Modifier` 分頁。
     - 驗證：父端完整 editor app Release 測試 **219 passed, 0 failed, 0 ignored**；其中 Modifier GUI integration tests 4/4 通過。
     - 具體缺口：兩個 Cargo 目標已分名；需完成並驗證可攜發行整合，確保 CLI 與 GUI 同時入包而不互相覆蓋。
   - 驗證命令：
     - CLI: `cargo run --release --manifest-path native/against-rome-modifier/Cargo.toml -- --help`
     - GUI/app: `cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated`
2. **獨立存檔管理器 GUI (`AgainstRomeSaveManager.exe`)**：
   - 現況：`apps/against-rome-editor/src/save_manager_ui.rs` 與獨立入口 `src/bin/save-manager.rs` 已實作 C# SaveManager 工作流；另有 `against-rome-save-manager.rs` 同名功能入口。父端 app suite 包含 Save Manager UI tests，整體 **219 passed, 0 failed, 0 ignored**。C# 原版未找到 Hex 檢視或位移搜尋功能，故不列為移植缺口。
   - 具體缺口：尚未確認可攜發行清單包含 `save-manager.exe`；父端尚未直接啟動該 binary 的 smoke-test 或驗收實際 Windows 視窗/遊戲目錄。
   - 驗證命令：`cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated`
3. **獨立地圖打包命令列工具 (`ArmMapPackage.exe`)**：
    - 現況：Rust CLI 對照 C# 語法、輸出與錯誤碼，50 項測試通過；六目標 portable staging 已成功，exe SHA256/length 全數符合 provenance。
    - 具體缺口：尚需無 .NET 的 clean Windows 驗收及零售遊戲安裝/可玩性驗收。
   - 驗證命令：`cargo test --release --locked --offline --manifest-path native/against-rome-packaging/Cargo.toml --target-dir re_workspace/parent-verify-packaging`
4. **多工具統一啟動面板 (`LauncherForm.cs`)**：
   - 現況：Launcher 依 Editor/Modifier/Save Manager CLI contract 路由參數，隔離 smoke-test 不啟動 sibling；父端 19 項 unit tests 通過，headless `--smoke-test` Exit 0。
   - 具體缺口：尚未做桌面 GPU 視窗或 clean portable package 驗收。
   - 驗證命令：`cargo run --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-verify-app-integrated --bin against-rome-launcher -- --smoke-test`

### 4.2 硬體圖形渲染缺口 (GPU Rendering Gaps)
5. **OpenGL 4 / wgpu 硬體著色器管線**：原 C# `Map3DViewControl.cs` 擁有專屬 GLSL 著色器 (`_terrainProgram`, `_shadowProgram`, `_spriteProgram`) 與 VAO/VBO 快取；Rust 目前為 egui CPU 繪製，無 GPU 頂點著色與片元著色器管線。
6. **動態粒子著色器與光暈**：未支援硬體混合模式（Additive blending 等）之即時動態粒子光效。

### 4.3 系統發行與作業系統整合缺口 (OS & Distribution Gaps)
7. **Windows 安裝程式 (Installer / Setup)**：缺少 InnoSetup 或 MSI 形式的標準安裝部署包。
8. **副檔名註冊 (File Associations)**：缺少對 `.arm`（地圖包）、`.artroop`（兵種預設）、`.bci`（腳本）的系統關聯註冊。
9. **自動更新升級與自我修復 (Auto-Updater / Self-Repair)**：無內建線上檢查更新或損壞檔案自動校驗下載功能。

### 4.4 輔助功能與次要對話框缺口 (Auxiliary Dialog Gaps)
10. **技術文件瀏覽視窗 (`TechDocForm.cs`)**：已移植完成！`apps/against-rome-editor/src/tech_doc_ui.rs` 實作 `TechDocUi`，透過 `include_str!` 內嵌 `TechDoc.md`（繁中）與 `TechDoc_EN.md`（英文），支援語系切換與捲動閱讀，並已於 `app.rs` 頂部功能表整合「Tech Doc」按鈕，通過單元測試 `test_tech_doc_app_integration`。此項已不再是未移植缺口。
11. **檔案挑選器對話框整合 (Native File Dialogs)**：Rust Modifier `.artroop` open/save 與 Maps 遊戲目錄 folder picker 已接上 `rfd`，對應測試透過 injected picker 驗證取消與路徑安全；實際 Windows 原生對話框仍未手動啟動驗收。
   - 驗證命令：`cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-modifier-dialog-check --lib modifier_ui`；`cargo test --release --locked --offline --manifest-path apps/against-rome-editor/Cargo.toml --target-dir re_workspace/parent-modifier-dialog-check --lib maps_ui`

### 4.5 遷移期間的發行與 C# 退場紀錄（歷史）
以下內容保留遷移期間的狀態，不代表目前尚有 C# 產品程式或 .NET 發行管線。

12. **Rust Portable Publisher 六執行檔與實際 ZIP 發行的遷移期驗證 (`tools/publish-rust.ps1`)**：
     - 現況：已支援 editor、Modifier GUI、Modifier CLI、Launcher、Save Manager、ArmMapPackage 六個 Cargo targets，以及 Rust i686 trace DLL；父端發行 13 個 stage files，建立含 14 entries 的 ZIP 與 `.zip.sha256`。`tools/test-publish-rust.ps1 -SkipNetworkFetch` 全測試通過，兩次獨立快取離線 staging 的 provenance/ZIP checksum 通過。
    - 具體缺口：
      1. 完整 portable staging 與產物 hash/provenance 已在 repo 外 TEMP 驗證；clean Windows 驗收仍未完成。
      2. 尚未在乾淨且無 .NET/CRT 開發環境的 Windows 驗收便攜包。
      3. 零售遊戲實際安裝與 Rust trace DLL 啟動掛鉤驗收仍未執行。
    - 驗證命令：`pwsh -NoProfile -ExecutionPolicy Bypass -File tools/test-publish-rust.ps1`
13. **C# oracle/probe 退役紀錄與現存驗收缺口**：
    - 遷移時使用的 C# oracle/probe 專案已自目前 tree 移除；Rust fixtures 與測試保留可重現的輸出基準，舊來源可由 Git 歷史唯讀取回。
    - 舊 `tools/publish.ps1`、`.NET solution` 與 C# 主程式已退役；目前使用 `tools/publish-rust.ps1` 和 Cargo targets。clean Windows portable 驗收、實際 GUI 與遊戲內驗收仍未完成，不能從遷移期 staging 測試推定通過。

---

## 五、後續可獨立交付工作與驗證命令總表 (Actionable Next Steps & Verification Commands)

以下為按優先順序整理之可獨立交付任務與精準驗證命令清單：

| 模組 / 領域 | 交付工作名稱 | 具體內容與檔案範圍 | 驗證命令 |
|---|---|---|---|
| **Release Packaging** | 實際建置並驗收六個 Rust 執行檔發行目錄 | 在工作樹穩定時執行真實 portable build/stage，確認六個 binary、第三方資產、provenance 與 hash 均吻合；再於 clean Windows 驗收 | `pwsh -NoProfile -ExecutionPolicy Bypass -File tools/publish-rust.ps1`；檢查輸出目錄 manifest/hash |
| **Launcher** | 驗收 child CLI 參數與隔離 smoke test | 已有參數路由測試、父端 19 項測試及 headless smoke Exit 0；尚缺桌面視窗驗收 | Launcher `--smoke-test` Exit 0，桌面 GPU 尚待驗收 |
| **Release Packaging** | 自動化發行 ZIP 與 SHA256 產出 | 已新增 `tools/RustPortableArchive.psm1`；publisher 在 ShouldProcess 核准後產生六個 exe/資產的 ZIP 與 `.zip.sha256`。會逐項核對 ZIP entry 名稱/內容 hash、拒絕覆寫並在失敗時只清理由本次產生且未被改動的輸出。合成封裝測試通過；真實六 binary ZIP 尚待工作樹穩定後重跑 staging | `pwsh -NoProfile -ExecutionPolicy Bypass -File tools/test-publish-rust.ps1`；真實 staging 使用 `pwsh -NoProfile -ExecutionPolicy Bypass -File tools/publish-rust.ps1` |
| **Modifier UI** | 原生檔案選擇器接線 | 整合輕量原生檔案選擇對話框（如 `rfd`）至 `apps/against-rome-editor/src/modifier_ui.rs`，取代手動路徑輸入 | `cargo test --release --locked --manifest-path apps/against-rome-editor/Cargo.toml modifier_ui` |
| **Differential Gates** | 消除殘留 Ignored 差分閘門 | 針對 `against-rome-editor`、`placement`、`scenario` 的 ignored 測試進行逐項比對並清理為常規測試 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -Differential` |
| **Trace Runtime** | 追蹤 DLL 實機記錄驗證 | 在授權受控遊戲目錄中部署 Rust `winmm.dll`，驗收真實遊戲啟動時 `argm_trace.log` 產生與 Direct3D 掛載 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1 -TraceWin32` |
| **In-Game Map** | 空白與編輯地圖遊戲實機驗收 | 依 AGENTS.md 授權規範將 Rust 編輯產出之地圖備份寫入測試目錄（如 `MAPS/ENDL_005`），進行進遊戲載入與遊玩驗收 | 手動啟動遊戲驗證地圖載入，完成後復原檔案 |

---
*盤點文件版本：2026-10-18 | 產生依據：Repository 原始碼、專案檔與即時測試輸出*
