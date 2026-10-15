# AI Handoff — Rust 全程式改寫

更新：2026-10-15，Antigravity；依照使用者指示恢復 placement crate 任務並完成驗證。

## 目標、Git 與安全界線

- 原目標仍為「整個程式改寫成 Rust 語言」，不是僅加 Rust DLL／CLI／部分新視窗；Modifier、SaveManager、MapEditor、Shared/Core/Modules 及 C++ trace 都要替代。不可縮小完成條件。
- 分支 `主要開發`；最後已提交且核對遠端的基準 `fa16a428cf959645ea2a20ac4e2f459dcdda3297`。本輪第二批尚未 commit/push，無 staged changes；保留全部 dirty/untracked。不要丟棄、reset、clean 或直接提交所有半成品。
- 必先讀 AGENTS.md、這份交接、git status/diff/cached/recent log。已授權 verified work 自動 commit/push，post-commit hook 會推 origin；破壞性操作仍需另外授權。本次暫停不代表授權提交未驗證半成品。
- 全目標尚未完成。installed `C:\Program Files (x86)\Against Rome` 本輪唯讀；修改只在 TEMP 工作副本。真遊戲寫入仍僅特定 custom map，先 TEMP 備份、測後還原；本輪未啟動遊戲、未改安全政策。
- 舊交接完整封存於 `docs/ai-handoff-history-2026-10-15-before-rust-rewrite-pause.md`，包含先前各代理證據、歷史、工具鏈與遊戲限制，未刪除舊代理紀錄。封存 SHA256：5CC30A96A2F85FEEE9E5C14E89EAD42C2A900AF72ED938D899C3EF0DAC611C2E。

## 本輪程序與 ownership

- 原 ownership：placement core → Antigravity（本輪完成安全修正與功能移植）；scenario core/UI → rust_scenario；textures core → Agy；其餘 app/navigation/map_settings/scripts/CI/handoff → Parent。

## 已提交基準 fa16a42（前輪證據，非本輪最新程式驗證）

- 獨立 Rust egui 桌面地形入口、terrain/navigation/modifier/saves crates、run/test 腳本已提交推送。
- 前輪整合 193 Rust pass/0fail；地形 C# differential 24 ramps、22 snapshots、3,502,254 bytes、289 noise 完全一致；TUTOR_01 native smoke 3次 UI update 成功，TUTOR_01/ENDL_000 BMP來源 SHA256 不變。
- 前輪 .NET Host1292pass/33skip、Modules1152pass/2skip，0fail。這些結果不能替本輪新增 GUI、最新安全修正或完整改寫背書。

## 本輪第二批 authoritative 狀態（全部未提交）

### native/against-rome-placement

- 新 Rust 真 DATA pools、parent/light family move/clone/remove、undo/redo、UID/aux tail remap、staged multi-pool save、CLI `ar-placement inspect/move/rotate/set-team/set-vitals`。
- **安全修正實證（已完成修復並全數驗證）**：
  - 修正原因：先前 `fixture_count` 未初始化 `objdata` active byte（`d[8 + s * 9] = 1`），導致 `remove_family(0, 1)` 未變更 `objdata`，使其未被納入 save changes，造成 staging/backup/rollback 4 項測試失敗。現已在 `fixture_count` 正確初始化 active 標記並於釋放槽位時同步清除。
  - 安全保護機制全數通過測試：唯讀 preflight 阻擋（`save_readonly_preflight_never_changes_originals_or_creates_stage`）、檔案權限與 exact backup 保留（`save_preserves_permissions_and_exact_backup_bytes`）、staging 失敗保留 partial candidate（`stage_failure_preserves_partial_candidates_and_all_originals`）、備份失敗保留既有證據（`backup_failure_preserves_existing_evidence_and_all_originals`）、外部意外檔案保護（`rollback_preserves_external_target_and_reports_original_backup`）、外部篡改備份拒用防護（`rollback_never_uses_externally_changed_backup`）。
- **已移植 C# 放置功能（旋轉、陣營隊伍、Vitals 生態值編輯）**：
  - 旋轉：`set_family_rotation`、`rotate_family`（相對角度）、`set_rotation`；角度自動 normalize 為 `[0, 360)` 度；拒絕 nonfinite 角度並維持 snapshot。
  - 陣營隊伍：`set_family_team`、`set_team`；支援 0..15 及 65535（未指派/-1），嚴格邊界驗證，隨 family 閉包傳遞，支援 undo/redo。
  - Vitals：新增 `Vitals` 結構體（HP、morale 與對應比率）、`vitals(slot)` 讀取器、`WorldObject.vitals` 欄位；`set_family_vitals`、`set_vitals`、`set_hp_morale` 寫入器；拒絕 nonfinite 及負數比率，支援 undo/redo。
  - 磁碟往返保存：`placement_edits_disk_save_and_reload` 驗證旋轉/隊伍/vitals 經安全 save 寫入與 reload 後數值 bit-exact 一致。
- **測試命令與通過數**：
  - 一般單元測試：`cargo test --manifest-path native/against-rome-placement/Cargo.toml --target-dir re_workspace/rust-rewrite-targets/native-against-rome-placement`（**20 passed, 0 failed, 3 ignored**）。
  - 顯式 Oracle Gates：`cargo test --manifest-path native/against-rome-placement/Cargo.toml --target-dir re_workspace/rust-rewrite-targets/native-against-rome-placement -- --ignored --nocapture`（**3 passed, 0 failed**：`managed_family_light_auxiliary_differential`、`retail_temp_roundtrip`、`retail_managed_differential`）。
  - 總計 23 項測試全數通過（23 passed, 0 failed）。
- 尚未移植：particle/garrison family、跨圖匯入、自定義新增物件範本；core 維持明拒不支援關聯，不靜默丟棄。

### native/against-rome-scenario + app scenario_ui.rs

- 新 BCI reader/writer、JSON v1–6、GUID/spawn/event conditions/actions/repeat/wave orders/victory/defeat/camera、PFIL保存、CLI；結構化 egui 生成物件/事件/條件/動作/camera、catalog、preview、四檔新目錄export（**不是完整地圖保存**）。prebuilt DATA欄位唯讀。
- 安全修正前：11一般tests +2顯式gate pass；synthetic雙camera2次、retail ENDL_000/TUTOR_01共6次完整 C#/Rust BCI bytes一致，來源 SHA256 不變；CLI真圖 inspect→TEMP compile→parse成功。
- CI synthetic oracle：`cargo test --locked --manifest-path native/against-rome-scenario/Cargo.toml --target-dir re_workspace/rust-scenario-target --test compiler csharp_synthetic_differential -- --ignored --exact --nocapture`（不需retail）。retail gate 缺來源應fail，不得默默跳過當通過。
- **獨立review已證實而未完成最終重驗的缺口**：空compile fastpath接受3-byte壞BCI；CONS缺NUL使append後原constant `abc`變`abcs_getTime`；空alias/越界位置/超count被clamp或接受；alias catalog失敗後驗證放行；prebuilt DATA UID未讀取核對/watch；刪prebuilt遺留DataSlots卡export。TEMP repro `arm-scenario-review-ad324f8e30d04c4eb33b6e12a538dafb`。
- 暫停時已有部分修正落盤：compile_script先parse原BCI、require_alias_catalog、BCI NUL驗證、`compile_script_verified(...active_bindings)` 與 source snapshot；其餘請逐項讀diff/tests查證。**額度中斷，不能宣稱上述缺口全部已修或最後tests通過**。UI8項controller/egui tests也尚未執行。
- pure compiler與caller真DATA驗證契約需釐清；新export只有script/JSON/原版BCI備份，完整DATA與全圖交易仍未整合。

### native/against-rome-textures

- Agy半成品新crate，真資源為地圖 `boden.txt`（常為PFIL）與根目錄 `floortex.dat` ZIP，並非假定textures.dat；含document/catalog/authoring_map/resolver/session/region/CLI/tests。
- Parent native script `67542` 在其尚開發時失敗：installed_maps_tests讀TUTOR_01 `InvalidFormat("boden.txt 缺少有效的 [Dimension]")`，log `re_workspace/rust-wave2-native-tests.log`。當時未decode PFIL；後續Agy已新增lzss/document改動，但**未取得最終tests／真圖往返結果**。
- 恢復時重點review：PFIL短header是否在驗證前 `bytes[..64]` panic；CP1251/UTF8與byte-offset切片；保留未知bytes／無改動原壓縮；save備份/readonly/來源衝突；transition catalog覆蓋；installed test含缺檔return/continue及`if paint.succeeded`，不能當真編輯已成功證據；format.rs對EXE逆向/3005entries等宣稱需證據，不可照單全收。

### Rust app／整合與 CI（尚未成功編譯）

- 新 `toolbox.rs`：直接Rust Modifier/SaveManager控制器，Review/Apply、backup/restore/delete確認與stale snapshots，4tests尚未執行。
- 新 `placement_ui.rs`：真ObjectStore list/filter/move/clone/remove/undo/redo/save、dirty與路徑防護，7tests尚未執行。DATA保存與terrain分開，UI有說明。
- 新 `scenario_ui.rs` 如上；新 `navigation.rs` current-layer連通/path診斷；`map_settings.rs`讀PFIL/raw boden.ini水位/Heightmapstep，尺寸從BMP而不是height scale推導。
- Parent修改app/main/lib/Cargo：工具按鈕、原生中文字型fallback、toolbar換行/sidebar捲動、set_loaded_map、含各工具dirty的主關窗guard、dependencies接線。
- **已知編譯必修**：app/lib.rs與app.rs已引用 `textures_ui::TexturesUi`，但 `apps/against-rome-editor/src/textures_ui.rs` 目前 `Test-Path=False`。該代理額度中斷未建檔；恢復需完成真材質UI或先明確拆除半成品接線，不能宣稱現有app可建置。
- 新 `.github/workflows/rust-rewrite.yml`（Windows runner、.NET8/10、Rust tests+desktop）、修改 `tools/test-rust-rewrite.ps1`納三新crate與synthetic differential。**尚未commit/push，CI未曾執行**，不能聲稱遠端驗證通過。docs/rust-rewrite.md目前仍主要反映fa16a42，待整合後更新。

## 本輪驗證與阻礙

- Parent `dotnet build AgainstRomeModifier.slnx -c Release --nologo --no-restore`：0errors/71warnings；Modules完整1152pass/2skip/0fail。logs `re_workspace/rust-wave2-dotnet-{build,tests}.log`。本輪沒再跑全Host。
- Debug app test反覆在serde build-script被 Windows應用程式控制原則 **4551** 擋；CodeIntegrity Operational事件3077已唯讀查證，尚未到app編譯。`rust-wave2-desktop-tests.log`。
- 正常Release test `18385`也在parking_lot_core/paste build-scripts4551終止，`rust-wave2-desktop-release-tests.log`。未調整policy、未重命名/搬移binary或變更編譯設定繞過限制。遇同handle timeout要確認process終態，不能盲目重啟。
- Rust工具鏈process-local：CARGO_HOME=`D:\Github\AgainstRomeModifiere_workspaceust-runtime\cargo`，RUSTUP_HOME同root的`rustup`，PATH前加cargo/bin；cargo.exe位於CARGO_HOME/bin，repo-local stable1.99。targets一律re_workspace；不要改全域環境。

## 明確恢復後的先後順序

1. 讀現場與這份交接，重新建立disjoint ownership；先處理缺失textures_ui與未完成source修正，不能先claim build。
2. 逐項重驗placement readonly/rollback、scenario六項review finding與新tests；一般＋C#synthetic/retail gates分別記錄真覆蓋。
3. 對Agy textures做獨立review，真PFIL圖／正向paint／save reload／原hash保留；補C# differential，修skip/no-op假正向。
4. 在不變更安全policy前提下，取得正常Rust app編譯與controller tests證據。CI只是尚未提交的驗證入口；若採CI，要如實checkpoint及監看指定run，不得先宣稱成功。
5. 更新功能盤點/交接，審查diff，僅提交適當已驗證範圍。整個程式還缺3D、完整材質/物件family、劇本圖/cutscene/objective、AI與其餘modifier/GUI、C++ trace及發行；不要把本批完成當總目標完成。

目前狀態：**恢復開發：placement 安全性修正與功能移植已完成並驗證通過。**
- 2026-10-15 已提交推送：0a7d3a6（placement/scenario/textures + inventory，Claude 獨立重跑 20/17/24 pass）、527c4f1（WP-03..08，獨立重跑 modifier44/saves23/environment33/landscape40/packaging28/trace73 pass，0 fail；trace i686 DLL 未建置）。
- app（apps/against-rome-editor）WP-09 已完成移植與驗證：event_graph（節點模型/無損雙向轉換/靜態診斷/egui畫布）、macro_cmd（指令解析/變數/歷史/回滾/地形/物件/材質/尋路修復）、console（互動控制台/自動補全/清屏/歷史），`cargo test --release` 75 pass / 0 fail；未 commit/push。
- 第三波 job `7edf487115c7`：maps、modifier 剩餘(EndlessAi/Features/Services/CLI)、campaign(AI/Objectives/Cinematics)、ambience(Atmosphere/Soundscape/Localization)、ai-planner、scene(3D 場景邏輯)。之後還需：app 接線所有新 crate、3D viewport 繪製、Modifier GUI 對話框、最終移除/退役 C#。
