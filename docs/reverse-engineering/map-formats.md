# 地圖格式與 Phase 3 閘門

> 狀態：靜態驗證與候選假設（2026-07-11；2026-10-06 追加 EXE 靜態反組譯證據）。本文件不是直接修改原始遊戲檔的授權；所有產品功能必須經修改器的受控寫入／還原流程。

2026-10-09 補充：[action／anim 資料池](map-data-pools.md) 記錄原生布局、載入失敗流程及 74 張地圖結構驗證；欄位語意與空白地圖實機載入仍待確認。

## 2026-10-06 EXE 靜態證據：地圖圖層語意與快取重建

依據對唯讀副本 `Against_Rome.exe`（SHA-256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，置於 `re_workspace/`，已 gitignore）之 Capstone 反組譯靜態分析，釐清地圖載入管線、圖層語意與快取重建機制：

### 地圖載入管線（Map Load Pipeline）

由 `0x487180`–`0x4873e1` 附近的 timing labels 可完整還原地圖載入順序：
`LDFLOORINI`（`boden.ini`，`0x481de0`）
→ `LDFLOORTEX`（`boden.txt`）
→ `LDFLOORHMAP` `boden.bmp`（`0x483410`）
→ `LDFLOORCMAP` `collision.bmp`（`0x486ec0`）
→ `LDFLOORSMAP` `smooth.bmp`（`0x4835f0`）
→ `LDFLOORVMAP` `vertex.bmp`（`0x483bf0`）
→ `LDFLOOREMBMAP` `emboss.bmp`（`0x4845b0`）
→ `CALCWATERWARP`
→ `LDSKYMAP` `skydens.dat`
→ `LDVISMAP` `visible.dat`
→ `LDCLIPMAP` `cliprect.dat`
→ `LDSHADMESHES` `shadows.dat`
→ 載入關卡存檔資料 `DATA`（經由 `mp_lsave.c` 載入 `way.dat`、`hirarchy.dat`、`particle.dat`；這些是關卡存檔資料，非高度衍生快取）。

全域參數方面：`[Heightmapstep]` 存於全域 `0x771880`（setter 為 `0x498d70`）；`[Waterlevel]` 存於全域 `0x77187c`。

### 各圖層語意分析

- **`boden.bmp` = 高度圖（HEIGHT MAP）**：
  尺寸必須為 `(W+1) × (H+1)` 頂點（64 tile 地圖對應 257×257），載入器轉換為 32bpp；高度 `height[y][x]` = GREEN channel × `Heightmapstep`，以 `int16` 網格（stride `0x202`）存放於 `0x16121e8`。
- **`smooth.bmp` = 平滑遮罩（Smoothing mask）**：
  GREEN 通道數值代表載入期對高度場進行疊代加權平均平滑（iterative weighted-average smoothing）的次數（當數值大於 pass index 時持續執行 passes；鄰居權重 2/3、中心權重 256）。
- **`vertex.bmp` = 逐頂點 RGB 色彩調色（Per-vertex RGB color tint）**：
  24-bit（遮罩 `0xffffff`）存放於 `0x16365e8`；若檔案缺失則預設為純白（white）。**不是高度圖**（更正了舊假設）。
- **`emboss.bmp` = 逐頂點光照強度（Per-vertex lighting intensity）**：
  GREEN 通道（0–255）存放於 `0x1676df0`，在光柵化器（rasterizer，約 `0x4907dc`）中跨地形多邊形進行內插；檔案缺失時預設為 255。字串 `GENEMBOSS`（`0x4a9330`）為貼圖凹凸生成（texture bump generation），而非地形光照。
- **`collision.bmp` = 256×256 tile 像素地圖**：
  在 `0x4d87a0` 先將陣列清為 0，非黑色像素以 `(R+G+B)/3` 作為 `int16` 存放於 `0x1e22c50`；`0` = 無碰撞（可通行），`>0` = 阻擋（不可通行；除錯覆蓋層將 `>0` 繪製為紅色）。此外，`0x4d8830` 會將所有 `visible.dat` bit0 未設定的 tile 強制設為 255。

### 快取重建機制（skydens.dat, visible.dat, cliprect.dat, shadows.dat）

- `skydens.dat`、`visible.dat`、`cliprect.dat` 與 `shadows.dat` 為高度場衍生的快取檔案。
- 其標頭（header）包含地圖寬度、高度、**所有高度總和（SUM OF ALL HEIGHTS，函式 0x484e50）** 以及其他參數（如 `Waterlevel`）。
- 若檔案缺失或標頭資訊不符，遊戲引擎會自動重新計算資料（例如 visible 於 `0x492830`、cliprect 於 `0x48fb70`）並以 `"wbp"` 模式寫回檔案。
- **編輯器處理原則**：編輯地形高度後，編輯器應直接刪除這四個快取檔案（若高度編輯剛好維持總和不變，引擎可能誤用舊快取）。

### 編輯器實作規劃（Editor Plan）

- **高度筆刷（Height brush）**：寫入 `boden.bmp` 灰階（R=G=B）。
- **地形打光（Emboss relighting）**：僅在坡度改變處重新計算 `emboss.bmp`，利用取自該地圖原始資料計算的最小二乘法擬合：`emboss ≈ c0 + cx * dh/dx + cy * dh/dy`。
- **碰撞筆刷（Collision brush）**：寫入 0 / 255。
- **快取刪除**：在儲存交易（`FileRollbackScope`）內一併刪除上述四個快取檔案。

## 2026-10-06 場景編輯（放置物件／事件）可行性證據

- **SDL `onload` 不會自動建造**：EXE 會列舉地圖目錄 `*.sdl`（`%s%s/*.sdl`），但 `tcon_build`／`tcon_buildall`／`tcon_buildonload`／`tcon_delobj` 是開發用聚落主控台 UI 的按鈕名稱（`0x473600` 起依名稱分派到 `0x472ad0` 模式 1/2/3）。實機測試：在 ENDL_005 寫入 `ARM_Placed.sdl`（`onload=1` 的日耳曼主屋與匈人部隊、隊伍 0），進遊戲後未出現、「前往主屋」無效、人口不變；地圖仍正常載入且未當機。SDL 聚落是 AI 建村藍圖，由地圖腳本（`s_setVillageTemplate`、`Endlos_*_Siedlung*` 符號）使用。
- **預放世界物件存放在 `DATA/*.dat`（關卡存檔格式，由 `mp_lsave.c` 讀寫，`FUN_00487F70` 寫出）**。Agy 靜態解析（ENDL_000，PFIL 解壓後）：`objects.dat` 標頭 16 B（ver=1, count=14000, 字串長 30/30），14,000 筆固定 79 B 紀錄（+0x00 active、+0x01 team u16〔8=中立〕、+0x03 uid u32、+0x07 name[30]、+0x25 idname[30]、+0x43/+0x45 位置索引、+0x4B type_id u16），其後 18 個平行欄位陣列；`position.dat` 標頭 8 B（ver=1, count=33000），每筆 17 B（valid u8, x/y/z/rot float32）；`objdata.dat` 14,000 槽的執行期數值（HP、lprel、mprel…）。ENDL_000 有 6,618 個啟用物件，全為 team 8 的樹木／岩石／植被（160 種 type_id）。新增靜態物件可用空槽完成；建築與單位可能牽涉 `hirarchy/anim/action.dat`，尚未證實。
- **建築可直接寫入 DATA（2026-10-07 實機驗證）**：KAMP_* 的 912 筆以上建築紀錄全部未連結；同型建築在不同隊伍間只差 +0x01 team、uid、位置索引與自身索引（`objdata` 各段相同）。以原版完工建築為範本複製並改 team，ENDL_005 開局即出現完工、可選取、屬於玩家的主屋／住宅／農場；腳本 `s_createObj` 生成的建築則是 0% 工地。
- **`objdata.dat` 完整佈局（2026-10-07，依 `0x48c760` 讀取序列逐段推得並以 ENDL_000 驗證，結尾位移＝檔案大小）**：8 B 標頭後為 36 段「14000 × 寬度」陣列，寬度依序 `9(u8,f,f) 8(f,f) 4 4(u16,u16) 2 16(u16,u16,f,f,f) 4 4 2 2 2 2 4 2 2 2 4 4 4 4 2 4 2×10 4 4 2 2`，每槽 123 B。世界物件只在前 3 段有差異（第 0 段 active＋HP＋lprel；第 2 段 0.35 只出現在 400 個與欄 10 連結的物件）。`objects.dat` 的 `type_id` 就是 `SYSTEM/DATA_MP/DEFAULTS/objdef.dau` 的列索引（第 52 欄名稱；例 146＝`LanGerGra00_Gras_M`、232＝`BauGerHau00_Haupthaus`）。ENDL_000 另含 40 個 `Skriptmark01_Infanterie` 腳本標記，編輯器不得刪除。
- **自然物件實機驗證（2026-10-07）**：編輯器以「複製同類型未連結物件的整個槽位，只改 UID／位置／自身索引」在 ENDL_005 地圖中央種下一排闊葉樹並寫回三個 DATA 檔；進入無盡模式後地圖中心出現等距成排的大樹，遊戲穩定運作且無崩潰傾印。

- **遊戲內建 IPR 腳本編譯器**（「IPR - Copyright (C) 2000-2003 by Uwe Schmelich」）：類 C 文法（yacc/flex；int/double/string/object、if/while/switch/for、#include/#define、陣列、wait/exit、`SYSTEM` 外部函式宣告），流程 `.ics` → `.ias` → `.bci`。`SYSTEM/CLAK/cl_scint.ini`（PFIL）`[General] CompileScripts=0`；開啟後 `0x52FA91` 會把所有腳本副檔名改為 `ics`，而遊戲未附任何 `.ics` 原始碼，因此不能直接全域開啟。事件系統可行路線：自行產生 BCI（或 IAS）並注入地圖的 `ak_level` 腳本，呼叫既有 API（`s_createObj`、`s_createUnitAndMems`、`s_showTextBox`、`s_timeReached`、`s_setTeamHostile`、`GLOBAL_MISSION_RESULT`…）。
- 開發者內建編輯器的說明 overlay（`FUN_0047AE20`）由全域 `0x771D8C` 控制，唯一設定點寫 0；按鍵回呼 `0x47ABE0` 已是 `xor eax,eax; ret` 空殼，無法完整啟用（Codex 靜態分析）。

## 已靜態驗證

以工作區的五張原版 `MAPS/ENDL_000` 至 `ENDL_004` 為唯讀樣本：

| 檔案 | 格式 | 尺寸 | 穩定結論 |
|---|---:|---:|---|
| `vertex.bmp` | 24-bit BMP | 257×257 | 頂點格候選圖層；不是灰階單通道圖。|
| `boden.bmp` | 24-bit BMP | 257×257 | 三通道幾乎相同的灰階圖；用途尚未確認。|
| `emboss.bmp` | 24-bit BMP | 257×257 | 陰影／光照候選圖層；有些原版圖全黑。|
| `smooth.bmp` | 24-bit BMP | 257×257 | 灰階遮罩候選圖層。|
| `collision.bmp` | 24-bit BMP | 256×256 | 三通道相同的純灰階、含大面積 0 與 255 區塊。|
| `minimap.bmp` | 24-bit BMP | 256×256 | 彩色預覽圖，可安全唯讀顯示。|

`boden.txt` 是 64×64 tile 名稱表，而 `boden.ini [Heightmapstep]` 的原版值為 4。因此 `64 × 4 + 1 = 257` 與頂點圖尺寸吻合，且 `64 × 4 = 256` 與 tile 圖尺寸吻合。這個格網關係是**結構事實**；不代表任一 BMP 色值的語意已被證實。

## 樣本通道觀察

- `collision.bmp` 在五張圖均為 R=G=B；平均亮度介於約 10 至 52，標準差 48 至 102，確認不是彩色資料。
- `boden.bmp` 在五張圖均近乎 R=G=B；`emboss.bmp` 與 `smooth.bmp` 亦主要為灰階，但 `vertex.bmp` 的三通道明顯不同。
- `ENDL_000/001/002/003/004` 的 `vertex.bmp` 通道範圍分別不同，不能使用固定「某一顏色等於高度」公式。
- 目視 `ENDL_000`：`vertex.bmp` 的低色彩區與 minimap 水域／地勢特徵有表面關聯，但這只是候選相關性，尚未具備寫入資格。
- 2026-07-12 追加唯讀跨層目視：`KAMP_000/boden.bmp` 的連續灰階坡面、河谷與人工高低差和 `minimap.bmp` 明確對齊；`vertex.bmp` 則更接近彩色地表快取。這足以讓離線 renderer 使用 `boden.bmp` 的局部梯度產生只讀 hill-shading，但仍不足以推導世界高度單位或授權高度寫入。
- 同日比對 `KAMP_000`、`ENDL_000`、`MP_000`、`HIST_000`：`Heightmapstep` 均為 4，`Waterlevel / Heightmapstep` 分別為 62、30、30、36，與各圖 `boden.bmp` 河谷低灰階區吻合。離線 renderer 因此以此門檻和 `WaterColor` 產生只讀水面遮罩；此證據仍只授權顯示，不授權直接改寫高度圖。
- SDL 唯讀解析確認 `[settlement] refpos` 加上各 `[objectNNNN] pos` 得到物件世界座標；連續柵欄以 64 世界單位排列，而 16,384 世界單位對應 256 地圖像素，因此 `world / 64` 可直接落到地圖像素。`KAMP_000/TEAM_7.sdl` 解析出 138 個有效物件，`ENDL_000` 八個聚落 SDL 合計 613 個。離線 renderer 依此顯示建築／單位／其他物件。2026-07-14 已補齊純文件層的 settlement/object 欄位解析、`refpos` 平移、物件欄位修改、增刪與連續重編號；合成 PFIL round-trip 測試確認未知欄位、註解與空值不會遺失，另將 repo fixture 的八個真實 ENDL_000 聚落 SDL 複製到暫存目錄做 no-op 儲存，解壓文字也逐 byte 相同。場景檢查器目前只在 marker-backed `ENDL_005–999` 提供既有物件 `team`／相對 `pos` 的受控驗證編輯，並可還原到開啟時值；尚未取得遊戲內變更與還原證據，因此不得標記為 runtime verified，也不開放物件增刪。

## 未證實，禁止寫入（與靜態證據釐清狀況）

> 依據 2026-10-06 EXE 靜態反組譯證據，下列第 1 至 4 項之底層格式與管線語意已由靜態分析釐清；但在修改器功能正式標記為 runtime-verified 之前，仍須由使用者透過遊戲內實機執行期驗證（in-game runtime verification）確認。

1. ~~`vertex.bmp` 哪一個通道（或其組合）代表高度、其量尺與 `Heightmapstep` 的換算方式~~ → **靜態證據已釐清**：`vertex.bmp` 為頂點 RGB 色彩調色，**不是高度圖**；真實高度圖為 `boden.bmp`（GREEN channel × `Heightmapstep`，stride `0x202` 之 `int16`）。（仍待使用者遊戲內實機驗證）
2. ~~`collision.bmp` 的黑、白、灰階是否分別代表可通行、不可通行或其他導航遮罩~~ → **靜態證據已釐清**：`0` = 可通行（無碰撞），`>0`（如 255）= 阻擋。（仍待使用者遊戲內實機驗證）
3. ~~`boden.bmp`、`emboss.bmp`、`smooth.bmp` 是否由遊戲重建，或必須與 vertex／材質資料同步修改~~ → **靜態證據已釐清**：`boden.bmp` 是高度圖權威來源；`smooth.bmp` 為載入期平滑疊代次數遮罩；`emboss.bmp` 為逐頂點光照，遊戲不會在載入時自動從高度重新計算地形 emboss，須由編輯器在坡度變更處擬合重算。（仍待使用者遊戲內實機驗證）
4. ~~`DATA/*.dat`、`cliprect.dat`、`shadows.dat`、`skydens.dat`、`visible.dat` 的權威性及重建規則~~ → **靜態證據已釐清**：`skydens.dat`、`visible.dat`、`cliprect.dat`、`shadows.dat` 為快取檔案，標頭含高度總和驗證碼，缺失或不符時由遊戲引擎自動重算並寫回；編輯高度後編輯器應直接刪除此四快取。`DATA/` 內存檔（`way.dat` 等）為關卡存檔資料，非高度衍生快取。（仍待使用者遊戲內實機驗證）
5. SDL `refpos`／`pos` 與 256 像素 minimap 的座標轉換。Phase 2 不得疊加物件位置，避免製造誤導性視圖。

因此目前的「從無盡範本建立」是完整複製已知可載入的 `ENDL` 範本，不是空白地圖生成器。只刪除 SDL 物件或將 `boden.txt` 鋪成單一材質，仍會保留範本的高度、碰撞與 `DATA/*.dat` cache，不能對玩家宣稱為真正空白；空白生成與自由編輯必須在上述圖層之重建規則與寫入功能通過 modifier workflow 的遊戲內實機驗證後，方可正式開放。

## 原遊戲渲染與內部 TextureEditor

### 可供獨立 renderer 使用的原始資源（2026-07-12）

- 2026-07-14 起，離線 3D renderer 對缺少 `boden.bmp`、無法讀取 `floortex.dat`、OpenGL 3.3 context／shader 失敗提供可複製診斷，並停用 3D、回退至 2D。診斷實際找出 `ENDL_005` 因 atlas 欄數強制使用 2 的次方而在超過 256 種貼圖後誤判 4096 atlas 容量不足；改用緊密矩形排列後，4096 atlas 可保持 128×128 texture 與 8px gutter 並容納 784 種。使用者實機截圖確認同一張圖已成功進入 3D，完整地勢、水面與地表貼圖可見，該容量問題已 runtime verified；混合素材邊界與 `4T` 規則仍不在此次驗證範圍。

- `floortex.dat` 是標準 ZIP 容器（副檔名雖為 `.dat`），共 3,005 個 entry；地表圖位於 `SYSTEM/DATA/FLOORTEXTURE/*.bmp`。
- `boden.txt [Texturen]` 的名稱可直接對應上述 BMP basename。例如 `4BJ___51` 與 `L5B09T1A` 均已在容器內找到；抽查圖檔為 128×128、8-bit BMP。
- 2026-07-14 的靜態縮圖比對顯示，`4B?___5?` 是基礎地表家族：第二個代碼代表材質，同一家族尾碼提供數個自然變體。這一層才適合作為一般使用者的「顏料」。
- `4T*`、`4U*`、`L*B*T*` 等圖塊具有明顯的邊界、轉角或混合圖樣，屬於渲染拼接素材；`AA_Brush01`～`AA_Brush36` 是筆刷形狀／遮罩。它們不是各自獨立的基礎地表，不得出現在玩家調色盤或要求玩家手動選用。
- `4Uxy__dV` 的 `x/y` 對應兩個基礎材質代碼，`d` 只使用數字鍵盤方向 `1/2/3/4/6/7/8/9`。原先將 `V` 視為純自然變體的假設已被否定：高對比 `4U89` 的 `x0` 像素覆蓋約 74–79% 第二材質、`x1` 約 23–28%、`x2` 約 45–47%，而原版地圖鄰域也顯示多數方向的 `x0` 比 `x1` 更靠近第二材質區。可是跨全部 pair 的同尾碼統計不維持這個固定次序，因此 exporter 不得全域硬編碼尾碼強度。
- 2026-07-14 全庫盤點：75 張 base `4B`、876 張 `4U`、333 張 `4T`、35 張 `AA_Brush`（缺號 31）。原圖 quadrant 比對確認 `4TABC_dV` 的 `A` 佔一個半面／相鄰兩角，`B/C` 各佔另一角，`d` 只出現 `2/4/6/8`；因此 native baker 應以每 tile 四角材質為輸入。`4U` 完整方向為 `1/2/3/4/6/7/8/9`。pair 是 canonical order：例如實庫有 14 張 `4U89`、沒有 `4U98`，不能假設交換兩材質代碼即可反向輸出。編輯器以 `floortex.dat` 內每張 transition BMP 的 TL/TR/BR/BL 實際角落小區塊與其 base material 實圖做色距分類，直接建立「四角配置 → 既有原生 tile」索引；不可用整個象限均色，否則 `x1` 的窄材質角會被背景稀釋而漏掉。3D 暫存 smoke 已將 B9 畫入全 B8 地形並安全儲存為 1 個 `4B9___50` 中心加八方向 `4U89`，undo/redo 亦通過。既有 `boden.txt` 可投票重建 corner grid，未知 tile 與相鄰 tile 的角點衝突會明確回報；找不到原版 junction 時整筆拒絕，不降級成硬方格。
- `alr.dat` 同樣是 ZIP 容器，共 2,075 個 `SYSTEM/DATA/ALR/*.alr` 資源（歷史盤點）。2026-10-07 EXE 靜態證據確認 ALR 含逐格矩形資料，不能概稱為已確認的 3D mesh；載入器與範圍計算見 [原生場景分析](native-scene-rendering.md)。
- `apt.dat` 是 ZIP 容器，共 222 個 `SYSTEM/DATA/APT/*.apt` 資源；`shad.dat` 是 ZIP 容器，共 2,674 個圖示／陰影資源。
- 唯讀遊戲樣本的 `MAPS` 下共有 73 個具備 `boden.txt` 與 `minimap.bmp` 的可渲染目錄：`KAMP` 34、`MP` 20、`HIST` 10、`ENDL` 5、`TUTOR` 4。
- 歷史規劃曾以自行讀容器的離線 renderer 為主。2026-10-07 使用者要求改為像《世紀帝國 II》一樣，在真實遊戲場景直接編輯；目前離線地形／markers 尚未達標，下一步應沿原生資源與引擎接口驗證完整場景。原始資源不得加入版本庫；分析安裝目錄須遵守最新版 AGENTS 授權限制。

- EXE 靜態庫在 `FUN_004a4c20`（`0x004a4c20`）包含 `TextureEditor V0.1`、參數清單與鍵盤操作字串；主畫面迴圈 `FUN_0047ae20` 僅在全域模式值為 `9` 時呼叫它。
- 正常模式轉換函式只公開模式 `0、1、2、4、7、8`，目前沒有找到將模式設為 `9` 的正常選單或命令列路徑。
- 該函式處理 buffer、seed、sharpen、blur、combine 等程序影像操作。現有證據較符合內部材質產生／除錯工具，不能視為完整地圖編輯器或安全的 3D 地圖預覽入口。
- 尚未找到原遊戲可直接載入 `ENDL_NNN` 的命令列參數。產品 UI 的 `minimap.bmp`／資料圖層必須標示為「編輯底圖」而非真實遊戲預覽；真實預覽目前只能安全儲存後啟動 `Against_Rome.exe`，再由使用者從無盡模式選單進入目標槽位。

## 下一輪受控實驗

每次只在**自製 `ENDL_005+` 地圖的 modifier workflow**中做一件可還原的小改動，並保留原始與修改後雜湊、遊戲結果與 restore 結果。

1. 複製一張原版地圖，僅在 `vertex.bmp` 中心 3×3 像素改一個通道；三個通道各做一份獨立副本。進入遊戲檢查地形高度，後續還原。
2. 以已證實影響高度的通道，量測色差 1、16、64 對應的世界高度，並跨兩張原圖重複。
3. 只將 `collision.bmp` 一個遠離建築的 4×4 tile 區塊改為 0 或 255，測單位能否步入；不要與 vertex 或其他資料同時修改。
4. 對每個 `DATA/*.dat` 使用「只複製／暫時移除／啟動」測試，記錄遊戲是否重建、載入失敗或無變化。未知檔案不納入編輯器寫入清單。
5. 每個實驗都以新增的自製圖為目標；不得手動改寫安裝目錄的原廠圖。

## 開放功能條件

- 高度筆刷：靜態語意已確立（寫入 `boden.bmp` 灰階、`emboss.bmp` 坡度最小二乘法擬合重繪、交易內刪除四項快取），須完成 modifier workflow 實作，並經使用者在遊戲內實機驗證（in-game runtime verification）高度起伏與 `FileRollbackScope` 還原功能。
- 碰撞筆刷：靜態語意已確立（0 為通行、255 為阻擋），須經使用者在遊戲內實機驗證單位阻擋與尋路行為。
- minimap 自動重繪：權威圖層已由靜態載入管線確立（`boden.bmp` 高度、`boden.txt` 材質），四項快取重建規則已明確，完成離線重繪實作並經實機確認。
- SDL 疊圖：須以至少三個明確地標校正並驗證世界到像素轉換。
