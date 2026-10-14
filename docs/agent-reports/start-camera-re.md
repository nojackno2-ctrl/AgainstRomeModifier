# 逆向工程報告：地圖開局相機原生原語與腳本注入證據 (Start Camera Reverse Engineering & Injection)

- 日期：2026-10-14
- 分析者：Antigravity
- 分析目標：Against Rome 遊戲開局相機定位機制、原生函式呼叫協定、實機關卡腳本調用證據及地圖編輯器開局相機功能實作。

---

## 1. 原版遊戲關卡腳本靜態檢測 (Retail Level Script Survey)

我們掃描了遊戲安裝目錄（唯讀檢測 `C:\Program Files (x86)\Against Rome`）中全部 110 個 `.bci` 位元碼檔案（含所有戰役 `MAPS/**/SCRIPT/*.bci`、多人地圖 `MAPS/MP_*`、教學關 `MAPS/TUTOR_*` 及系統核心腳本 `SYSTEM/CLAK/SCRIPT/*.bci`）。

### 核心發現：
1. **`s_lgcSetEnginePos` 與 `s_lgcSetEngineZoom` 在原版關卡中為 0 處呼叫**：
   - 儘管這兩個符號確實註冊在 `Against_Rome.exe` 原生 API 表中，但原版發行關卡腳本中完全沒有呼叫它們。
2. **原版關卡開局相機全部使用 `s_centerToPos(int x, int z)`**：
   - 所有戰役開局（如 `KAMP_002_i`、`KAMP_007_o`、`KAMP_024`）、無盡戰役（`ENDL_000` 至 `ENDL_005`）與多人對戰地圖（`MP_000` 至 `MP_009`）開局時，均使用 `s_centerToPos` 將視角對焦到起始基地或部隊位置。
   - 範例位元碼證據（`KAMP_002_i\SCRIPT\ak_level.bci` 偏移 `0x157e4`）：
     ```
     0x157e4: 66 14816              (OpPushLiteral: 14816)  ; Z 世界座標
     0x157ec: 66 12512              (OpPushLiteral: 12512)  ; X 世界座標
     0x157f4: 128 257               (OpCallNative: <s_centerToPos>)
     0x157fc: 73 -2                 (OpClear: 清除 2 個 stack 字組)
     ```

---

## 2. EXE 原生原語註冊與實作剖析 (Native Executable RE)

針對 `Against_Rome.exe`（SHA-256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，Base `0x400000`）進行反彙編與呼叫追蹤：

### 2.1 `s_centerToPos` (註冊虛擬位址 `0x5200e1`)
- **原生簽名**：`v(ii)`（回傳 void，接收兩個整數）。
- **底層包裝函式**：`0x51fcb0` -> `0x51f120`。
- **參數順序與單位**：
  - VM 調度器（`0x5b1700`）從運算棧頂提取參數：棧頂為第一個參數（Arg0 = X），次層為第二個參數（Arg1 = Z）。因此入棧順序為先推入 Z，後推入 X。
  - **單位：世界座標 (World Coordinates, 0..16383)**，非網格單元格索引 (Cells/Tiles, 0..63)。
  - `0x51f120` 內部首先呼叫 `0x49ba80(x, z)` 取得該座標處的地表高度 `height`（以單元格 64 進行位移與插值取樣）。
  - 接著呼叫 `0x499090(float x, float height, float z)` 計算相機等角投影空間座標：
    ```c
    // 0x499090 計算公式：
    // 0x5f4b4d 為浮點常數 2.0f (HeightScale 高度係數)
    float engineX = x - height * 2.0f;
    float engineY = 0.0f;
    float engineZ = z - height * 2.0f;
    // 寫入全域相機座標
    *(float*)0x7717e8 = engineX;
    *(float*)0x7717ec = engineY;
    *(float*)0x7717f0 = engineZ;
    ```
- **結論**：`s_centerToPos` 是原版引擎專為開局相機對焦設計的高階 API，具有**自動地圖高度取樣與等角投影偏移補償**能力。但它不設定縮放。

### 2.2 `s_lgcSetEnginePos` (註冊虛擬位址 `0x54c8d1`)
- **原生簽名**：`v(ddd)`（回傳 void，接收三個 double）。
- **底層包裝函式**：`0x54c400` -> `0x54c010` -> `0x4989d0`。
- **參數順序與「第三個參數」含義**：
  - 參數 1 (Arg0)：`engineX`（寫入 `0x7717e8`）。
  - 參數 2 (Arg1)：`engineY`（寫入 `0x7717ec`）。**此即 3D 空間中的相機海拔/高度 (Elevation)**。在正常俯視地面時為 `0.0f`（高度偏移已由 X/Z 投影吸收）。
  - 參數 3 (Arg2)：`engineZ`（寫入 `0x7717f0`）。
  - VM 入棧順序：由於 VM 為倒序入棧且 double 佔兩字組，入棧順序為 `Double(z); Double(y); Double(x);`。
  - 呼叫後清理 6 個字組（`argc -6`，即 `73, -6`）。
- **單位**：連續浮點世界尺度（World coordinate units）。

### 2.3 `s_lgcSetEngineZoom` (註冊虛擬位址 `0x54c98e`)
- **原生簽名**：`v(d)`（回傳 void，接收一個 double）。
- **底層包裝函式**：`0x54c620` -> `0x54c1f0` -> `0x498a30`。
- **縮放範圍與夾限**：
  - `0x498a30` 將輸入轉為單精度浮點，若 `< 0` 夾限為 `0.0f`；與 `0x5f4b3d`（浮點數 `9.0f`）比較，若 `> 9.0` 夾限為 `9.0f`。
  - 寫入相機縮放全域變數 `0x771800`，並呼叫 `0x491d80` / `0x491bd0` 更新投影矩陣。
  - BCI 入棧順序：`Double(zoom); 128 <s_lgcSetEngineZoom>; 73, -2;`。

---

## 3. 開局相機功能設計與實作 (Map Start Camera Implementation)

基於上述確鑿證據，我們實作了精準、最小、具備實機語義的「地圖開局相機」功能：

### 3.1 雙原語支援模式 (`StartCameraPrimitive`)
企劃可依需求選擇發射方式：
1. **`CenterToPos` (預設與推薦，原版腳本實機證據)**：
   - 先發射 `s_centerToPos(x, z)`（世界座標 0..16383），讓引擎自動計算地圖地表高程偏移。
   - 隨後發射 `s_lgcSetEngineZoom(zoom)`（原生縮放 0..9，預設 5.0）。
2. **`DirectEnginePos` (直接引擎空間座標)**：
   - 發射 `s_lgcSetEnginePos(x, y, z)`，允許精確覆寫相機海拔 Y。
   - 隨後發射 `s_lgcSetEngineZoom(zoom)`。

### 3.2 腳本注入架構 (`MapStartCameraInjector`)
- 在關卡腳本末端附加專屬的開局相機進入 shim。
- 呼叫相機原語設定後，以 `OpJump (112)` 無條件跳躍返回原本的腳本入口點（`originalMain`）。
- 完全不佔用本地變數框架（無 `OpPushFp`/`OpSetFp`），不發射 `OpReturn (86)`，不影響後續的 `ScenarioSpawns` 或 `ScenarioEvents`。
- 支援原版備份（`ak_level.arm_original`）與 PFIL 壓縮還原/封裝。
- 編輯器中資料保存於地圖目錄的 `arm_start_camera.json`。

### 3.3 企劃 UI 整合 (`MapEditorForm.StartCamera.cs` & `StartCameraDialog`)
- 在主編輯器「配置」(Layouts) 選單中新增「開局相機設定…」功能。
- 支援直接從目前編輯器視圖（2D 畫布中心或 3D 攝影機視角）一鍵讀取當前世界座標（`Capture View`）。
- 支援座標、海拔、縮放數值範圍嚴格檢查（X/Z: 0..16383, Zoom: 0..9）。

---

## 4. 跨 Agent 協作：建議未來由 `src.Shared/Scripting/*` 整合之項目

依據 `AGENTS.md` 規則，本輪未修改由其他 Agent 負責的 `src.Shared/Scripting/*`。建議未來負責該模組的 Agent 進行以下整合：

1. **擴充 `ScenarioDocument`**：
   在 `src.Shared/Scripting/LevelScriptInjector.cs` 的 `ScenarioDocument` 類別中，加入：
   ```csharp
   public MapStartCamera? StartCamera { get; set; }
   ```
2. **在 `LevelScriptInjector.Apply` 內原子合併相機 shim**：
   目前 `MapStartCameraInjector` 以單獨 pass 附加相機 shim 並跳轉至原 main。若在 `LevelScriptInjector.Apply` 內部直接先發射相機呼叫、再依序發射部隊/建築生成 shim，可減少一次位元碼 jump 跳轉，進一步使所有關卡開局注入流程在單一 BCI 編輯交易內完成。
