# AI Handoff - Live Project Memory

本檔案記錄了 Against Rome Modifier 專案的當前目標、最新狀態、歷史完成事項以及後續計畫。
This file records the current objective, latest state, completed historical items, and next steps for the Against Rome Modifier project.

## 當前目標 / Current Objective

執行 `docs/DECOUPLING_PLAN.md`：進行行為保留的階段性功能解耦（Behavior-preserving staged feature decoupling）。

## 最新狀態 / Current State (2026-07-10)

- **分支 (Branch)**: `主要開發`
- **本輪完成**: `PatchOptions` 已刪除；Apply/Detect 經 `IFeatureModule` Registry 迴圈；四個 Restore 端點統一走 `RestoreCategories`；FoodHealing 與 Endless AI 共用 BCI cache/`SaveAll`；中英文 README/TechDoc 已更新。
- **最終驗證**: `dotnet build AgainstRomeModifier.csproj -c Release` 為 0 warnings/errors；重建後 `dotnet test tests/AgainstRomeModifier.Tests -c Release --no-restore` 為 90/90 passed；`git diff --check` 通過。
- **Git 狀態**: 本輪變更尚未 commit（依規範未自動提交）；基線 commit 為其他代理建立的 `79e6585`。
- **未暫存變更 (User Changes)**: 保持未追蹤的 `docs/DECOUPLING_PLAN.md`，請勿覆蓋。
- **基線驗證 (Verification)**:
  - 建置指令：`dotnet build AgainstRomeModifier.csproj -c Release` (0 warnings/errors)
  - 測試指令：`dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build` (目前共有 **88 個測試全部通過** 88/88 passed)
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

## 歷史完成事項 / Completed History (2026-07-09 及之前)

以下為專案先前已完成並通過實機驗證的重要功能，保留作為開發背景知識：

### 1. 主控制台 UI 扁平化與防截斷優化
- 重構主介面為「單頁扁平無外框佈局」，移除所有分頁導覽與卡片外框，在單一高度內展示所有選項，避免捲動。
- 移除所有問號提示圖標（`lblHelp...`），將提示訊息直接綁定在 `ModernToggle` 及速度控制項的 Tooltip 上，滑鼠懸停即可看見。
- 精簡並人性化所有修改選項的名稱與提示，避免文字截斷（保持在 6~12 字內）。
- 將「AI 終極戰爭模式」區塊排版重構為與其他分類一致的單一垂直直列排版，消除橫向雙欄排版的截斷與對齊問題。

### 2. 本機實體備份與還原機制 (.bak)
- 為了解決開源編譯版在無內建 `Backup.zip` 時可能重複讀取已修改檔案而導致還原失效的問題，重構了 `BackupManager.cs`。
- 新增機制：當檢測到 `.bak` 備份檔時直接讀取；若不存在則自動校驗原檔狀態（檢查 `Against_Rome.exe` 補丁狀態與 `objdef.dau` 領袖 HP）確認安全後，複製生成實體 `.bak` 檔案。
- 建立 `PhysicalBackupTests.cs` 驗證自動備份、備份加載、髒資料攔截與還原同構性。

### 3. 雙語 README 與技術文件更新
- 重構 `README.md`（英文）並新建 `README.zh-TW.md`（繁體中文），提供對齊的雙語說明文件。
- 同步更新中英文技術文件 `TechDoc.md`、`TechDoc_EN.md` 與 `known-patches.md`，補齊「主堡生命值提升 10 倍」與「全地圖建造限制消除」的技術規格與 Ghidra 逆向細節。

### 4. 建造紅框範圍修改為全地圖 (EntireMap)
- 升級「村莊建造範圍」修改功能，將 ESI/EDI 暫存器乘上 5 倍的指令替換為直接寫入常數 `30000` (`0x7530`)，徹底移除地圖建造限制。
- 原有的 5x 狀態歸為 `Legacy5x`，保留偵測與平滑遷移還原的相容性。

### 5. 主堡生命值提升 10 倍 (HqHp10x)
- 於 `objdef.dau` 中修改 `Bau` 開頭且名稱含 `Hau` 的行，將其生命值 `Hp` 欄位（Index 19）數值提升 10 倍。
- 整合至 `ObjdefFeaturePatcher` 與 `FeatureDetector`。

### 6. 羅馬增援 AI 終極模式實機修復 (P9)
- 修復了增援隊伍（type-5 `RoemischerNachschub`）只有村民而無士兵的 Bug。
- **根因**：Site 8 原本被改為 `[66,0]`，但它在 BCI 中實際上是「士兵生成預算」而非撤退配額，導致預算歸零不產兵。
- **解決方案**：Site 8 保持原版 `[90,6]`；僅 Site 9 修改為 `[66,0]`（捐贈剩餘歸零）。並修改狀態 49 的型態過濾（jz 偏移量由 `92 -> 0`），使士兵小隊也受歸零配額影響而一併捐贈給 type-4 軍事 AI。
- 實機確認：增援部隊會正常包含村民與 2~4 名士兵，且抵達村莊後留守不撤退。

---

## 後續步驟 / Next Steps

1. **已完成：移除 `PatchOptions` 相容層**
   - `PatchProfile` 現為 Core、UI 與測試唯一選項載體；`PatchOptions.cs` 已刪除，搜尋無剩餘引用。
   - Release 建置成功（0 warnings/errors），完整測試 88/88 通過。
2. **完成以 FeatureCategory 為基礎的還原編排（Restore Orchestration）邏輯**
   - 已清除 `RestoreOriginalFilesInternal` / `RestoreStatsOnlyInternal`；四個公開 Restore 端點與 Apply 前置還原統一走 `RestoreCategories`，並保留原本落地順序。
   - 首次建置的 CS8602 nullable warning 已以明確非 null 保證解決。Release 建置 0 warnings/errors，T1–T4 5/5、完整測試 88/88 通過。
3. **Phase 4: 透過共用的 BCI 快取/上下文路由 FoodHealing**
   - 已將 `FoodHealingFeature` 接入 `EndlessAiOrchestrator` 的 `BciScriptFile` 快取；Apply、Restore、Startup migration 均先規劃後由一次 `SaveAll` 統一落地，不再直接寫 BCI。
   - Release 建置 0 warnings/errors，T1–T4 5/5、完整測試 88/88 通過。
   - 已更新 `TechDoc.md`、`TechDoc_EN.md`、`README.md`、`README.zh-TW.md` 的 Features/Profile/category restore/BCI cache 架構說明與新增功能流程。
   - `BciCacheIntegrationTests` 驗證 Apply 前磁碟不變、`SaveAll` 後才落地；重建測試專案後完整測試為 89/89 通過。
4. **最終驗證**
   - Release 建置目前為 0 warnings/errors，重建後完整測試 89/89 通過。
   - 尚需檢查最終 diff、搜尋過時架構名稱與確認工作樹只包含本次預期變更。

## 稽核發現 / Audit Finding

- 已補上 `IFeatureModule`、邏輯 `PatchContext`、`DetectContext`；所有 Registry 項目均實作契約，Apply/Detect 實際經 Registry 迴圈傳遞值。共享檔案由既有 per-file composers 一次產生，保留單次 PFIL 改寫語意。
- 新增 Registry contract round-trip 測試；Release 建置 0 warnings/errors，完整測試 90/90 通過。
