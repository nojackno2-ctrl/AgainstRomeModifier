# 地圖編輯器與存檔管理器功能驗證盤點（2026-10-21）

狀態：**部分靜態證據可追溯，並非所有功能驗證完成。** 此分工由 Codex 唯讀盤點目前 Rust 來源、格式文件及本地反編譯紀錄，只新增本檔；分工未執行遊戲、測試、建置或 Git 操作。父端另執行新反編譯、workspace 與零售 EXE 測試並維護交接，最終結果見 [本輪索引](feature-audit-2026-10-21.md)。

## 範圍與證據口徑

- 起始 HEAD `4ef8282`、分支 `主要開發`；起始未提交 `AGENTS.md`、`README.md`，均保留。工作期間交接由其他工作更新，故其行號僅為本次讀取快照。
- 已讀 `AGENTS.md:1` 起與 `AI_HANDOFF.md` 的 Rust 遷移、retail gate、水筆刷、cross-pool、事件圖、聚落整合段落。本表僅列分工已讀取的證據；父端後續驗證結果集中記錄於本輪索引。
- 功能入口來自 `apps/against-rome-editor/src/app.rs:8912`–`9007` 的實際工具列及各面板／native 實作，不以 README 當清單。獨立存檔管理器與內嵌工具箱分開核對；修改器個別補丁應另對照 modifier 稽核，不能以本表代替。
- 本次 `Get-FileHash -Algorithm SHA256` 實讀安裝版 `C:\Program Files (x86)\Against Rome\Against_Rome.exe` 與本地 `re_workspace/Against_Rome.exe`，皆為 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。地址均為此 x86 PE 的 VA，非檔案位移；不外推其他版本。
- 「靜態部分」表示有 loader／consumer 或 VM 鏈可支持資料語意，**不表示這次已逐欄位對齊全部 Rust writer**。「工具」表示原遊戲反編譯無法證明工具自身正確；「混合」須兩者。表內測試路徑是現存覆蓋定位，本次未重跑，亦不把交接中的舊測試數當成本次驗證結果。
- 下列 `A/` = `apps/against-rome-editor/src/`，`N/` = `native/`，`D/` = `docs/reverse-engineering/`。`路徑:行號` 供直接搜尋定位；同列 native crate 為資料流歸屬。

## 本地反編譯抽查（並非本次重新反編譯）

| 編號 | 已讀來源／精確定位 | 觀察與限制 |
|---|---|---|
| E1 | `re_workspace/map-pools-20261009-evidence.json:1`，`records` 中 `ev_294c5cb999953ced778a0bad0bc09cf23c19f088ae24e0b8521b0389b6d123de`，`procedure_pseudo_code`，查詢 `0x0048742f`，實際函式 `FUN_004870e0` | 偽碼依序呼叫 `481de0→481ba0→483410→486ec0→4835f0→483bf0→4845b0`，支持地圖載入順序；查詢內部地址不代表新函式入口。 |
| E2 | `re_workspace/map-load-contract-20261009-evidence.json:1`，`ev_47c2c76e8e966f30dfb2478700e29e67353583191de9da0764b6af62f9c6e5e1`，`read_function_instructions(0x48e960)`；`re_workspace/map-load-pipeline-instructions-20261009.txt:1` 起 | 有 `0x48ea00` 共用錯誤返回及各 reader 呼叫；這份是反組譯，不能稱為偽碼。另 `ev_eb772e7ffac907d85d099ac70027f3e34314c0ca6aab86b4fb73f5c53f0f3ba3` 保存 `0x48742f` 的 75 bytes。 |
| E3 | `re_workspace/family-movement-20261010-evidence.json:1`，`ev_0e47aab2b1d1e49cd0cbb0ebb80f4c5c511b796f550dacb18e7800163ef49dbe`，`batch_decompile`，地址 `0x4dbe60/0x4bd3a0/0x4bc1f0` | 已抽查粒子更新偽碼的 active、owner object／type 查詢及時間更新；不能由此證明任意 family 可安全複製、刪除或遊戲內顯示。 |

JSON 為單行檔，故以 `:1`＋Evidence ID／procedure 定位；它們是本地歷史分析輸出，未重新驗證每筆 ledger 完整性。E1／E2 與 EXE 相同雜湊支持重用；其餘地址若僅列於文件，以下明列為文件證據。

## 主要功能覆蓋表

| 使用者功能 | Rust 入口／核心 | 遊戲格式、loader／consumer 證據 | 目前判定與缺口 |
|---|---|---|---|
| 開啟地圖、圖層與地圖設定 | `A/app.rs:874`；`A/map_settings.rs:14`；`A/textures_ui.rs:784`；`N/against-rome-maps`、`against-rome-textures` | E1；`D/map-load-pipeline.md:23`；INI `0x481de0`，材質 `0x481ba0`，高度 `0x483410` | 靜態部分：還須逐欄位比對尺寸、PFIL／文字編碼、缺檔 fallback；開檔失敗 UI 另測。 |
| 升／降／整平／平滑／雜訊筆刷、坡道、重設／整張整平 | `A/app.rs:51,3396`；`A/terrain.rs:133,173`；`N/against-rome-editor/src/terrain.rs:27,138` | `D/map-formats.md:30`：高度 GREEN×Heightmapstep；`D/map-lighting.md:82,87`：native smooth 多 pass | 混合：輸出高度語意有證據；自製筆刷／ramp 演算法不等於 native smooth。需邊界、量化、undo、存後重載；`tests/terrain_adapter.rs`、`region_tools.rs`。 |
| 水筆刷、阻擋／清除碰撞 | `A/terrain.rs:152,161`；`A/map_settings.rs:52`；native editor terrain | `D/map-formats.md:28,41`，`0x486ec0/0x4d87a0/0x4d8830`；visible bit0 可再強制阻擋 | 混合：水位換算與碰撞語意可追；不能只用畫面判斷可走。`tests/editor_tests.rs` 有跨高度／碰撞往返定位；需核對下列門檻疑點。 |
| 材質繪製、取樣、空白底材、混合與圖層 | `A/textures_ui.rs:1001,1411,1612,1671`；`N/against-rome-textures/src/{document,format,floortex,terrain_layer}.rs` | `D/map-load-pipeline.md:26` `0x481ba0`；E1 | 混合：boden.txt consumer 有入口，仍需逐 token／混合旗標對照；`N/against-rome-textures/tests/installed_maps_tests.rs` 是 retail TEMP gate，非實際遊戲載入。 |
| 2D／3D、原生資產、光照、小地圖、縮圖與點選／框選 | `A/app.rs:43,1961`；`A/viewport_3d.rs`；`A/minimap_ui.rs:164`；`N/against-rome-scene`、`against-rome-data` | `D/map-lighting.md:80` 起（`0x49a350/0x499620/0x49ffe0`）；`D/native-scene-rendering.md`、`alr-rust-scanlines.md` | 混合：資產／部分光照鏈可追；UI 投影、拾取、快取、縮圖需工具／視覺測試。emboss 最終混色、部分日夜列仍未知，不能宣稱像素等價。 |
| DATA 物件移動／多選／吸附／複製／刪除／生命與士氣資料 | `A/placement_ui.rs:374,486,616`；`N/against-rome-placement/src/lib.rs:2829,3231,3370,3402,3810` | `D/map-data-pools.md`；`D/objdata-contract.md`；`D/map-load-pipeline.md:67` 起 readers、`0x4acfe0/0x4ad110` consumers | 混合：runtime stride 不可當磁碟 stride；跨池指標、UID、未管理引用及可寫欄位需逐一驗證。現有 `tests/family_differential.rs`、retail roundtrip 不涵蓋所有 family。 |
| Families & Effects：family 預覽／複製；Lights／Particles／Definitions 檢視 | `A/family_ui.rs:76,132,232,391,409,577`；native placement/data | E3；`D/local-lights.md`、`saved-light-render-contract.md`、`particle-definition-loaders.md`；light reader `0x48ab70`、particle `0x48dd90` | 靜態部分：檢視器不等於任意特效可編輯；owner／parent／light／lager／biglager／form 關聯、unsupported 欄位拒絕須測。 |
| SDL 場景物件增／複製／刪除、位置／隊伍／角度、整座聚落平移 | `A/scene_ui.rs:222,271,733,846,939`；`N/against-rome-maps/src/sdl_scene.rs` | `D/map-formats.md:57`：`0x473600` 開發 UI 與建造路徑；SDL `onload` 不等於自動建造 | 靜態部分：寫對 SDL 不保證遊戲啟動會建立；須沿實際 BCI 呼叫或 DATA 預建鏈核對，特別是套用次數／重複建造。 |
| 出生點放置／批次／隊伍／方向／人數／縮圖與預建綁定 | `A/spawn_placement_ui.rs:1` 起；`A/scenario_ui.rs:150`；`N/against-rome-scenario/src/compiler.rs:434`；maps `spawn_data.rs` | `D/spawn-formations.md:37,52,94`；`0x52a020→0x524530→0x5245d0`；`D/scenario-event-conditions.md:117` | 混合：士兵上限20、旗幟另建、隊形320為縮放非相鄰距離。預建 DATA／腳本 fallback／slot+UID 綁定須各測；`tests/spawn_placement.rs`、`spawn_canvas.rs`。 |
| 劇本事件：條件、動作、增刪改、別名綁定與編譯 | `A/scenario_ui.rs:219,250,260,318`；`N/against-rome-scenario/src/compiler.rs:50,195,641,663` | `D/scenario-events.md:5` 起：opcode128→`0x5b1700`、86、比較96→101；`D/scenario-event-conditions.md:15,83`；`scenario-area-mission-result.md:11,46` | 靜態部分：可驗 VM ABI、字串引用、slot+1／UID；事件組合、重入、每次觸發、舊腳本 hook 唯一性仍須工具及遊戲驗收。 |
| 事件圖（面板與工具列）載入／驗證／套用 | `A/event_graph/ui.rs:163,229,250`；`A/app.rs:1399,1466` | 繼承事件 compiler 證據，圖形本身無遊戲格式 | 工具：兩份草稿、來源衝突、未套用阻擋儲存、單步 undo；`tests/toolbar_event_graph/mod.rs`。反編譯僅能驗編譯後 BCI。 |
| 開場鏡頭／過場／飛行路徑／字幕與淡入淡出 | `A/campaign_ui.rs:737`；scenario compiler `:396`；`N/against-rome-campaign/src/cinematics/` | `D/cinematic-camera.md:48,68,93`：`0x54c400→0x54c010→0x4989d0`；zoom `0x498a30` clamp0..9 | 混合：參數型別／堆疊清除可驗；預覽距離82不是 native zoom。時間軸、曲線與實際鏡頭效果未驗。 |
| 戰役目標／勝敗、波次／錯峰進攻／路點與計畫合併 | `A/campaign_ui.rs:330,544,558,690`；`N/against-rome-campaign/src/{objectives,ai}/` | `D/objective-rules.md`；`D/scenario-area-mission-result.md:46`：GLOBAL_MISSION_RESULT→`0x478970/0x479020`；繼承 spawn/event ABI | 混合：目標及波次生成是工具政策；需逐模板事件、任務結果0..3、alias／單位目標及錯峰時間測試；不能用 native 存在證明整個戰役可玩。 |
| 自然筆刷／移除、植物散佈、野獸巢穴、聚落／資源布局 | `A/app.rs:2967`；`A/environment_ui.rs:619,703,753,821`；`N/against-rome-environment/src/{nature,wild_lair,settlement}/` | `D/wild-lairs.md:102,114`；`0x5192e0` createObj／`0x52a020` createUnit；SDL／DATA 鏈同上 | 混合：生成演算法、地形適合度、includeNature、跨 session 一次套用／回滾需工具測試；`app/tests/settlement_apply_tests.rs`。野獸模板名稱不是原生 lair 復活機制證明。 |
| 崖壁、河流、道路、城牆、侵蝕、區域與折線工具 | `A/landscape_ui.rs:290,328,371,406,501,1515`；`A/design_apply.rs:203,224,329,387,460`；`N/against-rome-landscape` | 最終輸出走高度／材質／collision／placement 證據 | 工具為主：native 不提供自製規劃演算法的正確性標準；逐輸出層重載，驗城牆基礎／碰撞、邊界與單步 undo；`tests/region_tools.rs`。 |
| Layout 匯入／匯出與範本重用 | `A/layout_io.rs:70,85,160,220,299,344,440` | `.arm-layout.json` 為工具格式；套用後繼承 DATA／自然輸出鏈 | 工具：型別名稱解析、版本、中文、未知欄位、相對錨點、容量不足拒絕；`tests/layout_compat.rs`。不能由遊戲反編譯驗 JSON。 |
| 氛圍：水／天氣／光照 preset、聲音區域、briefing | `A/ambience_ui.rs:281,333,380,420,546`；`N/against-rome-ambience` | `D/boden-ini-values.md`；`D/map-lighting.md:80` 起；`D/sound-zones.md:75` 起，`0x4e1260/0x4e1880/0x4e2790` | 混合：INI 值／BCI 聲音呼叫部分可追；shader／閃電預覽不等於遊戲。需逐 preset 寫檔白名單與 BCI 聲區 consumer 完整追蹤；`tests/blank_map_and_ambience_e2e.rs`。 |
| AI 企劃：模型清單、生成／取消／審閱／限定區套用 | `A/ai_planner_ui.rs:238,311,401,545`；`N/against-rome-ai-planner` | 服務與計畫非遊戲 ABI；只有套用後圖層繼承格式鏈 | 工具：需 transport、取消、過時結果拒絕、範圍／水位、失敗不變更 session 測試；無法以 EXE 反編譯證明模型輸出品質。 |
| 地圖目錄／複製成自訂圖／刪除／空白與平地模板／Starter Kit | `A/maps_ui.rs:262,325,390,420,441`；`A/app.rs:1116,3824`；maps `blank_builder.rs:27`、`custom_map.rs:56` | E1/E2；`D/map-load-pipeline.md:64,106`：24 pools 缺檔仍進 CHECK；`object-create-contract.md` | 混合：空白地形不等於完整可載入關卡；須24池及預設標頭、team/script/UID、自訂標記及刪除邊界。`app/tests/starter_kit_tests.rs` 不等於遊戲驗收。 |
| 導航／可達性診斷、設計檢查、診斷／preflight | `A/navigation.rs:22`；`A/design_check_ui.rs:253`；`A/maps_ui.rs:461,491,524`；`N/against-rome-navigation`、maps | `D/map-load-pipeline.md:54`，`0x4d8830/0x4ae320`；collision 語意 | 混合：編輯器圖搜尋不等於引擎完整路徑搜尋；pool／格式檢查可驗部分硬限制。乾淨報告不代表遊戲可玩，需列啟發式／未檢欄位。 |
| Safe Save、備份／交易回滾／cache 失效、未儲存保護 | `A/app.rs:324,574,1245,2427`；`N/against-rome-maps/src/transaction.rs:205,289,506` | `D/map-formats.md:43`：高度總和／水位 cache key、失配重建；E2 提醒缺 pool 風險 | 混合：cache 刪除合理性可靜態追；atomicity、鎖、外部變更、失敗保留草稿、跨層 baseline 只能工具注入故障測試。刪 cache 後遊戲會寫回，本次未觸發。 |
| Undo／redo、快捷鍵、跨面板草稿／歷史 | `A/app.rs:3334,3352,3374,3596`；`A/terrain.rs:117,123,126` | 無原遊戲 consumer | 工具：`tests/shortcuts_and_camera_keys.rs`、`editor_tests.rs`、各 session 測試；須驗操作優先序、group、重載／存後歷史重設，不能由 loader 背書。 |
| 打包／Manifest／匯出前檢查 | `A/packaging_ui.rs:236,262`；`N/against-rome-packaging/src/{exporter,preflight,manifest}.rs` | `D/virtual-file-system.md:9,55`：`0x5801e0` 開檔、ZIP backend `0x552820/0x552a20` | 混合：遊戲支援 ZIP backend 不表示匯出包會自動掛載；須驗目的路徑、必需依賴、白名單、manifest、覆寫／回滾與實際安裝方式。 |
| Map Diff／baseline／budget profiler | `A/packaging_ui.rs:302,321,366,600`；native packaging `diff/`、`profiler/` | 無對應原生功能；limits 可對照24池容量 | 工具：`tests/packaging_map_diff.rs` 與 native packaging tests；有限 baseline／Unavailable 層不能報 PASS。budget 是估計，不是 FPS／記憶體 runtime 保證。 |
| 主控台／巨集、技術文件、雙語、Test in Game | `A/console.rs:105`、`A/macro_cmd/`、`A/tech_doc_ui.rs:114`；`A/app.rs:4067`；`A/packaging_ui.rs:1195,1203` | 遊戲啟動規則參考 `D/game-command-line.md`；其餘為工具 | 工具：命令逐一映射 session／失敗語意、文件是否過時、UI、程序參數需另測；本次未呼叫啟動功能。 |

## 存檔管理器與工具箱（不能漏算）

| 功能 | 入口與核心 | 可反編譯驗證部分／目前證據 | 必須補的工具驗證或缺口 |
|---|---|---|---|
| 獨立視窗：路徑選擇、重新掃描、雙列表、標題／關卡／時間／預覽 | `A/save_manager_ui.rs:389,424,462,498,533,939`；`N/against-rome-modifier/src/save_manager.rs:76,87,150`→saves | `N/against-rome-saves/src/lib.rs:196,285,672,681` 讀 save.ini／TGA；`D/map-load-pipeline.md:15` 只確認存檔入口 `0x431410` | 尚缺 save.ini 各欄位、preview TGA 的原遊戲完整讀取鏈；CP1251 為目前工具實作，不代表所有語系已證實。現有 UI 測試 `:1640,1700,2013`，非本次執行。 |
| ZIP 備份／自訂標題／manifest | 獨立 `A/save_manager_ui.rs:594`；工具箱 `A/toolbox.rs:287,386`；saves `lib.rs:378,384` | ZIP 為工具封裝；原遊戲不需直接讀此 manifest | CRC、全檔完整性、命名衝突、時間與缺 metadata、唯讀／惡意路徑；不能以遊戲 ZIP backend 證明備份可完整還原。 |
| 還原／覆寫確認／失敗回復／相容性修補 | 獨立 `A/save_manager_ui.rs:702,798`；工具箱 `A/toolbox.rs:394`；saves `lib.rs:449`、`backup_health.rs:107` | 實讀 restore：先解至 temp，再移走舊目錄、發布、失敗回滾；`D/map-load-pipeline.md:15` 無法證明恢復目錄的全部 consumer | 須故障注入、目的地重現、link／重複 entry／超限／缺檔拒絕、cleanup warning；heal 行為須對照實際修補檔案與版本，完整存檔載入鏈待追。 |
| 刪除存檔／刪除備份／取消與狀態回報 | 獨立 `A/save_manager_ui.rs:642,672,794,798`；工具箱 `A/toolbox.rs:408,416`；saves `lib.rs:695,703` | 無原生相容性語意可替代確認流程 | 工具：選擇失效、掃描失敗、path traversal、junction／symlink、Program Files guard、操作成功後重掃失敗須保留錯誤；UI 測試 `:1775,1814,1911,1949,1974`。 |
| 無盡 AI 修復（兩入口） | 獨立 `A/save_manager_ui.rs:764`；工具箱 `A/toolbox.rs:424`；saves `lib.rs:226`、`ai_repair.rs:239` | `D/endless-mode-ai.md:770` 起：定位 CLAK/scr.dat 內 ENDL BCI，P6/P8/P9；VM `D/scenario-events.md:5`；BCI offsets 不是 EXE VA | 混合：實讀程式先備份並 verify，再 stage／比來源／rename；須唯一簽章、legacy／already repaired／未知版本、PFIL 保留、deadline scalar／task 同步證據。文件本身仍標 runtime pending，不能宣稱修完所有 AI。 |
| 工具箱專有 Verify selected save／backup（與獨立視窗勿混同） | `A/toolbox.rs:445,464,609,613`；saves `lib.rs:526,629` | 工具檢查 ZIP CRC／路徑／大小／metadata 可讀，不是遊戲 consumer 驗證 | 現有 toolbox 測試 `:1030` 後涵蓋損壞／失效選取／唯讀不寫入；is_ok 不保證24池、BCI task state 或實際遊戲載入相容。獨立 adapter 的公開入口未見 verify，不應合稱兩者皆有按鈕。 |
| 工具箱 EXE Inspect／Review patches／Apply 與 Modifier Studio | `A/toolbox.rs:123,501,526,540,546`；`A/app.rs:8912,8915`；`A/modifier_ui.rs`；native modifier | 各補丁應逐 FeatureRegistry 追 writer→detect→restore→loader／consumer；舊 `D/feature-verification-matrix.md:3` 僅49個舊登錄的靜態快照 | **另案逐 feature 盤點**；本表確認入口存在，未把舊矩陣 verified 自動轉成目前 Rust 全通過。Review 後來源變更及拒絕未知 EXE 需工具測試。 |

## 優先疑點與下一步

1. **碰撞門檻不一致（已解決，保留原稽核疑點）**：後續直接追到 loader、單位門檻尋路 consumer 及 `<=5` 選點 consumer，並確認 `0x4ae320` 不讀碰撞格。聚落已改用 `>=6` 排除；導航診斷仍採非零保守策略。指令與限制見[碰撞契約](collision-passability-contract.md)，未做遊戲內驗收。
2. **地圖與存檔載入不可混用（高）**：E2 證明地圖24池開檔失敗會返回、caller仍進CHECK；不自動證明 `0x431410` 存檔路徑相同。應補存檔入口→CLAK、DATA、save.ini、腳本task恢復的完整鏈及各版本格式。Verify OK／restore成功不能當作遊戲接受。
3. **儲存鏈尚無逐 writer crosswalk（高）**：按 Safe Save staged files 列到 BMP通道／列向、boden token、PFIL、SDL、BCI、DATA header／record／tail，逐一對 reader與consumer；只找到loader名稱或檔案存在不算完成。
4. **Family 可寫範圍（高）**：交接記錄74圖／4140 roots，僅2912接受，其餘fail closed，耗時約102秒；這是既有報告而非本次重跑。分類unsupported欄位／損壞／外部引用，不應把拒絕全部視為正確或全部視為缺功能。inactive position/light孤兒引用須保留。
5. **BCI／SDL／DATA生成邊界（高）**：SDL onload不自動建造；腳本spawn與預建slot+UID必須互斥且重載不重複。條件slot+1／UID、20人隊形上限、任務結果0..3、字串常數與opcode86逐項驗，code-stream位移與容器位移差`0x24`勿混。
6. **氛圍、光照與資產（中）**：emboss最終raster混色、daynight row4及部分visibility語意仍未證實；不能把shader近似、聲區預覽或loader存在稱為原生等效。只追與地圖編輯／儲存相容性直接相關的缺口。
7. **工具狀態與安全（高）**：地圖交易、ZIP還原、AI安全備份、dirty／undo／redo、兩份事件圖草稿、外部改檔、缺session、容量耗盡、取消、link/path guard，均須獨立故障／回歸測試。沙箱／檔案鎖失敗應另列，不能用靜態證據略過。
8. **驗收分層**：逐功能定向測試及 TEMP 往返、workspace check/test/build、ignored gate 分開記錄。GUI 點擊與遊戲內載入／存後再讀屬實機驗收，本輪未啟動遊戲。AI 服務、打包安裝與流程功能也需各自實測。

結論：已建立地圖編輯器、兩個存檔入口及工具箱的主要功能覆蓋表，並抽查可重用的本地反編譯證據。這是驗證工作清單與部分靜態交叉檢查，**不是「修改器所有功能已反編譯驗證完成」的結案報告**。
