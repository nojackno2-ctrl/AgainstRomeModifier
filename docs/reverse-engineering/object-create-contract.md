# 原生物件建立與 UID 契約

2026-10-09 Codex。唯讀分析 `Against_Rome.exe`，SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。REA Ghidra 12.1.4 反編譯及 Capstone 原指令交叉核對；未執行遊戲、未改原 EXE。本文件補充 [map-data-pools.md](map-data-pools.md)，不代表產品跨池功能已實作。

## 建立順序及失敗處理

`0x4aa780` 先呼叫 objects allocator `0x4ab450`；失敗時回傳 -1。成功即將 object active 置1、設定 UID／team／名稱，再依非 null 的範本參數建立各關聯池。Ghidra 的 param_1..param_6 是本次分析的參數位置標記，並非已完整確認的公開 API。

| 範本 | allocator／容量 | copy helper | 搜尋 cursor | object link |
| --- | --- | --- | --- | --- |
| objects 主槽 | 0x4ab450／14,000 | 主函式直接初始化 | 0x7718a0 | 回傳 object slot |
| param_1 position | 0x4ab4a0／33,000 | 0x4aae40 | 0x7718a4 | 通常配置兩筆；type 條件可能再配置第三筆 |
| param_2 anim | 0x4ab500／14,000 | 0x4aaea0 | 0x7718ac | runtime +0x12／serialized +71 |
| param_3 gfxtype | 0x4ab550／14,000 | 0x4aaf50 | 0x7718a8 | runtime +0x14／serialized +73 |
| param_4 action | 0x4ab5b0／14,000 | 0x4aaff0 | 0x7718b0 | runtime +0x18／serialized +77 |

allocator 從各自 cursor 向後尋找 validator 判定未使用的槽，抵達容量回傳 -1；此搜尋本身不回繞。釋放／初始化對 cursor 的維護另見 pool 文件。

上述 copy helpers 均先檢查 signed index≥0、index<容量、範本非 null；無效時回傳0且不寫入。原生建立函式在 anim `0x4aa9b7`、gfxtype `0x4aaa11`、action `0x4aaa6c` 呼叫 copy 後，不使用其回傳值作整體失敗／回滾判斷，而繼續更新 cursor 與 object link。分配失敗的 -1 會以 `0xffff` link 保存；null 範本同樣保留初始 -1。

**觀察：** 此建立函式未見針對這些關聯池失敗的全體 rollback，不是所有 pool 成功才啟用 objects。**限制：** 不代表所有 caller 皆會允許容量耗盡、不代表必然崩潰；caller preflight、後續 consumer 的容錯及完整副作用仍需追查。position 可配置第三筆，不能將產品目前的兩個位置一概視為完整契約。

copy 使用的是 runtime 結構布局（position stride20、anim32、gfxtype18、action28），與 serialized record＋尾端陣列不同；不能直接將磁碟整筆資料當 runtime 範本指標。現有產品 `LevelObjectTemplate` 也尚未持有這三個關聯池範本。

## UID 原生生成

建立函式 `0x4aa7e7` 呼叫 `0x4ac4f0(team)`，生成 UID 後寫 object runtime +4。`0x499240` 讀全域 `0x771c3c`；`0x499220` 寫回它。令讀到的計數器為 C，原生行為是：

1. 全域計數器寫為 C+1（32-bit 運算）。
2. UID 低24 bits 為 C & 0x00ffffff。
3. signed team≥8 時夾為8；對正常 team 0..8，UID 為 `(team << 24) + (C & 0x00ffffff)`。

原始指令 `0x4ac4ff and ebx,0xffffff`、`0x4ac51d shl eax,0x18`、`0x4ac520 add eax,ebx` 確認此公式。不是從 active objects 掃描最大 UID，也不是對每個 team 各自取最大值。負 team 行為及24-bit wrap的碰撞處理未確認。

`LevelObjectStore.Add` 現為最大 active UID+1，且 team override 只改 team 欄位。這與原生生成公式不同；還不能直接修改產品：尚缺全域計數器在 DATA／其他存檔中的序列化位置、各 UID consumers 是否依高位解碼，以及 team 變更後是否重寫 UID 的證據。現有 scenario persistent IDs 仍需保留，不能用新的 native UID 取代編輯器身分。

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

**推論與限制：** 現有 `LevelObjectStore.Save` 只寫objects／objdata／position，沒有更新engine counter；與自訂測試圖差異相符，但不能僅由現在快照證明所有歷史修改來源。候選重複UID也不是實機bug證明：事件查詢使用slot＋UID配對，caller可能在生成前處理counter，不同team前綴亦影響生成值。新增策略必須把engine counter納入研究與交易設計，不能只靠最大active UID＋1，也不能直接重寫既有UID。

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
