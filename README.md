# Against Rome Modifier (羅馬的榮耀修改器)

這是一款專為即時戰略遊戲《羅馬的榮耀》（Against Rome）開發的現代化圖形修改器。專案基於 C# (.NET 8) 與 Windows Forms 構建，具備美觀的深色系科技風無邊框介面，並將所有底層修改與還原邏輯整合為單一獨立運行的執行檔。

---

## 核心功能特色

* **人口上限突破**：可將無盡模式與多人模式的人口上限從原版 200 人大幅提升至 1600 人（或自訂上限）。
* **全建築與生產免費**：清除所有建築物建造、科技升級、兵種與攻城機具生產的資源及榮譽點數消耗。
* **回血食物消耗優化**：為解決生產免費後的平衡問題，加入單位回血時的食物消耗機制，並顯著提升回血效率與消耗比率。
* **兵種與陣營特色平衡**：
  * 全面優化 43 個兵種（含攻城武器）的基礎生命、防禦、戰鬥與傷害。
  * 提供羅馬、條頓、塞爾特、匈奴四大陣營的獨特屬性加成與兵種特化。
  * 部隊移動速度加倍，遠程單位射程與視野提升 3 倍、裝填速度提升 1.5 倍，法師視野與施法距離提升 30 倍。
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
