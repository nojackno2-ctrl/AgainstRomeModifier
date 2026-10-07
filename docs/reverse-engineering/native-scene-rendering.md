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

## 下一個實作與驗收點

已取得允許分析的素材副本與 ALR/APT 解碼核心；下一步接宿主物件定義與素材名稱對應、動畫／方向映射、色彩與錨點，對照同物件的原遊戲畫面。啟動遊戲仍未包含在本次唯讀授權中。

同時追蹤 ALR/APT 消費端、原引擎相機與地圖重建接口，評估原引擎中直接編輯的可行性。範圍函式不能代替場景渲染；單張遊戲截圖或「儲存後另開遊戲」也不能算即時編輯完成。原引擎模式入口、thread/context、座標拾取、物件新增刪除與高度／材質即時重建均未驗證。

完整目標必須以地形、樹木、建築、單位的實際遊戲外觀與操作驗證：筆畫／放置／移動／刪除立即反映畫面，點選命中可見物件，undo/redo 還原場景，儲存重開與遊戲讀取一致。目前仍未完成這些驗收。
