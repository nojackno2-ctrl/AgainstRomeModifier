# 全程式 Rust 改寫：最終遷移紀錄

使用者於 2026-10-15 要求整個程式改寫成 Rust。本文件是遷移完成後的最終紀錄（原為進行中的計畫與對照表）。舊 C#／.NET 與 C++ 原始碼已自工作樹移除，仍可由 Git 歷史取回，例如 `git show 8d49cb3:<path>`。

## 完成條件（已達成）

- 所有產品程式碼均為 Rust：修改器、存檔管理器、地圖編輯器、啟動器、封裝 CLI、追蹤 DLL 與逆向工程探針。
- 建置、測試與執行不再需要 .NET、MSBuild、C++ 工具鏈或 Python。
- 已有 Rust 實際入口：`apps/against-rome-editor`（egui 桌面程式，含 `against-rome-launcher`、`against-rome-modifier-gui`、`against-rome-save-manager` 等 bin）、`native/against-rome-modifier`（CLI）、`native/against-rome-packaging`（`ArmMapPackage`）。
- 遷移期間用來比對行為的 C# oracle 輸出，已在刪除 C# 前凍結成 fixtures（Rust 測試不再需要 dotnet）。

## 對照摘要（原 C#／C++ 範圍 → Rust crate）

| 原範圍 | Rust 位置 |
|---|---|
| `src.Shared`（PFIL/LZSS、ALR、APT、粒子、資料池）、NativeAssets | `native/against-rome-data` |
| `src.Core`（Patches、Features、Bci、EndlessAi、Services、TroopConfig、Localization）、`src.Modifier` | `native/against-rome-modifier`（`features.rs`、`services.rs`、`exe_*`、`objdef`、`ress`、`cl_script`、`epara`、`team_dat`、`bci_features`、`endless_ai`、`install`、CLI）；本地化在 `native/against-rome-ambience` |
| `src.SaveManager`、SaveBackupService | `native/against-rome-saves` + `apps/against-rome-editor/src/save_manager_ui.rs` |
| `src.Shared/Maps`、地圖複製／刪除／空白地圖／交易 | `native/against-rome-maps` |
| Modules/Terrain | `native/against-rome-editor`、`native/against-rome-landscape` |
| Modules/Pathfinding | `native/against-rome-navigation` |
| Modules/Placement、LevelObjectStore | `native/against-rome-placement` |
| Modules/Scripting、Events | `native/against-rome-scenario` |
| Modules/Cinematics、Objectives、AI、campaign | `native/against-rome-campaign`、`native/against-rome-ai-planner` |
| Nature、Settlement、WildLair、Shadow | `native/against-rome-environment` |
| Atmosphere、Soundscape、Localization | `native/against-rome-ambience` |
| Modules/Rendering、2D／3D view | `native/against-rome-scene`、`native/against-rome-textures` + app 的 `viewport_3d.rs` 等 |
| Packaging、Diff（C# `ArmMapPackage`） | `native/against-rome-packaging` |
| `native/argm-trace`（C++） | `native/against-rome-trace`（Rust Win32 proxy／hook／trace） |
| `src.Launcher`、`MapEditorForm`、WinForms UI | `apps/against-rome-editor`（egui） |
| C# RE probes、Python 探針（`ar-re`） | `tools/re/*-rs`（含 `ar-re-tools-rs`） |
| csproj／MSBuild／`tools/publish.ps1` | Cargo workspace、`tools/publish-rust.ps1` |

逐項遷移期間的詳細盤點仍保存在 [rust-rewrite-inventory.md](rust-rewrite-inventory.md)、[rust-port-inventory.md](rust-port-inventory.md)、[rust-rewrite-app.md](rust-rewrite-app.md)（皆為歷史文件）。

## 驗證指令

```powershell
cargo test --workspace --locked
powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-rust-rewrite.ps1            # 逐 crate；-IncludeDesktop 納入桌面、-Release、-Retail（需本機零售圖資）、-TraceOnly
powershell -NoProfile -File tools/test-publish-rust.ps1                                    # 發行腳本契約測試（PowerShell 7）
powershell -NoProfile -File tools/publish-rust.ps1                                         # 可攜 ZIP staging（PowerShell 7）
```

最後一次完整本機驗證（2026-10-20）：`cargo test --workspace --locked --offline` 2215 passed / 0 failed / 15 ignored（忽略項目需要零售遊戲或本機證據）。CI 見 `.github/workflows/rust-rewrite.yml`、`rust-release-candidate.yml`、`release.yml`。零測試的閘門不得視為通過。

## 尚未驗證（Not yet verified）

- 沒有執行過實際 GUI 互動（桌面程式的點擊／拖曳／畫面與 3D 視圖僅有單元與整合測試）。
- 沒有遊戲內驗收（修改器補丁、地圖編輯結果在真實遊戲中的行為）；測試通過不等於遊戲內可用。
- 沒有在乾淨 Windows 上驗收可攜發行包（解壓、啟動、權限／UAC、dgVoodoo2／追蹤 DLL 部署）。
- `tools/test-publish-rust.ps1` 的 PowerShell 7 發行契約測試只在 CI 執行；本機沒有 pwsh 7，僅做過語法解析。
- 渲染保真度（粒子混合、超過 64 盞光源、RNG／FPU 重播）不在範圍內：本專案是地圖編輯器，不是遊戲重製。

不得將測試全綠視作遊戲內可用的證據。
