# Against Rome Modifier — 全專案重構審查與執行計畫

> 本文件由靜態審查產生（2026-07-20）。
> 目的：交給後續 AI / 開發者執行「優化、解耦、去除死程式碼」的重構工作。
> 所有項目均為**行為不變（behavior-preserving）**的重構；任何會改變使用者可見行為的項目已明確標註 ⚠️。

## ✅ 已由本次作業完成的項目（278 測試全綠、build 0 警告）

以下已實際套用到程式碼，其餘章節仍待執行：

- **§2.1 死碼刪除**：`ExperimentalFeatureIds` 欄位、`PatchEngine.SafeCopyFile/SafeDeleteFile`、孤兒 XML 註解、4 處註解掉的 `CardPanel_Paint` 掛接。
- **§2.2 死計算**：`LoadDefaultStatsData` 中 35 行「算完即被 `bases[]` 覆寫」的 speed/sight/range/spellRadius 計算與恆為 1.0 的 `defHpMult`。
- **§1.2 csproj**：移除 `src.Core` 不必要的 `UseWindowsForms`。
- **§8.1 ParseCol**：新增 `PatchText.ParseDouble`，收斂 `ObjdefFeatureDetector`（18 站）與 `ModifierForm.Data`（多站）的 `double.TryParse` 樣板（僅轉換忽略成功旗標者；用作 `if` 條件的保留）。
- **§8.2 拆門面**：刪除 `BackupManager` 六個 static 轉呼叫，呼叫端（含 2 個測試）改用 `UnitStatParser` / `CleanEparaBaseline`。
- **§1.3 語言覆寫 bug** ⚠️：移除 `ModifierForm` 建構式每次以系統語系覆寫並存檔的區塊。
- **§9.2 初始路徑去重**：新增 `GameDirectoryLocator.ResolveInitialGamePath()`，`ModifierForm` / `SaveManager` 共用（兩者原 fallback 順序完全相同）；連帶刪除兩處變成死碼的 `DetectGamePathFromRegistry` 私有包裝。

淨變動約 −136 行。**尚未執行**的高價值項目：§3（UI 樣板去重 / `DarkFormBase`）、§3.4（功能開關描述表）、§3.5（還原流程樣板）、§1.4（UI 解析下沉 + `CurrentStatsReader`）、§9.3（MapEditor 在地化統一）、§7.1（Directory.Build.props + 分析器）。

---

## 0. 執行前必讀（約束條件）

1. 先讀 `AGENTS.md` 與 `AI_HANDOFF.md`，遵守其中規則。
2. **絕不**讀寫遊戲安裝目錄（`C:\Program Files (x86)\Against Rome`）；所有驗證走測試（`tests/` 使用 Backup.zip fixture）。
3. **絕不**自行建立 Git 分支或 commit/push（需使用者明確授權）。
4. 每一步重構後執行：`dotnet build AgainstRomeModifier.slnx` 與 `dotnet test`（45 個測試檔是主要安全網）。
5. 「行為保留重構」與「功能修改」必須分開進行，不可混在同一批變更。
6. 大檔案（`CustomPatches.cs`、`ExePatchModel.cs`）中的**位元組簽章、offset、opcode 常數是逆向工程成果，一個字都不能動**；重構只能搬移/包裝，不能「順手整理」數值。

### 建議執行順序

| 優先級 | 項目 | 風險 | 章節 |
|---|---|---|---|
| P0 | 刪除已確認死碼 | 極低 | §2 |
| P0 | 合併重複的 UI 基礎設施（視窗外框/按鈕樣式/TGA 解碼） | 低 | §3.1–3.3 |
| P1 | ModifierForm 功能開關中繼資料表統一（5 份手寫清單 → 1 份） | 中 | §3.4 |
| P1 | 還原/套用流程樣板去重 | 低 | §3.5 |
| P1 | 命名空間統一 | 低（純機械） | §1.1 |
| P1 | `Loc` 職責拆分 + 語言初始化去重 | 中 | §1.3 |
| P2 | UI 與檔案解析解耦（LoadCurrentData 下沉到 Core） | 中 | §1.4 |
| P2 | Orchestrator 的 pattern 字串魔法值收斂 | 中 | §1.5 |
| P2 | 效能微調 | 低 | §4 |
| P2 | 健壯性/在地化補洞 | 低 | §5 |

---

## 1. 解耦（架構層）

整體評價：專案分層其實已經不錯——`src.Shared`（無 UI 依賴）→ `src.Core`（patch/detect 引擎，`FeatureRegistry` + `IPatchFileContributor` 註冊表模式）→ 三個 UI 專案。主要問題集中在 **UI 層自己長出的平行真相來源** 與 **少數放錯位置的職責**。

### 1.1 命名空間不一致（機械性修正）

`src.Core` 內有兩套命名空間並存：

- 根命名空間 `AgainstRomeModifier`（舊）：`Core/EndlessAi/*`（`EndlessAiOrchestrator`、`CustomPatches`、`BciLiteralPatch`、`EndlessAiModule`、`PatchState`、`IEndlessPatch`）、`Core/Bci/*`（`BciScriptFile`、`BciPattern`）、`Localization.cs`（`Loc`）、`TroopConfig.cs`
- `AgainstRomeModifier.Core.*`（新）：其餘全部（Features/Patches/Services）

**動作**：把舊命名空間檔案改為對應的 `AgainstRomeModifier.Core.EndlessAi` / `Core.Bci` 等，並修正所有 using。純機械性，IDE 重新命名即可；測試專案也要跟著改。
（副作用：`src.Modifier` 內大量 `AgainstRomeModifier.Core.Services.XXX` 全限定名可簡化為 using。）

### 1.2 `src.Core` 不必要的 WinForms 依賴

[AgainstRome.Core.csproj](src.Core/AgainstRome.Core.csproj) 宣告 `<UseWindowsForms>true</UseWindowsForms>`，但整個 `src.Core` **沒有任何** `System.Windows.Forms` / `System.Drawing` 引用（已 grep 確認）。

**動作**：移除 `UseWindowsForms`；可進一步比照 `src.Shared` 改成 `net8.0`（目前 `net8.0-windows`）。若因 Registry API（`GameDirectoryLocator`）需要，保留 `net8.0-windows` 即可，但 WinForms 一定可移除。這能讓 Core 理論上可被 CLI/測試更輕量引用。

### 1.3 `Loc`（Localization.cs）職責過載 + 語言初始化三處重複

[Localization.cs](src.Core/Core/Localization.cs) 目前同時負責：

1. 翻譯字典（En/Zh、單位名、tier/style 文案）— 合理
2. **使用者設定持久化**（`%APPDATA%\AgainstRomeModifier\settings.json` 的讀寫）
3. **實驗性功能推廣狀態**（`PromotedFeatures` / `PromoteFeature` / `DemoteFeature`）— 這是純 UI 偏好，放在 Core 的在地化類別裡

**動作**：
- 拆出 `UserSettings`（或 `AppSettingsService`）類別：負責 settings.json 讀寫、`PromotedFeatures`、語言偏好。`Loc` 只留翻譯查表 + `CurrentLanguage`。
- ⚠️ **疑似 bug（先確認再改）**：[ModifierForm.cs:229-234](src.Modifier/UI/ModifierForm.cs:229) 建構式無條件用「系統語言」覆寫 `Loc.CurrentLanguage`，而 setter 會立即 `SaveSettings()` —— 也就是**每次開啟修改器都會把使用者已儲存的語言偏好蓋掉**。`Loc` 靜態建構式已經正確載入偏好（settings.json 優先、否則跟隨系統），LauncherForm 也依賴 `ReloadLanguage()` 讀回。ModifierForm 這段應整段刪除。
- `OverrideLanguageForTesting` 是全域可變靜態狀態的症狀；拆分後可讓測試注入設定路徑而不需後門。

### 1.4 ModifierForm 直接解析遊戲檔案（UI ↔ 資料層滲漏）

[ModifierForm.Data.cs](src.Modifier/UI/ModifierForm.Data.cs) 的 `LoadCurrentData()`（~190 行）在 UI 層直接做了：

- `File.ReadAllBytes` + `GameLZSS.DecompressPfil` 解 `objdef.dau`
- CP1251 解碼、CSV 切欄、`ObjdefIndex` 欄位語意
- 用 `RegexSpellLoad` 直接 regex 解析 `cl_script.ini` 拿法術半徑
- `LoadIcons()` 直接開 `gui.dat` zip + 解析內嵌 `icon.ini`

而 Core 已有 `UnitStatParser` / `UnitBaselineCatalog` / `UnitStatsProjectionService` 做幾乎同樣的事。

**動作**：在 `Core.Services` 新增（或擴充現有服務）：
- `CurrentStatsReader`（暫名）：輸入 gamePath + BackupManager，輸出 `IReadOnlyList<UnitStatRow>`（orig/current 成對數值 + faction/utype/tier），把上述解析全部下沉。
- `UnitIconLoader`：gui.dat + icon.ini → `Dictionary<string, byte[]>`（回傳 TGA bytes，UI 端再轉 Bitmap，避免 Core 依賴 System.Drawing）。
- UI 端 `LoadCurrentData` 只剩「呼叫服務 → 填 grid → 同步 toggle」。
- 順帶消滅 `ParseCsvLine`（[ModifierForm.Data.cs:31](src.Modifier/UI/ModifierForm.Data.cs:31)，只是 `line.Split(',')` 的包裝，Core 的 parser 已有同邏輯）。

### 1.5 `EndlessAiOrchestrator` 的 pattern 魔法字串

[EndlessAiOrchestrator.cs](src.Core/Core/EndlessAi/EndlessAiOrchestrator.cs) 中 `ResolvePaths()` 與兩個 `GetExpectedFileCount()` 多載用**字串比對** `"MAPS/ENDL_*/SCRIPT/ak_level.bci"` 等 5 種 pattern 分派邏輯，且預期檔案數（5 / 1 / 1 / 42 / 2）與解析邏輯分散兩處。每新增一種目標檔就要改 3 個 switch。

**動作**：定義 `BciTarget`（或 enum + 描述表）：

```
sealed record BciTarget(string Pattern, Func<string, List<string>> Resolve, int? FixedExpectedCount);
```

由各 `IEndlessPatch.TargetPattern`（改為回傳 `BciTarget`）攜帶，orchestrator 不再認識任何具體路徑字串。`DetectModule`/`ApplyModule`/`ApplyMandatoryRepair` 邏輯不變。

### 1.6 三種狀態聚合邏輯重複

`DetectModule()`（[EndlessAiOrchestrator.cs:310](src.Core/Core/EndlessAi/EndlessAiOrchestrator.cs:310)）與 `DetectGlobalState()`（:365）各自維護 `allOriginal/allUltimate` 三旗標合併演算法，完全相同。

**動作**：抽出 `PatchStateAggregator`（`Add(PatchState)` + `Result` 屬性），兩處共用。

### 1.7 PatchEngine 門面的殘留相容端點

[PatchEngine.cs](src.Core/Core/Services/PatchEngine.cs)：
- `DetectCurrentPatchProfile` 只是 `DetectCurrentPatchState` 的別名（:31）— 二選一保留（建議留 `DetectCurrentPatchState`，UI 改呼叫它）。
- `SafeCopyFile` / `SafeDeleteFile`（:38-:55）**無任何呼叫者**（Install features 各自有 private 副本）→ 刪除；`SafeWriteAllBytes` 亦僅是 `SafeFileWriter` 轉呼叫，呼叫端可直接用 `SafeFileWriter`。

### 1.8 `SafeDeleteFile`/`SafeCopyFile` 的三份 private 副本

`ArgmTraceFeature`（:269）、`DgVoodooFeature`（:25）、`LanguagePackFeature`（:18/:23）各自實作相同的 safe delete/copy。

**動作**：統一搬進 `src.Shared` 的 `SafeFileWriter`（已是共用寫檔者），三個 feature 直接呼叫。

---

## 2. 死程式碼（已交叉 grep 確認）

### 2.1 可直接刪除

| 位置 | 內容 | 證據 |
|---|---|---|
| [ModifierForm.cs:128-137](src.Modifier/UI/ModifierForm.cs:128) | `ExperimentalFeatureIds` 靜態欄位 | 全 repo 僅此一處出現，無任何讀取（實際來源是 `Loc.IsPromoted` + `experimentalToggleOriginalParents`） |
| [PatchEngine.cs:38-55](src.Core/Core/Services/PatchEngine.cs:38) | `SafeCopyFile`、`SafeDeleteFile` | 無呼叫者 |
| [ModifierForm.Data.cs:233-235](src.Modifier/UI/ModifierForm.Data.cs:233) | 孤兒 XML 註解「將裝備分類代碼轉換為易懂的中文文字說明」 | 其對應方法已被刪除，註解殘留 |
| ModifierForm.cs 內 4 處 `// pnlXxxCard.Paint += CardPanel_Paint;` | 註解掉的舊繪製掛接（:701、:814、:897、:995） | 已由 `ConfigureSettingsCard` 底色取代 |

### 2.2 死計算（結果被覆寫）

[ModifierForm.Data.cs](src.Modifier/UI/ModifierForm.Data.cs) `LoadDefaultStatsData()`：

- `defaultSpeed`/`defaultSight`/`defaultRange`/`defaultSpellRadius` 在 :526-:559 花約 35 行依 utype 計算，隨後在 :623-:626 被 `bases[4..8]`（`UnitStatsProjectionService.Project` 的結果）**整批覆寫**，計算完全丟棄。
- `defHpMult`（:571）恆為 `1.0`，:638 的 `bases[0] * defHpMult` 等於沒乘。

**動作**：刪除 :526-:559 的計算塊與 `defHpMult`；只留 orig 值解析（供 scale 比例用的 `origPrimaryDam`/`origPrimaryRelt` 要保留）。

### 2.3 幾乎死亡（僅測試/腳本引用，需決策）

| 位置 | 內容 | 現況 |
|---|---|---|
| `EndlessAiOrchestrator.DetectGlobalState`（:365） | 全域狀態聚合 | 產品程式碼零引用；僅 `tests/verify_split_patches/verify_split_patches.cs`（獨立驗證腳本）使用 |
| `tests/verify_split_patches/`（505 行） | 手動驗證 console 專案 | 功能已被 `EndlessAiCoreIntegrationTests` 等 xunit 測試覆蓋；確認後可整個移除（連同 `DetectGlobalState`）。**注意**：`.github/workflows/ci.yml` 有明確 restore/build 此專案（防編譯腐化），移除時必須同步刪除 ci.yml 中對應兩行 |
| `PatchProfile.VillageGarrisonQuota3x` legacy bool view（:32） | 舊 API 相容層 | 僅測試使用；若確定無外部設定檔序列化依賴，可標 `[Obsolete]` 後移除 |

### 2.4 InitializeComponent 的「殭屍佈局」

[ModifierForm.cs](src.Modifier/UI/ModifierForm.cs) `InitializeComponent()`（:425-:1504，約 1080 行）為每個控制項寫死 `Location`/`Size`（如 `pnlTitleBar` 寬 1450、`btnClose` 位置 1410 等），但 `ApplyModernLayout()` + `LayoutModernShell()` + `ConfigureSettingsCard()`（ModifierForm.Layout.cs）隨後**全部重新佈局**。舊座標全是死值，且維持了兩套心智模型。

**動作**（中風險、高收益）：`InitializeComponent` 精簡為「建立控制項 + 掛事件 + 設定不變屬性（字型/顏色/文字）」，座標與尺寸完全交給 Layout partial。同時可刪掉建構式中 `Size`, `AutoScrollMinSize` 等被 `ApplyModernLayout` 覆寫的初始化。改動後需人工開啟 UI 目視驗證（可用 `dotnet run --project src.Modifier`）。

### 2.5 在地化字典孤兒 key（自動化清理任務）

`Loc` 的 En/Zh 字典有數百個 key。**動作**：寫一次性腳本交叉比對 `Loc.Get("Key")` 全部使用點 vs 字典鍵，列出並刪除孤兒（注意 `string.Format` 動態 key 不存在於本專案，可安全比對字面值）。En/Zh 兩邊 key 集合也應驗證一致（缺 key 時 fallback 顯示 key 本身，屬靜默錯誤）。

### 2.6 Repo 雜項（非程式碼，僅提醒）

以下皆已在 `.gitignore`、未進版控，但佔用本機空間、且會干擾全文搜尋：

- `.codex/` **1.6 GB**（整套 Ghidra）、`遊戲原始檔案/` 1.4 GB、`re_workspace/` 27 MB、`scratch/` 7.5 MB
- 根目錄 `AgainstRomeModifier_v1.0.0_win-x64.zip`（135 MB 發行包）
- `AI_HANDOFF.md` 已膨脹至 180 KB — 依 AGENTS.md 精神應精簡為「當前狀態」，歷史內容可歸檔到 `docs/handoff-archive/`

**不要**把這些加入版控；只需（經使用者同意後）搬離 repo 目錄或壓縮。

---

## 3. 重複程式碼（去重）

### 3.1 視窗外框（chrome）樣板 ×5～6 份 — 最大宗

以下成員在 **ModifierForm、LauncherForm、TroopPresetForm、TechDocForm、SaveManagerForm**（部分亦在 MapSelectionForm）中逐字重複：

- `CreateRoundRectRgn`/`DeleteObject` P/Invoke 宣告
- `GetRoundPath()`（圓角路徑）
- `TitleBar_MouseDown/Move/Up`（無邊框拖曳三兄弟 + `dragging`/`dragStart` 欄位）
- `StyleButton()`（漸層自繪按鈕，含以 `backColor==特定ARGB` 判斷角色的脆弱邏輯）
- `CardPanel_Paint()`、`Load` 事件的圓角 Region、`Paint` 的霓虹外框
- `UpdateLanguageButtonStyles()`（Launcher/SaveManager/Modifier 三份）
- `fontJhengHei*` 字型欄位組 + Dispose 樣板

**動作**：新增共用 UI 基礎（建議 `src.Modifier/UI/Common/` 起步即可，因為所有 Form 目前都被 Modifier exe 引用；若要 SaveManager/MapEditor 獨立使用則放新專案 `src.UI.Common`）：

1. `DarkFormBase : Form` — 無邊框、圓角 Region、霓虹外框、標題列拖曳、關閉/最小化鈕、DoubleBuffered。各 Form 改繼承。
2. `static class UiTheme` — 顏色常數（目前 `Color.FromArgb(0, 230, 255)` 等魔法色值在 5 個檔案出現 60+ 次）與共享 `Font` 實例。
3. `static class ButtonStyler.Apply(btn, ButtonRole role)` — 以 enum 取代「用背景色反推角色」的判斷。

### 3.2 `LoadTga()` ×2 份（各 ~100 行）

[ModifierForm.Data.cs:46](src.Modifier/UI/ModifierForm.Data.cs:46) 與 [SaveManagerForm.cs:663](src.SaveManager/UI/SaveManagerForm.cs:663) 為同一 TGA 解碼器的兩份副本。

**動作**：抽到共用處（`src.Modifier/UI/Common/TgaDecoder.cs` 或 shared UI 專案；因回傳 `Bitmap` 依賴 System.Drawing，不放 `src.Shared`）。注意兩份實作若有差異（索引色透明處理），以 ModifierForm 版為準並比對。

### 3.3 Stats DataGridView 工廠 ×2 份

`CreateCurrentStatsGrid()`（:239）與 `CreateDefaultStatsGrid()`（:356）約 90% 相同（同欄位、同配色），僅欄位標題（對比 vs 純值）、Tier 欄可見性與 CellFormatting 不同。

**動作**：合併為 `CreateStatsGrid(StatsGridMode mode)`；欄位定義用一張 `(name, headerKey, width, visible)` 表驅動，順便解決欄標題目前寫死中文、未走 `Loc` 的問題（`ApplyLanguageToUI` 之後要重設表頭也會因此變簡單）。

### 3.4 功能開關中繼資料散落 5+ 份手寫清單（高價值）

同一個「功能」目前要在這些地方各登記一次：

1. `FeatureKeys` + `FeatureRegistry`（Core，單一真相 ✅）
2. `ModifierForm` 欄位宣告 + `InitializeComponent` 建構（~40 個 `chkXxx`）
3. `BuildFeatureToggleMap()`（id→toggle 映射；有 `ValidateToggleIds` 防漏 ✅）
4. `BtnEnableAll_Click` / `BtnDisableAll_Click`（[ModifierForm.Presets.cs](src.Modifier/UI/ModifierForm.Presets.cs)）— 手寫逐一 `chkXxx.Checked = ...`，且「排除清單」只存在於註解
5. `ApplyLanguageToUI()`（[ModifierForm.Localization.cs](src.Modifier/UI/ModifierForm.Localization.cs)）— 手寫逐一 `chkXxx.Text = Loc.Get("Key")`
6. `ConfigureSettingsCard(...)` 卡片分組（[ModifierForm.Layout.cs:164-212](src.Modifier/UI/ModifierForm.Layout.cs:164)）
7. `LoadCurrentData` 中手動退訂/重訂 7 個 CheckedChanged 事件清單

新增一個功能要改 5～7 處，漏一處就是隱性 bug（例如新 toggle 忘了加進 DisableAll）。

**動作**：建立 UI 端單一描述表：

```csharp
sealed record ToggleDescriptor(
    string FeatureId,        // = Loc key = FeatureKeys id
    CardId Card,             // 所屬卡片
    bool IncludeInEnableAll, // 取代註解中的排除規則
    bool HasCompanion = false);
```

由這張表生成：toggle 建立、`featureToggles` 映射、EnableAll/DisableAll（迴圈）、`ApplyLanguageToUI`（`toggle.Text = Loc.Get(id)`）、卡片掛載順序、事件退訂清單。`FeatureRegistry.ValidateToggleIds` 保持既有防護。
7 個內容完全相同的 `ChkXxx_CheckedChanged` handler（Balance/RangedRange3x/AllUnitsEntireMapVision/UnitMovementSpeed2x/VillagerMovementSpeed5x/SpellEntireMap/SpellRange3x，全部只是 `LoadDefaultStatsData()` + log）也收斂成一個共用 handler。

### 3.5 還原/套用流程樣板 ×5 份

`BtnApply_Click` / `RestoreAll` / `RestoreStatsOnly` / `RestoreCompatOnly` / `RestoreLanguageOnly`（[ModifierForm.Patches.cs](src.Modifier/UI/ModifierForm.Patches.cs)）共享完全相同的 40 行骨架：路徑檢查 → EnsureBackup → `SetActionButtonsEnabled(false)` → `patchOperationRunner.ExecuteAsync` → `ResetTogglesForCategory` → MessageBox → `LoadCurrentData` → finally 復原按鈕。

**動作**：抽出一個泛用方法：

```csharp
private async Task RunGuardedPatchOperation(
    Func<FileRollbackScope, Task or Action> op,
    FeatureCategory[] categoriesToReset,
    string logStartKey, string logDoneKey, string msgSuccessKey,
    bool requireBackup = true, bool requireExe = true);
```

五個入口各縮成 3～5 行。同時把這些 `async void` 私有方法改為 `async Task`（僅事件 handler 本體保留 `async void`），避免例外遺失。

### 3.6 其他小型重複

- `DetectGamePathFromRegistry()` 包裝：ModifierForm.Data、LauncherForm、SaveManagerForm 三份 → 直接呼叫 `GameDirectoryLocator.DetectFromRegistry()`。
- LauncherForm 三個按鈕 handler（Modifier/SaveManager/MapEditor）各自重複「解析 gamePath → Hide → ShowDialog → finally 恢復語言與視窗」樣板 → 抽 `LaunchChildForm(Func<Form> factory)`。
- `GetExpectedFileCount(string)` 與 `GetExpectedFileCount(string, string)`（Orchestrator）在 §1.5 目標化後自然合併。

---

## 4. 效能

均為小改善，優先級 P2；此工具是桌面工具，無熱路徑瓶頸，**不要為效能犧牲可讀性**。

1. **`FeatureRegistry.GetDisabledValue` 線性搜尋**（[FeatureRegistry.cs:61](src.Core/Core/Features/FeatureRegistry.cs:61)）：`PatchProfile.Get(id)` 每次 miss 都 `All.FirstOrDefault(...)` 掃 50+ 個定義，而 `PatchProfile.Get` 在 detect/apply 路徑被大量呼叫。→ 用 `static readonly Dictionary<string, FeatureDefinition>`（OrdinalIgnoreCase）建索引，`ById(id)` O(1)。
2. **偵測路徑重複解壓 BCI**：`PatchEngine.DetectCurrentPatchState` 每次 new `FeatureDetector` → `BciFeatureDetector` 內部各自 new `EndlessAiOrchestrator`，5 張 ENDL 地圖的 `ak_level.bci` 全部重新讀檔+LZSS 解壓。`ApplyPatches` 已示範共用 orchestrator 快取的正確作法（單一 orchestrator 貫穿 restore+apply）。→ 讓 `DetectCurrentPatchState` 建立一個 orchestrator 傳入 `BciFeatureDetector`（建構參數已存在的話直接用）。
3. **`EndlessAiOrchestrator.ApplyModule` 內含一次 `DetectModule`**：apply 全部 5 個 UserModules 會做 5 次 module 偵測；因檔案快取存在成本可接受，但若做了 §1.5/§1.6 可順手讓 Detect 結果重用。
4. **`LoadDefaultStatsData` 於 7 個 toggle 的 CheckedChanged 都全表重建**：切換單一開關就重解析+重繪 4 個 grid。可接受；若要改，加 200ms debounce 即可，不必大動。
5. **`ModernToggle` 每實例一個 `Timer`**（40+ 個）：僅動畫期間啟動，可不改；若要改，共用一個靜態動畫 clock。
6. **`Log()` 每行 `File.AppendAllText`**（開檔/關檔）：量小可接受；如要改，持有 `StreamWriter`（AutoFlush）並在 Dispose 關閉。

---

## 5. 健壯性 / 在地化補洞（行為微調，逐項確認）

1. ⚠️ **語言偏好被覆寫**：見 §1.3 第二點，這是實際 bug，修正會改變（修好）行為。
2. **硬編碼中文字串未走 `Loc`**：
   - [ModifierForm.Data.cs:227](src.Modifier/UI/ModifierForm.Data.cs:227)「成功載入 {0} 個兵種圖示。」、:470「請先在右上角設定遊戲路徑。」
   - [ModifierForm.cs:267](src.Modifier/UI/ModifierForm.cs:267)「舊版腳本安全遷移失敗: 」
   - [ModifierForm.Patches.cs](src.Modifier/UI/ModifierForm.Patches.cs) 內 `ExecuteAsync` 的三段 rollback 訊息（「已建立修改前檔案回復點。」等，5 處 ×3 句）
   - `InitializeComponent` 中所有初始 `Text = "中文..."`（隨後被 `ApplyLanguageToUI` 覆寫的可直接改成空字串或 `Loc.Get`）
   - `menuRestore` 四個 `ToolStripMenuItem` 初始文字（:1311-:1314；`ApplyLanguageToUI` 有重設，初始值可改 `Loc.Get`）
   - 英文狀態字樣三元運算子模式 `Loc.CurrentLanguage == Language.English ? "enabled" : "啟用"` 重複 7 次 → `Loc.Get("StatusEnabled"/"StatusDisabled")`
3. **靜默吞例外**：`SaveManagerForm.cs:524/:535`（SelectionChanged 的 `catch (Exception) { }`）、`MapSelectionForm.cs:344`、`GameMapCatalog.cs:54`、`EndlessMapCatalog.cs:60` — 至少補 `Debug.WriteLine` 或 log，維持行為但留下診斷線索。
4. **`async void` 非事件方法**：`RestoreAll`/`RestoreStatsOnly`/`RestoreCompatOnly`/`RestoreLanguageOnly`（ModifierForm.Patches.cs）與 `RefreshSavesAndBackups`（SaveManagerForm:477）→ 改 `async Task`，呼叫端事件 handler `await`。
5. **`GameLZSS.Decompress` 對 `input == null` 回傳 `input ?? Array.Empty`**：nullable 已啟用，簽章應收緊為非 null 並移除防禦分支（確認呼叫端）。

---

## 6. 不建議動的部分（明確排除）

- `CustomPatches.cs`（1624 行）：雖大，但每個 `P*Patch` 類別是自足的逆向成果單元，內聚性高、有完整測試對應。只做「一類一檔」的**純搬移**（`Core/EndlessAi/Patches/P4_RetreatDeadlinePatch.cs` 等）可以，**不要**改任何簽章/常數/演算法。
- `ExePatchModel.cs`（927 行）：純資料表（offset + 位元組序列），配 `ExePatchModelTests` 697 行守護。保持原樣。
- `Backup.zip` 內嵌資源機制、`FileRollbackScope`/`SafeFileWriter` 寫入管線：設計良好，勿動。
- `IPatchFileContributor` / `FeatureRegistry` / `FeatureKey<T>` 模式：這是本專案解耦的核心，新功能應**加入**此模式，而非另闢路徑。
- MapEditor（OpenTK 3D）：類別切分合理（Terrain* 系列單一職責），本輪不動；唯一例外是 `MapEditorForm.cs`（1275 行）可日後比照 ModifierForm 拆 partial。

---

## 7. 審查者附加建議（超出本次範圍的主動建議，選做）

以下不是這次審查發現的「問題」，而是能讓後續維護與 AI 協作更省力的投資，依 CP 值排序。

### 7.1 用工具鏈自動守住死碼與風格（CP 值最高，先做）

- 新增根目錄 `Directory.Build.props`，把五個 csproj 重複的共通屬性（`TargetFramework`、`LangVersion`、`Nullable`、`PlatformTarget`、`InternalsVisibleTo` 樣板）收斂到一處——這本身就是本文件精神（單一真相來源）在建置層的落實。
- 同時開啟 .NET 分析器，讓「死碼掃描」從一次性人工變成每次編譯自動執行：

  ```xml
  <PropertyGroup>
    <EnableNETAnalyzers>true</EnableNETAnalyzers>
    <AnalysisLevel>latest-recommended</AnalysisLevel>
  </PropertyGroup>
  ```

  搭配 `.editorconfig` 把 `IDE0051/IDE0052`（未使用的私有成員/欄位）、`IDE0005`（多餘 using）設為 `warning`。§2 列出的死碼多數會被直接點名，之後也不會再累積。
- CI（`.github/workflows/ci.yml` 已存在且健全）加一步 `dotnet format --verify-no-changes`，鎖住風格與 using 整潔度。
- 警告清零後，逐步在 `Directory.Build.props` 開 `TreatWarningsAsErrors`（可先只針對 nullable：`<WarningsAsErrors>nullable</WarningsAsErrors>`）。

### 7.2 為「語言偏好覆寫」bug 先補一條特徵測試

§1.3 的修正屬行為變更，建議先寫測試再修：`settings.json 已存 English + 系統語系 zh-TW → new ModifierForm 後 Loc.CurrentLanguage 應維持 English`。`Loc` 拆出 `UserSettings` 後注入設定路徑即可測（不再需要 `OverrideLanguageForTesting` 後門）。這也是日後所有 UI 邏輯下沉的示範樣板：**先讓它可測，再改它**。

### 7.3 功能組合可匯出/匯入（功能建議，非重構）⚠️

`PatchProfile` 已是完整的抽象（id→value 字典），距離「把目前勾選組合存成 JSON、一鍵還原」只差一個 codec——而 `TroopPresetCodec` 已示範過同樣模式。對使用者價值高（重灌/換機/分享設定），對程式碼是純增量。若做，記得在 JSON 加 `version` 欄位，沿用 `NormalizeCompositeValues()` 既有的舊格式遷移機制。

### 7.4 AI_HANDOFF.md 瘦身策略

180 KB 的交接檔已超過多數 AI 單次好好閱讀的合理範圍，且「歷史嘗試記錄」與「當前狀態」混在一起。建議：

- `AI_HANDOFF.md` 只保留：當前目標、進行中事項、活躍問題、最近一次 build/test 結果（目標 < 15 KB）。
- 已驗證的**事實性知識**（offset、簽章、檔案格式結論）移入 `docs/reverse-engineering/`——該目錄已存在且結構良好（`exe-functions.md`、`known-patches.md` 等），別讓同一事實在兩處各自演化。
- 已結案的歷史嘗試歸檔到 `docs/handoff-archive/YYYY-MM.md`。

### 7.5 發行流程小補強

- `tools/publish.ps1` 的 `$Version` 目前是手動參數；建議同步寫入組件版本（`-p:Version=$Version`）並在 About/標題列顯示，方便使用者回報問題時對版本。
- 產出的 zip 檔名已含版本號，建議同時輸出 `SHA256SUMS.txt`（發佈到論壇/GitHub Release 時可驗證完整性）。
- 根目錄的 135 MB 舊發行 zip 移到 repo 外的發行資料夾，避免誤入未來的打包腳本輸入。

### 7.6 明確「不值得做」的清單（避免後續 AI 過度工程）

- **不要**引入 DI 容器、MVVM 框架或把 WinForms 改寫成其他 UI 框架——本專案規模下純屬負資產。
- **不要**為 `ExePatchModel` / `CustomPatches` 寫抽象層或 code-gen；位元組表就該是位元組表。
- **不要**追求測試覆蓋率數字；現有測試策略（characterization + round-trip + fixture）方向正確，補測試只補在「即將要改的地方」。

---

## 8. 追加審查發現（第二輪深讀，Core 資料解析層）

這批集中在「兵種屬性解析」這條路徑，是全專案重複密度最高的角落。

### 8.1 `double.TryParse` 樣板出現 64 次（最高頻重複）

`double.TryParse(cols[i].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out var v)` 這一長串在 **12 個檔案共 64 處**逐字重複（`ObjdefFeatureDetector` 18、`ModifierForm.Data` 18、`UnitStatParser` 5、`UnitBaselineCatalog` 5、`ClScriptFeatureDetector` 3、`TroopPresetForm` 7…）。每處都在做「取欄位 → Trim → 以不變文化解析 double，失敗給 0」。

**動作**：在 `Core` 加一個小工具（例如 `ObjdefRow` 擴充或 `static double ParseCol(string[] cols, int index)`）：

```csharp
internal static double ParseCol(string[] cols, int index) =>
    index >= 0 && index < cols.Length &&
    double.TryParse(cols[index].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double v)
        ? v : 0;
```

呼叫端 `double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), …, out curHp);` → `double curHp = ObjdefRow.ParseCol(cols, (int)ObjdefIndex.Hp);`。順帶把目前散落各處「忘了做邊界檢查」的解析統一成安全版本（`UnitStatParser` 已做邊界檢查，多數 UI 呼叫沒做）。這是 §1.4「UI 解析下沉」的前置基礎。

### 8.2 `BackupManager` 淪為 static 轉呼叫門面（命名雙軌）

[BackupManager.cs:134-182](src.Core/Core/Services/BackupManager.cs:134) 有一整段「屬性解析核心方法（原本寫在 UI 裡的）」，全部只是**改名轉呼叫**：

| BackupManager static 方法 | 實際實作 |
|---|---|
| `GetMeleeAndRangedDmg` | `UnitStatParser.GetMeleeAndRangedDamage` |
| `GetMeleeAndRangedRelt` | `UnitStatParser.GetMeleeAndRangedReload` |
| `GetUnitMaxRange` | `UnitStatParser.GetMaximumRange` |
| `SupportsConfigurableSpellRadius` | `UnitStatParser.SupportsConfigurableSpellRadius` |
| `MergeUnitStatsLayers` | `UnitStatParser.MergeLayers` |
| `GetCleanEparaText` | `CleanEparaBaseline.Text` |

結果是**同一個運算有兩個名字**：`ObjdefFeatureDetector` 呼叫 `BackupManager.GetMeleeAndRangedDmg`，而 `ModifierForm.Data`/`LoadCurrentData` 呼叫 `UnitStatParser.GetMeleeAndRangedDamage`——讀 code 的人會以為是兩件事。而且這些是掛在**有狀態的** `BackupManager` 上的 `static` 方法，語意誤導（看起來需要 backup 實例，其實不需要）。

**動作**：刪除這些 static 轉呼叫，呼叫端一律直接用 `UnitStatParser` / `CleanEparaBaseline`。`BackupManager` 只保留真正需要實例狀態的成員（`GetBackupUnitRows`、`GetOriginalStats`、`GetBaseStatsForUnit` 等走 `_unitBaselines`）。這讓 `BackupManager` 回歸單一職責（管備份檔位元組），解析邏輯集中在 `UnitStatParser`。

### 8.3 `ObjdefFeatureDetector.Detect` 是 200 行巨型方法

[ObjdefFeatureDetector.cs:15](src.Core/Core/Features/Objdef/ObjdefFeatureDetector.cs:15) 的 `Detect` 一路做完：解壓、解析當前+原版兩份 objdef、六項旗標偵測、range3x 探測、然後一個**逐兵種比對 8 種屬性判斷 balance** 的內嵌迴圈（每個屬性各兩行 TryParse，約 40 行）。這個「逐兵種讀 orig/cur 成對屬性」的迴圈，與 UI 的 `LoadCurrentData`（[ModifierForm.Data.cs:757](src.Modifier/UI/ModifierForm.Data.cs:757)）**是同一份解析邏輯的第三份拷貝**。

**動作**：§1.4 提議的 `CurrentStatsReader` 應設計成同時服務三方——balance 偵測、當前數值頁、預設數值頁都吃它輸出的 `UnitStatRow`（含成對 orig/cur）。`Detect` 內的 balance 判斷抽成 `IsFileBalanced(IReadOnlyList<UnitStatRow>)`。做完這步，§8.1 的 TryParse 樣板在解析路徑上會自然消失大半。

### 8.4 魔法欄位索引與 `ObjdefIndex` enum 並用

[ObjdefFeatureDetector.cs:49](src.Core/Core/Features/Objdef/ObjdefFeatureDetector.cs:49) 用 `cols[52].Trim()` 取兵種名，但同檔 :80 起又用 `cols[(int)ObjdefIndex.Name]`——`ObjdefIndex.Name` 應該就是 52。同一份資料的同一欄，一處魔法數字一處具名 enum。

**動作**：全部改用 `ObjdefIndex`；grep `cols\[\d+\]` 找出所有裸數字索引一併具名化。

---

## 9. 第三輪發現（SaveManager / MapEditor 子系統）

### 9.1 正面範例（勿動，可作為其他重構的樣板）

- **`SaveBackupService`**（[SaveBackupService.cs](src.Core/Core/Services/SaveBackupService.cs)）：寫得很好——寫入走 `.tmp` + `File.Move` 原子替換、還原有完整 rollback（movedOld/movedNew 兩段旗標）、`IsSimpleName()` 防路徑穿越、以 lastWrite 失效的執行緒安全快取、legacy 檔名向後相容。這是本專案 IO 服務的黃金標準，UI 只是薄殼呼叫它。**§1.4 的 `CurrentStatsReader` 可參考此類別的介面設計**（record 輸出 + 純函式 + 例外語意清楚）。

### 9.2 「初始遊戲路徑解析」三段 fallback 重複 4+ 處

`registry → AppContext.BaseDirectory 有 Against_Rome.exe 就用它 → C:\Program Files (x86)\Against Rome` 這串 fallback 在 [ModifierForm.cs:1299](src.Modifier/UI/ModifierForm.cs:1299)、[SaveManagerForm.cs:61](src.SaveManager/UI/SaveManagerForm.cs:61)、[LauncherForm.cs](src.Modifier/UI/LauncherForm.cs)（三個 handler 各一份）逐字重複，且魔法路徑 `C:\Program Files (x86)\Against Rome` 也散落多處。

**動作**：在 `GameDirectoryLocator`（Core，已負責 registry 偵測）加 `ResolveInitialGamePath(string? preferred)`，把整串 fallback 收進去，四處各縮成一行。魔法路徑集中成一個常數。

### 9.3 ⚠️ 專案內存在**兩套**在地化機制（架構級不一致）

- 機制 A：`Loc.Get("key")` 字典查表 — ModifierForm、LauncherForm、SaveManager 使用。
- 機制 B：內嵌三元 `isEn ? "English" : "中文"` — **整個 MapEditor**（[MapSelectionForm.cs:166-177](src.MapEditor/MapSelectionForm.cs:166) 等數十處），加上 `MapEditorForm` 把中文字面值**寫死在欄位初始化器**（`"搜尋草地、沙地…"`、`"目前筆刷：尚未取樣"`、`"儲存"`、`"復原"`…約 60+ 處）。

後果：(1) 同一份翻譯要維護兩種格式；(2) MapEditor 欄位初始化器裡的字串是建構時定死的，語言切換鈕（`_btnLangZH`/`_btnLangEN`）能不能覆蓋到全都得靠 `ApplyLanguageToUI` 逐一重設，極易漏；(3) MapEditor 的翻譯完全沒有進 `Loc` 字典，未來要加第三語言得改兩個地方。

**動作**（P2，範圍較大可分期）：
- 短期：把 MapEditor 的 `isEn ? … : …` 與欄位初始化字串，全部改走 `Loc.Get(key)`，翻譯集中到 `Loc` 字典。欄位初始化器只留 `Dock`/`Size` 等不變屬性，文字交給 `ApplyLanguageToUI`。
- 這也順勢解決 §5.2（硬編碼中文）在 MapEditor 的部分，兩者一起做。

### 9.4 MapEditorForm 1275 行單檔

與 ModifierForm 同類問題（§2.4 已提 ModifierForm）。`MapEditorForm` 已把地形/場景邏輯正確下放到服務（`SdlSceneEditService`、`TerrainBlendEditSession`、`FloorMaterialCatalog`），所以**這不是職責問題、只是檔案大小問題**。可比照 ModifierForm 拆 partial：`MapEditorForm.Layout.cs`（InitializeComponent/佈局）、`MapEditorForm.Scene.cs`（場景物件 CRUD）、`MapEditorForm.Terrain.cs`（材質筆刷/blend）、`MapEditorForm.cs`（協調 + 狀態）。純搬移、低風險，但收益也只是可讀性，優先級 P2。

### 9.5 SaveManagerForm 的 in-flight 旗標樣板

`_savesRefreshInFlight`、`_repairInFlight` 兩個「防重入」布林 + try/finally 復位，與 ModifierForm 的 `SetActionButtonsEnabled(false/true)` 是同一個「操作進行中鎖 UI」需求的兩種不同實作。§3.5 抽 `RunGuardedPatchOperation` 時，可把「防重入 + 禁用按鈕 + finally 復原」做成同一個共用 helper，讓兩個 Form 共用。

---

## 10. 驗收清單（每批變更後）

```
dotnet build AgainstRomeModifier.slnx -warnaserror:nullable
dotnet test
```

- [ ] 全部測試綠燈（目前測試涵蓋：patch round-trip、characterization、UI preset、layout、backup 基礎設施）
- [ ] `dotnet run --project src.Modifier` 手動冒煙：啟動器 → 修改器開啟 → 切換語言 → 切分頁 →（不要按「執行修改」除非在測試副本上）
- [ ] 死碼刪除批次：確認 `grep -r` 無殘留引用
- [ ] UI 去重批次：截圖前後比對主視窗、TroopPresetForm、SaveManagerForm 外觀一致
- [ ] 未經使用者同意不建立分支、不 commit
