# 安裝其他玩家的地圖 ZIP

先關閉遊戲。需要相容的 Against Rome 遊戲與原廠素材；ZIP 只含地圖資料。
編輯器的「匯出模組…」會產生 schema 1.1 ZIP。不要直接將 map/ 複製到任意
ENDL_XXX：遊戲從 ENDL_005 連續列出地圖，空洞會讓高位地圖不可見，而且 SDL
內的來源路徑必須改寫。

目前安裝入口是 CLI，沒有模組管理器 UI。從 repository 根目錄，用可建置此
solution 的 .NET SDK 執行一次：

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build tools/ArmMapPackage/ArmMapPackage.csproj -c Release -p:UseAppHost=false
```

接著將以下兩個範例路徑替換成 ZIP 檔與你的遊戲**根目錄**（包含 MAPS）：

```powershell
dotnet tools/ArmMapPackage/bin/Release/net8.0/ArmMapPackage.dll install "D:\Downloads\MyMap.zip" "D:\Games\Against Rome"
```

安裝器會顯示實際 ENDL 槽位与標題，選擇從 005 起的第一個空位、改寫已知 SDL
路徑、建立自訂地圖 marker 並登記 catalog；不覆蓋已有地圖，也不更動原廠 000–004。
一次執行一個安裝程序。失敗會顯示原因並回傳非零 exit code，勿自行更名繞過驗證。

舊版 1.0 ZIP、缺 manifest、雜湊不符或不允許改槽的包可能被拒絕；請作者用新版
重新匯出。CLI 也提供 `export "<ENDL_NNN目錄>" "<輸出ZIP>"`，使用相同匯出管線。

完成後開啟遊戲 endless 地圖列表找標題。安裝驗證只確認資料與已知路徑；不同
遊戲版本、素材依賴、其他硬編碼路徑與實際遊玩結果仍需要作者和收件人實測。
