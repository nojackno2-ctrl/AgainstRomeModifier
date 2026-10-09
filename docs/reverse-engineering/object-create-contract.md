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

REA ledger `re_workspace/create-contract-20261009-evidence.json`（18 records）已匯出、session 關閉。create evidence `ev_b3c1598a525224776467ee3cd83f4fd14dbb5eb105cf90bed75eb0842702253c`；UID generator `ev_f7a122e9f25dc7de645839a26ff5c1109aa82c75f4671f545e02609a4cd29e05`；counter getter／setter `ev_c9f6e164a33df59fb9c25291109d4f7a817a415cfc5be567e6104ebee7d4699f`／`ev_4364baf7a67d616297ed4c0167945b93800de8341509f498b928bffd238dd433`。

`create-contract-instructions-20261009.txt` 保存514條原指令，涵蓋 create 前段、完整 UID generator 及 copy guards；不是 create 全函式原始指令清單。主函式完整失敗分支依上述 REA ledger，後續可針對 caller 取得更強證據。

下一步追 `0x771c3c` 初始化／存讀檔 consumers、各範本建構 caller，並建立產品新增前的容量／範本／跨池交易要求。實機物件新增、移除、空白地圖與存讀檔仍未由本輪驗證。
