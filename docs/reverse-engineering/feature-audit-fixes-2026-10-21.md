# 2026-10-21 功能稽核修復

本輪承接 `bfb6597` 的 [49 項稽核](feature-audit-2026-10-21.md)。稽核文件保留為修復前的歷史快照；下表記錄新行為與驗證，不能用編譯通過代替遊戲內驗收。

| 問題 | 修復行為 | 驗證狀態 |
|---|---|---|
| A1 原生寬螢幕實驗重新啟用 | 只還原可辨識的舊 EXE 六點，不再寫入實驗補丁；全套安裝自動部署 dgVoodoo 置中，未知原生狀態拒絕啟用 | EXE 原版不變、舊補丁遷移、未知拒絕與安裝組合定向測試通過；畫面未重新實機驗收 |
| A2 直接 M2／M3／M4 遺失 | 直接 ID 與舊 modules map 合併為 Core、讀回鏡射；切換 Core／RomanReinforcementGarrison 主功能會同步別名，避免停用後又啟用 | JSON 往返、舊版輸入、停用後往返的回歸測試通過 |
| A3 駐軍倍率固定 3 | registry Int 與舊 Multiplier API 共用唯一值；setter 更新真正 P20 patch，保留舊布林輸入與明確優先序 | 零售 BCI 記憶體 2／3／5／10 倍切換、偵測與精確還原 **1 passed**；別名／倍率回歸納入 workspace |
| A4 代表列／單檔偵測 | 以原版基準和實際 patcher 重建完整候選，核對全部適用列與必要檔案；未知／部分套用不得報啟用 | 完整偵測與相關舊測試納入 workspace，**0 failed** |
| A5 未知 team.dat 誤判 | 人口欄位與所有適用地圖完整核對；合法無關欄位可保留，未知不推定已套用 | 所有地圖／隊伍完整性回歸納入 workspace，**0 failed** |
| A6 自訂屬性與狀態 | 不再依賴 Balance：關閉 Balance 時以原版基準合併；讀取目前狀態清除未驗證草稿；全部關閉也清除自訂設定 | metadata、GUI model 與偵測回歸納入 workspace，**0 failed**；任意磁碟數值不虛構成原始自訂設定 |
| A7 未知速度靜默成功 | 要求倍率大於 1 而兩組常數未知時回傳錯誤，不發布局部補丁；未要求改速度時保留未知資料 | 0／負值／NaN／Infinity／不一致倍率、冪等與還原定向測試通過 |
| A8 部署身分與完整性 | trace 為 winmm.dll；核對所有受管理 dgVoodoo binaries 與 trace DLL 雜湊，必要設定檔存在；合法使用者設定保留；重複置中設定鍵不認作啟用 | 3 項部署完整性回歸與 workspace **0 failed**；DLL 已載入／hook 已執行仍須獨立驗收 |
| collision 門檻疑點 | 從真正 loader、尋路與空地選點 consumer 確認；聚落改用 `>=6`，診斷保留非零保守阻擋 | navigation／editor 聚落契約測試與 workspace **0 failed**；遊戲內路徑未驗 |

## 界限

- 安裝版 EXE／BCI 僅讀取與記憶體測試；TEMP 測試與真實遊戲內行為分開記錄。
- Frozen C# fixtures 保留為歷史資料；已確認的舊缺陷不再以「parity」為由保留，相關測試改驗新契約。
- 工作樹既有 AGENTS.md、README.md 修改保留；未改根 Cargo.lock。

## 最終驗證

`cargo check --workspace --locked --offline --target-dir re_workspace/target-w20`、`cargo test --workspace --locked --offline --target-dir re_workspace/target-w20 -- --test-threads=1`、`cargo build --workspace --locked --offline --target-dir re_workspace/target-w20` 均完成；282 test-result groups 累計 **2,347 passed／0 failed／18 ignored**。零售唯讀 gates（`retail_exe_feature_audit`、P20 BCI）另 **2 passed／0 failed**，全程在記憶體操作安裝版來源，沒有啟動遊戲。`git diff --check` 通過；`cargo fmt --all -- --check` 受 repo 內既有未格式化檔案阻擋，沒有套用全 workspace 格式化。保留 `AGENTS.md`／`README.md` 使用者修改，未改根 Cargo.lock；遊戲內 GUI／可玩性及 DLL 實際載入仍未驗證。
