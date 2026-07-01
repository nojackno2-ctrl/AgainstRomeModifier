# Against Rome 修改器完整技術文件

本文件描述目前程式碼、資料格式、反編譯證據、已啟用補丁、候選功能與已否定方案。它不是版本更新紀錄；同一功能只保留目前有效的說明。若文件、程式碼與實機結果衝突，以可重現的實機結果及最新反編譯證據為準，並同步修正本文件、`docs/reverse-engineering/` 與 `data/game_schema.json`。

## 1. 文件規範與證據等級

### 1.1 編碼與編輯規範

- 本文件及專案 Markdown 使用 UTF-8，不得把終端機的亂碼或 `<truncated ...>` 標記寫回檔案。
- 遊戲解壓後的文字資料使用 Windows code page 1251；不得直接以 UTF-8 回寫遊戲資料。
- 欄位索引一律採 0-based。
- 數值範例使用 `原值 -> 修改值`，位元組使用十六進位。
- `TechDoc.md` 為中文技術文件，`TechDoc_EN.md` 為英文技術文件；兩者皆會嵌入編譯產物。

### 1.2 證據等級

- **穩定**：程式已實作，格式或位址有簽章檢查，且功能有實機或資料往返驗證。
- **已實作候選**：程式可寫入及還原，靜態證據完整，但仍缺少足夠實機覆蓋；介面必須明示風險。
- **僅讀取候選**：只記錄欄位或函式，不應自動寫入。
- **已否定**：靜態推論經實機測試不成立。保留精確證據是為了避免重複嘗試，修改器不得套用。
- Ghidra 的 `FUN_*` 名稱不等於已了解語意；必須有呼叫路徑、資料流、字串註冊或實機行為佐證。

## 2. 專案架構

| 檔案 | 職責 |
|---|---|
| `Program.cs` | WinForms 入口、系統管理員權限提升、High DPI 啟動與全域例外處理。 |
| `GameLZSS.cs` | `PFIL@` 包裝的 LZSS 解壓、壓縮、標頭重建與輸入邊界檢查。 |
| `TroopConfig.cs` | `ObjdefIndex`、`RessIndex`、兵種 ID、顯示名稱、陣營、階級、類型及平衡基準。 |
| `ModifierForm.cs` | 主視窗佈局、控制項、備份快取、單位資料快取與共用 UI 狀態。 |
| `ModifierForm.Data.cs` | 現況讀取、CSV-like 解析、數值比較、圖示載入與 EXE 狀態偵測。 |
| `ModifierForm.DataExt.cs` | 備份兵種列的安全索引與共用資料擷取。 |
| `ModifierForm.Patches.cs` | 寫入、還原、交易回滾、EXE、INI、DAU、team.dat 與 BCI 補丁。 |
| `ModifierForm.Presets.cs` | `.arpreset` 匯入/匯出與舊格式相容。 |
| `ModifierForm.SaveManager.cs` | SAVE 掃描、ZIP 備份、還原、刪除及 `BackupSaveCache`。 |
| `TroopPresetForm.cs` | 43 個兵種的 9 屬性編輯及 `.artroop` 匯入/匯出。 |
| `UIElements.cs` | `ModernToggle`、深色選單渲染及 GDI 資源釋放。 |
| `Localization.cs` | 中英文 UI 字串及操作日誌。 |

目標框架為 `.NET 8`、`net8.0-windows`、WinForms、x64、Nullable 開啟，High DPI 模式為 `PerMonitorV2`。`Backup.zip` 只有存在時才嵌入；技術文件固定嵌入。

## 3. 安全寫入與備份模型

### 3.1 原始資料來源

- 優先讀取嵌入的 `Backup.zip`。
- 公開儲存庫不應追蹤原版遊戲資產；若沒有 `Backup.zip`，修改器從使用者選定的遊戲安裝目錄讀取必要原檔並存入記憶體。
- `backupFiles` 使用 `StringComparer.OrdinalIgnoreCase`，避免 Windows 路徑與 ZIP entry 大小寫不同造成查找失敗。
- 備份是每次重建目標檔的基準，不代表每個還原動作都應覆蓋所有檔案。統計還原、相容性還原與語言還原是分離操作。

### 3.2 `FileRollbackScope`

每次套用或還原會建立一個交易範圍：

1. 第一次寫入某個目標檔前，記錄它是否存在及原始 bytes。
2. `SafeWriteAllBytes` 先在同目錄建立暫存檔，再以替換或移動完成寫入。
3. 任一後續步驟拋出例外時，依記錄恢復本次操作開始前的檔案狀態。
4. 所有步驟成功後才呼叫 `Commit()`。
5. 必須先 `Commit()`、`Dispose()` 並清空 rollback 物件，再呼叫 `LoadCurrentData()`，避免 UI 重新讀取期間仍持有暫存資源。

這是「單次操作回滾」，不是永久 `.bak` 系統。遊戲目錄不會持續產生大量 `.bak`。

### 3.3 套用順序

主套用流程固定為：

1. 驗證目錄存在且包含 `Against_Rome.exe`。
2. 載入原始備份並顯示確認對話框。
3. 在 UI 執行緒先複製所有控制項值，背景工作不得直接讀 UI 控制項。
4. `ApplyExePatch`。
5. `ApplyClScriptPatch`。
6. `ApplyRessPatch`。
7. `ApplyObjdefPatch`。
8. 還原全部原始 `team.dat`，再套用目前人口設定。
9. `ApplyEndlessAiUltimateModePatch`。
10. `ApplyLanguagePatch`。
11. 成功後提交交易、重新讀取現況。

此順序可避免舊補丁殘留，也避免修改 A 時把使用者在同一次操作中選擇的 B 覆蓋掉。

## 4. `PFIL@` 與 LZSS

### 4.1 已知使用者

- `SYSTEM/ress.ini`
- `SYSTEM/cl_script.ini`
- `SYSTEM/banner.ini`
- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
- `MAPS/**/team.dat`
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`
- 部分 endless settlement `.sdl`

### 4.2 實作要求

- `GameLZSS.DecompressPfil` 分離原始包裝標頭與壓縮 payload。
- `GameLZSS.CompressPfil` 重用原檔標頭所需資料並更新大小。
- 解壓大小若為負數或超過 50 MB，視為無效輸入，避免記憶體炸彈。
- 4096-byte 環狀視窗使用 `& 4095`。
- 壓縮器使用 16-bit hash table、hash chain 與有限搜尋深度；短位移重複匹配必須避免引用尚未更新的環狀視窗資料。
- 修改後至少要驗證 `decompress(compress(payload)) == payload`；`objdef.dau` 另有文字長度不變限制。

### 4.3 CSV-like 相容規則

遊戲不是 RFC 4180 CSV parser。現在使用 `Split(',')` 與 `string.Join(",", cols)`，以保留舊引擎可接受的格式。不得自動加雙引號或 escape，否則物件 ID、路徑與建築按鈕可能無法辨識。原始換行格式與尾端空欄必須保留。

## 5. 語言資源覆蓋

勾選英文資源時，修改器從所選遊戲目錄的 `ToEng` payload 覆蓋對應遊戲檔。首次受管理的覆蓋前，會把 332 個目標檔案的原始狀態保存到 `.against-rome-modifier-language-backup`；取消時依該基線完整恢復，原本不存在的檔案會刪除。若 `ToEng` 不存在、沒有檔案，或舊版修改器已覆蓋全部檔案卻沒有留下基線，操作會中止而不再顯示假成功。主要範圍：

- `SYSTEM/TEXT/US/`：`opt.put`、`g_mscr.put`、`g_bann.put`、`g_brief.put`、`g_kamp.put`、`debriefg.put`、`objnames.put`、`g_volk.put`、`netdlg1.put`、`netdlg2.put`、`nettexts.put`、`msgbox.put`、`g_tut.put`。
- `SYSTEM/CLMK/DLG/`：結算、設定、確認及主選單等含文字 TGA 資源。
- `MAPS/`：多人地圖與教學關卡的 `briefing.put`、`netgame.put`、`text.put` 等。

語言覆蓋不應刪除使用者安裝目錄中的未知檔案，也不應和數值還原綁定。

## 6. `SYSTEM/cl_script.ini`

### 6.1 格式與比對

此檔為 `PFIL@` + LZSS 的 cp1251 文字。修改器使用預先編譯 Regex 定位：

- `Radius = <GER|KEL|HUN>, SpellN, value`
- `CiviDelay = <faction>, value`
- `MoralsDecLostMem`
- `MoralsDecFlee`
- `MoralsDecOverPop`
- `MoralsIncIdle`

只改數值與必要對齊，不移除原行註解。

### 6.2 目前功能

- **村民生產速度**：核心開關開啟時將所有 `CiviDelay` 設為 500 ms（最快 10 倍）；關閉時保留原始備份值。EXE setter 最低只接受 500 ms。
- **無限士氣**：`MoralsIncIdle` 使用 EXE 可接受的最低值 500 ms；介面描述為快速恢復，不宣稱無法達成的 1 ms 即時恢復。
- **無限士氣**：把士氣下降參數設為 0，並把 idle 回復調整到功能要求值；關閉時由原始備份重建。
- **平衡模式法術半徑**：預設為原版 `Radius` 的 2.5 倍。
- **個別祭司設定**：若 `.artroop` 或 `.arpreset` 提供第 9 項 SpellRadius，僅原版具有 Radius 記錄的 KEL/HUN 會用 `customSpellRadius / 500.0` 得到倍率；GER 固定為 0，不提供無效設定。

沒有任何相關修改時，直接寫回原始 `cl_script.ini`，不在現檔上累乘。

## 7. `SYSTEM/ress.ini`

### 7.1 反編譯依據

- `0046c1c0` 載入 `SYSTEM/ress.ini`。
- `0042a230` 是區段/列解析器。
- `[objres]` 以最大 500 列、欄位參數 `0x1f` 交給 callback `0046bd00`。
- `[volkres]` 以 6 列、欄位參數 `0x128` 交給 callback `0046b200`。
- callback 的 switch/連續寫入是欄位分組的主要證據；未確認語意仍標成 candidate。

### 7.2 `[objres]` 欄位

| Index | 語意 | 目前行為 |
|---|---|---|
| 0 | 物件 ID | 永不修改。 |
| 1-6 | 引擎 `bau` 六資源建造成本群組 | `Art`、`Bar`、`Fal` 類在免費生產時清 0；反編譯確認由建造資源檢查與扣除路徑使用。 |
| 1-6 | 建築 `bau` 六資源建造/修復成本 | `Bau*` 在免費建造時完整清 0。 |
| 7-12 | 建築 `upg` 六資源升級成本 | 免費升級時完整清 0。 |
| 13-18 | 引擎 `aus` 六資源群組，單位訓練成本 | 免費生產時清 0；反編譯確認由單位生產數量計算、資源檢查與扣除路徑使用。 |
| 19-24 | 引擎 `auf` 群組，裝備關聯/解除返還 | **保留原版**，不得清 0。 |
| 25-28 | 祭司/德魯伊法術 MP 成本 | 無法術消耗時清 0。 |
| 29 及尾端 | 空欄或 padding | 原樣保留。 |

### 7.3 重要例外與已修正認知

- `FigTiePac00_Packpferd` 整列排除免費生產修改。原版非零欄位包含 `18:1` 與 `24:1`，同時表示馬匹成本與相關解除資料。
- 不得新增 `VerGerZivIco*`、`VerKelZivIco*` 等 UI icon 列；它們屬於 `objdef.dau`/banner 物件關係，不是缺少的 `ress.ini` 資源列。
- 以前把 19-24 清 0 的做法已否定。它可能讓裝備 UI 與 endless AI 失去「單位使用哪種裝備/村民」的關聯。
- 可重現症狀：選取 24 位村民，先預選 4 位帶馬平民，再預選 20 位戰鬥單位，畫面仍顯示 4 位未裝備村民。正確應為 0。這是 mounted-civilian 與 battle-equipment 的共享預留計數問題，不是人口上限。
- 目前策略是保留 19-24 原值，優先維持 UI 與 AI 行為。若要阻止解除武裝取得資源，必須另找 EXE 邏輯，不可再破壞此資料關係。

### 7.4 `[volkres]` 欄位

| Index | 語意/狀態 | 目前行為 |
|---|---|---|
| 0-7 | 陣營全域技能參數；包含 `goldgeschenk`、`heilen`、`opferung`、`manaaufladen`、`motivieren` 等 | 保守處理。Index 2 是治療參數，不是人口。 |
| 8,10,12,14 | 陣型/基礎研究成本 | 免費升級時清 0。 |
| 8-23 | 成本與 ID 配對區 | 未確認欄位保持原值。 |
| 24-263 偶數欄 | 單位、技能、建築、科技解鎖成本 | 免費升級時清 0；奇數 ID 保留。 |
| 264-295 | `befehl`、`motivieren`、`angriff`、`verteidigung` 四組、每組 8 階的屬性升級成本 | 免費升級時全部清 0；反編譯確認四組完整範圍。 |
CSV 分割後的 index 296 是行尾逗號產生的空字串，不是引擎載入的資料欄位。

人口上限不得寫入 `[volkres]`。曾把 Index 2 當人口的解讀已被 EXE callback 與實機資料否定。

## 8. `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`

### 8.1 格式與不可變條件

- `PFIL@` + LZSS + cp1251 CSV-like 文字。
- 以欄位 52 的內部名稱對應 `TroopConfig.UnitMeta`。
- 修改後的解壓文字長度必須和原始長度完全相同。每個數字先經 `CheckLen`，以空白補齊原欄寬；超長則略過整個單位修改並記錄警告。
- 不修改 `Bau*` 的 28-39。這些不是建造成本，而是生產建築的物資槽；清 0 會讓馬匹、武器等庫存 UI 消失。

### 8.2 穩定欄位索引

| Index | 欄位 | 用途 |
|---|---|---|
| 4 | Moves | 主要移動速度。 |
| 19 | Hp | 生命值。 |
| 23 | Movsf | 跑動/第二速度。 |
| 24 | Sirad | 視野。 |
| 52 | Name | 內部單位 ID；語意可靠但 schema 保留 candidate 標記。 |
| 78/79/84 | Weapon1 Akti/Dam/Relt | 武器 1 啟用、傷害、冷卻。 |
| 80-81 | Weapon1RangeMin/Max | 武器槽 1 的 `w1_rad1` / `w1_rad2`。 |
| 82 | Weapon1Angle | 武器槽 1 的 `w1_angl`；不可當作射程修改。 |
| 86/87/88/89/92 | Weapon2 Akti/Dam/Min/Max/Relt | 武器 2。 |
| 94/95/96/97/100 | Weapon3 Akti/Dam/Min/Max/Relt | 武器 3。 |
| 142 | Aw | 戰鬥值。 |
| 146 | Vw | 防禦值。 |
| 191 | Bmovs | 基礎/替代移動速度。 |
| 199 | Weapon1Dtyp | 武器型態候選；目前值 1-4 用於遠程分類輔助。 |

武器槽以 8 欄為 stride，程式會巡覽最多 8 組，僅處理 `Akti == 1` 的槽。

### 8.3 九項兵種屬性

`double[9]` 固定順序：

1. HP
2. Damage
3. VW
4. AW
5. Speed
6. Sight
7. Relt
8. Range
9. SpellRadius

平衡模式先由 `TroopConfig.CalculateFactionBaseStats` 依陣營、階級、類型與裝備風格建立基準，再由個別設定覆蓋。舊的 4 欄設定載入時，缺少的 5 欄從原版或目前平衡基準補齊，避免索引越界。

### 8.4 寫入公式

- HP、VW、AW 直接使用目前基準的整數值。
- Damage 先算 `finalDamage / originalPrimaryDamage`，再同比縮放啟用武器槽。遠程步/騎的 Weapon 1 視為近戰備用，不套遠程主傷害倍率。
- Speed 以自訂速度與原始 `Moves` 推導倍率，再同步套用 Moves、Movsf、Bmovs。
- Sight 寫入 Sirad。
- Range：祭司縮放 80-82；其他單位縮放 Weapon 2/3 的 min/max。
- Relt 以 `customRelt / originalPrimaryRelt` 得到比例，再套用各啟用武器槽。Relt 越小代表攻擊越快。
- SpellRadius 不在 `objdef.dau`，它由 `cl_script.ini` 的 faction Radius 實作。

平衡模式的預設方向包含 2 倍移速、遠程/攻城 3 倍射程、約 1.5 倍射速、祭司視野/技能距離強化及 2.5 倍 spell radius。以下為目前 `TroopConfig.CalculateFactionBaseStats` 的精確四屬性基準。

通用 HP 依階級設定：low 110、mid 130、high 150、ace 160、leader 450。祭司與攻城類由此函式回傳 0，不以這套四屬性矩陣覆蓋原始值。

| 陣營 | Tier | 類型 | HP | Damage | VW | AW |
|---|---|---|---:|---:|---:|---:|
| Roman | low | melee_inf | 110 | 20 | 8 | 12 |
| Roman | mid | melee_inf | 130 | 28 | 14 | 20 |
| Roman | mid | ranged_inf | 130 | 22 | 12 | 24 |
| Roman | high | melee_inf | 150 | 42 | 20 | 22 |
| Roman | high | ranged_inf | 150 | 30 | 16 | 26 |
| Roman | high | hybrid_inf | 150 | 38 | 18 | 22 |
| Roman | ace | cav | 160 | 50 | 24 | 26 |
| Roman | leader | leader_melee | 450 | 80 | 28 | 36 |
| Teuton | low | melee_inf | 110 | 25 | 10 | 12 |
| Teuton | low | ranged_inf | 110 | 20 | 6 | 12 |
| Teuton | mid | melee_inf | 130 | 32 | 16 | 22 |
| Teuton | mid | hybrid_inf | 130 | 28 | 14 | 20 |
| Teuton | high | melee_inf | 150 | 38 | 14 | 26 |
| Teuton | high | cav | 150 | 42 | 20 | 24 |
| Teuton | ace | melee_inf | 160 | 65 | 12 | 30 |
| Teuton | leader | leader_melee | 450 | 70 | 26 | 38 |
| Celt | low | melee_inf | 110 | 24 | 10 | 12 |
| Celt | low | ranged_inf | 110 | 20 | 8 | 12 |
| Celt | mid | melee_inf | 130 | 24 | 18 | 18 |
| Celt | mid | ranged_inf | 130 | 20 | 12 | 18 |
| Celt | high | melee_inf | 150 | 38 | 12 | 24 |
| Celt | high | cav | 150 | 38 | 22 | 22 |
| Celt | ace | ranged_inf | 160 | 65 | 18 | 25 |
| Celt | leader | leader_melee | 450 | 60 | 30 | 28 |
| Hun | low | melee_inf | 110 | 26 | 10 | 10 |
| Hun | low | ranged_inf | 110 | 20 | 8 | 12 |
| Hun | mid | melee_inf | 130 | 24 | 12 | 18 |
| Hun | mid | cav | 130 | 32 | 16 | 20 |
| Hun | high | melee_inf | 150 | 36 | 8 | 22 |
| Hun | high | ranged_inf | 150 | 32 | 16 | 24 |
| Hun | high | cav | 150 | 45 | 18 | 26 |
| Hun | high | ranged_cav | 150 | 36 | 16 | 24 |
| Hun | ace | cav | 160 | 52 | 22 | 26 |
| Hun | leader | leader_cav | 450 | 80 | 25 | 36 |

特化兵種會在通用矩陣前直接回傳，下表數值優先：

| Unit key | 說明 | HP | Damage | VW | AW |
|---|---|---:|---:|---:|---:|
| `FigKelInf01_Lanze` | 塞爾特槍兵，防禦特化 | 180 | 22 | 32 | 18 |
| `FigRomInf00_Lanze_Schild` | 羅馬輕裝步兵 | 130 | 24 | 22 | 18 |
| `FigRomSch00_Speer_Schild` | 羅馬重裝遠程步兵 | 140 | 25 | 26 | 24 |
| `FigRomInf01_Schwert_Schild` | 羅馬禁衛軍 | 200 | 36 | 28 | 28 |
| `FigHunInf01_Schwert_Schild` | 匈奴劍盾兵 | 140 | 24 | 24 | 20 |
| `FigKelInf02_Doppelschwert` | 塞爾特雙劍兵 | 130 | 40 | 15 | 28 |
| `FigGerInf03_Doppelhammer` | 條頓雙錘兵 | 150 | 60 | 16 | 34 |

這些是寫入前的基準，最終結果仍可能由九屬性自訂值取代；持盾等遊戲內加成也可能讓畫面最終 VW 高於資料基準。

### 8.4 人口建築容量

- `objdef.dau` 零起算 Index 156、欄位名 `wohnwer`。
- 核心開關會將原版中所有正數容量乘以 20，包含各陣營主屋與住宅。
- 每次套用都從記憶體原版備份重建，取消開關即可還原，並維持解壓文字長度不變。

## 9. `MAPS/**/team.dat`

### 9.1 人口上限

- 檔案為 `PFIL@` + LZSS + cp1251 INI-like 文字。
- `[maxteamobjgenerell]` 由遊戲寫出但載入器不讀取，因此保持原值。
- `[teamdata]` Index 4 才是實際人口值，並受 EXE 全域上限 1600 約束。
- `[teamdata]` 每列欄位 4 為人口上限；只有原值大於 0 的隊伍才修改，避免啟用原本停用的 team slot。
- 最大人口核心開關開啟時固定寫入 1600；關閉時只還原原始備份。寫入前先從原始備份恢復所有 team.dat，確保 AI 難度及其他 map-specific 欄位不受舊修改污染。

### 9.2 `[teamdata]` 第六欄不是 AI 開關

0-based Index 5 是 `bver`，有效範圍 0-9。EXE 將它與 faction 組合，查找 `SYSTEM/banner.ini` 的：

- `[volk%02ld_vicon_bver%02ld]`
- `[volk%02ld_obdef_bver%02ld]`

它選擇 `Ver*ZivIco`、`Ver*KamIco`、`Ver*Sta*` 等旗幟/圖示/物件版本。修改它不會啟用 AI，也不應用來調 AI 強度。

## 10. 無盡模式 `ak_level.bci`

### 10.1 格式與定位

- 路徑：`MAPS/ENDL_*/SCRIPT/ak_level.bci`。
- 外層為 `PFIL@`，payload 為 `BCI0` 編譯腳本，不是可直接編輯的文字。
- 補丁以 opcode/literal 特徵序列搜尋，不能只依賴固定 offset。
- 已確認 `ENDL_000` 至 `ENDL_004` 存在相同目標序列。

### 10.2 AI Ultimate Mode 寫入

- 軍事建立呼叫：解壓 offset 約 `0x17B60`，callback `s_addNPCJob_createUnit`。
- BCI stack 參數順序反轉後，邏輯呼叫為 `s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)`。
- 兩個數量 literal 位於約 `0x17B2C`、`0x17B34`：`4 -> 20`。
- 完成工作自動回收旗標位於約 `0x17B1C`：`0 -> 1`；讓已完成的軍事增援工作釋放 NPC-job slot，供後續波次持續重用。
- 舊版曾修改 `ak_npc.bci`、`ak_produktion.bci`、`ak_haupthaus.bci` 三個全域經濟腳本。實機確認它們並非安全地只作用於 NPC，會讓玩家所有資源建築在新局也停產；現行版本無論 AI 終極模式是否啟用都強制還原三者原值。
- EXE callback `0054aa80 -> 00547f50` 對此模式把數量 clamp 在 1..20，所以 20 是目前確認的上限，不是任意值。
- 重生等待：`180000 ms -> 5000 ms`，定位於 `CIVRECREATE_WAIT` 周邊特徵。
- 前三個軍事增援輪詢迴圈改為 `5000..10000 ms`，讓 5 秒重生冷卻可在 5–10 秒內被檢查；其他 AI 行動迴圈保留原始等待區間。
- active-party 比較 literal 位於解壓 offset 約 `0x195F8`：`4 -> 8`；`0x1960C` gate 保持 `66,0`。
- 舊版寫入的 `112,272` gate bypass 與全部 `5000..10000 ms` 行動迴圈會自動遷移；僅保留有上場上限與 job 回收保護的三個增援輪詢迴圈加速。
- 關閉或相容性還原時，數量、等待、上限及 gate 全部寫回原始值。

數量參數在引擎中代表建立的軍事單位/編隊，畫面個別士兵總數會再受編隊內容影響。EXE 每個隊伍只有 20 個 NPC-job slots，因此長時間無盡模式不能完全移除 gate。

## 11. `Against_Rome.exe`

### 11.1 失焦繼續執行（穩定）

- 檔案 offset：`0x161a88`。
- 原始：`89 15 C4 7D 9E 02`。
- 修改：`90 90 90 90 90 90`。
- 原指令把狀態寫入全域暫停 flag；NOP 後遊戲失焦或縮小時不再由此處設定暫停。
- 寫入前必須確認完整 6-byte 簽章。未知 bytes 一律不寫，避免其他版本 EXE 損壞。

### 11.2 村莊建造範圍（setter 補丁已實機驗證；舊四位址已否定）

靜態分析找到兩組看似共用 village delta 的計算：

| 作用假設 | 函式 | File offset | 原始 | 已測候選 |
|---|---|---|---|---|
| 邏輯 X 半徑 | `00536630` | `0x1366c4` | `C1 E2 06` | `C1 E2 07` |
| 邏輯 Z 半徑 | `00536630` | `0x1366cd` | `C1 E1 06` | `C1 E1 07` |
| 顯示 X 半徑候選 | `004d7160` | `0x0d722c` | `C1 E6 06` | `C1 E6 07` |
| 顯示 Z 半徑候選 | `004d7160` | `0x0d723b` | `C1 E7 06` | `C1 E7 07` |

相關呼叫與資料流：

- `s_setVillageAeraDeltas` callback `0053ba20 -> 00536450 -> 004c0900` 寫入 X/Z delta。
- `s_villageAeraDeltas`：`0053ba60 -> 00536510`。
- `s_getVillageAeraDeltas`：`0053ba80 -> 00536580`。
- `s_getTeamVillageAera`：`0053bad0 -> 00536630`。
- `s_inTeamVillage`：`0053bb40 -> 00536770`，內部到 `00536820` 做 bounds test。
- `s_setShowTeamVillageAera`：`0053c140 -> 00537d60`。
- `s_showTeamVillageAera`：`0053c170 -> 00537da0`。
- `00535060` 根據 team-village 顯示旗標加入 display bit `0x1d`。
- `00536450` 先呼叫 `004c0900` 把 X/Z 寫入村莊物件的 type-definition rectangle，再把同值寫入 per-object village state。
- `004c0970` 讀取 type-definition rectangle 做通用 point-in-object 測試，共有 10 個 UI caller；`004d7160` 讀取同一份資料並透過 `00495360` 畫四條虛線。
- `00536630` 則讀取 per-object village state。兩條路徑共享 setter 輸入，但使用不同儲存位置。

原靜態假設是把 shift 6 改 7，使 `delta * 64 + 32` 變成 `delta * 128 + 32`。四處補丁只改了 `00536630` 與 `004d7160` 的最終倍率，漏掉使用 type-definition rectangle 的 `004c0970`，因此沒有同步全部 consumer；而且四處全部修改後，實機觀察到建造範圍與截圖中的紅色虛線框都**完全沒有改變**。因此：

- 不能再稱此方案為已完成或同步放大。
- `004d7160` 可能畫的是另一種 team-village 顯示矩形，或實際紅框使用另一組資料/渲染路徑。
- 舊四位址補丁不再套用；現行開關只控制 `005364c1 -> 0056258f` setter trampoline。
- 修改器保留四處狀態辨識，只為還原舊版曾寫入的兩處或四處候選 bytes。
- 偵測到 `LegacyLogicOnly` 或四處 `Expanded` 候選時，會把四處全部恢復成 shift 6。
- `00451650` / overlay type `0x28` 也已否定。它的 caller 是 `igm_but_kampf_beserk` 與 `igm_but_kampf_normal`，屬戰鬥模式 UI，不是村莊紅框。
- `00539700` 在 pending-village 初始化時透過 `00536450` 寫入 logical village state。`00536820` 的直接 caller 只有 script/AI wrapper `005367c0` 與候選位置搜尋 `00544fd0`；玩家預覽 `0044f4b0`、`0044f7b0` 都不呼叫它，因此 `00536630` 不是一般玩家建造範圍 gate。
- 現行補丁改在 `00536450` setter：於 `005364c1`（檔案 `0x1364c1`）跳到 `0056258f`（檔案 `0x16258f`）的 289-byte 可執行零填充區，保留兩個負值檢查後以 `(value * 5) >> 1` 把 `ESI`/`EDI` 各放大為原版 2.5 倍，再呼叫 `004c0900` 並回到 `005364d1`。這會讓 type-definition 與 per-object 兩份資料同步放大。相同 setter 路徑的舊 2 倍版本已實機確認可擴大玩家建造範圍；新的 2.5 倍倍率與紅色虛線框仍須實機驗證。

後續必須從玩家建造命令的接受/拒絕路徑，以及實際紅線的 runtime breakpoint 反向追蹤，不應再只搜尋相似的 `SHL 6`。

### 11.3 祭壇與其他舊 EXE 嘗試

過去放寬祭壇限制的組合語言修改會造成崩潰，沒有納入現行修改器。除失焦補丁外，任何 EXE 寫入都必須有版本簽章、原始 bytes、還原 bytes、呼叫路徑及實機驗證；不滿足者只可記錄研究，不可寫入。

## 12. `apt.dat`

- 已辨識為 ZIP-like 容器，內含 `SYSTEM/DATA/APT/*.apt` 二進位模型/配置。
- 現行修改器不修改碰撞尺寸、不修改 UI layout，也不重新封裝此檔。
- 過去放大 projectile X/Y/Z collision 的方向已還原。
- 在 checksum、entry 語意與實機載入規則確認前，維持 read-only candidate。

## 13. 預設、兵種檔與存檔管理

### 13.1 `.arpreset`

- INI-like 純文字，數字使用 `InvariantCulture`。
- `[Settings]` 以 `MaxPopulation`、`FastCiviProduction` 儲存最大人口與最快村民生產開關，並儲存其他功能開關。
- `[TroopStats]`/相容單位區儲存兵種九屬性。
- 舊版 `PopLimit`、`CiviSpeed` 欄位仍可匯入；只有 1600 與 10 倍會轉換成新開關的啟用狀態。
- 舊 4 屬性列自動補為 9 屬性。
- `VillageBuildRange` 控制項與設定檔欄位對應已實機驗證的 setter trampoline，不會重新啟用舊四位址補丁。

### 13.2 `.artroop`

每列格式：

`UnitKey=HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius`

- 支援全部 43 個已知兵種，包括 3 個祭司與 7 個攻城單位。
- 匯入先驗證 unit key、欄數及每個數值，再更新 grid；不得部分寫入錯誤列。
- `TroopPresetForm` 依 GER/KEL/HUN/ROM 分頁，套用後把結果回傳主表單 `customUnitStats`。

### 13.3 存檔管理

- 掃描遊戲 SAVE 與修改器建立的 ZIP 備份。
- 即時存檔的唯一正式根目錄是 `Path.Combine(GetGamePath(), "SAVE")`；以預設安裝為例即 `C:\Program Files (x86)\Against Rome\SAVE`。修改器不得把 `%LOCALAPPDATA%\VirtualStore` 當成遊戲資料來源或修改目標。
- 舊版遊戲若從 `Program Files (x86)` 以一般使用者權限啟動，Windows UAC 檔案虛擬化可能把相對寫入重新導向至 `%LOCALAPPDATA%\VirtualStore\Program Files (x86)\Against Rome\SAVE`，造成與正式目錄互不相通的第二套存檔。
- 修改器的 manifest 使用 `requireAdministrator`，啟動遊戲時 `FileName` 指向所選目錄的 `Against_Rome.exe`，`WorkingDirectory` 也固定為所選遊戲目錄。因此從修改器啟動時，遊戲會讀寫正式目錄。直接啟動遊戲的快捷方式也必須以系統管理員身分執行；否則可能再次建立 VirtualStore 分流。
- `BackupSaveCache` 以 ZIP 路徑及最後修改時間快取 `save.ini` 顯示資料，避免每次刷新重解壓全部備份。
- 刪除備份時同步移除 cache entry。
- 還原存檔前必須驗證目標與 ZIP entry 路徑，避免 path traversal 或寫到 SAVE 以外位置。

### 13.4 2026-06-29 本機存檔整併紀錄

- 搬移前先備份正式 `SAVE` 與完整 VirtualStore 遊戲目錄；備份位於 `%LOCALAPPDATA%\AgainstRomeModifier\MigrationBackups\20260629_155551`。
- 正式目錄原有 `ESAVE_000`、`ESAVE_001`；VirtualStore 原有 `ESAVE_000`、`001`、`002`、`003`、`004`、`009`。為避免覆蓋，虛擬的 `000/001` 分別改存為正式的 `005/006`，其餘保留原槽位。
- 正式目錄中較新的 `game.cfg` 保持有效；舊的虛擬版本保存為 `SAVE\game.virtualstore-20260607.cfg`。兩邊 `key.cfg` 雜湊相同，因此只保留正式副本。
- VirtualStore 其餘 crash dump、相容層日誌及 `mod_info.md` 均搬至正式遊戲目錄。驗證完成後，`%LOCALAPPDATA%\VirtualStore\Program Files (x86)\Against Rome` 已移除。
- `Against_Rome.exe` 的使用者相容性層加入 `RUNASADMIN`；開始功能表快捷方式仍指向正式 EXE，工作目錄仍是正式遊戲目錄。之後無論從修改器或該快捷方式啟動，都應使用同一套正式資料。

## 14. UI 與效能實作

- UI 是無邊框深色 WinForms，側邊導覽切換隱藏標頭的 TabControl。
- `ModernToggle` 自繪開關；自訂 region 使用 `CreateRoundRectRgn` 後必須 `DeleteObject`，避免 GDI handle 洩漏。
- 表格使用固定欄寬、垂直捲動與數值增減色彩，避免水平欄位錯位。
- `_backupUnitRows` 在備份載入後只解析一次，Default/Current grid 共用，避免反覆解壓 `objdef.dau`。
- Regex 是 `private static readonly` 並使用 `RegexOptions.Compiled`。
- 日誌實體寫入使用 lock，背景工作透過 UI-safe 路徑更新畫面。
- 錯誤日誌包含 exception message 與 stack trace。

## 15. 反編譯與研究資產

### 15.1 完整函式清冊

目前 Ghidra 對本機 EXE 建立 7,381 個函式的機器產生清冊：

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

這些檔案是本機研究產物，不是原始碼，也不表示每個函式語意都已確認。它們預設不發佈原版程式內容。

### 15.2 可重現工具

- `tools/re/GhidraVillageRedFrameAnalysis.java`：輸出 village script API、callback、bounds 與顯示候選資料流。
- `docs/reverse-engineering/decompilation-workflow.md`：headless Ghidra 建立、匯入、分析與匯出流程。
- `docs/reverse-engineering/exe-functions.md`：已命名函式與證據。
- `docs/reverse-engineering/known-patches.md`：穩定、候選與已否定補丁。
- `docs/reverse-engineering/objdef-fields.csv`、`ress-fields.csv`：欄位索引資料庫。
- `data/game_schema.json`：工具可讀的單一 schema。

研究新功能時，必須先查以上資產，不應每次重新做全 EXE 匯出。只有 EXE hash/版本改變或現有輸出缺少必要分析資料時，才重建 inventory。

## 16. 驗證清單

### 16.1 每次建置前

- `dotnet build` 成功，沒有新增 compiler warning/error。
- `data/game_schema.json` 可被 JSON parser 讀取。
- 中英文文件各自同步，沒有截斷標記與亂碼新增。
- `git diff --check` 無 trailing whitespace 或 patch 格式錯誤。

### 16.2 資料補丁

- 每個 PFIL 目標可解壓、重壓、再解壓且 payload 相同。
- 未勾選功能時由原始備份重建，不能在已修改檔上反覆累乘。
- `objdef.dau` 解壓文字長度完全相同。
- `ress.ini` 保留 Index 19-24、Pack horse 原列與尾端逗號。
- `team.dat` 只改遊戲實際載入的 `[teamdata]` Index 4，Index 5 `bver` 與 `[maxteamobjgenerell]` 均不變。
- Endless BCI 每個目標 map 都通過 signature；signature 不符時略過並記錄，不猜測 offset。

### 16.3 EXE 補丁

- 先核對檔案大小/hash 或至少完整原始 byte signature。
- 失焦補丁只接受原版 6 bytes 或已知 NOP 狀態。
- 村莊四處候選永遠不寫 `07`；只允許從已知候選恢復 `06`。
- 未知混合狀態不得強制覆蓋，應警告並保留檔案。

### 16.4 實機案例

- 遊戲可進主選單、載入地圖、建立/升級建築、生產及解除單位。
- 四陣營人口值在目標地圖生效，原本停用 team 不被啟用。
- 平衡開關關閉時回到原始 stats；開啟時 Default/Current grid 與遊戲一致。
- 祭司法術半徑、MP、士氣、村民生產速度分別測試，不用單一案例推論全部功能。
- 24 村民 + 4 mounted civilians + 20 battle units 的 UI 計數案例需保留為回歸測試。
- AI Ultimate 至少覆蓋五張 endless map、重生、行動週期與同時上場數，不只觀察第一次 spawn。
- 村莊紅框候選的既有結論是「四處修改無變化」，除非有新的 runtime 證據，不得改寫成成功。
- 啟用 `VillageBuildRange` 後，確認玩家可用的村莊建造範圍改為原版 2.5 倍；舊 2 倍 setter 路徑已有實機證據，但新的倍率與紅色虛線框須重新驗證。

## 17. 已知限制與後續研究邊界

- 不可能從機器產生的 C-like decompile 還原原作者的每一行原始碼、變數名、註解與建置工程；函式 inventory 是導航資料，不是 100% 語意真相。
- `apt.dat` entry、部分 `ress.ini` candidate 欄位與 BCI opcode 尚未完整命名。
- AI Ultimate 的數量、等待值、同時上場限制與完成工作自動回收仍需長時間無盡模式回歸；全域村民生產／轉職補丁已因玩家資源停產回歸而停用。
- setter trampoline 的 2 倍版本已實機驗證；現行 2.5 倍版本與紅色虛線框是否同步改變仍待實機驗證。
- 新結論必須區分「檔案中儲存的值」與「執行時真正代表的行為」。只有欄位相鄰、名稱相似或畫面看起來合理，不能當作語意證明。

## 18. 公開儲存庫邊界

- 可發布：C# 原始碼、`data/game_schema.json`、`docs/reverse-engineering/`、`tools/re/` 的可重現分析腳本、`tools/Repair-LanguageBackup.ps1`，以及具備再散布依據的 `ThirdParty/dgVoodoo2/` 整合檔案。
- 不可發布：`遊戲原始檔案/`、`Original game archives/`、`Backup.zip`、遊戲 EXE/DAT/地圖/語言/存檔，以及任何由原版 EXE 產生的完整反編譯輸出。
- 僅供本機：`.codex/`、`.agents/`、`re_workspace/`、IDE/建置產物、dump/log、語言備份目錄及 `CodeAuditReport.md`。
- 發布前必須執行 `git status --short --ignored`，確認本機資產顯示為 `!!`，並檢查 `git ls-files` 中沒有原版遊戲 payload。
- dgVoodoo2 v2.87.3 官方條款允許遊戲或遊戲模組附帶個別檔案；本專案不是通用 launcher/framework，來源與條款記錄於 `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`。
