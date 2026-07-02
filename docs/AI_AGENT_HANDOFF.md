# Against Rome Modifier：AI 代理維護與除錯交接手冊

> 更新日期：2026-07-02
>
> 適用工作區：`C:\離線儲存\程式設計\Against_Rome_Modifier`
> 目的：讓新的 AI 代理先理解已證實的功能、失敗案例、回復契約與驗證方式，再修改程式。

本文件整合目前工作區、專案內技術文件，以及可存取的 2026-06-22 至 2026-07-01 Against Rome 專案對話摘要。它不是逐字聊天備份；它是把對話中的技術結論重新與現行程式碼核對後，整理成可執行的維護契約。歷史敘述與現行程式衝突時，以現行程式、目前資料檔、可重現的位元組證據及實機結果為準。

## 1. 新代理開始前必讀

1. 唯一的即時工作區是本文件頂端的離線路徑。舊的 OneDrive 路徑只可當歷史背景，不能作為讀寫目標。
2. 先執行 `Get-Location`。部分沙箱會忽略含中文的 `cwd`，實際從 `C:\` 開始；不要在根目錄做遞迴搜尋。
3. 先執行 `git status --short --ignored` 和 `git diff`。工作樹可能已有使用者或其他代理的變更，禁止任意覆寫。
4. Git 若回報 dubious ownership，使用一次性的 `git -c safe.directory=... -C ...`，不要改全域設定。
5. 修改任何遊戲檔前，必須確認所選路徑內存在 `Against_Rome.exe`。僅有「目錄存在」不足以證明它是遊戲根目錄。
6. 不得把未知 EXE 位元組、未知 BCI opcode、Ghidra 自動命名的 `FUN_*`，寫成已證實語意。
7. 不得弱化回復保護。遇到缺少原版基線、未知 patch state 或不安全 ZIP 路徑時，應停止並清楚報錯。
8. 每個新補丁至少要有：原始值、修改值、狀態偵測、回復方式、版本／長度防護、失敗時 rollback、文件與 schema 同步。

## 2. 證據等級

本專案文件使用以下語意；不要混用：

- **穩定／已實機驗證**：程式路徑與遊戲行為都有證據。
- **靜態驗證**：位元組、反編譯、檔案結構或 round-trip 已核對，但缺少足夠的長時間實機測試。
- **候選**：有合理的資料流或格式證據，只能讀取／研究，不應自動寫入。
- **舊版相容狀態**：目前不再產生，但偵測和回復必須保留。
- **已否決**：實機結果證明假設錯誤或造成回歸。保留精確紀錄是為了防止再次加入。
- **未知**：位元組或資料不符合任何已知狀態。必須保留原檔並警告，不得強寫。

## 3. 2026-07-02 工作樹快照

本次文件更新開始前已有下列非文件變更，均視為既有工作，不是本次建立：

- 已修改：`ModifierForm.Data.cs`
- 已修改：`ModifierForm.Patches.cs`
- 未追蹤：`changes_patch.diff`
- 未追蹤：`changes_summary.md`

現行程式已通過：

```text
dotnet build AgainstRomeModifier.csproj -c Release --no-restore
0 warnings, 0 errors
```

既有變更包含：

- `MergeUnitStatsLayers(...)`：先選原版／平衡層，再由自訂 preset 的現有欄位逐欄覆蓋；舊版短陣列只讓缺少欄位繼承 fallback；不支援法術半徑的單位強制第 9 欄為 0。
- EXE 狀態讀取和修改改用單一 `byte[]`，避免同一次流程重複開檔。
- `cl_script.ini`、`ress.ini`、`objdef.dau`、`team.dat`、ENDL BCI 先產生候選 `byte[]`，再集中呼叫 `SafeWriteAllBytes(...)`。

重要限制：這不是完整的跨檔案原子提交。程式仍逐檔寫入，只是 `FileRollbackScope` 能在失敗時回復；語言 overlay、dgVoodoo2，以及部分 restore 路徑仍包含各自的檔案 I/O。任何代理都不得把目前狀態描述成「所有功能均已在記憶體預演且一次原子 commit」。

## 4. 專案結構與修改位置

| 檔案 | 現行責任 | 修改時最常見風險 |
|---|---|---|
| `Program.cs` | WinForms 進入點、DPI、UAC | 改 manifest／啟動權限會影響 VirtualStore |
| `ModifierForm.cs` | 手工建立 UI、控制項位置、事件 wiring、內嵌文件頁 | 初始化順序、座標重疊、事件遺失 |
| `ModifierForm.Data.cs` | 備份來源、ZIP 載入、目前資料讀取、狀態偵測、顯示計算 | 把偵測誤當修改、短 row 越界、toggle 被自動同步 |
| `ModifierForm.Patches.cs` | 套用、回復、rollback、EXE／INI／DAU／team.dat／BCI | 寫入未知版本、破壞回復基線、誤解欄位 |
| `ModifierForm.DataExt.cs` | 額外資料／圖示相關邏輯 | 與 `Data.cs` 職責重疊時要先追 call site |
| `ModifierForm.Presets.cs` | `.arpreset` 匯入／匯出 | 新 toggle 漏存、舊欄位相容性中斷 |
| `ModifierForm.SaveManager.cs` | 遊戲存檔與 ZIP 備份 | path traversal、半成品 ZIP、回復後清理順序 |
| `ModifierForm.DgVoodoo.cs` | 內嵌 dgVoodoo2 安裝／移除與所有權 manifest | 覆蓋非受管 DLL、刪除使用者修改檔 |
| `TroopConfig.cs` | `ObjdefIndex`、`RessIndex`、單位 metadata、平衡規則 | magic number、欄位 offset 漂移 |
| `TroopPresetForm.cs` | 9 欄單位 preset 編輯 | 舊版 4 欄 preset 相容、法術半徑適用範圍 |
| `GameLZSS.cs` | LZSS 與 `PFIL@` 包裝 | header 長度、round-trip、解壓長度 |
| `Localization.cs` | 中英 UI 與 log 字串 | 新控制項只新增單一語言、鍵名不同步 |
| `data/game_schema.json` | 機器可讀的欄位、offset、patch 狀態 | 文件與程式已變更但 schema 沒同步 |
| `docs/reverse-engineering/` | 證據、格式、已知補丁、Ghidra 工作流 | 把候選寫成 stable、保留過期結論 |

行號會因工作樹變動而漂移。尋找位置時應搜尋穩定識別字，例如 `GetPatchedObjdefBytes`、`GetPatchedEndlessScripts`、`ExeVillageSetterHookOffset`，不要只依賴歷史行號。

## 5. 核心安全模型

### 5.1 備份來源順序

`ModifierForm.Data.cs` 的備份來源順序是：

1. 內嵌 `Backup.zip`（若建置時存在）。
2. 執行檔旁的本機 `Backup.zip`。
3. 使用者選定遊戲目錄中的現有原版檔案，建立記憶體基線。

公版 GitHub 專案不包含 `Backup.zip`，因此第三條路徑不是例外，而是公開建置的正常行為。補丁應盡量從 `backupFiles` 的原始內容重新產生，避免在已修改檔上反覆乘值或累積修改。

### 5.2 `FileRollbackScope`

`ModifierForm.Patches.cs` 的 `FileRollbackScope` 是目前跨檔回復機制：

1. 第一次追蹤檔案時記住原始 bytes 或「原先不存在」。
2. `SafeWriteAllBytes` 先讓 rollback 追蹤，再用暫存檔完成單檔替換。
3. 流程全部成功才 `Commit()`。
4. 未 commit 的 scope 在例外／Dispose 時回復已追蹤檔案。
5. commit 後應立刻 Dispose 並清空 scope，再做 UI refresh；UI refresh 失敗不應回滾已成功的遊戲檔。

這提供「失敗後盡力回復」，不是檔案系統層級的多檔原子交易。新增功能若直接 `File.WriteAllBytes`、`File.Copy` 或 `File.Delete` 而繞過 scope，會破壞整體契約。

### 5.3 ZIP 安全

`LoadZipToDictionary(...)` 必須拒絕：

- 絕對路徑；
- 以 `/` 或 `\` 開頭的路徑；
- 任一 `..` traversal segment；
- 正規化後離開預期根目錄的目標。

存檔備份 ZIP 應先寫成 `zipPath + ".tmp"`，完整建立 payload 與由修改器控制的 `manifest.json` 後，再移到正式名稱。禁止在失敗時留下看似正式、其實不完整的 ZIP。

## 6. 遊戲檔格式契約

### 6.1 `PFIL@` / LZSS

- `GameLZSS.DecompressPfil(...)`：有 `PFIL` header 才解壓；無 header 視為原始資料。
- `GameLZSS.CompressPfil(...)`：要求至少 64-byte 原始 header，重建 header 和解壓長度。
- 每個修改過的 payload 都應驗證 `DecompressPfil(CompressPfil(payload, header)) == payload`。
- `objdef.dau` 的修改還必須保持解壓後文字總長完全一致。
- 遊戲文字通常是 Windows code page 1251；專案文件一律 UTF-8。

### 6.2 CSV-like 資料

`objdef.dau` 和部分 INI section 使用簡單的逗號分隔格式。現行契約是 `Split(',')` 與 `string.Join(",", cols)`，不是 RFC 4180。不要擅自加入 quote/escape parser；過去「改良 CSV」曾造成與遊戲格式不相容。

### 6.3 未知資料處理

- 欄位不足時先做長度檢查，尤其 weapon slot 迴圈 `w = 1..8`。
- 數值解析／格式化使用 invariant culture。
- 不符合已知原版、目前版或舊版 patch signature 的檔案要保持不動。
- 不得為了「讓套用成功」而把未知 bytes 當成原版覆寫。

## 7. 功能與補丁契約

### 7.1 `objdef.dau`：單位數值與 20 倍人口容量

關鍵位置：

- 欄位 enum：`TroopConfig.cs` 的 `ObjdefIndex`。
- 套用：`ModifierForm.Patches.cs` 的 `GetPatchedObjdefBytes(...)`。
- 目前狀態偵測：`ModifierForm.Data.cs` 的 `HasHousingCapacityMultiplier(...)` 和 `LoadCurrentData(...)`。
- preset：`ModifierForm.Presets.cs` 的 `HousingCapacity20x`。

穩定欄位包含：`Moves=4`、`Hp=19`、`Movsf=23`、`Sirad=24`、weapon 1 基底 `78..84`、weapon 2 基底 `86..92`、weapon 3 基底 `94..100`、`Aw=142`、`Vw=146`、`HousingCapacity=156`、`Bmovs=191`。`Name=52` 與 `Weapon1Dtyp=199` 仍保留 candidate 標記。

20 倍人口容量規則：

- 欄位是 zero-based column 156，原始名稱 `wohnwer`。
- 只處理原版值大於 0 的 row；基準資料觀察到 22 rows。
- 值必須從原版基線計算成 `original * 20`，不得把目前值再乘 20。
- 欄位寬度與整個解壓 payload 長度必須保持。
- 狀態偵測比較所有原版正值 row 是否等於 `original * 20`，不是只抽查一列。

單位屬性層級規則：

1. 原版層。
2. 若啟用 balance 或 force balance，使用預設平衡層。
3. 自訂 preset 中實際存在的欄位逐欄覆蓋。
4. 舊版短 preset 缺少的欄位繼承第 1／2 層。
5. 不支援法術半徑的單位，第 9 欄固定為 0。

### 7.2 `ress.ini`：免費建造／生產／升級／法術

關鍵位置：

- 欄位 enum：`TroopConfig.cs` 的 `RessIndex`。
- 套用：`GetPatchedRessBytes(...)`。
- free-production 判斷：`ShouldZeroFigFreeProductionField(...)`。

`[objres]` 主要分組：建造 `1..6`、升級 `7..12`、單位生產 `13..18`、裝備／卸裝退還 `19..24`、祭司法術 `25..28`。

不可再犯的錯誤：

- 不得把 `19..24` 一律清零。這些欄位參與裝備 accounting，曾造成錯誤退還與無盡模式／單位關係問題。
- `FigTiePac00_Packpferd` 是明確例外：完全保留原版成本與退還，不納入免費模式。
- 過去曾加入 healing-food／healing-speed 修改，使用者明確要求移除；不得以其他名稱重新引入。
- 24 村民、4 騎乘村民、再配置 20 戰鬥單位時 UI 仍顯示 4 個未裝備村民，這是 EXE/UI 的共享 reservation count 問題，不是可用 `ress.ini` 粗暴清欄解決的問題。

### 7.3 `cl_script.ini`

關鍵位置：

- Regex：`ModifierForm.Data.cs` 與 `ModifierForm.Patches.cs` 頂端。
- 管理 key：`GetClScriptManagedKeys(...)`。
- 產生補丁：`GetPatchedClScriptBytes(...)`。

管理項目：`Radius`、`CiviDelay`、`MoralsDecLostMem`、`MoralsDecFlee`、`MoralsDecOverPop`、`MoralsIncIdle`。

維護規則：

- 以 faction + key 定位，保留註解與不受管內容。
- 不得因單一 faction 缺 row 而無聲產生不完整結果。
- 法術半徑只對支援的單位／faction 套用；不要把 GER/KEL/HUN 行為混在一起。
- balance toggle 的 UI handler 目前只應更新預覽／預設資料，不應偷偷重讀全部 live game files。

### 7.4 `team.dat` 最大人口

- 路徑：`MAPS/**/team.dat`。
- 目前 patch 值：每隊 1600。
- 真正載入的 per-team limit 是 `[teamdata]` column 4。
- `maxteamobjgenerell` 保留原值；不要因名稱看起來像總上限就一起改。
- `team.dat` 不決定 AI Ultimate 的無盡模式增援數量。

### 7.5 AI Ultimate Mode

關鍵位置：

- `ModifierForm.Patches.cs`：`FindEndlessMilitaryCreateUnitCall`、`FindEndlessRespawnDelayLiteral`、`PatchEndlessLoopDelayLiterals`、`PatchEndlessActiveAiLimit`、`TryReadEndlessAiModeState`、`GetPatchedEndlessScripts`。
- schema：`data/game_schema.json` 的 `endlessScript`。
-逆向文件：`docs/reverse-engineering/endless-mode-ai.md`。

目前安全目標值：

| 項目 | 原版 | Ultimate | 狀態 |
|---|---:|---:|---|
| 每次軍事增援 count | 4 | 20 | 受 EXE 1..20 clamp 限制 |
| respawn cooldown | 180000 ms | 5000 ms | 已核對 live/save bytes |
| active-party limit | 4 | 8 | 有界限方案 |
| completed-job recycle flag | 0 | 1 | 靜態與 save bytes 核對；仍需長時間回歸 |
| gate words | `66,0` | 保持 `66,0` | 舊版 bypass 必須還原 |

只加速前三個軍事增援 polling loops 到 `5000..10000 ms`；其他 AI action loops 保持原始節奏。

失敗歷史：

1. 舊版把 gate `66,0` 改為 `112,272`，並大幅加速所有 loops。
2. 早期戰鬥看似更猛烈，但長時間後 AI 不再補兵。
3. Ghidra 證據顯示 `FUN_00547f50` 使用每隊 20-slot NPC job table。
4. 無界派送耗盡 job slots，造成晚期 respawn stall。
5. 現行方案恢復 gate、限制 active-party 為 8、讓完成的增援 job recycle，並只加速相關 polling loops。

另有一組已否決的全域 CLAK 經濟 patch：

- `SYSTEM/CLAK/SCRIPT/ak_npc.bci`：`0 -> 20` at `0x1EA0`
- `ak_produktion.bci`：`117 -> 112` at `0x3710`
- `ak_haupthaus.bci`：`81,59 -> 66,20` at `0x3FCC`

實機證明這些不是安全的 NPC-only 路徑，會讓玩家有人員的資源建築仍然零產出。現行程式只允許還原原值，禁止重新啟用。

存檔除錯證據：2026-07-01 的 `ESAVE_000`（`ENDL_002`）中，`CLAK\scr.dat` 內嵌的 `ak_level` 與 live `MAPS\ENDL_002\SCRIPT\ak_level.bci` SHA-256 完全一致；讀回值為 respawn 5000、recycle 1、count 20、active limit 8、gate `66,0`。因此「看起來沒變」不能直接推論 patch 沒寫入；存檔仍可能保存已排程工作或計時狀態。除錯時要同時比對 live script、save-embedded script 與遊戲中的既有排程。

仍未完成：五張 ENDL 地圖的長時間 late-wave 回歸測試。不要因短期增援成功就把此項標成完全驗證。

### 7.6 `Against_Rome.exe`：背景執行

- file offset：`0x161a88`
- 原始 bytes：`89 15 C4 7D 9E 02`
- patch bytes：`90 90 90 90 90 90`
- 只接受符合已知原版／已 patch signature 的 EXE；其他版本標記 Unknown 並拒絕強寫。

### 7.7 `Against_Rome.exe`：村莊建造範圍／紅色虛線框

現行有效路徑是 setter trampoline，不是舊四處 shift patch：

- hook VA `005364c1`，file offset `0x1364c1`
- code cave VA `0056258f`，file offset `0x16258f`
- 現行倍率 `(value * 5) >> 1`，即 2.5 倍
- 保留 ESI／EDI 負值檢查
- 呼叫 `004c0900` 同步 type-definition rectangle
- 回到 `005364d1`，讓兩條儲存路徑保持一致

相容狀態：`Original`、`Legacy2x`、`Expanded2Point5x`、`Unknown`。`Legacy2x` 必須能被偵測、顯示為已啟用、升級到 2.5x 或回復。

已否決舊 patch：

- `0x1366c4`: `C1 E2 06 -> C1 E2 07`
- `0x1366cd`: `C1 E1 06 -> C1 E1 07`
- `0x0d722c`: `C1 E6 06 -> C1 E6 07`
- `0x0d723b`: `C1 E7 06 -> C1 E7 07`

這四處只改最後 consumer，漏掉 `004c0970` 等路徑，實機沒有產生預期效果。現行程式不得寫入 `07`，只保留偵測舊二處／四處狀態並還原為 `06` 的能力。

驗證邊界：setter 路徑的 2x 版本曾實機確認建造範圍與紅色虛線框同步擴大；2026-07-01 換成 2.5x 後，程式與 bytes 已靜態驗證，但目前文件沒有新的 2.5x 實機確認。不要把「2x 路徑證實」誤寫成「2.5x 已完整實機證實」。

### 7.8 強制英文與受管語言基線

- UI toggle 是手動控制、啟動預設關閉；不得再從 live overlay 狀態自動勾選。
- overlay 來源為 `ToEng`，原版基線存於遊戲根目錄 `.against-rome-modifier-language-backup`。
- 合約：overlay 存在但 baseline/manifest 缺失時，立即中止回復；禁止假裝成功。
- baseline 存在時精確回復；原版不存在的 overlay-only 檔案在回復時刪除。
- 2026-06-30 的真實安裝曾出現 332 個 overlay 檔但沒有 baseline；`Backup.zip` 只有 146 entries 且不含這 332 個目標，不能拿來猜原版。
- 修復工具：`tools/Repair-LanguageBackup.ps1`。在 `Program Files (x86)` 實際寫入需要真正 elevated 的 PowerShell；Codex 自稱管理員不代表子 shell 已提升。

### 7.9 存檔與遊戲路徑

- 所有正常讀寫的根是 `GetGamePath()` 回傳的 `txtGamePath.Text.Trim()`。
- canonical save root 是 `<gamePath>\SAVE`，不是 VirtualStore。
- manifest 使用 `requireAdministrator`；啟動遊戲時 `WorkingDirectory = gamePath`。從其他非提升捷徑啟動遊戲仍可能重新產生 VirtualStore。
- restore、delete、launch、patch、language、dgVoodoo 都要從同一選定 root 推導，禁止各自猜路徑。
- 存檔 ZIP restore 成功後先 commit，再做 staging cleanup；cleanup 失敗只記錄，不應回滾已成功的存檔。

### 7.10 dgVoodoo2

- 使用 `ThirdParty/dgVoodoo2` 內嵌的 x86 `D3D8.dll`、`DDraw.dll`、`dgVoodooCpl.exe`、`dgVoodoo.conf`。
- 不下載、不使用 runtime cache。
- `.against-rome-modifier-dgvoodoo.json` 記錄由修改器管理的檔案與 hash。
- 不覆蓋非受管 DLL；受管檔被使用者修改後也不可無聲刪除。
- 還原只移除修改器擁有且未被改動的檔案。
- 發佈來源與 SHA-256 記錄在 `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`。

### 7.11 WinForms UI 與 preset

- `mainTabControl` 的 tab header 故意以 `ItemSize = new Size(0, 1)` 隱藏；左側按鈕才是導航。
- `StyleNavButton(...)` 必須在對應 `TabPage` 已建立後綁定。過去曾因傳入 null，使選取高亮看似「無作用」。
- `pnlSwitchesCard` 是核心開關區；`chkBalance`、`chkVillageBuildRange`、`chkHousingCapacity20x`、`chkAiUltimateMode` 的 parent／座標有明確設計。
- UI 是手工座標，移動一個 control 要檢查鄰近 control，不要順手重排整頁。
- 新 toggle 必須同步：field、建立位置、localization、apply、restore、state detection、preset save/load、文件。
- `.arpreset` 保留舊欄位相容；`.artroop` 要支援舊短陣列並補 fallback。

## 8. 已發生的除錯案例與正確處理方式

| 症狀 | 根因 | 錯誤處理 | 正確處理 |
|---|---|---|---|
| 命令從 `C:\` 掃描並大量 access denied | 中文 cwd 未被沙箱採用 | 繼續 blanket recursion | `Get-Location`、完整路徑、`git -C`、目標檔搜尋 |
| Git 回報 dubious ownership | sandbox user 與 repo owner 不同 | 改全域 safe.directory | 單次 `-c safe.directory=...` |
| Build 讀不到 NuGet.Config | sandbox 權限 | 認定程式壞掉 | 先用 `--no-restore`；需要時核准讀取使用者設定 |
| UI 效果不顯示 | 初始化順序導致 null page | 刪掉「看似沒用」的 styling | 先查建立順序與事件 wiring |
| Toggle 又自動變回 live 狀態 | constructor 與 load path 同時寫 Checked | 只改一處 | 同時查初始化、event、`LoadCurrentData`、preset |
| 免費模式仍有錯誤退還 | 成本與退還欄位重疊 | 只清 obvious cost 或清掉 19..24 | 用原版資料核對完整欄位分組並保留 equipment refund |
| AI 前期正常、後期不補兵 | 20-slot NPC job table 耗盡 | 再提高 concurrency／bypass gate | 有界 active=8、gate 原值、completed job recycle、長測 |
| AI bytes 已改但玩家感覺沒變 | save 中已有排程／timer | 再次盲改 offsets | 比對 live BCI、save embedded BCI、遊戲排程狀態 |
| 語言回復丟 InvalidOperationException | overlay 存在但原版 baseline 缺失 | 移除 guard 或從 overlay 當原版 | 保留 loud guard，重建並 hash 驗證 baseline |
| 存檔 ZIP 半成品 | 直接寫正式 ZIP | 捕捉例外後繼續 | `.tmp` 完整建立後 move |
| restore 後 cleanup 失敗導致整體回滾 | cleanup 在 commit 前 | 把刪 staging 當核心交易 | 先 commit，再 best-effort cleanup |
| 廣泛 source patch 套不上 | 亂碼／行尾／context 漂移 | 擴大 patch context | ASCII identifier 小範圍 patch，先 `Select-String` |
| 文件把候選當事實 | 只看到 Ghidra decompile | 直接命名／寫 patch | call-path + bytes + runtime 分層記錄 |
| `.gitignore` 有英文規則但遊戲檔仍出現 | 真實資料夾是本地化名稱 | 假設已忽略 | 對照實際 on-disk name 與 `git status --ignored` |

## 9. 歷史修改時間線

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
- backup ZIP 改用 temporary archive + generated manifest；避免 lock 內 logging。

### 2026-06-30：搬移工作區、語言修復、村莊範圍與 GitHub 稽核

- 唯一 live checkout 搬到離線中文路徑；舊 OneDrive 只留歷史參考。
- 完成 332-file 語言 baseline 重建工具與 hash 驗證流程。
- 村莊 setter 的 2x 實機路徑與紅色虛線框同步行為獲確認；舊四處 shift patch 正式列為 rejected。
- 發現本地化原始遊戲資料夾約 1.42 GB、4623 files，必須保持 ignored/local-only。

### 2026-07-01：2.5x、手動 toggle、人口容量、公開邊界與 save readback

- setter trampoline 升級為 2.5x，保留 `Legacy2x` detection/migration；新倍率仍待實機確認。
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

## 10. 未完成與不得誤報為完成的項目

1. AI Ultimate：仍需五張 `ENDL_000..004` 的長時間 late-wave 測試，尤其 job recycle、active 8 與增援持續性。
2. VillageBuildRange：2.5x 倍率需要新的遊戲內建造範圍與紅色虛線框確認；2x 的證據不能直接代替。
3. `apt.dat`：只有格式候選，尚未整合安全 patch。
4. BCI opcode：尚未完整解碼，禁止把 pattern match 當成通用 assembler。
5. `ress.ini` 部分 `[volkres]` 欄位仍是 candidate。
6. 武器 damage type 欄位 `199` 仍是 candidate，現行程式的值域判斷不是完整語意證明。
7. 發佈前仍缺少頂層 `LICENSE` 決策。
8. 專案目前沒有自動化單元／整合測試；build 成功不等於遊戲 runtime 行為已驗證。
9. 目前集中產生 `byte[]` 的重構只完成靜態 build 驗證，尚需真實安裝 apply、故障注入、rollback、restore 全流程測試。

## 11. 驗證矩陣

### 11.1 每次文件或 schema 變更

```powershell
Get-Content .\data\game_schema.json -Raw | ConvertFrom-Json | Out-Null
git diff --check
```

確認 `README.md`、`TechDoc.md`、`TechDoc_EN.md`、`docs/reverse-engineering/known-patches.md`、`data/game_schema.json` 對同一功能沒有互斥狀態。

### 11.2 每次 C# 變更

```powershell
dotnet build .\AgainstRomeModifier.csproj -c Release --no-restore
git diff --check
```

若改 PFIL：加 round-trip。若改 `objdef.dau`：加解壓長度完全相等。若改 weapon：加 short-row bounds。若改 preset：測舊版欄位缺失。

### 11.3 EXE patch

- 檔案長度足夠。
- 原始／目前／舊版／未知 state 分開。
- 核對 hook、code cave 長度、call target、return target。
- enable、disable、legacy migration、unknown refusal 四條路徑都測。
- 在副本做 byte-level 測試後才碰 live install。

### 11.4 AI Ultimate

- 全部五張 ENDL map 的原始 signature 一致。
- enable state：count 20、respawn 5000、active 8、recycle 1、gate `66,0`。
- disable state：所有管理 literal 和 loops 回原值。
- 舊 gate bypass `112,272` 必須被遷回原值。
- 新遊戲與舊存檔都測；至少跑到多輪增援後確認沒有 stall。
- 如果玩家回報沒變，讀取 save embedded script，不要先改 code。

### 11.5 語言 overlay

- manifest count 等於實際 backup files。
- manifest 無 absolute／`..`。
- backup SHA-256 與可信原版來源一致。
- active overlay SHA-256 與 `ToEng` 一致。
- 缺 baseline 時必須 abort，不能顯示 restore success。

### 11.6 GitHub 發佈前

```powershell
git status --short --ignored
git ls-files .codex re_workspace Backup.zip MAPS SYSTEM SAVE ToEng
git log --oneline --all -- Backup.zip
```

不得上傳：原始遊戲資料、`Backup.zip`、遊戲安裝樹、存檔、`.codex/`、`.agents/`、`re_workspace/`、內部稽核報告、下載工具鏈。可上傳：C# source、結構化 docs/schema、`tools/re/` 可重現 scripts、已記錄來源的 dgVoodoo2 檔。不要在未獲明確授權時 commit、push、rewrite history 或 force-push；也不要使用 `git push --mirror`。

## 12. 建議的代理工作順序

1. 讀本文件與 `TechDoc.md`。
2. 讀該功能對應的 `docs/reverse-engineering/*` 與 `data/game_schema.json`。
3. 檢查工作樹差異，分離既有變更與自己的變更。
4. 以穩定識別字定位 code，不依賴舊行號。
5. 先列出原始 state、目標 state、unknown 行為、restore 行為。
6. 小範圍修改；不要順便重構無關區域。
7. 跑與風險相稱的 build、schema、round-trip、byte/state 測試。
8. 同步程式、README、技術文件、known patches、schema。
9. 最後重新看 diff，確認沒有覆寫使用者現有工作。

## 13. 相關文件

- `TechDoc.md`：乾淨中文版現行技術規格。
- `TechDoc_EN.md`：英文現行技術規格。
- `docs/reverse-engineering/known-patches.md`：高訊號 offset／bytes／狀態總表。
- `docs/reverse-engineering/endless-mode-ai.md`：AI Ultimate 深入證據。
- `docs/reverse-engineering/exe-functions.md`：EXE function anchors。
- `docs/reverse-engineering/file-formats.md`：檔案格式。
- `docs/reverse-engineering/decompilation-workflow.md`：本機 Ghidra 工作流。
- `data/game_schema.json`：供代理與工具讀取的 schema。
- `tools/Repair-LanguageBackup.ps1`：語言 baseline 修復。
- `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`：第三方來源與 hash。
