# 真實遊戲場景編輯：原生資源與引擎入口

2026-10-07 Codex。使用者要求像《世紀帝國 II》地圖編輯器，在真實遊戲畫面直接操作地圖。現有 OpenGL 地形與物件標記不滿足此要求。

## 本輪證據範圍

初始靜態分析只讀 repository 既存 `re_workspace/Against_Rome.exe`，SHA-256：
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
當時未存取安裝目錄；後續在使用者授權下唯讀複製素材（見下文），未啟動或注入遊戲。以下 x86 指令是靜態證據，尚非實機 API 契約。

`tools/re/scan_native_scene.py` 可重現字串引用、指定地址引用及指定 VA 範圍的反組譯。需要 `pefile`、`capstone`；依賴路徑應由呼叫者設定。輸入檔必須明確指定，不預設讀取安裝目錄。線性反組譯與引用是候選，不能單憑掃描推定函式邊界或 ABI。

```powershell
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe `
  --target 0x4E3A20 --target 0x4E4170 --target 0x4E5710 `
  --range 0x4E3A20:0x4E3CF0 --range 0x4E5710:0x4E5DB0 `
  --range 0x4AF270:0x4AF370 --output "$env:TEMP/arm-native-scene.txt"
```

原有 Ghidra 路徑已不存在；TEMP 留存 `ghidra.zip` 實際無法以 ZIP 開啟，不能沿用為正常工具。這次用既有 TEMP Python 依賴及 Capstone 完成分析，未下載／安裝新工具。

## 已核對的原生路徑

| 位置 | 直接可觀察的行為 | 解讀限制 |
| --- | --- | --- |
| `0x4AEC56`、`0x4AECBF` | `.alr` 資源／`error.alr` 備援呼叫 `0x4E4170`，結果存入以 ID 索引的指標表 | 備援載入不代表完整場景可獨立初始化 |
| `0x4E41A9` | `0x4E4170` 呼叫 `0x4E3A20` 讀取資源 | 包含後續轉換，不可直接把回傳指標當檔案原始布局 |
| `0x4E3A99` | 比較 `0x41524C41`，little-endian 為 `ALRA` | 魔數不是檔案副檔名本身 |
| `0x4E3AFD` | 版本 unsigned `> 6` 進錯誤路徑 | 舊版有條件欄位，不能固定套用 v6 布局 |
| `0x4E3B4C`、`0x4E3B6E`、`0x4E3B8E` | 版本分別 `> 1`、`> 2`、`> 3` 才讀取額外欄位 | 欄位全部語意尚未確認 |
| `0x4E3D2F..0x4E3DA5` | 讀 signed 索引；非負引用既有記錄，負值轉往 `0x4E402B` 建立新記錄 | 必須保留共享記錄關係，不能當獨立 BMP 串流解碼 |
| `0x4E407F..0x4E4092` | 新記錄的 packed word 拆成 `(word >> 11) & 0x7FF`、`(word >> 22) & 0x1FF`；後面讀取資料及對齊 | 頂層 header 與 runtime 指標不可混用；像素編碼／透明語意未完成 |
| `0x4AF270..0x4AF336` | 檢查 index 小於 runtime `+0x04`，取得 `+0x5C` 指標表；packed `+0x08` 的低 11 bit／接續 11 bit 作矩形寬高；依 camera scalar 轉得的 shift 縮放並輸出四個界限 | 這是**範圍計算**，不是已找到可直接呼叫的繪圖函式 |
| `0x4AF2A8..0x4AF2EA` | packed `+0x04` 的兩個 16 bit 分量與 runtime `+0x2C`、`+0x30` 參與矩形位置 | 需樣本確認物件錨點、朝向、動畫格映射與地形投影 |
| `0x4C1FEF`、`0x4C2102`、`0x4C250B`、`0x4C2587` | 呼叫上述 ALR 範圍函式 | 已找到消費端位置，未證實哪個能作完整場景拾取入口 |
| `0x4C5E64`、`0x4C5EA7` | `.apt`／`error.apt` 路徑呼叫 `0x4E5710` | APT 與 ALR 有不同讀取流程 |
| `0x4E57AE`、`0x4E57C9` | APT 比較魔數 `0x54415041`（`APAT`），版本 unsigned `> 3` 拒絕 | 尚未確立完整 APT 幾何／動畫布局 |

ALR 附近存在 `CLUS\us_rdani.c` 字串，APT 附近為 `CLUS\us_rdpat.c`。ALR runtime 明確含逐格矩形與尺寸資料；因此舊文件將 `*.alr` 全數概稱為「模型」不能當作已確認的 3D mesh 格式。尚不能據此宣稱所有遊戲物件都是 sprites，或宣稱 APT 就是三角網格。

## 8-bit ALR 行解碼（2026-10-07）

繼續追蹤 runtime `+0x5C` 指標表，已找到 `0x4E48F0` 的 frame 資料 helper、`0x4E49C0` 的 row helper，以及 `0x4E4A60` 的逐行繪製。`0x4CA0CA`、`0x4CA41C` 等消費端呼叫 frame helper；不能把這些位置單獨視為可嵌入的完整 renderer。

```powershell
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe `
  --target 0x4E48F0 --range 0x4E48F0:0x4E4A60 `
  --range 0x4E4A60:0x4E4E2F --output "$env:TEMP/arm-native-alr-rows.txt"
```

`0x4E49C0..0x4E4A58` 直接顯示以下 row descriptor 語意：

- bits 0–19 是資料 byte offset；下一個 descriptor 的低 20 bits 為終點，所以 height 列需 height+1 筆。
- bits 21–30 是列首透明像素數。bit 31 控制第一段的 index 0 是否照 palette 寫入。
- bit 20 未設時，整段為一段 indexed pixel bytes。
- bit 20 設定時，payload 前兩個 bytes 是「第一段 pixel 數」、「中間透明 gap（低 7 bits）＋第二段 index 0 的不透明旗標（bit 7）」。餘下為兩段連續存放的 pixel bytes；第二段長度為整列 payload 長度減 2 減第一段長度。
- `0x4E4C59` 分派第一段不透明路徑，`0x4E4CC3..0x4E4D1E` 再切到第二段；`0x4E4DAE..0x4E4DCF` 證實非不透明路徑的 index 0 略過 destination，不是固定 palette RGB 的 color key。

`src.MapEditor.Modules/NativeAssets/NativeAlrIndexedFrame.cs` 現在把**已抽出的 8-bit 行資料與選定 palette**解碼成自有 ARGB pixels；gap 與透明 index 0 保留為 alpha 0，寫入的 palette RGB 為 alpha 255。兩段各自使用旗標，不能把零號色一律當透明。對不合法 offset、前綴、palette index 或超出 frame 的 run 拒絕解碼。

`NativeAlrIndexedFrameTests.cs` 使用非對稱合成行資料驗證逐像素結果、兩段 opacity、palette 高 byte、快照隔離、對齊 padding 與損壞輸入。實際 indexed container 解碼的後續驗證見下節；方向／動畫選取、非 8-bit 分支、真正場景渲染及 UI 接線仍未完成。

## ALRA 容器的 v4–6 indexed 讀取

`NativeAlrDocument.Parse` 現在讀取 v4–6、8-bit 的 header 與 frame table，接 `DecodeFrame` 產生 pixels。這些版本在 native loader 使用相同讀取順序；v4/v5 的 allocation 欄位有後處理，未將其誤用為檔案長度。

```powershell
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe `
  --range 0x4E3AEB:0x4E3DA7 --range 0x4E402B:0x4E4145 `
  --range 0x4E48F0:0x4E49B4 --output "$env:TEMP/arm-native-alr-container.txt"
```

header 共有 23 個 32-bit little-endian words：magic、version、unknown、frame count、bits per pixel、unknown、layout columns、layout rows、palette variant count、unknown、5 個 unknown、anchor width／height、4 個 allocation 欄位、2 個 pre-frame 欄位。Unknown 欄位尚未命名為方向／動畫語意。

每個 frame 先讀 signed reference；非負值共享較早的記錄，負值讀新記錄。新記錄依序為 row table 相對 offset、packed X/Y、packed width/height/palette count、pixel byte count、各 variant 的 palette DWORDs、補齊至 4-byte 邊界的 pixel bytes，以及 height+1 個 row descriptors。

- `0x4E4035..0x4E4052` 將 row table 指標設為記錄起點＋12＋stored offset；`0x4E40D4..0x4E4140` 證明 palette／padded pixel bytes 後緊接 row table。
- `0x4E499F` 回傳的是 runtime frame+12。原先將此直接套用到磁碟布局的推導已由實際素材否定：檔案 row offsets 從全部本地 palettes 後的 pixel bytes 起算，DecodeFrame 必須先跳過 palette prefix。Runtime wrapper 的完整轉換語意仍待追蹤。
- `0x4E48F7..0x4E4913` 從**第一筆 frame**取 palette，再依 variant index 偏移；選到其他 frame 時不改用該 frame 的本地 palette。Parser 保留共用記錄，不重複複製 payload。

`NativeAlrDocumentTests.cs` 對 v4/v5/v6 合成容器驗證 pixels、palette 變體、不同／缺少本地 palette、直接／間接共用記錄、snapshot 隔離、每個 truncated prefix、錯誤 reference／offset／extent／byte count、明確拒絕不支援的版本與 bit depth。尾端 IFOM metadata 尚未解讀；layout 與 anchor 值只保留原值，未映射物件方向／動畫。

**2026-10-07 實際素材驗證**：使用者允許唯讀素材並複製到 TEMP。alr.dat SHA256 `54FE9E951F5038A29690718100146A6FF740BFDFA334DCA1C10406FDF3BED894`，原檔與副本一致。初版失敗 1662 個項目；修正 palette prefix 與合法零尺寸格後，2,075 個文件、358,083 格、含全部 palette 變體 2,921,730 次解碼皆成功，其中 2,374 個零尺寸格。這是結構／解碼完整性證據，仍非場景外觀與動畫方向驗收。已檢視 fialgesc00 首格透明 PNG，尚未與同格遊戲畫面比較。

可重現的唯讀工具（輸出必須為尚不存在的 TEMP 子目錄；報告與 RGBA 不加入 Git）：

```powershell
$env:DOTNET_ROLL_FORWARD='Major'
dotnet run --project tools/re/alr-probe -c Release -p:UseAppHost=false -- `
  "$env:TEMP/ArmNativeAssets_20261007/alr.dat" "$env:TEMP/ArmNativeAssets_20261007/new-report"
```

唯讀 PFIL 解碼 objdef.dau、SYSTEM/cl_alr.ini、SYSTEM/cl_apt.ini 顯示：objdef 欄 5 `alrid` 對應 `[AlrNames]` ID，欄 14 `aptix` 對應 APT ID。例：type 175 `LanGerNabu05_Nadelbusch` → ALR 206 `lagenabust05.alr`；type 42 `BauGerHau02_Haupthaus` 的 alrid=-1、aptix=6 → `gerhau02.apt`。因此不能只接 ALR 就宣稱完整建築場景。全庫 probe 已匯出並檢視 gersch01、lagenabust05、lagestgr00 首格；未核對實際遊戲相同方向／動畫格。其餘 config 欄位語意尚未驗證。

低版本、非 8-bit、footer、APT 與場景 UI 仍需完成或確認。

## APT v2/v3 diamond patch 解碼（2026-10-07）

`NativeAptDocument` 以自有 bytes 讀 APAT，`DecodeTile` 產生 64×31 ARGB diamond，`DecodeFrame` 依索引表在原 canvas 合成。它是 raster patch 格式，不是將建築當成一般三角網格。222 個實際檔案均為 8-bit、64×31，221 個 v3、1 個 v2；其他布局明確拒絕。

- Header 28 DWORDs／112 bytes；word2 是線性狀態數，word3 指向表格起點，word4 是 tile 數；word6／7 是 tile 寬高，word8..11 保留四維索引大小，word9 也是群組 variant slot 數，word13 是 depth；word20／21 是 palette colors／variants，word23／24 是 canvas 寬高，word27 是 pixel block byte 數。
- `0x4E5951` seek header offset；接兩個 height 長 DWORD 表（raw row offsets、diamond row widths），兩對位置／anchor 值、兩個額外 pair counts、group count。目前保留 native renderer 使用的第二對 anchor（runtime +0x78/+0x7C）。群組有兩個位置值、兩個 count、每個 variant 的 count、最後兩個 count；v2 只存一份 variant count／list，native loader 複製其他 slots。
- 額外 pair lists 後依各 group 讀／跳過 triples、pairs、per-variant pairs、pairs、quads；語意仍未命名，不當成頂點或任意忽略其長度。之後是狀態→(first tile,count) 表、palettes、tile metadata（offset、packed XY、四個 auxiliary bytes）、`PDAT`、pixel block。IFOM／footer 尚不解讀。
- `0x4E6B40` 的線性索引為 `(((a * size1) + b) * size2 + c) * size3 + d`；`0x4E6B80` 使用狀態表取得連續 tile 範圍。各軸的建造／方向／光照／動畫產品語意仍須核對，不能把 frame1000 的觀察概括為全部資源的完成狀態。
- Tile packed XY 的低11 bits／接續11 bits 是位置。Pixel block 每塊前8 bytes 是 row skip mask／index0 opacity mask；skip mask bit31 表示 compressed。Raw rows 使用 header 的 byte offsets／row widths；compressed 的32個 ushort 位於塊+8，start/end 低11 bits 是相對塊+8的 byte offset，`(start >> 9) & 0x3C` 是 row gap。Native `0x4E6CD0`／`0x4CC1F1..0x4CC319` 顯示 row skip、獨立 opacity 與 palette 寫入。
- 1:1 合成 x 為 `packedX + gap + 2 - fullDiamondRowWidth/2`，y 為 `packedY + row`；index0 只在該 row opacity bit 設定時寫色，否則保持已合成的底層像素。解析檢查變長輸入／count／tile reference，解碼檢查 row table、palette index 與可見像素範圍。

`NativeAptDocumentTests` 合成 v2/v3 容器涵蓋變長群組列表、raw／compressed／gap、兩個 palettes、透明覆疊、所有 truncated prefixes、錯誤引用／counts／offsets、unsupported formats、快照隔離。ARGB 目前沿 ALR 的低24 bits 慣例；色道、team palette 和原遊戲同格色彩未核對。

唯讀全庫 probe（输出必須為新 TEMP 子目錄）：

```powershell
$env:DOTNET_ROLL_FORWARD='Major'
dotnet run --project tools/re/apt-probe -c Release -p:UseAppHost=false -- `
  "$env:TEMP/ArmNativeAssets_20261007/apt.dat" "$env:TEMP/ArmNativeAssets_20261007/new-apt-report"
```

apt.dat SHA256 `CB8656A5F74CEFB2588A89D1EB25566CB4493A22A4C52E30E37EC15CFCF917DF`；副本與原檔一致。全庫 222 documents／500507 palette tiles／103601 palette frames／0 failures。TEMP/ArmNativeAssets_20261007/apt-verified-1 存 JSON、gerhau02 frame0／1000 的 RGBA／PNG；已檢視施工框架和完整主屋，但未與原遊戲同方向／光照畫面比較，亦未接 UI。

完整靜態範圍報告只存 TEMP，使用同一 EXE 可重建：`0x4E5560:0x4E5710`、`0x4E5BA8:0x4E6010`、`0x4E600A:0x4E6500`、`0x4E64CD:0x4E66B0`、`0x4E691D:0x4E6AA0`、`0x4E6B40:0x4E6D90`、`0x4CC080:0x4CC390`。用 `scan_native_scene.py --range ... --output <TEMP report>`；線性反組譯對齊須依有效函式入口核對。

## 編輯器場景接線與素材語意（2026-10-07 Claude）

以 TEMP 素材副本解碼後目視核對，得到以下可重現結論（scratch contact sheet，未提交素材）：

- **色道**：palette DWORD 為 `0x00BBGGRR`（紅色在低 byte）。未交換時 gerhau02 火把為青色、茅草為藍色；交換後火光黃、茅草黃褐。`NativePaletteColor.ToArgb` 由 ALR/APT 共用。
- **ALR 單位方向**：`LayoutColumns` 是每方向格數（步兵 24）、`LayoutRows` 是方向數（步兵 16、騎兵 32）。gersch01 的 frame 0,24,48… 依序旋轉一圈，frame 0..23 為同方向動畫，故 frame = `((動畫 × 方向數) + 方向) × LayoutColumns + 格`；方向 0 面向鏡頭。frame count / (rows × cols) = 動畫數（gersch01 為 17）。
- **ALR 錨點**：`AnchorWidth/Height` 是畫布大小，地面接觸點為畫布中心；frame 以 OffsetX/Y 置於畫布中。
- **雙層 ALR**：objdef 欄 8 `alrml` 是第二層 ALR、欄 10 `mltyp` 為類型。樹木（mltyp 2）alrid 是樹幹（lagenast）、alrml 是樹冠（lagenakr），兩者共用畫布錨點，樹冠疊在上層；樹冠 frame 0 為完整綠冠，1–3 為枯萎階段、4 為空白。只有 alrml 的物件（草、蘆葦等 mltyp 1/4–12）目前單獨顯示該層；這些「_M／_H」類型是否原生為多重散佈未驗證。
- **APT 四軸**：gerhau02 layout 5×2×5×25。軸 0 是建造階段（0 為地基、4 為完工），軸 2 是損毀程度，軸 3 是動畫格；軸 1 目前看兩值外觀相同（語意未確認）。完工完整外觀 = `(L0−1) × L1 × L2 × L3`（gerhau02 為 1000）。
- **palty**：欄 17 為 1 的單位有 9 個 palette variants，目前以隊伍編號直接當 variant（team 1 呈紅色系），未與遊戲內隊伍色比對。

`NativeSpriteCatalog` 依上述規則把物件名稱解析為裁切後的靜態 sprite，對 objdef 全 2159 筆：1711 筆成功、391 筆本身無 ALR/APT、57 筆為首格空白的特效（FX）。`NativeSpriteAtlas` 打包為單一貼圖；`Map3DViewControl` 以螢幕對齊 quad、由遠到近不做深度測試繪製（與原 2.5D 畫法相同），沒有 sprite 的物件仍畫標記點。預設相機改為 yaw 45°、pitch 35°，使地圖 +X 往右下、+Z 往左下，與原遊戲等角方向一致。sprite 尺寸以「APT 菱形 64 px = 一個地圖像素格對角線」估算（`SceneObjectRenderer.TilesPerSpritePixel`），尚未用遊戲截圖量測。

限制：地形不遮擋物件（山丘後的物件仍畫在前面）；動畫、角度→方向、陰影（shad.dat）、隊伍色、草地等多重散佈物件、2D 畫布 sprite、可見物件拾取尚未完成。

## 遊戲內同畫面驗證（2026-10-07 Claude，使用者授權啟動遊戲）

以 1024×768 遊戲（ENDL_005「Claude Test 1b」、日耳曼、主屋視角）無損截圖，對解碼後的 sprite 做正規化互相關（NCC）樣板比對，工具在 scratch，未提交：

- **APT 完工格**：gerwoh00 frame 40（`(L0−1)·L1·L2·L3`）NCC 0.998；gerhau00 frame 1000 NCC 0.918（火光動畫差異）。
- **色道**：逐色道增益擬合，`0x00BBGGRR` 殘差 0.0028，另一序 0.0119。遊戲另有日夜光照增益（白天約 R0.70/G0.89/B0.87，黃昏約 R0.48/G0.82/B0.83），編輯器目前未套光照。
- **錨點與比例**：主屋 (10624,10112) 與住宅 (11392,10112) 的 APT 錨點在畫面上為 (512,340)、(896,532)，位移 (384,192)：每世界單位沿地圖 +X 為 (0.5, 0.25) px，即 2:1 正交等角（俯角 30°），地圖格 64×32 菱形，sprite 1 px = 1 畫面 px。錨點就是物件座標。
- **隊伍色**：腳本生成的隊伍 0 步兵（gerinf01）9 個變體中變體 0 殘差最低（0.050，其餘 0.080–0.233）→ 變體 = 隊伍編號。
- **方向**：腳本 Angle 0 生成的步兵為方向列 14（NCC 0.92，其餘 ≤0.51）；往畫面右方移動後停下為方向列 1。兩點加上隊形與 22.5° 量化誤差，不足以推出一般角度公式。

編輯器因此改用正交投影、俯角 30°、單位預設方向列 14，並提供 `EditorCamera.ZoomToGameScale`。以同一地圖副本開編輯器、1:1 縮放擷取後再比對：主屋與住宅錨點位移同為 (384,192)，NCC 1.000。

## 下一個實作與驗收點

已取得允許分析的素材副本與 ALR/APT 解碼核心；下一步接宿主物件定義與素材名稱對應、動畫／方向映射、色彩與錨點，對照同物件的原遊戲畫面。啟動遊戲仍未包含在本次唯讀授權中。

同時追蹤 ALR/APT 消費端、原引擎相機與地圖重建接口，評估原引擎中直接編輯的可行性。範圍函式不能代替場景渲染；單張遊戲截圖或「儲存後另開遊戲」也不能算即時編輯完成。原引擎模式入口、thread/context、座標拾取、物件新增刪除與高度／材質即時重建均未驗證。

完整目標必須以地形、樹木、建築、單位的實際遊戲外觀與操作驗證：筆畫／放置／移動／刪除立即反映畫面，點選命中可見物件，undo/redo 還原場景，儲存重開與遊戲讀取一致。目前仍未完成這些驗收。

## shad.dat 物件陰影（2026-10-07 Codex）

本輪只讀 repository EXE 與使用者指定的 TEMP 素材副本，未存取安裝目錄、未啟動遊戲、未接 UI、未提交或修改 Git 歷史。依本輪允許路徑限制，不更新 AI_HANDOFF.md；本節記錄研究與驗證里程碑。

### 容器與單張格式：已驗證

shad.dat 為 ZIP，30,480,435 bytes，SHA256
`C25FDCDB846A6FF6DEC6B4BF2C3B47EF46352841F1A7FF2A27A16ECA9DBF201D`。
共 2674 項：3 個目錄、2671 張 BMP；其中 415 張位於
`SYSTEM/DATA/SHADOWTEXTURE/<name>.bmp`（副檔名含大小寫）。
**沒有 .sha、ALRA 或獨立 shadow name list**。其餘 2256 BMP 是 ICONGFX、
ENGINEGFX 等資源，不能全當成陰影；2670 張為 8-bit、1 張為 24-bit。
EXE 未找到 .sha / error.sha 字串，shadow 載入路徑明確使用 BMP 與 error.bmp。

415 張陰影全為 128×128、正高度（bottom-up）、8-bit BI_RGB，沒有動畫 frame table；
每張文件是一格。393 張有 2-byte trailer，22 張沒有；全部有 256 個 RGBQUAD。
414 份 palette 為 0..255 的線性灰階；LakaDorn_shadow.bmp 全為 (1,1,1)。
後者不應讓遮罩變成一片固定強度，因 native 直接取 pixel index。

| 磁碟位置 | 欄位與驗證 |
| --- | --- |
| +0..13 | BITMAPFILEHEADER：BM（0x4D42）、bfSize、reserved、bfOffBits；bfSize 必須等於輸入長度，reserved=0 |
| +14..53 | 40-byte BITMAPINFOHEADER：signed 寬高、planes=1、bpp=8、compression=0、biSizeImage、解析度、biClrUsed、biClrImportant |
| +54 起 | biClrUsed 個 4-byte RGBQUAD；biClrUsed=0 表示 256；只驗證表範圍與 index，顏色不作陰影強度 |
| bfOffBits 起 | 每列 stride=(width+3)&~3，height 列；正高度由下往上，負高度由上往下；padding 不解碼 |
| pixel block 後 | bfSize 內的 trailer 保留長度，內容不賦予動畫／anchor 語意 |

NativeShadowDocument 使用自有 top-down byte mask，不保留 caller 的可變記憶體，
DecodeFrame(0) 回傳隔離的遮罩／ARGB；其他 frame index 拒絕。
Parser 支援上述 BMP8 header 的一般尺寸與 top-down 合成 fixture；
實際素材只驗證了 128×128 bottom-up。不支援其他 DIB、depth、compression。
biSizeImage 只接受 0 或實際 stride×height，所有陰影符合；嚴格檢查 offsets、
palettes、dims、index、extent／overflow 與截斷。Trailer 不必為零，但必須在 bfSize 內。

### EXE 載入、遮罩與座標證據

同一 EXE SHA256 與前節一致。地址皆為 VA；線性反組譯仍須由有效入口核對，
不是可直接呼叫的 ABI。

| 位置 | 直接證據 | 結論／限制 |
| --- | --- | --- |
| 0x5F61A1、0x5F61B6、0x5F61CD | @SYSTEM\\cl_shado.ini、SYSTEM\\cl_shado.i%02ld、[ShadowNames] | ID 映射來自外部清單，ZIP 排序沒有映射意義 |
| 0x4C4410..0x4C4456 | 載入主清單，並迴圈載入 10 個分檔 | 同 ID 後載入可覆蓋；probe 的可選文字參數目前只接受一份合併／指定清單 |
| 0x4C44EE..0x4C456A | 前四字元按十進位組 ID，限制 0..1999；從 +5 起複製檔名到 40-byte stride 名稱表 0x019C93B0 | 第五字元是分隔符；不能把 objdef shidx 當 ZIP ordinal |
| 0x4C4706..0x4C4743 | 用 ID×40 取名，加 0x770EE8 路徑，呼叫 0x414AD0，存到 0x019DEB70[ID] | 一個 ID 對應一張 BMP，沒有方向／動畫 frame 選擇 |
| 0x4C47AE..0x4C4823 | 失敗時改用 error.bmp；0x414C00 取得 palette+pixels 區塊 | +0x400 是 palette 後的 pixel data；不是 BMP 檔案 bfOffBits |
| 0x4C38F3..0x4C3936 | 按 renderer mode 分派 0x40D8F0／0x40D380／0x40CE00 | 僅追蹤此路徑的 scalar 實作，不能聲稱全部 MMX/SSE 分支已逐指令驗證 |
| 0x40CE8F..0x40CEA2、0x40D195 | pixel base=texture+0x400，呼叫 0x404350 | 128×128 地面紋理取樣，而非螢幕 sprite 直接貼圖 |
| 0x410AAD..0x410B2C | 分派表指向 sampler 0x40FE80 及 blend 0x410380 | 可由兩者核對遮罩極性 |
| 0x40FEB0..0x40FEBD | 直接 movzx 讀 pixel byte 成 DWORD 強度，不經 palette | AlphaMask 保留 index 0..255，不採 RGB luminance |
| 0x410395..0x4103CD | 強度 0 略過；factor=256-strength，逐色道乘後 >>8 | RGB'=floor(RGB×(256-index)/256)，index 越大越暗 |
| 0x4C4398..0x4C4404 | 取主要 shadow ID 或另一個 shadow ID，再回傳 | sh2idx 與物件狀態可改變選擇；仍不是同文件的動畫格 |
| 0x4C42CE..0x4C42FB | signed word shsiz 作 float；shacz 加世界 Z，shacx 加世界 X，呼叫 0x4C31D0 | 修正量是世界 X/Z 單位，不能直接當畫面像素位移 |
| 0x4C32E6..0x4C332D、0x4C3522..0x4C35E9 | type 0 固定 4 點且做 square normalization；type 1 使用角度旋轉的圓周取樣 | shtyp=0 box、1 circle；不是貼圖 frame index |
| 0x4C3604..0x4C3664 | normalized X/Z 乘 shsiz，再加修正後中心 | shsiz 為世界空間 radius／box half extent；地形高度另參與投影 |

objdef loader 的相鄰 signed words 也核對了讀取順序：
0x4B1729→0xC648D8（shidx）、0x4B1761→0xC648DA（shsiz）、
接續 +0xDC（shtyp）、+0xDE（aptix）、0x4B1809→+0xE0（shacx）、
0x4B1841→+0xE2（shacz）；objdef runtime stride 為 0x2A4。
shtyp 語意依幾何分支確認，並非僅依 SHADOWTYPE_CIRCLE/BOX 字串猜測。

AlphaMask 的 native 分母為 **256**。黑色 ARGB 預覽用
alpha=round(index×255/256) 換成一般 alpha/255，會有量化差異；
NativeShadowFrame.DarkenArgb 才精確重現 scalar RGB 公式（保留 caller 的 alpha）。
例如 index255 並非 native alpha=1，仍留 1/256 RGB（8-bit floor 後為 0）。

### 三個物件：欄位與錨點已核對，檔名仍待清單

| objdef ID／名稱 | shidx | shsiz | shtyp | shacx／shacz | 相對物件畫面錨點的中心修正（1:1 平地） | 匯出候選，未確認 ID |
| --- | --- | --- | --- | --- | --- | --- |
| 42 BauGerHau02_Haupthaus | 259 | 300 | 0 box | -45／61 | (-53,4) px | GerHau02_shadow.bmp |
| 12 FigGerSch01_Axt_Schild | 1 | 25 | 1 circle | 0／0 | (0,0) px | round_small_shadow.bmp |
| 46 LanGerNad00_Tanne_gross | 205 | 75 | 1 circle | -15／-7 | (-4,-5.5) px | GerNadelbaum_shadow.bmp |

使用前節已核對的投影：ΔscreenX=(shacx-shacz)/2，
ΔscreenY=(shacx+shacz)/4（平地、同 camera zoom）。中心相對 ALR 畫布中心／
APT AnchorX,Y 所在的物件世界位置修正，**不是**相對 ALR 裁切矩形左上角。
坡地、shadh、其他物件狀態與高度偏移仍須核對。BMP 本身沒有 anchor 欄位；
不能把 128×128 遮罩當作 128×128 螢幕陰影。

目前提供的 TEMP 只有 objdef、cl_alr、cl_apt，repository 也未找到 cl_shado 副本；
ZIP 全項檢查確認沒有 name list。已向使用者詢問外部副本路徑，未存取安裝目錄。
因此上述三張只是依名稱／外觀選出的候選，**沒有證實 259/1/205 對應它們**。
sh2idx 選擇器的完整狀態語意、分檔覆蓋後的有效 ID 清單也未驗證。

已匯出並檢視候選原始 mask、綠底 native 混色及平地投影示意：主屋可見屋頂／
附屬結構長影，士兵為小圓影，冷杉為擴散影。紅十字是物件錨點，青十字為 shadow
中心。投影示意的 UV 軸向／circle 裁切／旋轉是假設，沒有疊物件 sprite，
未與原遊戲相同物件／角度畫面比較，不能算完整物件陰影驗收。

### 全庫 probe 與測試

```powershell
$env:DOTNET_ROLL_FORWARD='Major'
$output = Join-Path $env:TEMP ('ArmShadowProbe_' + [guid]::NewGuid().ToString('N'))
dotnet run --project tools/re/shad-probe -c Release -p:UseAppHost=false -- `
  "$env:TEMP/ArmNativeAssets_20261007/shad.dat" $output `
  "$env:TEMP/ArmNativeAssets_20261007/objdef.txt"
# 若已有授權的已解碼 cl_shado.txt，可加第四個參數，不能傳 PFIL bytes。
```

本輪報告與 PNG：TEMP/ArmShadowProbe_20261007_a。
2674 entries／415 documents／415 frames／3 directories／2256 other BMPs／0 failures。
所有 ZIP 項目皆打開檢查；非陰影 BMP 只驗證結構／palette index，沒有宣稱已解碼
其產品語意。非陰影有 160 筆 biSizeImage=16386 而 row extent=16384，
inventory 明列 imageSizeMatches=false；不套到全部陰影的嚴格 parser。
既有 output／TEMP 外輸出拒絕 exit2，report SHA256 不變；PNG 已由視覺工具檢視。
另以 Python 標準庫獨立驗證 13 張 PNG 的 CRC／inflate／尺寸，以及 81920 個 sample 像素；
可選 ShadowNames 參數通過合成清單 smoke，合成 ID 不作真實映射證據。

NativeShadowDocumentTests 新增 30 個案例，包含兩種列向、非4倍寬度的 padding、
palette 內容不影響強度、owned snapshot、所有 truncated prefixes（包括改寫
bfSize 以避開單純長度檢查）、壞 offsets／dims／extents／index、unsupported layouts、
零 colors／image size、合法 trailer、frame bounds、native 256 分母與 ARGB。

驗證：指定 Release solution build **0 errors／2 warnings**，兩者來自另一代理的
tests/AgainstRomeModifier.Tests/WinmmExportTests.cs（IDE0005、xUnit2029），依使用者
限制未修改。新 parser 的 CA1512 已改用 ThrowIfNotEqual；shad-probe 獨立 build
0 warnings／0 errors。指定 modules --no-build tests：167 passed／0 failed／0 skipped。
未達成整個 solution「0 warnings」要求，不隱藏或抑制其他代理警告。

可重建靜態範圍（先自行建立新的 TEMP 目錄，每份輸出取未存在檔名）：

```powershell
$env:PYTHONPATH="$env:TEMP/arm-re-python"
$reOut = Join-Path $env:TEMP ('ArmShadowRE_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $reOut | Out-Null
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe `
  --target 0x5F61A1 --target 0x5F61CD --target 0x019DEB70 `
  --range 0x4C4410:0x4C4457 --range 0x4C4470:0x4C45A4 `
  --range 0x4C46D0:0x4C4833 --range 0x4C4360:0x4C4407 `
  --range 0x4C4249:0x4C4303 --range 0x4C31D0:0x4C39BF `
  --range 0x40FE80:0x40FED3 --range 0x410380:0x4103DE `
  --output "$reOut/shadow-evidence.txt"
```

下一步需使用授權的 cl_shado.ini／分檔副本解 PFIL，再核對三個 ID 與 sh2idx，
確定貼圖 UV 與物件角度的關係、原生取樣／高度投影及同畫面外觀。此次只提供純解碼
與研究工具，沒有把陰影接到 2D／3D UI。