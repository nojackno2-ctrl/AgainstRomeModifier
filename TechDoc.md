# Against Rome Modifier 技術文件

> 更新日期：2026-07-04
>
> 編碼：UTF-8
> 對象：開發者、逆向工程研究者與 AI 維護代理

本文件描述目前程式與補丁契約。完整的修改時間線、除錯案例、代理操作規範與驗證方式均已整合於本文件尾部。機器可讀的欄位與 offset 位於 `data/game_schema.json`；精確 EXE／BCI bytes 位於 `docs/reverse-engineering/known-patches.md`。

## 1. 新代理必讀與修改原則

### 1.1 新代理開始前必讀
1. 唯一的即時工作區是本文件頂端的離線路徑。舊的 OneDrive 路徑只可當歷史背景，不能作為讀寫目標。
2. 先執行 `Get-Location`。部分沙箱會忽略含中文的 `cwd`，實際從 `C:\` 開始；不要在根目錄做遞迴搜尋。
3. 先執行 `git status --short --ignored` 和 `git diff`。工作樹可能已有使用者或其他代理的變更，禁止任意覆寫。
4. Git 若回報 dubious ownership，使用一次性的 `git -c safe.directory=... -C ...`，不要改全域設定。
5. 修改任何遊戲檔前，必須確認所選路徑內存在 `Against_Rome.exe`。僅有「目錄存在」不足以證明它是遊戲根目錄。
6. 不得把未知 EXE 位元組、未知 BCI opcode、Ghidra 自動命名的 `FUN_*`，寫成已證實語意。
7. 不得弱化回復保護。遇到缺少原版基線、未知 patch state 或不安全 ZIP 路徑時，應停止並清楚報錯。
8. 每個新補丁至少要有：原始值、修改值、狀態偵測、回復方式、版本／長度防護、失敗時 rollback、文件與 schema 同步。

### 1.2 證據等級定義
本專案文檔與開發使用以下語意；不要混用：
- **穩定／已實機驗證**（Stable）：程式路徑與遊戲行為都有證據。
- **靜態驗證**（Static Verified）：位元組、反編譯、檔案結構或 round-trip 已核對，但缺少足夠的長時間實機測試。
- **候選**（Candidate）：有合理的資料流或格式證據，只能讀取／研究，不應由修改器自動寫入。
- **舊版相容狀態**：目前不再產生，但偵測和回復必須保留。
- **已否決**（Rejected）：實機結果證明假設錯誤或造成回歸。保留精確紀錄是為了防止再次加入。
- **未知**（Unknown）：位元組或資料不符合任何已知狀態。必須保留原檔並警告，不得強寫。
- Ghidra 的 `FUN_*` 只是導航點，必須有 call path、資料流、字串註冊或實機證據，才能命名其意義。
- 每個寫入功能必須能偵測原版／目前版／舊版／未知狀態，且有明確回復路徑。

## 2. 技術架構

| 檔案 | 責任 |
|---|---|
| `src/Program.cs` | WinForms 入口、DPI、UAC |
| `src/UI/ModifierForm.cs` | 手工 UI、事件 wiring、文件頁 |
| `src/UI/ModifierForm.Data.cs` | 備份載入、目前資料讀取、狀態偵測、顯示 |
| `src/UI/ModifierForm.Patches.cs` | 套用、回復、rollback、遊戲檔補丁 |
| `src/UI/ModifierForm.Presets.cs` | 一鍵啟用／關閉所有功能控制 |
| `src/UI/ModifierForm.SaveManager.cs` | 存檔瀏覽、備份、回復、刪除 |
| `src/UI/ModifierForm.DgVoodoo.cs` | dgVoodoo2 受管安裝／移除 |
| `src/Core/TroopConfig.cs` | 欄位 enum、單位 metadata、平衡規則 |
| `src/UI/TroopPresetForm.cs` | 9 欄單位 preset 編輯 |
| `src/Core/GameLZSS.cs` | 遊戲 LZSS 與 `PFIL@` 包裝 |
| `src/Core/Bci/` | BCI 特徵碼搜尋、字組寫入與 PFIL 腳本封裝 |
| `src/Core/EndlessAi/` | AI Ultimate M1–M14 模組、狀態偵測與套用協調 |
| `src/Core/Patches/` | 純 patch 邏輯（`ObjdefPatcher`、`RessPatcher`、`ClScriptPatcher`、`ClEparaPatcher`、`ClScintPatcher`、`TeamDatPatcher`、`ExePatchModel`、`VerifiedBinaryWriter`），不依賴 WinForms，`ModifierForm.Patches.cs` 只負責把 UI 狀態轉成 Options／委派呼叫與記錄日誌 |
| `src/Core/Localization.cs` | 中英文 UI／log |
| `data/game_schema.json` | 機器可讀的欄位、offset 與 patch metadata |

目標框架為 .NET 8 Windows、WinForms、x64、nullable enabled、PerMonitorV2 DPI。程式 manifest 要求管理員權限，因為正常遊戲安裝位於 `Program Files (x86)`。

## 3. 遊戲根目錄與備份基線

`GetGamePath()` 回傳 UI 中選定的遊戲路徑。套用、回復、啟動、SAVE、語言 overlay、dgVoodoo2 都必須從這個根目錄推導。任何會寫入或刪除的入口，都應要求 `<gamePath>\Against_Rome.exe` 存在。

原版資料來源順序：

1. 內嵌 `Backup.zip`。
2. 執行檔旁的本機 `Backup.zip`。
3. 使用者所選遊戲安裝中的檔案，載入成記憶體基線。

公開版本不包含原始遊戲資料，因此第三種方式是正式支援路徑。補丁從原版基線重新產生，避免在已修改資料上累積倍率。

「執行修改」會在同一個 `FileRollbackScope` 交易內，先執行不改動 UI 勾選狀態的完整原版恢復階段，再依先前快照的選項重新產生並寫入補丁。恢復範圍與「恢復原版」共用 `RestoreOriginalFilesInternal`，包含 EXE、屬性檔、team.dat、無盡 AI、食物治療、語言與 dgVoodoo2。這可清除舊版已寫入、但新版 UI 已不再提供的殘留修改；若任一恢復或重套步驟失敗，整次操作回滾到執行前狀態。

## 4. 寫入與回復安全

`FileRollbackScope` 追蹤每個目標在第一次修改前的 bytes 或「原先不存在」狀態。`SafeWriteAllBytes` 使用暫存檔完成單檔替換；流程成功後才 `Commit()`，否則回復所有已追蹤目標。

這不是多檔案的檔案系統原子交易。2026-07-02 工作樹雖已把 EXE、INI、DAU、team.dat、ENDL BCI 的候選內容先產生為 `byte[]`，實際仍逐檔寫入，語言與 dgVoodoo2 也有各自 I/O。安全性來自預先驗證加 rollback，不可描述成真正的單次 atomic commit。

commit 後要先 Dispose／清空 rollback scope，再更新 UI；UI refresh 例外不應回滾已完成的遊戲檔。

## 5. `PFIL@` 與 CSV-like 格式

- `DecompressPfil` 讀取 64-byte PFIL header 與 LZSS payload；無 PFIL header 時回傳原始 bytes。
- `CompressPfil` 要求原始 header 至少 64 bytes，並重寫解壓大小（header offset 16 是唯一的大小欄位）。
- 遊戲文字使用 Windows-1251；專案文件使用 UTF-8。
- 修改後至少驗證 `decompress(compress(payload)) == payload`。
- **環狀視窗初始化契約（2026-07-02 修正）**：遊戲 EXE 解壓器（`FUN_00565c00`）
  只把環狀視窗前 `0xFEE` 個位置填為空格 `0x20`，最後 18 個位置
  （`0xFEE..0xFFF`）是 `memset(0)` 後的 `0x00`；寫入位置從 `0xFEE` 開始。
  壓縮器的視窗模型必須完全一致。舊版 `GameLZSS` 把整個視窗當成全空格，
  導致檔案開頭 18 bytes 內的空格串可能被匹配到 `0xFEE..0xFFF` 的「假想空格」，
  遊戲解壓時輸出 `0x00`——文字檔會被 NUL 截斷（症狀：`No Mem len=0
  CLMK\mk_tcon.c [1138]`，`.sdl` 解析出 0 個物件）。二進位檔（BCI）
  開頭無空格串所以從未觸發。
- `objdef.dau` 還要求解壓文字總長不變。
- 資料是簡單逗號分隔，現行相容契約為 `Split(',')`／`Join`；不要換成 RFC 4180 quote parser。

## 6. `objdef.dau`

路徑：`SYSTEM/DATA_MP/DEFAULTS/objdef.dau`。

主要 zero-based 欄位：

| Index | 名稱 | 狀態 |
|---:|---|---|
| 4 | Moves | stable |
| 19 | Hp | stable |
| 23 | Movsf | stable |
| 24 | Sirad | stable |
| 52 | Name | candidate |
| 78..84 | Weapon 1 | stable；angle 82 保留 |
| 86..92 | Weapon 2 | stable |
| 94..100 | Weapon 3 | stable |
| 142 | Aw | stable |
| 146 | Vw | stable |
| 156 | HousingCapacity / `wohnwer` | stable |
| 42 | StorageCapacity / `maxre` | stable |
| 73 | BuildTime / `buildt` | stable |
| 74 | UpgradeTime / `upgrdt` | stable |
| 191 | Bmovs | stable |
| 199 | Weapon 1 damage/type base | candidate |

### 6.1 單位屬性層級

原版／平衡數值先形成 fallback，自訂 preset 再逐欄覆蓋。舊版短 preset 只覆蓋存在的欄位；缺欄位繼承 fallback。不支援 spell radius 的單位，第 9 欄固定為 0。

內建平衡層由 `TroopConfig.BalancedUnitStats` 直接保存全部 43 個兵種的九項最終值：`HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius`。啟用平衡後不得再套用通用階級矩陣、盾牌、雙手武器或兵種類型倍率。靜態初始化會檢查平衡表與 `UnitMeta` 數量一致，且 `UnitOrder` 中每個兵種都有完整九欄。

設計方向：羅馬整體素質最強；條頓近戰輸出最高；塞爾特步兵防禦與步行遠程最強，投石兵採高單發傷害；匈奴騎兵最強，弓騎兵採低單發、高射速。步兵、騎兵與領主維持相同移動速度。精確數值以 `BalancedUnitStats` 為唯一來源。

### 6.2 人口建築容量 20 倍

- 對原版 `wohnwer > 0` 的所有 row 生效；已觀察到 22 rows。
- 寫入 `original * 20`，不是 `current * 20`。
- 保留欄位寬度與整個解壓 payload 長度。
- `LoadCurrentData` 以所有正值 row 對比 `original * 20` 來偵測狀態。
- 整合 UI、preset、apply、restore 與 state detection。

### 6.3 建築建造、升級與維修加速 10 倍

- 對所有以 `Bau` 開頭（即建築物）的 row 生效。
- 讀取備份原版的 `buildt` (Index 73) 與 `upgrdt` (Index 74)，若數值大於 0 則除以 10，最低限制為 1 毫秒，防止因 0 導致計時器異常。
- 修改後使用 `PadLeft` 與 `CheckLen` 維持原欄位字串長度，確保 `objdef.dau` 檔案解壓長度完全一致。
- 遊戲中的維修效率是由生命值與建造時間決定。當建造時間縮短 10 倍，每秒修復生命值比例即同步提升 10 倍，實現建造、升級與維修的全面加速。
- 整合 UI、一鍵預設開關、讀取現有設定偵測、套用及還原機制。已完成實機驗證，確認修改後數值在遊戲中生效，功能運作正常。

### 6.4 主堡與倉庫儲存量 10 倍

- 對所有以 `Bau` 開頭且名稱包含 `Hau`（主堡/主帳）或 `Lag`（倉庫/倉帳）的 row 生效。
- 讀取原版的 `maxre` (Index 42) 資源儲存容量，若大於 0 則乘以 10 倍。
- 修改後使用 `PadLeft` 與 `CheckLen` 維持原欄位字串長度，確保 `objdef.dau` 檔案解壓長度完全一致。
- 整合 UI、一鍵預設開關、讀取現有設定偵測、套用及還原機制。已完成實機驗證，確認修改後主堡與倉庫資源容量上限成功提升 10 倍，且無溢位或異常現象。

## 7. `ress.ini`

路徑：`SYSTEM/ress.ini`。

`[objres]` 主要欄位：

| 範圍 | 用途 | 修改規則 |
|---|---|---|
| 1..6 | 建造成本 | free construction 時清零 |
| 7..12 | 升級成本 | free upgrade 時清零 |
| 13..18 | 單位生產成本 | free production 時清零，保留明確例外 |
| 19..24 | 裝備／卸裝退還 | 必須保留 |
| 25..28 | 祭司法術成本 | no spell cost 時清零 |

`FigTiePac00_Packpferd` 完全排除於免費模式，保留原始成本與退還。不得重新加入 healing-food／healing-speed 修改，也不得為修 UI quota 問題而清除 19..24；騎乘村民與戰鬥單位的 reservation count 涉及 EXE/UI runtime 語意。

## 8. `cl_script.ini`

路徑：`SYSTEM/cl_script.ini`。

目前管理：

- `Radius = <faction>, SpellN, value`
- `Value = <faction>, SpellN, value` （法術效果主要值，如傷害、治療量或百分比）
- `Value2 = <faction>, SpellN, value` （法術效果次要值，如復活士氣百分比）
- `CiviDelay = <faction>, value`
- `MoralsDecLostMem`
- `MoralsDecFlee`
- `MoralsDecOverPop`
- `MoralsIncIdle`

修改時保留註解和未管理內容，依 faction + key 定位。Balance toggle 的事件只應更新預覽，不應偷偷重讀所有 live game files。強制英文 toggle 也不應由 live overlay 自動勾選。

## 8a. `cl_scint.ini`

路徑：`SYSTEM/CLAK/cl_scint.ini`。

目前管理：
- `SpellODef = <faction>, SpellN, alias` （復活技能男性屍體別名）
- `SpellODef2 = <faction>, SpellN, alias` （復活技能女性屍體別名）

修改時會根據法師技能與復活術強化勾選狀態，將塞爾特的 `SpellODef`（原為 `KEL_INF00` 劍士）改為 `KEL_INF01`（槍盾兵），`SpellODef2`（原為 `KEL_SCH00` 弓兵）改為 `KEL_INF02`（女雙劍士）。本檔案以 `PFIL@` 解壓與寫回。

## 9. `team.dat`

路徑：`MAPS/**/team.dat`。

最大人口補丁只修改 `[teamdata]` zero-based column 4 為 1600；`maxteamobjgenerell` 保留。column 5 是 banner version。`team.dat` 不負責無盡模式增援 count。

## 10. AI Ultimate Mode

AI Ultimate 已拆解並重構為 5 個可獨立控制的體驗型功能模組：M1 增援規模 (P1 增援人數 + P10 主營轉兵 + P12 村防轉兵)、M2 增援節奏 (P3 增援等待 + P6 排程迴圈優化)、M3 敗亡快速回收 (P4 撤退期限 + P5 死亡判定去彈跳 + P11 村莊拆除延遲)、M4 保證聚落生成與留守 (P7 聚落必定生成 + P8 增援門檻 + P9 撤退配額歸零)、M5 AI 開局資源 (P13 聚落開局資源)。
為了防止遊戲邏輯死結與重生死當，P2 (任務完工回收)、P15 (DELETE_PARTY 安全遷移) 則合併為常駐底層安全修復 R0，不提供 UI 開關，但套用時會自動常駐生效，以確保 AI 運行健全度。實作入口為 `src/Core/EndlessAi/EndlessAiOrchestrator.cs`。

P15 現為 R0 常駐安全修復：兩個定居型 party 的終態在解壓偏移
`0x109E8`、`0x16374` 必須維持原版 `DELETE_PARTY (256)`。舊版曾改成
`DELETE_TEAM (257)`，可能在 `ak_haupthaus.bci` 等待逐筆拆村確認時提前刪除
team，造成確認永不回傳與模擬迴圈死等。偵測會把 257 或 256/257 混合狀態
視為 Legacy，套用或還原時都遷移回 256；P15 不再屬於可切換的 M3。此外，P2/P4/P5/P11/P8/P9 亦一同作為安全閥常駐套用，以防止工作槽塞滿、重生重疊死當及增援中斷。

主要路徑：`MAPS/ENDL_000..004/SCRIPT/ak_level.bci`。

目前目標狀態：

- 增援 count：4 → 20；EXE runtime clamp 為 1..20。
- 軍事增援 cooldown（`0x178E0`）：180000 ms → 5000 ms。
- 政黨撤退／清理期限採混合值：非聚落型四處 `0x119C0`、`0x12FFC`、
  `0x13FE8`、`0x17F38` 由 600000 ms 改為 5000 ms；聚落型處理器的
  `0x10700`、`0x160EC` 保留 600000 ms。聚落型 state 51/52 會先等待引擎完成
  舊村莊／城牆清理，期限只是保險出口。舊版把六處全改為 5000 ms，會在清理完成
  前強制進入 DELETE_PARTY，使同一隊重生後仍把村民送往舊村莊；下次套用會自動
  把該舊啟用狀態遷移為目前的混合值。
- `SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci` 舊村莊逐物件清理節拍：唯一特徵碼位於
  `0x3248`，原版死亡後迴圈為 `1500 + rand(-25,25)` ms，每輪只處理一個已確認的
  建築／城牆物件。AI Ultimate 僅把基準 `1500 -> 100`，成為每個物件 75..125 ms，
  保留確認協定；71 個物件的理論清理時間由約 105 秒降到約 7 秒。停用時還原 1500，
  已啟用但仍為 1500 的舊版可在下次套用時遷移。此全域主建築腳本也會加速玩家主
  建築死亡後的村莊清理，但不影響正常存活村莊的生產節拍。
  同形的「政黨建立初始到達逾時」（`0x7F24`）刻意不改（改了會讓到達中的政黨
  在定居前就撤退）。
- 死亡確認計數（`0x1068C`）：20 → 3 個連續 tick（村莊、領袖、村民、成員
  全滅的確認去彈跳）。
- 軍事增援部隊數門檻：4 → 40（`s_searchTeamUnits(team) < 40`）；這不是電腦玩家數上限。2026-07-05 的 `ESAVE_000` 證實羅馬軍事聚落僅約 9 支 20 人兵團時，原始主建築資源條件已使後續增援停止；先前保留的領袖／村民判斷仍會因短暫狀態阻斷後續波次。P8 因此把 `0x1960C` 起的條件尾端改成三段等價的 `teamUnits < 40` 有界判斷。更前面的 type-4 聚落、建築存在、同時僅一支 type-5 增援隊等安全條件仍保留。舊版無條件 `jmp +272` 仍屬不安全狀態；它連 40 門檻也跳過，會耗盡工作槽。舊門檻 8、目前門檻 40 但原始 gate、兩種舊有界 gate、以及舊無條件 gate 都會在下次套用遷移至新條件。
- completed-job recycle：0 → 1。
- 增援捐贈公式維持原版；`v56[party]` 撤退配額由 `[90,15]` 改為 `[66,0]`，使整個 type-5 增援黨團移交村莊而不撤退。此補丁必須與門檻 40 同步套用及還原；先前只改配額、門檻仍為 8 的組合會在約一波後停止增援。
- gate 保持原版 `66,0`。
- 六個 AI 排程延遲點全部改為 `5000..10000 ms`。前三個是內層突襲計時器；後三個是外層排程的初始與更新範圍，會直接限制呼叫定居／軍事增援生成器的 dispatcher。後三個若維持原版 60–240 秒，即使前三個已加速，AI 出場仍會變慢。曾測試 `1000..2000 ms`，實機出現電腦不再重生，因此已否決；該暫行狀態可辨識並自動遷回 5–10 秒。
- 定居生成器的 default 與 0/1/2/3 現存政黨分支機率都改為 101，使有合格隊伍時必定觸發。單人模式的 occupied mask 固定保護玩家 team 0，`pickTeam` 只會從尚未占用的 CPU team 1–7 選擇，因此結果上限是玩家加 7 個電腦（共 8 隊），且不會重複建立已占用隊伍。
- M8 將新局初始化的 `s_randRange(4, 2) -> v70` 改為 `s_randRange(4, 4) -> v70`，讓 type-1 村莊型 AI 固定為 4，並保留獨立的 type-4 軍事定居 AI 作為第五個定居對手。停用時還原 2～4 隨機值；舊版 `s_randRange(3, 3)` 狀態可自動遷移。此值在開局時即寫入狀態，因此不改寫既有存檔。
- 聚落模板：`MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`（解壓後為 INI 文字）主建築
  （namedef 含 `_Haupt`；羅馬為 `Hauptzelt`）的 `resv` 由 `0,0,0,0,0,0` 改為
  `614,300,372,250,460,288`（各欄取原版戰役 AI 聚落實測最大值），加速村莊型
  AI 起步；還原時改回全零。MP_* 下的同名模板不動，維持 ENDL 範圍一致。

舊版 `112,272` gate bypass 已否決：它會配合過快 loops 耗盡每隊 20-slot NPC job table，造成後期不再補兵。現行 migration 一律恢復 `66,0`。

全域 CLAK 經濟 patch 部分否決：`ak_npc.bci`（自由平民保留）與 `ak_produktion.bci`（生產閘門）的舊修改會讓玩家資源建築停止生產，一律還原、不啟用。

`ak_haupthaus.bci` 的轉換人數修改（`0x3FCC` 處 `[81,59] -> [66,20]`，特徵碼唯一命中）已重新啟用並跟隨 AI Ultimate 開關：`s_createBattleUnitsMax` 的最後一個引數由「推入變數 59」改為「推入常數 20」。EXE 端實作 `FUN_005249d0` 證實該引數是每支戰鬥部隊的成員數（EXE 鉗制 0..20），且每次呼叫會收集最多 100 名閒置村民、按此人數分批全部轉換（一批 = 一支部隊）。實際運行時原版值為 6，即玩家觀察到的 6 人小隊。玩家手動轉換 UI 是否受影響仍需一次實機回歸驗證。

2026-07-03 修正：實機測試顯示僅改 `ak_haupthaus` 後 AI 轉換仍為 6 人。追查確認該呼叫位於 `var57 == 34`（CIVRECREATE_WAIT）分支，只在軍事增援重建鏈觸發；村莊型 AI 日常的「村民轉部隊」實際走 `Dorfverteidigung.bci` 的四個 `s_addNPCJob_createUnit(team, 1, type∈{1,2,6,3}, 0, 0, 6, 6, 1, 0)` 呼叫（pushsym 位於解壓後 `0xF1BC/0xF264/0xF30C/0xF3B4`）。引數 6/7 是每支部隊的成員數下限/上限（job `+0x11/+0x12`；arg2=1 時 EXE 鉗制 1..20），EXE job 執行器（約 `00548700`）按該範圍收集閒置村民後呼叫 `FUN_00523a00` 一次——一個 job 產生一支 N 人部隊。這同時證實 `ak_level.bci` 軍事 job 的 `4..4 -> 20..20` 是每隊成員數而非部隊數。AI Ultimate 現在以特徵碼 `[66,0, 66,1, 66,?, 66,?, 66,0, 66,0, 66,?, 66,1, 90,8, 128,157, 73,-9, 86]`（強制恰好 4 處命中）把八個字面值 `6 -> 20`，還原時改回 6。已於 2026-07-03 實機驗證：套用後村莊型 AI 確實一次轉換 20 名村民為一支部隊。

2026-07-01 曾從 `ESAVE_000`／`ENDL_002` 的 `CLAK\scr.dat` 取出內嵌 `ak_level`，與 live BCI 做 exact SHA-256 比對，兩者相同。2026-07-02 的存檔證據（所有參戰隊伍 `npcActive=0`、全部 NPC job slots 空閒、`0x17F38` 已為 5000 仍無人復活）推翻了「`0x17F38` 是村落型重生計時」的舊解讀：該位置實為 type-5 處理器的 RETREAT_INIT 期限。2026-07-03 新的 `ESAVE_000` 現場則證明六處全改 5000 會造成另一個回歸：team 3 已重新 active，但仍保留 71 筆舊村莊資料與舊基準座標，城牆尚未清除就已重生。完整狀態機顯示聚落型 `0x10700`、`0x160EC` 是 state 51/52 清理條件的保險期限，不可縮成 5 秒；目前只加速其餘四處，聚落型兩處維持 600000。`ak_npc.bci` 的重新啟用路徑為原生內建，存檔檔案一律不碰。

目前仍需五張 ENDL 地圖的長時間 late-wave regression test；短期成功不能標成完整 runtime verified。

2026-07-03 現場診斷紀錄：目前 `ESAVE_002` 是 `ENDL_002`，存檔時間為
11:04:54；五張 live `ak_level.bci` 約在 11:18:13 才重新寫入，因此存檔早於
最後一次套用。`CLAK\scr.dat` 內嵌腳本雖已有軍事 cooldown 5000、六處撤退期限
5000、死亡確認 3 tick、recycle 1、count 20、門檻 8、gate `66,0` 與六個生成
機率 101，但前三個 polling loops 仍是已否決的 `1000..2000 ms`。此狀態已知會
讓電腦重生停滯；套用程式不會回寫既有存檔的內嵌腳本，所以舊局不能拿來驗收
5–10 秒修正版。

同次唯讀檢查發現 live `ENDL_002` 解壓 payload 與存檔不相同（save SHA-256
`4dd021f2f86336e6fc61a269c677ef7403cad833494d467c8fe1cd5580f771a2`；live
SHA-256 `49839eb76743893b879be201c729c8104c09415acccc29928fbcea29eee02429`），相差
42,885 bytes，且 live payload 無法通過正常 BCI opcode／特徵碼結構辨識。即使它
可自我 decompress→compress→decompress 相等，也只代表能重現既有異常資料，
不代表遊戲相容。下一步必須先用可信原版基線還原五張 live 腳本、重新套用並逐張
解壓驗證特徵碼，之後開新無盡局測試；禁止直接修改存檔 `CLAK\scr.dat`。

## 11. EXE 補丁

### 11.1 失去焦點時繼續執行

- offset `0x161a88`
- original `89 15 C4 7D 9E 02`
- patched `90 90 90 90 90 90`

只接受已知 signature；Unknown EXE 不寫入。

### 11.2 村莊建造範圍與紅色虛線框

現行 patch 使用 setter trampoline：

- hook `005364c1`／file offset `0x1364c1`
- cave `0056258f`／file offset `0x16258f`
- 以 `value * 5` 產生 5x
- 保留負值檢查、呼叫 `004c0900`、回到 `005364d1`

狀態包含 Original、Legacy2x、Legacy2Point5x、Legacy3x、Expanded5x、Unknown；舊版狀態必須可偵測、升級與回復。

舊四處 shift-6 → shift-7 patch 已否決：

- `0x1366c4`, `0x1366cd`
- `0x0d722c`, `0x0d723b`

現行程式不再寫入舊 `07` bytes，只偵測並還原。相同 setter 路徑在 3x 時已完成建造範圍與紅框同步的實機驗證；5x 倍率目前完成程式碼與 bytes 測試，仍待遊戲內確認。

### 11.3 法術免除祭壇數量需求

- 祭司解鎖四個法術對獻祭所（祭壇，`OD_BAUOPF`）數量的限制位於 `FUN_0044a010` (VA `0x0044a010`)。
- 該限制為硬編碼於 EXE 的 `cmp esi, N` 指令位元組，其中 N 為 1、2、3、4。
- 修改方案是將這 12 處（3 陣營 × 4 法術） cmp 指令的第 3 個位元組（立即數）修改為 `0x00`，即完全免除祭壇數量限制。
- 檔案偏移量 (File Offsets)：
  - 條頓 (`FigGerPri00`): `0x4A112` (法術 1), `0x4A136` (法術 2), `0x4A15A` (法術 3), `0x4A0E3` (法術 4)
  - 塞爾特 (`FigKelPri00`): `0x4A1CC` (法術 1), `0x4A293` (法術 2), `0x4A2B7` (法術 3), `0x4A249` (法術 4)
  - 匈奴 (`FigHunPri00`): `0x4A329` (法術 1), `0x4A3F0` (法術 2), `0x4A414` (法術 3), `0x4A3A6` (法術 4)
- 偵測機制：必須 12 處原始位元組完全匹配或修補位元組完全匹配；任何混合或未知狀態皆判定為 Unknown 並跳過，以確保還原與寫入安全性。

### 11.4 遊戲整體運行速度（加速器）

- **狀態：已實機驗證（2026-07-05）**。套用後移動／生產／戰鬥／AI 一起以所選倍率加速，功能正常。
- 動機：原版整體時脈偏慢，逐項測試各修改功能耗時。此功能等同「遊戲加速器」（speedhack），
  以固定倍率縮放遊戲主時脈，使移動／生產／戰鬥／AI 一起變快，而非改單一子系統設定。
- 逆向定位：遊戲主時脈函式位於 VA `0x55e530`，回傳「奈秒級的當前時間(double)」，只有 3 個
  呼叫者（全部集中在 `0x566b53` 附近的主迴圈計時碼）。它有兩條路徑：
  - **QPC 路徑**（`QueryPerformanceCounter`，現代機器走這條）：啟動時由 `QueryPerformanceFrequency`
    算出 `scale = 1e9 / freq` 存入 `.bss 0x29e89c0`；`1e9` 常數位於 rodata VA `0x604214`／檔案偏移 `0x204214`。
  - **timeGetTime 路徑**（退回）：`time = (timeGetTime - baseline) * 1e6`；`1e6` 常數位於 rodata
    VA `0x60424c`／檔案偏移 `0x20424c`。
- 兩個常數各自經 xref 確認「只被時脈碼引用一次、單一用途」（`0x604214` 僅在 `0x55df8f`，
  `0x60424c` 僅在 `0x55e588`），因此同步把兩個 double 乘上相同整數倍率 `s`，即讓整個時脈以 `s`
  倍速前進，不論玩家機器走哪條路徑。差值運算在遊戲自身邏輯中先減基準再乘常數（浮點），無
  32-bit 溢位風險，時間仍由 ~0 開始、無跳變。完全可逆：常數改回原值即還原。
- 實作：`ExePatchModel.GetGameSpeedMultiplier`（回傳 1～10 的整數倍率，兩路徑不一致或非整數倍回傳
  0=Unknown 即不動作）與 `PlanGameSpeed(desired, current)`，倍率位元組以 `BitConverter.GetBytes(base * s)`
  動態產生，經 `VerifiedBinaryWriter` 逐一驗證預期位元組後寫入。支援倍率清單為
  `ExePatchModel.GameSpeedSupportedMultipliers = {1..10}`，UI 下拉選單（原版／2×～10×）由此陣列動態產生，
  套用前先由備份還原 EXE，故一律從 1× 乾淨狀態切換。
- 使用範圍：**最高開放到 10×**，但建議由 2×～3× 開始測試再逐步調高。倍率越高越可能撞到遊戲主迴圈的
  單幀最大 delta 上限而不再加速，或造成物理／碰撞不穩、音效（走系統時脈）不同步。這些皆可實測 A/B 確認；
  「全開」預設仍固定在 3×（最穩範圍），10× 需玩家自行於下拉選單選取。
- 已知邊界：另有一組「raw ms」讀取器（`0x55e520`，7 個呼叫者，`timeGetTime - baseline` 直接回傳
  整數毫秒、無常數可改），推測供 UI／動畫／輸入等非模擬用途。若日後實測發現某子系統只加速一半，
  即為該路徑未被縮放，屆時需改用 API-hook（注入）方案才能全域一致。
- **與 BCI 腳本毫秒常數的關係（2026-07-05 追蹤確認）**：BCI 腳本內所有「等待 N 毫秒」型的
  deadline 判斷（例：`v61 := s_getTime() + 600000`，見 §11.1 各修改）皆呼叫 host function
  `s_getTime()`，其實作鏈為 `s_getTime → 0x5098d0 → 0x566b90`，該函式直接呼叫本節 patch 的主時脈
  `0x55e530` 並乘上 `0.001`（ns→ms，常數位於 VA `0x60673b`，未被本功能改動）換算成整數毫秒回傳。
  **因此本加速器對所有 BCI 毫秒常數（P3/P4/P6/P11 等既有修改的目標值）具有乘法疊加效果**：
  例如「5000ms 重生」在時脈倍率 `s` 下，實際只需真實時間 `5000/s` ms 即觸發，兩者不是互斥而是相乘。
  另發現遊戲另有一組獨立的 `s_setTimeFactor`／`s_getTimeFactor`（全域變數 `0x77186c`，一般浮點縮放
  值），其實作未參照 `s_getTime()` 或主時脈，與本功能無關，尚不知其實際用途。

## 12. 強制英文與語言回復

強制英文開關是手動、預設關閉。語言 overlay 原版基線位於 `<gamePath>\.against-rome-modifier-language-backup`。

回復合約：

- overlay 存在且 baseline/manifest 缺失：中止並明確報錯。
- baseline 完整：精確回復原檔。
- 原版不存在的 overlay-only 檔案：回復時刪除。

不得把 active `ToEng` 檔當成原版。真實安裝曾有 332 個 overlay 檔但無 baseline，而當時 `Backup.zip` 的 146 entries 不含這些目標。修復工具為 `tools/Repair-LanguageBackup.ps1`。

## 13. 存檔與 VirtualStore

唯一 live save root 是 `<gamePath>\SAVE`。Windows 在非提升狀態啟動舊遊戲時，可能把寫入導向 `%LOCALAPPDATA%\VirtualStore\Program Files (x86)\Against Rome\SAVE`；修改器本身要求管理員並把遊戲 `WorkingDirectory` 設為所選根目錄，但外部非提升捷徑仍可能重建 VirtualStore。

ZIP 備份先建立 `.tmp`，加入修改器產生的 `manifest.json`，成功後再 move。ZIP entry 禁止 absolute path、leading slash 與 `..` traversal。存檔 restore 成功後先 commit，再清理 staging；cleanup 失敗只記錄。

## 14. dgVoodoo2

修改器內嵌 x86 `D3D8.dll`、`DDraw.dll`、`dgVoodooCpl.exe`、`dgVoodoo.conf`。它不下載 runtime dependency，也不覆蓋非受管 DLL。遊戲根目錄的 manifest 記錄受管檔與 hash；使用者改過的受管檔不會被無聲刪除。來源、版本與 SHA-256 見 `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`。

## 15. UI 與 preset

- `mainTabControl` 的 header 故意隱藏，左側按鈕負責導航。
- `StyleNavButton` 綁定前必須先建立對應 `TabPage`。
- `settingsLayout` 第一列放置 `pnlNumericCard`（系統）、`pnlSwitchesCard`（資源）與 `pnlBuildCard`（建設）三欄；第二列的 `pnlAiCard` 橫跨三欄，容納 M1–M14 並依寬度換行。
- `pnlTipsCard` 指南卡片拆分為左右雙欄，左半部 `lblTipsContent` 顯示操作指引，右半部 `lblTipsDetail` 顯示功能詳細說明，以避免說明文字過長導致的高度截斷問題。
- 新 toggle 必須同步 UI field、localization、apply、restore、state detection 與文件。
- 移除舊有的 `.arpreset` 全域設定檔匯入／匯出功能，改由一鍵「所有功能開啟」與「所有功能關閉」按鈕控制所有開關狀態。
- `.artroop` 目前有 9 個屬性；舊短格式缺欄位時使用 fallback。

## 16. 逆向工程工作流

查詢順序：

1. `docs/reverse-engineering/`
2. `data/game_schema.json`
3. `re_workspace/ghidra_inventory/against_rome_function_index.csv`
4. `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`
5. 必要時新增 focused script 到 `tools/re/`

`re_workspace/` 是本機證據與產物，禁止上傳；`tools/re/` 的可重現 scripts 可以公開。完整反編譯不等於取得原始碼，不能還原原始識別字、註解或 build system。

## 17. 未完成項目

- AI Ultimate 五張 ENDL 地圖的長時間回歸。
- `apt.dat` 的安全格式與用途。
- BCI opcode 的完整解碼。
- `[volkres]` 多個 candidate 欄位。

## 18. 驗證

基本驗證：

```powershell
dotnet build .\AgainstRomeModifier.csproj -c Release --no-restore
dotnet test .\tests\AgainstRomeModifier.Tests\AgainstRomeModifier.Tests.csproj -c Release
Get-Content .\data\game_schema.json -Raw | ConvertFrom-Json | Out-Null
git diff --check
```

`tests/AgainstRomeModifier.Tests`（xUnit）以合成 fixture 取代真實遊戲檔案，因此在乾淨 clone、無 `Backup.zip`／遊戲目錄的 CI 環境下也能執行（見 `.github/workflows/ci.yml`）。舊的 `tests/verify_split_patches` console 專案依賴本機 `遊戲原始檔案/`，僅供本機手動比對使用，不在 CI 內執行。

依修改類型追加：

- PFIL：壓縮／解壓 round-trip。
- `objdef.dau`：解壓長度完全相等、短 row bounds。
- EXE：original/current/legacy/unknown 四種 state；`ExePatchModelTests` 涵蓋失焦補丁、法術祭壇、村落建造範圍舊版候選還原、村落 setter（2x/2.5x/3x）各狀態的 enable/disable round-trip、遊戲整體運行速度（1～10× 偵測、兩路徑不一致與非整數倍判 Unknown、任意倍率間切換與還原、誤判狀態時中止），以及預期位元組不符時中止且不寫壞緩衝區。
- ENDL：五張 map、enable/disable/migration、長時間 waves、save embedded script。
- 語言：manifest、數量、path safety、SHA-256、缺 baseline abort。
- 存檔：完整 `.tmp` ZIP、manifest、path traversal、commit 後 cleanup。

## 19. 公開版本邊界

不得提交：原始遊戲資料、`Backup.zip`、`MAPS/`、`SYSTEM/`、`SAVE/`、`ToEng/`、原始遊戲封存、本機語言 baseline、`.codex/`、`.agents/`、`re_workspace/`、內部 AI 稽核、工具下載 cache。

可以提交：C# source、README、技術文件、schema、`tools/re/` scripts、已記錄來源與 hash 的 dgVoodoo2 檔。未獲使用者明確要求時，不 commit、不 push、不 rewrite history、不 force-push；本 repo 也不應使用 `git push --mirror`。

## 20. 建議的代理工作順序

1. 讀本文件（`TechDoc.md`）與 `data/game_schema.json`。
2. 讀特定功能對應的 `docs/reverse-engineering/*` 說明文件。
3. 檢查工作樹差異，分離既有變更與新開發的變更。
4. 以穩定識別字定位代碼位置，不要過度依賴歷史行號。
5. 在撰寫變更前，先明確定義：原始 state 匹配、目標 state、unknown state 的安全拒絕行為、以及 restore 的還原方式。
6. 保持小步前進與局部修改；不要在無關區域進行大範圍重構。
7. 跑與所改區域風險相稱的 build、schema、round-trip、或 byte/state 比對測試。
8. 變更完畢後，同步更新代碼、README、技術文件、補丁細節與 schema。
9. 最後提交前重新審視 diff，確保沒有覆寫使用者現有工作。

## 21. 歷史除錯案例與正確處理方式

| 症狀 | 根因 | 錯誤處理 | 正確處理 |
|---|---|---|---|
| 命令從 `C:\` 掃描並大量 access denied | 中文 cwd 未被沙箱採用 | 繼續進行盲目的全域遞迴搜尋 | `Get-Location`、使用絕對/完整路徑、`git -C`、直接定位目標檔 |
| Git 回報 dubious ownership | sandbox user 與 repo owner 不同 | 修改全域 safe.directory 設定 | 單次使用 `-c safe.directory=...` 參數 |
| Build 讀不到 NuGet.Config | sandbox 權限限制 | 認定 C# 專案損毀 | 先用 `--no-restore` 建置；必要時核准讀取使用者設定 |
| UI 效果不顯示 | 初始化順序導致 null page | 刪除「看似沒用」的 styling 設定 | 先查 UI 建立順序與事件 wiring |
| Toggle 又自動變回 live 狀態 | constructor 與 load path 同時寫 Checked | 只修改單一地方 | 同時檢查初始化、event、`LoadCurrentData`、以及 preset 相互干擾 |
| 免費模式仍有錯誤退還 | 成本與退還欄位重疊 | 只清 obvious cost 或清掉 19..24 欄 | 用原版資料核對完整欄位分組並保留 equipment refund (19..24) |
| AI 前期正常、後期不補兵 | 20-slot NPC job table 耗盡 | 再提高 concurrency 或直接 bypass gate | 設定有界的 active=8、gate 恢復原值、實施 completed job recycle，並進行長時間測試 |
| AI bytes 已改但玩家感覺沒變 | save 中已有排程／timer | 再次盲改 offsets 常數 | 比對 live BCI、save embedded BCI、與遊戲 runtime 排程狀態 |
| 語言回復丟 InvalidOperationException | overlay 存在但原版 baseline 缺失 | 移除防護 guard 或直接將 overlay 檔案視為原版 | 保留 loud guard，重建並藉由雜湊驗證 baseline 正確性 |
| 存檔 ZIP 半成品殘留 | 直接寫入正式 ZIP 檔案 | 捕捉例外後直接繼續 | 先將資料寫入 `.tmp` 暫存檔，完整建立後再 move/rename |
| restore 後 cleanup 失敗導致整體回滾 | cleanup 程序在 commit 之前執行 | 把刪除 staging 當作核心交易 | 先進行 commit，再對 staging 目錄執行 best-effort cleanup |
| 廣泛 source patch 套不上 | 亂碼／行尾／context 漂移 | 擴大 patch context 範圍 | 使用 ASCII 穩定識別字小範圍 patch，先用 `Select-String` 檢驗 |
| 文件把候選當事實 | 只看到 Ghidra decompile | 直接命義並寫成 patch | 結合 call-path + bytes + runtime 證據分層記錄 |
| `.gitignore` 有英文規則但遊戲檔仍出現 | 真實資料夾是本地化名稱 | 假設已忽略成功 | 對照實際 on-disk name 與 `git status --ignored` 的真實輸出 |

## 22. 歷史修改時間線

### 2026-06-22：免費生產與 Git 清理
- 修正 `ress.ini` 生產／退還分組；保留 `FigTiePac00_Packpferd`。
- 移除 healing 修改與 UI 敘述。
- 發現 `.codex/`、`re_workspace/` 不應納入版本庫；保留可重現的 `tools/re/`。

### 2026-06-23：UI、逆向資料庫、rollback 與初版 AI Ultimate
- 移除已證實無引用的 UI helper；修正 sidebar highlight 初始化順序。
- 建立 `docs/reverse-engineering/`、`data/game_schema.json` 與多個 Ghidra scripts。
- 公開建置改為 `Backup.zip` optional，並支援從使用者安裝建立基線。
- 修正 rollback commit/Dispose 語意、簡單逗號格式相容、weapon slot bounds。
- 建立 AI Ultimate toggle，開始區分 `team.dat` 與 `ak_level.bci` 的責任。
- 記錄騎乘村民／戰鬥單位 UI 計數不是單純資料欄位問題。

### 2026-06-24 至 2026-06-25：AI active-limit 實驗與 UI 移動
- 曾加入 active gate bypass；後續證明會導致隱藏資源耗盡，現在只保留遷移／回復紀錄。
- balance toggle 移入核心開關區，保留原事件 wiring。
- 針對「完整反編譯、DX12、大改架構」只做可行性評估；沒有把 Ghidra output 當原始碼。

### 2026-06-27：dgVoodoo2 內嵌
- 從下載／cache 構想改成建置時內嵌與受管 install/remove。
- 建立所有權 manifest、衝突拒絕和使用者修改檔保護。

### 2026-06-28：AI 晚期回歸、清理與回復強化
- 找到 20-slot NPC job table 是 late-run stall 根因。
- 改為 count 20、respawn 5s、active 8、gate 原值、job recycle。
- 強化語言 baseline、存檔回復 commit 後清理與 dead-code 驗證。
- 舊 checkout 當時移除過無效的 village-range UI；此結論已被後來 live checkout 的 setter patch 取代，不能拿來刪除現行功能。

### 2026-06-29：路徑、VirtualStore、稽核交接
- 確立 `<selected game path>\SAVE` 是唯一 live save root，並整理 VirtualStore 歷史存檔。
- 稽核所有 write/delete path，補上 restore game-root 與 ZIP traversal guard。
- 對 `CodeAuditReport.md` 的 AI claim 逐項查 code，只修已證實問題。
- backup ZIP改用 temporary archive + generated manifest；避免 lock 內 logging。

### 2026-06-30：搬移工作區、語言修復、村莊範圍與 GitHub 稽核
- 唯一 live checkout 搬到離線中文路徑；舊 OneDrive 只留歷史參考。
- 完成 332-file 語言 baseline 重建工具與 hash 驗證流程。
- 村莊 setter 的 2x 實機路徑與紅色虛線框同步行為獲確認；舊四處 shift patch 正式列為 rejected。
- 發現本地化原始遊戲資料夾約 1.42 GB、4623 files，必須保持 ignored/local-only。

### 2026-07-01：2.5x、手動 toggle、人口容量、公開邊界與 save readback
- setter trampoline 升級為 2.5x，保留 `Legacy2x` detection/migration；新倍率已完成實機確認。
- balance toggle 不再重讀 live files；強制英文 toggle 改為 manual、default off。
- 新增全 22 個正 `wohnwer` rows 的 20x switch，整合 preset/apply/restore/detection/docs。
- AI Ultimate 移入 core switches，沒有改 patch 語意。
- README／技術文件／ignore／dgVoodoo provenance 更新為公開發佈邊界。
- 讀取 `ESAVE_000` 的 ENDL_002 embedded script，證明 save 與 live BCI exact match。

### 2026-07-02：文件整合與既有重構核對
- 發現 `TechDoc.md` 與兩個未追蹤摘要是亂碼，重建乾淨 UTF-8 中文技術文件。
- 整合上述歷史為本手冊。
- 核對既有 in-memory patch generation／stats layering 重構可編譯。
- 明確記錄其不是完整跨檔原子交易，避免後續代理高估安全性。

### 2026-07-02（第二次會談）：全欄位位元組級稽核、LICENSE、發佈清理
- 以比對原版實機安裝與 `遊戲原始檔案\`，驗證修改器所寫的每個位置。
- 獨立小工具進行 `DecompressPfil`→`CompressPfil` 驗證，對 `objdef.dau`、`ress.ini` 等所有關鍵檔做 round-trip，位元組完全相等。
- 驗算村莊 setter 3x trampoline 的組語在 bytes 層級正確無誤。
- 證實 `objdef.dau` 內確實有 22 個正值 `wohnwer` 行。
- 掃描五張無盡地圖腳本，特徵碼全部命中且初始值正確。
- MIT LICENSE 新增並推送。

### 2026-07-02（第三次會談）：實機驗證建造速度與主堡倉庫存量 10 倍
- 完成 `objdef.dau` 建造/升級/維修 10 倍加速，以及主堡與倉庫儲存量 10 倍的實機驗證。
- 確認建造/升級時間縮小為 1/10，且修復速率相應提升 10 倍。
- 確認主堡（Haupthaus）與倉庫（Lager）資源儲存上限正確提升 10 倍。

### 2026-07-02（第四次會談）：將村莊建造範圍由 2.5 倍升級至 3 倍
- 將村莊建造與紅框範圍補丁的組語乘數從 2.5 倍升級至 3 倍（LEA 縮放與 NOP 補齊）。
- 更新狀態偵測列舉（Expanded3x）、語意字典與元數據。

### 2026-07-04：新增法師技能與復活術強化功能
- 實作傷害法術提升 5 倍，治療法術提升 50 倍的數據修改邏輯。
- 將 `SYSTEM/CLAK/cl_scint.ini` 納入備份與修改機制，動態替換復活召喚的 ODef 別名（男屍體轉槍盾兵 `KEL_INF01`，女屍體轉女雙劍士 `KEL_INF02`）。
- 實作復活後的單位生命值與士氣設定值為 100%。

### 2026-07-05：patch 邏輯抽離、xUnit 測試、CI 與 EXE 補丁測試補強
由 Codex 完成主要重構，經後續稽核與補強：

- 把 `ModifierForm.Patches.cs` 中的純位元組計算邏輯抽到 `src/Core/Patches/`（`ObjdefPatcher`、`RessPatcher`、`ClScriptPatcher`、`ClEparaPatcher`、`ClScintPatcher`、`TeamDatPatcher`、`VerifiedBinaryWriter`），不依賴 WinForms；`ModifierForm.Patches.cs` 由 ~2175 行降到約 1300 行，只負責把 UI 狀態轉成 Options 並委派。核對確認 `SelectValidatedPatchBase` 忠實保留了舊碼「優先讀遊戲目錄現有檔案做增量修改、結構驗證失敗才退回記憶體備份」的行為，非新增邏輯。
- 新增 `tests/AgainstRomeModifier.Tests`（xUnit，`net8.0-windows`），以合成 fixture（非真實遊戲檔案）驗證各 patcher 的 round-trip 與關鍵欄位值，可在乾淨 clone／CI 上執行。
- 新增 `.github/workflows/ci.yml`（windows-latest：restore／build Release x64／test）。
- 為每個直接寫固定偏移的 patch 加上落地前驗證：`VerifiedBinaryWriter.WriteBytes`／`BciPattern.WriteBciInt32` 在目前位元組不符預期時丟 `InvalidDataException` 並中止，不靜默寫入，防止對不同版本的 exe/bci 檔案寫壞。
- 主 csproj 曾對內嵌資源 `ak_anfuehrer.patched.bci`（位於被 gitignore 的 `re_workspace/`）加上 `Condition="Exists(...)"`；該功能後續因實機閃退回歸而完全撤出修改器，csproj 已不再內嵌此資源。
- 稽核發現「EXE patch 的 state → 預期/取代 bytes 選擇邏輯」（`ApplyExePatch`／`ApplyVillageSetterRangePatch`／`RestoreLegacyVillageBuildRangePatch`）雖然手動核對無誤，但完全沒有測試覆蓋。補強做法：新增 `src/Core/Patches/ExePatchModel.cs`，把失焦補丁、法術免祭壇需求、舊版村落建造範圍候選還原、村落 setter（2x/2.5x/3x 跳板）的常數、enum、狀態偵測與「state → `ExeWriteOp` 清單」規劃邏輯全部抽成純類別；`ModifierForm.Patches.cs`／`ModifierForm.Data.cs` 改為委派呼叫，只保留日誌與在地化。新增 `tests/AgainstRomeModifier.Tests/ExePatchModelTests.cs`（24 個測試，合成 exe 緩衝區），涵蓋四組補丁各自的狀態偵測、enable/disable round-trip、村落 setter 由任一舊版狀態遷移到 3x 再還原、以及預期位元組不符時中止且不破壞緩衝區。測試總數由 9 提升到 33。

### 2026-07-05：首領死亡榮耀保留功能撤出
- 實機測試確認：嘗試完全停用「首領死亡榮耀保留」行為後，遊戲會閃退；此功能視為無法安全修改。
- 修改器已移除 UI 選項、內嵌 patched BCI 資源、套用／偵測邏輯與還原邏輯；「所有功能開啟」、一般套用、完整還原與屬性還原均排除此功能。
- 修改器不再提供或套用首領死亡保留榮耀功能。由於舊版實驗腳本已確認會在戰鬥流程造成存取違規閃退，修改器啟動及套用／還原食物治療補丁時會精確辨識該舊版 `ak_anfuehrer.bci`（含治療值 1／10 兩種狀態），先以內嵌原版檔重建，再套用仍受支援的治療數值補丁。其他未知或第三方腳本不會被此遷移覆寫。
- 閃退根因尚未定位。未來若重新實作，必須完成實機驗證；編譯成功與靜態檢查不足以證明安全。

### 2026-07-05：村莊建造／紅框範圍升級為 5 倍
- setter trampoline 的 X/Z 縮放由 `LEA value*3` 改為 `LEA value*5`，hook、call 與 return 位址不變。
- 原 3x cave 納入 `Legacy3x` 偵測，可直接遷移至 5x，也能安全還原原版。
- 5x 已通過合成 EXE state、遷移及 round-trip 測試；實際遊戲內建造邊界與紅色虛線框仍待確認。

### 2026-07-05：新增「遊戲整體運行速度」加速器（EXE 主時脈縮放）
- 需求：使用者要的是「遊戲加速器」（speedhack）式的整體加速，而非改單一子系統設定；動機是原版
  時脈偏慢、逐項測試各修改功能耗時。
- 逆向：以 PE import + capstone 反組譯定位主時脈函式 `0x55e530`（回傳奈秒 double，3 個主迴圈呼叫者）。
  QPC 路徑的 `1e9`（VA `0x604214`／偏移 `0x204214`）與 timeGetTime 路徑的 `1e6`（VA `0x60424c`／偏移
  `0x20424c`）各自只被時脈碼引用一次；同步乘上倍率即全域加速。詳見 §11.4。
- 實作：`ExePatchModel` 新增 `GetGameSpeedMultiplier`／`PlanGameSpeed`（動態產生 double 位元組、
  `VerifiedBinaryWriter` 驗證寫入、兩路徑不一致或非整數倍判 Unknown 不動作）；UI 於「系統與相容性設定」
  卡片加下拉選單（原版／2×／3×／4×），接入 apply／restore／`LoadCurrentData` 偵測與「全開/全關」預設
  （全開預設 3×）。新增 12 個 `ExePatchModelTests` 案例，測試總數 39 全過，Debug 建置 0 警告 0 錯誤。
- **UI 未生效修正**：實際生效的版面邏輯是 `ConfigureSettingsCard`（把卡片高度鎖死、只排版傳入的開關
  清單），初版把加速選單擺在超出卡片高度的座標，被裁到視窗外看不見。改為把卡片高度加一列（230→278）
  並新增 `ConfigureGameSpeedRow` 沿用同一套列高公式把選單接進這層真正的版面系統，才真正顯示出來。
- **倍率上限提升至 10×**：使用者要求「最高加速到 10 倍」；`GameSpeedSupportedMultipliers` 由 `{1,2,3,4}`
  擴充為 `{1..10}`，UI 下拉選單改由此陣列動態產生選項（不再寫死 2×/3×/4×）。新增 1～10× 偵測與
  1×↔10× round-trip 測試，測試總數 60 全過。「全開」預設仍固定 3×（最穩範圍），10× 需玩家自行選取，
  且風險提示已更新為「越高越可能撞單幀 delta 上限或不穩，建議由低倍率開始測試」。
- 尚待實機驗證：需在遊戲內確認各倍率（尤其 5×~10×）實際加速效果、是否撞到單幀 delta 上限或物理不穩，
  以及 `0x55e520` raw-ms 路徑（非模擬用途）是否造成任何子系統只加速一半。
- **實機驗證結果（2026-07-05）**：使用者已於實際遊戲中測試「遊戲整體運行速度」功能，確認套用後移動／
  生產／戰鬥／AI 確實一起以所選倍率加速，功能運作正常。此功能自逆向、實作到實機驗證已全數完成，
  §11.4 所述的兩常數同步縮放方案視為 Static + 實機雙重驗證。
