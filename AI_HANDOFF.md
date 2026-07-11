# AI Handoff - Live Project Memory

本檔案記錄了 Against Rome Modifier 專案的當前目標、最新狀態、歷史完成事項以及後續計畫。
This file records the current objective, latest state, completed historical items, and next steps for the Against Rome Modifier project.

## 當前目標 / Current Objective

新增實驗性功能：「拋射彈道增高」(ProjectileArcHeight) 與「遠程命中強化」(RangedAccuracy)。

## 最新狀態 / Current State (2026-07-10)

- **分支 (Branch)**: `主要開發`
- **本輪完成**:
  - **拋射物彈道逆向研究**: 以 Ghidra 偽代碼庫 + capstone 反組譯完成拋射物飛行與命中機制解碼，完整證據見 `docs/reverse-engineering/projectile-ballistics.md`。核心結論：`w*_emit`（objdef）= 拋射垂直初速（16.16 定點，弓=110、投石車=155/142）；partgeo.dau `ysub` = 重力；命中無擲骰，落點 `w*_drad` 半徑內的敵人吃傷害；cl_epara `[ProjectileVarianceOnMove]` 控制對移動目標預判的隨機散布。
  - **ProjectileArcHeight（拋射彈道增高）**: objdef 所有「akti=1 且 emit>0」武器的 `w*_emit` ×1.5，同時 partgeo.dau 六個拋射物條目（Wurfspeer00/Wurfaxt00/Katapultstein00/Katapultstein01/Pfeil00/Spiess00）的 `ysub` ×1.5 —— 同倍率縮放使弧頂增高約 5 成、落點與飛行時間不變。新增 `PartgeoPatcher`（六筆全中否則整次中止）。
  - **RangedAccuracy（遠程命中強化）**: objdef 拋射武器 `w*_drad`（落點傷害半徑）×2，加上新增 `EparaPatcher` 將 cl_epara.ini `[ProjectileVarianceOnMove]` 0.5→0.0（預判零散布）。
  - **partgeo.dau 備份接線**: partgeo 不在內嵌 Backup.zip，`TryAutoHealBackupFiles` 以現場檔案補齊時會先做 `IsPartgeoOriginal`（Pfeil00 ysub==5832704）純淨檢查並立即建立 `.bak`；缺備份時功能關閉可正常運作、開啟則以明確錯誤中止（不列入 FindMissingBackupResources 硬性需求，避免舊環境被卡死）。
  - **偵測**: `FeatureDetector.HasProjectileWeaponScale` 全列比對 emit×1.5 / drad×2（與 patcher 寫入邏輯對稱）。
  - **UI**: 實驗性分區新增 `chkProjectileArcHeight`、`chkRangedAccuracy` 兩個開關（卡片高度 590→686），雙語名稱與 Tooltip 已綁定；一鍵全開維持排除實驗性功能、一鍵全關會關閉。
  - **注意（尚未實機驗證）**: 兩功能為靜態逆向推導結論（發射公式已逐指令確認），建議下一輪實機目測弧高與命中變化。敵我全陣營共用同一份數值，敵方遠程也會同步增強。
- **最終驗證**: `dotnet build AgainstRomeModifier.csproj -c Release` 為 0 warnings/errors；`dotnet test tests/AgainstRomeModifier.Tests -c Release` 為 **98/98 passed**（含對真實遊戲檔案副本的 apply→detect→restore→逐位元組還原 整合測試，已涵蓋兩個新功能與 partgeo 自動補齊）。
- **Git 狀態**: 本輪變更尚未 commit（依規範未自動提交）；基線 commit 為其他代理建立的 `9fb5a4a`。
- **未暫存變更 (User Changes)**: 保持未追蹤的 `docs/DECOUPLING_PLAN.md`，請勿覆蓋。
- **基線驗證 (Verification)**:
  - 建置指令：`dotnet build AgainstRomeModifier.csproj -c Release` (0 warnings/errors)
  - 測試指令：`dotnet test tests/AgainstRomeModifier.Tests -c Release` (目前共有 **91 個測試全部通過** 91/91 passed)
- **最新解耦進度**:
  - **Phase 0 (完成)**: `BackupZipGameFixture.cs` 將內嵌的備份解壓至暫存目錄，並補齊測試所需的 synthetic BCI/ToEng 資源。`CharacterizationTests.cs` 實現 T1–T4 基礎測試。
  - **Phase 1 (完成)**: 將 PatchEngine 中的各項 patches 成功移出至 `Features/` 子目錄：
    - `IniFeaturePatcher.cs` (cl_script/ress)
    - `ObjdefFeaturePatcher.cs` (objdef 數值修改)
    - `MaxPopulationFeature.cs` (team.dat 人口修改)
    - `ExeFeaturePatcher.cs` (EXE 補丁修改)
    - `FoodHealingFeature.cs` (待機回血 BCI 修改)
    - `LanguagePackFeature.cs` (語言包覆蓋)
    - `DgVoodooFeature.cs` (dgVoodoo 部署)
    - `FeatureDetector.cs` (整合偵測輔助)
    - `PatchEngine.cs` 已成功縮減至 283 行（低於 400 行目標），無任何功能特有的常數或正則表達式。
  - **Phase 2 (完成)**: 引入 `FeatureCategory`、`FeatureValue`、`PatchProfile` 與唯一的 `FeatureRegistry`。原本的 `PatchOptions` 已改寫為背後由 `PatchProfile` 驅動的相容層，所有 88 個測試均通過。
  - **Phase 3 (完成)**: UI 繫結遷移完成。`ModifierForm` 的 apply/detect/restore 邏輯已全面透過 `PatchProfile` 與 `FeatureRegistry` 驅動，移除了大批冗餘的手寫對應代碼。新增 `FeatureRegistryTests` 使測試總數達到 88 個，全部通過。

---

## Follow-up: SpellRange3x corrected to effect radius (2026-07-10)

- 依舊版 ClScript patch 文件，`SpellRange3x` 應修改 `cl_script.ini` `[Spells] Radius`，不是 `objdef` 祭司施法距離欄位。
- `SpellEntireMap` 仍保留給 `objdef` `PriestSpell1/2/3`；UI 文案已改回「法師法術範圍提升 3 倍」。
- 已同步 patch、detect、預覽與回歸測試；Release tests 97/97 passed。

## Follow-up: priest spell slots restored (2026-07-10)

- 舊版 `objdef-fields.csv` 與 `ModifierForm.Patches.cs` 確認祭司施法距離使用 `PriestSpell1/2/3` = columns 80/81/82；重構版誤將 80/81 當成一般武器距離，導致功能失效。
- 已恢復祭司專用 patch 與 detector/UI range reader，並保留非祭司武器 range 行為。
- Release tests: 96/96 passed。

## 2026-07-10 experimental feature verification

- 反編譯確認 `cl_script.ini` 的 KEL Spell1 `Value` 是治療量，Spell3 `Value/Value2` 是復活 HP/士氣；`objdef.dau` 的 priest `w*_rad1/w*_rad2` 才是施法距離，spell `Radius` 是效果/搜屍半徑。
- UI 文案已將 `SpellEntireMap` / `SpellRange3x` 從「施法範圍」改為「施法距離」，並明確說明不改變效果半徑。
- 新增 priest casting-distance round-trip test；Release build/test 已完成，96/96 通過。

## 歷史完成事項 / Completed History (2026-07-09 及之前)

以下為專案先前已完成並通過實機驗證的重要功能，保留作為開發背景知識：

### 1. 主控制台 UI 扁平化與防截斷優化
- 重構主介面為「單頁扁平無外框佈局」，移除所有分頁導覽與卡片外框，在單一高度內展示所有選項，避免卷動。
- 移除所有問號提示圖標（`lblHelp...`），將提示訊息直接綁定在 `ModernToggle` 及速度控制項的 Tooltip 上，滑鼠懸停即可看見。
- 精簡並人性化所有修改選項的名稱與提示，避免文字截斷（保持在 6~12 字內）。
- 將「AI 終極戰爭模式」區塊排版重構為與其他分類一致的單一垂直直列排版，消除橫向雙欄排版的截斷與對齊問題。

### 2. 本機實體備份與還原機制 (.bak)
- 為了解決開源編譯版原生無內建 `Backup.zip` 可能造成重複修改與還原失效的 Bug，重構了 `BackupManager.cs`，引進自動偵測、髒資料檢驗、與生成同構備份實體檔的機制。
- 建立 `PhysicalBackupTests.cs` 驗證自動備份、備份加載、髒資料攔截與還原同構性。

### 3. 雙語 README 與技術文件更新
- 重構並新增對齊的中英文技術文件，補齊「主堡生命值提升 10 倍」與「全地圖建造限制消除」的技術規格與 Ghidra 逆向細節。

### 4. 建造紅框範圍修改為全地圖 (EntireMap)
- 升級「村莊建造範圍」修改功能，替換原本的 5x 邏輯，改為直接寫入常數 `30000` (`0x7530`)，徹底移除地圖建造限制。

### 5. 主堡生命值提升 10 倍 (HqHp10x)
- 於 `objdef.dau` 中將其生命值 `Hp` 欄位（Index 19）數值提升 10 倍。

### 6. 羅馬增援 AI 終極模式實機修復 (P9)
- 修復了增援隊伍（type-5 `RoemischerNachschub`）只有村民而無士兵的 Bug。

---

## 後續步驟 / Next Steps

1. **已完成：施法範圍 Bug 修復與測試全綠通過**
   - 包含數值補丁修改、高精度回讀偵測、表格對比動態預覽連動、以及完整單元與整合測試。
2. **後續潛在規劃**
   - 依據使用者後續需求進行更多微調或版面美化。
## Follow-up: priest casting distance uses sight radius (2026-07-10)

- User reported the experimental priest casting-distance toggle had no effect and recalled it is bound to sight.
- Rechecked objdef headers/decompilation: index 24 is `Sirad` (sight radius); 80/81/82 are weapon `w1_rad1/w1_rad2/w1_angl`, so the prior PriestSpell slot patch was incorrect.
- `SpellEntireMap` now writes priest `Sirad` to 30000; 80/81/82 remain unchanged. Range readers and detection use `Sirad`.
- Updated reverse-engineering docs/schema and regression test. `dotnet test tests/AgainstRomeModifier.Tests -c Release --no-restore`: 97/97 passed.
## Follow-up: experimental modifiers take precedence over custom troop layers (2026-07-10)

- When the four toggles are enabled, custom troop stats now ignore overlapping fields at patch/preview time: movement speed (index 4), ranged/priest distance (index 7), and spell radius (index 8).
- Non-overlapping custom attributes remain active; no installed game files were touched.
- Added regression coverage for the layer merge. Release test suite: 98/98 passed; Release build: 0 warnings/errors.
## Follow-up: remove four fields from custom troop attributes (2026-07-10)

- User clarified the four experimental modifiers must be independent, not merely higher priority.
- Custom troop layers now always discard Speed (4), Range (7), and SpellRadius (8), using the baseline values; the four feature patchers own those changes exclusively.
- Troop preset editor marks Speed/Range/SpellRadius read-only and import/apply/export normalize them to baseline values, preventing legacy presets from reintroducing duplicate modifiers.
- Release verification: 98/98 tests passed; build completed with 0 warnings/errors.
## Follow-up: priest sight removed from custom layers (2026-07-10)

- Because priest casting distance is bound to `Sirad` sight, priest custom Sight is also normalized to the baseline and made read-only in the preset editor.
- Custom Speed/Range/SpellRadius remain permanently excluded from custom layers; legacy arrays are sanitized at merge time.
- Release tests remain 98/98 passed.
## Follow-up: balanced template normalization (2026-07-10)

- Confirmed the built-in balanced template previously still populated the removed fields in its custom dictionary.
- Added `NormalizeIndependentCustomFields`: template/import/apply now reset Speed, Range, SpellRadius, and priest Sight to original values; preset editor keeps them read-only.
- Release tests: 98/98 passed; build: 0 warnings/errors.
## Follow-up: remove fields from custom troop editor and file format (2026-07-10)

- User clarified normalization was insufficient; the custom feature itself must not contain the four independent modifiers.
- TroopPresetForm now exposes only six editable fields (HP, damage, VW, AW, sight, cooldown). Speed, range, and spell radius columns and validation were removed; priest sight is still normalized because it gates casting distance.
- New exports use `UnitKey=HP,Dmg,VW,AW,Sight,Relt`; legacy 9-field imports remain readable but discarded fields are never written back or applied.
- Release tests: 98/98 passed; build: 0 warnings/errors.
