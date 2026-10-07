# 真實遊戲場景編輯：原生資源與引擎入口

2026-10-07 Codex。使用者要求像《世紀帝國 II》地圖編輯器，在真實遊戲畫面直接操作地圖。現有 OpenGL 地形與物件標記不滿足此要求。

## 本輪證據範圍

只讀 repository 既存 `re_workspace/Against_Rome.exe`，SHA-256：
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
未存取安裝目錄、未啟動或注入遊戲。以下是 x86 指令的靜態證據，尚非實機 API 契約。

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

## 下一個實作與驗收點

先取得允許分析的原始素材樣本，沿已定位載入器完成一種實際物件的解碼、動畫／方向映射、透明與錨點，對照同物件的原遊戲畫面。未有實際樣本前，不猜測像素編碼或以合成模型替代真實外觀。

同時追蹤 ALR/APT 消費端、原引擎相機與地圖重建接口，評估原引擎中直接編輯的可行性。範圍函式不能代替場景渲染；單張遊戲截圖或「儲存後另開遊戲」也不能算即時編輯完成。原引擎模式入口、thread/context、座標拾取、物件新增刪除與高度／材質即時重建均未驗證。

完整目標必須以地形、樹木、建築、單位的實際遊戲外觀與操作驗證：筆畫／放置／移動／刪除立即反映畫面，點選命中可見物件，undo/redo 還原場景，儲存重開與遊戲讀取一致。目前仍未完成這些驗收。
