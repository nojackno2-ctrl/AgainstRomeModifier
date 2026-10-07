# shad-probe：唯讀物件陰影研究

C#/.NET 8，沿用 alr-probe 的獨立程式模式，直接連結 NativeShadow*.cs。
不連接 UI、不啟動遊戲、不查找安裝目錄，不匯出原始 BMP。

## 全庫驗證

輸出只允許尚不存在的 TEMP 子目錄。建議使用 GUID：

```powershell
$env:DOTNET_ROLL_FORWARD='Major'
$output = Join-Path $env:TEMP ('ArmShadowProbe_' + [guid]::NewGuid().ToString('N'))
dotnet run --project tools/re/shad-probe -c Release -p:UseAppHost=false -- `
  "$env:TEMP/ArmNativeAssets_20261007/shad.dat" $output `
  "$env:TEMP/ArmNativeAssets_20261007/objdef.txt"
```

可再加第四個參數：**已解碼文字** cl_shado.txt。格式為
`[ShadowNames]` 下的 `0001,filename.bmp`。工具不會從 ZIP 次序猜 ID。
未提供清單時，id 12/42/46 的預覽只使用明確標示的候選檔案。
清單不在目前的 shad.dat；EXE 引用 SYSTEM\cl_shado.ini 及 cl_shado.i00..i09。

- 開啟每個 ZIP 項目，檢查所有 BMP 的 signature、header、尺寸、offset、
  palette index、實際 payload 範圍。
- SHADOWTEXTURE 的 415 BMP 使用 NativeShadowDocument 嚴格解析和解碼；
  每張只有一格。其餘 2256 BMP 僅作結構檢查（含一張 24-bit 圖），不當作陰影。
- 非陰影 BMP 有 160 筆 biSizeImage 與實際 row extent 不同，
  仍檢查實際 byte 範圍；報告 imageSizeMatches=false，不掩飾欄位差異。
- report.json 記錄 archive SHA256、完整 inventory、分類統計、錯誤、
  samples、物件 shadow 欄位、映射是否已確認。
- sample-N.png 是透明 black ARGB 遮罩；sample-N-ground.png 用 native
  channel*(256-index)/256 精確混到綠色背景。
- object-ID-ground-assumption.png 是平地投影示意，紅十字為物件錨點，
  青十字為加上 shacx/shacz 的陰影中心。**UV 軸向、旋轉和裁切為假設**，
  沒有疊上物件 sprite，也沒有與遊戲畫面比對。

已有輸出會拒絕（exit 2）。無 apphost 的 --no-build 執行請直接使用 DLL：

```powershell
dotnet tools/re/shad-probe/bin/Release/net8.0/shad-probe.dll `
  <readonly-shad.dat> <NEW-TEMP-directory> [objdef.txt] [cl_shado.txt]
```

格式、EXE 地址、驗證與限制詳見
[原生場景研究](../../../docs/reverse-engineering/native-scene-rendering.md) 的
「shad.dat 物件陰影」節。靜態分析使用既有 scan_native_scene.py；
pefile / capstone 依賴由呼叫者設定 PYTHONPATH，不安裝新套件。