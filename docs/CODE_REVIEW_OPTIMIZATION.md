# Against Rome Modifier — 全程式碼檢查與優化建議報告

> ## 修復狀態(2026-07-08 已就地修復)
> 以下項目已直接修復並通過驗證(73/73 單元測試 + verify_split_patches golden 值驗證全綠):
>
> | 項目 | 修復內容 |
> |---|---|
> | **A1** | `CompressPfil(null!)` 改為合法 64-byte PFIL 標頭(`BackupManager.CreateEmptyPfilHeader`),兩處空 catch 改為記 log |
> | **E1** | `ObjdefPatcher` 武器射程欄位改為 active+2/+3(80/81),不再誤縮放角度欄 82;`GetMaxRange` 同步修正;新增回歸測試 `Objdef_unit_range_scaling_touches_range_columns_only_and_never_weapon_angle` |
> | **F1** | `DetectModule` 找不到任何檔案時回傳 `Unknown`;`CheckGameStatusTest` 兩測試補目錄存在性守衛 |
> | **A3** | 啟動不再跑完整 `ApplyPatches`,改為新的 `PatchEngine.RunStartupSafeMigrations`(只做 R0 常駐修復 + 首領閃退腳本重建,保留待機回血現況)——自訂兵種屬性不再於啟動時被覆蓋 |
> | **A4** | 遊戲加速 index↔倍率改為查 `GameSpeedSupportedMultipliers` 表 |
> | **A6** | `BtnDeleteBackup_Click` 的快取移除補上 lock |
> | **A8** | `ExePatchModel.Apply` 註解修正(部分套用後緩衝區不可再用) |
> | **B1/B2/B5/B7 + 死碼** | `ModifierForm.Data.cs` 刪除約 300 行死碼(分歧的 `GetCleanEparaText` 副本、objdef 偵測三函式副本、`IsMaximumPopulationApplied`、`CheckLen`、`ToCsvString`、4 個未用 Regex、3 個備份包裝方法);`ModifierForm.cs` 空註解殘骸 |
> | **B4+E4** | 新增共用 `Core/Services/SafeFileWriter`(含「內容相同即跳過寫入」),`PatchEngine` 與 `EndlessAiOrchestrator` 的重複實作改為委派;例外型別改 `IOException` |
> | **E2-1/2** | `LogRestoreAllSuccess`→`LogRestoreAllDone`;En 補 `AiM6Tip`;En `AiM4Tip` 移除 M6 拆分前的過期描述 |
> | **G1** | `.gitignore` 根目錄專屬模式加 `/` 錨定(已以 `git check-ignore` 驗證),新增 `__pycache__/`,並 `git rm --cached` 已追蹤的 `.pyc` |
> | **E8(部分)** | CI 加入 `verify_split_patches` 建置驗證 |
>
> **更正**:A5(`LogPresetImportError`)經查中英文字內容皆正確,僅鍵名誤導 → 降級為命名問題,未改程式。
>
> ### 第二批已修復(commit 24fb773 之後)
>
> | 項目 | 修復內容 |
> |---|---|
> | **A7** | 啟動崩潰改為 MessageBox 提示 + crash_log 指引;提權失敗(UAC 取消)明確告知需要管理員權限 |
> | **A9** | 字型/圖示/ToolTip 釋放改為標準 `Dispose(bool)` 覆寫,不再掛 FormClosing |
> | **A10/F5** | 13 處空 `catch { }` 補上具體 log(PatchEngine 偵測 9 處、BackupManager 自我修復 2 處、Data.cs 2 處) |
> | **E5** | TroopPresetForm:匯入改逐行 TryParse 容錯並回報略過行數;`LoadedFileName` 改為成功後才設定(匯入與匯出);匯入值以 `0.##` 保留小數精度 |
> | **E6** | 兵種圖示改為先 US、找不到時退回 `IGM0806/` 下任一語系資料夾的同名檔 |
> | **E7** | 人口上限統一為 `TeamDatPatcher.DefaultPopulationLimit` 常數 |
> | **E9** | 「當前兵種數值」頁的法術半徑對比套用 KEL/HUN 白名單,GER 祭司不再顯示假變動值 |
> | **F3(輕量)** | Backup.zip 缺失時測試輸出明確標示 `[SKIPPED]`(完整 SkippableFact 方案待議);順修 CS8625 警告 |
> | **F4** | `GetPatchedClScriptBytes` 改 `internal` + `InternalsVisibleTo`,測試移除反射 |
> | **C2** | `GameLZSS.Compress` 輸出緩衝改 `MemoryStream`(預留容量) |
> | **C3** | TroopConfig 排序改用 index 對照表,移除比較器內 O(n²) `IndexOf` |
> | **C4** | 導覽按鈕移除 `MouseMove → Invalidate` 的多餘整鈕重繪 |
>
> ### 第三、四批已修復(依審查建議)
>
> | 項目 | 修復內容 |
> |---|---|
> | **A2** | `PatchEngine.GetPatchedClScriptBytes` 統一委派 `ClScriptPatcher`(單一正本);平衡半徑以 target/500 倍率換算保持既有行為(含 GER=0);還原改為寫回備份原檔真實原值;刪除 9 個重複 Regex、`GetClScriptManagedKeys` 與未用的 `LogLock`;以真實 Backup.zip 驗證通過 |
> | **E3** | 刪除 Program.cs 手動提權碼(manifest `requireAdministrator` 已保證,該分支永遠走不到) |
> | **F2** | `VerifyRealGamePatches` 改由 `AR_GAME_PATH` 環境變數 gate,移除硬編碼安裝路徑 |
> | **G4** | README 新增 Maintenance Tools 章節(Repair-LanguageBackup.ps1、bcitool.py 同步義務、verify_split_patches 執行時機) |
> | **E2-3** | 服務層(PatchEngine/BackupManager)41 處 log 全部本地化:新增 37 組 `SvcLog*` 鍵(Zh/En 對稱,經指令碼驗證無缺鍵、無佔位符錯配),英文使用者的 modifier_log.txt 不再是中文 |
>
> ### 第五批已修復(收尾小項 + 端到端整合測試)
>
> | 項目 | 修復內容 |
> |---|---|
> | **C1** | ApplyPatches 還原/重套共用同一個 orchestrator,BCI 只解壓一次;中間狀態留在快取由最終 SaveAll 寫入(dry-run 失敗時磁碟零改動) |
> | **C5** | objdef 偵測改為單次解析(`ParseObjdefRows`),四項偵測共用 |
> | **D6(殘餘)** | `RefreshSavesAndBackups` 防重入旗標 |
> | **D7** | modifier_log.txt 超過 5MB 啟動輪替為 modifier_log.old.txt |
> | **D8(殘餘)** | 兩處 `throw new Exception` → `InvalidDataException` |
> | **新增** | `ApplyPatchesIntegrationTests`:本機遊戲檔複製到 temp,全功能套用→偵測回讀→還原→逐檔解壓後位元組比對 100% 相同(CI 自動略過) |
>
> **尚未修復(大型重構/儲存庫營運,建議各自獨立 PR 並人工過目)**:D1-D5(命名空間統一、ModifierForm 拆分、清單驅動 UI、`bool[6]` 模組對位、Tuple→record 等大型重構)、F3 完整 SkippableFact 方案(需新增套件依賴)、第七部分 git 歷史瘦身(599MB,需重寫歷史、影響所有 clone)。舊 Localization 孤兒鍵群(ColGlory*/Skills*/SpellEnhancement* 等)保留未刪——它們與 ClScriptPatcher 的 SpellEnhancement/GeneralSkills 參數同屬尚未接線的功能面,待產品決定去留。

> 產出日期:2026-07-08
> 檢查範圍:`src/` 全部 C# 原始碼(約 9,300 行)、`tests/`、`AgainstRomeModifier.csproj`、git 儲存庫結構。
> 本文件**只描述問題與建議修法,未修改任何程式碼**。供另一個 AI(或開發者)逐項修復使用。
> 重要背景:此工具會直接改寫遊戲安裝目錄的檔案,任何修復都必須保持「偵測 → 驗證 → 交易式寫入 → 可回復」的既有安全模型,不可弱化 `VerifiedBinaryWriter`/`FileRollbackScope` 的防護。修復後請執行 `dotnet test tests/AgainstRomeModifier.Tests`。

---

## 第一部分:正確性問題(建議優先修復)

### A1.【高】`CompressPfil(bytes, null!)` 必定丟例外 → cl_epara 備份自我修復是死路
- 位置:`src/Core/Services/BackupManager.cs:106` 與 `BackupManager.cs:229`
- 兩處都呼叫 `GameLZSS.CompressPfil(cleanEparaBytes, null!)`,但 `GameLZSS.CompressPfil`(`src/Core/GameLZSS.cs:282-286`)開頭就有 `ArgumentNullException.ThrowIfNull(origHeader)` 且要求 header 至少 64 bytes。
- 結果:這兩處**永遠丟出例外**,又被外層空的 `catch { }` 吞掉,所以:
  - `TryAutoHealBackupFiles` 永遠無法補回 `SYSTEM/cl_epara.ini`;
  - `TryLoadBackupFromGameDirectory`(當內嵌 Backup.zip 不存在、要從遊戲目錄自建備份時)一定會因缺 `cl_epara.ini` 而整個失敗。
- 目前僅因內嵌 Backup.zip 都存在才沒被使用者察覺,屬於潛伏的高風險 bug。
- 建議修法:自行組一個合法的 64-byte PFIL 標頭(`'P','F','I','L'` + 其餘 0)傳入,或為 `CompressPfil` 增加「無原始標頭」多載;同時把空 catch 改成記 log。

### A2.【高】cl_script.ini 補丁邏輯有兩套實作,而且行為不一致
- 生產路徑:`src/Core/Services/PatchEngine.cs:465-558`(`GetPatchedClScriptBytes`)。
- 另一套完整實作:`src/Core/Patches/ClScriptPatcher.cs` — **生產程式完全沒用到**。
- 測試現況(2026-07-08 第三輪檢查更正):兩套實作各自有測試——`PatcherRoundTripTests.ClScript_patch_...` 測的是未使用的 `ClScriptPatcher`;`ClScriptPatchTests` 則透過**反射呼叫 PatchEngine 的私有方法** `GetPatchedClScriptBytes`(見 F4)。「兩套實作並存且行為分歧」的核心問題不變。
- 行為差異:
  - `PatchEngine` 還原士氣時寫死 `3 / 5 / 50 / 200`(`PatchEngine.cs:521-539`),假設原版值固定;`ClScriptPatcher` 則是從原始備份讀回真正的原值。若遊戲版本原值不同,`PatchEngine` 會把「還原」寫成錯的值。
  - `ClScriptPatcher` 還支援 `SpellValue`/`AbilityValue`/`LPIncIdle` 等生產路徑沒有的鍵。
- 這是「測試在驗證 A 實作、產品在跑 B 實作」的典型風險。
- 建議修法:讓 `PatchEngine.GetPatchedClScriptBytes` 改為呼叫 `ClScriptPatcher.GetPatchedBytes`(以備份 bytes 作 `OriginalBytes`),刪除 PatchEngine 內的重複 regex 與行處理邏輯;或反向淘汰 `ClScriptPatcher` 並把測試改對準生產路徑。二擇一,不能維持現狀。

### A3.【高】啟動時自動執行完整 ApplyPatches,可能覆蓋使用者自訂兵種屬性
- 位置:`src/UI/ModifierForm.cs:239-251`(建構函式)。
- 行為:每次啟動都 `DetectCurrentPatchState` → `ApplyPatches`。註解寫「只對 foodHealing 修復」,實際是**完整套用流程**(先 RestoreOriginalFilesInternal 還原全部,再依偵測到的選項重套)。
- 問題:
  1. `DetectCurrentPatchState` 無法還原 `CustomUnitStats`(偵測不出使用者當初套的自訂數值,只能偵測出 `Balance=true`,見 `PatchEngine.cs:1169-1246` 的「任一屬性有差異就當 Balance」邏輯)。因此使用者若套過**自訂兵種屬性**,重開修改器的瞬間,objdef.dau 會被改寫成「內建平衡值」——自訂值默默遺失。
  2. 啟動時在 UI 執行緒同步做大量 I/O(解壓/重壓 objdef、cl_script、全部 team.dat、全部 bci),拖慢啟動且無進度提示。
  3. 若使用者的遊戲目錄有修改器不認識的第三方 mod 修改,啟動時會被 Restore 流程無聲覆蓋。
- 建議修法:啟動時只做「偵測 + 必要的安全遷移」(例如 `R0` 常駐修復與 ak_anfuehrer 閃退腳本遷移可保留,但應改為針對性修復,而不是整套 Apply);或至少在偵測到 Balance/差異時先詢問使用者。

### A4.【中】遊戲加速下拉選單的 index↔倍率換算與倍率表隱性耦合
- 位置:`src/UI/ModifierForm.Patches.cs:246-256`(`GetSelectedGameSpeedMultiplier` 回傳 `idx + 1`;`SetGameSpeedSelection` 用 `multiplier - 1`)。
- 這假設 `ExePatchModel.GameSpeedSupportedMultipliers`(`src/Core/Patches/ExePatchModel.cs:146`)永遠是連續的 `1..10`。若日後改成 `{1,2,3,5,10}` 之類,UI 會默默算錯倍率並直接寫進 EXE。
- 建議修法:兩個函式都改成以 `GameSpeedSupportedMultipliers` 陣列做 index↔值的對照,不要用算術。

### A5.【中】`LoadCurrentData` 的錯誤訊息用錯本地化鍵
- 位置:`src/UI/ModifierForm.Data.cs:1095`,catch 區塊用 `Loc.Get("LogPresetImportError")`(「預設檔匯入錯誤」)當前綴,實際是讀取現有設定失敗。應改用正確的鍵(或新增一個)。

### A6.【中】`_backupSaveCache` 的執行緒安全不完整
- `RefreshSavesAndBackups`(`src/UI/ModifierForm.SaveManager.cs:158-289`)在 `Task.Run` 背景執行緒讀寫快取時有 `lock (_backupSaveCache)`,但 `BtnDeleteBackup_Click`(`SaveManager.cs:596`)的 `_backupSaveCache.Remove(file)` **沒有加鎖**。與背景重新整理併發時可能例外或狀態不一致。加上同一把鎖即可。
- 另外 `RefreshSavesAndBackups` 是 `async void` 且會被多處連續呼叫(刪除→Refresh→again),沒有防重入;快速連點可能出現兩個併發掃描。建議加簡單的 in-flight 旗標或改 `async Task` + 序列化呼叫。

### A7.【中】`Program.Main` 把所有啟動例外吞掉,使用者只會看到程式閃退
- 位置:`src/Program.cs:16-23`。例外只寫 `crash_log.txt`,不顯示任何訊息。建議 catch 後additionally `MessageBox.Show`(在 `Application.SetCompatibleTextRenderingDefault` 之前要小心,可用 `MessageBox.Show` 的原生形式)。
- 另:提權失敗(使用者按 UAC 取消)時也只是默默記 log 然後退出,建議提示「需要系統管理員權限」。

### A8.【低】`ExePatchModel.Apply` 的文件註解與實際行為不符
- 位置:`src/Core/Patches/ExePatchModel.cs:355-360`。註解說「任一偏移不符預期即中止並丟例外,**緩衝區不被破壞**」,但多筆 `ExeWriteOp` 是逐筆套用:第 1 筆成功、第 2 筆失敗時,緩衝區已被第 1 筆改過。目前呼叫端(dry-run 模式,失敗就不寫檔)不受影響,但註解會誤導後續維護者。修正註解,或改成先驗證全部 expected 再一次寫入(更安全)。

### A9.【低】`ModifierForm` 字型釋放掛在 `FormClosing` 而非 `Dispose`
- 位置:`src/UI/ModifierForm.cs:256-274`。正確做法是覆寫 `Dispose(bool disposing)`(WinForms 慣例),`FormClosing` 在某些關閉路徑(例外、`Environment.Exit`)不保證觸發。同樣的 pattern 也出現在 `TroopPresetForm`(它有 `fontsDisposed` 欄位,處理方式亦可統一)。

### A10.【低】`DetectCurrentPatchState` 大量空 `catch { }`
- 位置:`src/Core/Services/PatchEngine.cs:1083、1111、1148、1248、1274、1287、1295、1302、1310`。偵測失敗一律靜默,UI 會顯示「未勾選」而不是「偵測失敗」。至少應記 log(`_logger` 就在手邊),理想上讓 UI 能區分「off / unknown」。

---

## 第二部分:重複程式碼(維護風險最大的一類)

這個專案最大的結構性問題是「UI 層留有服務層搬遷前的舊副本」,而且部分副本已經**與正本內容出現分歧**。修復方向一律是:刪除 UI 副本、統一呼叫 `Core` 服務層。

### B1.【高】`GetCleanEparaText` 有兩份,內容還不一樣
- `src/Core/Services/BackupManager.cs:545-656`(正本,生產使用)與 `src/UI/ModifierForm.Data.cs:1235-1345`(死副本,無任何呼叫者)。
- 分歧點:`[CloudReflectMoveSpeed]` 在 BackupManager 版是 `16`(原版預設),Data.cs 版是 `27`。若日後有人「順手」接回 UI 版,會寫入非原版值。
- 修法:刪除 `ModifierForm.Data.cs` 的整段副本。

### B2.【高】objdef 偵測三兄弟函式完整重複兩份
- `HasHousingCapacityMultiplier` / `HasStorageCapacityMultiplier` / `HasFastBuildUpgradeRepair`:
  - 正本:`src/Core/Services/PatchEngine.cs:1317-1446`(生產使用)
  - 死副本:`src/UI/ModifierForm.Data.cs:68-178`(無呼叫者)
- 修法:刪除 Data.cs 三個副本。

### B3.【中】屬性解析函式三重複
- `GetMeleeAndRangedDmg` / `GetMeleeAndRangedRelt` / `GetUnitMaxRange` / `MergeUnitStatsLayers` 存在於:
  1. `src/Core/Services/BackupManager.cs:455-543、434-453`(正本,static public)
  2. `src/UI/ModifierForm.Data.cs:1103-1178、393-411`(UI 私有副本,`LoadDefaultStatsData`/`LoadCurrentData` 在用)
  3. `src/Core/Patches/ObjdefPatcher.cs:82-84`(`GetDamage`/`GetReload`/`GetMaxRange`,壓縮寫法的第三份)
- 三份判斷 ranged 武器的規則目前一致(`Dtyp ∈ {1,2,3,4} || siege`),但任何一份改動都會造成 UI 顯示與實際寫入不一致。
- 修法:UI 改呼叫 `BackupManager` 的 static 版本;`ObjdefPatcher` 亦可共用(注意它先對 source 做過 `Trim()`,行為等價)。

### B4.【中】`SafeWriteAllBytes` 兩份完全相同
- `src/Core/Services/PatchEngine.cs:97-163`(public)與 `src/Core/EndlessAi/EndlessAiOrchestrator.cs:413-479`(private static,連註解都一樣)。
- 修法:抽成單一 static 工具類(例如 `Core/Services/SafeFileWriter`),兩邊共用。也順便統一 `throw new Exception(...)` → 應丟 `IOException` 等具體型別。

### B5.【中】偵測用 Regex 兩份
- `RegexSpellLoad`/`RegexCiviLoad`/`RegexMorale*Load` 同時宣告於 `src/Core/Services/PatchEngine.cs:31-36`(生產)與 `src/UI/ModifierForm.Data.cs:17-22`。UI 版只有 `RegexSpellLoad`(line 957)被用到,其餘四個是死的。
- 修法:UI 需要的話從服務層公開,其餘刪除。

### B6.【低】UI 繪製輔助重複
- `GetRoundPath` / `StyleButton` / 標題列拖曳三事件(`TitleBar_MouseDown/Move/Up`)在 `ModifierForm.cs` 與 `TroopPresetForm.cs` 各有一份;`CreateBaseGrid`(`ModifierForm.cs:2289`)與 `CreateCurrentStatsGrid`/`CreateDefaultStatsGrid`/`CreateSaveGrid`/`TroopPresetForm.CreateGrid` 的深色 DataGridView 樣式設定重複五次。
- 修法:抽 `UI/Theme.cs`(顏色常數 + `ApplyDarkGridStyle(dgv)` + `StyleButton` 擴充方法);顏色魔術數字(`Color.FromArgb(0,230,255)` 出現 20+ 次)集中成命名常數。

### B7.【低】`CheckLen`(UI)與 `PatchText.CheckLength`(Core)重複
- `src/UI/ModifierForm.Data.cs:1205-1223` 的 `CheckLen` 已無呼叫者(正本為 `src/Core/Patches/PatchText.cs:27-44`),刪除即可。

---

## 第三部分:死程式碼清單(可直接刪除)

| 位置 | 項目 | 說明 |
|---|---|---|
| `src/UI/ModifierForm.Data.cs:24-26` | `LoadBackupZipToMemory()` 包裝 | 無呼叫者(ModifierForm.cs:237 直接呼叫 backupManager) |
| `src/UI/ModifierForm.Data.cs:28-41` | `EnsureBackupLoadedForGamePath`/`TryLoadBackupFromGameDirectory` 包裝 | 確認呼叫端後可簡化;`TryLoadBackupFromGameDirectory` 包裝無人使用 |
| `src/UI/ModifierForm.Data.cs:68-178` | objdef 偵測三函式副本 | 見 B2 |
| `src/UI/ModifierForm.Data.cs:19-22` | 4 個 Morale/Civi 偵測 Regex | 見 B5 |
| `src/UI/ModifierForm.Data.cs:859-897` | `IsMaximumPopulationApplied` | 無呼叫者(偵測已改用 PatchEngine 的 byte 比對) |
| `src/UI/ModifierForm.Data.cs:1205-1223` | `CheckLen` | 見 B7 |
| `src/UI/ModifierForm.Data.cs:1235-1345` | `GetCleanEparaText` 副本 | 見 B1,**內容已分歧** |
| `src/UI/ModifierForm.cs:1873-1876` | 空的 `<summary>` 註解殘骸(「確保備份的 objdef.dau…」下面沒有方法) | 刪除 |
| `src/Core/Patches/ClScriptPatcher.cs` | 若採 A2 的「統一到 PatchEngine」方案 | 二擇一處理,不可放著 |
| `tools/__pycache__/bcitool.cpython-311.pyc` | 追蹤進 git 的 Python 快取 | 從 git 移除並加入 .gitignore |

刪除以上死碼估計可讓 `ModifierForm.Data.cs` 縮減約 300 行。

---

## 第四部分:效能建議

### C1.【中】啟動流程的重複解壓
- `EndlessAiOrchestrator` 每次 `new` 都重建檔案快取;`DetectCurrentPatchState`、`ApplyPatches`、`RestoreOriginalFilesInternal` 各自 new 一個 orchestrator,同一批 `.bci`(5 份 ak_level + haupthaus + Dorfverteidigung + npc + produktion)在一次「套用」流程中會被完整讀檔+LZSS 解壓多次。搭配 A3 的啟動自動套用,啟動時 I/O 相當可觀。
- 修法:讓一次操作流程共用同一個 orchestrator 實例(PatchEngine 建立並傳遞),或給 `BciScriptFile` 加上以 (path, mtime) 為鍵的程序級快取。

### C2.【低】`GameLZSS.Compress` 用 `List<byte>` 累積輸出
- `src/Core/GameLZSS.cs:88`。對數 MB 的 objdef 會有大量 grow/copy。改用 `MemoryStream` 或 `ArrayBufferWriter<byte>`,並可預先 `capacity = input.Length`。
- 同檔 `head = ArrayPool<int>.Shared.Rent(65536)` 之後全量初始化 65536 格——rent 回來的陣列可能更大,無礙正確性,但直接 `new int[65536]` 反而更清楚(初始化成本一樣要付)。

### C3.【低】`TroopConfig` 靜態建構的 O(n²) 排序
- `src/Core/TroopConfig.cs:248-255`:比較器內呼叫 `UnitOrder.IndexOf(a/b)`,對 44 個單位無感,但寫法可改為先建 index 字典。順手修即可,優先度低。

### C4.【低】導覽按鈕 `MouseMove → Invalidate()`
- `src/UI/ModifierForm.cs:1938`:滑鼠在按鈕上移動時每個事件都整鈕重繪。Hover 狀態只需 Enter/Leave 兩事件即可(Paint 內已用 `Cursor.Position` 判斷,MouseMove 的 Invalidate 是多餘的)。

### C5.【低】`DetectCurrentPatchState` 內多次全文 split + 全行 regex
- objdef 偵測把同一份 current/original 文本 split 了 4 次(Housing/Storage/FastBuild/Balance 各自 split+parse)。可一次 parse 成 `Dictionary<name, cols>` 後共用。屬讀取路徑,量不大,列為順手優化。

---

## 第五部分:架構與可維護性

### D1. 命名空間不一致
- `Core/EndlessAi/*`、`GameLZSS`、`TroopConfig`、`FileRollbackScope`、`Localization` 都在根命名空間 `AgainstRomeModifier`;而 `Core/Patches/*`、`Core/Services/*` 在 `AgainstRomeModifier.Core.*`。UI 內大量寫全名 `AgainstRomeModifier.Core.Services.BackupManager`。建議統一為 `AgainstRomeModifier.Core.*` 並補 `using`。

### D2. `ModifierForm` 職責過重(2,325 行 + partial 4 份)
- UI 建構全部手寫絕對座標(`new Point(25, 80)` 式魔術數字數百處),又疊了一層 `ApplyModernLayout` 重新覆蓋初始位置——同一控制項的位置被設定兩次(例如 `InitializeComponent` 排一次、`ConfigureSettingsCard`/`LayoutModernShell` 再排一次)。建議:
  1. 刪除 `InitializeComponent` 中會被 `ApplyModernLayout` 覆蓋的初始座標/尺寸(保留由現代化布局統一負責),減少「兩套布局」的困惑;
  2. 開關項(chkXxx + help label + Loc 鍵 + tooltip 鍵)重複樣板 17 組,可用一個 `record ToggleSpec(string LocKey, string TipKey, ...)` 清單驅動生成,`ApplyLanguageToUI`(2,000 行附近的手動逐一賦值)與 `BtnEnableAll/DisableAll`(Presets.cs 手抄 22 個開關)即可改為迴圈,新增功能開關時不再需要改 5 個地方。

### D3. `ApplyLanguageToUI` / `UpdateGridHeaders` 的手工映射
- `src/UI/ModifierForm.cs:1970-2171`:每個控制項一行 `X.Text = Loc.Get(...)`,17 個欄位標頭 × 2 組 grid 逐一 if。配合 D2 的清單驅動可縮到十幾行,且不會漏更新。

### D4. `PatchOptions.EndlessAiModules` 用裸 `bool[6]` 與 UI 六個 chk 手動對位
- `src/Core/Services/PatchOptions.cs:27`、`ModifierForm.Patches.cs:108-110`、`ModifierForm.Data.cs:940-945`。索引意義只存在於註解。建議改為 `Dictionary<string,bool>`(以模組 Id "M1".."M6" 為鍵)或六個具名 bool,消除順序耦合(`Orchestrator.UserModules` 順序若調整,現在會默默錯位)。

### D5. `TroopConfig.UnitMeta` 的 `Tuple<string,string,string,string>`
- `meta.Item1..Item4` 可讀性差(faction/tier/type/style 全靠註解)。改 `record UnitMeta(string Faction, string Tier, string UnitType, string Style)`,同時 `utype == "ranged_inf"` 這類魔術字串可改 enum。影響面廣但機械性,適合交給 AI 一次性重構,重構後務必跑測試。

### D6. `async void` 事件處理
- `BtnApply_Click`、`RestoreAll`、`RestoreStatsOnly`、`RestoreCompatOnly`、`RestoreLanguageOnly`、`RefreshSavesAndBackups` 都是 `async void`。WinForms 事件處理器可接受,但這些方法內部已自行 try/catch 全包,唯 `RefreshSavesAndBackups` 若在 await 前丟例外(例如 grid 尚未初始化)會直接炸程序。維持現狀可,但建議至少為 `RefreshSavesAndBackups` 補齊防重入(見 A6)。

### D7. Log 檔無上限
- `modifier_log.txt` 只增不減(`ModifierForm.Patches.cs:17-26`)。建議啟動時檢查大小,超過(如 5MB)即輪替。

### D8. 例外型別
- 多處 `throw new Exception(...)`(`PatchEngine.SafeWriteAllBytes`、`ApplyExePatch` 等)。建議改用 `IOException`/`InvalidDataException`/`InvalidOperationException` 等語義型別,呼叫端已按這些型別做訊息分類。

---

## 第六部分:測試與 CI

1. **A2 修復後必須調整測試對象**:`ClScriptPatchTests` 目前測的是生產未使用的 `ClScriptPatcher`。統一實作後,測試應對準實際生產路徑。
2. `PatchEngine.DetectCurrentPatchState` 與 `GetPatchedClScriptBytes` 目前無測試覆蓋(測試集中在 patcher 純函式與 Endless 補丁)。偵測邏輯是「重開修改器會不會弄壞使用者檔案」的關鍵(見 A3),值得補測試:以 SyntheticFixture 產生「已套用 X」的檔案,驗證 Detect 回讀正確。
3. `tests/verify_split_patches/` 是獨立 console 驗證程式(479 行),與正式測試專案並存。若 CI 沒有跑它,建議併入 xUnit 測試或在 README 註明用途,避免腐化。
4. `BackupManager.TryLoadBackupFromGameDirectory` 的無 Backup.zip 回退路徑(A1)完全沒測試——正因如此 `null!` bug 才存活至今。修 A1 時請加一個測試。

---

## 第七部分:儲存庫衛生

1. **git pack 高達 599 MiB**(工作樹追蹤檔案僅 93 個、最大不過 440KB)。歷史中曾提交過大型二進位(Backup.zip、遊戲檔案、Ghidra 工具等)。建議:若允許重寫歷史,用 `git filter-repo`/BFG 清理;若不允許,至少在 README 註明 clone 建議使用 `--depth 1`。
2. `tools/__pycache__/bcitool.cpython-311.pyc` 被追蹤,應移除並在 `.gitignore` 加 `__pycache__/`。
3. 工作目錄殘留大量未追蹤的產出物:`bin/CombatCrashRepair*`、`bin/PreRestoreApplyValidation`、`re_workspace/`、`scratch/`、`.codex/`(內含整套 Ghidra)、`遊戲原始檔案/`。它們未被 git 追蹤(好),但建議在 `.gitignore` 明列,避免日後誤加。
4. `AgainstRomeModifier.csproj.user` 存在於根目錄——確認是否被追蹤,`.user` 檔不應進版控。

---

## 第八部分:建議修復順序(給修復 AI 的執行清單)

1. **A1** `CompressPfil(null!)` 修復 + 補測試(小改動、高風險消除)。
2. **B1、B2、B5、B7、第三部分死碼**:純刪除,零行為風險,先做可大幅縮小後續 diff。
3. **A2** cl_script 補丁實作統一(此項改動最需要小心,務必用既有 `ClScriptPatchTests` + 新增 round-trip 測試守住)。
4. **A5、A8、A9、B4、B6、D8**:小型清理。
5. **A4** 遊戲加速 index 映射改為查表。
6. **A6** 快取鎖 + Refresh 防重入。
7. **A3** 啟動自動套用行為調整(需要與專案擁有者確認期望行為:建議「啟動只做 R0/閃退腳本安全遷移,不做完整 Apply」)。
8. **B3、C1** 屬性解析統一與 orchestrator 快取共用。
9. **D2、D3、D4、D5**(大型重構,分開 PR,每步跑測試)。
10. 第六、七部分的測試與 repo 衛生項目。

---

## 第九部分:第二輪深入檢查新增發現(2026-07-08 補充)

### E1.【高】`ObjdefPatcher.PatchUnit` 武器射程欄位 off-by-one,會誤改「武器角度」欄位
- 位置:`src/Core/Patches/ObjdefPatcher.cs:66-68`(`PatchUnit` 武器迴圈)與同檔 `GetMaxRange`(line 84)。
- 程式用 `min = active + 3, max = active + 4`,即欄位 **81 與 82**。但依照:
  - `ObjdefIndex` 列舉(`src/Core/TroopConfig.cs:16-17`):`Weapon1RangeMin = 80`(= active+2)、`Weapon1RangeMax = 81`(= active+3);
  - 專案自己的逆向文件 `docs/reverse-engineering/objdef-fields.csv`:**index 82 = Weapon1Angle,註記明寫「never scale as range」**;
  - `BackupManager.GetUnitMaxRange`(`src/Core/Services/BackupManager.cs:528-530`,UI 顯示用)正確使用 80/81。
- 後果(啟用 Balance 或自訂兵種屬性、rangeScale ≠ 1 時):
  1. 武器**角度欄位(82)被當射程縮放**——逆向文件明確禁止的操作,可能影響射擊扇形/命中判定;
  2. 真正的 RangeMin(80)**從未被縮放**,遊戲內最小射程與 UI 預覽不一致;
  3. `GetMaxRange` 把角度值納入 `Math.Max`,rangeScale 分母可能算錯。
- **測試沒抓到的原因**:`PatcherRoundTripTests.Objdef_...`(`tests/.../PatcherRoundTripTests.cs:18`)傳入 `NoStats`,`PatchUnit` 的武器迴圈完全沒有測試覆蓋。
- 建議修法:改為 `min = active + 2, max = active + 3`(`GetMaxRange` 同步修正),並新增含武器欄位(80/81/82 都填值)的 SyntheticFixture 回歸測試,斷言 82 欄位在任何縮放下不變。
- 附帶說明:因 Apply 一律從備份原檔重新生成,舊版錯誤寫入會在下一次套用時自我修復,無需額外遷移。

### E2.【中】本地化鍵交叉比對結果(以指令碼全量比對得出)
1. `Loc.Get("LogRestoreAllSuccess")`(`src/UI/ModifierForm.Patches.cs:184`)——**該鍵在中英字典都不存在**,還原成功時 log 會顯示原始鍵名。正確鍵應為既有的 `LogRestoreAllDone`(`src/Core/Localization.cs:328、593`)。
2. `AiM6Tip` 只在中文字典定義(`Localization.cs:419`),**英文字典缺漏** → 英文介面 M6 開關的 tooltip 顯示 "AiM6Tip"。
3. **服務層 log 全部硬編碼中文**:`PatchEngine`/`BackupManager` 的 `_logger.Log("已套用視窗失去焦點不暫停補丁。")` 等 30+ 處,而 `Localization.cs` 中早已定義好的對應鍵(`LogExePatchFocus`、`LogExePatchOrig`、`LogDgVoodooInstalled/Removed/Preserved/NotManaged`、`LogLangToEng/ToOrig`、`LogVillageBuildRange*`、`LogRestored`、`LogPreApplyRestore` 等)全數成為孤兒——邏輯從 UI 搬到服務層時沒把 `Loc.Get` 帶過去。英文使用者的操作 log 是中文。修法:服務層 log 改回用 `Loc.Get`(鍵都是現成的)。
4. 孤兒鍵群 `ColGlory*`、`GrpGeneralSkills`、`GrpLeaderGlory`、`NavSkills`、`SkillsHeading/Subtitle`、`SpellEnhancement(Tip)`、`ModSkillsAndGlory(Tip)` 屬於已移除的「技能與榮耀」功能,與 A2 的 `ClScriptPatcher`(`SpellEnhancement`/`GeneralSkills` 參數)同源;處理 A2 時一併決定去留。
5. **給修復 AI 的警告**:`NavSystem`/`NavDefaultStats`/`NavCurrentStats`/`NavSaveManager`/`NavDoc` 在靜態掃描下看似未使用,實際經 `StyleNavButton(btn, key, …)` → `Loc.Get(key)` **動態使用**(`ModifierForm.cs:1920`),**不可刪除**。

### E3.【中】app.manifest 已設 `requireAdministrator`,`Program.cs` 的手動提權是死碼
- `app.manifest:7` 為 `requireAdministrator` → OS 在行程啟動前就強制 UAC,`Program.RunApplication`(`src/Program.cs:27-61`)的 `isAdmin` 檢查與 `runas` 重啟分支**永遠不會走到**。
- 二選一:保留 manifest、刪除手動提權碼(建議,較簡單);或 manifest 改 `asInvoker` 並保留手動提權(可實現「非管理員也能開 UI、按需提權」,但要處理重啟後狀態)。現狀雙軌並存會誤導維護者。

### E4.【中】ApplyPatches 對內容未變的檔案也全部重寫
- `RestoreStatsOnlyInternal` + `patchedFiles` 寫入迴圈 + `GetPatchedTeamDatBytes`:每次「執行修改」(加上 A3,等於**每次啟動**)都把 objdef、cl_script、ress、cl_epara、cl_scint 與**所有地圖的 team.dat** 重寫一遍,即使位元組完全相同。
- 建議:`SafeWriteAllBytes` 寫入前先比對目標檔現有內容,相同即跳過(省 SSD 寫入、避免 mtime 汙染、縮小 rollback 備份量與失敗回復面)。

### E5.【低】TroopPresetForm 匯入/驗證細節
- `BtnImport_Click` 在解析**之前**就設定 `LoadedFileName`(`src/UI/TroopPresetForm.cs:417`);解析失敗後狀態殘留,主表單可能顯示「已載入 XXX」。`BtnExport_Click:496` 同樣在寫檔前設定。應成功後才賦值。
- 匯入用 `double.Parse`(433-442),檔案中一行格式錯誤 → 整批中止;建議 `TryParse` 逐行容錯並回報略過的行數。
- 匯入時把值 `Math.Round` 後寫回表格(462-472),再由 Apply 從表格讀回 → `.artroop` 檔內的小數精度在匯入瞬間遺失(HP/VW/AW/Sight/Relt/Range 全被取整)。
- `ValidateGridInputs`(569-667)九個欄位九段幾乎相同的驗證共約 100 行,可用 `(欄名, 訊息鍵, 是否允許0)` 清單驅動;訊息字串亦應改用 `Loc.Get` 而非 inline 中英文(整個 TroopPresetForm 都用 `isEn ? ... : ...` 內嵌雙語,與主表單的 Loc 機制不一致)。

### E6.【低】兵種圖示路徑硬編碼語系資料夾 "US"
- `LoadIcons`(`src/UI/ModifierForm.Data.cs:365`)固定讀 `SYSTEM/CLMK/DLG/IGM0806/US/`。非英文語系的遊戲封裝若使用其他資料夾(如 DE),圖示會整批載入失敗(僅剩 log)。建議 US 找不到時列舉 `IGM0806/` 下實際存在的語系資料夾。

### E7.【低】人口上限 1600 魔術數字出現三處
- `TeamDatOptions` 預設參數(`src/Core/Patches/TeamDatPatcher.cs:3`)、`PatchEngine.cs:612` 的 log 字串、`ModifierForm.Data.cs:886`(死碼 `IsMaximumPopulationApplied`)。統一為單一常數,log 改引用該常數。

### E8.【低】CI 缺口
- `.github/workflows/ci.yml` 只建置與執行 `AgainstRomeModifier.Tests`;`tests/verify_split_patches`(479 行驗證程式)**完全不在 CI 內**,連編譯都沒驗證,會無聲腐化。至少加一步 `dotnet build tests/verify_split_patches`。
- E1 的教訓:round-trip 測試對「未啟用的路徑」給 `NoStats` 形同空轉,補測試時應讓每個 option 至少有一組非預設值的斷言。

### E9.【低】GER 祭司在「當前兵種數值」頁顯示可變法術半徑的假象
- `LoadCurrentData`(`src/UI/ModifierForm.Data.cs:1056-1058`)對所有 `utype == "priest"` 顯示 `500 → 500×倍率` 對比,但 `SupportsConfigurableSpellRadius` 只允許 KEL/HUN。GER 祭司實際不可設定卻顯示變動值。顯示層應套用相同的白名單。

### 修復順序補充
- **E1 併入第 1 批**(與 A1 同級:小 diff、高風險、可立即加測試守住)。
- E2-1、E2-2(缺鍵)併入第 4 批小型清理;E2-3(服務層 log 本地化)可獨立一個 PR。
- E3、E4 併入第 7 批(與 A3 同屬「套用流程行為」,一起討論)。
- 其餘 E5-E9 依序排入小型清理。

---

## 第十部分:第三輪檢查——測試品質與偵測語意(2026-07-08 補充)

### F1.【高】`DetectModule` 對「找不到任何檔案」回傳 Original,造成偵測與測試雙重假象
- 位置:`src/Core/EndlessAi/EndlessAiOrchestrator.cs:263-308`。
- 當 `ResolvePaths` 回傳 0 個檔案時,程式只把 `allUltimate = false`,`allOriginal` 保持 true → **對一個空目錄/錯誤路徑偵測,結果是 `Original`**,而不是 Unknown。
- 兩個下游後果:
  1. **生產面**:遊戲路徑打錯或 MAPS 缺失時,UI 的 M1..M6 顯示為「未套用(原版)」,看似合理實則什麼都沒偵測;`ApplyModule(disabled)` 也會靜默跳過。
  2. **測試面**:`CheckGameStatusTest.VerifyOriginalGamePatches`(`tests/.../CheckGameStatusTest.cs:89-106`)依賴**未進版控**的 `遊戲原始檔案/` 目錄;在 CI 上該目錄不存在 → 0 個檔案 → DetectModule 回傳 Original → `Assert.Equal(Original, state)` **空洞地通過**。CI 綠燈但整條驗證什麼都沒驗。
- 建議修法:所有 patch 都解析到 0 個檔案時回傳 `PatchState.Unknown`(或新增 `NotFound`);`VerifyOriginalGamePatches` 開頭加 `Assert.True(Directory.Exists(...))` 或目錄不存在時明確 Skip(見 F3)。

### F2.【中】`CheckGameStatusTest.VerifyRealGamePatches` 是偽裝成測試的除錯腳本
- 位置:`tests/.../CheckGameStatusTest.cs:17-86`。
- 問題:(1) 硬編碼開發者機器的真實安裝路徑 `C:\Program Files (x86)\Against Rome`;(2) **整個測試沒有任何 Assert**,只有 WriteLine——永遠綠燈;(3) 內含一大段 P8/P9 專用 trace 與過期簽章的複本(line 44-74),與產品碼漂移後只會輸出誤導資訊。
- 建議修法:移出測試專案改為 `tools/` 下的診斷程式,或加上環境變數 gate(`AR_GAME_PATH` 未設定即 Skip)並補上實際斷言。

### F3.【中】「無資源就 return」的靜默略過模式讓 CI 統計失真
- 位置:`tests/.../ClScriptPatchTests.cs:25-28`(Backup.zip 缺失時直接 `return`)。
- CI 上這個測試永遠顯示 Passed,實際上一行斷言都沒跑。應改用 `Xunit.SkippableFact`(或至少 `Assert.Skip` 風格的自訂機制),讓測試報告顯示 Skipped 而非 Passed,維護者才知道 CI 的真實覆蓋率:**目前 CI 上真正有效的測試只有純合成 fixture 那幾檔**(PatcherRoundTrip、ExePatchModel、VerifiedPatch、Reinforcement/RomanFounder/SettlePlace/SettledPartyCleanup)。
- 同檔 line 53 有一句無用的 `engine.DetectCurrentPatchState("C:\\dummy", ...)` 呼叫,結果未使用,可刪。

### F4.【中】`ClScriptPatchTests` 以反射呼叫私有方法
- `tests/.../ClScriptPatchTests.cs:32-37` 用 `GetMethod("GetPatchedClScriptBytes", NonPublic)` 反射呼叫。任何重構(包括 A2 的統一方案)都會讓它在**執行期**才失敗,且參數列變更時只會 `TargetParameterCountException`。
- 建議修法:把方法改為 `internal` + `InternalsVisibleTo("AgainstRomeModifier.Tests")`,刪除反射。

### F5.【中】空 `catch { }` 全清單(21 處)
- 前文 A10 只列了 DetectCurrentPatchState;完整清單如下,修復時建議統一改為「記 log 後繼續」:
  - `src/Core/Services/PatchEngine.cs`:136、154(SafeWriteAllBytes 清理 temp 檔)、1083、1111、1148、1248、1274、1287、1295、1302、1310(偵測)
  - `src/Core/Services/BackupManager.cs`:109、121、143、231(備份自我修復——**A1 的 bug 正是被 109/231 吞掉**)
  - `src/Core/EndlessAi/EndlessAiOrchestrator.cs`:452、470(temp 檔清理)
  - `src/UI/ModifierForm.Data.cs`:203(registry 偵測)、964(cl_script 讀取)
  - `src/UI/ModifierForm.SaveManager.cs`:426(zip temp 清理)
- temp 檔清理類可保留但至少 `Debug.WriteLine`;資料讀取/修復類必須記 log。

### F6.【低】`GameLZSS.Decompress` 對截斷輸入的收尾行為
- 位置:`src/Core/GameLZSS.cs:46`。內層 `if (ip + 1 >= input.Length) break;` 跳出 for 後,外層 `while` 若 `ip` 仍小於長度,會把**殘留的半個字典指標位元組當成新的 flags** 再讀一輪,對截斷/損毀檔案產生無聲垃圾輸出而非停止。對正常檔案無影響;建議偵測到不完整 token 時直接結束主迴圈(或丟 `InvalidDataException`,與現有 50MB 防護一致)。

### F7.【低】golden 值防護只存在於不會自動執行的 harness
- `tests/verify_split_patches/verify_split_patches.cs` 的「測試 2.5 獨立 golden 值驗證」(line 132-459)是**唯一**一處以逆向文件硬編碼值反向驗證產品常數的防護(設計良好,刻意不引用產品常數)。但它:
  1. 依賴未進版控的 `遊戲原始檔案/`;
  2. 不在 CI 內(E8);
  3. 是手動 console 程式,無人記得跑就形同不存在。
- 建議:在 README/CONTRIBUTING 註明「改動任何 P1-P19 常數前必跑」,並在 CI 至少驗證其可編譯;長期可把 golden 斷言改吃合成 fixture。

### F8.【資訊】對修復 AI 的既有描述更正
- 第一部分 A2 原寫「測試只驗證 ClScriptPatcher」——經第三輪確認,`ClScriptPatchTests` 實際以反射測 PatchEngine 私有方法(本機有 Backup.zip 時)。A2 的核心問題(兩套分歧實作並存)不變,A2 內文已同步更正。

### 修復順序補充(第三輪)
- **F1 併入第 1 批**(小 diff,同時修正生產偵測語意與 CI 假綠燈)。
- F2、F3、F4 併入第六部分測試工作,建議在動 A2 之前先完成 F4(否則反射測試會在重構時炸)。
- F5 與 A10 合併為同一個「空 catch 治理」PR。

---

## 第十一部分:第四輪檢查——設定檔、工具與「已驗證安全」清單(2026-07-08 補充)

### G1.【中】`.gitignore` 的未錨定模式是顆定時炸彈
- 位置:`.gitignore` 的「Original game assets」區段。
- `MAPS/`、`SYSTEM/`、`SAVE/`、`ToEng/`、`Against_Rome.exe`、`Backup.zip`、`*.log`、`*.bin`、`*.pdb`、`*.dmp` 這些模式**沒有以 `/` 開頭錨定在根目錄**,因此在**任意深度**生效。後果:
  - 未來任何人想放 `tests/fixtures/SYSTEM/...`、`tests/fixtures/MAPS/...` 或任何 `.bin` 測試資料,都會被 git 無聲忽略;
  - 這不是假設——**同類事故已經發生過**:commit `f38e8fc`「修復 CI:Backup.zip 被 gitignore 時略過 ClScript 測試」正是這組全域 ignore 造成的(也是 F3 靜默略過模式的根源)。
- 建議修法:凡是「只該忽略根目錄那一份」的項目一律加 `/` 前綴(`/MAPS/`、`/SYSTEM/`、`/Backup.zip`、`/Against_Rome.exe` 等);`*.bin`/`*.log` 改成限定目錄或保留但在 tests/ 下用 `!tests/**` 反排除。

### G2.【已驗證安全】Windows-1251 編碼往返無損 — 修復時不可更換編碼
- 以 .NET 實測全部 256 個位元組:`GetEncoding(1251).GetString → GetBytes` **round-trip 100% 無損**。
- 意義:`PatchText`/各 patcher 以 CP1251 做「bytes → string → 改行 → bytes」的管線是位元組安全的,即使遊戲檔實際是德文(CP1252 內容)也不會被破壞——**前提是維持 1251**。修復 AI 不可「順手」改成 UTF-8 或 1252(1252 的 0x81/0x8D/0x8F/0x90/0x9D 未定義字元行為不同,可能破壞往返)。列入紅線。

### G3.【已驗證安全】本地化佔位符全數一致
- 以指令碼全量比對:Zh/En 同鍵的 `{0}`/`{1}` 佔位符集合完全一致;所有 `string.Format(Loc.Get(key), …)` 呼叫的參數數量與字串需求匹配;沒有「字串含佔位符卻被 `+` 串接」的情況。此面向無需修復(E2 的缺鍵/孤兒鍵問題仍在)。

### G4.【低】`tools/Repair-LanguageBackup.ps1` 品質良好,但邊界值得註明
- 腳本本身寫得謹慎(hash 三方驗證、temp 目錄原子 Move、拒絕清理不安全路徑)。兩個小點:
  1. 預設 `$GamePath = 'C:\Program Files (x86)\Against Rome'`,會直接對真實安裝目錄建立 `.against-rome-modifier-language-backup`——它是「修改器語言備份」的體外修復工具,屬刻意設計,但 README 未記載其用途與時機,建議補文件;
  2. 自動尋找原版目錄的邏輯(line 11-21)假設 repo 根下**恰好一個**含 `Against_Rome.exe` + `ToEng` 的目錄,多於一個即失敗——可接受,但錯誤訊息可列出找到的候選。

### G5.【低】LZSS 演算法現有第三份實作(Python)
- `tools/bcitool.py` 的 `pfil_decompress` 是 `GameLZSS.Decompress` 的 Python 移植(環狀視窗 0x20/0x00 初始化與 C# 一致,已核對)。工具性重複可接受,但**任何人修改 C# 的 LZSS 行為時必須同步這份**,否則逆向工具的輸出會與產品脫節。已加入紅線。

### G6.【資訊】合成 fixture 測試群健康度確認
- `ExePatchModelTests`(20 個 Fact/Theory)、`ReinforcementGatePatchTests`、`RomanFounderGatePatchTests`、`SettlePlacePatchTests`、`SettledPartyCleanupPatchTests` 全部使用自建合成 fixture,不依賴外部資源——這些就是 CI 上目前**真正有效**的測試(對照 F3)。結構無問題,不需修改;唯 E1(ObjdefPatcher off-by-one)所需的武器欄位測試仍待補。

### 修復順序補充(第四輪)
- G1 併入第七部分 repo 衛生工作,建議與 F3(靜默略過)同一個 PR 處理,因為兩者是同一事故鏈(gitignore 吞資源 → 測試靜默跳過 → CI 假綠燈)。
- G2、G3、G6 無需動作,列出目的是**防止修復 AI 畫蛇添足**。

### 修復時的紅線(務必遵守)
- 不得改動任何補丁的位元組簽名、偏移、期望值(`ExePatchModel`、`CustomPatches`、`BciLiteralPatch` 的常數是逆向工程結論,`docs/reverse-engineering/` 與 `TechDoc.md` 是其依據)。
- 不得移除「先驗證期望值再寫入」的 `VerifiedBinaryWriter` 防護,以及 Unknown 狀態即中止的行為。
- 不得直接對遊戲安裝目錄(`C:\Program Files (x86)\Against Rome`)做測試性寫入;測試一律走 `tests/` 的合成 fixture。
- PFIL/LZSS 的環狀視窗初始化(前 0xFEE 空格、尾 18 個 0x00)是與遊戲 EXE 位元相容的關鍵,不可「順手優化」;若修改 C# 的 LZSS 行為,必須同步 `tools/bcitool.py` 的 Python 移植版。
- 文字補丁管線必須維持 Windows-1251 編碼(已實測 256 位元組往返無損);不可改成 UTF-8 或 CP1252。
- `Localization.cs` 的 `NavSystem`/`NavDefaultStats`/`NavCurrentStats`/`NavSaveManager`/`NavDoc` 等鍵是動態查找使用的,靜態掃描看似未用,不可刪除(詳見 E2-5)。
