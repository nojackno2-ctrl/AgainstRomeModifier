# Against Rome 地圖編輯器 — 3D 遊戲視角檢視 實作交接規格書

> 本文件是自足的實作規格：實作者只需要本文件 + 本 repo 即可完成開發，不需要其他對話上下文。
> 語言慣例：說明用繁體中文，程式識別字／檔名／技術名詞保留英文。
> 前置閱讀（硬性）：`AGENTS.md`、`AI_HANDOFF.md`、`docs/map-editor-spec.md`、`docs/reverse-engineering/map-formats.md`。

---

## 0. 任務摘要

把地圖編輯器（`src.MapEditor/AgainstRomeMapEditor.csproj`）的中央顯示，從目前的 2D 俯視畫布升級為**遊戲同款斜視角的 3D 地形場景**，讓使用者「像在遊戲實機畫面裡」繪製地表材質。

**需求方已確認的決策（不可更改）：**

1. 目標是「遊戲視角風格的 3D 地形檢視 + 直接在 3D 場景上編輯材質」，**不是** 100% 實機畫面複製（原因見 §1.2）。
2. 3D 渲染採 **OpenTK（OpenGL）**，不用純 GDI+ 軟體投影（效能不可行）。
3. 現有 2D 畫布 `MapCanvasControl` **必須保留**，作為 GL 初始化失敗或 `floortex.dat` 缺失時的 fallback，並提供工具列 2D／3D 切換。
4. 編輯資料層完全不變：仍只寫入已驗證的 `boden.txt` 64×64 材質表與 Phase 1 已開放的欄位。**不新增任何遊戲檔案的寫入能力。**
5. 逆向 `.alr` 3D 模型格式**不在本任務範圍**（見 §6 Phase D 說明）。

---

## 1. 背景與可行性邊界

### 1.1 現況

中央畫布是 `src.MapEditor/MapCanvasControl.cs`（GDI+，約 400 行），已用真實遊戲素材做 2D 俯視合成：

- 每 tile 真實地表貼圖：`floortex.dat`（實為 ZIP，3,005 entry，`SYSTEM/DATA/FLOORTEXTURE/*.bmp`，128×128 8-bit）依 `boden.txt` 的 64×64 材質名繪製。讀取封裝在 `src.MapEditor/FloorTextureLibrary.cs`。
- 地勢：`boden.bmp`（257×257 灰階）梯度 hill-shading（`BuildHeightShade`）。
- 水面：`boden.ini` 的 `Waterlevel / Heightmapstep` 門檻 + `WaterColor` 半透明遮罩（`BuildWaterOverlay`）。
- 場景物件：`src.Shared/Maps/SdlSceneCatalog.cs` 解析 SDL，依「64 世界單位 = 1 地圖像素」畫成隊伍色 2D 符號。
- 互動：滾輪縮放、中鍵平移、左鍵筆刷、右鍵取樣；事件 `TexturePainted` / `TileHovered` / `TextureSampled` 由 `MapEditorForm` 接手（undo/redo、dirty、儲存都在表單層）。

### 1.2 為什麼不做 100% 實機畫面（已調查，不要重查）

`docs/reverse-engineering/map-formats.md` 已記錄：

1. **不能嵌入遊戲本體**：`Against_Rome.exe` 無載入指定地圖的命令列參數、無視窗嵌入介面；EXE 內部 `TextureEditor V0.1`（模式 9）無正常進入路徑，不是安全預覽入口。
2. **3D 模型未逆向**：建築／單位模型在 `alr.dat`（ZIP，2,075 個 `.alr`），格式未知。
3. **高度語意僅部分證實**：`boden.bmp` 灰階與地形起伏目視吻合，**已被授權作唯讀顯示**；但世界高度單位未量測、`vertex.bmp` 通道語意未解。因此 3D 檢視的垂直比例是「顯示近似」，UI 不得宣稱為精確遊戲地形。

### 1.3 已驗證、可直接使用的事實（來源：map-formats.md）

- 格網關係：`boden.txt` 64×64 tile；`Heightmapstep`=4；64×4+1=257 = `boden.bmp`/`vertex.bmp` 頂點格；64×4=256 = `minimap.bmp`/`collision.bmp` tile 格。
- 世界座標：16,384 世界單位 = 256 地圖像素 → **64 世界單位／地圖像素**；一個 tile = 4 地圖像素 = **256 世界單位**。SDL 物件世界座標 = `[settlement] refpos` + `[objectNNNN] pos`。
- `floortex.dat`／`shad.dat`（2,674 entry，圖示／陰影）／`alr.dat`／`apt.dat` 皆為 ZIP 容器，只從使用者選定的遊戲資料夾唯讀載入，**不得加入版本庫**。

---

## 2. 架構設計

### 2.1 新增相依

`src.MapEditor/AgainstRomeMapEditor.csproj` 加入 NuGet：

- `OpenTK`（4.x）
- `OpenTK.WinForms`（提供 `GLControl`，for net8.0-windows）

僅 MapEditor 專案引用；修改器主專案與 Shared 不得新增相依。CI 是 windows-latest **無 GPU**，所以：build 會過，但任何測試**不得建立 GL context**（見 §7 的可測性切割）。

### 2.2 新類別配置（全部在 `src.MapEditor/`）

| 類別 | 職責 |
|---|---|
| `Map3DViewControl` | 繼承 `GLControl` 的 3D 場景控制項。只負責 GL 資源、繪製迴圈、輸入轉發。 |
| `TerrainMeshBuilder`（純邏輯，無 GL 相依） | 從高度圖樣本 + Heightmapstep 產生頂點陣列（位置／法線／UV）與索引。可單元測試。 |
| `TerrainHeightField`（純邏輯） | 包裝 `boden.bmp` 灰階取樣：`float SampleHeight(float tileX, float tileY)`（雙線性內插）。可單元測試。 |
| `TerrainRayPicker`（純邏輯） | 滑鼠射線 ↔ 高度場求交，回傳 tile (x, y)。演算法：ray marching 沿射線步進（步長 ≤ 0.25 tile），跨越地表時二分收斂；可單元測試。 |
| `FloorTextureAtlas` | 把本圖用到的（去重後）128×128 地表 BMP 打包成一張 atlas 貼圖 + 每材質名的 UV rect；每 tile 網格的 UV 指到對應 rect。單格改材質時只更新該 tile 的 UV buffer 區段，不重建 atlas。 |
| `SceneObjectRenderer` | SDL 物件 3D 標記（隊伍色柱／方塊／菱形，種類對應現有 2D 符號），Y 座標取地形高度。 |
| `EditorCamera`（純邏輯） | 斜視角攝影機：yaw／pitch（pitch 限制約 20°–80°）、對焦點平移、距離縮放；輸出 view/projection 矩陣。可單元測試。 |

事件契約沿用 `MapCanvasControl.cs` 底部既有的 `TexturePaintEventArgs` / `TileHoverEventArgs` / `TextureSampleEventArgs`（同一組 class，`Map3DViewControl` 直接 raise 同型別事件），讓 `MapEditorForm` 的筆刷／undo／dirty 邏輯**零修改**接上。

### 2.3 MapEditorForm 整合

- 工具列（`tools` ToolStrip）新增互斥切換鈕：「2D 俯視」／「3D 場景」（預設可為 3D，GL 失敗自動退回 2D 並在 `_modeBanner` 說明）。
- `canvasHost` 同時容納 `_canvas`（既有）與新 `_view3d`，以 `Visible` 切換；兩者共用同一份 `BodenTexturesDocument` 狀態 —— 表單在 `PaintTexture`／`Undo`／`Redo`／`ResetTerrain` 已呼叫 `_canvas.SetTexture(x,y,t)`，同步加呼叫 `_view3d.SetTexture(x,y,t)` 即可。
- `LoadEditingScene()` 載入圖時同時初始化兩個 view（3D 端輸入：dimension、textures、baseline、地圖資料夾、floortex 路徑、SDL 物件、waterLevel、heightMapStep、waterColor —— 與 `MapCanvasControl.LoadTextures` 相同簽章為宜）。
- 既有右下角 `地圖概覽`（minimap PictureBox）保留不動。
- 「選用：啟動遊戲測試」按鈕保留 —— 它仍是唯一 100% 真實預覽；3D 檢視的 banner 文案必須寫「離線 3D 場景（近似顯示）」之類字樣，不得自稱實機畫面。

---

## 3. 渲染規格

座標系建議：X = tile 東西向、Z = tile 南北向（1 單位 = 1 tile）、Y = 高度。

1. **地形 mesh**：以 `boden.bmp` 為高度來源。頂點密度用 257×257（原生頂點格，≈6.6 萬頂點、13 萬三角形，一個 VBO 輕鬆處理）。高度公式：`Y = gray / 255 × HeightScale`，`HeightScale` 預設經目視校準（起始值建議 6.0，以 KAMP_000 河谷目視合理為準），並在 UI 提供「地形起伏」滑桿（0–2 倍）—— 因為垂直真實比例未證實（§1.2-3）。
2. **貼圖**：每 tile 從 `FloorTextureAtlas` 取 UV。一張圖去重後材質通常數十種，atlas 2048×2048 足夠；超出時退化為 4096 或多張 atlas 分批繪製。8-bit BMP 用 `new Bitmap(stream)` 轉 32bpp 後上傳（`FloorTextureLibrary` 已示範讀法）。
3. **光照**：單一方向光（建議自東南上方）+ 環境光，法線由高度場中央差分計算（`TerrainMeshBuilder` 內）。不做陰影貼圖。
4. **水面**：`Y = (Waterlevel / Heightmapstep) / 255 × HeightScale` 高度的半透明平面（alpha ≈ 0.55，色 = `WaterColor`），alpha blend、關深度寫入。
5. **格線**：`ShowGrid` 開啟且相機夠近時，以 tile 邊界線框疊加（深色半透明，模仿現有 2D 格線用途）；已變更格（與 baseline 不同）以黃色框標示，對齊 2D 行為。
6. **場景物件**：`ShowObjects` 開啟時，SDL 物件世界座標換算 tile 座標（`world / 64 / 4`＝tile；先除 64 到地圖像素、再除 Heightmapstep 到 tile），Y 貼地。建築＝方塊、單位＝圓柱、其他＝菱形（八面體），隊伍色沿用 `MapCanvasControl.TeamColor`。
7. **相機**：預設遊戲感視角（pitch ≈ 55°、距離看全圖）。滾輪＝縮放（拉近時朝游標點）、中鍵拖曳＝平移對焦點、Alt+中鍵（或右鍵拖曳空白處）＝旋轉 yaw／pitch。注意右鍵已有「取樣」語意：**點擊（位移 < 4px）＝取樣，拖曳＝旋轉**。
8. **重繪策略**：事件驅動（輸入／SetTexture 時 `Invalidate()`），不跑常駐 render loop，避免占用 CPU/GPU。

---

## 4. 編輯互動規格

- 左鍵按下／拖曳：`TerrainRayPicker` 求得 tile → 依 `BrushSize`（1/3/5）對周邊格套用 `BrushTexture`，raise `TexturePainted`（每格一次、單次拖曳去重 —— 對齊 `MapCanvasControl.TryPaint` 的 `_paintedInDrag` 行為）。
- 右鍵點擊：取樣該格材質，raise `TextureSampled`。
- 滑鼠移動：raise `TileHovered`（狀態列顯示格座標與樣式名，既有接線）。
- `EditingEnabled == false`（原廠圖）時只可瀏覽，繪製事件不觸發 —— 與 2D 相同。
- 繪製後的畫面更新：改該 tile 的 UV buffer 子區段 + `Invalidate()`；**禁止**整個 mesh／atlas 重建（拖曳筆刷會卡）。

---

## 5. 硬性安全與 UX 規範

1. 寫入面**零變更**：仍只有 `briefing.put` 標題、`boden.ini` 已開放鍵、`boden.txt` 材質表，全部走既有 `SafeFileWriter` + `FileRollbackScope`。3D 檢視不解鎖任何高度／碰撞／物件寫入（那些受 `docs/reverse-engineering/map-formats.md` 的閘門管制）。
2. 原廠圖唯讀、`遊戲原始檔案/` 絕不修改、測試絕不碰真實遊戲目錄 —— 沿用既有規範。
3. 玩家導向 UI：不得在主 UI 露出內部材質 ID、通道名、檔名等開發者術語（repo 歷史已為此返工兩次，見 `AI_HANDOFF.md`）。所有新字串繁體中文。
4. GL 初始化失敗（遠端桌面／舊顯卡）必須被 try/catch 捕捉，自動停用 3D 鈕並顯示友善訊息，2D 畫布照常運作。
5. 本文件與其他含中文的文件保持 UTF-8 with BOM。

---

## 6. 分段交付

| Phase | 內容 | 驗收重點 |
|---|---|---|
| **A** | OpenTK 接線、地形 mesh + atlas 貼圖 + 光照、相機操作、2D/3D 切換與 fallback（唯讀檢視） | 開 KAMP_000／ENDL_000 能看到與 minimap 特徵吻合的 3D 地形；切換與 fallback 正常 |
| **B** | 水面、格線／變更格標示、SDL 物件 3D 標記、`ShowGrid`/`ShowObjects` 接線、起伏滑桿 | KAMP_000 河谷水面與 minimap 河道吻合；ENDL_000 物件數與 2D 一致（613） |
| **C** | 射線拾取 + 筆刷／取樣／hover，與 2D 共用 undo/redo/dirty/儲存 | 在 3D 塗材質 → 切回 2D 看到相同變更；Ctrl+Z/Y、還原地表、儲存後遊戲內實測地表變更生效 |
| **D**（選做，需另行同意） | 從 `shad.dat` 抽建築圖示做 billboard，取代色柱標記 | 不阻塞 A–C；`.alr` 模型逆向明確**不做** |

每個 Phase 結束：Release build 0 warning/0 error、全測試綠、`git diff --check` 乾淨，並更新 `AI_HANDOFF.md` 進度段落（比照既有條目格式）。

---

## 7. 測試（xUnit，加入 `tests/AgainstRomeModifier.Tests`）

CI 無 GPU：**所有測試只碰純邏輯類別，不建 GL context、不建 `Map3DViewControl`。**

- `TerrainHeightFieldTests`：合成 3×3／257×257 灰階陣列，驗證角點取樣、雙線性內插、邊界 clamp。
- `TerrainMeshBuilderTests`：小尺寸高度場 → 頂點數／索引數正確；平地法線 = +Y；斜坡法線方向正確。
- `TerrainRayPickerTests`：平地垂直射線命中預期 tile；斜視射線命中；山丘遮擋時取近端交點；射線出界回傳 false。
- `EditorCameraTests`：pitch clamp、縮放距離 clamp、view 矩陣對焦點不變性。
- `FloorTextureAtlas` 的排版邏輯若抽成純函式（材質清單 → rect 配置），加 `AtlasLayoutTests`：不重疊、皆在界內、超量時的退化策略。
- 回歸：既有全部測試綠（目前基準見 `AI_HANDOFF.md`，121/121）。

視覺 QA 比照 repo 慣例：以唯讀遊戲 fixture 開圖，用 computer-use 截圖核對（A：地形 vs minimap；B：水面／物件；C：塗改前後）。

---

## 8. 驗收清單

1. `dotnet build`（三專案）Release 0 warning/0 error；`dotnet test` 全綠。
2. 3D 場景：地形、真實貼圖、水面、光照、物件標記、格線齊備；60 FPS 級順暢（事件驅動重繪）。
3. 3D 直接繪製材質，與 2D 檢視、undo/redo、儲存完全一致；儲存後使用者進遊戲驗證地表變更生效。
4. 無 GL 環境自動退回 2D，功能不缺。
5. 未新增任何遊戲檔案寫入面；原廠圖仍唯讀。
6. `AI_HANDOFF.md` 已記錄成果與驗證數字。

---

## 附錄：本任務主要參考碼位

| 路徑 | 用途 |
|---|---|
| `src.MapEditor/MapCanvasControl.cs` | 2D 畫布（事件契約、筆刷去重、TeamColor、水面門檻、fallback 行為的對齊基準） |
| `src.MapEditor/MapEditorForm.cs` | 表單接線（`LoadEditingScene` :219、`PaintTexture`/undo/redo :238-254、工具列 :67-121） |
| `src.MapEditor/FloorTextureLibrary.cs` | floortex.dat ZIP 讀取 |
| `src.Shared/Maps/SdlSceneCatalog.cs` | SDL 物件解析與世界座標常數（`WorldUnitsPerMapPixel`、`MapPixelSize`） |
| `src.Shared/Maps/MapTextDocuments.cs` | boden.txt / boden.ini / put 文件物件 |
| `docs/reverse-engineering/map-formats.md` | 已驗證事實與寫入閘門（本任務的邊界依據） |
| `docs/map-editor-spec.md` | 編輯器整體規格（Phase 1–3 背景） |
