# 遊戲內驗收候選圖

2026-10-08 Codex：候選圖只準備於 TEMP；編輯器保存／重開檢查不代表遊戲可啟動或可玩。此文件將六項建議剩下的遊戲驗收分成可重現案例。

## 三份候選

| 候選目錄 | 地圖 | 內容及用途 |
| --- | --- | --- |
| candidates/empty-control | ENDL_005 | 完全空場景，無放置與事件；先確認原無盡來源不再生成聚落、部隊、NPC 或特效 |
| candidates/empty-authored | ENDL_008 | ARM Empty Playtest；team 0、10名 GER_INF01，XYZ=(8192,200,8192)，朝向90°；3秒訊息，東側區域觸發勝利 |
| candidates/demo | ENDL_005 | 保留聚落／資源的完整示範圖，含土路、丘陵、小湖、森林配置與部隊／勝利事件 |

empty-control 與 demo 使用相同槽位，必須分開驗收，不能混合兩份 ENDL_005。候選目錄只包含地圖，並非完整遊戲或自動安裝套件；workspace/game 只供編輯器準備使用，缺少完整啟動素材。

`result.json` 記錄固定部隊與勝利區座標、待驗收項目；`candidate-hashes.json` 記錄交付時全部候選檔案 SHA256；`source-hashes.json` 記錄來源。新圖經真正表單保存、新表單重開及重存 bytes 一致，空白對照與兩份來源均保持不變。報告狀態只能是 `prepared-gameplay-not-run`。

## 遊戲內逐項確認

先使用獨立完整遊戲副本；若需要存取安裝目錄，必須先取得 AGENTS.md 限制的明確例外授權，再備份目標自製槽位、manifest 與相關存檔。現有準備流程不存取安裝目錄，也不自行安裝地圖。

1. **空白對照**：日耳曼無盡模式載入 ENDL_005。記錄能否進入、是否立即勝敗；持續至少60秒，確認沒有來源聚落／部隊／NPC 或殘留特效。沒有玩家物件時的結算行為需實測，不預設引擎一定接受。
2. **自行放置的空白圖**：載入 ENDL_008，確認恰有10名士兵可選取與下令，team 0 歸玩家控制。3秒訊息應只出現一次；未進目標區之前不能勝利。
3. **空白尋路與勝利**：先向北或西移動，再向東進入 X=9500..10500、Z=7600..8800。記錄移動及結算畫面。進區前另存／重讀一次，確認士兵與事件目標仍有效；重新開局再測一次不經存讀檔的版本。
4. **示範聚落與資源**：切換到 demo 的 ENDL_005。確認三棟既有完工建築及新增配置主屋的實際狀態、部隊人數與隊伍；檢查可採集資源與實際資源計數變化。缺少工人或建築的條件應記錄為未完成，不能以看見樹木當成可採集。
5. **示範尋路與外觀**：沿L形土路行走，分別繞過湖、森林與丘陵。近距離截取道路交界、水面、建築／物件陰影；對照編輯器的已保存內容。森林散佈數量以該次 report 為準。
6. **示範事件與存讀檔**：確認3秒提示、目標部隊未進區不結算，進 X=11000..12000、Z=8600..9700 後勝利。分別測直接進區與存讀檔後進區。

每案保存候選hash、遊戲EXE版本／hash、日期、部族／模式、步驟、預期、實際、截圖或錄影、是否經存讀檔。失敗應保留該候選；修改後建立新版本並重新核對hash，不覆蓋失敗證據。只有上述實際結果才能補上 completion plan 的遊戲內缺口。

## 重現候選準備

兩份來源均須是已驗證 TEMP 副本的 game 目錄；輸出必須是新的 TEMP 目錄。未設 opt-in 時案例直接返回，不能當成準備成功。

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
$env:ARM_GAMEPLAY_PREPARE = '1'
$env:ARM_GAMEPLAY_BLANK_SOURCE = Join-Path $env:TEMP 'ArmBlankAcceptance_20261008_06/game'
$env:ARM_GAMEPLAY_DEMO_SOURCE = Join-Path $env:TEMP 'ArmDemoAcceptance_20261008_10/game'
$env:ARM_GAMEPLAY_OUTPUT = Join-Path $env:TEMP ('ArmGameplayCandidates_' + [Guid]::NewGuid().ToString('N'))
$env:ARM_OPENGL_REQUIRED = '1'
Remove-Item Env:ARM_GAME_PATH -ErrorAction SilentlyContinue
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build --no-restore --filter 'FullyQualifiedName~Gameplay_candidates_prepare' --logger 'console;verbosity=normal'
Remove-Item Env:ARM_GAMEPLAY_PREPARE
```
