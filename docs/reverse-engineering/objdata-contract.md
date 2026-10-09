# objdata 建立、載入與物件索引

2026-10-09 Codex。Against_Rome.exe SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`；REA Ghidra及Capstone唯讀分析，未執行遊戲。補充 [物件建立契約](object-create-contract.md)。

## 獨立分配與關聯

`objdata` runtime base為 **0xf5ff94**，stride **0x90**（144 bytes），容量14,000。validator `0x4ab8d0` 要求 signed slot0..13,999且首byte非零；allocator `0x4ab600` 從cursor `0x7718b4` 往後找空槽，失敗回傳-1。cursor getter是0x4ac8d0。

低層建立0x4aa780的param_5非null時，呼叫allocator及copy `0x4ab060`，再把所得索引寫到 object runtime **+0x1a**（原指令0x4aab04）。objects reader0x48bae0將第 **1** 個平行欄位（從0開始）載入+0x1a。因此產品的`SelfColumn=1`實際是objdata link，不能僅依名稱當成永遠等於object slot的身分欄位。

本輪唯讀74對objects／objdata，392,344個active objects全有有效active target，0越界／inactive／shared／unreferenced active，這些樣本全部link==object slot。這是磁碟快照觀測；原生有獨立allocator，不證明所有未來建立或slot釋放後仍同步。74個objects decoded hashes與前輪pool links manifest一致。

copy接受signed有效slot及非null範本，成功回傳1，無效回傳0。低層create未以copy回傳值gate成功；分配失敗的-1仍可能存為ffff link。release `0x4abb80` 從object+0x1a取索引呼叫reset `0x4ac0c0`，再將link設ffff；reset降低cursor到較小的釋放slot。

## 磁碟布局與 runtime 差別

磁碟header為version/count兩個u32（8bytes），後面36段平行陣列，每slot共123bytes。每段內有多欄位時按該slot依序存放，並非整個runtime結構的memcpy。

| segment | bytes/slot | runtime offsets |
|---|---|---|
| 0 | 9 | byte+0、float+4、float+8 |
| 1 | 8 | float+0xc、float+0x10 |
| 2 | 4 | +0x14 |
| 3 | 4 | u16+0x18、u16+0x1a |
| 4 | 2 | +0x1c |
| 5 | 16 | u16+0x1e、u16+0x20、float+0x24/+0x28/+0x2c |
| 6..7 | 4 each | +0x30/+0x34 |
| 8..11 | 2 each | +0x38/+0x3a/+0x3c/+0x3e |
| 12 | 4 | +0x40 |
| 13..15 | 2 each | +0x44/+0x46/+0x48 |
| 16..19 | 4 each | +0x4c/+0x50/+0x54/+0x58 |
| 20 | 2 | +0x5c |
| 21 | 4 | +0x60 |
| 22..31 | 2 each | +0x64..+0x76（step2） |
| 32..33 | 4 each | +0x78/+0x7c |
| 34..35 | 2 each | +0x80/+0x82 |

reader `0x48c760` 對+0x24/+0x28/+0x2c的float讀值經helper0x5c5af2及整數轉換，再取signed16轉回float；原指令0x48caa2/0x48caa7/0x48cab6及後續同構序列確認轉換，helper的精確捨入語意本輪未辨識。因此不能承諾這三欄任意小數均由載入器原樣保留。

copy `0x4ab060` **不寫+0x7c/+0x80/+0x82**，卻會複製runtime **+0x84/+0x88**（0x4ab225..0x4ab237），後兩欄不在上述磁碟reader序列。reset也將+0x84/+0x88設ffffffff，另將+0x8c的u16設0。這些路徑的差別不等於檔案缺欄位或遊戲bug；尚需writer與其他consumer追查。

## active 範本與 reset

高層 `0x4a9740` 傳入param_5的stack範本起點對應local_188。已確認的初始化包括active1、+8/+0x10=1.0、+4及+0xc由type definition的0xc648e8／0xc648f0整數轉float（validator失敗時1.0）、+0x14來自0xc648c8、+0x4c來自0xc64ae8（definition stride0x2a4）。+0x1c通常0，指定definition/helper條件下100。本輪不為未知欄位命名。

範本含u16預設+0x20/+0x38/+0x3a=ffff、+0x68/+0x6a=8001，以及多個尾端ffff；+0x84/+0x88=ffffffff。inactive reset0x4ac0c0同樣有ffff／8001等值，並清active，並非全零。active範本的型別依賴欄位與reset不同，不能只在空白reset資料上設active1就當作完整新增。

產品`LevelObjectStore.Add`目前把36段範本複製到object相同slot，再覆寫column1=self；這對本輪同slot樣本相容，但尚未建立獨立objdata allocator與其他pool的完整交易。此文件不宣稱現有新增或刪除的實機效果已修復。

## 證據與待辦

REA `re_workspace/objdata-create-20261009-evidence.json`（12records）已匯出/session關閉。copy evidence `ev_9dc0228f39c51ca3e78e3e2d2ccc55d8b936417548c72cef69915c61ef4c2884`、reset `ev_be9e81afcce9572ea71e8540f2097dd42b67dc8c22ba43ef08d2d75d14c73e64`、reader `ev_2a7d78fec35a01abcf19dbca226df048dd8162bb7683913adc1fbd659226c98b`、objects reader `ev_5fd563d654c0befc8940ee6d332d7fb075124e6512a393035754d1715ed40614`。

本地唯讀probe／manifest `probe-objdata-create.py`、`objdata-links-20261009.json`及Capstone視窗`objdata-create-instructions-20261009.txt`保留於忽略的re_workspace。視窗包括函式後的padding／相鄰bytes，不能把linear decode的全部行都當成可到達指令。掃描對version1、容量及精確長度作assert，未驗證所有欄位語意。未改C#，本輪不重跑.NET測試。

下一步確認writer的欄位對應、+0x7c/+0x80/+0x82及runtime尾端的初始化／consumer，並將各獨立pool納入產品新增與移除的交易設計；實機載入、生成與存讀檔仍待驗證。
