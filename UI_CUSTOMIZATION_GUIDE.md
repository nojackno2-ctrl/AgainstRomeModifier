# Against Rome 修改器 UI 排版調整與自訂指南 (UI Customization Guide)

本修改器的 UI 採用了**手寫 C# 配合動態排版函式**的架構，雖然不支援 Visual Studio 的設計師拖曳，但它的結構非常清晰有規律。
所有的開關（`ModernToggle`）在卡片內的位置都是**自動垂直等距排列**的。您只需要修改關鍵數字，即可完成精準的版面微調。

---

## 1. 調整表單視窗大小

表單的初始大小與最小大小是在 [ModifierForm.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.cs) 的 `InitializeComponent()` 方法中設定的：

*   **修改檔案**：[ModifierForm.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.cs)
*   **關鍵程式碼**（約第 401-404 行）：
    ```csharp
    this.Size = new Size(1450, 880);          // 修改器的初始寬度與高度
    this.MinimumSize = new Size(1280, 720);   // 修改器允許拉伸的最小尺寸
    this.AutoScrollMinSize = new Size(1450, 880); // 當視窗小於此尺寸時，自動出現滾動條
    ```

---

## 2. 調整卡片面板高度與對齊

主控制台的五張功能卡片高度，以及它們所含有的 Toggle 開關是在 [ModifierForm.Layout.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Layout.cs) 中定義的。

*   **修改檔案**：[ModifierForm.Layout.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Layout.cs)
*   **關鍵程式碼**（在 `ConfigureSystemDashboard()` 方法中，約第 160-184 行）：
    ```csharp
    // 參數格式為：ConfigureSettingsCard(卡片容器, 卡片標題, 卡片高度, 內含的Toggle開關列表...)
    
    ConfigureSettingsCard(pnlNumericCard, lblNumericTitle, 230,   // 高度設為 230
        chkFocusLoss,
        chkToEng,
        chkDgVoodoo);
        
    ConfigureSettingsCard(pnlExperimentalCard, lblExperimentalTitle, 278, // 高度設為 278
        chkGameSpeed,
        chkSpellEnhancement,
        chkGeneralSkills,
        chkLeaderGlory);
    ```
    > [!TIP]
    > **如何微調卡片高度？**
    > 當您往卡片內「新增開關」時，請將高度數值加上 `48`；「移走開關」時，則減去 `48`。這樣可以確保卡片底部不會有過多留白或將開關截斷。

---

## 3. 調整開關在卡片內的垂直間距與樣式

卡片內部所有 Toggle 開關的自動定位公式，定義在 `ConfigureSettingsCard` 底部。

*   **修改檔案**：[ModifierForm.Layout.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Layout.cs)
*   **關鍵程式碼**（在 `ConfigureSettingsCard()` 的內部方法 `LayoutRows()` 中，約第 255-264 行）：
    ```csharp
    void LayoutRows() {
        for (int i = 0; i < toggles.Length; i++) {
            ModernToggle toggle = toggles[i];
            int y = 60 + i * 48; // "60" 是第一個開關的起點高度，"48" 是開關與開關之間的垂直間距
            toggle.Location = new Point(20, y); // "20" 是開關距離卡片左邊邊緣的 padding
            toggle.Size = new Size(Math.Max(120, card.Width - 40), 26);
            toggle.Font = fontJhengHei95R; // 可在此處更改開關所使用的字型或字體大小
            toggle.BackColor = card.BackColor;
        }
    }
    ```
    如果您覺得開關排列太鬆，可以把間距 `48` 改為 `40`；如果覺得太擠，可以調大。

---

## 4. 新增您自己的功能 Toggle 開關 (範例步驟)

如果您未來想在主控制台新增一個全新的修改功能 toggle，只需遵循以下五個步驟：

### 步驟 A. 宣告控制項變數
在 [ModifierForm.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.cs) 的變數宣告區（與其他 `chk...` 放在一起，約第 80-100 行）加入：
```csharp
private ModernToggle chkMyNewFeature = null!;
```

### 步驟 B. 初始化控制項
在 [ModifierForm.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.cs) 的 `InitializeComponent()` 中（與其他 `chk...` 放在一起）進行初始化：
```csharp
chkMyNewFeature = new ModernToggle {
    Text = "我的新功能描述",
    Checked = false,
    BackColor = Color.Transparent,
    Font = fontJhengHei10B
};
// 將其加入對應卡片控制項中，例如加入 pnlNumericCard 系統設定卡片
pnlNumericCard.Controls.Add(chkMyNewFeature);
```

### 步驟 C. 配置版面位置 (Layout)
開啟 [ModifierForm.Layout.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Layout.cs)，在 `ConfigureSystemDashboard()` 中，將 `chkMyNewFeature` 插入到對應卡片方法參數的合適位置。它便會自動加入垂直排列中：
```csharp
ConfigureSettingsCard(pnlNumericCard, lblNumericTitle, 278, // 高度從 230 加 48 變成 278
    chkFocusLoss,
    chkToEng,
    chkDgVoodoo,
    chkMyNewFeature); // 加在這裡！
```

### 步驟 D. 綁定特徵與 UI 對應 (Mapping)
開啟 [ModifierForm.Patches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Patches.cs)，在 `BuildFeatureToggleMap()` 字典中加入您的功能 Id 與 UI 開關對應：
```csharp
featureToggles = new Dictionary<string, ModernToggle>(StringComparer.OrdinalIgnoreCase) {
    // ...
    ["MyNewFeature"] = chkMyNewFeature, // 加在這裡！
};
```
*註：請確保在 `FeatureRegistry.cs` 中也有註冊對應的 `"MyNewFeature"`。這樣修改器在「執行修改」與「讀取現有設定」時，就會全自動處理此開關的狀態與寫入，不需額外寫任何套用代碼！*

### 步驟 E. 語系與 Tooltip 設定
在 [ModifierForm.Localization.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Localization.cs) 中套用在地化文字與說明：
```csharp
chkMyNewFeature.Text = Loc.Get("MyNewFeatureLabel");
myToolTip.SetToolTip(chkMyNewFeature, Loc.Get("MyNewFeatureTip"));
```
然後在 [Localization.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/Localization.cs) 的中英文語言字典中新增對應的 `"MyNewFeatureLabel"` 與 `"MyNewFeatureTip"` 字串即可。
