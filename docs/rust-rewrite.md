# 全程式 Rust 改寫

使用者於 2026-10-15 要求整個程式改寫成 Rust。完成條件是既有修改器、存檔管理器與地圖編輯器的功能均有 Rust 實作及實際入口，建置與執行不再需要 C#／.NET。Rust DLL、FFI 或只有部分功能的新視窗都不足以代表完成。

## 功能與證據對照

| 範圍 | 現有來源 | Rust 改寫位置／狀態 |
|---|---|---|
| 原生資料與素材 | `src.Shared`、`Modules/NativeAssets` | `native/against-rome-data` 已有 ALR、APT 文件／幾何／附件、light pool、particle definition、PFIL/LZSS 等解析與寫入核心；其餘資料池、資產載入和行為仍須逐項盤點。Release suite 目前通過；不代表完整覆蓋。 |
| 地形編輯 | `Modules/Terrain`、`MapEditorForm.Terrain*` | `native/against-rome-editor` 有尺寸、取樣、筆刷、歷史、斜坡與 emboss fit/relight；`native/against-rome-landscape` 有道路、河流、懸崖、堡壘和水力／熱侵蝕。父端 2026-10-18 Release tests：editor 14 passed，landscape 56 passed；live .NET differential 有 ignored gate。材質編輯另見下列 UI/材質列。 |
| 通行與道路診斷 | `Modules/Pathfinding` | `native/against-rome-navigation` 有路網、連通、formation、clearance、道路間隙與診斷；app 有 `navigation.rs` 即時診斷入口。Native Release tests 通過，仍待實際桌面互動驗收。 |
| 修改器 | `src.Core/Core/Patches`、`Features`、`src.Modifier` | `native/against-rome-modifier` 有 EXE/INI/BCI、物件資料、隊伍、部隊統計、安裝偵測、交易、安全存檔服務與 CLI；app 有 profile/preview/apply 入口。仍須逐項核對完整 Feature Registry、GUI parity 及真實遊戲檔相容。2026-10-18 Release crate tests 通過。 |
| 桌面入口 | `LauncherForm`、`MapEditorForm` | `apps/against-rome-editor` 是 egui 原生桌面入口，已有地圖、地形、材質、放置、scenario/event、campaign、environment、ambience、navigation、AI planning、modifier、save manager、packaging 等面板；Rust Modifier 已有 CustomUnitStats 六欄編輯及 `.artroop` 手動路徑匯入/匯出；Save Manager AI repair 於 `3313935` 加入，唯讀存檔完整性驗證於 `f018cbb` 加入。2026-10-18 父端 Release app suite 160 tests passed (148 unit + 12 integration)；`cargo check --all-targets` 無 warning。真正 GUI/遊戲工作流程仍需驗收。 |
| 存檔管理 | `SaveBackupService`、`src.SaveManager` | `native/against-rome-saves` 有 ZIP/manifest、scan、preview、backup/restore/delete、rollback、verification、backup health/recovery、AI repair；Rust app toolbox 已有 review/confirm 操作與 SaveService AI repair。仍須核對 C# 完整 UI parity、故障注入和使用者資料相容。2026-10-18 native 35 tests passed、1 .NET differential ignored。 |
| 放置與關聯池 | `Modules/Placement`、`LevelObjectStore` | `native/against-rome-placement` 已有交易式 family closure/remap、引用驗證、移動／複製／刪除、undo/redo、存檔安全與 app UI。2026-10-18 native 37 tests passed、6 differential/retail tests ignored；真圖往返與安裝版遊戲驗收仍未完成。 |
| 劇本、事件及開場 | `Modules/Scripting`、`Events`、`Cinematics`、`Objectives` | `native/against-rome-scenario` 已有 BCI parse/compile、safe load/save/rollback，app 有 scenario 與 event graph UI；`native/against-rome-campaign` 有 AI/cinematics/campaign pipeline。2026-10-18 scenario 26 tests passed、3 differential tests ignored；真實腳本 corpus 和遊戲內結果仍待驗證。 |
| 其他設計工具 | Nature、Atmosphere、Soundscape、Settlement、WildLair、Fortification、AI、Packaging、Diff、Localization | Rust crates 已涵蓋環境生成、氛圍／音景／本地化、AI map planner、campaign、packaging contracts 等多項；桌面有部分入口。需比對每個 C# 模組，尚未證明全功能 parity。 |
| 渲染與互動 | `Modules/Rendering`、2D／3D view | `native/against-rome-scene` 有相機、draw list、lighting、culling、picking、ray picking、terrain mesh 等 CPU-side 準備；不能據此宣稱完整 GPU renderer、完整材質／粒子/物件視覺或 3D app parity。需完成畫面與互動驗收。 |
| 發行 | csproj／MSBuild／publish scripts | Rust workspace、CLI 與 desktop crate 可建置，驗證腳本涵蓋 17 個 native crates；正式 Rust 可攜發行、版本/provenance、安裝／升級及替代舊 .NET 發行流程仍待完成。 |
| 原生追蹤 DLL | `native/argm-trace`（C++） | `native/against-rome-trace` 有 Rust Win32 proxy/hook/trace runtime，父端 x64 76 tests、i686 79 tests 通過；仍須核對 C++ 完整 parity、DLL 部署/卸載與授權範圍內真實遊戲 runtime，不能只以 crate tests 宣告替換完成。 |

## 驗證入口

`powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1` 對 Rust 資料核心與新核心逐 crate 執行 `cargo test --locked`；加 `-IncludeDesktop` 納入桌面 crate，加 `-Release` 測試 Release，加 `-Differential` 實際執行 C#／Rust 地形比對（需 .NET）。ExecutionPolicy 參數僅作用於該程序。腳本優先使用專案本機工具鏈，只更動該程序環境；所有編譯輸出位於 ignored `re_workspace`。

每個測試結果僅證明它涵蓋的功能。仍須完成桌面實際啟動、既有地圖往返、失敗回滾、修改器／存檔相容及授權範圍內遊戲驗收。不得將測試全綠視作整體改寫完成。移植期間保留現有 C#，待對應 Rust 功能、使用入口和驗證到位後再逐段移除。

2026-10-18：集中腳本已納入全部 17 個 native Rust crates；桌面入口仍由 `-IncludeDesktop` 啟用。`-Differential` 的各閘門必須實際產生至少一項 passing test，零測試不能視為通過。加 `-Retail` 顯式執行材質真圖 PFIL／TEMP 保存往返測試；需要本機零售遊戲圖資，缺檔應失敗。一般 CI 不執行這兩項 retail 測試，會清楚列為 ignored，其他 synthetic 測試照常執行。這些驗證不代表遊戲內驗收。
