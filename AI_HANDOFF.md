# AI Handoff - Live Project Memory

## 最新指示與狀態（2026-10-07 Antigravity；遊戲修改器 CLI 模式完成）

- 使用者要求「製作遊戲修改器CLI模式，讓AI代理人可以操作」。
- Antigravity（2026-10-07）：完成遊戲修改器 CLI 模式，提供 AI 代理人專用命令列介面與結構化 JSON 輸出（`--json`），涵蓋偵測（detect-game/status）、功能清單（features）、套用修改（apply，支援個別開關/全開/全關/倍率/兵種預設/自訂設定檔/dry-run）、還原（restore）、備份（backup）、兵種屬性（stats list/get/export）、設定檔架構（profile schema/export）、存檔管理（saves）與地圖清單（maps）。
- 既有 GUI 與 `--game/--map` 直達行為保持完全相容；無介面模式不觸發 MessageBox 或 UI 表單，Windows Console 下自動附著父行程終端並輸出 UTF-8。
- 驗證：新增 `CliTests.cs` 14 項測試全數通過；完整方案測試 645 通過/21 略過/0 失敗。本輪僅 repository/TEMP 操作，未存取真實遊戲安裝目錄。

## 歷史記錄與既有狀態（2026-10-07 Codex & Claude）

## Git 與保留工作

- 分支 `主要開發`，恢復起點 HEAD `7ed3157`；原先已有 staged 事件模組/solution/project/test，以及未提交的 host adapters/MultiAiMapPlanner。
- Codex 新本地 commits：`952b71e` 地形純state/resolver重構（height/history與HEAD逐行比較完全相同）；`bfdf813` 放置module/批次/驗證/history與回歸測試。使用明確路徑 `git commit --only`，原有事件等staged變更保留，不順便提交未review的host/AI/事件功能。未push。
- 已保留原有改動，沒有 reset/stash/清除。所有程式碼與測試已整合commit；本輪另提交README、指南、模組邊界與交接文件。原Git交接83個section已全部核對保存至歷史檔（補入2個缺失的舊版本段落），不刪其他代理紀錄。
- 續作commits：`c22dbbf`事件/nature純sessions；`1c4a28e`純宿主方法分檔；`22dc376`區域/勝敗與v6；`412abe6`序列三角色AI檢視/套用；`63e0772`放置/nature/交易整合、單兵與標記修正。純重構與功能分開，沒有push。
- 本輪已恢復整合，依既有授權審查後建立本地commits；所有本輪原有程式碼/測試已審查並整合，push等仍需另行授權。

## 已完成與目前可用邊界

- `src.MapEditor.Modules`：事件 ScenarioEventSession、NatureEditSession、PlacementEditSession、TerrainHeightEditSession/TerrainEditHistory、ScenarioSavePreflight；host 分成 Ai/Nature/Terrain/Placement/Scene/Persistence adapters。
- Nature：待存新增/移除與 stroke undo/redo 移入 session；快照隔離；原索引恢復新增物件順序，同值物件仍獨立；空白地形同時清除 pending additions/可移除地景，保留 linked/building，刷新 markers。
- Placement：Load/Add/Delete/Save baseline 接 session；新增編輯與複製 UI、命令與 undo/redo，保留編輯身份，複製產生新 GUID，TemplateFields 深拷貝。主表單 PlaceObject 模式接撤銷/復原。X/Z UI 上限收斂至16383。
- Save：寫檔前檢查事件、目標、座標與人數（1–20）；預建物件依 injector 原語意不要求腳本別名/尚未生成的 DATA binding。`TrySaveMap` 將交易與錯誤視窗分離；成功才更新基準，失败保留 dirty/history。
- AI UI：三角色分別選模型，生成→檢視→明確套用；取消、部分失敗診斷、重試、描述/模型變更使舊方案失效、晚到模型列表不覆蓋狀態。只生成不寫地圖，套用後仍需儲存。
- Events：ObjectInArea 為包含邊界、連續成立的 X/Z 矩形條件，使用部隊容器；Victory/Defeat 設 GLOBAL_MISSION_RESULT=1/0 後 s_quitGame，須為非重複事件的最後動作；terminal guard 防止同 tick 後續事件覆蓋。JSON 版本6。
- `docs/reverse-engineering/scenario-area-mission-result.md` 記 repo EXE 靜態反組譯證據；VM/compiler/UI 測試通過不等於遊戲結算已實測。
- README/user guide 已修正為 Laguna 序列推論、材質獨立而高度/通行共用撤銷歷史、建築空間提示非實機保證、自包含版本執行需求；本輪不沿用舊實機結果宣稱新版本可玩。

## 最新驗證與失敗紀錄

- Claude（2026-10-07 15:03）：`InGameAcceptanceScenarioTests` 新增 terrain 案例（出生點北方示範區），已寫入遊戲目錄 ENDL_005：材質 3/3（皆選到第一個可銜接的 BB）、粗糙化 713 頂點、茂密混合森林 22 株（20 種、含義大利柏樹等跨地區樹種）、印章道路 weg1 11 格。啟動遊戲後畫面全黑：前景被 Windows TextInputHost（輸入法面板）佔住，computer-use 無法點擊，SetForegroundWindow 失敗，重啟遊戲仍同；需使用者關閉輸入法面板或手動切到遊戲。ENDL_005 目前含示範區，備份仍在 `%TEMP%\ArmGameBackup_20261007_140419`，驗收後要還原。
- Claude（2026-10-07）：圖塊印章模式。調色盤上方「圖塊印章」勾選後列出素材庫全部原版圖塊並分類（道路與廣場／河流／岩壁／地板／其他／地區 L／過渡圖塊），一次放一格精確圖塊（`TerrainBlendEditSession.StampTexture`，不改角點、可復原重做，之後相鄰塗材質會依角點重烘並覆蓋）；印章模式右鍵取樣選取游標下圖塊。模組 1 項＋表單（合成 weg1：分類、蓋印、存檔、重開、取樣）通過；full 宿主574/略過21、modules57，共631通過/0失敗。注意：印章圖塊與周圍材質不做過渡檢查，是否形成原版不會出現的硬邊由使用者負責；遊戲內未驗證。
- Claude（2026-10-07）：自然物件散佈。新增 Modules `NatureScatter`（稀疏/普通/茂密機率 .15/.35/.7、最小間距 .9/.65/.45 格、筆刷範圍內逐格取點）；UI 密度選單與「混合同類物種」；1×1 筆刷維持每格一株且不做間距檢查（初版加間距導致既有歷史測試 5 株變少而失敗，已修）。原本種植完全忽略筆刷大小。單元 5 項＋表單 9×9 茂密混合 1 項通過；full 宿主573/略過21、modules56，共629通過/0失敗（含 DPI 版面檢查）。遊戲內未驗證大量散佈的 DATA 槽位上限與效能。
- Claude（2026-10-07）：新目標「先針對地圖編輯器本身開發，創造更豐富地圖」。完成：(1) 筆刷新增 9×9／15×15（BrushSizes 陣列）；(2) 高度「粗糙化」Roughen（固定種子平滑值雜訊，每筆畫新種子，可復原）；(3) 使用者要求「測試所有不同素材」：以真實 floortex.dat（唯讀、複製至 TEMP）測得原 4B 材質 28 種中 756 組配對只有 44 組有原版過渡，ENDL_000 隨機點塗佈成功率 1×1 約 5–18%、5×5 約 0–4%；原版地圖統計顯示幾乎無硬邊（全部 75 張共 11 處），但大部分地表是 L 系列（L2/L3/L5/L06…約 18 萬格）與道路／河流／岩壁等編輯器原本不認得的圖塊。(4) 新增 L 系列地區材質：T5-only 編號為純材質（ID 如 `L2:01`，61 種），T1–T9 編號以四角顏色推斷兩材質配對（同組優先，平均色差門檻 32），ENDL_001–004 四角覆蓋率由約 0% 升至 84–99.9%。(5) 自動過渡（autoBridge，UI 勾選預設開，AI 套用不變）：失敗時於外圈角點插入中介材質最多 2 圈，結果仍全為原版 tile；隨機點成功率大幅提升（例 ENDL_001 L06:28 5×5 1→114/200），所有成功筆畫 tile 皆存在素材庫且復原後逐字相同。部分材質（BJ、L06:06、L3:10、L2:06）仍幾乎無法塗，待查。報表測試在 `MapEditorAllMaterialsTests.cs`（需 ARM_GAME_PATH）。full 宿主572/略過21、modules51，共623通過/0失敗。遊戲內未驗證 L 材質與自動過渡輸出。下一步：自然物件散佈密度、裝飾圖塊（道路/河流/岩壁）印章模式、死路材質。
- Claude（2026-10-07 14:25–14:40）：遊戲內驗收（使用者授權並自行建立 `.claude/settings.local.json` 允許 `Bash(DOTNET_ROLL_FORWARD=Major ARM_GAME_PATH=*)`）。以 `InGameAcceptanceScenarioTests` 寫入 ENDL_005（v6、2 事件、ak_level.bci 重生）。遊戲操作：縮小後還原會使全螢幕畫面錯亂，需重新啟動；主選單「ОТКРЫТЬ-ЗАКРЫТЬ」(503,398) 才是無盡模式。結果：計時訊息 OK；先誤選平民隊（Гражданский отряд）移動未觸發，改選 10 名劍士經小地圖移入區域後勝利統計畫面（2:44）；defeat 案例存檔到 SAVE/ESAVE_000 後讀檔，失敗統計畫面（0:58，非 45 秒，讀檔後計時語意未確定）。證據 `%TEMP%\ArmInGameEvidence_20261007`（兩張統計截圖、測試時 scenario/bci、存檔）。還原：ENDL_005 兩個變更檔以備份覆蓋，雜湊與備份一致；`SAVE\ESAVE_000` 測試存檔因 Remove-Item 路徑保護無法刪除仍留在遊戲目錄（原 SAVE 為空），需使用者自行刪除或保留。未驗證：部隊死亡條件、重複事件、多事件同 tick、讀檔後建築/住房保留、其他部族、多人。
- Claude（2026-10-07 14:04）：使用者回覆「授權存取遊戲」。已唯讀確認遊戲目錄 `C:\Program Files (x86)\Against Rome`（登錄檔無 Path，使用預設路徑；MAPS 有 ENDL_000–005，ENDL_005＝先前測試圖「Claude Test 1b」，arm_scenario.json 仍 v2、10×GER_INF01 team0＋3 棟建築）；shell 為管理員。已備份 ENDL_005、MAPS 根 manifest、SAVE 至 `%TEMP%\ArmGameBackup_20261007_140419`。新增未提交的 `tests/AgainstRomeModifier.Tests/InGameAcceptanceScenarioTests.cs`（ARM_GAME_PATH＋ARM_INGAME_MAP＋ARM_INGAME_SCENARIO=victory|defeat 時以真正 MapEditorForm/TrySaveMap 寫入案例）。執行寫入遊戲目錄的命令被 Claude Code auto mode 權限分類器拒絕（Irreversible Local Destruction），連帶不得以其他方式重試；遊戲目錄未被修改、遊戲未啟動。等待使用者決定：自行執行命令、或在設定加入允許規則。
- Claude（2026-10-07）：真實行程煙霧測試。新增 `Export_synthetic_game_root_for_manual_smoke_runs`（設 ARM_FIXTURE_EXPORT 才匯出合成 root）。`dotnet AgainstRomeModifier.dll --game TEMP/ArmClaudeQA/smoke-root --map ENDL_005`：編輯器視窗開啟、無錯誤對話框、WM_CLOSE 後結束碼 0、無 crash_log。修正前 build（b940330）同流程也正常：Application.Run 主視窗結束時不 Dispose。但 LauncherForm 的 using+ShowDialog 路徑會 Dispose：新增 `Launcher_style_modal_editor_with_3d_closes_and_disposes_without_errors`，換回修正前兩個檔案時重現 InvalidOperationException（使用者會看到「無法啟動地圖編輯器」），修正後通過。full 宿主564/略過21、modules45，共609通過/0失敗。啟動器實際點擊流程未自動化（需遊戲路徑偵測）。
- Claude（2026-10-07）：完整還原對話框納入 DPI/截圖檢查。150% 說明文字被 TableLayoutPanel 壓在儲存格內截斷（原檢查未抓到：AutoSize 控制項被壓縮時不越界也不重疊）。檢查新增 GetPreferredSize 比對（Label 以實際寬度求高、按鈕比兩向）與 TLP 重疊；以 HEAD 舊版對話框確認能抓到後換回修正。修正：RestoreAllOptionsDialog 改 AutoScaleMode.Dpi＋AutoSize GrowOnly；放置物件對話框按鈕列 AutoSize（固定 44 高壓縮按鈕）。full 宿主562/略過21、modules45，共607通過/0失敗。
- Claude（2026-10-07）：GL 效能數據加入 opengl 測試（寬鬆門檻：重繪<50ms、筆刷p95<100ms）：RTX 4080 重繪 0.27ms、筆刷中位 1.9ms／p95 11.9ms（1734 事件）。證據 TEMP/ArmClaudeQA/opengl-perf/opengl.json。
- Claude（2026-10-07）：GL 選取一致性 `Real_opengl_pick_returns_the_tile_drawn_under_the_pointer`：RTX 4080、1084x751，5 格（含起伏地形）畫面像素中心經 TryGetTile 選回同一格，5/5。證據 TEMP/ArmClaudeQA/opengl-pick/pick.txt。
- Claude（2026-10-07）：3D fallback。`MapEditor3DFallbackTests.cs`：缺 floortex.dat 時 3D 按鈕停用、診斷按鈕可見且報告列出缺少 floortex.dat、2D 可見；要求 3D 仍維持 2D；2D 高度編輯→儲存→重開一致。通過；full 宿主561/略過21、modules45，共606通過/0失敗。已知限制（未修）：Disable3DView 永久停用，同一表單內資源後來補齊不會恢復 3D（每次開圖為新表單，影響小）。
- Claude（2026-10-07）：OpenGL 實畫面。新增 `Map3DViewControl.CaptureFrame`（離屏 FBO，共用抽出的 RenderScene）與 `MapEditorOpenGlTests.cs`（ARM_OPENGL_REQUIRED=1 強制；無 GL 時只檢查失敗原因）。初跑卡 2 分鐘：測試 STA 非主執行緒 → GLFW 主執行緒檢查例外被 ThreadExceptionDialog 擋住；測試改 ThrowException 模式並關 `GLFWProvider.CheckForMainThread`（僅測試）。之後發現產品 bug：關閉表單時父視窗先銷毀 GL handle/context，Dispose 再 MakeCurrent 觸發重新建立與 OnLoad 初始化失敗，InitializationFailed 對已銷毀表單 BeginInvoke 拋例外。修正：OnHandleDestroyed 在 context 消失前釋放 GL 資源；handle 重建時重新初始化；表單 handler 檢查 IsDisposed/IsHandleCreated。RTX 4080 實測：高度改變3744像素、undo殘差0、材質亮度55.1→58.2、水面藍色5.9→50.8、重建殘差0、Close 無例外。證據 TEMP/ArmClaudeQA/opengl-9。正式 exe 關閉流程未實際執行。
- Claude（2026-10-07）：入口驗收。`--game/--map` 直達編輯器原可開啟劇情戰役 KAMP_（選單排除），新增 `Program.ResolveDirectMap` 與選單同範圍；參數錯誤顯示用法、不寫 crash_log。`MapEditorEntryTests.cs` 通過；full 宿主559/略過21、modules45，共604通過/0失敗。未實際啟動 exe。
- 使用者指示（2026-10-07）：「AI 地圖生成先跳過」。AI 製圖功能（含 AiMapPlanningDialog 的 DPI）暫停，不再擴充；轉做其他編輯器缺口。
- Claude（2026-10-07）：高 DPI。新增 `MapEditorHighDpiTests.cs`（100/150/200% 模擬：所有字型乘倍率、Dpi 視窗宣告 96/倍率 設計 DPI 走真正 PerformAutoScale、縮到 MinimumSize，中英全部模式與五個對話框；輸出 PNG 與 layout.txt 到 ARM_DPI_OUTPUT/TEMP）。修正前 150% 140 項截字/越界（場景物件按鈕被切、清單擠壓），套用 AutoScaleMode.Dpi 後剩真實問題並逐一修正：放置物件對話框無 RowStyles 使最後一列「人數」標籤錯位（100% 亦存在）、`_sceneSummary`/`_natureHint` 固定高度截第三行（新增 FitWrappedLabelHeight）、地圖選擇最小寬 840 放不下六按鈕（改 900）、語言按鈕固定像素定位（改依按鈕高度換算，96 DPI 位置不變）、路徑列 AutoSize。原檢查對 AutoSize 控制項與按鈕單行寬度有誤判已移除。DrawToBitmap 對重疊的語言按鈕 z-order 畫錯（截圖看不到），以 layout.txt 數值確認位置正確。最終三倍率通過；full --no-build 宿主558/略過21、modules45，共603通過/0失敗。實體高 DPI 螢幕未驗證（本機 96 DPI，不改系統設定）。
- Claude（2026-10-07 13:20 起）：接手 Codex 留下的未提交 `MapEditorAiLiveAcceptanceTests.cs` 與 `RunInSta` timeout 參數。初次 build 有 CS8604（`layers.Collision.ToArray()`）與 CA1869，已修。設 `ARM_AI_ACCEPTANCE_LIVE=1` 實跑真實 Ollama laguna 三角色（約 17 秒，Terrain4/Water1/Materials1，水系河流缺 toLocation 被正規化移除）：預覽不改 bytes/dirty、套用一次且統計與預覽相同（高度14333/通行448/材質1）、undo/redo、儲存、新表單重開一致，通過。證據 `TEMP/ArmClaudeQA/ai-live-1`（result/progress/中英兩尺寸截圖，96 DPI）。完整 `--no-build` 測試宿主555通過/21略過、modules45，共600通過/0失敗。下一步：高 DPI（編輯器所有 Form 未設 AutoScaleMode，字型為點數而列高/欄寬為固定像素，PerMonitorV2 下 >96 DPI 可能截字）。

- Codex（2026-10-07）：開始第4階段；AI預覽WIP接上正式host/dialog，獨立height/collision/material Fork走同Applier，加入橙色材質變更與圖例/變更統計/拒絕提示，圖片於失效/重試/關閉釋放，預覽失敗不能套用。Release build 0警告/0錯誤；UI/host隔離與實際套用一致定向19通過。角色進度UI/planner接入，35項定向通過；新增progress序列1通過，redo/pending初測2失敗因Redo本身CommitStroke會清redo，已分開合法redo與pending操作案例，再跑3通過。加入紫點拒絕區域、橙點材質變更保留底層水域/高度。只用repository/TEMP，不宣稱live/可玩性。角色生成進度已整合；新增紫點與正式套用一致定向1通過，full --no-build宿主554通過/21略過、modules45通過，共599通過/21略過/0失敗。新UI視覺/真實完整流程仍待驗證。

- Codex（2026-10-07）：缺建築範本STA中英基線2失敗，已證實fallback/事件ID/無DATA binding/rollback/retry正常，但提示仍聲稱完工且未列實際腳本工地。測試初版CS0117使用不存在Language.Chinese已修TraditionalChinese/OverrideLanguageForTesting；IDE0005已清。已補放置提示/狀態列/成功訊息的工地類型名單（最多5種加餘數），重新開圖讀回、無變更重存保留、移除清除。定向中英fallback與四部族6通過；範本恢復後改存DATA/清提示/刪除案例已通過2項；補using錯放CS1529後重跑通過。IDE0005已清。Release build 0警告/0錯誤，full --no-build宿主549通過/21略過、modules45通過，共594通過/21略過/0失敗；最後提示移除DATA術語後build與定向2通過。僅TEMP，長提示視覺/實機未驗證，AI預覽WIP保留不提交。

- Codex（2026-10-07）：第3階段新增真正STA四部族Ger/Hun/Kel/Rom矩陣，各含建築team0–8、部隊team0–7與外交/生成/區域目標事件。初跑4通過，包含DATA/scenario先寫後BCI缺失的完整bytes/新檔回滾、retry、DATA slot/uid/team/座標/角度、fresh form/持久ID、刪除目標拒存/undo及重存bytes不變。5個xUnit2031警告已改predicate overload；四個非法隊伍案例初跑失敗是Add已有早期guard，不是產品缺陷；改先斷言Add拒絕且無mutation，再注入損壞pending快照驗證Save獨立guard，鎖briefing拒存已通過；再補fresh form移動中立建築/改team、部隊20改1，DATA重新綁定/無多餘物件/目標身份/重開與重存bytes一致。最終定向8通過；Release build 0警告/0錯誤，full --no-build宿主547通過/21略過、modules45通過，共592通過/21略過/0失敗。僅TEMP、不宣稱遊戲或多人同步已驗證。

- Codex（2026-10-07）：額外ENDL槽位核對新增EndlessAiAdditionalMapTests合成TEMP，初跑3項1通過/2失敗。真正P1七槽位（000–004/005/999）detect/apply/buffer/save/fresh reload/restore已通過；兩ResolvePaths測試證實script納非數字ENDL_ABC、SDL遞迴含.tmp_arm/.deleting_arm/巢狀複本。已限制正式數字槽位与root SDL，額外槽位/還原定向17通過。平坦範本/地形重設中英名稱與指南改為實際保留聚落/腳本的語意；地形/nature/額外槽位定向42通過，Release build 0警告/0錯誤；full --no-build宿主539通過/21略過、modules45通過，共584通過/21略過/0失敗。AI預覽WIP保留不提交。
- Codex（2026-10-07）：FileRollbackScope新增目錄快照/恢復（空目錄/未知檔，拒絕linked snapshot，復原失敗保留TEMP備份）。CustomMapRestoreService包住完整還原，預設保留；刪除先manifest/tmp前檢再走Deleter，外層交易保留目錄/manifest。新選項dialog預設勾選保留、取消不執行。15項合成TEMP定向通過：保留/005/999刪除、restore/後續/中途delete回滾、snapshot鎖檔拒絕、未登記拒絕、真正runner/engine拒絕與STA選項。檢查發現原restore仍解析custom腳本；僅完整還原啟用orchestrator排除custom與legacy custom team備份。鎖custom腳本證明不讀，而一般AI仍讀；實際engine合成EXE/12個回血腳本在兩選項成功。該fixture初次2失敗是缺法術祭壇原始bytes，補齊後2失敗是缺12個系統回血腳本；完善fixture後2通過，未放寬產品驗證。最終Release build 0警告/0錯誤，full --no-build宿主536通過/21略過、modules45通過，共581通過/21略過/0失敗。真實原版還原/新dialog視覺尚未驗證；AI預覽WIP保留不提交，下一步額外槽位AI證據/空白語意。
- Codex（2026-10-07）：備份規格§1.5核對。CustomMapBackupTests合成TEMP初跑4例3通過/1失敗：loader只檢查team.dat直接parent，custom/Extra/team.dat仍讀取；autohealer catch吞失敗，補logger斷言再跑2通過/2失敗，確認兩路徑都漏。新增CustomMapManifest.IsCustomMapFile以MAPS第一層擁有者marker判定，兩loader接入，排除整個自製目錄且不擴及鄰近root/其他map。新增5案例與既有備份定向14通過/1略過，涵蓋custom005/999、鎖檔證明不讀、既有bak保留、不產新bak、原版team基準仍正確。Release build 0警告/0錯誤，full --no-build test宿主521通過/21略過、modules45通過，共566通過/21略過/0失敗。RestoreAll沒有規格要求的保留/刪除custom選項；下一步補這項功能（含與PatchOperationRunner回滾的整合），不能只補文件。AI預覽WIP保留不提交。
- Codex（2026-10-07）：文字核對新增MapTextEscapingTests合成純文字/PFIL案例；基線 `dotnet test ... --filter FullyQualifiedName~MapTextEscapingTests` 2通過/7失敗：single讀不到escaped quote、反斜線沒有escape與正確常值長度計數、NUL接受、CP1251錯誤過晚/缺使用者提示。修Put單值escaped parse/Unescape/Escape、100 escaped bytes與NUL/CR/LF guard；單值/簡報CP1251寫入前驗證。新增STA表單拒存bytes/dirty保留/retry與標題/8team/簡報save/reopen/重存bytes相同；ErrorProvider輸入立即提示CP1251、合法輸入清提示。清除多餘using後，documents+Phase1定向43通過、最終文字/UI定向13通過。Release build 0警告/0錯誤，full --no-build test宿主516通過/21略過、modules45通過，共561通過/21略過/0失敗。只用TEMP，AI預覽WIP保留不提交；下一步備份排除/還原保留与額外槽位AI修補證據。
- Codex（2026-10-07）：核心環境屬性核對新增真正STA表單三地區測試（en-US/de-DE/fr-FR）。基線 `dotnet test ... --filter FullyQualifiedName~Environment_values_load_save_and_reopen` 1通過/2失敗：12.5於法文讀成0、德文讀成125。ParseDecimal、Heightmapstep解析與七項環境數值寫入改InvariantCulture；定向重跑3通過，涵蓋七值/高度比例/雨滴、dirty、重開與再次儲存全部bytes相同、未知行保留。完整Release build 0警告/0錯誤，full --no-build test宿主503通過/21略過、modules45通過，共548通過/21略過/0失敗。只用合成TEMP，不證明遊戲支援小數；AI預覽WIP保留不提交，下一步文字特殊字元與備份流程。
- Codex（2026-10-07）：核對原廠readonly與stale selection：新增8項合成TEMP測試先6失敗（原廠000/004帶marker判custom；000/004與移除marker的005/999鎖briefing後Save先碰檔得到IOException）。新增CustomMapAccess統一005–999/MAPS/marker判定，兩catalog、SDL service與Save接入；Save在CommitStroke/任何地圖檔讀寫前驗證選取root。定向42通過；另一個合成root的marked map拒存、bytes/dirty保留測試1通過。marker恢復後retry成功。Release build 0警告/0錯誤，完整test宿主500通過/21略過、modules45通過，共545通過/21略過/0失敗；未存取安裝目錄。AI預覽WIP保留，未納入提交；下一步文字/環境/備份證據與空白語意。
- Codex（2026-10-07）：已提交核心修正 `b9b9965`；以git archive解出TEMP/ArmCoreCommitQA_e70f5f1a2802428abb9fd7546428d1a2/source，獨立restore約1.12分鐘後Release build 0警告/0錯誤，full --no-build test宿主491通過/21略過、modules45通過，共536通過/21略過/0失敗。未重啟還原程序；確認commit不依賴未提交AI預覽。驗收矩陣/roadmap更新，下一步按requirements-audit核對catalog與儲存前置防護。
- Codex（2026-10-07）：按roadmap核對原spec建立/儲存流程。EndlessMapCloner在Move後manifest Load/Save失敗遺留正式槽位，兩項合成測試先失敗再修：manifest寫入FileRollbackScope，catch只刪本次成功Move的目標，同槽位retry成功；新增既有final/tmp內容保留2項。EndlessMapDeleter缺manifest登記/原廠slot拒絕，slots 0/4/5/999四項先實際刪除而失敗（僅TEMP），已补三重防護，在rename前拒絕。既有原廠拒刪測試初次僅錯誤文字「只能刪除」不符，已保留該訊息。完整Release build 0警告/0錯誤、test宿主491通過/21略過、modules45通過，共536通過/21略過/0失敗。docs/map-editor-requirements-audit.md列原規格與缺口，下一步catalog/SaveMap前置驗證、文字/環境/備份及空白語意。AI預覽WIP已被build涵蓋但功能未驗證，不納入本次提交。
- Codex（2026-10-07）：先讀AGENTS/交接/Git，HEAD beca463、起始乾淨，無活躍代理。只讀Ollama api/tags確認laguna/nemotron/gemma/qwen目前可用，未送推論。澄清後剛新增TerrainBlendEditSession.Fork與AiMapPlanPreviewBuilder（独立高度/材質快照），使用者隨即要求先整理順序；停止程式實作，兩檔未接UI、未build/test、未提交，不能沿用528通過宣稱新檔已驗證。保留WIP不清除，roadmap/交接單獨本地提交。
- Codex（2026-10-07）：AI材質拒絕修正與驗收矩陣已commit `572097e`。TEMP/ArmVisualQA_1c01f5e6b6e84fe490ec6339c257de49 的獨立.NET STA harness以合成fixture、2D模式DrawToBitmap核對main 1440x900/1100x700各tab、AI 880x740/640x580、事件中英560x420，DeviceDpi=96。發現Placement固定54px提示與36px三欄按鈕截字；改提示隨寬度自動換行，按鈕兩欄+整列delete、自動高度，重繪確認中英文最小視窗完整可見。harness初版ScenarioEvent List指定array的CS0029已改collection expression並重跑成功。版面修正後Release build 0警告/0錯誤、完整測試宿主483通過/21略過、modules45通過，共528通過/21略過/0失敗。高DPI/OpenGL/遊戲內仍未驗證。
- Codex（2026-10-07）：AI材質各區域原共用pending stroke，後續拒絕會CancelStroke撤回先前成功區域但摘要仍計成功。PaintCircle新增可選rollbackStrokeOnFailure（預設維持滑鼠筆畫原行為）；AI設false，拒絕只還原當次區域，整份已接受材質仍一次undo。定向測試modules4項、STA host1項通過；合成fixture驗證拒絕後保留base材質、Texture undo/redo、Height/Collision共用undo、磁碟寫入前不變、Save/reload清dirty/history。XML註解初版4個CS1573已修；完整Release build 0警告/0錯誤，full --no-build test宿主483通過/21略過、modules45通過，共528通過/21略過/0失敗。指南修正歷史分組，新增docs/map-editor-acceptance.md列測試證據與未驗證項；未啟用live環境的提前return測試不算實機證據。
- Codex 續作（2026-10-07）：發現單兵UnitCount=1存成Count=0，會走s_createObj而非部隊容器；Persistence改依Figure分類保留至少1人。新增STA交易測試：單兵區域目標失敗後重試/持久ID/再存同bytes/重新開圖；nature新增在其他檔已寫後BCI缺失rollback，再試只寫一次並清dirty/history。針對MapEditorSaveTransactionTests共5通過。nature新測試初次失敗是合成template active=0，不是rollback問題，補合法active/type字段後通過；未改LevelObjectStore。
- 同一nature測試證實成功存檔後canvas標記仍為pending索引-200000而非DATA槽位-100000-slot；Persistence成功後RefreshSceneMarkers，標記更新、再存同bytes與再刪除皆通過。新增删除測試fixture需columns[10]=0xFFFF才是未linked物件，已修。最新工作樹build 0警告/0錯誤，full test宿主482通過/21略過、modules44通過，合計526通過/21略過/0失敗。
- 已審查/提交`c22dbbf`事件/nature純sessions與tests。為遵守refactor與feature分開，利用Roslyn擷取HEAD原方法至TEMP/ArmHostSplitQA_11296a2402534a5cb68362dcc3fd5730/source的六個partial adapters，沒有改方法內容/欄位初始化順序；IDE0005清理後build 0警告/0錯誤、modules38+宿主418通過/21略過。以獨立TEMP index提交，保留工作樹全部功能與原index；首次diff check攔下舊方法搬移的兩行trailing whitespace，僅修TEMP whitespace並確認token完全相同後再提交。
- 恢復開發：Placement 新增 DuplicateMany/RemoveMany，先驗證全部索引/複本後才執行單一 batch，UI 改接整批指令；越界複製以狀態列拒絕。Add/Replace/Edit/複製驗證有限 X/Y/Z/Angle 與 X/Z 0–16383。新增 batch/邊界/redo 保留測試，modules 首輪41通過。曾遇 array.Reverse()選到void，已改Enumerable.Reverse；ScenarioSpawn.Angle實際為int，移除無效的float preflight測試/變更。
- TerrainBlendAuthoringMap/TerrainBlendEditSession 已移至 Modules/Terrain，僅依賴 INativeTerrainMaterialResolver，FloorMaterialCatalog 留在宿主實作。保留命名空間與 internal API/既有行為。新測試多餘using警告已移除，最終工作樹 Release solution build 0警告/0錯誤；full --no-build test：宿主480通過/21略過、modules44通過，合計524通過/21略過/0失敗。PlacementBatchUiTests實際STA表單多選複製/刪除/Undo/Redo與越界拒絕2項通過，未讀安裝目錄。
- 已以 `git archive bfdf813` 解出 TEMP/ArmCommitQA_8f51a47ff5d24dd59c000a7d92aa4b93/source，執行 `dotnet test <snapshot>/AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --logger 'console;verbosity=quiet'`：獨立restore/build/test通過，modules21通過、宿主418通過/21略過，合計439通過/21略過/0失敗。證明兩筆commit不依賴未提交的host等變更；這個snapshot不包含工作樹新UI/AI/事件/transaction功能。
- 環境：`DOTNET_ROLL_FORWARD=Major`；Release / `UseAppHost=false`；隔離 `--artifacts-path "$env:TEMP/ArmResumeQA"`；未設 ARM_GAME_PATH / ARM_OLLAMA_LIVE。
- `dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore`：0警告/0錯誤。
- `dotnet test AgainstRomeModifier.slnx -c Release -p:UseAppHost=false --artifacts-path "$env:TEMP/ArmResumeQA" --no-restore --logger 'console;verbosity=quiet'`：宿主478通過/21略過；modules31通過；合計509通過/21略過/0失敗。最後新增 think=false HTTP 斷言與座標 UI 限制後，再次 build 與 --no-build 測試，仍為509通過/21略過/0失敗。
- 新增真实 STA transaction tests：無效目標在寫檔前拒絕；缺失/損壞 BCI 在其他檔案已寫入後 rollback，還原 bytes/cache/新增檔案並保留記憶體 dirty，補有效 BCI 後 retry 成功。皆為合成 TEMP fixtures。
- Laguna 最初三角色回傳空 content（num_predict2048）；加入 `think=false` 後，真实 Ollama 三角色依序成功：Terrain5 / Water2 / Materials1，共8特徵；取消false、無角色錯誤。證據 `TEMP/ArmResumeQA/ollama-laguna-serial-smoke.json`。只生成，未寫地圖/遊戲。
- 前次 gemma3:12b 的18特徵成功僅為歷史，使用者模型現在以 Laguna 為準。
- 中途 MSB3027/3021 是本輪 PowerShell smoke 持有 DLL 導致 copy lock；smoke 結束後 build/test 正常。不必終止使用者程式或重試原失敗。
- 已修早期 CS1579（陣列Reverse選到void）與 CS0841（map宣告位置）、prebuilt別名前置誤判、JSON v6舊斷言。
- 移除 AGY 測試用 FindWindow('#32770',null) 自動關閉視窗機制，改呼叫 TrySaveMap，避免關閉無關視窗；歷史 AGY 關閉器描述已過時。

## 子代理交付（皆終態）

- Codex `ai_planning_ui` / `scenario_events` / `module_review` 完成各自範圍與 review；沒有活躍寫入。
- AGY batch `83e47c16f8af`：terrain move、placement、nature tests 三項 CLI 10分鐘逾時，保留部分交付並由整合者檢查/測試；guide 正常交付。外層 succeeded 不能當作四項全部完成。
- AGY `ec4d9dcd730d`：新增 MapEditorSaveTransactionTests（3項），外層 succeeded 但 CLI 結束時仍等待/終止自己的背景測試；採整合者完整測試509通過的證據，不採其自述等待作成功。
- AGY logs 在 TEMP/agent-delegation-logs：`540163f0-task-{0,1,2,3}-agy.log`、`4df64ac1-agy.log`；不納入 repository。

## 恢復時優先處理（尚未完成）

狀態（2026-10-07 Claude）：roadmap 第 1–3 階段完成；第 4 階段 AI 依使用者指示「先跳過」（真實生成→預覽→套用→儲存→重開已有一次 live 證據）；第 5 階段在 repository 內可做的已完成（高 DPI 模擬、真實 OpenGL 畫面／選取／效能、3D fallback、入口、關閉流程）。剩餘項目都需要使用者授權或決定：

1. 遊戲內驗收（第 6 階段）：匯出地圖載入、原生材質/碰撞、物件/部隊、區域事件/勝敗、存讀檔、部族、多人、完整原版還原。AGENTS.md 禁止存取安裝目錄，必須由使用者另行授權或自行執行。
2. AI 製圖：暫停中；恢復時處理 AiMapPlanningDialog 高 DPI（未改 AutoScaleMode）、水系 river 缺 toLocation 被丟棄的提示。
3. 實體高 DPI 螢幕／跨螢幕 DPI 切換：本機 96 DPI，只有模擬證據。
4. 空白地圖語意：目前「平坦範本」保留聚落/腳本；是否需要真正無聚落/無腳本地圖需使用者決定並經遊戲驗證。
5. 已知小問題（未修）：Disable3DView 永久停用，同一表單內資源補齊後不會恢復 3D（每次開圖為新表單，影響小）；3D 不顯示通行覆蓋（僅 2D）。
6. 整體目標未完成，不能標記 complete；push 需另行授權。

## 文件與歷史

- 模組邊界：`docs/map-editor-modules.md`；使用指南：`docs/map-editor-user-guide.md`。
- `docs/ai-handoff-history-2026-10-07.md` 保存之前全部記錄與本輪收尾前快照，含其他代理原文；舊 running、實機、暫停與版本敘述皆以本檔和Git新證據為準。
