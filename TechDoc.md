# Against Rome Modifier 技術文件

本文件只記錄目前修改器實際支援、已反編譯確認或仍標記為候選的內容。
不使用版本更新流水帳；功能狀態以「穩定」、「候選」、「停用」表示。

## 遊戲與工具路徑

- 實際遊戲安裝路徑由 UI 選擇。已用原版 `C:\Program Files (x86)\Against Rome` 驗證 EXE bytes。
- 本機 Ghidra：`C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\ghidra`
- 本機 JDK：`C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\jdk21`
- Ghidra 專案：`C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\AgainstRomeVillageBuildArea`
- 完整 EXE 函式索引：`re_workspace/ghidra_inventory/against_rome_function_index.csv`
- 完整 Ghidra pseudocode：`re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

`re_workspace/` 是本機反編譯底稿，不提交到 Git，也不發布遊戲原始資產或反編譯輸出。

## 修改器架構

- `Program.cs`：程式入口、DPI、權限設定。
- `GameLZSS.cs`：遊戲 `PFIL@` LZSS 解壓縮與壓縮。
- `TroopConfig.cs`：兵種 ID、分類、欄位索引與平衡規則。
- `ModifierForm.cs`：主 UI 與內嵌技術文件。
- `ModifierForm.Data.cs`：備份載入、目前資料讀取、圖示解析、狀態偵測。
- `ModifierForm.Patches.cs`：所有寫入與還原邏輯。
- `ModifierForm.Presets.cs`：`.arpreset` 與 `.artroop` 匯入匯出。
- `ModifierForm.SaveManager.cs`：存檔備份與還原。
- `docs/reverse-engineering/`：可查證的反編譯證據。
- `data/game_schema.json`：工具可讀的檔案格式、欄位與 patch metadata。

## 支援的遊戲檔案

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
  - 兵種生命、移動、視野、武器傷害、攻速、射程、牧師法術距離、VW、AW。
  - 寫入時必須保持解壓後文字長度不變。
- `SYSTEM/ress.ini`
  - 建造、生產、升級、法術消耗。
  - `[objres]` index `19-24` 是裝備/退款相關欄位，修改器保留原值，避免破壞 UI 裝備扣數與 AI/job 行為。
- `SYSTEM/cl_script.ini`
  - 村民生產速度、法術半徑、士氣參數。
- `MAPS/**/team.dat`
  - `[maxteamobjgenerell]` 與 `[teamdata]` 人口上限。
  - `[teamdata]` 第 6 欄是 `bannerVersion/bver`，不是 AI 開關。
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`
  - 無盡模式 AI Ultimate Mode 的軍隊數量、重生等待、行動迴圈等待、active-AI gate 候選 patch。
- `Against_Rome.exe`
  - 失焦不暫停 patch。
  - 舊村莊建造範圍候選 patch 只保留還原，不再套用。
- `apt.dat`
  - 已確認是 ZIP-like UI/layout container，目前修改器不寫入。

## 目前功能狀態

- 穩定：人口上限、免費建造/生產/升級/法術、兵種屬性、村民速度、法術距離、士氣、語言覆蓋、失焦不暫停、存檔管理、preset。
- 候選：AI Ultimate Mode 的無盡 AI 行為加速與 active-AI gate bypass。已由 BCI/EXE 路徑定位，但仍應以實機遊戲結果驗證每個效果。
- 停用：村莊紅色框框建造範圍 2 倍。舊 EXE patch 位址 `0x1366c4`/`0x1366cd` 只影響 `00536630` 邏輯村莊 bounds，沒有放大畫面紅色虛線框。修改器現在隱藏此選項，且只會還原舊 bytes。

## 反編譯與驗證規則

- 先查 `docs/reverse-engineering/`、`data/game_schema.json`、`re_workspace/ghidra_inventory/`。
- 只有檔案值被 EXE 或 BCI runtime 路徑使用時，才把欄位語意標為 confirmed/stable。
- Ghidra pseudocode 不是原始碼。未知 `FUN_*` 函式在沒有 call-path、字串或 runtime 證據前不得硬命名。
- EXE patch 必須驗證原始 bytes，並且要有還原 bytes。
- 每個寫入都要可從備份或原 bytes 還原。

## 村莊紅框目前結論

已驗證原版安裝檔：

- `0x1366c4`: `C1 E2 06`
- `0x1366cd`: `C1 E1 06`
- `0x136867`: `C1 E0 06`

先前嘗試改成 shift `7` 沒有放大畫面紅框，因此不是正確修正。下一個證據方向是 overlay path：

`0044e990 -> 00421c00 -> 00520320 -> 005203a0`

其中 `0044e990` 會建立 overlay type `0x28`。在沒有證明紅框實際大小來源前，修改器不提供此功能。
