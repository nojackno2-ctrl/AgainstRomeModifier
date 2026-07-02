# 修改器重構變更總結 (Against Rome Modifier Refactoring Summary)

此檔案為此對話中針對《Against Rome》修改器的所有優化重構總結，包含程式碼變更與設計架構，可用於其它 AI 代理審查。

完整的程式碼 Patch Diff 已儲存於本機專案目錄：[changes_patch.diff](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/changes_patch.diff)

---

## 1. 重構背景與目的
為了避免在同一個檔案上修改多個功能時互相干擾、發生 I/O 競爭與殘留檔案損壞，實作了三大架構優化：
1. **疊加式兵種屬性模型 (Layered Stats)**：解決「平衡模式」與「自訂微調」的衝突，使其有機結合。
2. **記憶體預檢驗與交易寫入 (Dry Run & Transactional Write)**：主要資料補丁先在記憶體中生成與校驗，再進入受 rollback 保護的寫入階段。
3. **cl_script.ini 增量修補與 Radius 累乘防禦**：支援保留現有非管理設定，同時確保 Radius 永遠以安全備份值為縮放基準。

---

## 2. 核心變更細節與邏輯

### 2.1 疊加式兵種屬性模型
* **檔案位置**：[ModifierForm.Data.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/ModifierForm.Data.cs)
* **優化前**：
  在 `GetBaseStatsForUnit` 中，開啟平衡模式與自訂微調是互斥或直接覆蓋的，玩家不能疊加。
* **優化後**：
  * 開啟平衡模式時，先在原始屬性上套用平衡強化。
  * preset 中實際提供的欄位一律視為使用者的明確覆寫，即使其數值剛好等於原版，也不會被平衡值取代。
  * 舊版或短格式 preset 未提供的欄位，才會繼承目前啟用的平衡層或原版層，避免速度、視野、冷卻與射程被錯誤歸零。

### 2.2 記憶體預檢驗 (Dry Run) 機制
* **檔案位置**：[ModifierForm.Patches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/ModifierForm.Patches.cs)
* **優化前**：
  呼叫 `ApplyClScriptPatch`、`ApplyRessPatch`、`ApplyObjdefPatch` 等方法時，會立即呼叫 `SafeWriteAllBytes` 寫入實體檔案。若中途某個地圖的 `team.dat` 損毀或解壓出錯，會導致部份檔案已被修改，留下不完整的半套變更，且容易造成 I/O 衝突。
* **優化後**：
  * 主要補丁生成方法改為先完成讀取、解壓、驗證與記憶體修改，再回傳待寫入的 `byte[]`：
    * `GetPatchedClScriptBytes`
    * `GetPatchedRessBytes`
    * `GetPatchedObjdefBytes`
    * `GetPatchedTeamDatBytes`
    * `GetPatchedEndlessScripts`
  * EXE 補丁（Focus Loss, Village Setter, Village Build Range）改為傳入 `byte[]` 的記憶體位元組修改方法（透過 `Buffer.BlockCopy`）。
  * 調整套用與還原（`BtnApply_Click`、`BtnRestore_Click`、`BtnRestoreCompat_Click`）流程：
    1. 在記憶體中建立字典 `patchedFiles`。
    2. 在記憶體中生成所有檔案的 `byte[]`。若有任何解壓或長度溢出異常，會在中途被 catch 截獲並安全中止。
    3. 主要檔案預檢驗通過後，批次執行 `SafeWriteAllBytes`；`FileRollbackScope` 涵蓋整個操作，包括後續語言、dgVoodoo 與無盡經濟相容性處理。

  此流程會先預檢主要遊戲資料與 EXE，但語言、dgVoodoo 和無盡經濟相容性處理仍在交易寫入階段執行，因此不是所有功能都屬於無副作用的完整 Dry Run。

### 2.3 cl_script.ini 增量修補與 Radius 累乘防禦
* **檔案位置**：[ModifierForm.Patches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/ModifierForm.Patches.cs)
* **優化前**：
  1. 每次套用都以備份檔重建完整內容，因此不會累乘，但會丟失其它工具加入的非管理設定。
  2. 若改成直接以現有檔案做增量基底，Radius 必須另外固定以備份原始值計算，否則才會產生重複乘算。
* **優化後**：
  1. 優先讀取遊戲目錄現有的 `cl_script.ini` 作為增量修改基底。
  2. 建立官方原版備份中 Radius 設定的對照字典。
  3. 現有檔案必須完整包含備份中的 Radius、CiviDelay 與各陣營士氣設定鍵，否則改用安全備份，避免把損壞或不相容內容當成增量基底。
  4. 修改 Radius 時永遠基於備份對照字典中的原始值縮放；關閉無限士氣時則以「設定名＋陣營」精確還原，避免 HUN、KEL、ROM 被錯誤替換成 GER。

### 2.4 無盡模式腳本安全檢查
* 啟用 AI 終極模式前，會驗證軍隊數量、工作回收旗標、重生延遲、輪詢延遲、同時上場限制與舊版 gate 狀態均屬可辨識版本。
* 只要任一 ENDL 腳本不相容，啟用流程即拋出錯誤並取消整批套用，不會靜默留下部分地圖已修改的狀態。
* 還原流程遇到未知腳本時不覆寫該檔案，但會留下明確警告。

---

## 3. 測試與驗證
* **編譯**：使用 `dotnet build -c Release --no-restore` 驗證。
* **靜態檢查**：使用 `git diff --check` 檢查空白與 patch 格式。
* **分層屬性回歸測試**：以 repo 外的暫存 .NET 8 驗證專案反射呼叫 `MergeUnitStatsLayers`，確認短格式 preset 的缺少欄位會繼承 fallback、明確填入原版數值仍保持覆寫，以及不支援法術半徑的兵種固定為 0。
* **實際資料整合測試**：使用內嵌 `Backup.zip` 與 `遊戲原始檔案` 中 5 個 ENDL 腳本驗證 EXE 往返、Radius 冪等、ress.ini、objdef.dau、70 個 team.dat、士氣陣營還原、損壞 cl_script.ini fallback，以及不相容 ENDL 腳本整批中止。
* **目前限制**：上述回歸工具為 repo 外暫存驗證專案，正式專案仍未納入可持續執行的自動化測試。
