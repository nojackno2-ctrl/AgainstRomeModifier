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
- **無盡模式羅馬陣營**：在「資源與戰鬥升級」啟用後，會將五張無盡地圖的 team 0 預設陣營改為羅馬，並安全修改無盡專用的 `dlg_volk` 選擇流程，讓三面部族旗都以羅馬開局。此功能已實機驗證，只影響新開局；羅馬依原版設計沒有祭司與榮耀技能樹。
- **羅馬增援完全移交**：獨立功能，讓羅馬增援的士兵、馱馬與平民抵達後全部解除增援隊 script mode 並移交村莊，不再依單位型別分流撤退。底層將 P8 門檻、P9 零配額／全單位 fall-through 及 release helper 原子套用；舊 `EndlessAi.M6`、`jnz+92` 分流、direct-mark 與誤用 opcode 160 的狀態都會自動遷移至 `jz+0` 加正確 opcode-120 helper。五圖靜態／整合測試及新開無盡局實機驗證均已通過，羅馬增援會完整移交村莊且不再撤退。
- **村莊駐軍配額 3 倍（實驗）**：獨立且可還原地修改 `Dorfverteidigung.bci`，將四類 ImportantPos 動態算出的部隊配額各乘以 3，保留原本比例與 0 值。此功能與 AI Ultimate M1 的「每支新部隊 20 人」互相獨立。靜態簽章、套用與精確還原已驗證；新開局實機與效能仍待確認，因此不納入「所有功能開啟」。
- **房屋容量 20 倍**：可還原修改，將 `objdef.dau` 中所有正值人口建築的 `wohnwer` 欄位值提升 20 倍。
- **住宅帳篷一次生產 20 名村民**：可還原、僅影響玩家的 EXE 補丁；男／女生產鈕點一次即可排入最多 20 名村民，已在遊戲內實機驗證。
- **部隊轉換一次選滿 20 名**：可還原、僅影響玩家的 EXE 補丁；村民轉部隊／裝備時點一次即可把選取數設為 20，仍受原版上限與可用村民數量限制，已在遊戲內實機驗證。
- **屍體保留量提高（實驗）**：可還原 EXE 補丁，將引擎開始回收最早死亡物件前保留的空槽由 500 降為 50，讓戰場可多保留約 450 個物件；不會完全停用回收，因為固定的 14,000 格物件池仍需給新單位、投射物與存檔流程使用。長時間大型戰役的效能與穩定性仍待遊戲內驗證，因此不納入「所有功能開啟」。
- **建造速度 10 倍**：可還原修改，縮短 `objdef.dau` 中的建造、升級與維修時間 10 倍（這會自動提升每秒維修率，已在遊戲內實機驗證）。
- **資源儲存容量 10 倍**：可還原修改，將 `objdef.dau` 中主堡（`Hau`）和倉庫（`Lag`）的資源儲存上限提升 10 倍（已在遊戲內實機驗證）。
- **主堡生命值 10 倍**：可還原修改，將 `objdef.dau` 中所有主堡（`Hau` 建築）的生命值（HP）提升 10 倍（已在遊戲內實機驗證）。
- **無盡模式 AI 終極模式**：可還原的五張 `ENDL_*` 地圖 BCI 補丁。現行安全組態是每支軍事增援隊 20 人、重生等待 5 秒、同時活躍隊伍上限 8；保留原始迴圈節奏與兩個聚落清理保護。過去會跳過安全閘的無條件繞過方案已否決且不會再寫入。實證、舊版遷移與 runtime 限制見 [`endless-mode-ai.md`](docs/reverse-engineering/endless-mode-ai.md)。
- **免費建設與生產**：透過修改 `ress.ini` 實現免費建造、生產、升級與法術消耗。
- **單位屬性自訂編輯**：透過 `objdef.dau` 僅調整生命值（HP）、傷害、防禦（VW/AW）、視野與冷卻時間。移動速度、攻擊射程、法術半徑與祭司視野／施法距離由獨立的實驗性功能專責，絕不與自訂兵種層混用。
- **所有單位全地圖視野**：將所有支援單位的 `Sirad` 設為 30000，敵我全陣營一體適用；全地圖視野效果已通過遊戲內實機驗證。
- **其他屬性功能**：遠程射程 3 倍（內建命中修正：落點傷害半徑加倍＋預判散布歸零，避免拉遠射程後打不中）、單位移速 2 倍、祭司全地圖施法距離、法術效果半徑 3 倍與拋射彈道增高。
- **高解析度置中（4:3）**：原生 1920×1080 EXE 實驗已因全螢幕／視窗化皆只繪製左上舊 surface 而停用。此選項會還原所有實驗位址並保留原版 1600×1200；全螢幕置中、視窗化正常比例皆已實測。不拉伸，也不增加 16:9 世界視野。
- **攝影機拉遠 +1（實驗性）**：使用遊戲第一個實際有效的原生縮放級距顯示更大戰場。0.5 已實測沒有可見作用，因此會自動遷移回 +1；代價是世界中的單位、血條與資訊也會一起縮小。選取、邊緣捲動、霧區、地圖邊界及任務相容性仍待重新驗證。
- **兵種預設匯出／匯入與一鍵控制**：`.artroop` 新檔格式固定為六欄 `HP,Dmg,VW,AW,Sight,Relt`。舊九欄檔仍可讀取，但已移除的欄位一定會捨棄；一鍵「所有功能開啟」包含已實機驗證的高解析度置中與所有單位全地圖視野。
- **失去焦點背景執行**：對 `Against_Rome.exe` 進行補丁，使遊戲在失去焦點（最小化或切換視窗）時仍能繼續執行。
- **全地圖建造限制消除**：修改 `Against_Rome.exe` 的 setter trampoline，免除村莊建設範圍與紅框限制，允許在整張地圖的任何地方進行建造（已在遊戲內實機驗證，包含紅框範圍）。
- **內建 dgVoodoo2 整合**：可選的 dgVoodoo2 整合，直接安裝隨附的 32 位元 D3D8/DirectDraw 包裝外掛，且不會覆寫系統中其他非託管的 DLL。
- **路徑偵測與一鍵啟動**：自動偵測遊戲安裝路徑，並支援在修改器中一鍵啟動遊戲。
- **遊戲存檔管理**：提供存檔備份、還原與歷史紀錄管理功能。
- **內建技術規格文件**：可在程式內直接閱讀相關技術文件。
- **本地逆向工程工作流**：在忽略的 `re_workspace/` 下，保留產生的 Ghidra 函式索引與虛擬碼庫。

## 使用無盡模式羅馬陣營

1. 在修改器選擇包含 `Against_Rome.exe` 的遊戲根目錄。
2. 勾選「資源與戰鬥升級」卡片中的「無盡模式羅馬陣營」，再按「執行修改」；「一鍵全開」也會啟用它。
3. 啟動遊戲並開啟任一無盡模式地圖，建立**新**對局。無盡模式的三面部族旗不論選哪面都會進入羅馬；旗幟高亮可能不顯示選中，屬正常視覺限制。
4. 要還原原版，取消勾選後再次執行修改或使用還原功能。既有存檔保留其內嵌 team 資料，不會被改成或改回羅馬。

選項畫面的 `swi_volk` 若由玩家手動切換，仍可主動改回蠻族；這是原版另一條設定路徑，不在此功能的強制範圍內。

## 技術架構

方案（`AgainstRomeModifier.slnx`）已拆解為多個獨立專案，各自產出執行檔或函式庫：

- [`src.Launcher/`](src.Launcher/)：`AgainstRomeLauncher.exe` — 套件入口，負責啟動其他工具並內建技術文件檢視器（`TechDocForm`）。
- [`src.Modifier/`](src.Modifier/)：`AgainstRomeModifier.exe` — 修改器主介面。`ModifierForm` partial 類別涵蓋版面配置、數據檢查（備份載入、TGA 圖示解析）、補丁套用（功能開關 → `PatchProfile` → 交易式套用／分類還原）、一鍵預設，以及兵種屬性預設編輯器（`TroopPresetForm`）。
- [`src.SaveManager/`](src.SaveManager/)：`AgainstRomeSaveManager.exe` — 存檔備份、還原與預覽（`SaveManagerForm`）。
- [`src.MapEditor/`](src.MapEditor/)：`AgainstRomeMapEditor.exe` — 地圖編輯器，含無盡地圖複製／刪除管理。
- [`src.Core/`](src.Core/)：`AgainstRome.Core.dll` — 修改器共用核心：以 `FeatureRegistry` 管理的功能定義（`Core/Features/`）、各檔案補丁器（`Core/Patches/`）、交易式服務如 [`PatchEngine.cs`](src.Core/Core/Services/PatchEngine.cs)（FoodHealing 與 Endless AI 共用 BCI 快取並由一次 `SaveAll` 落地）、[`TroopConfig.cs`](src.Core/Core/TroopConfig.cs)（已知單位 ID、分類、欄位索引與平衡規則）、在地化字串，以及內嵌的 `Backup.zip` 與 dgVoodoo2 資源。
- [`src.Shared/`](src.Shared/)：`AgainstRome.Shared.dll` — 最底層共用函式庫：[`GameLZSS.cs`](src.Shared/Core/GameLZSS.cs)（遊戲專用 PFIL/LZSS 壓縮）、`SafeFileWriter`、`FileRollbackScope`，以及地圖目錄／複製／刪除服務（`Maps/`）。
- [`tools/publish.ps1`](tools/publish.ps1)：發佈全部四個執行檔並打包成釋出 ZIP。
- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1)：在語言移轉中斷或不完整時，驗證並修復本地語言覆蓋備份基準。
- [`docs/reverse-engineering/`](docs/reverse-engineering/)：結構化的逆向工程筆記。
- [`data/game_schema.json`](data/game_schema.json)：工具可讀取的檔案格式與補丁元數據（Metadata）。

## 內建嵌入資源

- 本地選用的 `Backup.zip` 檔案依設計不會被提交至 GitHub。
- 若可執行檔旁存在 `Backup.zip`，或 `AgainstRome.Core.dll` 內嵌有此資源，它將被載入為還原來源。
- 若不存在 `Backup.zip`，修改器會直接讀取使用者選擇的遊戲目錄中的原始檔案，並在記憶體中建立備份基準。
- `TechDoc.md` 與 `TechDoc_EN.md` 內嵌於啟動器，供技術文件檢視器使用。
- 遊戲資源資料在需要時會以 Code Page 1251 (Windows-1251) 解碼；本專案的所有說明文件均採用 UTF-8 編碼。

## 逆向工程資料

本專案將逆向工程筆記整理在 [`docs/reverse-engineering/`](docs/reverse-engineering/) 目錄下。同時將這些逆向資訊以機器可讀的格式鏡像輸出至 [`data/game_schema.json`](data/game_schema.json)，使未來的 UI 與補丁程式碼能避免使用硬編碼（hardcoded）的索引。

目前涵蓋的範圍包括：

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`：單位屬性與武器屬性欄位。
- `SYSTEM/ress.ini`：建築、生產、升級與法術的消耗資源。
- `SYSTEM/cl_script.ini`：村民生產延遲、法術半徑與士氣參數。
- `MAPS/**/team.dat`：人口上限、玩家 faction 與 Banner 版本語義；無盡羅馬功能只將 `ENDL_*` 的 team 0 faction 改為 `ROM`。
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`：受約束的無盡模式 AI 終極模式補丁；因 runtime 只有 20 個 NPC-job slots，同時活躍隊伍上限固定為 8。
- `Against_Rome.exe`：失去焦點背景執行補丁、已實機驗證的村莊建造範圍與無盡模式羅馬陣營解鎖，以及針對已被捨棄的舊版 4 站點建造/紅框補丁的「僅還原」處理，並包含完整的 Ghidra 本地函式清單。

產生的 Ghidra 輸出僅做為本地研究參考，並非原始碼。在沒有明確呼叫路徑或運行期實證支持前，未知的 `FUN_*` 函式將不會被標記為已解析。

## 開發環境

- 開發語言：C# 12
- 目標框架：.NET 8.0 Windows
- 使用介面：Windows Forms
- 目標平台：x64

## 編譯步驟

1. 安裝 .NET 8.0 SDK 與 Visual Studio 2022。
2. 對於公開版本，`Backup.zip` 為選用項目。若您有此檔案，請將其保留在本地。
3. 開啟 `AgainstRomeModifier.slnx`。
4. 選擇編譯組態為 `Release` 且平台為 `x64`。
5. 進行方案編譯。各程式的輸出位於各自專案資料夾下，例如 `src.Modifier/bin/Release/net8.0-windows/`。若要產生包含全部四個執行檔的釋出包，執行 `tools/publish.ps1`。

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
