# against-rome-textures

Against Rome 零售版地圖地形材質儲存、LZSS/PFIL 解壓縮與交易式混合編輯引擎（Rust 完整移植）。

## 1. 真格式覆蓋界線 (Format Truth Boundary)

逆向工程查證與 C# 現行契約核對結果：

1. **地圖材質格持久化檔為 `boden.txt`**：
   - 零售版地圖目錄（如 `MAPS/TUTOR_01/`、`MAPS/ENDL_000/`）內記錄地表圖塊名稱的實際檔案為 `boden.txt`。
   - 檔案包含 `[Dimension]`（如 64 或 48）與 `[Texturen]` 區段，共 `Dimension × Dimension` 個圖塊名稱標記（如 `4BB___50`）。
   - 官方已安裝地圖（如 TUTOR_01、ENDL_000 等）在磁碟上普遍包裝為 **64-byte PFIL 標頭 + LZSS 壓縮體**（`PFIL` 魔術字節）；本 crate 支援透明解壓，並在未修改儲存時達到 **100% 位元級 SHA-256 原字節不變**；修改時支援標準 PFIL 壓縮回寫或純文字回寫。
2. **全域材質庫檔案為 `floortex.dat`**：
   - 位於遊戲主目錄根路徑，實為標準 ZIP 封裝體，內含 3,005 個 128×128 24-bit BMP 格式地表圖塊，與 `boden.txt` 中的名稱一一對應。
3. **`textures.dat` / `texture.dat` 之真實現況說明**：
   - 針對遊戲主程式 `Against_Rome.exe` 7.6MB 映像及零售地圖全目錄搜尋，證實**原版遊戲引擎與地圖格式中並不存在 `textures.dat` 或 `texture.dat` 檔案**。
   - 地圖 `DATA/` 目錄下的唯一包含 tex 字樣之檔案為 `hitex.dat`（40,008 位元組），該檔案為命中特效（hit effects）粒子池，非地表材質網格。
   - `TextureResourceKind::resolve` 提供容錯重新導向：若呼叫端傳入 `textures.dat` 或目錄路徑，會自動探測並對應至同目錄下之標準 `boden.txt`。

## 2. 核心功能與架構

- **`BodenTexturesDocument`**：
  - 完整保留檔頭註解、空格、換行符（CRLF/LF）與未知文字。
  - 支援單格 `set_texture(x, y, text)` 與整網格 `set_textures(&[String])` 批次寫入。
  - 支援 PFIL 壓縮識別與回存，保證無變更存檔還原原 hash。
- **`FloortexLibrary`**：
  - 讀取遊戲根目錄 `floortex.dat`（ZIP），提供紋理名稱列舉與 BMP 原生字節讀取。
- **`TerrainBlendAuthoringMap`**：
  - 地形材質角點網格（尺寸 `(Dimension + 1) × (Dimension + 1)`）。
  - `import`：從現有 tile 網格反向投票重建角點材質，追蹤未知/未解析圖塊與投票衝突。
  - `paint_circle`：半徑平方幾何檢定之圓形筆刷。
  - `bake`：調用解析器烘焙圖塊。
- **`TerrainBlendEditSession`**：
  - 交易式筆刷編輯會話：角點變更與烘焙圖塊原子綁定。
  - 遇到不支援交界（unsupported native junction）時整筆交易回滾（rollback pending stroke），杜絕半烘焙殘留。
  - 支援 `auto_bridge`（自動插入外圈中介材質銜接，最多 2 圈）。
  - 支援保護塗刷界線（`allows_tile` 閉包防護）。
  - 支援圖塊印章（`stamp_texture`，直接指定道路、水面、裝飾圖塊不改變角點）。
  - 支援歷史復原/重做（`undo` / `redo`）、基準點保存與重置（`commit_baseline` / `reset_to_baseline`）。
- **`TerrainRegionPlanner`**：
  - `rectangle`：外框兩點矩形圖塊計算。
  - `connected`：同材質 4 方向 BFS 洪水填充計算。
  - `polyline`：Bresenham 直線演算法帶筆刷寬度蓋印。

## 3. 可接 UI 的 API 總覽 (UI-Ready API)

供桌面編輯器或前端綁定呼叫：

```rust
// 1. 載入與初始化
let doc = BodenTexturesDocument::load("MAPS/ENDL_000/boden.txt")?;
let textures = doc.textures(); // Vec<String>
let catalog = Arc::new(StandardFloorMaterialCatalog::from_texture_names(&textures));
let import = TerrainBlendAuthoringMap::import(doc.dimension(), &textures, catalog.as_ref(), "BB")?;
let mut session = TerrainBlendEditSession::new(import, &textures, catalog)?;

// 2. 筆刷與編輯操作
// 圓形材質筆刷（支援自動過渡與保護邊界）
let result: TerrainBlendPaintResult = session.paint_circle(
    center_x, center_y, radius, "BC", 
    /* rollback_on_failure: */ true, 
    /* auto_bridge: */ true, 
    /* allows_tile: */ None
)?;

// 圖塊印章（道路/裝飾，不改角點）
let change: Option<TerrainTextureChange> = session.stamp_texture(x, y, "weg1");

// 區域填色（矩形或洪水填充）
let tiles = TerrainRegionPlanner::rectangle(session.tile_dimension(), (x1, y1), (x2, y2))?;
let result = session.paint_tiles(&tiles, "sand", false)?;

// 3. 交易提交與復原重做
session.commit_stroke();          // 筆畫結束提交到 Undo 堆疊
let undone_changes = session.undo(); // Option<Vec<TerrainTextureChange>>
let redone_changes = session.redo(); // Option<Vec<TerrainTextureChange>>

// 4. 存檔與基準同步
session.commit_baseline();
doc.set_textures(session.current_textures())?;
doc.save("MAPS/ENDL_000/boden.txt")?;
```

## 4. CLI 工具 (`ar-textures`)

```powershell
# 稽核地圖 boden.txt 結構與角點解析率
ar-textures audit "C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01"

# 驗證未修改回存之 SHA256 原字節完全一致
ar-textures roundtrip-test "C:\Program Files (x86)\Against Rome\MAPS\ENDL_000" "$env:TEMP\test"

# 於 TEMP 副本執行交易式塗刷、烘焙、Undo/Redo 與重新載入驗證
ar-textures paint-test "C:\Program Files (x86)\Against Rome\MAPS\ENDL_000" "$env:TEMP\test" 32 32 3 "BC"

# 唯讀掃描遊戲目錄下全 74 張地圖（295,936 格）
ar-textures verify-all-installed
```
