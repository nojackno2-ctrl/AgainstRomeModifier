[English](README.md) | [繁體中文](README.zh-TW.md)

---

# Against Rome 修改器

> [!WARNING]
> 這個修改器還在測試中，如果要使用請將原始的檔案進行備份。
> (This modifier is still in testing. Please backup your original files before using it.)

這是針對即時戰略遊戲《Against Rome》（羅馬帝國：黃金版 / 對抗羅馬）開發的 Windows Forms 修改器。
本專案基於 C# 與 .NET 8 建置。公開的編譯版本不包含原始遊戲資料；若沒有提供選用的本地 `Backup.zip`，修改器會在首次執行時，從使用者選取的遊戲安裝目錄中自動建立還原基準。

## 維護與開發文件

- [`TechDoc.md`](TechDoc.md)：目前的繁體中文技術規格文件（包含 AI Agent 協作檢核清單、除錯歷史、失敗案例以及文末的驗證步驟）。
- [`TechDoc_EN.md`](TechDoc_EN.md)：目前的英文技術規格文件。
- [`docs/reverse-engineering/`](docs/reverse-engineering/README.md)：檔案格式、偏移量、補丁位元組、實證紀錄以及本地 Ghidra 工作流程。

## 專案起源與作者說明

本專案的作者是一位多年前熱愛《Against Rome》的忠實玩家。本修改工具是在 AI Agent 的協助下建立，並作為研究與個人模組化（Modding）專案持續維護。

## 核心功能

- **地圖人口上限解鎖**：修改地圖的 `team.dat` 檔案，啟用後可將人口上限提升至 1600。
- **房屋容量 20 倍**：可還原修改，將 `objdef.dau` 中所有正值人口建築的 `wohnwer` 欄位值提升 20 倍。
- **建造速度 10 倍**：可還原修改，縮短 `objdef.dau` 中的建造、升級與維修時間 10 倍（這會自動提升每秒維修率，已在遊戲內實機驗證）。
- **資源儲存容量 10 倍**：可還原修改，將 `objdef.dau` 中主堡（`Hau`）和倉庫（`Lag`）的資源儲存上限提升 10 倍（已在遊戲內實機驗證）。
- **主堡生命值 10 倍**：可還原修改，將 `objdef.dau` 中所有主堡（`Hau` 建築）的生命值（HP）提升 10 倍（已在遊戲內實機驗證）。
- **無盡模式 AI 終極模式**：拆分為 6 個獨立可選的模組：
  - 將部隊生成數量上限提升至原版腳本的最大限制。
  - 將已完成的軍事增援任務回收，以實現持續不斷的後續攻勢。
  - 將軍事增援的等待時間縮短至 5 秒。
  - 將 4 個非聚落隊伍的撤退期限由 10 分鐘縮短至 5 秒。
  - 保留 2 個聚落清理機制作為後備（維持 10 分鐘），以確保舊村莊與其柵欄在該隊伍插槽被重複使用前清理完畢。
  - 將已確認的「每次僅清理一個物件」的清理頻率從約 1.5 秒加速至每物件 0.1 秒。
  - 將軍事增援的單位門檻從 4 提升至 40，並繞過會阻礙後續波次的「主屋資源」與「臨時領袖/村民」檢查。
  - 將整個增援部隊轉移至村莊，而不是撤退（保留第 4 類聚落、建築、一次一隊與單位數量的安全限制閥）。
  - 無盡模式的聚落模板也會獲得可還原的開局資源加成。
  - 新遊戲可將第 1 類村莊 AI 的上限固定在 4，同時保留獨立的第 4 類軍事聚落作為第 5 個對手。
  - （不安全的全局 `CLAK` 生產修改保持停用狀態）。
- **免費建設與生產**：透過修改 `ress.ini` 實現免費建造、生產、升級與法術消耗。
- **單位屬性自訂編輯**：透過修改 `objdef.dau` 和 `cl_script.ini` 調整生命值（HP）、傷害、防禦（VW/AW）、移動速度、視野、冷卻時間、射程與法術半徑等。
- **兵種預設匯入/匯入與一鍵控制**：支援透過 `.artroop` 檔案匯出/匯入兵種屬性預設，並提供一鍵啟用或停用所有功能的快捷按鈕。
- **失去焦點背景執行**：對 `Against_Rome.exe` 進行補丁，使遊戲在失去焦點（最小化或切換視窗）時仍能繼續執行。
- **全地圖建造限制消除**：修改 `Against_Rome.exe` 的 setter trampoline，免除村莊建設範圍與紅框限制，允許在整張地圖的任何地方進行建造（已在遊戲內實機驗證，包含紅框範圍）。
- **內建 dgVoodoo2 整合**：可選的 dgVoodoo2 整合，直接安裝隨附的 32 位元 D3D8/DirectDraw 包裝外掛，且不會覆寫系統中其他非託管的 DLL。
- **路徑偵測與一鍵啟動**：自動偵測遊戲安裝路徑，並支援在修改器中一鍵啟動遊戲。
- **遊戲存檔管理**：提供存檔備份、還原與歷史紀錄管理功能。
- **內建技術規格文件**：可在程式內直接閱讀相關技術文件。
- **本地逆向工程工作流**：在忽略的 `re_workspace/` 下，保留產生的 Ghidra 函式索引與虛擬碼庫。

## 技術架構

- [`src/Program.cs`](src/Program.cs)：應用程式進入點、DPI 設定與 UAC 系統管理員權限提升。
- [`src/Core/GameLZSS.cs`](src/Core/GameLZSS.cs)：遊戲專用的 PFIL/LZSS 壓縮與解壓縮演算法實作。
- [`src/Core/TroopConfig.cs`](src/Core/TroopConfig.cs)：已知單位 ID、單位分類、屬性欄位索引與平衡規則。
- [`src/Core/Features/`](src/Core/Features/)：以 `FeatureRegistry` 與 `PatchProfile` 管理功能、分類還原、偵測及各檔案補丁規劃。
- [`src/Core/Services/PatchEngine.cs`](src/Core/Services/PatchEngine.cs)：精簡的交易編排器；FoodHealing 與 Endless AI 共用 BCI 快取並由一次 `SaveAll` 落地。
- [`src/UI/ModifierForm.cs`](src/UI/ModifierForm.cs)：主 UI 版面配置與內建技術規格文件檢視器。
- [`src/UI/ModifierForm.Data.cs`](src/UI/ModifierForm.Data.cs)：備份載入、數據檢查、TGA 圖示解析與顯示格式化。
- [`src/UI/ModifierForm.Patches.cs`](src/UI/ModifierForm.Patches.cs)：將集中管理的功能開關轉成 `PatchProfile`，並啟動交易式套用／分類還原。
- [`src/UI/ModifierForm.DgVoodoo.cs`](src/UI/ModifierForm.DgVoodoo.cs)：內建 dgVoodoo2 檔案釋放、託管安裝、衝突偵測與移除邏輯。
- [`src/UI/ModifierForm.SaveManager.cs`](src/UI/ModifierForm.SaveManager.cs)：存檔備份、還原與快取處理。
- [`src/UI/ModifierForm.Presets.cs`](src/UI/ModifierForm.Presets.cs)：一鍵啟用或停用所有修改項目的功能。
- [`src/UI/TroopPresetForm.cs`](src/UI/TroopPresetForm.cs)：兵種屬性預設編輯器。
- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1)：在語言移轉中斷或不完整時，驗證並修復本地語言覆蓋備份基準。
- [`docs/reverse-engineering/`](docs/reverse-engineering/)：結構化的逆向工程筆記。
- [`data/game_schema.json`](data/game_schema.json)：工具可讀取的檔案格式與補丁元數據（Metadata）。

## 內建嵌入資源

- 本地選用的 `Backup.zip` 檔案依設計不會被提交至 GitHub。
- 若可執行檔旁或嵌入資源中存在 `Backup.zip`，它將被載入為還原來源。
- 若不存在 `Backup.zip`，修改器會直接讀取使用者選擇的遊戲目錄中的原始檔案，並在記憶體中建立備份基準。
- `TechDoc.md` 作為內建資源嵌入。
- `TechDoc_EN.md` 作為內建資源嵌入。
- 遊戲資源資料在需要時會以 Code Page 1251 (Windows-1251) 解碼；本專案的所有說明文件均採用 UTF-8 編碼。

## 逆向工程資料

本專案將逆向工程筆記整理在 [`docs/reverse-engineering/`](docs/reverse-engineering/) 目錄下。同時將這些逆向資訊以機器可讀的格式鏡像輸出至 [`data/game_schema.json`](data/game_schema.json)，使未來的 UI 與補丁程式碼能避免使用硬編碼（hardcoded）的索引。

目前涵蓋的範圍包括：

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`：單位屬性與武器屬性欄位。
- `SYSTEM/ress.ini`：建築、生產、升級與法術的消耗資源。
- `SYSTEM/cl_script.ini`：村民生產延遲、法術半徑與士氣參數。
- `MAPS/**/team.dat`：人口上限與 Banner 版本語義。
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`：受約束的無盡模式 AI 終極模式補丁，包含位元組與存檔狀態驗證；長期運行的後期波次迴歸測試仍保留。
- `Against_Rome.exe`：失去焦點背景執行補丁、遊戲內實機驗證的村莊建造範圍解鎖，以及針對已被捨棄的舊版 4 站點建造/紅框補丁的「僅還原」處理，並包含完整的 Ghidra 本地函式清單。

產生的 Ghidra 輸出僅做為本地研究參考，並非原始碼。在沒有明確呼叫路徑或運行期實證支持前，未知的 `FUN_*` 函式將不會被標記為已解析。

## 開發環境

- 開發語言：C# 12
- 目標框架：.NET 8.0 Windows
- 使用介面：Windows Forms
- 目標平台：x64

## 編譯步驟

1. 安裝 .NET 8.0 SDK 與 Visual Studio 2022。
2. 對於公開版本，`Backup.zip` 為選用項目。若您有此檔案，請將其保留在本地。
3. 開啟 `AgainstRomeModifier.slnx` 或 `AgainstRomeModifier.csproj`。
4. 選擇編譯組態為 `Release` 且平台為 `x64`。
5. 進行方案編譯。編譯輸出將位於 `bin/Release/net8.0-windows/` 下。

## 公開版本行為說明

本 GitHub 倉庫不包含遊戲的原始檔案。使用者必須擁有並安裝《Against Rome》正版遊戲，並在修改器中選擇遊戲安裝資料夾。修改器會先使用這些本地檔案作為乾淨的還原基準，然後才套用補丁。

以下內容依設計僅保留於本地，並已列入 `.gitignore`：

- `遊戲原始檔案/`、`Original game archives/`、`Backup.zip`，以及解壓縮後的遊戲樹狀目錄（如 `MAPS/`、`SYSTEM/`、`SAVE/` 和 `ToEng/`）。
- `.codex/`、`.agents/`、`re_workspace/`、建置輸出、IDE 狀態檔、傾印檔（Dumps）、記錄檔（Logs）與私有的審計交接檔案。
- 產生的 Ghidra 分析資料與下載的分析工具鏈。

位於 `tools/re/` 底下的小型可重複分析腳本屬於源始研究材料，依設計會隨專案公開發布。隨附的 dgVoodoo2 檔案亦然：上游的分發條款允許將特定檔案與遊戲或遊戲模組一同發行；詳情請參閱 [`ThirdParty/dgVoodoo2/REDISTRIBUTION.md`](ThirdParty/dgVoodoo2/REDISTRIBUTION.md)。

## 維護工具

- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1)：針對英文語言包覆蓋的備份基準（遊戲資料夾內的 `.against-rome-modifier-language-backup`）進行離線修復。僅在修改器回報語言備份指令清單遺失或損壞時使用。它藉由比對乾淨的原始遊戲樹來重新建立該基準。由於此腳本會直接寫入遊戲安裝目錄，執行前請詳閱腳本內的驗證步驟。預設參數：`-GamePath 'C:\Program Files (x86)\Against Rome'`；原始的乾淨檔案樹會自動在倉庫根目錄下偵測。
- `tools/bcitool.py`：用於讀取與反組譯 `BCI0` 腳本位元組碼的 Python 工具。其內建的 PFIL LZSS 解壓縮器是從 `GameLZSS` 移植而來；若 C# 的壓縮演算法有任何變更，請同步更新此 Python 演算法。
- `tests/verify_split_patches/`：針對無盡模式 AI 補丁的黃金值（Golden values）手動驗證套件。執行此驗證需要本地的 `遊戲原始檔案/` 目錄。在修改任何 P1–P19 補丁常數前後，請務必執行此驗證。

## dgVoodoo2 整合說明

啟用修改器中的 dgVoodoo2 開關並套用變更，即可直接從修改器中釋放並安裝內建的 v2.87.3 封裝檔案，無需網路連接或額外下載。修改器僅會安裝 32 位元的 `D3D8.dll`、`DDraw.dll`、`dgVoodooCpl.exe` 以及 `dgVoodoo.conf`。取消核取該開關並套用，或點選「還原相容性/所有設定」，即可移除修改器所建立的這些檔案。修改器絕不會覆寫玩家自行放置且未受控的現有 DLL，並會保留使用者自行修改的配置。

上游原始碼與分發條款連結：
- [dgVoodoo2 v2.87.3 發行頁面](https://github.com/dege-diosg/dgVoodoo2/releases/tag/v2.87.3)
- [官方分發條款說明](https://dege.fw.hu/dgVoodoo2/ReadmeGeneral/)

## 免責聲明

本修改器僅供學術交流與個人模組化（Modding）研究使用。遊戲的所有智慧財產權均屬於原始版權所有者。請勿散布原始遊戲資源或反編譯後的原始碼。
