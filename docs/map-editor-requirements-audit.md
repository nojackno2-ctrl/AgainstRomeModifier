# 地圖編輯器需求與證據核對

2026-10-07，Codex。依 `map-editor-roadmap.md` 第 1 階段核對。這是缺口清單，不是完整完成宣告。

## 需求來源與已變更項目

- 原任務 `map-editor-spec.md` §0 是從範本建立、編輯 ENDL 無盡地圖。其他地圖可列出、檢視，不代表多人同步或劇情編輯已成為正式支援範圍。
- 原 §1 的獨立 EXE／修改器分頁敘述，已由同文件 Phase 1 的統一程式與 `--game`／`--map` 增補更新。現況：MapEditor 是 library，`src.Modifier/Program.cs` 處理入口；不為符合舊段落再拆 EXE。
- `map-editor-3d-view-spec.md` 的「不新增寫入面」是當時 3D 檢視階段限制；後續整體規格已增補高度／通行與 AI 編輯。保留後續功能，但不能省略原版輸出及遊戲內驗收。
- 3D Phase D 的 billboard 是選做且需另行同意；`.alr` 模型逆向不在該規格範圍。不能把選做項目當成目前完成門檻。
- 使用者已澄清開發用本地 AI 與產品 AI 不同；產品 AI 現在明確要求保留並完善。

## 核對結果

| 需求 | 目前證據 | 判定／下一步 |
| --- | --- | --- |
| 啟動與直接開圖 | `Program.ResolveDirectMap`；`MapEditorEntryTests.cs` 合成 root：大小寫不敏感開啟自製、原版唯讀、缺 --game／不存在／不安全代號／缺遊戲目錄拒絕 | 2026-10-07 修正：直接開圖原可繞過選單開啟劇情戰役 KAMP_，現與選單同範圍拒絕；參數錯誤改為用法提示，不再當成崩潰寫 crash_log。實際 exe 啟動未執行 |
| 原版唯讀、自製槽位 005–999 | `CustomMapAccess` 統一兩個catalog、SDL service與Save；真正表單鎖briefing測試證明拒絕先於檔案開啟 | 已修：原廠帶marker仍唯讀；stale selection／移除marker／錯誤root拒存，bytes與dirty保留，合法marker恢復可retry |
| 完整複製未知檔案、header、SDL 路徑 | `EndlessMapCloner`、Phase1 Clone / Documents 測試 | 合成測試涵蓋；遊戲內載入仍缺 |
| 複製任何階段失敗無半成品 | 新 manifest Load／Save 失敗測試先重現正式槽位殘留 | 本輪修正：manifest 交易與本次目標回滾；成功可原槽位重試 |
| 不覆蓋既有正式／暫存目錄 | Clone 在 copy 前檢查；新增 sentinel 測試 | 本輪測試涵蓋，既有內容不得刪除 |
| 刪除三重防護與原廠槽位拒絕 | 原 Deleter 缺 manifest membership／slot guard；新 slots 0/4/5/999 測試重現 | 本輪補回登記＋marker＋受控槽位／路徑條件，在 rename 前拒絕 |
| CP1251、文字常值限制、多行簡報 | Phase1與新MapTextEscapingTests、真正表單MapEditorTextSaveTests | 單值引號/反斜線/tab安全round-trip、escaped bytes限制、NUL拒絕、原行保留；輸入CP1251即時提示，拒存保留bytes/dirty，修正可retry；遊戲內文字呈現仍待驗收 |
| 環境參數與未知行保留 | 真正表單三地區（en-US/de-DE/fr-FR）七值、Heightmapstep、雨滴、dirty、儲存／重開與重存bytes相同測試 | 修正依Windows地區讀錯數值及寫出逗號問題；未知行保留。合成小數案例不證明遊戲接受所有小數參數 |
| 聚落移動、複製、刪除、重新編號 | SdlSceneEditService／SdlDocument，Phase1 多項測試 | 合成路徑已有證據；遊戲內仍缺 |
| 自製圖不進備份基準 | `CustomMapBackupTests` 鎖檔、005/999及nested案例；兩backup loader統一判定擁有者marker | 修正子目錄team.dat誤納入問題；不讀自製team、不產bak、已有bak保留；原版基準正常 |
| 完整還原保留／刪除自製圖 | 新dialog→PatchEngine→CustomMapRestoreService；15項合成流程/STA/真正runner與engine測試 | 已實作預設保留、受控刪除、目錄/manifest回滾；僅完整還原跳過custom腳本/team基準，正常AI仍涵蓋custom；原版成功還原與新dialog視覺仍待驗收 |
| AI 修補涵蓋新 ENDL 圖 | `EndlessAiAdditionalMapTests` 真正 P1 七槽位 Detect／Apply／Save／fresh reload／還原 | 已修正式數字槽位搜尋；排除非數字、暫存、刪除中與巢狀複本；SDL 僅搜尋地圖根目錄 |
| 地形、材質、通行、歷史／儲存交易 | 驗收矩陣列出的 session、真正表單及 AI 跨模式測試 | 合成路徑已驗證；原版輸出仍有遊戲內 gate |
| 自然物件／人物／部隊／建築與事件 | 單兵與nature fail/retry、身份、VM／dialog；四部族建築0–8/部隊0–7與事件的STA共同儲存矩陣；缺建築範本中英提示/目標/retry/重開/範本恢復 | 合成資料整合與工地提示已補；接著完成AI預覽；不推定多人同步或真實範本已驗證 |
| 空白地圖語意 | 選單改稱 Flat Template／平坦範本地圖；工具改稱 Reset Flat Terrain／重設平坦地形 | 保留範本聚落、腳本與連結物件，整平與可移除地景清除仍走既有待儲存流程；真正無聚落／無腳本地圖尚未驗證 |
| 3D 真貼圖、水面、光照、標記、筆刷與 fallback | `MapEditorOpenGlTests.cs` 真實 GPU 離屏擷取（RTX 4080／GL 3.3） | 高度/undo/材質/水面更新已有真實畫面證據；修正關閉表單時 GL 資源在 context 銷毀後才釋放、觸發重新初始化與 BeginInvoke 例外。資源失敗診斷、無 GPU fallback 與互動效能尚待驗收 |
| UI 中英／尺寸／DPI | 96 DPI 證據；`MapEditorHighDpiTests.cs` 100/150/200% 模擬 | 編輯器視窗（AI 對話框除外）改 AutoScaleMode.Dpi 並修正截字；實體高 DPI 螢幕未驗證 |
| AI 正式生成／預覽／套用 | 序列鎖、取消、部分失敗、套用與儲存；視覺預覽接UI，隔離/拒絕區域/統計/圖片釋放、角色進度合成測試 | 第4階段續做新UI視覺與真實生成→預覽→套用→儲存/reload；不能沿用舊live結果 |
| 遊戲載入、存讀檔、原版品質 | 本輪沒有執行遊戲 | 尚未完成；仍遵守 AGENTS.md 禁止存取安裝目錄 |

## 下一批優先工作

1. 本輪建立／刪除修正已提交 `b9b9965`；獨立 git archive 快照 build 0 警告/0 錯誤，完整測試 536 通過/21 略過/0 失敗，已確認不依賴 AI 預覽 WIP。
2. 原廠圖 catalog 與 SaveMap 前置驗證已補9項測試；最新完整build 0警告/0錯誤，545通過/21略過/0失敗。
3. 地圖文字、環境、備份／完整還原、額外槽位 AI 修補與平坦範本命名已補專屬合成證據；真實遊戲與新對話框視覺缺口仍保留。
4. 接著進入第 3 階段物件／部隊／事件整合，補部族、隊伍槽位與共同儲存／重新載入矩陣；第 4 階段再完成 AI 預覽 WIP。

尚未完成的項目保留在清單中，不以目前可通過的測試重新定義成功。所有執行驗證只使用 repository／TEMP 合成資料。
