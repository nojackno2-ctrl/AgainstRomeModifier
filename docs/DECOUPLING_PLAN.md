# Against Rome Modifier — 功能解耦計畫（AI 執行交接文件）

> 本文件是交給 AI 代理執行的完整工作規格。執行者不需要事先了解本專案，
> 依本文件的順序執行即可。每個 Phase 結束時必須建置成功、測試全綠才能進入下一個 Phase。

---

## 0. 專案背景與硬性規則（執行前必讀）

### 0.1 專案是什麼

- 一個 Windows Forms（.NET 8，`net8.0-windows`）單機工具，用來修改 2004 年遊戲《Against Rome》的檔案：
  EXE 位元組補丁、PFIL/LZSS 壓縮的 ini/dau 文字檔、BCI 位元組碼腳本、語言包覆蓋、dgVoodoo2 檔案投放。
- 主專案：根目錄 `AgainstRomeModifier.csproj`；測試專案：`tests/AgainstRomeModifier.Tests/`。

### 0.2 建置與測試指令（每個 Phase 的驗收都用這兩條）

```powershell
dotnet build AgainstRomeModifier.csproj -c Release
dotnet test tests/AgainstRomeModifier.Tests -c Release
```

### 0.3 硬性規則（違反即算任務失敗）

1. **絕對不可讀寫遊戲安裝目錄**（`C:\Program Files (x86)\Against Rome`）。所有驗證只能透過
   測試專案的合成夾具（`tests/AgainstRomeModifier.Tests/SyntheticFixture.cs`）。
2. **這是行為保持（behavior-preserving）重構**：所有偵測啟發式、還原順序、錯誤訊息、
   log 訊息鍵、SHA-256 指紋常數必須逐字搬移，禁止「順手優化」邏輯本身。
   優化項目已另列在第 7 章，屬於獨立任務，不可與解耦重構混在同一個 commit。
3. 交易語意不變：先在記憶體 dry-run 生成全部補丁位元組，全部成功後才透過
   `SafeFileWriter` + `FileRollbackScope` 落地；任何一步失敗要能整批回復。
4. 每個 Phase 一個（或數個）獨立 commit，commit 訊息用中文，並在訊息中標注 Phase 編號。
5. 不可更動 `Backup.zip`、`data/`、`ThirdParty/`、`re_workspace/`、`遊戲原始檔案/`。

---

## 1. 現況架構盤點

### 1.1 檔案地圖（與本計畫相關者）

| 檔案 | 行數規模 | 職責 |
|------|---------|------|
| `src/Core/Services/PatchEngine.cs` | ~1,400 行 | **上帝類別**。套用/還原/偵測全部功能 + 語言包 + dgVoodoo + 待機回血 |
| `src/Core/Services/PatchOptions.cs` | 41 行 | 扁平選項袋：17 個 bool + GameSpeed(int) + EndlessAiModules 字典 + CustomUnitStats |
| `src/Core/Services/BackupManager.cs` | ~700 行 | Backup.zip 記憶體載入、備份鍵查詢、兵種原始屬性解析 |
| `src/Core/Services/SafeFileWriter.cs` | 小 | 原子寫入（temp + replace） |
| `src/Core/Services/ILogger.cs` | 6 行 | `void Log(string)` |
| `src/Core/FileRollbackScope.cs` | 小 | 修改前快照、失敗回復、Commit |
| `src/Core/GameLZSS.cs` | ~500 行 | PFIL 容器壓縮/解壓（`DecompressPfil` / `CompressPfil`） |
| `src/Core/Patches/ExePatchModel.cs` | ~600 行 | EXE 特徵碼定位 + WriteOp 規劃（已是純函式，品質良好） |
| `src/Core/Patches/ClScriptPatcher.cs` / `RessPatcher.cs` / `ObjdefPatcher.cs` / `TeamDatPatcher.cs` | 各數百行 | 純函式 `GetPatchedBytes(original, options)`（品質良好，保留不動） |
| `src/Core/EndlessAi/*`（IEndlessPatch、EndlessAiModule、EndlessAiOrchestrator、BciLiteralPatch、CustomPatches） | 合計 ~1,700 行 | 無盡模式 AI 模組系統（**本計畫要推廣的正確範本**） |
| `src/Core/Bci/BciPattern.cs` / `BciScriptFile.cs` | 小 | BCI 特徵碼搜尋/寫入、BCI 檔案快取單元 |
| `src/UI/ModifierForm.cs` + 6 個 partial（`.Data` / `.Patches` / `.Layout` / `.Localization` / `.Presets` / `.SaveManager`） | 合計 ~5,500 行 | 全部 UI；`chkXxx` 欄位宣告在 `ModifierForm.cs:67-100` |
| `src/Core/Localization.cs` | ~52KB | `Loc.Get(key)` 雙語字串表 |

### 1.2 現有的正確範本：`Core/EndlessAi`

`EndlessAiOrchestrator`（`src/Core/EndlessAi/EndlessAiOrchestrator.cs`）已實現本計畫要的全部性質：

- `IEndlessPatch`：`Id` + `TargetPattern` + `Detect(bytes)` + `Apply(ref bytes, enabled)` —— 套用/還原/偵測同一個類別。
- 還原就是 `Apply(enabled: false)`，沒有獨立的 Restore 程式碼。
- `_fileCache`（path → `BciScriptFile`）：同一批檔案只解壓一次，多個 patch 疊加修改，最後 `SaveAll` 一次寫入。
- 模組以 `Id`（"M1".."M6"、"R0"）查表，與排列順序無關。

本計畫 = 把這個模式推廣到其餘所有功能。

---

## 2. 耦合問題的精確定位

「新增一個功能」目前必須同步修改 **7 個位置**：

| # | 位置 | 現況 |
|---|------|------|
| 1 | `PatchOptions.cs` | 加 bool 屬性 |
| 2 | `PatchEngine.ApplyPatches`（`PatchEngine.cs:110-177`） | 手寫 A~H 段落，每個功能一段 |
| 3 | `PatchEngine.DetectCurrentPatchState`（`PatchEngine.cs:917-1180` 左右） | 另一份手寫偵測段落 A~D + 各功能 |
| 4 | `PatchEngine` 的四個還原端點：`RestoreOriginalFiles`(201) / `RestoreStatsOnly`(206) / `RestoreCompatOnly`(212) / `RestoreLanguageOnly`(239)，內部 `RestoreOriginalFilesInternal`(248) / `RestoreStatsOnlyInternal`(284) | 「哪個功能屬於哪個還原分類」的知識以硬編碼重複四次 |
| 5 | `ModifierForm.cs:67-100`（欄位）+ `ModifierForm.Layout.cs`（建立 toggle） | 每功能一個 `ModernToggle` |
| 6 | `ModifierForm.Patches.cs`：`BtnApply_Click:107-133`（手寫映射 UI→PatchOptions）、`RestoreAll:189-204` / `RestoreStatsOnly:305-314` / `RestoreCompatOnly:357-362`（三處手寫 UI 重設）、`ModifierForm.Data.cs` 的 `LoadCurrentData`（偵測結果→UI 回填） | 每功能被手寫映射 3–5 次 |
| 7 | `Localization.cs` | 文字鍵 |

同一功能的三份知識（套用 / 偵測 / 還原分類）分散在 2、3、4 三處，靠人工對齊；
歷史上已因此出過「補丁寫了但沒接進模組」（見 EndlessAiOrchestrator.cs:128-130 P17 的註解）這類 bug。

---

## 3. 目標架構

### 3.1 新增檔案總覽

```
src/Core/Features/
    IFeatureModule.cs        # 核心介面
    FeatureCategory.cs       # enum: Stats | Compat | Language
    FeatureValue.cs          # bool/int/object 三態值容器
    PatchProfile.cs          # Dictionary<string, FeatureValue>，取代 PatchOptions
    PatchContext.cs          # 共用檔案快取 + 交易提交
    DetectContext.cs         # 偵測用唯讀檔案快取（可與 PatchContext 合併實作）
    GameFile.cs              # 遊戲檔案描述子（路徑、是否 PFIL、備份鍵）
    FeatureRegistry.cs       # 唯一功能清單
    Exe/
        FocusLossFeature.cs
        VillageBuildRangeFeature.cs
        NoSpellAltarFeature.cs
        GameSpeedFeature.cs
    Ini/
        FastCiviProductionFeature.cs
        InfiniteMoraleFeature.cs
        FreeProductionFeature.cs
        FreeUpgradeFeature.cs
        NoSpellCostFeature.cs
    Objdef/
        BalanceFeature.cs
        HousingCapacityFeature.cs
        StorageCapacityFeature.cs
        HqHpFeature.cs
        FastBuildUpgradeRepairFeature.cs
        CustomUnitStatsFeature.cs
    Map/
        MaxPopulationFeature.cs
    Bci/
        FoodHealingFeature.cs
        EndlessAiFeature.cs      # 包裝 M1..M6（一個類別，六個實例）
    Install/
        LanguagePackFeature.cs
        DgVoodooFeature.cs
```

### 3.2 介面定義（照抄即可）

```csharp
namespace AgainstRomeModifier.Core.Features;

public enum FeatureCategory { Stats, Compat, Language }

/// <summary>bool 為主；GameSpeed 用 Int、CustomUnitStats 用 Object。</summary>
public readonly struct FeatureValue
{
    public bool AsBool { get; }
    public int AsInt { get; }
    public object? AsObject { get; }
    public static FeatureValue Of(bool b) => ...;
    public static FeatureValue Of(int i) => ...;
    public static FeatureValue Of(object? o) => ...;
    /// <summary>此功能的「停用/還原」值：bool→false、int→1（GameSpeed 原版）、object→null。</summary>
    public static FeatureValue Disabled(IFeatureModule m) => m.DisabledValue;
}

public interface IFeatureModule
{
    /// <summary>穩定識別字，如 "FreeProduction"、"GameSpeed"、"EndlessAi.M4"。UI 綁定表與 PatchProfile 都用它。</summary>
    string Id { get; }

    FeatureCategory Category { get; }

    /// <summary>還原此功能時應使用的值（多數為 false；GameSpeed 為 1）。</summary>
    FeatureValue DisabledValue { get; }

    /// <summary>
    /// 在 ctx 的共用檔案快取上生成補丁（dry-run，不寫磁碟）。
    /// value = DisabledValue 即為還原；沒有獨立的 Restore 方法。
    /// 失敗以擲例外表示（維持現有錯誤訊息字串）。
    /// </summary>
    void Plan(PatchContext ctx, FeatureValue value);

    /// <summary>從目前遊戲檔案讀取此功能的狀態。偵測失敗時回傳 DisabledValue 並記 log（維持現行為）。</summary>
    FeatureValue Detect(DetectContext ctx);
}
```

### 3.3 PatchContext：共用檔案快取

```csharp
public sealed class PatchContext
{
    public string GamePath { get; }
    public BackupManager Backup { get; }
    public ILogger Logger { get; }

    /// <summary>沿用既有 EndlessAiOrchestrator（含 BCI 檔案快取）。
    /// FoodHealingFeature 與 EndlessAiFeature 共用其 _fileCache，同一 BCI 檔只解壓一次。</summary>
    public EndlessAiOrchestrator BciOrchestrator { get; }

    /// <summary>讀檔（必要時 DecompressPfil），快取；同一 GameFile 第二次呼叫回快取副本。</summary>
    public byte[] GetFileBytes(GameFile file);
    /// <summary>登記補丁後位元組（覆蓋快取，標記 dirty）。</summary>
    public void SetFileBytes(GameFile file, byte[] bytes);

    /// <summary>語言包 / dgVoodoo 這類「檔案集合安裝/移除」不走位元組快取，
    /// 改為登記延遲執行的檔案操作，於 Commit 階段依登記順序執行。</summary>
    public void AddFileOperation(Action<FileRollbackScope> op);

    /// <summary>dry-run 全部完成後呼叫：寫入所有 dirty 檔（PFIL 檔先 CompressPfil）、
    /// BciOrchestrator.SaveAll、再依序執行 AddFileOperation 登記的操作。</summary>
    public void Commit(FileRollbackScope rollback);
}
```

`GameFile` 為靜態描述子集合，集中全部檔案知識：

```csharp
public sealed record GameFile(string RelativePath, bool IsPfilCompressed, string? BackupKey)
{
    public static readonly GameFile Exe      = new(@"Against_Rome.exe", false, null);
    public static readonly GameFile ClScript = new(@"SYSTEM\cl_script.ini", true, "SYSTEM/cl_script.ini");
    public static readonly GameFile ClEpara  = new(@"SYSTEM\cl_epara.ini", false, "SYSTEM/cl_epara.ini");
    public static readonly GameFile ClScint  = new(@"SYSTEM\CLAK\cl_scint.ini", false, "SYSTEM/CLAK/cl_scint.ini");
    public static readonly GameFile Ress     = new(@"SYSTEM\ress.ini", true, "SYSTEM/ress.ini");
    public static readonly GameFile Objdef   = new(@"SYSTEM\DATA_MP\DEFAULTS\objdef.dau", true, "SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
    // team.dat 是多檔（MAPS/**/team.dat），提供 EnumerateTeamDats(BackupManager) 輔助
}
```

**注意**：EXE 補丁在「原始位元組」上做（不解壓）；cl_script/ress/objdef 的既有 Patcher
接受「原始 PFIL 位元組」且自己處理解壓/回壓 —— 遷移時维持各 Patcher 的既有輸入格式，
`PatchContext` 對這三個檔快取「原始位元組」即可，不要改動 Patcher 內部。

### 3.4 新的 PatchEngine（收縮為編排器）

```csharp
public void ApplyPatches(string gamePath, PatchProfile profile, BackupManager backupManager, FileRollbackScope rollback)
{
    var ctx = new PatchContext(gamePath, backupManager, _logger);

    // 1) 先整體還原（等價於現行 RestoreOriginalFilesInternal 前置步驟）：
    foreach (var m in FeatureRegistry.All)
        m.Plan(ctx, m.DisabledValue);
    ctx.BciOrchestrator.ApplyMandatoryRepair(gamePath);   // R0 常駐修復，固定步驟

    // 2) 依 profile 疊加啟用的功能：
    foreach (var m in FeatureRegistry.All)
        m.Plan(ctx, profile.Get(m.Id));

    // 3) 落地：
    ctx.Commit(rollback);
}

public void RestoreOriginalFiles(...)  => PlanAllDisabled(全部) + MandatoryRepair + Commit;
public void RestoreStatsOnly(...)      => PlanAllDisabled(Category == Stats)   + Commit;
public void RestoreCompatOnly(...)     => PlanAllDisabled(Category == Compat)  + MandatoryRepair + Commit;
public void RestoreLanguageOnly(...)   => PlanAllDisabled(Category == Language)+ Commit;

public PatchProfile DetectCurrentPatchState(...)
{
    var ctx = new DetectContext(gamePath, backupManager, _logger);
    var profile = new PatchProfile();
    foreach (var m in FeatureRegistry.All)
        profile.Set(m.Id, m.Detect(ctx));
    return profile;
}

public void RunStartupSafeMigrations(...)   // 語意不變：只做 R0 修復 + 首領榮耀腳本安全遷移
```

四個 Restore 端點的簽名保持不變（UI 不用改呼叫方式），但內部重複全部消失。

### 3.5 功能分類對照（Category 的正確歸屬，依現行 Restore 行為反推）

| Category | 功能 | 依據 |
|----------|------|------|
| Stats | FastCiviProduction, InfiniteMorale, Balance, FreeProduction, FreeUpgrade, NoSpellCost, HousingCapacity20x, StorageCapacity10x, HqHp10x, FastBuildUpgradeRepair, FoodHealing10x, MaxPopulation, CustomUnitStats | `RestoreStatsOnlyInternal`(284-301) 還原 cl_script/cl_epara/cl_scint/ress/objdef/team.dat；`RestoreStatsOnly`(206-210) 另外還原 FoodHealing |
| Compat | FocusLoss, VillageBuildRange, NoSpellAltar, GameSpeed, DgVoodoo, EndlessAi.M1..M6 | `RestoreCompatOnly`(212-237) 還原 EXE 四項 + EndlessAi + dgVoodoo |
| Language | LanguagePack (ToEnglish) | `RestoreLanguageOnly`(239-242) |

### 3.6 UI 綁定表（ModifierForm）

版面**不**自動生成（手工卡片式 Layout 保留），只把「綁定」改為查表：

```csharp
// ModifierForm.cs 新增欄位；於 Layout 建立完 toggle 後（InitializeLayout 尾端）填表
private Dictionary<string, ModernToggle> featureToggles = null!;

private void BuildFeatureToggleMap() {
    featureToggles = new(StringComparer.OrdinalIgnoreCase) {
        ["FocusLoss"] = chkFocusLoss,           ["FastCiviProduction"] = chkFastCiviProduction,
        ["InfiniteMorale"] = chkInfiniteMorale, ["FreeProduction"] = chkFreeProd,
        ["FreeUpgrade"] = chkFreeUpgrade,       ["NoSpellCost"] = chkNoSpellCost,
        ["MaxPopulation"] = chkMaxPopulation,   ["Balance"] = chkBalance,
        ["HousingCapacity20x"] = chkHousingCapacity20x, ["StorageCapacity10x"] = chkStorageCapacity10x,
        ["HqHp10x"] = chkHqHp10x,               ["FastBuildUpgradeRepair"] = chkFastBuildUpgradeRepair,
        ["FoodHealing10x"] = chkFoodHealing10x, ["VillageBuildRange"] = chkVillageBuildRange,
        ["NoSpellAltar"] = chkNoSpellAltar,     ["DgVoodoo"] = chkDgVoodoo,
        ["ToEnglish"] = chkToEng,
        ["EndlessAi.M1"] = chkAiM1, ["EndlessAi.M2"] = chkAiM2, ["EndlessAi.M3"] = chkAiM3,
        ["EndlessAi.M4"] = chkAiM4, ["EndlessAi.M5"] = chkAiM5, ["EndlessAi.M6"] = chkAiM6,
    };
}

// BtnApply_Click 收集（取代 ModifierForm.Patches.cs:107-133 的手寫映射）：
var profile = new PatchProfile();
foreach (var (id, toggle) in featureToggles) profile.Set(id, FeatureValue.Of(toggle.Checked));
profile.Set("GameSpeed", FeatureValue.Of(GetSelectedGameSpeedMultiplier()));
profile.Set("CustomUnitStats", FeatureValue.Of(this.customUnitStats));

// Restore 後 UI 重設（取代三處手寫）：
private void ResetTogglesForCategory(FeatureCategory cat) {
    foreach (var m in FeatureRegistry.ByCategory(cat))
        if (featureToggles.TryGetValue(m.Id, out var t)) t.Checked = false;
    if (cat == FeatureCategory.Compat) {
        cmbGameSpeed.SelectedIndex = 0;
        chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(GetGamePath()); // 特例維持現行為
    }
}

// LoadCurrentData 的 UI 回填：同一迴圈以 profile.Get(id) 反向設定。
```

GameSpeed（下拉選單）、CustomUnitStats（表格）不在 toggle 表內，維持專屬控制項，但值一律走 `profile.Set/Get`。

---

## 4. 功能抽取對照表（Phase 1 的逐項工單）

「來源」欄的行號以目前 `src/Core/Services/PatchEngine.cs` 為準（重構開始前先重新確認一次，行號可能漂移；用方法名定位為主）。

| 新檔案 | Id | 搬移來源（Apply/Plan 部分） | 搬移來源（Detect 部分） | 一併搬移的常數/輔助 |
|--------|-----|---------------------------|------------------------|--------------------|
| `Exe/FocusLossFeature.cs` | `FocusLoss` | `ApplyExePatch` 中 focus 段（314-329） | `DetectCurrentPatchState` A 段 928-932 | — |
| `Exe/VillageBuildRangeFeature.cs` | `VillageBuildRange` | `RestoreLegacyVillageBuildRangePatch`(343-359) + `ApplyVillageSetterRangePatch`(361-389) + 330-336 的 legacy 檢查 | 934-942 | 注意：legacy 還原永遠先執行（無條件），再套 setter 補丁 —— 順序不可反 |
| `Exe/NoSpellAltarFeature.cs` | `NoSpellAltar` | `ApplySpellAltarPatch`(391-406) | 944-948 | — |
| `Exe/GameSpeedFeature.cs` | `GameSpeed` | `ApplyGameSpeedPatch`(408-423) | 950-951 | `DisabledValue = Of(1)` |
| `Ini/FastCiviProductionFeature.cs` | `FastCiviProduction` | `GetPatchedClScriptBytes`(434-456) 的 civi 參數 | 956-969（`RegexCiviLoad`） | Regex 常數 21 |
| `Ini/InfiniteMoraleFeature.cs` | `InfiniteMorale` | 同上 morale 參數 | 971-979（四條 Morale Regex） | Regex 常數 22-25 |
| `Objdef/BalanceFeature.cs` | `Balance` | `GetPatchedClScriptBytes` 的 spellRadius 倍率（438-444）+ `GetPatchedObjdefBytes` 的 balance 旗標 | DetectCurrentPatchState D 段的 unitRows 比對（1044 起） | 祭司半徑 target/500 換算註解一併搬 |
| `Ini/FreeProductionFeature.cs` | `FreeProduction` | `GetPatchedRessBytes`(458-461) 參數 1 | 984-1000 | — |
| `Ini/FreeUpgradeFeature.cs` | `FreeUpgrade` | 同上參數 2 | 1002-1008 | — |
| `Ini/NoSpellCostFeature.cs` | `NoSpellCost` | 同上參數 3 | 1010-1016 | — |
| `Objdef/HousingCapacityFeature.cs` | `HousingCapacity20x` | `GetPatchedObjdefBytes`(463-472) 對應旗標 | 1037 + `HasHousingCapacityMultiplier`(1197) | `HousingCapacityMultiplier = 20`(27) |
| `Objdef/StorageCapacityFeature.cs` | `StorageCapacity10x` | 同上 | 1038 + `HasStorageCapacityMultiplier`(1225) | `StorageCapacityMultiplier = 10`(28) |
| `Objdef/HqHpFeature.cs` | `HqHp10x` | 同上 | 1039 + `HasHqHpMultiplier`(1254) | — |
| `Objdef/FastBuildUpgradeRepairFeature.cs` | `FastBuildUpgradeRepair` | 同上 | 1040 + `HasFastBuildUpgradeRepair`(1283) | — |
| `Objdef/CustomUnitStatsFeature.cs` | `CustomUnitStats` | `GetPatchedObjdefBytes` 的 unitStats 組裝（466-470） | 無檔案偵測（無法回推），Detect 回傳 null | 這是 `RunStartupSafeMigrations` 不整套重套的原因，註解一併搬 |
| `Map/MaxPopulationFeature.cs` | `MaxPopulation` | `GetPatchedTeamDatBytes`(474-483) | DetectCurrentPatchState 對應段 | team.dat 多檔枚舉 |
| `Bci/FoodHealingFeature.cs` | `FoodHealing10x` | `ApplyFoodHealingAmountPatch`(485-553) + `BuildFoodHealAmountSignature`(555-558) | `TryReadFoodHealingAmountState`(1339-) | `FoodHealAmountSites`(35-48)、兩個 SHA 常數(32-33)、`FoodHealAmountOriginal/Ultimate`(29-30)。**含首領榮耀腳本安全遷移邏輯（502-529），逐字搬** |
| `Bci/EndlessAiFeature.cs` | `EndlessAi.M1`..`M6` | 包裝 `ctx.BciOrchestrator.ApplyModule(gamePath, module, enabled)` | `DetectModule(...) == PatchState.Ultimate` | 建構子收 `EndlessAiModule`；六個實例在 Registry 建立 |
| `Install/LanguagePackFeature.cs` | `ToEnglish` | `ApplyLanguagePatch`(562-601) + `EnsureLanguageBackup`(623) + `RestoreLanguageBackup`(692) + `GetLanguageBackupDirectory`(602) + `GetSafeLanguagePath`(607) + `FilesAreEqual`(739) | `TryGetLanguageOverlayState`(721) | manifest 常數(16-17)、`LanguageBackupManifest`(71-75)。透過 `ctx.AddFileOperation` 執行 |
| `Install/DgVoodooFeature.cs` | `DgVoodoo` | `ApplyDgVoodooPatch`(762)、`RemoveDgVoodoo`(810)、`LoadEmbeddedDgVoodooFiles`(892) 等 | `IsDgVoodooInstalled`(860)（**保留一個 public 轉呼叫**，UI 有直接用） | DgVoodoo 常數群(50-69)、`ComputeSha256`(907) |

搬移後 `PatchEngine` 保留：`ApplyPatches` / 四個 Restore / `DetectCurrentPatchState` / `RunStartupSafeMigrations` /
`SafeWrite*` 三個包裝 / `IsDgVoodooInstalled`（轉呼叫 DgVoodooFeature）。其餘全部移出。

---

## 5. 執行步驟（Phase 0–4）

### Phase 0 — 特徵化測試護欄

**目的**：建立「重構前後行為相同」的機器判準。

1. 讀 `tests/AgainstRomeModifier.Tests/SyntheticFixture.cs` 了解合成遊戲目錄怎麼建。
2. 新增 `tests/AgainstRomeModifier.Tests/CharacterizationTests.cs`：
   - **T1 全開往返**：合成目錄 → 全部布林選項 true + GameSpeed=2 → `ApplyPatches` → `DetectCurrentPatchState` → 斷言每一項都偵測回啟用狀態（CustomUnitStats 除外，它偵測不回來）。
   - **T2 還原等於備份**：T1 之後 `RestoreOriginalFiles` → 逐檔與 `BackupManager.BackupFiles` byte-equal（語言包與 dgVoodoo 檔案不存在）。
   - **T3 分類還原**：全開後 `RestoreStatsOnly` → Stats 檔案等於備份、EXE 補丁仍在；`RestoreCompatOnly` → 反之。
   - **T4 重複套用冪等**：`ApplyPatches` 連跑兩次，第二次不擲例外且產物 byte-equal。
3. 驗收：`dotnet test` 全綠。Commit：`Phase 0: 特徵化測試護欄`。

### Phase 1 — 機械抽取（行為零改變）

**目的**：把 §4 對照表的每一列搬成獨立檔案；先用 `internal static` 類別 + 原簽名，不引入新介面。

1. 建 `src/Core/Features/` 目錄結構（§3.1）。
2. 依 §4 表逐列搬移：方法本體、私有常數、Regex、輔助函式**原樣剪貼**，
   `PatchEngine` 對應處改為轉呼叫（例如 `FoodHealingFeature.Apply(gamePath, enabled, backupManager, rollback, _logger)`）。
3. `ParseObjdefRows`(1186) 被多個 objdef 偵測共用 → 搬到 `Objdef/ObjdefDetectHelper.cs`。
4. 每搬 2–3 個功能就 build + test 一次；全部搬完後 `PatchEngine.cs` 應 < 400 行。
5. 驗收：Phase 0 四條測試 + 既有全部測試綠。Commit：`Phase 1: PatchEngine 功能機械拆檔`。

### Phase 2 — 介面統一 + 刪除還原重複

1. 加入 §3.2 的 `IFeatureModule` / `FeatureValue` / `PatchProfile`、§3.3 的 `PatchContext` / `GameFile`、`FeatureRegistry`。
2. Phase 1 的 static 類逐一改為實作 `IFeatureModule`：
   - EXE 四功能共用 `ctx.GetFileBytes(GameFile.Exe)`，各自 `Plan` 疊加寫入同一份快取位元組。
     **順序約束**：VillageBuildRange 的 legacy 還原必須先於 setter 套用（見 §4）；
     Registry 排列順序即執行順序，EXE 功能維持 Focus → VillageRange → SpellAltar → GameSpeed（照抄現行 `ApplyExePatch` 順序）。
   - cl_script 三功能（FastCivi/InfiniteMorale/Balance 的半徑部分）：`ClScriptPatcher.GetPatchedBytes` 一次接收全部參數，
     設計為一個內部 `ClScriptComposer`：三個 Feature 把各自參數寫進 ctx 的 per-file 選項槽，
     由 `PatchContext.Commit` 前的 finalize 步驟呼叫一次 Patcher（objdef 五功能同理，共用 `ObjdefOptions` 槽）。
     這保持「同檔案單次改寫」的現行語意，避免多次解壓/回壓造成差異。
   - `RestoreOriginalFilesInternal` / `RestoreStatsOnlyInternal` 刪除，四個 Restore 端點改為 §3.4 的分類迴圈。
   - `cl_epara.ini` 與 `cl_scint.ini` 的「一律還原為備份」（ApplyPatches 137-141）沒有對應功能開關 →
     做成 Registry 外的固定步驟（放在編排器，或一個 `AlwaysRestoreFeature` 皆可，擇一並註明）。
   - `PatchOptions` 暫留為 `PatchProfile` 的相容包裝（屬性 get/set 轉呼叫 profile），UI 尚未改。
3. `DetectCurrentPatchState` 改為 Registry 迴圈；回傳型別可暫時仍是 `PatchOptions`（包裝）。
4. 驗收：全部測試綠；特別確認 T3 分類還原與 T4 冪等。Commit：`Phase 2: IFeatureModule 統一 Apply/Detect/Restore`。

### Phase 3 — UI 綁定表

1. 依 §3.6 在 `ModifierForm` 建 `featureToggles` 表 + `ResetTogglesForCategory`。
2. 改寫 `BtnApply_Click` 收集、三處 Restore UI 重設、`LoadCurrentData` 回填為迴圈。
3. 刪除 `PatchOptions` 相容包裝，全面改用 `PatchProfile`。
4. 驗收：build + test 綠；人工冒煙（僅用合成目錄或審視程式碼路徑，不碰真實遊戲目錄）。
   Commit：`Phase 3: UI 功能綁定表`。

### Phase 4 — 收編與收尾

1. `FoodHealingFeature` 改用 `ctx.BciOrchestrator` 的 `BciScriptFile` 快取讀寫 12 個腳本
   （目前自己 `File.ReadAllBytes` + 立即寫檔，是唯一繞過 dry-run 的功能；改為 Plan 進快取、Commit 統一落地）。
   注意首領榮耀遷移分支中「以備份原檔整檔覆蓋」的路徑也要走快取。
2. `RunStartupSafeMigrations` 改用同一 ctx（語意不變：只 R0 + 食物回血現值重寫）。
3. 全域搜尋確認：`PatchEngine.cs` 中不再有任何功能專屬常數/Regex；`ModifierForm.Patches.cs` 不再有逐 checkbox 手寫映射。
4. 更新 `TechDoc.md` / `TechDoc_EN.md` 的架構章節與 `README`（若有描述架構）。
5. 驗收：全部測試綠。Commit：`Phase 4: BCI 快取統一與文件更新`。

### 完成後的「新增功能」流程（寫進 TechDoc）

1. 新增 `src/Core/Features/<分類>/XxxFeature.cs`（Plan + Detect + DisabledValue 同檔）。
2. `FeatureRegistry.All` 加一行。
3. `ModifierForm.Layout` 加 toggle、`BuildFeatureToggleMap` 加一行。
4. `Localization.cs` 加鍵。
5. 加對應單元測試。

移除功能 = 反向刪除同四處，編排器/還原/偵測/UI 迴圈零修改。

---

## 6. 不變式與已知陷阱（執行中隨時對照）

1. **`RunStartupSafeMigrations` 絕不可改成整套 ApplyPatches**：自訂兵種屬性偵測不回來，整套重套會在啟動瞬間無聲覆蓋玩家設定（PatchEngine.cs:179-186 註解）。
2. **EXE 補丁順序**：legacy 村莊半徑還原 → 新 setter 補丁；且 `GetExePatchState == Unknown` 時必須擲例外中止（不能靜默跳過）。
3. **首領榮耀腳本安全遷移**（FoodHealing 內，PatchEngine.cs:502-529）：SHA-256 指紋比對、以備份整檔重建、`retiredLeaderScriptMigrated` 時即使值相同也要落盤 —— 三個分支都要保留。
4. **偵測失敗 = 記 log 後當作未啟用**，不能擲例外中斷整個 DetectCurrentPatchState（現行 try/catch per-file 語意）。
5. **EndlessAi `ApplyModule` 的防呆**：`Unknown && enabled` 擲例外、`!enabled && (Original||Unknown)` 直接跳過 —— 包裝進 Feature 時不可繞過。
6. `cl_epara.ini` / `cl_scint.ini` 每次套用一律還原為備份（無開關），別在遷移中弄丟。
7. `DgVoodoo` 的 Detect 依 manifest + 檔案 SHA 判斷，UI 另外直接呼叫 `IsDgVoodooInstalled` —— 保留該 public 入口。
8. 語言包操作對象是整個目錄樹覆蓋，含路徑安全檢查 `GetSafeLanguagePath`（防 zip-slip 式路徑逃逸）——逐字搬。
9. 測試專案有多個直接測 Patcher 的測試（`ClScriptPatchTests`、`PatcherRoundTripTests`、`SettlePlacePatchTests` 等）——這些 Patcher 介面不動，測試不應需要修改；若某測試直接呼叫 `PatchEngine` 私有方法（`GetPatchedClScriptBytes` 是 `internal`），改為呼叫新 Feature 的對應方法。

---

## 7. 優化方向（解耦完成後的獨立任務清單，依優先序）

> 每一項都是獨立 PR/commit，不與第 5 章的重構混做。

### 7.1 消除 UI/Core 重複邏輯（正確性風險最高，優先）

`ModifierForm.Data.cs` 與 `BackupManager.cs` 存在**整組複製的函式**，兩份會漂移：

- `MergeUnitStatsLayers`（ModifierForm.Data.cs:255 vs BackupManager.cs:566）
- `GetBaseStatsForUnit`（ModifierForm.Data.cs:275 vs BackupManager.cs:553）
- `GetMeleeAndRangedDmg`（ModifierForm.Data.cs:927 vs BackupManager.cs:587）
- `GetMeleeAndRangedRelt`（ModifierForm.Data.cs:954 vs BackupManager.cs:618）
- `GetUnitMaxRange`（ModifierForm.Data.cs:986 vs BackupManager.cs:655）
- `ParseCsvLine`（ModifierForm.Data.cs:29 vs `PatchText.ParseCsvLine`）

**做法**：以 Core 版為單一正本，UI 全改為呼叫 Core；先 diff 兩份實作確認是否已經漂移（若有差異，逐一判定哪邊是對的再統一）。

### 7.2 統一日誌管線

`ModifierForm.Patches.cs:21-46` 自己寫 `modifier_log.txt`（含輪替），Core 走 `ILogger`。
**做法**：抽出 `FileLogger : ILogger`（含輪替邏輯）放 `Core/Services`，UI 的 `Log()` 變成轉呼叫；UI 執行緒安全由 FileLogger 的 lock 保證。

### 7.3 `ModifierForm.Data.cs`（50KB）再拆分

`LoadCurrentData`(724) 同時做「偵測、表格填充、UI 同步」三件事。
**做法**：拆出 `CurrentStatsViewBuilder`（純函式：偵測結果 + 備份 → 表格列資料），Form 只負責把列資料塞進 DataGridView。此後屬性顯示邏輯可測試。

### 7.4 `EndlessAiOrchestrator.ResolvePaths` 的 if-chain 資料化

`ResolvePaths`(203-251) 與 `GetExpectedFileCount`(253-261) 是 pattern 字串的硬編碼 if-chain，新增目標檔要改兩處。
**做法**：改為 `TargetPattern` 描述子物件（glob 根目錄 + 檔名模式 + 期望數量），一處註冊。

### 7.5 效能：偵測與套用共用快取

- `DetectCurrentPatchState` 與 `ApplyPatches` 各自完整讀檔解壓一輪；UI 啟動時 `LoadCurrentData` 也會再來一輪。
  解耦後 `DetectContext` / `PatchContext` 可共用同一份快取實例（同一次使用者操作內）。
- `GetPatchedObjdefBytes` 對每個兵種呼叫 `GetBaseStatsForUnit`，其內部重複解析備份 objdef 文本 —— `BackupManager` 可把 `GetBackupUnitRows()` 結果做惰性快取（確認目前是否每次重解析，若是收益很大：objdef 解壓 + CSV 解析是啟動路徑最重的一段）。
- `ApplyPatches` 在 `Task.Run` 內單執行緒逐檔處理；各 GameFile 之間無依賴，可平行 Plan（低優先，先量測再說）。

### 7.6 `Localization.cs`（52KB 單檔）外部化

**做法**：改為內嵌資源 `.json`（zh-TW / en 各一檔）+ 啟動載入；鍵不變。附帶寫一個測試：兩個語言檔鍵集合必須相等，防漏譯。

### 7.7 錯誤處理一致化

功能失敗訊息目前混雜「擲例外（中止整批）」與「記 log 繼續」。解耦後可在 `IFeatureModule` 層明確分級：
`Plan` 失敗一律擲例外（交易會回復）；`Detect` 失敗一律降級為停用 + log。把這個約定寫進介面 XML doc。

### 7.8 UI 自動生成（長期、可選）

解耦完成後，Registry 已含每個功能的 Id/Category，可再加 `DisplayNameKey` / `TooltipKey` / `CardGroup` 中繼資料，
讓 `ModifierForm.Layout` 依 Registry 自動生成 toggle 卡片 —— 屆時「新增功能」縮到 2 處（Feature 檔 + 本地化鍵）。
風險：現版面是手工微調的卡片式布局，自動生成需先建立版面快照比對，建議等前述項目全部穩定後再評估。

### 7.9 CI 強化

`.github/` 已有 CodeQL。可加：PR 上跑 `dotnet test`（Windows runner）、
以及一條「PatchEngine.cs 行數上限 400」的守門檢查，防止上帝類別回潮。

---

## 8. 驗收總表

| Phase | 驗收條件 |
|-------|----------|
| 0 | 新增 T1–T4 特徵化測試，全綠 |
| 1 | PatchEngine < 400 行；全部功能邏輯在 `Core/Features/`；測試綠且 T1–T4 無需修改 |
| 2 | `Restore*Internal` 刪除；Registry 迴圈驅動 Apply/Detect/Restore；測試綠 |
| 3 | `ModifierForm.Patches.cs` 無逐 checkbox 手寫映射；`PatchOptions` 刪除；測試綠 |
| 4 | FoodHealing 走 BCI 快取（無直接 File.Write）；TechDoc 更新；測試綠 |
