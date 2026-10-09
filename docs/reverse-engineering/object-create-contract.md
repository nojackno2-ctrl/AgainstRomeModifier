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

下一步追場景初始化／載入入口先後順序、counter的其他間接consumer與各範本建構caller，並建立產品新增前的容量／範本／跨池交易要求。實機物件新增、移除、空白地圖與存讀檔仍未由本輪驗證。
