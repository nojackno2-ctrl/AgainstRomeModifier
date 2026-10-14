# 原生物件建立與 UID 契約

2026-10-09 Codex。唯讀分析 `Against_Rome.exe`，SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。REA Ghidra 12.1.4 反編譯及 Capstone 原指令交叉核對；未執行遊戲、未改原 EXE。本文件補充 [map-data-pools.md](map-data-pools.md)，目前已實作部分磁碟跨池功能；完成範圍與限制見下方產品接線。

## 編輯器產品接線（2026-10-09 Codex）

`LevelObjectStore` 的地景/預建物件儲存已改為 anim、gfxtype、action、objdata **各池獨立搜尋空槽**，objects +71/+73/+77 及 column1 寫入真正取得的索引。Templates、OwnedTemplate 與可編輯性檢查依 column1 讀 objdata；Remove 在清除 object 前擷取 links 並釋放真正目標，遇到他物件共用的目標則拒絕移除，不破壞另一物件。

容量及範本尺寸在首次寫入前檢查；任一需要配置的已載入關聯池耗盡即回傳 -1 且不改任何池。這是編輯器自己的保護契約，比低層 native create 的部分成功行為嚴格。保留原 PFIL 包裝與 FileRollbackScope 儲存交易；不模擬 runtime 刪除佇列或 allocation cursor。

非同號及四池耗盡故障注入6項在原實作全FAIL，修正後通過；另覆蓋 shared target 與不合法範本。舊 fixture 的 inactive objdata 曾被隨機pattern誤標active，現只修正其 active 首byte。此外用 ARM_GAME_PATH 將原版 ENDL_000 的8池唯讀複製至TEMP，add/save/reload/remove後原生診斷均通過、active計數恢復，8個安裝原檔SHA256不變。預建物件與地景儲存端會將移除遭拒視為失敗，避免留下重複物件。這些證據驗證磁碟寫回與隔離，不證明遊戲內效果。

本輪以安裝 EXE 重驗 SHA256，Capstone 在 create 0x4aa780（0x420-byte視窗）和release 0x4abb80（0x230-byte視窗）確認四個allocator/reset direct calls，manifest `re_workspace/independent-pool-recheck-20261009.json`。UID計數器與已捕獲的第三position接線見下節；null範本與缺壞目標池的接線見下節；type初始化、缺失來源範本重建與runtime效果仍未完成，不宣稱整個native create已重建。

### 可選範本與缺壞資料池（2026-10-09）

create先將anim/gfxtype/action局部索引設為-1；null分支分別位於0x4aa9a4、0x4aa9fe、0x4aaa59，跳過配置與copy，仍將低16bits FFFF寫入object link。`probe_optional_creation_contract.py`對指定SHA256的完整create反組譯做19項原指令斷言；不執行遊戲。

產品現在對null serialized範本保留FFFF，不配置該池、不製造active零資料，也不以object同號槽替代。未使用的池即使滿載也不阻擋新增；非null範本若缺有效目標池，則在首次寫入前報錯。已存在但讀取或格式失敗的anim/gfxtype/action/engine檔會保留錯誤狀態，拒絕新增、移除及空白重設；Save不覆寫該壞檔。engine version1必須完整94bytes，截短檔在「地圖檢查」顯示警告。已載入關聯池的越界或inactive來源link不能被捕獲成null範本。

8種null組合、缺失/損壞目標、截短engine與失效來源均有回歸；原版兩/三位置TEMP副本驗收與原檔hash檢查通過。表單儲存遇壞engine會回滾並保留pending與Undo，修復檔案後重試只新增一次。

限制：legacy不完整地圖仍可載入檢視；缺失來源池的範本無法完整重建，未證明每個type允許缺anim/gfx/action。這是磁碟儲存與診斷驗證，尚未證明遊戲內顯示與行為。

### 第三位置、可編輯性與原生欄位（2026-10-09 續查）

原生create在非null position範本下，連續呼叫獨立allocator取得兩個位置；type validator成功且definition `0xc64922 + type*0x2a4` 的u16非零時，再呼叫一次。已確認此欄來自objdef **第60欄 afstr**：parser入口0x4b1480按欄號跳轉table0x4b1130，case60→0x4b21bc，0x4b21e4寫入此u16。0x4b1130是資料表，不是函式；Ghidra未辨識parser入口，因此欄位寫入依原始Capstone指令核對。三筆皆由同一position範本copy。primary/secondary/third分別為object runtime **+0xc/+0xe/+0x10**。磁碟前兩筆是record +67/+69，第三筆是**zero-based平行column10的u16**，缺省為FFFF；它不是其他物件的連結。

objects reader第11個array read在0x48bf3a將目的地加0x10、0x48bf3f呼叫u16-array reader；release0x4abcd7/0x4abce9/0x4abcfb分別呼叫position reset。原先文件釋放鏈的+8/+a/+c由錯位換算而來，已修正為+c/+e/+10；其他關聯亦必須把讀dword後sar16取出的高半部計入。

唯讀74圖的三位置audit：14,263筆第三links，types312..318，0越界、0inactive、0共用。ENDL_000有400筆。這是資料快照，不能據此將所有未知type硬編為2/3位置，也不能為第三位置賦予未追查的用途。

產品`LevelObjectTemplate`捕獲可選Position2；Add先預檢2/3筆獨立空槽，寫新的column10，三筆座標按新增位置更新，Remove釋放三筆，分配/移除均保護所有active object的三種引用。取消偶數相鄰位置對要求，使零散空槽可用。第三資料失效或範本不一致時拒絕複製。2026-10-09後續接線：儲存時重讀設定的DefaultPath下主objdef與numbered overlays，afstr非零但serialized範本缺第三位置時，依native create以Position0作為同一初始化範本，先預檢三槽再配置；已有第三範本則保留。afstr零卻帶第三範本、inactive或未知type會在首次寫入前拒絕。沒有definition來源時維持既有範本契約，不宣稱完整type初始化。

原版TEMP驗收初次找不到三位置範本，追到舊`IsLinked`把objdata segment2任何非零都視為關聯；400筆此值為0x3eb33333（float約.35）。xrefs→0x4bb5d0與原指令0x4bb661 **FLD [objdata+0x14]**確認它用於浮點計算，而非backlink，因此已刪除該heuristic。具體數值語意仍未命名。產品保護columns2/3/4/5/8/11：reader映射+1c/+1e/+28/+20/+2c/+22，release涉及其他池及群組清理；尚未實作其完整跨池修改，所以帶這些references的物件仍不可直接複製/移除。第三位置與float非零本身不再使物件不可編輯。

新增5項位置故障回歸在舊實作全FAIL；另測scalar、六個受保護欄、第三容量及範本一致性。舊fixture的真實reference欄原填隨機pattern，修為FFFF/明示reference，保留其他bytes驗證；偶數pair斷言依原生獨立配置契約修正。原版ENDL_000兩/三位置TEMP副本add/save/reload/remove、diagnostics及8原檔SHA256驗收通過。原圖無上述references，舊測試First(Linked)假設作廢。這些是磁碟驗收，沒有啟動遊戲。

`tools/re/probe_object_position_links.py <game-root> <new-output.json> --exe <Against_Rome.exe>`可重跑唯讀74圖audit與指紋限定的完整create/reader/release/float-consumer原指令核對。REA `third-position-20261009-evidence.json`（3records）及`objdata-scalar-20261009-evidence.json`（6records）已匯出/session關閉；create ev_ec5e6dc6f5c419e432c9d7d0a0ee802826036727b5acd1aec41ad24c75694997，reader ev_82b018d5450e9e8ba615243eaee46ba37587c1d1f290f9406eeb69b7bbaf00e9，float consumer ev_6cb0710ac28ca408607a19596db6c98b7086660131f24291ebadabb8f4737b5d。

### afstr 條件與第三位置 consumer（2026-10-09）

目前原版objdef有24個active definitions的afstr非零，包含311、312..318及其他特效，不能硬編成312..318。唯讀74圖392,344個active objects：14,263個afstr非零且有third、378,081個afstr零且無third，0不一致。第三位置資料存在性與type需求是不同檢查；產品NativePoolDiagnostics已加入必要第三位置缺失、額外第三位置與inactive/unknown type診斷，宿主「地圖檢查」仍標示saved DATA警告，不含pending編輯。objdef格式錯誤顯示定義檔警告，其他池檢查仍可執行；儲存不會忽略此錯誤。

REA確認consumer鏈4b6f30在afstr非零時呼叫4bc1f0；該函式讀primary runtime+c及third runtime+10，經4bd3a0更新secondary runtime+e。其中含地形取樣與插值，不能只憑afstr名稱將其定義為完整動畫或移動旗標。產品只接建立時的位置需求，沒有重建這段runtime計算。

`tools/re/probe_type_position_contract.py <game-root> <new-output.json> --exe <Against_Rome.exe>`驗證EXE指紋、parser case與原指令斷言（原階段14項，來源分析後擴充至35項），並掃描原圖需求關係；輸出不得位於來源/遊戲目錄或覆寫既有檔。REA8records保存在type-third-position-20261009-evidence.json，session已關閉。xrefs Evidence ev_6c863b356d2fd93eeb74d057d4525f0818a04167b25a410ccb3fce2b7a6fcc24；其他完整Evidence見bundle。

限制：NativeObjectDefinitions目前讀取clengine.ini [DefaultPath]所選loose directory，先套objdef.dau再套objdef.d00..d09；缺clengine時沿用舊MP DEFAULTS fallback。虛擬目錄policy、其他archive backend及遊戲當下來源仍未完整重建。TEMP原生兩/三位置roundtrip、來源hash不變與表單診斷是磁碟/產品證據，不能代替遊戲內驗收。

### 定義檔來源與覆蓋順序（2026-10-09 續查）

REA原生480850：先4afdc0初始化2500筆type，再4b4200("objdef.dau")，最後counter0..9依0x5f3359格式字串載入**objdef.d00..objdef.d09**。原字串為`objdef.d%02ld`；初次把反編譯symbol中的底線誤讀成檔名，正式probe字串斷言失敗後已修正，先前錯誤檔名的綠測試不作最終證據。4afec2將record+64整個dword清零，所以+66 afstr初值0；4aff39將active初值清零。

47e850讀SYSTEM/clengine.ini [DefaultPath]寫770de8。4b4200以%s%s連接此prefix及filename（不自動插入分隔符，產品亦保留literal prefix），572f50讀檔，再由42a230從[ObjectDefaults]逐欄呼叫4b1480。同type後載入的欄值覆蓋先前值，未出現欄保留；不是每個overlay都重置整個type。產品儲存與地圖檢查共用這個合併結果，接受缺失optional layers，已存在且無效的相關欄位不會被忽略。主檔缺失但overlay存在時也能套入初始化狀態。絕對／超出game root的DefaultPath目前明示不支援；未宣稱native具有相同限制。

安裝clengine指定SYSTEM\DATA_MP\DEFAULTS\。目前loose source與可讀ZIP inventory只有主objdef；不代表所有PUA/virtual backends都已排除。REA `object-definition-source-20261009-evidence.json`（12records）保存路徑、loader、generic parser及callers證據。擴充probe_type_position_contract.py核對原始格式、順序、loop bound、初始化與原指令，輸出effective definitions和來源hash。

## 建立順序及失敗處理

`0x4aa780` 先呼叫 objects allocator `0x4ab450`；失敗時回傳 -1。成功即將 object active 置1、設定 UID／team／名稱，再依非 null 的範本參數建立各關聯池。Ghidra 的 param_1..param_6 是本次分析的參數位置標記，並非已完整確認的公開 API。

| 範本 | allocator／容量 | copy helper | 搜尋 cursor | object link |
| --- | --- | --- | --- | --- |
| objects 主槽 | 0x4ab450／14,000 | 主函式直接初始化 | 0x7718a0 | 回傳 object slot |
| param_1 position | 0x4ab4a0／33,000 | 0x4aae40 | 0x7718a4 | 通常配置兩筆；type 條件可能再配置第三筆 |
| param_2 anim | 0x4ab500／14,000 | 0x4aaea0 | 0x7718ac | runtime +0x12／serialized +71 |
| param_3 gfxtype | 0x4ab550／14,000 | 0x4aaf50 | 0x7718a8 | runtime +0x14／serialized +73 |
| param_4 action | 0x4ab5b0／14,000 | 0x4aaff0 | 0x7718b0 | runtime +0x18／serialized +77 |
| param_5 objdata | 0x4ab600／14,000 | 0x4ab060 | 0x7718b4 | runtime +0x1a／平行欄位1 |

allocator 從各自 cursor 向後尋找 validator 判定未使用的槽，抵達容量回傳 -1；此搜尋本身不回繞。釋放／初始化對 cursor 的維護另見 pool 文件。

上述 copy helpers 均先檢查 signed index≥0、index<容量、範本非 null；無效時回傳0且不寫入。原生建立函式在 anim `0x4aa9b7`、gfxtype `0x4aaa11`、action `0x4aaa6c` 呼叫 copy 後，不使用其回傳值作整體失敗／回滾判斷，而繼續更新 cursor 與 object link。分配失敗的 -1 會以 `0xffff` link 保存；null 範本同樣保留初始 -1。

**觀察：** 此建立函式未見針對這些關聯池失敗的全體 rollback，不是所有 pool 成功才啟用 objects。**限制：** 不代表所有 caller 皆會允許容量耗盡、不代表必然崩潰；caller preflight、後續 consumer 的容錯及完整副作用仍需追查。position可配置第三筆，產品現依同型範本捕獲並保存，尚不代表全部caller與type初始化已重建。

copy 使用的是 runtime 結構布局（position stride20、anim32、gfxtype18、action28），與 serialized record＋尾端陣列不同；不能直接將磁碟整筆資料當 runtime 範本指標。產品 `LevelObjectTemplate` 現已持有三個關聯池的 serialized 範本；仍不能將其直接傳成 runtime 範本指標。

objdata runtime stride144、磁碟每slot123bytes，並有型別依賴初值與copy／reader處理不同欄位的限制；最新獨立分配、column1關聯及74圖檢查見 [objdata-contract.md](objdata-contract.md)。

## UID 原生生成

建立函式 `0x4aa7e7` 呼叫 `0x4ac4f0(team)`，生成 UID 後寫 object runtime +4。`0x499240` 讀全域 `0x771c3c`；`0x499220` 寫回它。令讀到的計數器為 C，原生行為是：

1. 全域計數器寫為 C+1（32-bit 運算）。
2. UID 低24 bits 為 C & 0x00ffffff。
3. signed team≥8 時夾為8；對正常 team 0..8，UID 為 `(team << 24) + (C & 0x00ffffff)`。

原始指令 `0x4ac4ff and ebx,0xffffff`、`0x4ac51d shl eax,0x18`、`0x4ac520 add eax,ebx` 確認此公式。不是從 active objects 掃描最大 UID，也不是對每個 team 各自取最大值。負 team 行為及24-bit wrap的碰撞處理未確認。

2026-10-09 Codex 產品修正：`LevelObjectStore.Add` 現使用持續的 next UID，初值取 `max(engine.dat counter, 最大 active UID 低24 bits + 1)`；成功新增後才將 counter 寫為 UID+1，Remove不回退。選擇上面已證明的 team0-create→team setter 路徑，因此新 DATA UID 高 byte 為0，team override仍只改team欄；這不是要求既有所有UID都使用此前綴。

此前 max-active+1 在刪除最新物件後會重用slot/UID，使舊RemoveIfUid可能誤刪替換物件；故障注入已重現同store及存檔重開兩種情境。現在engine與objects仍由同一FileRollbackScope寫回，既有UID及scenario GUID不改。若counter落後active資料，僅在下一次成功新增時前移，不改寫已有物件。

當next counter大於0x00ffffff時，編輯器在首次池寫入前拒絕新增，避免套用尚未確認的native wrap碰撞行為；最後可用UID0x00ffffff仍可配置一次。這是產品保護政策，並非native已有此拒絕分支。缺engine的舊/不完整fixture只保證同store高水位，刪除後重開無法恢復歷史，不能宣稱永久防重用。

可重跑唯讀 probe：`python tools/re/probe_uid_generation_contract.py <Against_Rome.exe> <new-output.json>`。指紋限定既有SHA256，保存generator/getter/setter/reader/writer/caller六段原指令；caller從完整函式入口解碼。初次探索reader中段0x48d8c3視窗有錯位，正式工具改0x48d8d1已確認指令邊界；不引用錯位反組譯。manifest `re_workspace/uid-generation-product-20261009.json`。只核對局部靜態路徑，完整存檔生命週期與遊戲內效果仍待驗證。

## 既有 DATA 的反例

唯讀掃描74張地圖：共392,344個 active objects，**所有 UID 高 byte 都是0**，各圖內0重複 UID；將建立時 team 前綴公式機械套到現有資料，會得到377,781筆不相等（74圖都有）。例如 ENDL_000 的6,618物件 team=8，UID高 byte仍為0。這不是損毀診斷；建立時 team／後續改隊伍／編輯器與原生地圖生成路徑可能不同，未由本輪證明原因。不能把原生建立公式升格為磁碟資料通用 invariant，或自動重寫現有 UID。

scan 對 version1、N≤14,000、name widths30/30及objects精確長度作防護；UID讀serialized+3、team讀+1。74份 decoded SHA256 與前輪 anim links manifest全部一致，避免用不同快照混合推論。manifest `re_workspace/uid-contract-20261009.json`、唯讀脚本 `probe-uid-contract.py` 保留在忽略的分析目錄。只涵蓋各地圖當下active物件的精確UID重複，不證明跨存檔、歷史或全域唯一性。

## 證據與後續

### UID counter 的 engine.dat 保存／還原（2026-10-09 續查）

writer `0x487f70` 的 engine.dat 區段在 `0x489d1a` 讀全域 `0x771c3c`，`0x489d22` 呼叫 u32 writer；loader `0x48e960` 傳 base `0x7717e8` 給 reader `0x48d6e0`。reader `0x48d8d4` 將讀到的 u32 寫入 `[base+0x454]`，恰為 `0x771c3c`。這是 indexed write，單查全域 xrefs 會漏掉 reader；不能據 direct xrefs 宣稱沒有還原路徑。

| 解碼後 offset | 長度 | writer version1 欄位 |
| --- | --- | --- |
| 0 | 4 | version=1 |
| 4 | 28 | 七個4-byte engine scalars（最後一個經zoom setter載入） |
| 32 | 2 | 對應 runtime0x771c28 的 u16 |
| **34（0x22）** | **4** | **UID counter0x771c3c** |
| 38 | 16 | 八個u16，runtime0x771828起 |
| 54 | 36 | 九個u32，runtime0x771804起 |
| 90 | 4 | runtime0x771838欄位 |

總長94bytes；版本及整數使用目前檔案的 little endian。整數 writer helper `0x580cc0`／`0x580c70` 可依全域 endian flag 選擇編碼，因此此 probe 是現有 writer version1 artifacts 的驗證，不是任意平台的通用格式。reader 第二個 scalar 讀取後將 runtime +4 強制為0，不能將整檔誤認成可直接 memcpy 的 runtime 狀態。

初始化函式 `0x480850` 在 `0x480aac push 0`／`0x480aae call0x499220` 清 counter；getter／setter直接xrefs分別只有UID generator及這個初始化呼叫。但完整場景初始化、載入入口先後順序、其他存檔路徑及間接引用仍需追查，不代表每次讀檔都先清零。

唯讀掃描74份 engine.dat：全部version1／94bytes；73張 counter 高於目前active UID低24bits最大值。ENDL_000為20224、最大active UID20222；ENDL_005的engine原bytes與ENDL_000相同（source SHA256相同），但objects最大UID已20255。ENDL_005的next team0候選UID為20224，現有slot1491／team8／type717已占用此UID。其餘73張沒有這個next team0精確UID重複候選。

**推論與限制：** 當時 `LevelObjectStore.Save` 只寫objects／objdata／position，沒有更新engine counter（現已修正；此段保留當時快照推論）；與自訂測試圖差異相符，但不能僅由現在快照證明所有歷史修改來源。候選重複UID也不是實機bug證明：事件查詢使用slot＋UID配對，caller可能在生成前處理counter，不同team前綴亦影響生成值。新增策略必須把engine counter納入研究與交易設計，不能只靠最大active UID＋1，也不能直接重寫既有UID。

manifest `re_workspace/engine-uid-20261009.json`；74份objects decoded hashes與前輪anim manifest全同。REA ledger `engine-uid-20261009-evidence.json`（17records），已匯出/session關閉。writer evidence `ev_079078b34efa67e5a7b76a78d7795f98e911075d237f0d713c68c3813c1ee7bb`，reader `ev_51dd48b3a17a5d14c1f322613ed42499dc3ecaaee3eb3b1387f67b34ee52d353`，init `ev_a256b35e2696ccd5e8b36238e301239cec50fe5c98fcb3e8e6cdd35dc7e1c791`。

Capstone `engine-uid-instructions-corrected-20261009.txt` 保存423行（含region標記）。初版從writer指令中段0x489c63起讀，已改從完整函式入口解碼再篩選；舊檔保留為失敗嘗試，不能引用其錯誤首條指令。以上關鍵writer／reader／init位址已用修正後原指令核對。

REA ledger `re_workspace/create-contract-20261009-evidence.json`（18 records）已匯出、session 關閉。create evidence `ev_b3c1598a525224776467ee3cd83f4fd14dbb5eb105cf90bed75eb0842702253c`；UID generator `ev_f7a122e9f25dc7de645839a26ff5c1109aa82c75f4671f545e02609a4cd29e05`；counter getter／setter `ev_c9f6e164a33df59fb9c25291109d4f7a817a415cfc5be567e6104ebee7d4699f`／`ev_4364baf7a67d616297ed4c0167945b93800de8341509f498b928bffd238dd433`。

`create-contract-instructions-20261009.txt` 保存514條原指令，涵蓋 create 前段、完整 UID generator 及 copy guards；不是 create 全函式原始指令清單。主函式完整失敗分支依上述 REA ledger，後續可針對 caller 取得更強證據。

## 上層建立 caller 與隊伍變更（2026-10-09 續查）

Ghidra 對底層 `0x4aa780` 找到一個直接 xref：`0x4a9c84`，所在函式 `0x4a9740`。這是已分析的直接引用結果，不排除間接 caller。上層 `0x4a9740` 另有12個直接 xrefs，尚未逐一定位遊戲操作入口。

上層順序：座標轉換→`0x4a9d50` preflight→建立 stack 上的 runtime 範本→呼叫底層create→若回傳slot≥0，呼叫 `0x4f6260`、team setter `0x4bd970` 及其他後處理。反編譯的 param_4 是type候選、param_5是傳入隊伍候選；完整公開 ABI 仍需由上層caller確認，不能只據Ghidra的參數名稱建立API。

### team 0 建立，再變更隊伍

`0x4a9c41 push0` 對應底層create第10個參數（前輪確認為team）；`0x4a9c84 call0x4aa780`，之後 `0x4a9cad call0x4bd970(slot,param_5)`。team setter先經object validator `0x4ab7b0`，有效時只寫 runtime object +2（指令 `0x4bd9a6`／base0xa14bfe），不寫 +4 UID。

因此這条原生路徑的UID生成高byte是0，即使完成後物件team為8也不變。這為74地圖觀測提供相容的原生解釋，但未證明全部歷史DATA都由此路徑生成。**不能把「UID高byte必須等於目前team」當新增後驗證規則。** 前輪磁碟反例不再只有未知假說；目前已確認一個team先後不同的具體路徑。

### preflight 與範本邊界

`0x4a9d50` 先以 `0x4afd80(type)` 驗證signed type0..2499且type table base0xc648bc、stride0x2a4首byte非零，再進行座標／type欄位與其他helper條件，最後有條件檢查 `0x4db770()` 是否回傳負值。這不是已確認的anim/gfxtype/action容量交易預檢；其helpers的完整語意與間接保護仍需追查，不能據此斷言引擎毫無容量防護。

caller建構的position、anim、gfxtype、action及另外兩組資料是stack上的runtime布局，並非直接複製serialized磁碟record。gfxtype建立範本已知active=1、runtime+4/+a=ffff、尾端+0x10=99，但+0xc=0；前輪**inactive reset**的+0xc=0100。anim建立範本的+0x1c=0，inactive reset則為1。這些差異證明「把reset預設值改active=1」不能當作通用建立範本。action只能確認部分顯式初始化欄位；未把未解釋的stack資料臆測為全零。

REA ledger `re_workspace/create-caller-20261009-evidence.json`（6records）已匯出/session關閉。caller evidence `ev_30a50d042942c4f41c7ac8ad4cd915a6585c6d78bf40731d87103b2876379c4f`；team setter `ev_c55bc20a46bd8fc439efaf650de721ab2112c89a411a9b08cf54c6a9198ef425`；preflight `ev_e7c55c01f5b69b4425ab924698da9b766c6ba8d199642cef2178f91811e2fb54`。Capstone `create-caller-instructions-complete-setter-20261009.txt` 553行（含region標記）核對push0／call／team write；初版setter視窗未覆蓋最末寫入，擴為64bytes後確認，舊短視窗不作完整setter證明。

## Particle preflight 與更高層失敗清理（2026-10-09 續查）

### Light component 批次操作（2026-10-10）

`0x49f480` 首空槽掃描上限1024；`0x49f500` 呼叫reset後初始化位置、色彩、两份radius、flicker type／parameter、counter=0、special fx與definition；owner／parent起始為-1。`0x49f5f0` 上層先以`0x4d2f50`驗definition範圍0..255及active。reset `0x49f700` 對有效owner清objects第12平行欄回連，parent清理由`0x4c9590`掃描。布局詳見[資料池](map-data-pools.md#lightdat-原生欄位2026-10-10)。

`NativeLightPool`是完整batch預檢後才修改的光源元件模型，容量不足返回null、不留下部分配置；回收會清欄位並返回需要清objects回連的owner。`NativeLightDefinitions`已解析原生初值、DefaultPath及dau/d00..d09覆蓋，definition.Create驗active並提供建立參數，預覽亦共用此loader。

2026-10-10 後續：`LevelObjectStore`已接直接光源Load／template capture／Add／Remove／Save；新光源配置獨立槽，objects第12欄与light owner改為新索引，同一FileRollbackScope寫回。官方palette在提供兩組definitions時以目前lidef及active lightdef初始化，座標為原生create傳入的position XYZ；不另外猜造lihei初始化。OwnedTemplate複製保存16欄（包含counter與第二radius）並以原物件位置的相對偏移移動，這是產品保留狀態語意，不宣稱原生copy ABI等價。缺失回連、共用光源、parent light未實作完整投影的物件仍保護；inactive物件槽若被light引用不重用。

最初10交易case被Windows Smart App Control阻擋AgainstRome.Shared.dll（0x800711C7／CodeIntegrity3077），當時0 passed。方向公式接線後重新build，新的DLL可載入，全部15直接光源case通過（含原10交易case與5新增方向/definition案例）；沒有修改系統policy。APT附屬光源／附屬物件整組建立、複製、刪除及實機驗收仍待完成。

### Direct light 執行時座標更新（2026-10-10）

**觀察：** `0x49f660` 驗證光源槽後只寫 runtime XYZ（`0x7f9ef4/8/c`，stride `0x44`）。其兩個直接 caller 位於 `0x49f810` 光源更新與 `0x4dbe60` particle 更新；後者以 particle 子項座標更新光源，不可套成 object owner 的規則。

`0x49f810` 遍歷1024槽，增加counter、處理flicker／失效回收；有效owner則呼叫 `0x4bd320` 取物件 position XYZ、`0x4bd5f0` 取同一position的第四欄。兩者讀 `a14c06 + slot*4c` 的dword再sar16，取得 runtime **+c** primary position index（舊文+6是錯誤換算）；再讀 position stride20的 `+0/+4/+8` 與方向 `+12`。有效type時，Y加 type表 `0xc64930 + type*0x2a4` 的 signed16值，X/Z加入方向偏移，最後呼叫setter。Ghidra未恢復的浮點順序已以完整Capstone指令與symbolic x87 stack補證。

**公式：** `theta = (rotationDegrees - 90 - lightang) * float32(1/360) * 2 * float32(pi)`；`X += sin(theta)*lightrad`、`Y += lihei`、`Z += cos(theta)*lightrad`。callback欄67/70/71寫type+74/+78/+7c，初值皆0；lihei寫低16再由update signed讀取。`lightrad`在此是光源位置的水平偏移距離，並非lightdef的照明半徑。sin/cos wrapper分別以FSIN/FCOS實作。

**產品：** 共用definition已解析三欄並提供DirectLightPosition。OwnedTemplate在有active type定義時用新位置/方向重新計算，保留其餘光源狀態；沒有定義時保留舊relative fallback。Palette先用create傳入XYZ建立，尚未模擬首次update。計算使用double中間值與原float常數，但不是bit-exact x87/FPU重建，仍待轉向、移動及首次遊戲update驗收。BlankMapContent已移除舊13個4bytes arrays的light通用reset，統一使用NativeLightPool的16 mixed-width平行欄，避免覆蓋正確reset；滿光源池亦可清理，不依赖空槽範本。

`probe_direct_light_transform.py` fingerprint gate、16指令斷言、三欄callback table、四個float常數及完整x87 XYZ symbolic stack核對通過，來源前後bytes相同；manifest `re_workspace/direct-light-transform-final-20261010.json`。還核對position reader第四float直接寫runtime+10，74原圖方向常見90/180/270，原檔使用度數。Editor API/renderer使用弧度，LevelObjectStore現於Objects讀取轉弧度、Add預檢後轉度數寫入兩/三positions並供光源計算；舊程式直接寫弧度有單位錯誤。15交易測試TRX在 `re_workspace/direct-light-transform-transactions-20261010`；另見本輪完整回歸結果，測試不代表遊戲內驗收。

REA ledger `re_workspace/direct-light-runtime-update-20261010-evidence.json`（11 records）；setter evidence `ev_9ad47571adefd39666483037b46d3655d7c1c78c1885478481d02dea6276d7ac`，caller evidence `ev_26c28f656bdd51ebf55677694f2cb4933ec10cbdb46db6fd62d60a6e4fd53b17`。來源EXE SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，靜態反編譯證據，未執行遊戲。

方向存取修正後完整回歸：`dotnet build AgainstRomeModifier.slnx -c Release --no-restore` 成功0errors；`dotnet test AgainstRomeModifier.slnx -c Release --no-build --no-restore` 為1790passed／30skipped／0failed（Modules822/2、Host968/28），TRX `re_workspace/direct-light-transform-verified-20261010`。包含Blank滿1024光源池raw/PFIL清理与回復、15直接光源交易及轉向/存檔度數/回讀弧度案例；未執行遊戲，不能以此證明完整family或原生FPU效果。

### Particle 證據

`0x4db770` 從slot0起掃0..1023，使用validator `0x4db7c0`，全部已占用則回傳-1。validator檢查 `0x12355d4 + slot*0x8e0` 的u16非零；與particle writer／reader的runtime布局相符。因此前輪preflight最後的capacity候選已確認是**1,024槽particle pool**，不是14,000槽anim/gfxtype/action，也不是全局objects數量。

preflight只在三個type所引用的definition validators皆成功時要求particle空槽：`0x4dde40`、`0x4de540`、`0x4de9e0` 分別檢查signed index0..511與definition首dword非零。三組表base/stride為0x80efcc/0x58、0x819fcc/0x58、0x824fcc/0x3c。完整definition欄位命名未確認。`0x4db960` 再次搜尋同一particle空槽，成功後初始化runtime entry、寫owner object及type設定；最多64個子項，與已有[投射物證據](projectile-ballistics.md)相容。preflight搜尋不占用slot，不是建立時的保留交易。

`0x4a9430` 是進一步的建立包裝：先preflight，再呼叫 `0x4a9740`（內部亦preflight），成功後以object slot／UID呼叫 `0x4a9590`。對它的direct xrefs為0x4307c2及0x50ed5a；前者位於0x430750，反編譯部分參數未恢復，不宣稱已完整辨識UI操作。

0x50ed5a位於 `0x50ecb0`，它檢查team0..8（大於7夾8）、非空script/type條件及依mode取得的資源計數，再經0x4a9430建立物件。之後仍有script／狀態／隊伍相關後處理；當0x518b80失敗或後處理回傳錯誤bits，會呼叫 **0x50f0d0(slot,0)** 並回傳-1。刪除dispatcher又依物件種類選0x50f170或0x524830；已核對0x50f170會做多組清理並呼叫0x4acb00，且可能回傳失敗。因此確認的是「存在補償性清理路徑」，不是所有錯誤都能完整rollback、也不是磁碟交易。

`s_createObj`既有註冊handler0x5192e0的原指令0x51934e呼叫0x50eaf0；該helper解析alias後呼叫0x50ecb0，再以0x518db0輸出身分。handler本次Ghidra pseudocode未成功返回函式本文，保留此限制，未將原指令片段宣稱為完整handler ABI證明。另一條0x50ef10路徑也呼叫0x50ecb0，涉及type/team/資源限制，仍待對應完整功能入口。

**修正結論範圍：** 底層0x4aa780缺少關聯池失敗時的整體rollback仍成立，但不能推出所有腳本建立入口都沒有失敗清理。更高層另有資源、位置、particle檢查與補償刪除。產品DATA新增若要接近原生結果，仍需獨立確認objdata／action初值及磁碟跨池寫入契約，不可把高層script helper與磁碟pool操作視為等價。

REA ledger `re_workspace/create-preflight-20261009-evidence.json`（14records）已匯出/session關閉。allocator evidence `ev_572f0de883c2fff9154679522839875a1b6f56442e09d080d6574ba8f0fa6657`、validator `ev_d192702e7f4f92c95660b6f9c621e9d014697791cb6059ab8edea316cb5efa7d`、上層建立 `ev_c0714319d6a6e3dd22a147129b1d3ae5ae93a5f13a7757fcead2a9aeaa7429e0`、清理dispatcher `ev_f61dfa980ad91a1b549b76bd118a1f30222fc9a5021d1d6a3ed6eb2a69632dc3`。Capstone `create-preflight-instructions-20261009.txt` 569行（含region標記）核對0x400上限、particle state位址、兩個create caller及清理call；各region只代表明示視窗，不是所有函式完整反組譯。

## 刪除佇列與 action 範本限制（2026-10-09 續查）

`0x4acb00(slot)` 先驗證active object，視其他條件可能記錄相關操作，再搜尋count0x1e53450／array0x1e53454。尚未入列則追加slot並增加count、回傳1；重複slot回傳-2；無效物件回傳-1。**此函式不立即呼叫pool release**，不能把它的成功回傳當成物件已不存在或槽位已可重用。

`0x4acab0` 處理佇列：逐slot呼叫 `0x4abb80`，將已處理項設為-1，最後count設0。已分析直接callers為0x47ae0a（函式0x47add0）、0x48798d（pool initialization0x487910）、0x4c9a52（函式0x4c9a20）。其中0x47add0在計時相關呼叫間處理佇列；其更高層frame入口／觸發週期本輪未確認，不能說固定每tick即時清理。另一0x4c9a20依mode條件才處理佇列。

因此0x50f170→0x4acb00這條補償路徑包含**排程刪除**，前輪「存在補償清理」仍成立，但不能表示每次create失敗返回前已釋放全部關聯槽。磁碟編輯器需要自己的原子交易，不能模仿排入引擎執行期佇列就當儲存成功。

另外，caller原指令的stack核對以0x4a9744 prologue後的ESP為frame基準，`0x4a9c64 lea` 傳入action範本frame+0x120。找到四個直接寫入：0x4a99b4將+0x120的byte設1；0x4a9929／30／37將+0x124/+0x128/+0x12c三個u32設0。這對應runtime action active及前三個words。copy0x4aaff0會讀六個words；對後三個words（+0x130/+0x134/+0x138）本次未找到直接ESP-relative初始化。

此audit只涵蓋成功create路徑的直接stack stores，不模擬helper別名寫入、完整CFG或執行，因此**未證明後三words必為未初始化，也不能假定全零**。初版linear audit把早退epilogue混入ESP追蹤而assert失敗；排除後又因選到param_5的lea而失敗，核對推參順序改為param_4 lea0x4a9c64後assert frame+0x120通過。兩次失敗是分析腳本假設錯誤，不是遊戲缺陷。

0x4f6260在本EXE反編譯為直接return0，不能只凭其在create後的呼叫位置就假定它初始化objdata。真正的objdata初值仍待追查；action words的consumer見下節。

證據：`re_workspace/delete-queue-20261009-evidence.json`（9records）已匯出/session關閉；queue evidence `ev_b5f583f93e767c33b5330a435f159241b44c064af01cf1c3a8fd128d7f1184bf`、drain `ev_652da68ee91d61875ad031ada911b2861c5f639a051608f1bbe4ee4fafaa229e`。`delete-queue-instructions-20261009.txt` 148行（含region標記）核對enqueue／release／drain caller。`action-stack-writes-20261009.json`保存四個直接stores、基準及明示限制；原完整caller指令見前輪檔案。

此階段後續問題為action後三words的helper寫入／初值、objdata實際初始化及佇列上層觸發順序，再建立產品新增前的容量／範本／跨池交易要求；consumer已於下節補上。實機物件新增、移除、空白地圖與存讀檔仍未由本輪驗證。

## Native post-create 與 parent 關聯（2026-10-10 Codex）

### 隊伍 marker 類型來源（2026-10-10 續查）

`771804` 是九個 signed32 類型，磁碟對應完整94-byte version1 `engine.dat` payload offset54。writer `489d49..489d5f` 寫九個值，reader `48d8fe..48d905` 讀九個值到 runtime+1c。不能從 BSS 位址假造 file-backed 初值。

載圖 `4870e0` 在 CHECK/refresh 前，以 `46a9c0(team,0)` 選 team0..7 的候選，送入 setter `4c9d40`。候選來源是 team runtime `73ca88` 的部族與 `73ca8c` 的旗幟版本：部族 stride3c4、版本 stride60、obdef entry stride4。`469cd0` 初始化六部族×十版本×二十個 object aliases 為-1，再 `469ab0` 讀 `SYSTEM/banner.ini` 的 `[volkNN_obdef_bverNN]`；entry0 用於 marker。alias 透過 `444200`／`430060` 的 objdef name lookup 解析，比較器 `4441d0` 使用 `5675f0(...,0)`（不分 ASCII 大小寫）。`team.dat` `[teamdata]` col1 是 GER/KEL/HUN/ROM→0/1/2/3（未知字串回0），col5 是旗幟版本；`468a10` 將兩欄初始設0。

**setter 不會清空無效候選。** `4c9d40` 先檢查 team0..8 與 active object type；type=-1、未啟用或越界時完全不寫。載圖最後的 `(team8,-1)` 同樣是 no-op。已載入的 saved type 因此須保留，不能無條件將 team8 變成-1，也不能將沒有alias的隊伍重設為空。

共用程式庫新增 `NativeMarkerTypeTable`，唯讀載入 banner/team/engine 與目前合併後的 `NativeObjectDefinitions.Name`，公開候選、保存值及套用上述setter後的有效值，供後續完整family建立流程使用。缺檔會明示失敗；重複設定區段、實際用到的重名alias及無效旗幟版本由產品拒絕，這是產品驗證政策，不宣稱模擬原生所有scanner/VFS或不安全索引行為。其他無關重名不拒絕（原版箭矢891/1272即重名）。此模型尚未接入建立／刪除整組附屬物件的交易，也不代表遊戲內驗收。

唯讀 probe `tools/re/probe_native_banner_types.py` 核27條原指令及字串/jump table；74圖中71圖有team.dat，568隊伍530筆saved吻合／38筆不同，3圖缺檔分開報告，所有team8 saved=0。有效候選在載圖時覆寫saved值，因此這38筆不是損壞判定。產品解析亦對71圖重現530/38及來源SHA不變。manifest `re_workspace/native-banner-types-20261010.json`，REA28 records 匯出 `native-marker-types-20261010-evidence.json`；setter evidence `ev_0ae2f1bfe867d72d31b0d306c04bc5c25fdc1cd84d9e11a83dfa532a088677fd`、banner loader `ev_acc133a358e6eba2e37228e4fcd9e4757fa71e228fbd2de6e5f6bc52f790a34f`、selection `ev_4ebfac1ec284f7646f1492be68b27fd814b1f40653d6f14f2fb0538ed41d46be`。此為靜態程式證據與磁碟快照，尚未觀察runtime VFS或完整生命週期。

`4a9740` 成功後順序呼叫 `4c9590/4c9790/4c9db0` 清理舊附屬資料，再 `4c9610/4c9820/4c9e70` 建立附屬資料，最後 `4e1470(slot,0x23)` 為已有證據的 `SFX_ACTION_PERMANENT` selector，不能當成 action.dat 初始化。

- `aptli` col162 signedword 位於 type+248（c64b04）；validator4d2f50檢查256筆active表80afcc/stride64。4c9610透過4c82a0取最多20個APT座標，再4d2f50/49f5f0建立獨立1024槽、68byte效果。`aptlh` col163是高度偏移。效果parent寫7f9f26，與objects.dat parent不同；完整效果池交易未重建。
- `aptrau` col180 signedword 位於type+24c（c64b08）；4c9820驗證其為active object definition，從4c84e0取最多20個APT座標，再呼4a9740建立team8 children；`rauhei` col181是高度偏移。建立後4c99ba將parent slot写object+30。
- `4c9e70` 另依父物件team0..8從771804取child type，4c7cf0取APT frame+2c/+30 marker（-1000000表示缺）；同樣建立child並在4c9f9f寫parent。清理4c9790/4c9db0掃active children、讀a14c2a的signed high half（a14c2c=runtime+30），再呼4acb00。

reader48bae0依十八平行欄序列載入；第十四個array callback48bfc3對runtime+30，證明磁碟為zero-based **column13 u16 parent slot**，FFFF表示無parent。全74圖505 children引用388個parents，全部parent active，沒有越界、自身或inactive；這是disk關聯證據，不是runtime成功。

產品已納入parent雙向保護：children與有children的parents標成Linked，不能單獨複製/移除；cached或外部範本含parent時在首次寫入前拒絕。新增配置跳過被現存child引用的空parent槽，避免損壞資料意外接到新物件；blank-map reset同時清索引。地圖檢查報parent越界/自身/未啟用。這只是已知關聯保護，完整family clone/delete、APT投影與效果池仍待整合；不得稱完整附屬建立已完成。

### 整組刪除與原版資料依賴（2026-10-10 續查）

`LevelObjectStore.FamilyAt(root, uid)` 已建立root UID、完整子樹、物件/光源回連與外部共用資料預檢；`RemoveFamilyIfUid` 按反向子樹釋放已支援的物件各池與parent-only APT光源，重建parent/owner快取。單物件Remove仍保留Linked保護；不允許以子物件作root、缺失光源資料的多物件family、未知關聯或混合owner/parent。ScenarioLevelObjects只在預建root被刪除時接此操作；仍存在的family移動/複製尚未支援。這是編輯器儲存交易，不模仿原生刪除佇列或條件式type清理。

合成26定向case通過，包括多層子物件、parent-only/direct light、raw/PFIL檔案回復、外部position/objdata/light引用與UID/未知links拒絕；首次11新case因fixture填滿objects而不能reload，擴成5slots保留空範本後通過，沒有放寬產品規則。完整回歸Modules833passed/2skip、Host967passed/28skip/1fail；唯一失敗是SaveManager DLL的Windows載入政策0x800711C7。新增ARM_GAME_PATH原圖定向127passed/1fail：沒有任何原圖多物件family通過預檢，原圖TEMP刪除測試停在Assert.NotNull，**尚未驗證原圖整組刪除**。

不能把這個失敗當作測試資料問題。只讀74圖發現388parents／505children，其中313parents的object平行column5引用lager，414children的column11引用particle；其餘75個沒有受保護root欄位的parent也都有particle child。現有模型沒有這兩池的完整回收，原圖全被拒絕是實際功能缺口，需繼續重建兩池，而不是放寬保護。

原指令：reader column5寫runtime+20、column11寫+22；release讀a14c1a/a14c1c的signed高16，分別呼4ac3e0（lager base120fdd4）/4db800（particle base12355d4）。Particle reset不僅清active：runtime+8的有效owner會送4acb00入刪除佇列，並掃64個runtime+85c light索引呼49f700。此額外光源關係不能只靠light owner/parent判定，完整closure仍待實作。

`probe_family_dependency_frontier.py` fingerprint gate、14指令斷言、18callback序列及74圖來源前後bytes核對通過；manifest `re_workspace/family-dependency-frontier-20261010.json`。REA cleanup重核ledger `family-cleanup-recheck-20261010-evidence.json` 2records已匯出/session關閉。原圖交易測試在 `family-delete-original-20261010/family-original.trx`，合成交易在 `family-delete-final-directed-20261010/family-final-directed.trx`。尚未啟動遊戲或修改原圖。

後續粒子池補強（2026-10-10）：上述「原圖全部拒絕」是接入particle前的實際失敗。NativeParticlePool現在保留完整2252-byte records與四個尾陣列；FamilyAt納入object col11↔emitter owner及64子light索引，拒絕外部emitter共用family light、外部object backlink、owner不符及缺失pool。RemoveFamilyIfUid將相關粒子與額外light納入同一Save rollback；單物件仍受保護，col5 lager仍未支援。

再後續營地交易（2026-10-10）：col5 lager已接入FamilyAt預檢及RemoveFamilyIfUid。小型與有效biglager的駐軍成員都作為ownership邊，與parent子樹一起展開；重複到达同物件的不同路徑可保留，實際依賴cycle拒絕。此為離線整組操作的完整資料閉包，不聲稱等同原生runtime queue（原object release只掃10small slots；biglager reset清800索引但不自行queue所有成員）。有外部parent、其他camp/未刪biglager引用成員、外部object共用camp、共享biglager、count不符或失效索引皆先拒絕。單獨操作駐軍仍受保護，未知hirarchy/formation等關聯仍需另行重建。

Save包含lager/biglager與既有object/position/aux/light/particle同rollback；Blank滿池reset不依賴空範本，generic營地reset已移除。合成14新增case與原圖HIST_004含large camp交易通過（root318/UID44048，2objects/1lager/1biglager/4lights），另root1970營地與particle閉包通過。原圖只在TEMP副本寫入，11來源檔hash與未刪營地槽bytes均保留。完整family移動／複製／palette建立、場景投影與遊戲內驗收仍待完成。

原圖TEMP交易在`particle-original-20261010/particle-original.trx`曾通過（54定向passed/0failed），不代表74圖全部family可編輯。新增八個粒子family合成case全部通過，Modules完整841passed/2skip；本輪Host測試DLL被Windows policy0x800711C7擋在discovery，新七個NativeParticlePool專項未執行，不用舊TRX充作新版Host驗證。74圖靜態probe另確認1714emitter owner/backlink與172child-light有效，21原指令布局斷言通過，來源前後bytes未變。完整family建立／移動／複製、lager closure、APT投影與遊戲內驗收繼續待做。

REA retained evidence及probe `tools/re/probe_native_object_parents.py` 同時核對caller/reader/parent writes/cleanup/parser；HIST_004八池只在TEMP副本驗證拒刪與回存，原檔SHA256保持不變。

驗證：Release build2既有warnings/0errors；ARM_GAME_PATH定向146passed，完整Modules789/Host927（1716passed/24skip/0fail）。probe30指令/18callback順序/4parser case、unknownEXE及輸出guards、py_compile/diff check通過。完整附屬建立及實機行為仍未驗收。

## 整組移動所需的位置契約（2026-10-10 Codex）

REA `4bd3a0` 取 object runtime+e（secondary position），驗證 object／position 後只寫 XYZ；X/Z 分別夾在 32 與兩個 map dimension×64-32 之間，Y 原樣傳入，沒有更新 direction。無效 object／position 分別回傳 -1／-2。`4bc1f0` 以 primary／third position、地形取樣及插值呼叫這個 setter；反編譯對部分 caller 的參數數量推斷不完整，不能由 pseudocode 猜出新的移動 ABI。

particle reader `48dd90` 將子項前三個 dwords（serialized child+1/+5/+9）分別寫 runtime+5c/+15c/+25c，update `4dbe60` 以 65536 scale 將 owner 的 primary XYZ轉為定點來源；`4dc6ad/4dc6d1/4dc6f5` 對子項 Z/Y/X 做 signed SAR16，再 FILD 轉 float傳給 `49f660` 更新子光源。因此子項座標是 signed16.16，光源更新使用整數部分，負數也必須保留有號右移規則。更新包含速度、壽命、重生及地形處理，不能當成一個無副作用的物件平移 API。

`probe_family_movement_contract.py` 通過23原指令斷言與常數核對，唯讀掃74張圖：1714 active emitters、40707 saved active children，其中40333有非零低16bits、245至少一軸為負。EXE及每個particle來源前後bytes相同；這是所有saved active children的觀察，非runtime current-count迴圈驗收。unknown fingerprint／existing output拒絕與py_compile通過。manifest `re_workspace/family-movement-contract-20261010.json`；REA三函式證據保存於 `family-movement-20261010-evidence.json`，session已關閉。

產品 `NativeParticlePool.TranslateBatchFixed` 接受精確signed16.16 delta：整批槽位、active狀態、三軸checked加法及active child light index先驗，全部成功才寫XYZ，重複emitter只平移一次，inactive children／速度／其他狀態bytes保持。回傳各child所需的light XYZ，讓未來family transaction同步更新；不自行改light pool，不推進simulation，也未選定float→fixed量化規則。原生loop使用current count，本操作則平移所有saved active children，屬編輯器保存狀態政策。18個粒子模型case及原圖／診斷定向合計46passed/0failed；Release build0errors/2既有warnings。完整Family Move、轉向／APT投影、Scenario綁定、copy/create及遊戲內驗收仍待完成。

2026-10-10 粒子／光源同步後續：`NativeLightPool.SetPositionsBatch` 整批驗證槽位active、finite XYZ、重複槽位的相同目標，衝突時不修改任何light；成功只寫三個座標欄。`NativeParticlePool.TranslateBatchWithLightsFixed` 在detached particle candidate先計算平移，再用這個light API驗證／寫入，最後將固定長度candidate拷回particle。缺光源、不同座標共用光源及粒子overflow皆保持兩池原bytes；原pool的PFIL wrapper不由candidate取代。這是saved effect平移，仍需上層檢查外部owner／emitter共用，不自動移動object positions、APT attachments或native simulation。

新版66定向passed/0failed（ARM_GAME_PATH已設），含原圖兩檔TEMP交易：預期XYZ獨立手算，所有其餘bytes相同，故意IOException觸發共同rollback、commit後reload、PFIL opaque header與原來源bytes保留。原圖新增case曾因xUnitalways-throw lambda選错Func<Task>導致CS0619，修正Action後重新build並驗證；失敗後舊DLL65passed不算新增case證據。最後Release build0errors/2既有warnings、diff check通過；TRX `re_workspace/particle-light-sync-verified-20261010/particle-light-sync-verified.trx`。尚未接完整Family Move/UI，未執行遊戲。

2026-10-10 整組平移接線：`LevelObjectStore.TranslateFamilyIfUidFixed(root, uid, dx, dy, dz)` 以既有 `FamilyAt` 驗證閉包與外部共用，對主要／次要／第三position去重並平移XYZ，direction、object UID、所有pool索引與營地bytes原樣保留。particle以精確signed16.16平移active children；其light按SAR16重算，其餘family light按原保存位置加delta。所有候選position／particle／light成功後才固定長度copy回各原pool，PFIL header保持。這是offline saved-state整組平移，沒有使用secondary setter夾邊界，也不執行native地形插值或APT投影／simulation。轉向與完整copy/create仍待做，Scenario及專用UI尚未接。

8個raw/PFIL×parent/particle/small/large family案例驗證所有非座標bytes不變、內部shared third position只平移一次、direction123保留、UID與family成員一致、外部slots不變、存檔失敗回復／reload／反向平移。4個candidate late失敗（NaN位置、Infinity光源、particle overflow、共用子light不同座標）保持全檔，外部camp/member sharing預檢亦拒絕move。Modules65定向passed/0failed。ARM_GAME_PATH Host68定向passed/0failed，包含HIST_004 root318/UID44048（2objects/1camp/1big/4lights）、root1970/UID49270（3objects/1camp/2particles/1light）的11檔TEMP transaction與獨立完整payload預期、來源hash。最新Release build0errors/2既有warnings，diff check通過；TRX `family-move-candidates-20261010`／`family-move-original-20261010`。證據限選定原圖family，不是74圖全部family或game acceptance。

2026-10-10 場景儲存接線：`ScenarioLevelObjects.Apply` 保留同SpawnId／alias、root UID吻合、隊伍／方向符合實際root的獨立群組binding，以目前DATA root與目標座標差計算exact signed16.16 delta，呼叫整組平移；零位移跳過，避免單純save改寫既有粒子light量化狀態。重複／重疊binding、方向／隊伍變更、無法精確表達的位移與距離超界拒絕；完整float量化、旋轉、隊伍交易及copy/create仍待研究。缺官方建立範本不再使已綁定群組變成script工地。既有MapEditorForm.TrySaveMap透過此Apply寫回共用FileRollbackScope，沒有增加專用群組UI；整個Apply的記憶體修改仍需呼叫端在失敗時丟棄store，不能稱全Apply原子回復。

首版80定向passed/0failed，涵蓋raw/PFIL×parent/particle/small/big的binding保留、catalogue不被呼叫、重排、重複save與後續整組刪除，以及方向／隊伍／alias／精度／範圍／NaN／舊角度拒絕。完整solution回歸1903passed/27skip/0fail（Modules887、Host1016）；之後新增零位移active particle light保留、重複／child binding、actual-angle拒絕及原圖改走Scenario入口的cases，最新Modules84／Host214定向全部passed。Host含既有STA存檔回歸，兩組HIST_004仍驗11檔payload／rollback／原來源hash及binding相同；沒有實際群組拖曳或遊戲驗收。最後build0errors/9既有warnings、diff check通過；TRX scenario-family-regression-20261010、scenario-family-reviewed-20261010與scenario-family-original-final-20261010。

下一步量化證據：原生419c80取input sign bit與612c70常數fdffff3e（0.49999991059303284）組合，FSUB後FISTP；局部函式沒有設定FPU control word。原指令／常數及EXE前後bytes核對保存於re_workspace/float-fixed-rounding-frontier-20261010.json，尚未確認caller rounding policy，不能直接換成C#truncate/round並宣稱等價。

2026-10-10 FPU 契約補證：`probe_native_float_conversion.py` 核對25處原指令、20個startup callback records與70個條件計算。entry `5cd8c0` 經 `5ccce8`／`5d2180` 的priority初始化表呼叫 `5cabf0`，其中 `5cac06 FNINIT`；source guard初始為0。這支持啟動初始化的extended precision／nearest模式，但不證明後續執行緒、外部API或gameplay保持該設定。`5c5af2` 暫設control word高byte `1f` 做FRNDINT後恢復原值；已識別的generic setter `5d6050` caller `5d595d` 傳mask `9f`，只涉及例外遮罩，不涉及precision／rounding bits。

條件模型使用精確有理數，分別模擬FSUB的precision rounding與FISTP的integer rounding。float32 bits `3f7fffff`（0.9999999403953552）在control word `037f` 下結果1，在 `007f` 下結果0；因此此converter不能一概用C#truncate代替。這是條件模型的邊界反例，沒有觀察遊戲執行時的control word，也未模擬例外trap／溢位indefinite結果。signed zero保留input sign以選擇bias。

指令與條件結果：`re_workspace/native-float-conversion-20261010.json`；REA證據12 records：`re_workspace/native-float-conversion-20261010-evidence.json`。`py_compile`、四組獨立正負half/ties-even rounding向量、unknown fingerprint與existing-output拒絕通過。首次guard fixture將output放在fake source同目錄，先被來源目錄保護拒絕；改為獨立output後才驗到fingerprint拒絕。REA session已關閉。本輪未修改C#座標政策；下一步需取得圖形初始化與實際converter執行期FPU設定，才能定義完整float群組移動。



圖形初始化追查（2026-10-10 Codex）：直接解析已知EXE的PE import directory，7個descriptors／261 imports，沒有靜態D3D／DDraw imports；LoadLibraryA IAT=5d8804、GetProcAddress IAT=5d87d8。來源前後bytes一致，manifest `re_workspace/graphics-imports-20261010.json`。目前Python缺pefile，未安裝套件；改用struct與既有read_va解析PE32表。

`D3D8.DLL`字串603727的xref557a63位於557810（caller5577f0）。此函式動態探測DDraw、DInput、DirectMusic與DirectDrawCreateEx；成功後LoadLibrary D3D8並FreeLibrary，返回800；沒有在此路徑取得Direct3DCreate8或建立D3D8 device。故此字串只支持DLL相容性探測，不能推定實際renderer或套用D3D8 device預設FPU政策。實際DirectDrawCreate字串604614與其GetProcAddr/create failure路徑仍須追查，FPU runtime未驗證。REA7records保存 `re_workspace/graphics-startup-20261010-evidence.json`，session已關閉，未執行遊戲或修改C#。


DirectDraw實際建立追查（2026-10-10 Codex）：5601f0（caller55cd3a）動態LoadLibrary ddraw.dll並GetProcAddress DirectDrawCreate，儲存於29e9fc0。560290（caller5619e6）在5602d5呼叫該pointer，參數為0、output、0；成功後5602f3對物件QueryInterface，IID原bytes為9a50599cbd39d1118c4a00c04fd930c5。其後處理window/fullscreen cooperative level、display mode fallback與GetCaps，與前述557810相容性檢查是不同路徑。

新唯讀工具 `tools/re/probe_graphics_fpu_frontier.py` 確認7個指令斷言、159／2477bytes兩個已識別函式完整線性解碼（50／719條指令），沒有直接FLDCW／FNINIT／環境恢復指令。這只涵蓋兩個body，不涵蓋transitive callees或外部DLL，不能把「未見direct writer」當作API保留FPU的證明。manifest `re_workspace/graphics-fpu-frontier-20261010.json`，REA6records `re_workspace/directdraw-init-20261010-evidence.json`；source bytes前後一致、py_compile、unknown fingerprint／既存output拒絕與diff check通過，REA已關閉。完整float movement仍待converter實際control word觀測；沒有修改C#或執行遊戲。


x87 controlled replay（2026-10-10 Codex）：本輪Get-Process Against_Rome未發現執行中的遊戲。原生particle caller4dbf81..4dbfe4三軸先FLD位置／FMUL65536／FSTP float32至參數，再CALL419c80。新 `tools/re/replay_x87_conversion.py` 在獨立Windows x64 Python程序重建sign bias／FSUB／FISTP，逐組設定並恢復control word；70組硬體結果全部吻合有理數模型，包括PC24／64差異及masked signed32 overflow的indefinite值。這不是執行原遊戲EXE，也不是原x86 ABI或遊戲CW觀測；replay不保留x87 exception status，須以獨立程序使用。首次inline stub的FLDCW恢復缺少disp8造成錯結果，修正後補完整decode／restore operand檢查再驗證，不能把首次失敗當模型錯誤。工具py_compile通過；最終證據 `re_workspace/x87-controlled-replay-reviewed-20261010.json`。下一步仍需實際converter CW，或接續其他已可靜態證明的family copy/create契約；不把條件模型預設當遊戲模式。

## action 的 192-bit 狀態旗標（2026-10-09 續查）

原生 set `0x4bd780`、clear `0x4bd830`、query `0x4bd8d0` 以 object 的 action link 選取 pool slot，再依旗標編號 `n` 選取 word。合法範圍為 **0..191**，runtime 位址為 `0xf00454 + slot*0x1c + 4 + 4*(n >> 5)`，mask 為 `1 << (n & 31)`。因此六個 u32 是可索引的狀態旗標集合，後三個 words 也有消費端；對 `0xf00464/68/6c` 的 direct xrefs 為空不能證明它們未使用。

| word | 旗標編號 | serialized record offset | runtime offset |
|---|---|---|---|
| 0 | 0..31 | 1 | 0x04 |
| 1 | 32..63 | 5 | 0x08 |
| 2 | 64..95 | 9 | 0x0c |
| 3 | 96..127 | 13 | 0x10 |
| 4 | 128..159 | 17 | 0x14 |
| 5 | 160..191 | 21 | 0x18 |

set／clear 成功回傳1；無效 object、無效 action、旗標越界分別回傳-1、-2、-3。query 回傳1或0，但無效輸入也回傳0，故0不能單獨當成有效物件的旗標未設定。clear 使用 `word -= word & mask`，等價於清除此 bit。Capstone 核對 set 的 OR `0x4bd806`、clear 的 word write `0x4bd8b8`、query 的 TEST `0x4bd947` 及範圍／shift 指令。

已觀察兩組呼叫用途：`0x4bee90` 先清 flags `0x10..0x17`，mode0..7再設其中一個，mode-1全部清除；本輪不為各 mode 補上未證實名稱。`0x4bf0f0` 在 flags3或4存在時回傳-3，否則有效 anim link 才將 anim runtime+4 設0；死亡條件的既有證據見 [scenario-event-conditions.md](scenario-event-conditions.md)。

`0x4fa8f0` 從一個 packed input 的 offset `0x1f/0x23/0x27` 複製 **前三個 u32** 到 action，前後比較 flags `0x2b/0x2c/0x2d`，改變時呼叫 `0x4bf0f0`。本輪未辨識其完整 caller／輸入協定，不能稱為已證實網路封包，也不能推出後96 bits的保存或初始化規則。

唯讀掃描74張地圖的392,311個 active action slots，其中 **392,218個** 至少設定一個後96 bits；全部74張都有此現象。74個 decoded SHA256 與前輪 action link manifest 一致，安裝 EXE fingerprint亦一致。這排除「後三words可忽略／一律零」的假設，但既有資料非零不證明每一 bit 均具遊戲語意，亦未解決新建物件範本的初值来源。產品新增仍須追 helper alias writes、完整初始化與跨池交易。

證據：`re_workspace/action-flags-20261009-evidence.json`（16 records，session已關閉）；set `ev_8d26dc2abeecfbc8eab51f194286beb356aa759c2b7708f05430893f9eb0a278`、clear `ev_f843d154d16f15d2500fac5a39004cc438a848d9dca686e1283c05c84c714532`、query `ev_52088b1d0175657b8eadf0aee251c9153b656e494b3fb7a4a3eed6989c1488ab`、packed input `ev_eb106c25c994b8348c7167f135c566ecade7960f85defb3112f2c7e1dc1e1b18`。原指令 `action-flags-instructions-20261009.txt` 及分布 `action-flag-distribution-20261009.json` 保留於忽略的 research workspace。未修改或執行遊戲；本輪未改C#，未重跑.NET回歸，實機效果仍未驗證。

## action 範本堆疊寫入與釋放/重置鏈（2026-10-09 續查）

### 1. 建立時 action 範本的成因確認
在 `0x4a9740` 建立 stack 範本時：
- 入口 `sub esp, 0x178` 未進行全堆疊清零。
- 指令 `0x4a99b4` 寫入 active byte = 1。
- 指令 `0x4a9929`、`0x4a9930`、`0x4a9937` 僅將 word 0、word 1、word 2 寫為 0。
- 後三個 words（word 3..5，即 flags 96..191）**未被顯式初始化**，直接保留先前的堆疊調用殘餘資料。
- 複製器 `0x4aaff0` 依據 28 bytes 結構將 active byte 及全部 6 個 words（192 bits）完整拷貝至 action pool。
- 這完全解釋了為何 74 張官方地圖的 active 物件中，高達 392,218 筆在後 96 bits 具有非零值。

### 2. action 重置函式 0x4ac070 的行為
與建立時不同，原生 action 重置函式 `0x4ac070`：
- 將 active byte 設為 0（`0x4ac093`）。
- 透過迴圈將全部 6 個 words（`0x4ac0a3`）**完整清零**。
- 若釋放槽位小於分配游標 `0x7718b0`，則將游標回退至該槽位（`0x4ac0b7`）。

### 3. 物件釋放鏈 0x4abb80
`0x4abb80` 逐一釋放原生關聯池：
- `+0x0c` / `+0x0e` / `+0x10`：position 槽位，呼叫 `0x4abec0`。
- `+0x12`：anim 槽位，呼叫 `0x4abf30`。
- `+0x14`：gfxtype 槽位，呼叫 `0x4abfc0`。
- `+0x18`：action 槽位，呼叫 `0x4ac070`。
- `+0x1a`：objdata 槽位，呼叫 `0x4ac0c0`。
- `+0x22`：particle 槽位，呼叫 `0x4db800`。
- 釋放完成後將 object 中各 link 重設為 `0xffff`。

### 定義 overlay 的逗號 tokenizer（2026-10-09 Codex）

原指令 `0x567d50` 會跳過連續逗號；`0x42a2df` 呼叫欄位 callback 後，`0x42a2f5` 才遞增邏輯欄號。因此 `770,,,1` 的 activ 是 1，空欄不佔欄號，也不終止本列。空白字元不是逗號分隔符：`770, ,0` 的第二個 token 仍會交給 activ scanner。產品共同 definition loader 已按此規則讀取，Parse 測試也改用同一實作；對無效數字拒絕載入並由既有 UI 顯示警告，不模仿原生忽略 scanner 回傳值的未明結果。REA 三筆證據存於 `re_workspace/objdef-tokenizer-20261009-evidence.json`；這是靜態證據，不代表執行期 VFS policy 已重建。

### 新放置的 HP／第二數值初值（2026-10-09 Codex）

parser case19（0x4b18c1）及case21（0x4b1930）均用 `%ld`，分別將signed32寫到type+0x2c（c648e8，lpmax）與+0x34（c648f0，mpmax）；不是float definition。type initializer 4afe10將兩欄設0。caller4a9740的FILD signed32→float將兩值寫objdata runtime+4/+c，runtime+8/+10的比例均設1.0；磁碟為segment0 offset1/5及segment1 offset0/4。以有active definition的建立分支為依據，未模仿invalid-type fallback。

產品NativeObjectDefinitions合併欄19/21，partial overlay保留前層，首次出現且省略的欄為0，signed32越界／無效文字顯示定義錯誤。嚴格數字驗證是產品政策；不宣稱模擬原生scanner忽略回傳值及部分字串轉換的全部行為。LoadOfficialTemplates標記新放置範本；Add在存檔時用當下definition寫兩值與比例1，避免使用palette快取或官方地圖範本的舊值／舊比例。Templates／OwnedTemplate捕獲不帶新放置標記，因此既有物件移動、複製及還原仍保留所有原值；未修改範本byte arrays或其他objdata segments。無definition的legacy路徑仍保留原範本，不能宣稱其初值已對齊原生。

REA `object-vitals-init-20261009-evidence.json` 四筆保存caller pseudocode、assembly及type initializer；probe_type_position_contract.py另對原始parser/write/FILD/比例/初值指令及 `%ld` 字串作assert。定向84tests通過，包含palette建立後修改definition、signed32邊界/壞值、owned複製保留、官方來源byte不變及原生TEMP兩/三位置讀回四個float與8原檔hash不變。仍未重建所有type初值、action來源及runtime後處理，未驗證遊戲內效果。

### typus 條件 field1c（2026-10-10 Codex）

objdef col54 `%ld`→word c6497a（type+be），初值ffff。建立4a9740讀signed high half；typus必須在0..2499且表active非零，才檢查9f5031+index*52的byte mask04（低32bit flags **bit10/0x400**）。通過時runtime objdata+1c=100，否則0；磁碟對應segment4 u16。typus是表索引，不是Nature/Bau等分類，欄位完整遊戲用途尚未確認。

480ca5先初始化2500筆typus：4d7bd0清active與四組flags，再480dcc載typus.dau、480ff8在00..09迴圈載typus.d%02ld。4d7a20使用同DefaultPath literal prefix與[TypusDefaults]；callback4d7830 col1 `%ld`寫signed32 active，col3 `%x`寫低flags，未出現欄位沿用舊值。共同loader重建上述必要欄位並拒絕壞數值；未重建其餘flags消費機制或VFS來源。

官方palette新放置按目前definition寫segment4；OwnedTemplate複製/還原保留所有原bytes。原圖saved值可因後續處理不同於create初值，probe只列觀察，不把差異當作損壞。REA16筆證據保存於ignored re_workspace/typus-create-20261010-evidence.json；原指令與全圖唯讀probe為tools/re/probe_typus_creation_contract.py。遊戲內行為仍未驗收。

### 新放置的 moves／maxruhm 欄位（2026-10-09 Codex）

原始jump table確認欄4 case4b157a以`%f`寫type+0xc（c648c8），欄152 case4b35bb以`%ld`寫type+0x22c（c64ae8）；原版header分別為moves/maxruhm。caller4a9a25/4a9a2b與4a9af8/4a9afe將原32bits複製到objdata runtime+0x14/+0x4c，對應磁碟segment2/16。這裡沒有HP/Morale使用的整數→float轉換。type initializer4afe2b/4b0077均設0。不能僅依maxruhm名稱宣稱完整榮譽機制已重建。

REA4bb5d0沿群組成員的objects→objdata link讀+0x14，取最小值，負結果夾0後乘全域771c68；原指令4bb661 FLD、4bb66d比較分支、4bb68b清零及4bb693 FMUL已核對。尚未確認乘數完整來源與所有移動路徑，沒有執行遊戲。

共同definition loader已合併moves float32與maxruhm signed32；首次省略為0、後層省略保留、文字以InvariantCulture解析。非有限／float32溢位及無效signed32由產品拒絕，不模仿原生scanner的所有特殊輸入行為。官方palette的新放置按儲存時definition更新兩段，不使用快取舊值；OwnedTemplate與一般複製保留原數值及其他bytes。直接傳入NaN／Infinity的definition也會在首次池寫入前拒絕。沒有相關definition欄位的legacy範本仍保留原值，未宣稱完整重建其初值。

舊實作10/10定向回歸FAIL；修正後含API finite預檢的13項通過，ARM_GAME_PATH定向96項通過（新欄三語系、signed32邊界、壞值、palette後改definition、owned複製保留、官方來源不改、原生兩／三位置TEMP寫回及8原檔hash不變）。REA三筆匯出`object-moves-maxruhm-20261009-evidence.json`，session關閉。probe_type_position_contract.py另驗原parser、建立、初始化及consumer指令。其餘type/aux初值與受控實機驗收仍未完成。


### Particle建立雙路徑與copy契約（2026-10-10 Codex）

4db960先驗owner/type與三個definition，搜尋空particle slot、取得primary X/Z與地形高度，呼4db800 reset再写owner/runtime state。由definition 80efe0選路徑：非零分支先把children active設1、XYZ fixed設80010000/d8f00000/80010000（-32767/-10000/-32767），呼49f5f0建立同座標light並保存回傳索引；完成迴圈後4dbcc3把current count設0。零分支以owner X/Z、terrain Y與definition offsets建立children，亦建立light。兩分支的light call之後直接保存AX，局部迴圈沒有先驗回傳成功的branch；這不是全呼叫鏈rollback結論。

因此saved emitter複製必須重配置全部64個子項的保存light link，不能只用current count推定已配置資源，也不能把初始count0當作沒有children。現有NativeParticlePool.Capture掃全部64個light links符合此保存政策；尚無完整emitter snapshot／allocate/remap API，family copy仍未完成。definition命名與完整runtime simulation仍待研究。

`tools/re/probe_particle_creation_contract.py` 核22指令與完整4db960..4dbe43 body、65536／1/65536常數、source前後bytes一致；unknown fingerprint及existing-output拒絕通過。最終manifest `re_workspace/particle-creation-contract-reviewed-20261010.json`，REA2records `re_workspace/particle-creation-contract-20261010-evidence.json`，session已關閉。沒有執行遊戲或修改C#；此為靜態create契約，不是完整family copy驗證。

2026-10-10 snapshot產品進展：NativeParticlePool.CaptureSnapshot返回完整2252bytes record與四個分離state words的Detached NativeParticleSnapshot；RecordBytes／StateWords返回防禦性複本，LightSlots保留全部64個子項中的非-1索引，含inactive與current count之外的連結。最後槽、current count0／child63 light、source release後snapshot保持、修改返回陣列不污染snapshot新增測試通過。Release solution build0errors/31既有warnings；ARM_GAME_PATH NativeParticlePoolTests24passed/0fail/0skip，TRX particle-snapshot-20261010。此API只capture保存狀態，尚未allocate/remap或接family copy，不宣稱遊戲內驗證。

2026-10-10 particle copy配置進展：NativeParticlePool.AddCopies以整批候選先驗owner範圍、全64 child light完整有效mapping與單snapshot不同source不可合併target，再找足夠未active且未excluded slots，寫完整record及四state words。原source snapshot不變；不負責light pool allocation／實際owner存在／跨emitter與外部共享，caller須納同一family transaction。新增完整payload獨立預期、inactive child63／reserved slot、late mapping failure與capacity不變測試，但Host discovery被AgainstRome.Core.dll應用控制0x800711C7阻擋，未執行新cases；Release build0errors/31既有warnings僅證明可編譯。上一版24passed不能作此API驗證，full family copy仍未完成。

2026-10-10 批次copy review補強：AddCopies現在拒絕整批重複OwnerObject及不同new emitters共用target light；同一snapshot重複原light連結仍可保留同一映射。容量失敗fixture使用不同owner／target，避免先被新guard拒絕而失去容量覆蓋。新增兩項Theory案例驗拒絕前全pool bytes不變；Release build0errors/2既有warnings、diff check通過，但既有Core.dll應用控制discovery阻擋未有解除證據，這些新case尚未執行，未重跑同gate。

2026-10-10 營地snapshot進展：NativeLagerPool.CaptureSnapshot保留43bytes完整record及尾陣列的BigLager索引；NativeBigLagerPool.CaptureSnapshot保留1601bytes完整record與全部800成員位置，sentinel與順序不裁切。兩snapshot的RecordBytes返回複本，source release不改snapshot。四個既有raw/PFIL最後槽測試增加完整byte預期及immutability斷言（小營地未知欄非零）。Release build通過，但Host Core.dll政策gate未有解除證據，新斷言尚未執行；未接營地allocate/remap或完整family copy。

2026-10-10 Biglager配置進展：NativeBigLagerPool.AddCopies預檢整批800member arrays的完整object mapping、source/target範圍及local/batch成員不得重複，保留sentinel與成員位置；找足够未active且未excluded slots才一次寫record。caller須驗新object存在與外部dangling refs reserve，這不是整個family跨池交易。四個新增測試覆盖完整payload、首末成員與reserved槽、late missing map、跨camp共用及容量失败；Release build0errors/31既有warnings、diff check通過，Host政策gate未有解除證據，新tests尚未執行。

2026-10-10 小營地配置進展：NativeLagerPool.AddCopies預檢全部10個保存member的完整object mapping，以及独立BigLager link mapping／batch唯一，保留43record opaque欄與sentinel位置，再配置未active／未excluded槽並寫尾陣列。occupancy可用容量依有無big link為800或10；完整member/count一致、新target存在與外部共享仍須full family caller驗。兩新增完整payload／late missing big map測試尚未執行；Release build0errors/31既有warnings、diff check通過。唯讀CodeIntegrity查到原16:07:32 blocker，沒有解除證據，未重跑同gate。此為跨池copy基礎API，未接完整群組複製／UI／遊戲驗收。

2026-10-10 Family capture整合：LevelObjectStore.CaptureFamily先由FamilyAt證明supported closed family，再以原slot鍵保存完整LevelObjectTemplate及light/particle/lager/biglager snapshots，dictionary只讀。OwnedTemplate仍保留原UID與IsLinked gate，CaptureSavedTemplate共用原body；snapshot未分配新UID或重配置索引。Modules LevelObjectDirectLightTests84passed/0failed，含各pool key集合與stale UID capture拒絕，TRX family-capture-20261010；Release solution build0errors/31既有warnings。Host AddCopies新cases仍未執行，不能以此84項覆蓋完整配置；full family transaction與UI仍待完成。

2026-10-10 候選store交易基礎：LevelObjectStore.CloneForEditing深複製所有可編輯pool、headers及UID cursor，再依建構器重建ownership caches；definition lookup為共用唯讀輸入。尚無candidate整組copy/accept操作。8個既有family raw/PFIL case增加candidate removal不污染原objects/particle/camp的斷言，但Modules本轮84failed/0passed均因AgainstRome.Shared.dll應用控制0x800711C7而未進功能assertions；TRX family-candidate-20261010。Release build0errors/31既有warnings與diff check通過僅證明編譯，Clone行為未驗證，前版84passed不可代替。

2026-10-10 core allocation計畫：PlanFamilyCoreAllocation返回只讀object/UID/position/objdata source→target mapping，保留內部共用position和data；避開既有active refs與particle owner指向的inactive物件槽，預檢24bit UID range與容量。計畫不改store、不耗UID，尚未包括aux/effect/camp pools，不能當完整family複製。新增repeatability/unique target/stale UID案例尚未執行，前述Shared.dll policy gate未有解除證據。

2026-10-10 auxiliary plan：PlanFamilyCoreAllocation新增Anim/GfxType/Action mapping，group source link去重、active原record驗證，未active target仍若被active object引用即排除；缺池/壞link拒絕、容量不足null，與核心plan同樣不寫入。新增repeatability斷言未執行，Shared.dll政策gate未有解除證據，編譯通過不當功能驗證。完整effect/camp預留與候選copy尚待接線。

2026-10-10 dedicated allocation預檢：同一plan新增Light/Particle/Lager/BigLager source→target maps，排除active池槽及active objects、全部active emitter child light、active camps big link所引用的inactive槽；任一池容量不足返回null，所有映射只讀、不改資料。仍不是持久reservation，須在同候選transaction內重新核對/使用。新增repeatability與light集合/target獨立斷言未執行，既有Shared.dll policy gate未有解除證據。所有已支援pool配置現可一起規劃，但完整byte copy、UID/link remap、cache及UI尚未完成。


2026-10-11 原圖來源 API 補記：`ScenarioFamilyCopyRequest.IsNativeSource` 明確區分未綁定原圖來源，呼叫端須提供可信的 typeId→alias 解析器；來源 UID、獨立閉包、隊伍及方向均預檢，只替新副本 root 建立 Scenario 綁定。`ScenarioSpawn.RawRotation` 保存編輯器讀回的旋轉弧度快照，須為有限值、預建物件且對應整數顯示角；實際已保存角度 bytes 由候選複製與 retained 平移保留。即使單物件仍可取得 OwnedTemplate，含此 metadata 的重存也走 retained 流程，避免重建 UID、量化角度或改寫 opaque light 欄位。原生 API／失敗保護／既有群組專項187passed/0failed/0skipped，TRX `re_workspace/native-source-retained-contracts-20261011/`（16_07_37）；新原圖選取 UI 已接入，修復後來源／metadata／intent專項215passed（native-ui-repaired-modules，16_27_33）；完整Modules1022passed/2既有skip。表單已驗3D來源選取到Save rollback/retry/reload、fractional快照及stale/unknown/shared拒絕；Host完整回歸另發現DPI文字越界，換行修正仍待重驗。這些不代表遊戲執行期驗收。

2026-10-10 完整supported-family候選copy接線：CreateFamilyCopyCandidate先capture與plan，再CloneForEditing；依各source→target mappings複製core bytes、aux records／平行tails、重寫objects UID與position/objdata/parent/light/particle/camp索引。light保留原完整fields只重配owner/parent；particle與兩camp池按plan選定槽套用完整snapshot mappings。候選完成後重新建構ownership caches，驗證獨立family closure，再套用精確signed16.16位移。任一中途失敗只丟棄候選，原store不写；返回候選Store/new root/UID與plan，尚未接接受或UI，也不是native definition create/simulation。

新增8 raw/PFIL×parent/particle/small/big案例通過candidate閉包及各池count、source memory不變、新root位移、Save/reload兩family都有效與全檔rollback恢復。Modules完整LevelObjectDirectLightTests93passed/0failed，Host ARM_GAME_PATH particle/lager40passed/0failed，均terminal exit0；TRX family-copy-regression-20261010與family-pool-copy-20261010。前輪受policy阻擋的snapshot／AddCopies斷言現已在此版本執行，但不稱政策全局解除。Release product build0errors/31既有warnings、diff check通過。原圖完整family副本／獨立11payload預期／full solution／Scenario/UI與遊戲內驗收仍待做，未以這些合成case宣稱全部功能完成。
