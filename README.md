# Against Rome Modifier (羅馬的榮耀修改器)

這是一款專為即時戰略遊戲《羅馬的榮耀》（Against Rome）開發的現代化圖形修改器。專案基於 C# (.NET 8) 與 Windows Forms 構建，具備美觀的深色系科技風無邊框介面，並將所有底層修改與還原邏輯整合為單一獨立運行的執行檔。

## 開發初衷與作者的話

> [!NOTE]
> 本專案的作者是一位在 20 年前深愛《羅馬的榮耀》的忠實玩家。出於對這款遊戲的熱愛，在完全不具備軟體開發背景的情況下，藉由 **AI 代理人 (AI Agent)** 的協助完成了這款修改器的開發。
>
> 本專案內的所有程式碼與架構設計均由 AI 代理人撰寫與建構。若您有任何建議、反饋或修改想法，非常歡迎來信討論！後續的任何功能調整與優化，作者也將持續透過 AI 代理人來進行修改與維護。

---

## 核心功能特色

* **人口上限突破**：可將無盡模式與多人模式的人口上限從原版 200 人大幅提升至 1600 人（或自訂上限）。
* **全建築與生產免費**：清除所有建築物建造、科技升級、兵種與攻城機具生產的資源及榮譽點數消耗。
* **回血食物消耗優化**：為解決生產免費後的平衡問題，加入單位回血時的食物消耗機制，並顯著提升回血效率與消耗比率。
* **自訂兵種屬性檔案自訂與特色平衡**：
  * **9 大屬性全面開放自訂**：支援完全自訂所有 43 個兵種（含攻城武器）的 9 大屬性（生命、傷害、防禦、戰鬥、移動速度、視野、冷卻、射程、法術半徑）。
  * **陣營分類分頁與拉桿消除**：自訂編輯子視窗 `TroopPresetForm` 優化放大至 1250x790，採圓角無邊框美學，並以「條頓、塞爾特、匈奴、羅馬」四個陣營分頁呈現，徹底消除所有水平與垂直拉桿。
  * **獨立檔案存取 (.artroop)**：支援自訂兵種屬性的匯出與匯入，且與修改器全域設定檔 (`.arpreset`) 完美整合，支援舊設定檔自動補齊的向下相容性。
  * **當前屬性來源顯示**：主介面新增 `lblTroopPresetFile` 狀態標籤，即時高亮顯示當前套用的兵種屬性檔案來源。
  * **自訂兵種特色與倍率**：部隊移動速度加倍，遠程單位射程與視野提升 3 倍、裝填速度提升 1.5 倍，法師視野與施法距離提升 30 倍。
* **相容性與便利性修正**：
  * **防止失焦暫停**：修改遊戲主程式（以 NOP 阻斷 Flag 寫入），使遊戲在視窗失去焦點或最小化時仍能持續在背景執行。
  * **自動路徑偵測**：啟動時自動偵測登錄檔中《羅馬的榮耀》的安裝路徑。
  * **一鍵啟動遊戲**：修改完成後，可直接在修改器中一鍵開啟遊戲。
* **偏好設定匯入/匯出**：支援將您的自訂修改參數匯出為 `.arpreset` 檔案，方便隨時備份與載入。
* **存檔管理器**：提供遊戲存檔（SAVE）的備份、還原與歷史管理功能，並導入快取機制以維持介面流暢。

---

## 技術架構與專案結構

專案採用模組化結構設計，各檔案分工明確：

* **[Program.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/Program.cs)**：程式入口點，處理系統高 DPI 自適應與 UAC 管理員權限提升。
* **[GameLZSS.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/GameLZSS.cs)**：針對遊戲特化之高效 LZSS 壓縮與解壓縮核心演算法，採用雜湊鏈（Hash Chains）與 16-bit 雜湊表，大幅優化大檔案壓縮速度。
* **[TroopConfig.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/TroopConfig.cs)**：儲存兵種對應資料、排序規則、屬性矩陣與特色加成計算邏輯。
* **[ModifierForm.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.cs)**：主表單介面佈局、繪圖控制與自訂 `ModernToggle` 元件之 UI 宣告。
* **[ModifierForm.Data.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Data.cs)**：載入並分析現有遊戲數據、處理 TGA 圖像解析與數據格式化。
* **[ModifierForm.Patches.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Patches.cs)**：核心修改邏輯，以多執行緒背景套用/還原各種補丁（`objdef.dau`、`ress.ini`、`cl_script.ini`、`Against_Rome.exe` 及平行處理 `team.dat`）。
* **[ModifierForm.SaveManager.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.SaveManager.cs)**：處理遊戲存檔的壓縮備份與快取管理。
* **[ModifierForm.Presets.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Presets.cs)**：實作 Preset 自訂預設檔的讀寫與 UI 控件連動。
* **[TroopPresetForm.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/TroopPresetForm.cs)**：新增的自訂兵種屬性檔案編輯子視窗，實作了 1250x790 圓角無邊框版面、陣營 TabControl 分頁與無拉桿顯示。

---

## 內嵌資源與獨立運行機制

本專案完全擺脫對外部備份資料夾的依賴，實現了「單一執行檔獨立運行」：
1. **內嵌原始資源 (`Backup.zip`)**：將遊戲初始的乾淨設定檔與主程式（如 `objdef.dau`、`team.dat`、`Against_Rome.exe` 等）打包，在編譯時作為 `EmbeddedResource` 嵌入至執行檔中。
2. **動態釋放與清理**：修改器啟動時會自動解壓縮內嵌的 `Backup.zip` 到系統臨時目錄（Temp）的隨機資料夾中作為修改基準；在程式關閉時，會自動完整刪除該臨時資料夾，保證系統乾淨。
3. **技術文件內嵌**：[修改技術文件.md](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/修改技術文件.md) 同步內嵌於執行檔中作為內置說明書。

---

## 開發與編譯環境

* **開發語言**：C# 12
* **目標框架**：.NET 8.0-windows
* **專案類型**：Windows Forms (WinExe)
* **支援平台**：Windows (x64)

### 編譯步驟
1. 確保已安裝 .NET 8.0 SDK 與 Visual Studio 2022。
2. 確保專案根目錄下存在 `Backup.zip` 檔案（此為編譯所需的 `EmbeddedResource`）。
3. 使用 Visual Studio 開啟 `AgainstRomeModifier.slnx` 或專案檔。
4. 選擇 `Release` / `x64` 設定。
5. 點擊「建置方案」即可在產出的 `bin/Release/net8.0-windows/` 下取得單一執行檔。

---

## 授權與聲明

本修改器僅供學術交流與程式設計研究之用。遊戲智慧財產權歸原開發商所有。

---

# Against Rome Modifier

This is a modern graphical modifier developed specifically for the real-time strategy game *Against Rome*. The project is built on C# (.NET 8) and Windows Forms, featuring a beautiful dark-themed tech-style borderless interface, and integrating all underlying modification and restoration logic into a single, standalone executable.

## Project Origin & Author's Note

> [!NOTE]
> The author of this project is a devoted player who loved *Against Rome* 20 years ago. Out of deep passion for the game, the author created this program with the assistance of an **AI Agent**, without having any prior programming background.
>
> All code and architectural decisions in this project were generated and written by the AI agent. If you have any suggestions, feedback, or ideas for modification, please feel free to reach out for discussion! Any future updates or adjustments will also be implemented through the AI agent.

---

## Core Features

* **Break Population Limits**: Dramatically increase the population limit in endless and multiplayer modes from the original 200 to 1600 (or custom limits).
* **Free Construction & Production**: Eliminate all resource and honor point costs for building construction, technology upgrades, unit training, and siege weapon production.
* **Healing Food Consumption Optimization**: To address balance issues after eliminating production costs, a food consumption mechanism has been introduced for units when healing, significantly boosting healing efficiency and consumption ratios.
* **Troop Stats Configuration File Customization & Balance**:
  * **9 Stats Customization**: Fully customize HP, Dmg, VW, AW, Speed, Sight, Relt, Range, and SpellRadius for all 43 units.
  * **UI Tabbed Layout & Zero Scrollbars**: The preset child form is enlarged to 1250x790 and restructured into Teutons, Celts, Huns, and Romans tab pages to completely eliminate scrollbars.
  * **Independent Storage (.artroop)**: Support importing/exporting customized stats in `.artroop` format, fully integrated with global presets (`.arpreset`) and backward-compatible with 4-stats configurations.
  * **Source Labeling**: Main UI features a status label (`lblTroopPresetFile`) showing the active configuration file.
  * **Custom Faction Specials**: Doubled movement speed; tripled range and vision for ranged units with 1.5x reload speed; wizard vision and spell range increased by 30x.
* **Compatibility & Convenience Patches**:
  * **Background Execution**: Patches the main game executable (using NOP to block the pause flag write) so the game continues running in the background when minimized or losing focus.
  * **Auto Path Detection**: Automatically detects the installation path of *Against Rome* from the registry upon startup.
  * **One-Click Launch**: Launch the game directly from the modifier with a single click after applying modifications.
* **Import/Export Preferences**: Support exporting your custom configuration parameters into an `.arpreset` file for easy backup and quick loading.
* **Save Manager**: Provide backup, restore, and history management functions for game saves (SAVE) with a caching mechanism to maintain fluid UI response.

---

## Technical Architecture & Project Structure

The project is designed with a modular structure:

* **[Program.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/Program.cs)**: Application entry point, handling high DPI adaptation and UAC administrator privilege escalation.
* **[GameLZSS.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/GameLZSS.cs)**: Highly optimized LZSS compression/decompression core algorithm specialized for the game, utilizing Hash Chains and a 16-bit hash table to speed up large file compression.
* **[TroopConfig.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/TroopConfig.cs)**: Store unit correspondence data, sorting rules, attribute matrices, and faction bonus calculation logic.
* **[ModifierForm.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.cs)**: Main form layout, paint overrides, and UI declarations for custom `ModernToggle` controls.
* **[ModifierForm.Data.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Data.cs)**: Load and analyze game data, handle TGA image parsing, and format unit parameters.
* **[ModifierForm.Patches.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Patches.cs)**: Core patching logic applying and restoring various configurations (`objdef.dau`, `ress.ini`, `cl_script.ini`, `Against_Rome.exe`, and parallel processing for `team.dat`) via multi-threaded background tasks.
* **[ModifierForm.SaveManager.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.SaveManager.cs)**: Compress, backup, and cache game saves.
* **[ModifierForm.Presets.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/ModifierForm.Presets.cs)**: Import/export configuration presets and interface data binding.
* **[TroopPresetForm.cs](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/TroopPresetForm.cs)**: Stand-alone preset child form with 1250x790 layout, faction TabControl, and scrollbar elimination.

---

## Embedded Resources & Standalone Mechanism

The project runs completely independently of external backup directories:
1. **Embedded Original Resources (`Backup.zip`)**: Package the initial game files and executable (e.g. `objdef.dau`, `team.dat`, `Against_Rome.exe`) as an `EmbeddedResource` built directly into the modifier executable.
2. **Dynamic Extraction & Cleanup**: Upon startup, the modifier automatically decompresses the embedded `Backup.zip` into a random folder in the system Temp directory as a modification base; when closed, it deletes the folder automatically to keep the system clean.
3. **Embedded Technical Document**: [修改技術文件.md](file:///c:/Users/nojac/OneDrive/程式/Against_Rome_Modifier/修改技術文件.md) (and its English version) is embedded to serve as an in-app documentation.

---

## Development & Compilation Environment

* **Language**: C# 12
* **Target Framework**: .NET 8.0-windows
* **Project Type**: Windows Forms (WinExe)
* **Platform**: Windows (x64)

### Compilation Steps
1. Install .NET 8.0 SDK and Visual Studio 2022.
2. Ensure the `Backup.zip` file exists in the project root directory (required as `EmbeddedResource` for compiling).
3. Open `AgainstRomeModifier.slnx` or the project file using Visual Studio.
4. Select `Release` / `x64` configuration.
5. Click "Build Solution" to get the standalone executable under `bin/Release/net8.0-windows/`.

---

## License & Disclaimers

This modifier is developed for academic exchange and programming research purposes only. The intellectual property rights of the game belong to the original game developer.

