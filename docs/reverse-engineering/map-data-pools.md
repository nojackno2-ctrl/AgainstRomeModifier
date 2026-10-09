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

## action 關聯與 hirarchy.dat（2026-10-09 續查）

action link 為 objects serialized record **+77 → runtime +0x18**。create `0x4aa780` 使用 allocator `0x4ab5b0`／copy `0x4aaff0`，指令 `0x4aaaa6` 寫入 `0xa14c14 + objectSlot*0x4c`。validator `0x4ab8a0` 檢查 0..13,999 且 action 首 byte 非零；release `0x4abd31` 呼叫 reset `0x4ac070`，清首 byte 及六個 u32、回退 `0x7718b0` allocation cursor。與 anim 不同，action 沒有尾端預設為 1 的欄位。

`hirarchy.dat` reader `0x48cf70`／writer `0x487f70` 的 version 1 格式：

| 位置 | 編碼 | 內容 |
| --- | --- | --- |
| 0、4、8 | 三個 u32 | version、group count N、member width M（writer 1／3,200／50） |
| 12 起，每筆 103 bytes | u8 + u16 + 50×u16 | active、member count、物件 slot 索引陣列 |
| records 之後 | N×u16 | 平行尾端欄位，語意未確認 |

runtime base `0x114c294`、stride `0x6a`：active +0，member count +2，50 個 slots 從 +4 起，尾端欄位 +0x68。writer 總長 `12 + 105N`，N=3,200 時為 336,012 bytes。reader 按 header M 讀取，未見 M≤50／N≤3,200 的明確防護；probe 採嚴格 writer 格式。

validator `0x4ab910` 判斷 0..3,199 且 active 非零；create `0x4ad820`／copy `0x4ab250` 有最多 50 members 的檢查，並將 objects runtime **+0x28** 置為 group slot。objects reader 將第 **4 個平行欄位（zero-based）** 寫入 +0x28；這是群組 backlink。remove `0x4ad9c0` 對有效物件 member 做移除及重排，空群組呼叫 reset `0x4ac260`；reset 會處理物件端關聯、清 active/count、50 個 member 與尾端欄位置 `0xffff`，並回退 cursor `0x7718b8`。此處只確認群組／物件索引的用途，未完整解釋所有 side effects。

新唯讀 `probe_object_pool_links.py` 掃 74 組 objects/action/hirarchy，0 parse errors；action 共 392,344 個有效範圍內物件 links，0 shared slots／0 unreferenced active slots。ENDL_005 同樣有 33 個 inactive action targets。hirarchy 合計 2,312 active groups，0 member/backlink 越界、0 inactive group/member、0雙向索引 mismatch；MP_016 的 group 21/22/23/28 在宣告的 20 members 內有重複索引，原始解碼 bytes 確認。重複可能有其他語意／歷史來源，不能僅此判為損毀或主張應自動去重。

manifest `re_workspace/object-pool-links-20261009.json`；REA ledger `action-hirarchy-20261009-evidence.json`（15 records）；原始指令 `action-link-instructions-20261009.txt`。hirarchy reader evidence `ev_1bcf3845e8b01697503033f51d8771d45debcbe1f131c0c66cbc91c5a55ef8d4`、group create `ev_0d18580a3f1973014c162c6cda0d923ca648f96d3ea89038001e31dcff929e48`、reset `ev_bde6287b82c3d8c4eb888b6c305f70747d411dd91e81949e37f1b7fa5d9acb3c`、action validator `ev_cf8939954fabcad6a850d37c9f190eceb24cd2e5b7fb165219d36fdd25f932e5`。

## gfxtype.dat 與物件關聯（2026-10-09 續查）

reader `0x48c460`、writer `0x487f70`：header 為 u32 version／slot count N，接 N 筆 **15-byte records（u8 + 七個 u16）**，再接一個獨立 N×u16 平行陣列。總長 `8 + 17N`；N=14,000 時 238,008 bytes。runtime base `0xc2705c`、stride `0x12`：serialized +0 寫 runtime +0，serialized +1/+3/+5/+7/+9/+11/+13 分別寫 +2/+4/+6/+8/+a/+c/+e；尾端陣列寫 +0x10。不能當成連續 17-byte records。reader 拒絕 version>1，未見容量上限；probe 的 version=1／N≤14,000 是分析防護。

objects serialized record **+73 → runtime +0x14** 是 gfxtype link。create `0x4aaa00` 呼叫 allocator `0x4ab550`，`0x4aaa11` 呼叫 copy `0x4aaf50`，`0x4aaa4f` 寫入 `0xa14c10 + objectSlot*0x4c`。validator `0x4ab860` 檢查 signed 0..13,999 且 active byte 非零；allocator 有獨立 cursor `0x7718a8`，不可假定 pool slot 等於 object slot。release `0x4abd12` 讀 object runtime +0x12 的 dword，再取高半部 +0x14；`0x4abd1f` 呼叫 reset `0x4abfc0`。

reset 清 active，但七個 runtime words +2/+4/+6/+8/+a/+c/+e 分別是 `0 / 0xffff / 0 / 0 / 0xffff / 0x0100 / 0`，尾端 +0x10 是 **99（decimal）**；同時回退 cursor 至較小的 released slot。因此原生清空不是整筆全零。各 word 的語意及所有 consumers 未確認，尚不能直接据此改產品的清空／新增策略。

`probe_map_pools.py` 擴充 gfxtype：74 張地圖共 **222 檔 action／anim／gfxtype，0 errors**。`probe_gfxtype_links.py` 掃 74 對 objects／gfxtype，0 parse errors／0 越界／0 shared／0 unreferenced active slots；唯 ENDL_005 有 33 inactive targets，**物件 slot 集合與 anim／action 的 33 筆完全相同**。74 份 objects decoded hashes 與前輪 anim manifest 一致。這只證明三個關聯欄位的同組差異，未證明歷史來源、UID 正確性或實機卡死原因。

synthetic valid layout、截斷／多餘尾端／version／容量拒絕、signed／sentinel／inactive／shared links、輸出覆寫及安裝路徑防護、py_compile 通過。manifest：`re_workspace/map-pools-gfx-20261009.json`、`gfxtype-links-20261009.json`。唯讀原指令：`gfxtype-link-instructions-20261009.txt`。REA ledger `gfxtype-20261009-evidence.json`（4 records）已匯出、session 關閉；reader evidence `ev_c26f4dde6bad999b2152e5cf5287d0ec727c9902081e375bbbfa1cec71cdbfed`，reset `ev_106337f4ba0163d5a3a3a6588448323f6c9d21cda61ee1b97f8fea28a5b9e2b1`。

## 載入流程及功能缺口

2026-10-09 補充：[原生物件建立與 UID 契約](object-create-contract.md) 核對關聯池耗盡時的 copy guard、第三個 position 分配及 UID 計數器；原生建立公式不能當作現有 DATA 的通用 invariant。

`CL_LoadLevelData`（`0x48e960`）依序 nested open：

`light → gametime → rain → hagel → snow → flash → objects → position → anim → gfxtype → action → objdata → hirarchy → formatio → lager → engine → fow → fowreq → way → particle → explos → hitex → stat → biglager`。

某檔 open 失敗會跳過後續 open，記錄 `err LoadLevelData: 1000` 並直接退出；呼叫端 `0x487180`（`CLMP_LoadLevel`）未檢查回傳值即進入 CHECK，造成未初始化指標存取卡死（詳見 [地圖載入管線](map-load-pipeline.md)）。可載入原生地圖之 `DATA/` 必須完整保留全部 24 個檔案。

CHECK `0x4acfe0`／`0x4ad110`／`0x4adf40` 走固定 14,000 objects：CHECK 1 計算 HP 初值與士氣、CHECK 2 計算 MP 初值、CHECK 3 以座標對 Archetype 週期取模計算 anim offset 4 相位偏移。`CALCLOW` 依序初始化 10 大池之全域分配游標（`0x77189c`–`0x7718c0`）。

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
