# 隊伍色彩、地形細節與動畫語意逆向工程報告 (Colors, Terrain Detail, and Animation Semantics)

日期：2026-10-07  
代理人：Antigravity  
工作範圍：唯讀分析 `%TEMP%\ArmNativeAssets_20261007` 原生素材與 `re_workspace/Against_Rome.exe` 靜態反組譯，探究隊伍色、配置 INI、單位與建築動畫語意，並產出實體驗證工具 `tools/re/anim-probe` 與圖集。

---

## 摘要與核心結論

1. **`cl_color.ini` 與色彩系統**：
   - `cl_color.ini` 定義**遊戲 UI 覆蓋色與色彩調整**（生命條、法力條、建造過濾器、選擇圈、亮度、對比度、飽和度），格式為 `0x00bbggrr`。它**不包含隊伍色定義**。
   - 真正的隊伍色定義存於 `shad.dat` 中的 `SYSTEM/DATA_MP/PALETTEN/teamcolors.bmp`（9×2 像素 24bpp BMP）。引擎啟動時在 `0x47EC8C..0x47EC9F` 將其載入到全域隊伍色表 `0x771c04`（9 個隊伍色）。
   - ALR 單位檔案自身的 9 種色盤變體（`PaletteVariantCount = 9`）是針對兵種各部位（如衣服、盾牌圖紋、披肩）專門烘焙的 9 套著色表，以隊伍編號 $0..8$ 直接作為色盤變體索引（Variant Index）。
   - APT 建築格式的色盤變體數多數為 1（即無隊伍變體）；建築在小地圖上的色彩則由 `teamcolors.bmp` 中的隊伍色代表。

2. **`cl_detai.ini` 與 `cl_epara.ini`**：
   - `cl_detai.ini`（Client Detail Settings）定義 5 種畫質檔次（`Wire`、`Low`、`Medium`、`High`、`VeryHigh`）的引擎渲染功能開關（顯示模式、水面特效、法線浮雕 Emboss、陰影模式 Stippled/Alpha、雲影、光暈等）。它**不包含地圖編輯器所需的地形細節貼圖**。
   - `cl_epara.ini`（Engine Parameters）定義陣型移動速度係數、旋轉係數、尋路深度、狂暴與護盾戰鬥數值、拋物線彈道誤差等**遊戲邏輯物理與平衡常數**。
   - **編輯器結論**：地圖編輯器的地形渲染器**不需要**讀取 `cl_detai.ini` 或 `cl_epara.ini`。地圖地形貼圖完全由地圖 `boden.bmp`、`vertex.bmp` 與貼圖集提供。

3. **動畫語意（單位 ALR）**：
   - 單位在 `objdef.txt` 中的關鍵動畫欄位：
     - 欄 2 `alen`：每套動畫的基準循環週期（毫秒），如步行/待機標準為 `1000` ms。
     - 欄 6 `anadd`：單位動畫動作集分類索引（0..12）。
     - 欄 45 `afram`：32-bit 位元遮罩（Packed Hex），每個 bit 代表該單位是否具有第 $k$ 種動畫動作。
   - **待機動畫（Idle/Standing）**：
     - 動畫索引 **0** 即為待機/呼吸/站立循環（Idle Animation）。
     - 步兵單位每方向固定有 **24 格**（`LayoutColumns = 24`），方向數為 16（`LayoutRows = 16`）。
     - 在 `alen = 1000` ms 的基準下，待機動畫的標準幀率為 **24 FPS**（每幀約 41.67 ms）。

4. **動畫語意（建築 APT）**：
   - APT 四維狀態布局為：`Layout[0]`（建造階段，0..4）、`Layout[1]`（變體/光照狀態，值為 2）、`Layout[2]`（損壞程度，0..4）、`Layout[3]`（動畫格數，1 或 25 或 50）。
   - **Axis 1 語意解密**：全庫 222 個 APT 中，有 221 個 `Layout[1] == 2`（僅 `error.apt` 為 1）。原先懷疑可能為光照或替代狀態。經逐像素比對，完工狀態（$a=4, c=0$）下 $b=0$ 與 $b=1$ 的解碼像素完全一致（差異為 0）。在引擎運行時（`0x4C69D3`）Axis 1 是物件內部結構欄位 `+0x6E`，用於動態建築狀態分組，在靜態與預設編輯視圖中傳入 0 即可。
   - **建築動態格**：主要包含煙囪冒煙、屋頂火把閃爍（如 `gerhau02` 的 25 格火光循環）與旗幟飄動。
   - 建築循環週期 `alen` 多為 3000 ms（25 格時約 8.33 FPS）或 4500 ms（50 格時約 11.11 FPS）。

---

## 一、`cl_color.ini` 與隊伍色彩系統

### 1. `cl_color.ini` 的解析與真實意圖

經以 `AgainstRomeModifier.GameLZSS.DecompressPfil` 解壓縮 `%TEMP%\ArmNativeAssets_20261007\cl_color.ini`，其內容為純文字 INI，格式為 `0x00bbggrr`：

| 區段標籤 | 原始數值 (BGR Hex) | 解碼 RGB Hex | 語意說明 |
| :--- | :--- | :--- | :--- |
| `[LifePointBar]` | `0x00b36b` | `#6bb300` | 生命值條顏色（綠色） |
| `[LifePointBarEnemy]` | `0x0000ff` | `#ff0000` | 敵方生命值條顏色（紅色） |
| `[LifePointBarFriend]` | `0x00ffff` | `#ffff00` | 友方生命值條顏色（黃色） |
| `[ManaPointBar]` | `0xff9000` | `#0090ff` | 法力值條顏色（藍色） |
| `[ProduceBar]` | `0xffffff` | `#ffffff` | 生產進度條顏色（白色） |
| `[UpgradeBar]` | `0x7f7f7f` | `#7f7f7f` | 升級進度條顏色（灰色） |
| `[BuildBar]` | `0x5060a0` | `#a06050` | 建造進度條顏色 |
| `[SelectionCircle]` | `0x00b36b` | `#6bb300` | 單位選取底圈顏色 |
| `[FlankenkreisFront]` | `0x00bfbf` | `#bfbf00` | 陣型前側面標記圈 |
| `[ColorFilter1]` | `0x00ff00` | `#00ff00` | 建築放置有效時的覆蓋濾鏡（綠色） |
| `[ColorFilter2]` | `0x0000ff` | `#ff0000` | 建築放置無效時的覆蓋濾鏡（紅色） |
| `[Brightness*]`, `[Contrast*]` | - | - | 全域亮度與對比度調節值 |

**結論**：`cl_color.ini` 與單位/建築的本體隊伍顏色無關，而是掌管 HUD、介面條、放置確認反饋等著色。地圖編輯器可引用 `ColorFilter1` 與 `ColorFilter2` 用於 3D/2D 放置物件時的合法性預覽。

### 2. 真正的隊伍色：`teamcolors.bmp`

靜態分析 Against_Rome.exe `0x47EBEA..0x47ECA1`：
- 引擎尋找 `teamcolors.bmp`（位於 `shad.dat` 的 `SYSTEM/DATA_MP/PALETTEN/teamcolors.bmp`）。
- 格式為 9 寬 × 2 高、24bpp BMP。
- 引擎在 `0x47EC5D` 檢查寬度為 9、高度為 2。
- 迴圈呼叫 `0x499100(teamIndex, colorDWORD)`，將 9 個像素寫入全域隊伍色陣列 `0x771c04[teamIndex]`。

從 `teamcolors.bmp` 提取出的隊伍色定義如下：
- **Row 0（預設隊伍主色）**：
  - 隊伍 0：`#FF0000`（紅）
  - 隊伍 1：`#8F572F`（棕褐）
  - 隊伍 2：`#F0D700`（金黃）
  - 隊伍 3：`#00B700`（綠）
  - 隊伍 4：`#00BFC0`（青藍）
  - 隊伍 5：`#999A9B`（灰白）
  - 隊伍 6：`#000080`（海軍深藍）
  - 隊伍 7：`#BF0090`（洋紅/紫）
  - 隊伍 8：`#FFFFFF`（亮白）
- **Row 1（副色/邊框色）**：
  - 各隊伍輔助色（由 `#9C795A` 開始偏移）。

### 3. ALR 與 APT 的隊伍變體機制

1. **ALR（單位與散佈物件）**：
   - ALR 格式在 header 包含 `PaletteVariantCount`（若 `objdef` 欄 17 `palty == 1`，此值通常為 9）。
   - 在第一格（Frame 0）內，儲存了 $9 \times \text{PaletteColorCount}$ 個色盤 DWORD。
   - 著色邏輯：當解碼隊伍 $N$ 的單位時，以 `variant = N` 提取第 $N$ 套色盤進行 8-bit 行解碼。
   - 差異部位：經比對 `gersch01.alr` 發現，9 套色盤中非隊伍部位（如皮膚、金屬鎧甲、武器）保持完全一致，僅兵種衣服、盾面圖紋等特定色盤索引隨變體轉換成隊伍對應的色系（變體 0 為紅色系、變體 2 為金黃色系等）。
2. **APT（建築）**：
   - 全庫 222 個 APT 檔案中，`PaletteVariantCount` 全部為 1。
   - 建築本身不具備 ALR 式的多套隊伍著色盤；建築的隊伍歸屬在原版遊戲中小地圖以小點顯示，底層像素則不隨隊伍變更。

---

## 二、`cl_detai.ini` 與 `cl_epara.ini` 分析

### 1. `cl_detai.ini`（客戶端畫質設定）

解壓縮後顯示其結構為 5 組渲染開關：`Wire`、`Low`、`Medium`、`High`、`VeryHigh`。
關鍵鍵值包括：
- `DisplayStyle`：0 = Wireframe，1 = Textured。
- `WaterDetail`：0 = 靜態水面，1 = 焦散（Caustics），2 = 雲影倒影，3 = 浪花泡沫，4 = Emboss 浮雕水面。
- `TextureDetail`：0 = 普通紋理，1 = Emboss 浮雕地形貼圖。
- `LightDetail`：0 = 無光效，1 = Glow，2 = Lens Flare。
- `ShadowDetail`：0 = 無陰影，1 = 點陣網點（Stippled），2 = 25% 半透明，3 = 完整 Alpha Blending。
- `SkyDensMode`：天空漫射環境光開關。

**地圖編輯器結論**：
這是典型的 DirectX 畫質檔次切換配置。編輯器的 3D 視圖採用現代 OpenGL 渲染，不需要讀取或套用此檔案中的切換邏輯。

### 2. `cl_epara.ini`（引擎運行參數）

定義戰鬥與物理常數：
- `FormationRotationFaktor = 500.0`：陣型旋轉速度上限。
- `FormationSpeedFaktor = 0.7`：陣型中脫隊士兵追趕速度比例。
- `FormationPathDepth = 2`：碰撞阻擋時的尋路重試深度。
- `MoveVector3DIntensity = 100`：3D 速度向量與 2D 投影速度混合權重。
- `BerserkerAWfaktor` / `BerserkerDAMfaktor`：狂暴狀態攻防倍率。
- `ProjectileVarianceOnMove` / `ProjectileVarianceMaximumAngle`：遠程武器提前量預測與散射角度。

**地圖編輯器結論**：
純屬遊戲執行期（Runtime Gameplay）邏輯常數，與地圖編輯、地形繪製及物件顯示完全無關，編輯器完全不必解析。

---

## 三、動畫語意：單位 ALR

### 1. `objdef.txt` 動畫相關欄位逆向

通過對 Against_Rome.exe `0x4B082C..0x4B1F0A` 與 `0x4BB770..0x4BBE7D` 的反組譯分析：

1. **`anadd`（欄 6）**：
   - 單位動畫類型（Animation Set ID）。例如步兵 `anadd=0` 或 `5`，騎兵 `anadd=1`，平民 `anadd=2`，弓手 `anadd=9`。
2. **`afram`（欄 45）**：
   - 在 EXE 中以 `%lx` 讀取為 32-bit 無號整數（例如 `00008008`、`00008010`、`00004004`）。
   - 在 `0x4BB863` 中，引擎使用 `(1 << actionId) & afram` 檢查該物件是否支援某種動作！
   - 例如步兵的 `afram = 0x00008008`：代表 bit 3（`1 << 3 = 8`）與 bit 15（`1 << 15 = 0x8000`）有效。
3. **`alen`（欄 2）**：
   - 動畫週期長度（毫秒）。絕大多數活體單位為 `1000`（即 1.0 秒循環一次）。
4. **`aafrn`（欄 46）與 `aafrw`（欄 47）**：
   - 輔助動作幀數偏移量。

### 2. 單位待機動畫（Idle Animation）確定

- **待機動畫索引**：**動畫索引 0** 即為單位的靜態站立/待機呼吸動畫。
- 驗證證據：
  - 根據原版 ALR 布局公式：
    $$\text{FrameIndex} = ((\text{AnimIndex} \times \text{Directions}) + \text{Direction}) \times \text{LayoutColumns} + \text{Frame}$$
  - 當 $\text{AnimIndex} = 0$、$\text{Direction} = 14$（面向鏡頭正向）時，$\text{Frame} = 0..23$ 呈現完整的持盾守備、輕微呼吸起伏循環，首尾自然接合。
  - 當 $\text{AnimIndex} = 1$ 時，格數呈現行進踏步動作。
- **幀率與時序**：
  - 每方向格數 $\text{LayoutColumns} = 24$。
  - 循環週期 $\text{alen} = 1000\text{ ms}$。
  - **推薦幀率**：$$\frac{24\text{ frames}}{1.0\text{ s}} = \mathbf{24\text{ FPS}}$$（每幀持續時間為 $41.67\text{ ms}$）。

---

## 四、動畫語意：建築 APT

### 1. 四維布局軸向定義

$$\text{LinearIndex} = (((a \times L_1) + b) \times L_2 + c) \times L_3 + d$$

- **Axis 0 ($a$)**：建造進度階段（Construction Stage），$0..4$。0 為地基，4 為完工。
- **Axis 1 ($b$)**：變體狀態 / 群組狀態（常態為 $2$ 種）。
- **Axis 2 ($c$)**：損壞狀態（Damage / Destruction Level），$0..4$。0 為無損，4 為嚴重毀損。
- **Axis 3 ($d$)**：動畫幀（Animation Frame），$0..(L_3-1)$。

### 2. Axis 1 的深度驗證與結論

- 全庫 222 個建築 APT 檔案中，除損壞備援的 `error.apt`（$1\times 1\times 1\times 1$）外，全部 221 個檔案的 $L_1$ 皆為 **2**。
- 透過 `anim-probe` 對 `gerhau02.apt` 完工狀態（$a=4, c=0$）比對 $b=0$ 與 $b=1$ 的全部解碼畫布：
  - 寬 752 × 高 550 = 413,600 個像素中，**差異像素數為 0**。
  - 導出檔案 `gerhau02_axis1_val0.png` 與 `gerhau02_axis1_val1.png` SHA-256 完全相同。
- 在 EXE 反組譯 `0x4C69D3` 中，傳入此軸的參數為物件結構 `[eax + 0xBB9A6E]`。原引擎設計此軸預留作為不同建築外觀變體（如屋頂積雪/季候變化），但實際商業發行版中兩個槽位引用了相同的幾何貼圖圖集。
- **編輯器具體指引**：在顯示完工建築時，固定取 $b = 0$ 即可。

### 3. 建築動態動畫幀（Axis 3）

全庫 APT 的 $L_3$ 分布：
- 靜態建築（$L_3 = 1$）：173 個（如圍牆、木樁、住宅 `gerwoh00`、裝飾石雕等）。
- 動態建築（$L_3 = 25$）：22 個（如主屋 `gerhau00/02`、武器鐵匠鋪 `gerwaf00/01/02`、金礦等）。
- 高幀數動態建築（$L_3 = 50$）：27 個（如屠宰場 `gerschla00`、木工廠 `gerschre00`、馬廄 `gersta00` 等）。

動態內容：
- 主要為火把火光搖曳、煙囪裊裊白煙、祭壇法陣動態或懸掛的旗幟飄揚。
- `objdef` 中的建築 `alen`：
  - 25 格建築（如 `BauGerHau02`）的 `alen = 3000` ms：
    $$\text{FPS} = \frac{25}{3.0} \approx \mathbf{8.33\text{ FPS}}\quad (\text{每幀 } 120\text{ ms})$$
  - 50 格建築（如 `BauGerSchla00`）的 `alen = 4500` ms：
    $$\text{FPS} = \frac{50}{4.5} \approx \mathbf{11.11\text{ FPS}}\quad (\text{每幀 } 90\text{ ms})$$

---

## 五、地圖編輯器「待機動畫」實作具體建議

為在地圖編輯器（Map Editor）中支援單位與建築的流暢待機動畫預覽，建議設計如下：

1. **動畫時鐘與更新頻率**：
   - 建立全域或視圖級別的動畫計時器（`AnimationTimer`），以 60 FPS 累積全局毫秒時間 $T$。
2. **單位幀選取算法**：
   - 設單位朝向方向列為 $D \in [0..15]$（或 32 方向騎兵 $D \in [0..31]$）。
   - 每方向格數 $C = \text{LayoutColumns}$（通常為 24）。
   - 循環週期 $T_{\text{loop}} = 1000\text{ ms}$。
   - 當前幀號 $f = \lfloor (T \bmod T_{\text{loop}}) / T_{\text{loop}} \times C \rfloor$。
   - 最終 ALR 幀索引：
     $$\text{frameIndex} = ((0 \times \text{LayoutRows}) + D) \times C + f$$
3. **建築幀選取算法**：
   - 建築完工狀態取 $a = 4, b = 0, c = 0$。
   - 動畫格數 $K = L_3$。若 $K == 1$，固定使用 $d = 0$。
   - 若 $K > 1$（如 25 或 50），取對應建築之 `alen`（預設 25 格為 3000 ms，50 格為 4500 ms）：
     $$d = \lfloor (T \bmod \text{alen}) / \text{alen} \times K \rfloor$$
   - 最終 APT 線性幀索引：
     $$\text{frameIndex} = (((4 \times L_1) + 0) \times L_2 + 0) \times K + d$$
4. **紋理快取與效能優化**：
   - 單位待機動畫僅 24 格，可在放置或進入視圖時一次性將 24 格快取為 Atlas 或點陣精靈圖。
   - 建築 APT 僅更新頂層動態菱形磚（Tile），不必重新解碼整個 752×550 畫布。

---

## 六、實體驗證產出與檔案清單

### 1. 新增檔案
- [`docs/reverse-engineering/colors-detail-animation.md`](file:///D:/Github/AgainstRomeModifier/docs/reverse-engineering/colors-detail-animation.md)（本技術報告文件）
- [`tools/re/anim-probe/anim-probe.csproj`](file:///D:/Github/AgainstRomeModifier/tools/re/anim-probe/anim-probe.csproj)（動畫與色彩驗證工具專案檔）
- [`tools/re/anim-probe/PngWriter.cs`](file:///D:/Github/AgainstRomeModifier/tools/re/anim-probe/PngWriter.cs)（標準 BCL 無依賴 PNG 匯出器）
- [`tools/re/anim-probe/Program.cs`](file:///D:/Github/AgainstRomeModifier/tools/re/anim-probe/Program.cs)（實測驗證程式）

### 2. 產出的驗證圖像（位於 `%TEMP%\AnimProbeExports_20261007`）
- `gersch01_anim0_idle_sheet.png`：日耳曼步兵待機動畫（方向 14）全部 24 格之 6×4 聯合格圖集。
- `gersch01_team_variants_sheet.png`：同一單位在隊伍 0..8 之 9 套色盤變體橫向對比圖集。
- `gerhau02_axis1_val0.png` 與 `gerhau02_axis1_val1.png`：主屋完工狀態下 Axis 1 = 0 與 1 之完整渲染對比（證明 0 像素差異）。
- `gerhau02_anim_torch_sheet.png`：主屋屋頂火把 25 格動態循環動畫聯合格圖集（5×5 排列）。
- `probe_summary.json`：匯出報告摘要。

### 3. 解壓縮文字檔案（位於 `%TEMP%\ArmNativeAssets_20261007`）
- `cl_color.txt`（1,530 bytes）
- `cl_detai.txt`（2,734 bytes）
- `cl_epara.txt`（4,463 bytes）
