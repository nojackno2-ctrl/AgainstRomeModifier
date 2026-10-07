# 地圖編輯器驗收矩陣

更新：2026-10-07，Codex。此表以目前 repository 程式碼與合成 TEMP 地圖測試為準。整體開發尚未完成。

## 已驗證的編輯與儲存流程

| 項目 | 驗證內容 | 證據（tests 下的測試檔） |
| --- | --- | --- |
| 地圖建立／刪除 | manifest 失敗回滾新建正式槽位、保留來源、原槽位重試；拒絕覆蓋既有final/tmp；拒刪原廠與未登記地圖 | `AgainstRomeModifier.Tests/MapEditorPhase1Tests.cs` |
| 原廠唯讀／儲存前置驗證 | 原廠帶marker仍唯讀；移除marker、原廠槽位、錯誤root在開啟briefing前拒存；保留dirty與bytes，恢復marker可retry | `AgainstRomeModifier.Tests/MapEditorSaveAccessTests.cs` |
| 備份排除 | 自製005/999含子目錄team.dat不進基準、不讀鎖檔、不產bak；保留既有bak，原版team/bak正常 | `AgainstRomeModifier.Tests/CustomMapBackupTests.cs` |
| 額外槽位 AI 修補 | 真正 P1 在 000–004／005／999 的 detect、buffered apply、save、fresh reload、還原；排除非數字／暫存／刪除中／巢狀路徑 | `AgainstRomeModifier.Tests/EndlessAiAdditionalMapTests.cs` |
| 自製圖完整還原選項 | 預設保留，受控刪除；未知/空目錄保留；還原/後續/中途刪除失敗恢復目錄與manifest；真正runner/engine拒絕/STA dialog | `AgainstRomeModifier.Tests/CustomMapRestoreTests.cs`、`RestoreAllOptionsDialogTests.cs` |
| 環境屬性 | en-US/de-DE/fr-FR下七值、Heightmapstep、雨滴儲存／重開一致；dirty清除、未知行保留、重存bytes相同 | `AgainstRomeModifier.Tests/MapEditorEnvironmentSaveTests.cs` |
| 文字與輸入提示 | 單值跳脫/100 bytes/NUL/CP1251防護；純文字/PFIL保留其他行；輸入提示與拒存bytes/dirty保留、retry；標題/8team/簡報save/reopen/重存不變 | `AgainstRomeModifier.Tests/MapTextEscapingTests.cs`、`MapEditorTextSaveTests.cs` |
| 高度、通行 | 筆畫、undo/redo、基準；真正表單儲存圖層與快取失效 | `AgainstRomeMapEditor.Modules.Tests/TerrainHeightEditSessionTests.cs`、`AgainstRomeModifier.Tests/MapEditorFormTerrainIntegrationTests.cs` |
| 3D 通行覆蓋 | 真正 GPU 擷取驗證非對稱遮罩與 picker 位置、非零值／快照隔離、paint/undo/redo、切換工具、save/reopen／清除；texture 隨 context 釋放 | `AgainstRomeModifier.Tests/MapEditorCollisionViewTests.cs`（本機強制 ARM_OPENGL_REQUIRED=1） |
| 3D 資源恢復 | 缺高度圖退回 2D，恢復高度圖並重開後按鈕與診斷恢復，保留 fallback 時儲存的文字；未驗證 GPU 驅動故障後重試 | `AgainstRomeModifier.Tests/MapEditor3DFallbackTests.cs` |
| 同表單顯示重試 | 補回素材庫、拒絕損壞 ZIP/BMP、替換素材後新印章可用，保留未存文字／高度／材質及 undo/redo，重試不寫地圖；實際釋放 GL 資源後重建 context，高度與通行覆蓋 framebuffer 完全相同。未模擬實體 GPU 驅動故障 | `AgainstRomeModifier.Tests/MapEditorResourceReloadTests.cs`（本機強制 ARM_OPENGL_REQUIRED=1）、`TerrainBlendModuleTests.cs` resolver 重綁 |
| 原版材質 | 四角烘焙、不支援接縫拒絕、滑鼠筆畫整筆回滾、undo/redo | `AgainstRomeMapEditor.Modules.Tests/TerrainBlendModuleTests.cs`、`AgainstRomeModifier.Tests/MapEditor3DTests.cs` |
| 圖塊印章連續繪製 | 漏格補點、分筆／換筆刷、undo/redo、save/reopen；2D/3D 真正滑鼠處理及 picker 折返回報（非遊戲內外觀證據） | `AgainstRomeModifier.Tests/MapEditorStampTests.cs` |
| 圖塊印章地區篩選 | 依目前地圖 L 系列、無證據 fallback、混合系列、搜尋與手動顯示其他系列 | `AgainstRomeModifier.Tests/MapEditorStampTests.cs` |
| AI 套用 | 材質區域拒絕保留先前接受區域；模式切換；材質獨立 undo，高度/通行共用 undo；儲存及重新開图 | `AgainstRomeModifier.Tests/MapEditorAiWorkflowIntegrationTests.cs` |
| AI 生成 UI | 預設 Laguna；先生成/檢視再套用；取消、部分失敗、重試、舊方案失效；一次套用 | `AgainstRomeModifier.Tests/AiMapPlanningDialogTests.cs` |
| AI 視覺預覽與角色進度 | 獨立快照、不改bytes/dirty/history；材質拒絕區域與變更量和正式套用一致；圖片失效/釋放、預覽失敗拒套用/重試；角色起訖/部分失敗順序，取消後晚到進度不覆蓋UI | `AiMapPlanningDialogTests.cs`、`MapEditorAiWorkflowIntegrationTests.cs`、`MultiAiMapPlannerTests.cs` |
| AI 真實完整流程 | 真實 Ollama `laguna-xs-2.1:latest` 三角色依序生成→預覽（不改 bytes/dirty）→一次套用（統計與預覽相同）→材質與高度/通行分別 undo/redo→儲存→新表單重開一致；中英 880×740/640×580 截圖 | `AgainstRomeModifier.Tests/MapEditorAiLiveAcceptanceTests.cs`（須 `ARM_AI_ACCEPTANCE_LIVE=1`，否則直接返回）；2026-10-07 證據 `TEMP/ArmClaudeQA/ai-live-1` |
| AI 序列推論 | 三角色依序執行；跨 planner 共用單一推論鎖；取消排隊不送出請求 | `AgainstRomeModifier.Tests/MultiAiMapPlannerTests.cs`、`OllamaSingleInferenceTests.cs` |
| 放置物件 | 編輯/複製身份、快照隔離、批次原子性；多選 undo/redo、越界拒絕 | `AgainstRomeMapEditor.Modules.Tests/PlacementBatchRegressionTests.cs`、`AgainstRomeModifier.Tests/PlacementBatchUiTests.cs` |
| 單兵與自然物件 | 儲存失敗後重試、保留部隊目標身份；自然物件只寫入一次、更新標記及再次刪除 | `AgainstRomeModifier.Tests/MapEditorSavePlacementRegressionTests.cs` |
| 部族／隊伍共同儲存 | Ger/Hun/Kel/Rom 各含建築0–8、部隊0–7、外交/生成/區域目標；缺BCI完整回滾/retry、fresh form、DATA slot/uid、移動/改隊伍/人數、刪目標拒存與undo；非法隊伍Add/Save兩層拒絕 | `AgainstRomeModifier.Tests/MapEditorTribeTeamIntegrationTests.cs`（合成別名與注入建築範本） |
| 缺完工建築範本 | 工地生成保留目標ID、不留DATA配對；失敗回滾/retry；中英提示、重存/重開保留、範本恢復後改回DATA並清提示、刪除 | `AgainstRomeModifier.Tests/MapEditorBuildingFallbackTests.cs`（合成範本） |
| 儲存交易 | 前置驗證拒絕不寫檔；缺失/損壞 BCI 後回滾 bytes、快取及新增檔案；保留 dirty 可重試 | `AgainstRomeModifier.Tests/MapEditorSaveTransactionTests.cs` |
| 事件與目標 | 事件 session、永久目標身份、矩形包含邊界、勝敗 terminal guard、JSON v6、對話框編輯 | `AgainstRomeMapEditor.Modules.Tests/ScenarioEventSessionTests.cs`、`AgainstRomeModifier.Tests/ScenarioEventsTests.cs`、`ScenarioEventDialogTests.cs` |
| 啟動器路徑關閉 3D 編輯器 | using＋ShowDialog→回到地圖選單後 Dispose 不拋例外（修正前重現 InvalidOperationException）；真實行程直接開圖、關閉結束碼 0、無 crash_log | `AgainstRomeModifier.Tests/MapEditorOpenGlTests.cs`、手動煙霧（`MapEditorFixtureExport.cs` 匯出合成 root） |
| 3D 無法使用時退回 2D | 缺 floortex.dat：3D 停用、診斷報告列出缺少項目、2D 可編輯並儲存／重開 | `AgainstRomeModifier.Tests/MapEditor3DFallbackTests.cs` |
| 地形工具擴充 | 9×9／15×15 筆刷、粗糙化（決定性、可復原、15×15 表單筆畫儲存重開）；L 系列地區材質（61 種，真實素材庫推斷過渡）；自動過渡中介材質（單元測試：缺直接過渡時插入中介、兩圈鏈、無中介仍拒絕；真實地圖 5 張×5 材質×200 點，產生 tile 均在素材庫、復原逐字相同） | `AgainstRomeMapEditor.Modules.Tests/TerrainRoughenTests.cs`、`TerrainAutoBridgeTests.cs`、`AgainstRomeModifier.Tests/MapEditorTerrainToolsTests.cs`、`MapEditorAllMaterialsTests.cs`（ARM_GAME_PATH） |
| 水域工具 | 挖到水面下固定深度、筆刷外不變、可復原、水面過低時提示且不改地形 | `AgainstRomeModifier.Tests/MapEditorTerrainToolsTests.cs` |
| 自然物件散佈 | 筆刷範圍內依密度散佈、最小間距、反覆塗抹只補空隙、混合同類物種、單一復原；1×1 維持每格一株 | `AgainstRomeMapEditor.Modules.Tests/NatureScatterTests.cs`、`AgainstRomeModifier.Tests/MapEditorNatureHistoryTests.cs` |
| 圖塊印章 | 原版道路／河流／岩壁／地板等圖塊分類列出，單格精確放置、復原重做、存檔重開、印章取樣 | `AgainstRomeMapEditor.Modules.Tests/TerrainAutoBridgeTests.cs`（TerrainStampTests）、`AgainstRomeModifier.Tests/MapEditorStampTests.cs` |
| 3D 資料計算 | 高度插值、網格、相機限制、射線選取、材質 atlas | `AgainstRomeModifier.Tests/MapEditor3DTests.cs` |
| 原生 ALR 8-bit 行解碼核心（未接 UI） | 兩段像素／gap／各段 opacity、快照隔離、損壞輸入拒絕；實際 alr.dat 全部 2,075 個文件與 palette 變體解碼成功。未驗證遊戲場景外觀 | `AgainstRomeMapEditor.Modules.Tests/NativeAlrIndexedFrameTests.cs`、`tools/re/alr-probe` |
| ALRA v4–6 indexed 容器解析（未接 UI） | palette 選取、共用影格、截斷拒絕；實際素材修正 pixel-relative offsets 與零尺寸格，358,083 格解碼成功。動畫方向／場景仍未驗證 | `AgainstRomeMapEditor.Modules.Tests/NativeAlrDocumentTests.cs`、`docs/reverse-engineering/native-scene-rendering.md` |
| APAT v2/v3 indexed diamond patches（未接 UI） | raw／compressed rows、row gap／skip／opacity、透明合成、變長群組列表與輸入拒絕；222文件／500507 tiles／103601 frames 全庫解碼成功，已檢視主屋兩格。色彩／方向／動畫／完整場景尚未遊戲驗收 | `AgainstRomeMapEditor.Modules.Tests/NativeAptDocumentTests.cs`、`tools/re/apt-probe` |

完整 solution 同時涵蓋 Modifier 與 SaveManager，總通過數不能當成地圖編輯器的功能數。部分測試在未提供原版資料或未啟用 live 環境時直接返回；通過總數也不能證明這些實機路徑已執行。

## 尚待驗收

| 範圍 | 必須取得的證據 | 目前狀態 |
| --- | --- | --- |
| UI 視覺 | 主畫面及 AI/事件對話框在常用視窗尺寸、DPI 下沒有遮擋，模式切換提示一致 | 96 DPI 已檢查主畫面 1440×900/1100×700、最小尺寸各分頁、AI 880×740/640×580、中英事件 560×420；修正放置提示與刪除按鈕截字。高 DPI：除 AI 對話框外的編輯器視窗改用 `AutoScaleMode.Dpi`，`MapEditorHighDpiTests.cs` 以 100/150/200% 模擬（字型放大＋真正 PerformAutoScale、最小尺寸）中英無截字/越界，並修正放置對話框最後一列錯位、場景摘要/自然物件說明固定高度截斷、地圖選擇最小寬度與語言按鈕定位。實體高 DPI 螢幕與跨螢幕 DPI 切換尚未驗證；AI 對話框依使用者指示暫不處理 |
| OpenGL 顯示 | 真正渲染高度、通行覆蓋與材質更新，選取位置符合畫面 | `MapEditorOpenGlTests.cs` 以 `Map3DViewControl.CaptureFrame`（與畫面同一繪製路徑的離屏 FBO）在本機 NVIDIA RTX 4080／OpenGL 3.3 讀回像素：地形有繪製、高度筆畫改變畫面且 undo 後殘差 0、材質與水面更新反映在亮度／藍色分量、handle 重建後重新初始化畫面相同、關閉時不再例外。通行覆蓋在 3D 未繪製（僅 2D）、滑鼠選取位置與畫面一致、效能與無 GPU 環境 fallback 仍未以真實畫面驗證 |
| 遊戲載入與存讀檔 | 匯出地圖能載入；儲存/讀檔後事件與永久目標仍正確 | 事件版 v6 地圖可載入；遊戲內存檔（SAVE/ESAVE_000）→讀檔後計時失敗事件仍執行。讀檔後放置建築／住房數是否保留未逐項核對 |
| 完整還原 | 真正原版還原成功、保留/刪除選項及新對話框視覺 | 已有合成callback流程、runner回滾與未知EXE拒絕證據。新對話框已納入 100/150/200% 中英截圖檢查：150% 原本截斷說明，已改 Dpi 縮放與自動加高。完整原版成功還原尚待驗收 |
| 區域事件、勝敗 | 邊界進出、部隊死亡、重複條件、同 tick 多事件、結算畫面 | 2026-10-07 遊戲內（使用者授權，ENDL_005、日耳曼、無盡模式）：3 秒計時訊息顯示；10 名 GER_INF01 未進入矩形時無勝利，移入後出現勝利統計畫面（遊戲時間 2:44）；45 秒失敗事件在存檔→讀檔後仍觸發失敗統計畫面（遊戲時間 0:58，讀檔後計時語意未確定）。案例以 `InGameAcceptanceScenarioTests.cs` 經真正 MapEditorForm 儲存。證據 `%TEMP%\ArmInGameEvidence_20261007`。部隊死亡、重複條件、同 tick 多事件未實測 |
| 部族、多人 | 真實部族範本、放置、腳本、勝敗及多人同步 | 四部族/隊伍合成共同儲存矩陣已通過；不證明真實部族範本可用、腳本實機執行或多人同步 |
| AI 地圖可玩性 | 通路、水域、起始位置、資源與建築空間在遊戲內可用 | 生成/套用通過不等於可玩性驗收 |

本輪不得存取或修改遊戲安裝目錄。開發順序以 `map-editor-roadmap.md` 為準；`map-editor-requirements-audit.md` 列出原規格的實際缺口。核心專屬核對後，四部族／隊伍／事件共同儲存矩陣已補；缺建築範本提示已補；接著完成AI預覽與生成進度。平坦範本仍保留聚落、腳本與連結物件，不代表真正空白地圖；合成小數案例不證明遊戲接受所有小數參數。

96 DPI 視覺證據位於 `TEMP/ArmVisualQA_1c01f5e6b6e84fe490ec6339c257de49`：獨立 STA harness 使用合成地圖、2D 畫布及 stub AI，透過 `DrawToBitmap` 產生 PNG 後檢視；中英放置分頁最小尺寸皆完整顯示提示與按鈕。這不驗證實際推論、OpenGL 或高 DPI。

## 重現本輪完整驗證

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore
dotnet test AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore --no-build --logger 'console;verbosity=quiet'
```

最新工作樹結果：build 0 警告/0 錯誤；宿主 589 通過/21 略過，modules 57 通過，共 646 通過/21 略過/0 失敗（未設 live 變數時 AI live 測試直接返回，不算實機證據）。真實生成→預覽→套用→復原/重做→儲存→重開已另以 live 變數執行通過。未設定 `ARM_GAME_PATH`、`ARM_OLLAMA_LIVE`。以上 `--no-restore` 命令須先有相同 artifacts 路徑的 restore；新環境先移除 build 的 `--no-restore`。

`b9b9965` 的獨立 git archive 快照位於 `TEMP/ArmCoreCommitQA_e70f5f1a2802428abb9fd7546428d1a2/source`，不包含 AI 預覽 WIP；独立還原／建置／完整測試同樣為 0 警告/0 錯誤、536 通過/21 略過/0 失敗。
