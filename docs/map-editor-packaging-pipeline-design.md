# 地圖模組封裝與安裝契約

2026-10-08 Codex，audit task 3。適用 `ModBundleExporter` schema **1.1**。

## 稽核結果與邊界

原安裝器只在偏好槽位被佔用時重新配置，所以來源 ENDL_008 能被安裝到
空的 008，即使 005–007 不存在。依使用者 2026-10-08 的遊戲實測，自訂 endless
列表從 ENDL_005 連續掃描，遇缺口即停止。原程式另有解壓路徑未驗證、manifest
清單與 ZIP 不一致、SDL 改寫失敗被吞掉及登記失敗留下已發布目錄的問題。

現在預設每次選 `EndlessMapCatalog.GetNextFreeSlot` 的第一個缺口；來源槽位只用於
改寫來源路徑，不能決定安裝目的地。這會填入缺口，但不搬動或修復既有的高位地圖。
這個契約驗證檔案安裝與 catalog 整合，不保證地圖可玩或遊戲內完整載入。

## ZIP 與 manifest

```text
bundle.zip
  manifest.json       必要，schemaVersion = "1.1"
  README.txt          安裝說明
  thumbnail.bmp       可選，供預覽，不安裝到槽位
  map/                相對於一個槽位的內容，不含 MAPS/ENDL_XXX 前綴
    boden.bmp, boden.ini, collision.bmp, …
    Endlos_*_Siedlung*.sdl
    DATA/*.dat
    SCRIPT/*.bci
    TEXT/<兩字母語系>/*.put
    arm_scenario.json
```

必需欄位：`packageId`、`title`、`schemaVersion`、`payloadLayout = "map-relative"`、
`sourceMapId = "ENDL_NNN"`、`compatibility`、`files`、`packageChecksum`。
`compatibility.preferredSlot` 必須為 5–999，且和 `sourceMapId` 完全一致；它在
1.1 代表原始路徑槽位。`allowDynamicSlotRemapping` 控制可否改槽；`standaloneLevel`
由原 marker 讀取，用來重建收件人的 marker。`files` 包含每個 payload 的相對路徑、
大小、SHA-256 和分類；**清單與實際 map/ entries 必須完全一致**。
`packageChecksum` 為元資料和 inventory 的組合 SHA-256；不是簽章或作者身份證明。

匯出槽位來自來源目錄 ENDL_NNN（一般編輯器自訂地圖使用此名稱）。
ZIP 內 SDL 保留來源路徑，安裝時才改寫；檔名維持原名，與 cloner 行為一致。
其他 manifest 欄位（作者、版本、語系、尺寸、玩家數等）沿用既有資料模型。
舊 schema 1.0 或缺 manifest 的 ZIP 拒絕安裝，請重新匯出，避免猜測來源槽位。

## 可發布內容

`MapBundleContract` 是地圖檔案白名單；根目錄只接受地形、通行、minimap、
地圖設定、briefpic/daynight、preload、SDL 和 scenario metadata。
子目錄只接受 DATA dat、SCRIPT bci、TEXT put；不接受 EXE、DLL、APT、
全域 TEXTURES/OBJECTS 等素材目錄。收件人須自行擁有相容的遊戲與原廠素材。
地圖的地形圖與 DATA/腳本是地圖 payload；不打包遊戲安裝樹或全域素材庫。

`.arm_custom_map` 和 `arm_custom_maps.json` 是本機登記資料，不能從 ZIP 複製。
備份、快取、未知檔案均不在白名單內。`CleanCaches=false` 不放寬內容白名單。
來源中有 junction/symlink 時拒絕匯出。`MapExportPreflightChecker` 顯示排除清單；
`MapPackageManifest` 與 `ModBundleExporter` 使用同一 inventory 寫入 ZIP。
預檢與縮圖不會驗證收件人的素材庫或遊戲相容性。

## 安裝 API 與交易

```csharp
ModInstallResult installed = ModBundleExporter.InstallFromZip(zipPath, gameRoot);
// gameRoot 是含 MAPS 的遊戲根目錄，不是 MAPS 或某個 ENDL 目錄。
```

1. 拒絕重複 entries（大小寫亦視為重複）、非白名單 entry、路徑穿越、絕對路徑、
   反斜線、ADS 與 Windows 裝置名稱。必須提供有效 schema 1.1 manifest。
2. 驗證 inventory、大小、package checksum。單檔上限 128 MiB，payload 總量
   上限 512 MiB，manifest 上限 1 MiB。SHA-256 在解壓後、改寫前逐檔比對。
3. 選第一個空位 005–999；原廠 000–004 不可選。預設 auto 策略忽略偏好/指定
   槽位並配置第一個缺口。`FailIfOccupied` 的指定目標必須正好為第一個缺口。
   `OverwriteCustom` 保留 enum 以相容呼叫端，但明確拋出 NotSupportedException；
   此安裝 API 永不覆蓋既有地圖。不同槽位且不允許 remapping 時中止。
4. 在 MAPS 下建立唯一 `.arm_install_<guid>` 暫存目錄，抽取 map/ 並驗證預檢。
5. 呼叫 **同一個** `EndlessMapCloner.RewriteKnownFiles`：更新 TEXT/US/briefing.put
   的 briefing_titel_1，並透過 SdlDocument 改寫根目錄 Endlos_*_Siedlung*.sdl
   中已知的地圖路徑。沿用遊戲文字格式；改寫錯誤會中止。這不是對任意 binary
   或腳本路徑的全域替換；其他硬編碼路徑仍需另行實測與擴充契約。
6. 產生新 `.arm_custom_map`：Slot=實際槽位、SourceSlot=來源槽位、目前時間、
   Packaging ToolVersion，以及 manifest 的 StandaloneLevel。
7. Directory.Move 發布到尚未佔用的 ENDL_NNN，再透過 FileRollbackScope 登記
   MAPS/arm_custom_maps.json。失敗時刪除本次發布的目錄、回復 registry 並清理 staging。
   既有地圖內容保持原狀。Move 遇同時佔位會失敗；多個安裝器同時更新 registry
   並非跨程序交易，應一次只執行一個安裝程序。

## 操作入口

編輯器工具列「匯出模組…」的 `ExportModPackage` 先儲存 dirty 自訂地圖，然後
呼叫相同 ExportToZip API。現階段沒有「模組管理器」安裝 UI；README 已改成
指向實際 CLI `tools/ArmMapPackage`。玩家步驟見 [安裝指南](map-package-install.md)。
CLI 的 export 與 install 都使用相同 API，失敗回傳非零 exit code。

## 驗證

`tests/AgainstRomeMapEditor.Modules.Tests/Packaging/MapPackagingTests.cs` 測試：

- 來源 ENDL_008 匯出後安裝到空的 005 或已佔用 005 後的 006，含 catalog 標題、
  SDL 路徑、marker、registry 與完整 ZIP inventory 比對。
- 全域素材、EXE、本機 marker 不進 payload；CleanCaches=false 亦不放寬白名單。
- 官方槽位、超界槽位、空洞、已佔用和 overwrite 策略拒絕。
- 穿越/ADS/裝置名稱、重複 entries、不支援素材、缺失 manifest、錯誤 schema、
  checksum、禁止改槽與壞 registry 的失敗路徑；失敗不發布殘留地圖。
- StandaloneLevel 在新 marker 保留；既有 metadata、縮圖、預檢測試保持。

要求的驗證命令（在 worktree 執行）：

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
dotnet test AgainstRomeModifier.slnx -c Release --no-build
```

本輪只讀指定 TEMP 編輯器已儲存地圖；額外 CLI round-trip 可在 TEMP 新建假遊戲
目錄驗證真實 PFIL 地圖格式。不得以這些測試宣稱安裝到真實遊戲或遊戲內可玩。
