# 地圖編輯器需求與證據核對

## 2026-10-27 最新 Rust 驗收補記

- `PlacementUi::batch_edit` 與單筆隊伍設定／批次欄位提交現在共用物件類別隊伍驗證，避免 Squad 從公開批次入口改至 team 8 等非法隊伍。拒絕同批角度變更時 store payload、dirty、undo/redo、history token／receipt 均不變；team 7 加旋轉可 undo/redo。聚焦 3 passed；workspace check、完整序列 test（editor app 863 passed／6 ignored）及 build 通過。詳見 `AI_HANDOFF.md` 2026-10-27 批次物件編輯隊伍類別驗證。
- `4T` 三材質 junction 不再只依名稱 shape 推測：有 floortex 像素 probe 時以三個 base material 代表色與圖塊四角像素推斷，僅在三材質且每角 RGB 距離不超過 32 時收錄，無把握就拒絕。合成測試覆蓋 4 個 shape × 2 順序、像素推斷覆寫名稱 fallback、native resolver 回烘及低信心 fail-safe。只讀安裝版 `floortex.dat` 稽核 333 張 `4T`：308 張推斷並可回烘原生三材質角點，202 張改正名稱 shape 推測，25 張未通過像素證據門檻；來源封存檔未修改。`against-rome-textures` 13 unit／69 integration tests 通過（3 個安裝素材／地圖測試略過）。這不是遊戲渲染驗收；25 張仍保留未解析，遊戲內混合素材外觀待驗。詳見 `AI_HANDOFF.md` 2026-10-27 4T 三材質圖塊像素推斷。
- Phase 5 fallback 診斷與真實缺檔桌面路徑已驗證：在 TEMP ENDL_005 副本（無 `floortex.dat`）以新建置切至 3D，狀態列顯示 relief fallback；「複製 3D 診斷」按鈕於中英介面皆可用，實際點擊後 Windows 剪貼簿與各語系完整訊息逐字相同。桌面驗收也發現並修正切換語言後 fallback 訊息仍顯示舊語系；錯誤診斷細節保留原文。TEMP 與安裝來源 55 檔 SHA-256 全相同，未 Safe Save。headless egui 測試涵蓋雙語、語言往返及錯誤訊息原文；workspace check／test／build 通過（editor app 860 passed／6 ignored）。本次再以新建置實際切換 2D→3D→2D：3D 畫面顯示診斷／按鈕，回 2D 後畫面與 UIA 樹均無兩者；只切視圖，未寫圖。遊戲內效果仍未驗。詳見 `AI_HANDOFF.md` 2026-10-27 桌面驗收紀錄。
- AI 工作室已接入 `AiMapPlanPreviewBuilder`，方案審查新增目前地形／水域視覺圖、變更統計及材質／阻擋／拒絕／保護區圖例；方案與底圖來源未變時重用 texture，來源變更後重建。無視窗 UI 測試通過：預覽未改高度、碰撞、dirty 或 undo，底圖修改後快取更新。2026-10-26 實際桌面以 Ollama `laguna-xs-2.1:latest` 完成 3 角色企劃、含湖泊／河流水深的預覽、套用、Safe Save、關閉重開；詳見 `AI_HANDOFF.md` 最新紀錄。真實桌面完整互動已驗，高 DPI／多螢幕仍待驗收。
- AI 工作室「連線並列出模型」改由背景 worker 執行，UI 逐幀輪詢；端點 URL 納入方案輸入指紋，切換端點後會丟棄舊模型清單回應並清除舊方案。mock 延遲傳輸及端點切換測試通過；接入視覺預覽後 AI 工作室 21 項測試通過；完整 workspace 序列測試、check、build 通過。實際 Ollama 網路延遲／桌面操作仍需 live／GUI 驗收。
- 新增物件／事件整合回歸：Safe Save 將新放置主屋轉為 DATA 預建物件，同時儲存以該物件 UUID 為目標的矩形區域勝利事件；重載後核對 DATA slot／UID、UUID、事件條件與勝利動作均一致，且 BCI 可解析。聚焦測試 1 passed、workspace check/build 通過、編輯器 app 852 passed／6 ignored。完整 workspace 測試僅 `ollama_wire_tests` 兩項固定連接埠測試在並行執行時互相干擾；該測試檔序列隔離重跑 16 passed。遊戲內觸發未驗證。詳見 `AI_HANDOFF.md` 2026-10-26 DATA 建築區域勝利事件整合重載。
- Phase 3 物件刪除／事件目標跨功能補測：新增 app-level案例證明啟用事件引用的 spawn 無法刪除；刪除拒絕訊息提供中英文操作指引，提示停用事件或改選目標。原 UUID、DATA 與乾淨劇本維持不變；先停用事件後可刪除、Safe Save，並於全新 app 重載確認 DATA binding/spawn 移除、事件與原 target UUID 保留。聚焦 1 passed；workspace check／序列 test／build 通過（editor app 861 passed／6 ignored）。這驗證編輯器的拒絕／修復流程，未驗遊戲內事件觸發。詳見 `AI_HANDOFF.md` 2026-10-27 Phase 3 事件目標刪除整合。
- 空白場景建置現在會從 staging 初始化的實際寫入清單產生檔案報告，列出重設 DATA、未重設的所有 DATA 檔、保留 SDL 與重建腳本；建置後的 Maps UI 可展開查看，並明示保留池相容性與空白開局未經遊戲內驗證。staging、builder、wizard Safe Save/reload/Design Check 聚焦案例各 1 passed；workspace check、序列完整 test（editor app 861 passed／6 ignored）及 build 均通過。此報告只證明檔案處置清單，不構成遊戲相容性或開局證據。詳見 `AI_HANDOFF.md` 2026-10-27 空白場景初始化檔案報告。
- 原生 DATA 群組副本可批次變更隊伍／方向；來源 family 不變，既有 root binding 仍拒絕直接更改。遇到零售圖既存的無效 lager 連結時，Safe Save 會原樣保留未變更 root，壞 family 副本不深拷貝，改用可信 singleton template 或 script fallback。新增 2 個回歸測試。完整 workspace app 851 passed／6 ignored、`against-rome-maps` 86 passed／3 ignored，workspace check／test／build 均通過。2026-10-26 真實 TEMP 桌面複製 DATA 農場並設 T1／45°，Safe Save 12 files 成功；關閉重開後副本值與原三個 DATA 物件皆保留。該副本使用 script fallback，未驗證遊戲內效果。詳見 `AI_HANDOFF.md` 2026-10-26 零售原生 DATA 舊連結容錯與回歸驗證。
- 實際桌面：TEMP 地圖已完成 2D 高度筆畫→Safe Save→關閉→重新開啟；另驗證水位與起始鏡頭的 undo／redo。結果與 hash 記錄見 `docs/map-editor-ingame-checklist.md`。
- 遊戲啟動：2026-10-26 安裝版標題正常，但畫面仍黑。程序有回應、載入 `ddraw.dll`／NVIDIA `nvd3dum.dll`／`ds_andll.dll`／`Quartz.dll`；約 12 秒 CPU 增量 0.03 秒、PID 專屬 GPU 3D 三次取樣皆 0%。WPR 因追蹤權限失敗（`0xc5585011`），因此尚無堆疊證據。這不推翻 2026-10-23 既有遊戲內勝敗／波次／尋路證據，也不構成目前版本重驗通過；詳見最新 `AI_HANDOFF.md`。
- 3D 通行遮罩的 LOD 著色會掃描每個地表四邊形覆蓋的 collision cells，避免粗 LOD 中心格取樣漏掉局部阻擋區；無視窗 egui 測試以 LOD 4、偏離 quad 中心的單一阻擋格驗證紅色 polygon。2026-10-26 Codex 以原生 app 在 TEMP ENDL_005 進行桌面操作：在記憶體中用 2D 阻擋筆刷畫一筆，切至 3D Collision 後實際截圖可見紅色 polygon；點擊紅區後 UI 回報圖塊 (31,30)、通行格 (124,123) 阻擋，證明 GPU 畫面、picker 格定位與 collision 值相符。關閉時捨棄變更，未存檔；TEMP `collision.bmp` 長度維持 196662 bytes，時間戳未更新。桌面分別切換 LOD 2 與 LOD 4 均看到紅色阻擋 polygon；LOD 4 數值按鈕由原先超出 1280×830 視窗修為可見、可點選的精簡數字按鈕，tooltip 保留中英細節。workspace check／test／build 先前全數通過。
- 待完成仍包括：可觀察遊戲啟動後重驗必要玩法、原版材質／地形／部隊／事件全流程及支援範圍；實體高 DPI／跨螢幕；空白地圖開局。整體目標未完成。

## 2026-10-22 Rust 現況（取代下方 C# 時期細節）

下方 2026-10-07 表格引用的 `.cs` 測試與 WinForms 表單已隨 Rust 遷移退役；需求本身仍有效，證據改看 Rust 測試與 `AI_HANDOFF.md`。安裝目錄存取規則已改為 AGENTS.md 2026-10-08 授權（只寫自製測試圖並先備份）。

| 範圍 | Rust 證據 | 狀態 |
| --- | --- | --- |
| 開圖、CLI、唯讀驗證 | `cli_startup_tests`；`--validate-map [--lang zh]` 無視窗跑完整設計檢查（有 Error 時 exit 2） | 已驗證：40 張非戰役安裝版地圖唯讀批次檢查全部可開；壞別名 TEMP 副本實測 exit 2 |
| 設計檢查／Safe Save 阻擋 | `design_check_ui`、`MapDiagnostics`、`MapDesignValidator`、`route_check` 測試 | 已驗證；2026-10-22 去除進攻波次、別名目錄缺失、未編輯原版圖的誤報 |
| 地形、材質、水、通行、放置、劇本、事件圖、Safe Save 交易 | `against-rome-editor-app` 全套（lib 528）與 workspace 2,352 passed／0 failed | 合成與 TEMP 真實地圖往返已驗證 |
| AI 製圖（路線圖第 4 階段） | ignored live gates `tests/ai_planner_live.rs`、`live_ollama_water_materials_apply_undo_save_reload`；retail gate `retail_ai_offered_materials_all_paint_on_endl_005` | 已驗證（本機 laguna-xs-2.1）：地形、水域、材質三角色皆有 生成→預覽不改圖→套用（範圍外不變）→一次 undo/redo→Safe Save→重載一致。2026-10-22 修正 tile 座標、單步 undo、材質層未接、虛構材質 id、套用後擋存檔 |
| 桌面視窗 | `--smoke-test`、無視窗完整 UI 事件 gate；2026-10-26 真實桌面開啟 TEMP ENDL_005，AI 面板三角色生成→預覽→套用→工具列 Undo／Redo | 已完成 AI 對話框這條人工桌面路徑：三角色 3/3 成功；套用 67,221 高度樣本／381 通行像素／3 材質筆刷／74 操作；關閉選擇捨棄，未 Safe Save。1280×830 畫面中 AI 預覽需捲動；**仍未完成**：實體 DPI／多螢幕、其他面板全面實際操作、GPU 畫面與遊戲內驗收 |
| 遊戲內載入、尋路、勝負 | 2026-10-22 使用者授權 computer use；ENDL_005 先備份至 `%TEMP%`、測後還原（55 檔 SHA-256 全符） | **已驗證**：自製圖「Claude Test 1b」出現在無盡模式清單並載入；預建 DATA 主屋與住宅完工出現；劇本訊息第 3 秒顯示；勝利於設定的 40 秒準時觸發；以 Rust 編輯器重新編譯並存檔（Wave_01 改 10 秒、勝利改 120 秒）後，勝利準時於 2:00 觸發、波次在指定座標（主屋旁）生成；玩家部隊依右鍵命令移動（尋路）。同日修正後：腳本生成給隊伍 0 的部隊可選取並依命令移動（編譯器 AI 控制旗標隊伍 0 改傳 0，與原版範本一致）；敵方波次（隊伍 1）於主屋東側目視生成；失敗動作於設定的 15 秒準時觸發並顯示失敗統計。劇本未設定外交時，事件生成的敵方隊伍為非敵對（黃色血條）；設計檢查會以 `wave-not-hostile` 提醒並提供一鍵修正，2026-10-23 實機確認修正後同一波次顯示紅色敵對血條；腳本生成部隊不計入人口顯示。畫面／GPU／DPI 視覺仍以遊戲實機截圖為主，編輯器視窗本身未以 computer use 操作 |
| 3D GPU 管線 | CPU draw list | 非編輯阻礙，依範圍政策不追求原生渲染精度 |

## 2026-10-07 C# 時期核對（歷史）

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
| 空白地圖語意 | Flat Template／平坦範本地圖維持既有整平流程；獨立「空白場景（實驗）」清來源物件／事件、13份runtime池（含光源與特效）與原生成腳本，以新idle main與backup取代 | 真實TEMP建立／保存／重開／新放置及事件／移除後回復bootstrap已驗證；其餘原生DATA池與遊戲開局仍未驗證，詳見空白契約；未宣稱可玩 |
| 3D 真貼圖、水面、光照、標記、筆刷與 fallback | `MapEditorOpenGlTests.cs` 真實 GPU 離屏擷取（RTX 4080／GL 3.3） | 高度/undo/材質/水面更新已有真實畫面證據；2026-10-26 真實桌面 TEMP ENDL_005 3D 材質貼圖模式選「土路」並拖曳筆刷，材質鄰接篩選由隱藏 72 種變為 60 種；工具列 Undo／Redo 均顯示已套用統一復原紀錄，篩選隨之回復／重現，最後 Undo 回原狀。缺少 `floortex.dat` 的 fallback 及中英診斷複製按鈕已由 TEMP 桌面操作驗證；切換 UI 語言後訊息同步更新，兩語剪貼簿文字逐字正確；TEMP 55 檔未變。新增 headless egui 回歸並完成新建置桌面切換驗證：3D 顯示 fallback 診斷與複製按鈕，2D 隱藏；無 GPU fallback、筆刷效能及高 DPI 尚待驗收 |
| UI 中英／尺寸／DPI | 96 DPI 證據；`MapEditorHighDpiTests.cs` 100/150/200% 模擬 | 編輯器視窗（AI 對話框除外）改 AutoScaleMode.Dpi 並修正截字；實體高 DPI 螢幕未驗證 |
| AI 正式生成／預覽／套用 | 序列鎖、取消、部分失敗、套用與儲存；視覺預覽接UI，隔離／拒絕區域／統計／圖片釋放、角色進度合成測試；`tests/ai_planner_live.rs` | 2026-10-26 現版以實際 `laguna-xs-2.1:latest` 完成單角色與三角色 live gate；另於桌面完成三角色生成→含水域預覽→套用→undo/redo 操作→Safe Save→關閉重開。三角色方案實際含 1 湖 amount 12、2 河 amount 16／16；套用 35,025 高度取樣、5,245 碰撞像素、1 材質筆刷、62 操作，Safe Save 一交易寫入 6 檔並由 TEMP 遊戲根目錄重載成功。首次 `--map` 無別名目錄的測試未能 Safe Save，後以 `--game <TEMP> --map ENDL_005` 修正。角色範圍外特徵會被忽略；遊戲內水面表現、實體 DPI／多螢幕與 GPU 畫面仍待驗收 |
| 遊戲載入、存讀檔、原版品質 | 2026-10-22 舊輪未執行遊戲；10-26 已依授權啟動安裝版並檢查模組、CPU、執行緒及 GPU counters，畫面黑且 WPR 無權錄製 | 遊戲內 gate 未完成；依 `AGENTS.md` 僅能改動先備份的自製測試地圖，原版檔案唯讀；追蹤權限或可觀察環境仍待解決 |

## 下一批優先工作

1. 本輪建立／刪除修正已提交 `b9b9965`；獨立 git archive 快照 build 0 警告/0 錯誤，完整測試 536 通過/21 略過/0 失敗，已確認不依賴 AI 預覽 WIP。
2. 原廠圖 catalog 與 SaveMap 前置驗證已補9項測試；最新完整build 0警告/0錯誤，545通過/21略過/0失敗。
3. 地圖文字、環境、備份／完整還原、額外槽位 AI 修補與平坦範本命名已補專屬合成證據；真實遊戲與新對話框視覺缺口仍保留。
4. 接著進入第 3 階段物件／部隊／事件整合，補部族、隊伍槽位與共同儲存／重新載入矩陣；第 4 階段再完成 AI 預覽 WIP。

尚未完成的項目保留在清單中，不以目前可通過的測試重新定義成功。所有執行驗證只使用 repository／TEMP 合成資料。
