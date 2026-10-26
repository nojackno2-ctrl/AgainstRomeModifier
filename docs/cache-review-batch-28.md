# 批次 #28 快取輸入審查（2026-10-26，Codex）

審查提交：b1cf474d3f01a995e81b6785fa5746272e8536e7。

## NavigationCacheKey

- 物件來源：loaded_map_path、placement.is_loaded、placement.loaded_map、history_token、物件數，以及每個 WorldObject 的位置、旋轉、類型、父物件等內容。原 key 遺漏類型名稱與載入來源；新增 type_names、placement_map、objects，避免重新載入或還原同 token 時沿用舊占地。
- 建築分類／近似占地／APT 選擇：placement.type_names；現在完整比對名稱表。
- 遊戲根目錄：troop_game_root 讀取 map_path_input 的祖先與 MAPS 是否存在，否則讀 maps.game_path_input 與 SYSTEM/clengine.ini 是否存在；key 比對解析後的 footprint_game，等價輸入無須重算。
- APT 解析器：apt.dat、SYSTEM/clengine.ini、SYSTEM/cl_apt.ini、DefaultPath 下 objdef.dau/d00..d09 與 typus.dau/d00..d09。加入長度與修改時間戳記；資產改變時重新載入解析器，包含先前載入失敗的結果。
- 尚未載入物件時的 DATA 是外部可變來源，沒有共同修訂號，開啟導航時每次重讀；關閉面板仍無重算，並清除舊路線覆蓋，避免背景物件／地形編輯留下舊路線。
- 路線診斷另讀 terrain.heights/collision、water_surface、seed/target、近似建築及 APT 模式；NavigationUi.refresh 比對完整地形與水位，既有端點／模式操作自行 invalidate。這些不屬於物件占地快取。
- 語言、戰役、劇本、聚落、自然地景、場景、材質、recovery、starter-kit、唯讀旗標不直接進入導航物件計算；套用到物件或地形後由上述內容／失效路徑涵蓋。
- 限制：資產戳記不是內容雜湊；外部工具若刻意保留相同長度與修改時間，無法偵測。資產修改與讀取間也可能有競爭；遊戲目錄本次僅唯讀。

## SaveBlockersCacheKey

舊 key 的 token 與 dirty 布林不足以代表下列輸入；尤其「已 dirty 後再編輯」不一定推進 token。基於正確性，每次 evaluate_save_blockers_cached 都先失效，再執行原 save_blockers；因此以下輸入一律重新讀取，無跨次結果重用。

- 待套用草稿：event_graph、scenario.graph_ui、maps、environment、landscape、packaging、campaign、ai_planner、toolbox 的 dirty／unsupported 狀態。
- 動作可用性：placement/scenario 是否載入；environment.generated_flora/generated_lairs/generated_lair_events/last_distribution；landscape 的 cliff/river/road/wall/erosion 計畫；ai_planner.current_plan/is_generating；campaign.objectives/last_wave_result；原始地圖唯讀判斷。
- 地形：session 存在與完整尺寸、高度、碰撞；emboss session 與其照明／基準；reset_auxiliary_layers；height_step、water_surface。
- 氛圍：dirty、完整 profile（含預覽欄位、boden）、global_sound_id/global_volume/zones、briefing 與本地化資料及儲存基準。
- 物件／自然：placement 與 nature 的 dirty、實際 store／編輯紀錄、載入目錄、重播候選與來源資料；聚落套用後產生的 DATA 由此涵蓋。
- 材質／場景：完整 session、dirty、載入來源、交易檔案與目標。
- 劇本／生成／戰役：scenario 文件、graph dirty、known_aliases、DATA 綁定與 spawn 候選、pending_starter_kit、camera_pending 與完整鏡頭候選。
- 路徑：map_path_input、loaded_map_path、canonicalize 結果、由目標推導的遊戲根目錄與 alias／objdef／APT 資源。
- 磁碟：儲存目錄與每個候選目標的存在／型別／權限、marker、連結／路徑安全檢查，以及 staging／diagnostics 讀取的 DATA、boden.ini、briefing、SDL、材質、arm_scenario.json、BCI、alias 與輔助 BMP。
- 語言：blocker 同時保存英中兩份文字，顯示當下讀 loc；不需以語言重算。recovery 不直接被 save_blockers 讀取；恢復後的實際子系統狀態仍重新驗證。

Safe Save 的 ensure_saveable_panel_state 直接呼叫 save_blockers；正式交易重新 staging／preflight／diagnostics。save_now_from_blockers 與 resolve_save_blocker 也直接重驗，均不信任 UI 儲存的結果。

## DesignCheckCacheKey

report_rows 只讀 report（Unavailable 訊息或 map_issues/preflight_error/design_issues）及 extra_issues；排序使用列本身。原 key 完整包含上述內容與 lang，無遺漏。搜尋、嚴重度篩選、選取、定位及格式化在快取外處理。地圖／磁碟／子系統修改需要重新執行設計檢查以產生新 report，這是明確的報告快照語意。

## 新增效能快取

檢查 placement、scene、family、textures；選擇 Family 的粒子定義列。原本每幀遍歷所有有效定義、建立列及複製名稱；現在依正規化 filter、include_inactive_definitions 快取，load_definitions 每次（成功、失敗或無目錄）均失效。definitions 是私有明確載入的快照，遊戲目錄改變仍沿用既有「請重新載入」提示。列不含語言文字，語言切換無須重算。計數器測試涵蓋命中、篩選、未啟用開關與重新載入。
## 驗證結果

- cargo check、cargo build：--workspace --locked --offline --target-dir re_workspace/codex-wave-dest-target，皆 exit 0。
- cargo test --workspace --locked --offline --no-fail-fast --target-dir re_workspace/codex-wave-dest-target：exit 0；287 組，2655 passed、0 failed、25 ignored。日誌：re_workspace/codex-batch-28-test.log。
- install_resource_discovery：12 passed、0 failed；需獨立列出的升權環境失敗為 0。
- 資產回歸測試曾誤把 apt.dat 放在合成 root，實際遊戲根目錄為 root/game；修正後單項及完整重驗均通過。
- 未驗證 GUI 視覺效果或實際每幀耗時；效能證據為不變輸入的重算計數器。