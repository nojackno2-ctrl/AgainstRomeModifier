# 示範地圖整合驗收

2026-10-08 Codex：以現有 TEMP 真實素材副本完成編輯、儲存、新表單重開與再次儲存驗收；後續加入土路折線與聚落／森林配置保存套用。這是編輯器整合證據；遊戲內可玩性尚未驗證。

## 案例與結果

- 來源：`%TEMP%/ArmGameCompare_20261007`，來源 ENDL_005 所有檔案 SHA256 在驗收後保持一致。
- 初始輸出：`%TEMP%/ArmDemoAcceptance_20261008_04`；配置套用定向案例為 `_07`，含 `game/MAPS/ENDL_005`、`demo-3d.png`、`result.json`、`source-hashes.json`、forest／settlement `.arm-layout.json`。最新完整回歸另記於AI_HANDOFF.md。
- 地圖保留原範本聚落、腳本與資源；新增 L 形 PFAD 土路、丘陵、小湖與森林示範區。森林區先清除可移除地景再種植，最終本次新增16株；散佈採隨機取樣，重跑数量可能不同。
- 保留三棟 DATA 建築及玩家10人部隊，修改部隊朝向；配置案例額外套用一棟主屋，合計4棟DATA建築／5個放置物件。新增3秒提示與區域勝利事件，目標矩形世界座標 X=11000..12000、Z=8600..9700。
- 驗證土路 undo/redo、湖中心高度27×4低於水位120、儲存前磁碟不變、儲存後無髒狀態、部隊／建築持久 ID 與事件目標保留、新表單地形／材質／通行層一致、同表單與新表單重存全部 bytes 一致。
- 真 OpenGL context 擷取整張地圖概覽並目視檢查；概覽不代表材質接縫或遊戲內視覺驗收。
- 最終 Release build 零警告／零錯誤；完整測試明確啟用示範案例及真 OpenGL，宿主650通過／22略過、modules275通過，零失敗。四項新增 DATA 範本回歸案例涵蓋正常沿用、UID不符、alias改變、linked／位置索引／未擁有槽位拒絕。

整合時發現：外部官方地圖副本缺建築範本，修改部隊會讓既有完工建築改為腳本工地。現在先保留同持久 ID、同 alias、同 runtime UID 的未連結 DATA 範本，再重建建築；UID不符、alias改變或不安全的槽位不沿用。新放置建築可用官方範本或目前圖內安全的同類型範本，兩者均缺少時維持工地回退並列出警告。

## 重現

來源目錄須位於 TEMP，包含 `ENDL_000`、帶 marker 的 `ENDL_005`、`SYSTEM` 與 `floortex.dat`、`alr.dat`、`apt.dat`、`shad.dat`。輸出必須是 TEMP 下尚不存在的新目錄，不覆蓋已有工作。以下命令不使用 `ARM_GAME_PATH`，不存取遊戲安裝目錄。

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
$env:ARM_COMPARE_GAME = Join-Path $env:TEMP 'ArmGameCompare_20261007'
$env:ARM_DEMO_ACCEPTANCE = '1'
$env:ARM_DEMO_OUTPUT = Join-Path $env:TEMP ('ArmDemoAcceptance_' + [Guid]::NewGuid().ToString('N'))
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build --no-restore --filter FullyQualifiedName~Demo_real_copy --logger 'console;verbosity=normal'
Remove-Item Env:ARM_DEMO_ACCEPTANCE
```

未設定 `ARM_DEMO_ACCEPTANCE=1` 時此案例直接返回，不能把一般全套測試的通過數當成示範地圖驗收證據。啟用後素材缺失或 OpenGL 不可用會失敗。測試輸出只含本案例所需資料，不是完整可啟動的遊戲目錄；素材與地圖不納入 repository。

## 後續遊戲內驗收

目前 AGENTS.md 禁止代理存取安裝遊戲目錄。這階段沒有安裝地圖或啟動遊戲；後續應由使用者執行，或另行明確調整此限制。

1. 確認玩家10名士兵與三棟完工建築可選取、使用，出生區未被障礙堵住。
2. 沿土路移動，確認湖泊、丘陵與森林周邊尋路可用；檢查既有資源是否能採集。
3. 開局3秒提示正常；未進入目標區域不結算，指定起始部隊進入後觸發勝利。
4. 勝利前存檔、讀檔，再確認部隊目標與事件行為。
5. 檢查道路交界、水面、物件陰影與部隊方向，記錄地圖、遊戲版本及截圖。

完成上述步驟之前，示範地圖只標記「編輯器整合通過」。下一個開發重點是地圖檢查面板，把阻擋、連通性與事件目標問題轉成可定位的提示。
