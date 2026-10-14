# 地圖 action／anim 資料池

> 2026-10-09，static-verified。支援物件放置、資料一致性檢查與地圖載入診斷；尚未證明新建或清空地圖可在遊戲載入。

唯讀分析 EXE SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。以下為 PFIL 解壓後的 little-endian 樣本布局，欄位用途尚未命名。

## light.dat 原生欄位（2026-10-10）

reader `0x48ab70`／writer `0x487f70`：header 為 version=1、N=1024，後接十六個 N 筆平行陣列。不能把每槽合计 56 bytes 當成連續 record；runtime base `0x7f9ef0`、stride `0x44`。

| zero-based column | 寬度／編碼 | 用途 | runtime offset |
| --- | --- | --- | --- |
| 0 | u32 | active（非零有效） | 0 |
| 1–3 | 各 float32 | X／Y／Z | 4／8／0x0c |
| 4–6 | 各 float32 | R／G／B | 0x10／0x14／0x18 |
| 7–8 | 各 float32 | radius 兩欄，create 寫相同輸入 | 0x1c／0x20 |
| 9 | signed16 | flicker type | 0x24 |
| 10 | float32 | type parameter | 0x28 |
| 11 | u32 | counter，create 清零 | 0x2c |
| 12 | u32 | special fx | 0x30 |
| 13–15 | 各 signed16 | owner／parent／definition，-1 sentinel | 0x34／0x36／0x38 |

欄位 c 的槽 s 位址為 `8 + N*sum(widths[:c]) + s*widths[c]`。直接光源 owner 與 **objects 第12個平行欄位 → runtime +0x24** 雙向關聯；APT 附屬光源的 parent 是另一欄，不能混用。原生 reset `0x49f700` 清所有欄及三個 sentinel，並清有效 owner 的物件回連；`0x4c9590` 按 parent 清附屬光源。

`NativeLightPool` 已支援解析、batch 全量預檢／容量不足不變更、釋放及 PFIL／rollback 儲存；`NativePoolDiagnostics` 已接編輯器地圖檢查。後續已接LevelObjectStore直接光源配置／副本／移動／釋放及objects回連，Save沿同一rollback；最初受Windows DLL載入政策阻擋，重新編譯後15交易/方向測試通過。Owned copy提供active type定義時按原生lihei/lightrad/lightang計算新方向的光源位置。BlankMapContent統一使用此模型清理light，不以錯誤的通用record/column布局覆蓋結果，满1024槽亦可reset。完整APT／物件附屬組交易仍未完成，尚未遊戲內驗收。產品拒絕非有限參數及越界索引，不宣稱完全模仿原生所有輸入行為或驗證 PFIL 壓縮尾端完整性。

唯讀 probe `tools/re/probe_native_light_pool.py`／manifest `re_workspace/native-light-pool-20261010.json`，REA ledger `native-light-pool-20261010-evidence.json`（13 records）；來源 hash 前後核對。74 圖共 1415 active、447 owner、553 parent，447 直接回連全吻合。僅驗靜態程式與已儲存資料，尚未做遊戲內驗收。

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

## 24 大資料池完整規格與解密長度不變量（2026-10-09 逆向與全圖驗證）

唯讀掃描全部 74 張原生地圖之 `DATA/` 目錄，共 **1,776 個池檔案**（74 × 24）：
- **版本符合率**：1,776 / 1,776（100%，全部為 Version 1）。
- **長度符合率**：1,776 / 1,776（100%，解密尺寸全數精確吻合）。
- **異常數量**：0（零異常）。

| 池檔名 | 標頭 (bytes) | 宣告槽位 (Count) | 記錄結構 (bytes) | 平行陣列 / 尾端資料 | 解密長度 (bytes) | Reader VA | Buffer VA |
|---|---|---|---|---|---|---|---|
| `light.dat` | 8 | 1,024 | 16 平行陣列，共 56 bytes/slot | 全部欄位均為平行陣列 | 57,352 | `0x48ab70` | `0x7f9ef0` |
| `gametime.dat` | 32 (固定) | - | 環境時間與時鐘狀態 | 無 | 32 | `0x48b0c0` | `0x771d30` |
| `rain.dat` | 20 | 16,384 | 16 (雨滴粒子/高度) | 無 | 262,164 | `0x48b230` | `0x771ecc` |
| `hagel.dat` | 20 | 8,192 | 28 (冰雹粒子) | 無 | 229,396 | `0x48b440` | `0x7b1ed8` |
| `snow.dat` | 20 | 4,096 | 16 (雪花粒子) | 無 | 65,556 | `0x48b720` | `0x7e9ee4` |
| `flash.dat` | 16 | 100 | 232 (閃電光影) | 16 subslots | 23,312 | `0x48b910` | `0x11a0574` |
| `objects.dat` | 16 | 14,000 | 121 (物件核心狀態) | 無 | 1,694,016 | `0x48bae0` | `0xa14bfc` |
| `position.dat` | 8 | 33,000 | 17 (世界座標/導航點) | 無 | 561,008 | `0x48c0c0` | `0xb1883c` |
| `anim.dat` | 8 | 14,000 | 21 (動畫狀態) | 14000×2 + 14000×2 (u16) | 350,008 | `0x48c250` | `0xbb9a5c` |
| `gfxtype.dat` | 8 | 14,000 | 15 (渲染外觀定義) | 14000×2 (u16) | 238,008 | `0x48c460` | `0xc2705c` |
| `action.dat` | 12 | 14,000 | 25 (1+6×4 旗標) | 無 | 350,012 | `0x48c630` | `0xf00454` |
| `objdata.dat` | 8 | 14,000 | 123 (屬性/生命/精力) | 無 | 1,722,008 | `0x48c760` | `0xf5ff94` |
| `hirarchy.dat` | 12 | 3,200 | 103 (部隊編制/成員) | 3200×2 (u16 backlink) | 336,012 | `0x48cf70` | `0x114c294` |
| `formatio.dat` | 8 | 14,000 | 15 (陣型間距/關聯) | 14000×4 + 14000×4 + 14000×2 | 350,008 | `0x48d0f0` | `0x11b0294` |
| `lager.dat` | 16 | 3,200 | 43 (營地/聚落槽位) | N×u16 biglager 索引 | 144,016 | `0x48d2d0` | `0x120fdd4` |
| `engine.dat` | 94 (固定) | - | 引擎全域計數器/狀態 | UID counter @ offset 34 | 94 | `0x48d6e0` | `0x7717e8` |
| `fow.dat` | 5 | 256×256 | 1 byte 迷霧可見度網格 | 5 bytes 標頭 | 66,053 | `0x48d980` | `0x199e228` |
| `fowreq.dat` | 8 | 256 | 18 (迷霧更新請求) | 無 | 4,616 | `0x48da30` | `0x19be62c` |
| `way.dat` | 12 | 768 | 路徑節點拓撲網格 | 無 | 791,052 | `0x48dba0` | `0x15488d8` |
| `particle.dat` | 12 | 1,024 | 2,252 (特效發射器) | 四個各 N×4 bytes 的平行陣列 | 2,322,444 | `0x48dd90` | `0x12355d4` |
| `explos.dat` | 8 | 1,000 | 54 (爆炸實體槽位) | 無 | 54,008 | `0x48e210` | `0x9d29f4` |
| `hitex.dat` | 8 | 1,000 | 40 (命中粒子槽位) | 無 | 40,008 | `0x48e4d0` | `0x9e04b4` |
| `stat.dat` | 12 | 8 | 816 (隊伍 0..7 統計) | 無 | 6,540 | `0x48e720` | `0x1546f58` |
| `biglager.dat` | 12 | 32 | 1,601 (大型駐軍表) | 無 | 51,244 | `0x48d590` | `0x153a718` |

### formatio.dat 結構細節
- reader `0x48d0f0`：標頭 8 bytes（u32 version 1, u32 count 14,000）。
- 主記錄區：$14,000 \times 15 = 210,000$ bytes。
  - `active`: uint8 (1 byte)
  - `target_slot`: uint16 (2 bytes, 預設 0xffff)
  - `formation_spacing`: float32 (4 bytes, 預設 1.0f = `0x3f800000`)
  - `flags / group`: uint32 (4 bytes, 預設 0)
  - `timer / interval`: uint32 (4 bytes, 預設 1000)
- 平行尾陣列：總長 140,000 bytes。
  - 陣列 1 (`0x48d19e` 讀取)：$14,000 \times 4 = 56,000$ bytes（uint32，預設 0）。
  - 陣列 2 (`0x48d24c` 讀取)：$14,000 \times 4 = 56,000$ bytes（uint32，預設 0xffffffff / -1 哨兵）。
  - 陣列 3 (`0x48d270` 讀取)：$14,000 \times 2 = 28,000$ bytes（uint16）。
- 總長：$8 + 210,000 + 56,000 + 56,000 + 28,000 = 350,008$ bytes。

### lager.dat 結構細節
- reader `0x48d2d0`：標頭 16 bytes（u32 version 1, u32 count 3,200, u32 extra1 6, u32 extra2 10）。
- 主記錄區：3,200 筆，每筆43 bytes，之後接獨立3,200×u16尾陣列。舊版45連續record推論已更正。
  - `active`: uint8 (1 byte)
  - `type / id`: uint32 (4 bytes)
  - `ratio`: float32 (4 bytes)
  - occupancy：signed16，磁碟+9、runtime+c。
  - 六個u16輔助欄：磁碟+11、runtime+e；不是14bytes字串，完整欄位用途未確認。
  - 小型駐軍物件索引：10個signed16，磁碟+23、runtime+1a，-1表示無引用。
  - 尾陣列：signed16 biglager索引，runtime+2e，-1表示無引用。
- 總長：$16 + 3200\times43 + 3200\times2 = 144,016$ bytes。

biglager reader `0x48d590`：header三個u32為1/32/800，每槽active byte加800個signed16物件索引，runtime stride1602、磁碟1601bytes，總長`12+32*1601=51244`。`0x415030`證明陣列count×2及預設stride2；不是先前平均值1600.875bytes。

`0x4d4870`依有效biglager選10或800容量、以物件索引填入駐軍並增加occupancy；`0x4d49f0`取出／移位駐軍並可將成員送入刪除佇列。原object release `0x4abc29..65`則掃全部10個小型成員，對有效物件queue delete，不按occupancy裁切。lager reset `0x4ac3e0`清前欄／六輔助欄、10個小型成員設-1，再呼biglager reset `0x4ac4a0`清active與800個索引；它沒有重建所有成員runtime狀態，不能把磁碟reset當完整營地生命週期。

`probe_native_lager_pool.py`19指令斷言及74圖唯讀核對：2765個object col5引用中2763指向active lager、2指向inactive；2763active lager全部有唯一object使用、occupancy0且沒有小型成員。91biglager引用全部active；93active biglager中2個沒有lager引用，全部沒有駐軍，未發現跨lager共用。異常／未引用槽保留，不自動修復。每圖lager/biglager/objects前後bytes及EXE前後一致；manifest `native-lager-audit-20261010.json`、REA11records `lager-layout-20261010-evidence.json`。

NativeLagerPool/NativeBigLagerPool提供獨立parse/capture/reset/rollback Save，6個模型case通過。後續LevelObjectStore已接兩池Load/Save/Blank與family刪除：object col5指定營地，將小型／大型駐軍物件及其子樹納入刪除閉包，拓撲排序拒絕cycle；預檢失效成員、occupancy不符、外部object/camp/biglager共用及其他未知池關聯，完成全部預檢才回收。單物件仍不能獨立移除／複製駐軍；Add保留被active駐軍表引用的空物件槽，回收後重建快取。Blank使用原生兩池reset，不依賴inactive模板，也不由generic reset覆寫。

新增14個camp合成case通過（nested小/大駐軍、descendant與外部關聯拒絕、raw/PFIL rollback）。原圖TEMP驗收已通過HIST_004兩組：root318/UID44048有2物件、1lager、1biglager、4light；root1970/UID49270有3物件、1lager、2particle、1light。11個檔案交易回復、所有未刪營地槽bytes及原來源hash保留均驗證；此不是74圖所有family皆可修改，也不是啟動遊戲驗收。完整create/copy、runtime/APT投影仍待完成。

完整solution回歸為Modules860passed/2skip、Host988passed/25skip，合計1848passed/27skip/0fail，包含滿3200/32槽Blank reset、單純駐軍保護與inactive member槽保留。Review補上Blank缺四個專用池時在修改前失敗，最後Release build0errors/2既有warnings，新增guard後Host99相關定向passed/0fail；未重跑完整不相關測試。TRX `camp-family-regression-20261010`及`camp-family-final-20261010`。檔案交易驗證不能當作完整runtime行為證明。

### particle.dat 結構細節（2026-10-10 Codex）

原生 reader `0x48dd90` 每槽讀取12 bytes前綴與64個35 bytes子項，然後才讀四個平行陣列；不能將平均每槽2268 bytes當作連續record。header為u32 version1、count1024、children64。

- record+0：u16 active；+2：u32未命名欄位；+6：signed16 owner object；+8：u32 counter。
- 每個子項由u8、七個u32、三個u16組成；最後的signed16為light槽位，位置是record+12+child×35+33，-1表示無引用。
- 尾端四個陣列對應runtime+10/+14/+18/+8dc，每個1024×4 bytes；原生reset `0x4db800` 將前三值清0、第四值設30，owner和64個light索引設-1。
- reset對64個light索引都呼叫light release，不以子項active byte作前提。有效owner另送入原生object刪除佇列；編輯器以完整family预檢及離線交易處理，沒有模擬runtime佇列或粒子運算。

`probe_native_particle_pool.py` 核對21原指令及序列化reader型別順序；74原圖的1714個active emitter全部有有效owner與吻合object col11回連，172個child-light引用全部active，未發現跨emitter共用light或重複owner。三個來源檔每圖前後bytes一致，EXE前後一致。manifest為`re_workspace/native-particle-audit-20261010.json`，REA兩筆證據為`particle-layout-20261010-evidence.json`；此為磁碟／靜態證據，不是遊戲畫面驗收。

NativeParticlePool已接入LevelObjectStore Load/Save/Blank及family刪除交易。新增八個family案例通過，完整Modules841passed/2skip；Host測試DLL被Windows政策0x800711C7擋在discovery，新增七個粒子專項尚未執行。lager、完整建立／複製及遊戲內驗收仍待完成。

後續NativePoolDiagnostics加入粒子target/owner雙向檢查、子light有效性／共用錯誤與ActiveEmitters統計，既有地圖檢查可使用這些結果。新增invalid-header/inactive-light與診斷斷言後build仍0errors，但最新Modules測試DLL也被policy0x800711C7擋在discovery，最新診斷接線尚未完成測試；前述841passed是診斷改動前的回歸，不作新改動證明。

後續驗證更新：營地模型加入後當前Host/Modules DLL可載，沒有修改Windows政策。Host定向首次89passed/3failed，因三個healthy fixture的object col11為0卻缺particle池；fixture更正無引用sentinel FFFF後92passed/0failed（含粒子七case、營地六case、診斷及ARM_GAME_PATH原圖TEMP交易）。Modules完整843passed/2skip/0failed，包含兩新診斷case。TRX為`lager-particle-final-20261010`與`lager-particle-modules-20261010`；此證明當前版本可測，不代表policy已全局解除，也不是遊戲內驗收。

## 載入流程及功能缺口

2026-10-09 補充：[原生物件建立與 UID 契約](object-create-contract.md) 核對關聯池耗盡時的 copy guard、第三個 position 分配及 UID 計數器；原生建立公式不能當作現有 DATA 的通用 invariant。

`CL_LoadLevelData`（`0x48e960`）依序 nested open：

`light → gametime → rain → hagel → snow → flash → objects → position → anim → gfxtype → action → objdata → hirarchy → formatio → lager → engine → fow → fowreq → way → particle → explos → hitex → stat → biglager`。

某檔 open 失敗會跳過後續 open，記錄 `err LoadLevelData: 1000` 並直接退出；呼叫端 `0x487180`（`CLMP_LoadLevel`）未檢查回傳值即進入 CHECK；這是靜態錯誤傳遞缺口，未證明未初始化指標存取或卡死根因（詳見 [地圖載入管線](map-load-pipeline.md)）。可載入原生地圖之 `DATA/` 必須完整保留全部 24 個檔案。

CHECK `0x4acfe0`／`0x4ad110`／`0x4adf40` 走固定 14,000 objects：CHECK 1 計算 HP 初值與士氣、CHECK 2 計算 MP 初值、CHECK 3 以座標對 Archetype 週期取模計算 anim offset 4 相位偏移。`CALCLOW` 依序初始化 10 大池之全域分配游標（`0x77189c`–`0x7718c0`）。

## 驗證與限制

### objects → anim 關聯與清空（2026-10-09 續查）

`0x4ab830` 以 signed index 0..13,999 加上 anim runtime offset 0 的 byte 非零判定 slot 有效；`0x4ab7b0` 對 objects 做相同範圍／active 檢查。這補足首 byte 的原生用途，不代表 byte 僅允許 0/1。

objects reader `0x48bae0` 將 serialized record offset **71** 的 u16 寫入 runtime object offset **0x12**。object base `0xa14bfc`、stride `0x4c`；CHECK `0x4adf6c` 讀 `[base + slot*0x4c + 0x10]` 的 dword，再 `sar 16`，取得 offset 0x12 的 signed anim index，呼叫 `0x4ab830`。`0xffff` 對應 -1／無索引。

native create `0x4aa780` 在 anim template 非 null 時呼叫 allocator `0x4ab500`、copy `0x4aaea0`，最後 `0x4aa9f1` 將 anim index 寫到 `0xa14c0e + slot*0x4c`。allocator 從 `0x7718ac` 的搜尋起點往後找第一個無效 slot，耗盡回傳 -1；不可假定 anim index 等於 object index。

native object release `0x4abb80` 經 `0x4abd0d` 呼叫 anim reset `0x4abf30`，並將 object anim link 置為 `0xffff`，另外處理 position 及其他 pools／runtime 系統。anim reset 清除各有效欄位，但 runtime offset **0x1c = 1**，不是整筆全零；同時將 allocation search cursor 往較小的已釋放 slot 更新。`0x4aba40` 對全部 14,000 anim slots 呼叫 reset，由 pool initialization `0x487910` 使用；此處尚未追到所有入口的初始化呼叫順序。

**修正過的嘗試：** 初版新 probe 錯將 objects 第 10 個平行欄位當成 anim link，產生 1,295 越界／8,603 inactive 統計。原始指令及 reader／create／release 三條路徑證明該欄位是 runtime offset 0x10，anim link 應是其高半部 offset 0x12／serialized offset 71。初版 manifest `object-anim-links-20261009.json` 保留作失敗紀錄，不能作結論；正式工具已修正。當時 `LevelObjectStore.IsLinked` 使用 column 10 + objdata segment 2 作可編輯性 heuristic；續查已確認前者是第三position、後者是float scalar，並已修正產品判斷及三位置寫回，詳見 [物件建立契約](object-create-contract.md)。

`probe_object_anim_links.py` 唯讀掃描 74 對 objects／anim：0 parse errors、0 越界、0 shared slots、0 unreferenced active anim slots；**ENDL_005 有 33 inactive targets，其餘 73 張為 0**。抽看前三筆為 type 717／team 8，不據此宣稱全部同型或根因。ENDL_005 是既有自訂測試目錄，差異尚未歸因；不能把原生不一致觀測當成原版格式錯誤。有效性關聯僅涵蓋這一個欄位，不驗證 UID、其他 pools 或實機載入。

正式 manifest `re_workspace/object-anim-links-corrected-20261009.json`；Capstone 原始指令 `object-anim-link-instructions-20261009.txt`；REA ledger `object-anim-links-20261009-evidence.json`。關鍵 evidence：validator `ev_7b3bcbd623ecc9309026a7aa8dcf001c60789087fd7c4fe9f2b21fdae2e7b669`、reader `ev_9d7d9efc7c46bae1d7dffbf42f732263ecc3ab24c05f488c9bc8e8f3a94306ce`、create `ev_fe134aa9ec9c7d8bd78038a5b76f8a405f2f833f961b98e342b5d823e03b56a0`、reset `ev_c6c8722858b2ad3f3415ce0e761b37a58be86208ed9ca69f09e8344815892c1b`。

`python tools/re/probe_map_pools.py "C:\Program Files (x86)\Against Rome" re_workspace/map-pools-20261009.json`

74 個 action + 74 個 anim 完整解壓到宣告長度，148 檔 header／精確長度全通過，0 errors；所有 N=14,000。ENDL_000 兩池首 byte 同有 7,382 個 0、6,618 個 1；分布相符不證明首 byte 用途或物件關聯。

probe 不驗證缺檔完整性、PFIL 壓縮尾端或 wrapper integrity，不執行遊戲、不輸出解壓素材、不推論 runtime 成功。完整 REA ledger：忽略的 `re_workspace/map-pools-20261009-evidence.json`，22 records。action evidence `ev_2e8eaaf34e6d59f1c369401d7e64f807ceaade42af0aca5105e1154a8b06cad8`；anim `ev_f95a69a9f4d12c94e3990d017f1e7403e742ac7e5d37f44f86b2abb939cd6ca7`；loader `ev_f1c5bd225c1a22202fcb1100a098423990fb08ae60b31e7aa44c73ff3cdee6ed`。
# 原圖 family copy 驗證補記（2026-10-10 Codex）

2026-10-11 滿容量邊界：`LevelObjectStore` 沒有 inactive object 樣本時，改用 detached editor reset 範本供後續刪除，載入與候選重建不改 active payload；有樣本仍保留原取樣策略。此為編輯器重置政策，不宣稱是原生完整列清除行為。raw/PFIL 測試驗證 family 複製剛好填滿容量、完整池 Clone/Save/reload、滿池 Add 拒絕且 bytes 不變、刪除後重用槽位及持久 UID cursor 不重用退休身份。相關專項合計161passed/0failed/0skipped，TRX `re_workspace/family-full-capacity-20261011/`（15_30_01）；未執行遊戲。

2026-10-11 更新：原圖副本測試已由「計畫 target 外不變」提升為完整 11 payload 的獨立 raw-byte expected 比對，包括新 slots、UID 與跨池 links、auxiliary tails、所有 64 粒子 child light links 與 800 大營地成員位置。HIST_004 兩個 roots 的零位移與非零位移（X +1.25、Y -0.5、Z +2）共4個案例通過；packed rollback、Save/reload、原角度與原始來源 hashes 亦通過。主代理最新 Host 專項合計18passed/0failed/0skipped，TRX 在 `re_workspace/native-family-host-contracts-20261011/`（16_00_41）。這是已保存資料的檔案驗證，不代表 UI 或遊戲 runtime 驗收。

`CreateFamilyCopyCandidate` 現在同步更新 `engine.dat` offset34 的 UID cursor。`Original_family_copy_in_temp_preserves_outside_payloads_and_persists_uid_cursor` 以 HIST_004 的 roots318/44048、1970/49270 驗證獨立 TEMP 副本：兩項通過，11 個 decompressed payload 在計畫 target 範圍外逐 byte 不變，rollback 後 packed files 完全相同，commit/reload 後原物件與舊／新 family 保持有效，cursor 為新 UID 上限加一；11 個原始來源 SHA256 不變。最新 Release build 為0errors/2既有warnings。

本輪複製位移為零；尚未完成新 target 全欄位的獨立預期、最新完整測試、Scenario/UI 接受候選與遊戲內驗收。證據位於忽略工作區 `re_workspace/original-family-copy-20261010/`。使用者要求本輪完成後暫停，後續整合待恢復工作。
