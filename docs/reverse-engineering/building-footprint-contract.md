# 建築與物件地面占用契約

2026-10-25，Codex。唯讀資料與零售 EXE 靜態分析；未執行遊戲，未改編輯器。

## 結論與信心

**已找到可驗證來源：建築 APT（`APAT`）資產的群組內逐格 `(value, x, y)` 清單。** `objdef.dau` 第 14 欄 `aptix` 選擇 APT，`SYSTEM/cl_apt.ini [AptNames]` 提供檔名，`clengine.ini [AptPath]` 指向 `SYSTEM\DATA\APT\`；目前三個實例皆位於 ZIP 容器 `apt.dat`。不是從圖像透明度、陰影、選取半徑或單一固定邊距推算。

信心：**資料格式、三個實例及原生占用寫入鏈為高；標準投影下數字換算為高；完整尋路／建造可行性為部分驗證。** 這裡的 footprint 是「原生寫入占用格區的 stencil」，不表示清單內每格在所有單位、狀態下都永久不可走。尚未取得實機驗收。

## 證據與重現

- 已讀 [collision-passability-contract.md](collision-passability-contract.md)、[objdef-fields.csv](objdef-fields.csv)、[objdata-contract.md](objdata-contract.md)、[object-create-contract.md](object-create-contract.md)。原有 footprint 未驗證結論由本文件補充，並未改動原契約。
- 既有 Rust：`tools/re/ar-re-tools-rs/src/cmd/defaults.rs`（DefaultPath、覆蓋順序）、`cmd/zip_assets.rs`、`cmd/pua.rs`；`native/against-rome-data/src/{lzss,apt,apt_geometry}.rs`。既有 APT parser 把第一類每筆 12 bytes 當作未解釋資料略過；它的 `attachment_groups` **目前不暴露此 stencil**。
- 新增研究探針僅存 `re_workspace/footprint-probe/`，重用 `against-rome-data` PFIL、APT、SHA256 與 ZIP reader；不擷取或修改遊戲資產。它解析三個 APT 的全部 50 個群組，檢查每組投影後無重複格，再把跳過群組資料後的 frame table 與既有 APT parser 的每筆 frame 交叉比對，全數通過。
- 指令：使用 repo-local `CARGO_HOME=re_workspace/rust-runtime/cargo`、`RUSTUP_HOME=re_workspace/rust-runtime/rustup`；`cargo check`、`cargo test`、`cargo build --offline --manifest-path re_workspace/footprint-probe/Cargo.toml --target-dir re_workspace/footprint-target`（三者均加 `--offline` 與相同 manifest/target 參數）。check、test、build 均完成；test 為 0 個單元測試，實際資料驗證是執行 `re_workspace/footprint-target/debug/footprint-probe.exe`，exit 0。沒有執行產品 workspace 測試。
- 原始輸出：`re_workspace/footprint-numeric.txt`、`footprint-decoded.txt`、`footprint-clapt.txt`、`footprint-pua.json`。既有 Ghidra project `re_workspace/collision-contract-ghidra/CollisionContract`，以 `-process Against_Rome.exe -readOnly -noanalysis` 配合未修改的 `tools/re/GhidraFeatureVerification.java` 定向查詢；`footprint-ghidra.log`、`footprint-apt-native.log`、`footprint-transform.log` 分別完成 13、6、8 個函式（以各 log 完成標記為準）。Java 將 `JAVA_TOOL_OPTIONS` 訊息送至 stderr，PowerShell 因而回報 NativeCommandError／非零；三份 log 的腳本完成標記與指令內容才是成功證據，不以外層 exit code 聲稱分析成功。

本輪輸入 SHA256：

| 輸入 | SHA256 |
|---|---|
| `Against_Rome.exe` | `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf` |
| `apt.dat` | `cb8656a5f74cefb2588a89d1eb25566cb4493a22a4c52e30e37ec15cfcf917df` |
| `SYSTEM/clengine.ini` | `4c84f17ad5ddad3bbbf4fb25b9653d90d6968881942beaa29b2b37e03bd17c02` |
| `SYSTEM/cl_apt.ini` | `bffa9be2889e03d2965df285feeb453eed891490fce1f3fb77bfac344f0c5db6` |
| `SYSTEM/DATA_MP/DEFAULTS/objdef.dau` | `4471856e6354d73198e12ae79d99abb038949dd460c59df4371272d2340745fe` |

## 原生資料鏈

| 入口／位置 | 本輪確認 |
|---|---|
| `0x4b0300` 與表頭 | 定義輸出順序對應 zero-based 第 14 欄 `aptix`；實際 signed16 位址 `0x00c648de + type*0x2a4`。`0x4d7d80` 以 base-2 dword 再 `SAR 16` 取它。不能把 `0x00c648dc` 當有效欄位起點。 |
| `0x4c5e00` | 依 APT index／檔名與 prefix 載入，呼叫 `0x4e5710`；失敗有 `error.apt` fallback。 |
| `0x4e5710` | 讀 APAT、群組表及每組清單；第一類 count=N，讀 N 次 u32 value，再讀兩個 32-bit 座標。runtime group stride `0x4c`；count `+0`，value array pointer `+0x14`，point array pointer `+0x18`。 |
| `0x4e6ec0` | 依 gfxtype 的三個 selector 取得群組；第 1、3 selector 組合選群組，第 2 selector 選 light variant，不可無條件使用 group 0。索引超過維度時夾到最後。 |
| `0x4d7d80` | 取 position 與 gfxtype，選群組；投影 anchor 與每筆 point，逐格寫入 `0x01de2c50`（物件 slot）及 `0x01e02c50`（value 低 16 bits），範圍限定 0..255。不寫地形 `0x01e22c50`。建立路徑 `0x4aa780` 及更新路徑 `0x4b7e50` 均有 caller。 |
| `0x4d7ff0` | 同一 APT 清單反向清除物件占用，證實這是 runtime stamp，不是圖像尺寸表。 |
| `0x4d85c0` | 另一種單格占用：定義 `coll1` 的 signed16 實際位址 `0x00c6491a + type*0x2a4` >0 時，只寫所在格。**不能將 `coll1=0` 解讀成建築沒有 footprint。** |
| `0x4da370` | 檢查地形門檻、目的格 owner 與其 runtime flag，另在占用 value 非零時比較 `0x49c500` 的相鄰高度差。單純將 APT value 對照地形 collision 門檻並不等價。 |
| `0x4d8560` | 僅占用值 >1 才回傳 owner；顯示 value=1 外圈與內部值存在消費差異，不宜全改成同一布林。 |

value 的正式名稱、所有數值／旗標語意未完整命名；20、64 等原值不應擅自稱為半徑、格寬或永久阻擋等級。`0x49c500` 從兩格中心 `(64*x+32,64*z+32)` 取樣後回傳高度差絕對值。

## APT 磁碟格式與座標

以下 offset 是 **ZIP member 解壓後的 APAT bytes**，不是 `apt.dat` 容器 offset，也不是 runtime 結構 offset。數值為 little-endian 32-bit；座標按 signed32 解讀。

1. Header 共 28 words／112 bytes；`+0x00="APAT"`，`+0x04=version`，`+0x0c=T` 為 row tables 起點；`+0x20/+0x24/+0x28` 為三個 selector 維度。三個樣本皆 version 3、T=`0x70`、維度 `(5,2,5)`。
2. T 起先放 31 個 row offsets、31 個 row widths，共 248 bytes。令 B=T+248；B+0/+4 為 projection origin `(Ox,Oy)`；B+8/+12 為 anchor `(Ax,Ay)`；B+16/+20 是兩個 extra point counts；B+24 為 group count。
3. Group headers 起點 B+28。v3 每組 `(6+variant_count)*4` bytes：`marker_x,marker_y,N,M,L[variant_count],S,Q`。v2 為 7 words，只有一份 L；本輪實例均 v3。N 是本次 footprint 清單長度，M、L、S、Q 是後續其他清單的筆數。
4. 所有 group headers 後，先略過兩個 extra lists 的 `(extra_a+extra_b)*8` bytes；每群組依序讀 N 筆 **12 bytes：`value:u32, x:i32, y:i32`**，接著 M*8、sum(L)*8、S*8、Q*16 bytes，再進下一組。不是「所有 value 後接所有 points」；磁碟交錯、runtime 分離。
5. 標準投影（`0x4e5420` 的 `0x626928 !=0` 分支）：令 `dx=x-Ox,dy=y-Oy`，`P(x,y)=(floor((32*dy+16*dx)/1024),floor((32*dy-16*dx)/1024))`。listing 核對負數除法與負餘數修正，需向負無限取整；不能用 Rust signed `/` 直接代替。另有可設定係數分支，本輪未驗證其實際啟用時機。
6. `0x49c2a0` 把浮點 world 經 `0x419c80` 轉整數再 `SAR 6` 得到中心格 G；`0x419c80` 的精確小數邊界／FPU 模式未在本輪補全。每筆 stamp 在 **G + P(point) - P(anchor)**，不是直接把 sprite bounding box 換算成地面面積。
7. 256×256 格，stride 每列 `0x200` bytes、每格 signed16，即每軸 16384 world units、**每格 64×64 world units**。形狀外接框 W×H 格代表 W*64×H*64 的格域包絡，不等於以中心對稱的半徑；不規則孔洞及偏心 anchor 都必須保留。

## 三個數值實例

皆使用 **group 0**、未平移的相對格；不是所有建造／動畫狀態的通用結果。三個 type 的 `coll1=0`，足跡由 APT 提供。

| type／名稱 | aptix → member（`SYSTEM/DATA/APT/`） | group 0 清單 offset／筆數 | 相對 X、Z inclusive | 外接格域／world units | value>1 子集 |
|---|---|---|---|---|---|
| 232 `BauGerHau00_Haupthaus` 德國主屋 | 9 → `gerhau00.apt` | `0x804`／101 格 | X=-5..7；Z=-5..3 | 13×9／832×576 | 48 格，X=-4..2、Z=-4..2 |
| 37 `BauGerWoh00_Wohnhaus` 德國住宅 | 1 → `gerwoh00.apt` | `0x7ec`／36 格 | X=-4..1；Z=-3..2 | 6×6／384×384 | 12 格，X=-3..0、Z=-2..1 |
| 34 `BauGerBau00_Bauernhof` 德國農場 | 0 → `gerbau00.apt` | `0x7ec`／49 格 | X=-4..2；Z=-4..2 | 7×7／448×448 | 24 格，X=-3..1、Z=-3..1 |

Group 0 值分布：主屋 `{1:53,20:35,63:2,64:2,65:3,95:2,96:3,97:1}`；住宅 `{1:24,20:7,32:1,63:2,64:1,65:1}`；農場 `{1:25,20:17,61:1,64:4,65:2}`。value>1 子集僅是已驗證 getter 的分類，**不是另行證實的建築施工禁放範圍**。主屋 group 4 仍為 101 筆，但值改為 `{1:52,25:49}`；groups 25..49 的 N=0，不能把非零群組的聯集冒充所有狀態共用足跡。

相對格示意：每列從 X 最小值向右；`1` 為 value=1、`#` 為 value>1、`.` 未列入清單；這是數值清單的圖示，不是碰撞遊戲截圖。

```text
主屋 Z=-5..3      住宅 Z=-3..2   農場 Z=-4..2
111111111....     111111         1111111
1#######1....     1####1         1#####1
1#######11111     1###11         1#####1
1#######11111     1####1         1####11
1######111111     11#111         1#####1
1#######11111     111111         1#####1
1#######11111                    1111111
1#######1....
111111111....
```

Member SHA256：`gerhau00.apt=a2a57992a0c89a23a35ea580d92f039b4013e42b88109da44323887b4d9e29d2`；`gerwoh00.apt=6c596a0025190d205d599efb468b8004f90a42b685a16f475b10d46666a9e262`；`gerbau00.apt=578ff6b135b2c112cdae63a7ab1701eab6617b18395f72801111951c08ec3884`。

塔樓限制：`cl_apt.ini` 有 index 46 → `romtur00.apt`，但本輪安裝版 loose objdef 沒有引用 aptix 46 的有效資料列；`apt.dat`／`mp.dat` inventory 沒有該 member。既有 Rust `probe-pua` 確認 `cl.pua` 的 169 個 members（CRC 全通過）無 APT／DAU。沒有可驗證塔樓 numeric footprint，故提供三個實際存在的建築例子，不能採用設計文件中的假定 2×2。以上只是明列容器與 loose 來源的查詢，不宣稱已排除所有虛擬資源後端。

## 對編輯器的建議與未驗證範圍

- `apps/against-rome-editor/src/navigation.rs` 的 approximate 模式仍是固定主屋 384、其他 family 64 的外擴距離。下一步應新增唯讀 APT stencil reader，沿 effective objdef → aptix → AptNames → AptPath 解析，保留 source hash、群組與 raw value；從 DATA 的 gfxtype selectors 選當前群組。資產缺失或 selector 未知時明示 fallback／近似，不能宣稱 native parity。
- 路線估計可先在暫存 occupancy 圖層中蓋入選定 stencil，保留 owner/value，與地形 collision 分開；若產品只能接受 bool，可把整個已列 stencil 當保守障礙，但應明示是估計，並保留 value=1 外圈可切換的診斷。不以 `value>=6` 取代原生 owner、邊高度差與單位門檻邏輯；不改寫原始 collision.bmp。
- 放置重疊先測兩個實際 stencil 的世界格交集；把道路／額外 clearance 當獨立設計者設定，不能繼續用同一 family 方框宣稱原生禁放。value>1 核心與 value=1 外圈可分層回報，但原生建造是否允許外圈重疊、入口／門是否須額外保留、各狀態／方向如何選 selector，仍需定向 consumer 與實機驗證。
- 尚未完整驗證：所有建築／裝飾與單格路徑的分類、動態重疊寫入次序與刪除恢復、各 raw value 的語意、標準投影係數切換、FPU world 轉格邊界、旋轉／施工狀態、完整建造合法性及真正遊戲內走路效果。沒有編輯器程式碼變更，也未聲稱完成此產品接線。
