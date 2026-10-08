# 懸崖圖塊 TEMP 分析（2026-10-08 Codex）

本次僅讀 `%TEMP%/ArmGameCompare_20261007/floortex.dat` 與其中 `ENDL_000/005` 的 `boden.txt`、`boden.bmp`；未存取安裝目錄或啟動遊戲。

## 名稱與解碼

`FloorTextureLibrary.Names` 的 3005 個名稱中，76 個含 fels：`fels01_00..35`、`fels02_00..03`、`fels03_00..15`、`fels1..4`、`Fels_AA_001..009`、`ita_fels1..7`。全部可用 `FloorTextureLibrary.Get` 解碼。

16 個方向式名稱 `FELS_N1/N2/S1/S2/E1/E2/W1/W2/NE_OUT/NW_OUT/SE_OUT/SW_OUT/NE_IN/NW_IN/SE_IN/SW_IN`，精確與忽略大小寫搜尋均不存在。`FELS1/2` 只可忽略大小寫找到 `fels1/2`；後者是均勻岩石，不能證明朝向。

## 四角與岩石側

與 `FloorMaterialCatalog` 的 4U 推斷相同，使用貼圖 TL/TR/BR/BL 的 32×32 小角落均色，另外以 16×16 抽樣確認。四角也列出距離最近的三種既有地表材質。沒有把 fels 的編號當成 4U 九宮格／位元遮罩。

乾草／淡岩候選以 `fels01_35`／`ita_fels3` 整圖均色為參考；AA 組以 `Fels_AA_009` 的 BR 草地角與 `Fels_AA_005` 整圖岩石為參考。報表逐角保留 RGB、兩種色距、差距、16/32px rock mask、候選岩石側與歧義數。乾草與岩石顏色相近、混合角、均勻岩石、對角組合均不足以自動認定懸崖幾何。

經色距、兩種抽樣尺度與 TEMP 拼貼交叉判讀，先使用以下四個直向圖塊（1 代表岩石，0 代表草地）：

| CliffFacing | 精確名稱 | TL/TR/BR/BL 岩石角 | 32px 四角 RGB |
|---|---|---|---|
| North | Fels_AA_008 | 1/1/0/0 | 141,142,135 / 145,148,136 / 127,134,92 / 128,133,95 |
| East | Fels_AA_004 | 0/1/1/0 | 132,138,102 / 145,145,134 / 138,140,114 / 123,131,91 |
| South | Fels_AA_002 | 0/0/1/1 | 131,137,100 / 130,133,104 / 142,144,138 / 141,143,138 |
| West | Fels_AA_006 | 1/0/0/1 | 148,151,142 / 124,134,85 / 129,136,96 / 135,137,128 |

岩石側判讀信心高；把岩石側放在 downhill side 是作者工具目前的視覺對應，**沒有原圖高低坡面或遊戲執行期證據**。它們是草地／岩石過渡，不是已證明的立體壁面。不宣稱能把平緩斜坡變成垂直懸崖，也未證實與 B8 等其他地表無縫銜接。

AA 的單角岩石只證明材質區域轉角，不證明懸崖的內凹／外凸幾何。八個內外角、None、fels01/02/03、均勻 fels/ita_fels 均未登記為 cliff face。

## 原始地圖高度證據

兩張 TEMP `boden.txt` 以 `BodenTexturesDocument.Load` 解壓讀取，均為 64×64；`boden.bmp` 由 `TerrainLayerFiles.Read` 讀 GREEN 高度，257×257，每 tile 4 個頂點間距。

**ENDL_000 與 ENDL_005 都沒有任何 fels 使用格。** 因此沒有可列舉的 fels 座標或高度差，不能以這兩張原圖驗證岩石側的高低關係。分析程式仍會逐一列出任何 fels 使用格的座標、四角高度、整 tile 最大落差、N/E/S/W 鄰格中心高度及現有 detector 朝向；此次 Occurrences 為空。

4U／PFAD 的數字鍵盤 convention 是材質區域邊界位置，不能套用到 fels 數字。參見 [道路分析](map-editor-road-tiles.md)、[地圖格式](reverse-engineering/map-formats.md) 與 `FloorMaterialCatalog`。

## 工具行為與重現

`CliffTileCatalog.BuildRealNames` 只登記上表且保留 archive 中的精確拼字；`FilteredBy` 保留 strict facing 政策。host 另驗證可解碼，從不印章缺檔或錯方向。未支援格子不貼圖、不生成碎石／crest 或碰撞；混合選取可套用已支援格子，狀態列回報略過數，完全無支援時不變更。

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
$env:ARM_CLIFF_ANALYSIS = '1'
dotnet test tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj -c Release --no-build --filter FullyQualifiedName~CliffTextureAnalysisTests
Remove-Item Env:ARM_CLIFF_ANALYSIS
```

輸入位置固定，環境變數只控制啟用。輸出固定為 `%TEMP%/ArmCliffAnalysis_20261008/analysis.json` 與 `fels-contact-sheet.png`（重跑覆寫這兩份分析輸出）。JSON 含來源 SHA256、全部名稱／解碼失敗、每張四角判讀、採用 catalog、地圖用量／座標及高度資訊。沒有原始素材加入 repo。

Modules 測試涵蓋實名拼字、缺方向與 None 不回退、FilteredBy 保持策略、未支援格子的附帶效果隔離。host 測試涵蓋直向 stamp、矩形 dialog、texture＋collision 單步 undo/redo，以及損壞實名圖塊與猜測名稱的拒絕。