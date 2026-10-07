# 地圖編輯器驗收矩陣

更新：2026-10-07，Codex。此表以目前 repository 程式碼與合成 TEMP 地圖測試為準。整體開發尚未完成。

## 已驗證的編輯與儲存流程

| 項目 | 驗證內容 | 證據（tests 下的測試檔） |
| --- | --- | --- |
| 地圖建立／刪除 | manifest 失敗回滾新建正式槽位、保留來源、原槽位重試；拒絕覆蓋既有final/tmp；拒刪原廠與未登記地圖 | `AgainstRomeModifier.Tests/MapEditorPhase1Tests.cs` |
| 原廠唯讀／儲存前置驗證 | 原廠帶marker仍唯讀；移除marker、原廠槽位、錯誤root在開啟briefing前拒存；保留dirty與bytes，恢復marker可retry | `AgainstRomeModifier.Tests/MapEditorSaveAccessTests.cs` |
| 備份排除 | 自製005/999含子目錄team.dat不進基準、不讀鎖檔、不產bak；保留既有bak，原版team/bak正常 | `AgainstRomeModifier.Tests/CustomMapBackupTests.cs` |
| 環境屬性 | en-US/de-DE/fr-FR下七值、Heightmapstep、雨滴儲存／重開一致；dirty清除、未知行保留、重存bytes相同 | `AgainstRomeModifier.Tests/MapEditorEnvironmentSaveTests.cs` |
| 文字與輸入提示 | 單值跳脫/100 bytes/NUL/CP1251防護；純文字/PFIL保留其他行；輸入提示與拒存bytes/dirty保留、retry；標題/8team/簡報save/reopen/重存不變 | `AgainstRomeModifier.Tests/MapTextEscapingTests.cs`、`MapEditorTextSaveTests.cs` |
| 高度、通行 | 筆畫、undo/redo、基準；真正表單儲存圖層與快取失效 | `AgainstRomeMapEditor.Modules.Tests/TerrainHeightEditSessionTests.cs`、`AgainstRomeModifier.Tests/MapEditorFormTerrainIntegrationTests.cs` |
| 原版材質 | 四角烘焙、不支援接縫拒絕、滑鼠筆畫整筆回滾、undo/redo | `AgainstRomeMapEditor.Modules.Tests/TerrainBlendModuleTests.cs`、`AgainstRomeModifier.Tests/MapEditor3DTests.cs` |
| AI 套用 | 材質區域拒絕保留先前接受區域；模式切換；材質獨立 undo，高度/通行共用 undo；儲存及重新開图 | `AgainstRomeModifier.Tests/MapEditorAiWorkflowIntegrationTests.cs` |
| AI 生成 UI | 預設 Laguna；先生成/檢視再套用；取消、部分失敗、重試、舊方案失效；一次套用 | `AgainstRomeModifier.Tests/AiMapPlanningDialogTests.cs` |
| AI 序列推論 | 三角色依序執行；跨 planner 共用單一推論鎖；取消排隊不送出請求 | `AgainstRomeModifier.Tests/MultiAiMapPlannerTests.cs`、`OllamaSingleInferenceTests.cs` |
| 放置物件 | 編輯/複製身份、快照隔離、批次原子性；多選 undo/redo、越界拒絕 | `AgainstRomeMapEditor.Modules.Tests/PlacementBatchRegressionTests.cs`、`AgainstRomeModifier.Tests/PlacementBatchUiTests.cs` |
| 單兵與自然物件 | 儲存失敗後重試、保留部隊目標身份；自然物件只寫入一次、更新標記及再次刪除 | `AgainstRomeModifier.Tests/MapEditorSavePlacementRegressionTests.cs` |
| 儲存交易 | 前置驗證拒絕不寫檔；缺失/損壞 BCI 後回滾 bytes、快取及新增檔案；保留 dirty 可重試 | `AgainstRomeModifier.Tests/MapEditorSaveTransactionTests.cs` |
| 事件與目標 | 事件 session、永久目標身份、矩形包含邊界、勝敗 terminal guard、JSON v6、對話框編輯 | `AgainstRomeMapEditor.Modules.Tests/ScenarioEventSessionTests.cs`、`AgainstRomeModifier.Tests/ScenarioEventsTests.cs`、`ScenarioEventDialogTests.cs` |
| 3D 資料計算 | 高度插值、網格、相機限制、射線選取、材質 atlas | `AgainstRomeModifier.Tests/MapEditor3DTests.cs` |

完整 solution 同時涵蓋 Modifier 與 SaveManager，總通過數不能當成地圖編輯器的功能數。部分測試在未提供原版資料或未啟用 live 環境時直接返回；通過總數也不能證明這些實機路徑已執行。

## 尚待驗收

| 範圍 | 必須取得的證據 | 目前狀態 |
| --- | --- | --- |
| UI 視覺 | 主畫面及 AI/事件對話框在常用視窗尺寸、DPI 下沒有遮擋，模式切換提示一致 | 96 DPI 已檢查主畫面 1440×900/1100×700、最小尺寸各分頁、AI 880×740/640×580、中英事件 560×420；修正放置提示與刪除按鈕截字。高 DPI 尚待驗收 |
| OpenGL 顯示 | 真正渲染高度、通行覆蓋與材質更新，選取位置符合畫面 | 純計算與隱藏表單測試不驗證 OpenGL 畫面 |
| 遊戲載入與存讀檔 | 匯出地圖能載入；儲存/讀檔後事件與永久目標仍正確 | 尚未驗證 |
| 完整還原 | 預設保留自製圖；選擇刪除時使用受控刪除，失敗保留資料 | 目前RestoreAll尚無保留／刪除選項，仍待實作與驗證 |
| 區域事件、勝敗 | 邊界進出、部隊死亡、重複條件、同 tick 多事件、結算畫面 | Compiler/VM 與 repo EXE 靜態證據已有；遊戲內尚未驗證 |
| 部族、多人 | 不同部族與多人槽位的放置、腳本、勝敗及同步 | 尚未完成涵蓋矩陣與遊戲內驗證 |
| AI 地圖可玩性 | 通路、水域、起始位置、資源與建築空間在遊戲內可用 | 生成/套用通過不等於可玩性驗收 |

本輪不得存取或修改遊戲安裝目錄。開發順序以 `map-editor-roadmap.md` 為準；`map-editor-requirements-audit.md` 列出原規格的實際缺口。原廠圖判定／儲存前置驗證與環境跨地區資料流程已修；接著核對文字與備份流程。合成小數案例驗證編輯器的數值保存，不證明遊戲接受所有小數参数。

96 DPI 視覺證據位於 `TEMP/ArmVisualQA_1c01f5e6b6e84fe490ec6339c257de49`：獨立 STA harness 使用合成地圖、2D 畫布及 stub AI，透過 `DrawToBitmap` 產生 PNG 後檢視；中英放置分頁最小尺寸皆完整顯示提示與按鈕。這不驗證實際推論、OpenGL 或高 DPI。

## 重現本輪完整驗證

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore
dotnet test AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore --no-build --logger 'console;verbosity=quiet'
```

最新工作樹結果：build 0 警告/0 錯誤；宿主 521 通過/21 略過，modules 45 通過，共 566 通過/21 略過/0 失敗。AI 預覽 WIP 已編譯但功能未驗證，不納入這批提交的功能成果。未設定 `ARM_GAME_PATH`、`ARM_OLLAMA_LIVE`。以上 `--no-restore` 命令須先有相同 artifacts 路徑的 restore；新環境先移除 build 的 `--no-restore`。

`b9b9965` 的獨立 git archive 快照位於 `TEMP/ArmCoreCommitQA_e70f5f1a2802428abb9fd7546428d1a2/source`，不包含 AI 預覽 WIP；独立還原／建置／完整測試同樣為 0 警告/0 錯誤、536 通過/21 略過/0 失敗。
