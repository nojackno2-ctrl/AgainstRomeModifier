# 全程式 Rust 改寫

使用者於 2026-10-15 要求整個程式改寫成 Rust。完成條件是既有修改器、存檔管理器與地圖編輯器的功能均有 Rust 實作及實際入口，建置與執行不再需要 C#／.NET。Rust DLL、FFI 或只有部分功能的新視窗都不足以代表完成。

## 功能與證據對照

| 範圍 | 現有來源 | Rust 改寫位置／狀態 |
|---|---|---|
| 原生資料與素材 | `src.Shared`、`Modules/NativeAssets` | `native/against-rome-data` 已有部分解析；仍須逐項對照其餘資料池、寫回與資產載入 |
| 地形編輯 | `Modules/Terrain`、`MapEditorForm.Terrain*` | `native/against-rome-editor` 已有尺寸、取樣、筆刷、歷史與斜坡；24 ramps／22 snapshots 真 C# differential 通過。材質與其餘 sculpt 仍待移植 |
| 通行與道路診斷 | `Modules/Pathfinding` | `native/against-rome-navigation` 已有網格、連通、clearance、道路間隙與修復建議，12 tests；UI 尚待接入 |
| 修改器 | `src.Core/Core/Patches`、`Features`、`src.Modifier` | `native/against-rome-modifier` 三種 EXE 補丁、交易與 CLI 已有15 tests（含部分替換失敗恢復）；其餘 feature registry、INI／BCI、部隊設定與 UI 尚須移植 |
| 桌面入口 | `LauncherForm`、`MapEditorForm` | `apps/against-rome-editor` 已接入 Rust 地形核心，22 tests、原生視窗 smoke 與兩張原版圖唯讀載入通過；只具備部分 2D 地形工具，其餘 UI 尚待移植 |
| 存檔管理 | `SaveBackupService`、`src.SaveManager` | `native/against-rome-saves` 已有 ZIP／manifest、備份、還原回滾、預覽讀取及 CLI，8 tests；AI 修復與 UI 尚待移植 |
| 放置與關聯池 | `Modules/Placement`、`LevelObjectStore` | 尚待完整交易、family remap、存讀檔、undo/redo 與真圖驗證 |
| 劇本、事件及開場 | `Modules/Scripting`、`Events`、`Cinematics`、`Objectives` | 尚待 BCI 編碼、注入、事件／波次／鏡頭與存檔回滾 |
| 其他設計工具 | Nature、Atmosphere、Soundscape、Settlement、WildLair、Fortification、AI、Packaging、Diff、Localization | 尚待逐項移植、UI 接入與行為驗證 |
| 渲染與互動 | `Modules/Rendering`、2D／3D view | 尚待完整材質／物件／粒子／光源、picking、工具互動與實際畫面驗證 |
| 發行 | csproj／MSBuild／publish scripts | 尚待 Rust 發行入口、可攜包、版本與 provenance；保留現有發行流程直到替代品驗證完成 |
| 原生追蹤 DLL | `native/argm-trace`（C++） | 尚待 Rust 替代及 Win32 ABI／hook／記錄相容驗證；不能只移除 C# 便宣稱整個程式是 Rust |

## 驗證入口

`powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1` 對 Rust 資料核心與新核心逐 crate 執行 `cargo test --locked`；加 `-IncludeDesktop` 納入桌面 crate，加 `-Release` 測試 Release，加 `-Differential` 實際執行 C#／Rust 地形比對（需 .NET）。ExecutionPolicy 參數僅作用於該程序。腳本優先使用專案本機工具鏈，只更動該程序環境；所有編譯輸出位於 ignored `re_workspace`。

每個測試結果僅證明它涵蓋的功能。仍須完成桌面實際啟動、既有地圖往返、失敗回滾、修改器／存檔相容及授權範圍內遊戲驗收。不得將測試全綠視作整體改寫完成。移植期間保留現有 C#，待對應 Rust 功能、使用入口和驗證到位後再逐段移除。
