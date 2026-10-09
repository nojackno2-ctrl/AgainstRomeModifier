# 地圖 action／anim 資料池

> 2026-10-09，static-verified。支援物件放置、資料一致性檢查與地圖載入診斷；尚未證明新建或清空地圖可在遊戲載入。

唯讀分析 EXE SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。以下為 PFIL 解壓後的 little-endian 樣本布局，欄位用途尚未命名。

## action.dat

writer `0x487f70`、reader `0x48c630`。header 三個 u32：version、slot count N、words per record。writer 為 version 1、N=14,000、words=6。每筆 u8 + 6×u32，serialized offsets 為 0、1、5、9、13、17、21；runtime stride `0x1c`，byte 在 offset 0、words 從 offset 4 起。無尾端陣列；總長 `12 + 25N`，原版為 350,012 bytes。

原生 reader 拒絕 version 大於 1，沒有可見的 N 上限或 words=6 檢查。probe 的 version=1、N≤14,000、words=6 是較嚴格的分析防護。

## anim.dat

writer `0x487f70`、reader `0x48c250`。header 兩個 u32：version、N。接著 N 筆 21-byte records，再接兩個獨立 N×u16 平行陣列。

| serialized record offset | 編碼 | runtime offset（stride 0x20） |
| --- | --- | --- |
| 0 | u8 | 0 |
| 1 | u32 | 4 |
| 5 | u16 | 8 |
| 7 | u32 | 0x0c |
| 11 | u16 | 0x10 |
| 13 | u16 | 0x12 |
| 15 | u16 | 0x14 |
| 17 | u16 | 0x16 |
| 19 | u16 | 0x18 |

兩個尾端陣列分別寫入各 runtime record offset `0x1a`、`0x1c`。總長 `8 + 21N + 2N + 2N = 8 + 25N`，N=14,000 時為 350,008 bytes。不能把尾端陣列當成每筆連續的 25-byte record。原生 version／容量檢查同樣需要與 probe 防護區分。

## 載入流程及功能缺口

`CL_LoadLevelData`（`0x48e960`）依序 nested open：

`light → gametime → rain → hagel → snow → flash → objects → position → anim → gfxtype → action → objdata → hirarchy → formatio → lager → engine → fow → fowreq → way → particle → explos → hitex → stat → biglager`。

某檔 open 失敗會跳過後續 nested open，記錄 `err_LoadLevelData`。reader error 雖被保存，沒有立即阻止下一檔 open。呼叫端 `0x487180` 之後仍進入 CHECK，診斷應記錄第一個缺檔及 reader 結果，不能只檢查 objects.dat。

CHECK `0x4acfe0`／`0x4ad110`／`0x4adf40` 走固定 14,000 objects；`0x4adf40` 取 packed object link 的 high16，經 slot 驗證取得 anim record 並修改 runtime offset 4。完整欄位語意及浮點公式尚未確認。下一步應追初始化、slot／UID、anim／action／hirarchy cross-reference，再做受控物件新增／清空測試。靜態流程尚未證明空白地圖卡死根因。

## 驗證與限制

### objects → anim 關聯與清空（2026-10-09 續查）

`0x4ab830` 以 signed index 0..13,999 加上 anim runtime offset 0 的 byte 非零判定 slot 有效；`0x4ab7b0` 對 objects 做相同範圍／active 檢查。這補足首 byte 的原生用途，不代表 byte 僅允許 0/1。

objects reader `0x48bae0` 將 serialized record offset **71** 的 u16 寫入 runtime object offset **0x12**。object base `0xa14bfc`、stride `0x4c`；CHECK `0x4adf6c` 讀 `[base + slot*0x4c + 0x10]` 的 dword，再 `sar 16`，取得 offset 0x12 的 signed anim index，呼叫 `0x4ab830`。`0xffff` 對應 -1／無索引。

native create `0x4aa780` 在 anim template 非 null 時呼叫 allocator `0x4ab500`、copy `0x4aaea0`，最後 `0x4aa9f1` 將 anim index 寫到 `0xa14c0e + slot*0x4c`。allocator 從 `0x7718ac` 的搜尋起點往後找第一個無效 slot，耗盡回傳 -1；不可假定 anim index 等於 object index。

native object release `0x4abb80` 經 `0x4abd0d` 呼叫 anim reset `0x4abf30`，並將 object anim link 置為 `0xffff`，另外處理 position 及其他 pools／runtime 系統。anim reset 清除各有效欄位，但 runtime offset **0x1c = 1**，不是整筆全零；同時將 allocation search cursor 往較小的已釋放 slot 更新。`0x4aba40` 對全部 14,000 anim slots 呼叫 reset，由 pool initialization `0x487910` 使用；此處尚未追到所有入口的初始化呼叫順序。

**修正過的嘗試：** 初版新 probe 錯將 objects 第 10 個平行欄位當成 anim link，產生 1,295 越界／8,603 inactive 統計。原始指令及 reader／create／release 三條路徑證明該欄位是 runtime offset 0x10，anim link 應是其高半部 offset 0x12／serialized offset 71。初版 manifest `object-anim-links-20261009.json` 保留作失敗紀錄，不能作結論；正式工具已修正。現有 `LevelObjectStore.IsLinked` 的 column 10 + objdata segment 2 是既有可編輯性 heuristic，不能單憑名稱將其解釋為 anim link；本輪未修改產品行為。

`probe_object_anim_links.py` 唯讀掃描 74 對 objects／anim：0 parse errors、0 越界、0 shared slots、0 unreferenced active anim slots；**ENDL_005 有 33 inactive targets，其餘 73 張為 0**。抽看前三筆為 type 717／team 8，不據此宣稱全部同型或根因。ENDL_005 是既有自訂測試目錄，差異尚未歸因；不能把原生不一致觀測當成原版格式錯誤。有效性關聯僅涵蓋這一個欄位，不驗證 UID、其他 pools 或實機載入。

正式 manifest `re_workspace/object-anim-links-corrected-20261009.json`；Capstone 原始指令 `object-anim-link-instructions-20261009.txt`；REA ledger `object-anim-links-20261009-evidence.json`。關鍵 evidence：validator `ev_7b3bcbd623ecc9309026a7aa8dcf001c60789087fd7c4fe9f2b21fdae2e7b669`、reader `ev_9d7d9efc7c46bae1d7dffbf42f732263ecc3ab24c05f488c9bc8e8f3a94306ce`、create `ev_fe134aa9ec9c7d8bd78038a5b76f8a405f2f833f961b98e342b5d823e03b56a0`、reset `ev_c6c8722858b2ad3f3415ce0e761b37a58be86208ed9ca69f09e8344815892c1b`。

`python tools/re/probe_map_pools.py "C:\Program Files (x86)\Against Rome" re_workspace/map-pools-20261009.json`

74 個 action + 74 個 anim 完整解壓到宣告長度，148 檔 header／精確長度全通過，0 errors；所有 N=14,000。ENDL_000 兩池首 byte 同有 7,382 個 0、6,618 個 1；分布相符不證明首 byte 用途或物件關聯。

probe 不驗證缺檔完整性、PFIL 壓縮尾端或 wrapper integrity，不執行遊戲、不輸出解壓素材、不推論 runtime 成功。完整 REA ledger：忽略的 `re_workspace/map-pools-20261009-evidence.json`，22 records。action evidence `ev_2e8eaaf34e6d59f1c369401d7e64f807ceaade42af0aca5105e1154a8b06cad8`；anim `ev_f95a69a9f4d12c94e3990d017f1e7403e742ac7e5d37f44f86b2abb939cd6ca7`；loader `ev_f1c5bd225c1a22202fcb1100a098423990fb08ae60b31e7aa44c73ff3cdee6ed`。
