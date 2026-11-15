# 故事地圖：The Last Granary（最後的糧倉）

本文件說明使用本專案 Rust 地圖編輯器（`against-rome-editor-app`）公開 API 在 `ENDL_005` 工作副本建置的可遊玩故事劇本 **The Last Granary**。所有遊戲內訊息皆採用純 ASCII 英文（因遊戲以 Windows-1251 編碼顯示文字，無法顯示中文）。

---

## 故事概要

- **地圖槽位**：`ENDL_005`
- **玩家陣營**：Team 0（沿用 `ENDL_005` 既有的日耳曼聚落：主屋 `GER_HAU00` 位於 `(10624, 10112)`、住宅 `GER_WOH00`、農莊 `GER_BAU00` 與開局步兵 `GER_INF01` × 10 位於 `(9856, 9344)`）
- **敵方陣營**：Team 1（匈人騎兵與弓騎兵波次）
- **目標**：堅守村莊 8 分鐘（480 秒）直到羅馬援軍抵達；若玩家主屋 `GER_HAU00` 被摧毀則立即失敗。

---

## 建置工具與成品位置

- **建置測試**：[story_map_builder.rs](file:///D:/Github/AgainstRomeModifier/apps/against-rome-editor/tests/story_map_builder.rs)（`#[ignore]` 整合測試）
- **工作副本成品路徑**：`%TEMP%\arm_story_20261115\Against Rome\MAPS\ENDL_005`
- **地形／碰撞檢查**：透過 `EditorApp::height_at_world`、`EditorApp::effective_map_settings`（水位）與 `TerrainSession::collision` 驗證生成點與中央偵察區皆位於可行走陸地（`collision == 0` 且高於水位）：
  - 玩家主屋：`(10624, 10112)`（高度 `35`）
  - 敵軍西北邊緣生成點：`(1280, 1280)`（高度 `35`，距離主屋 `>= 5000`）
  - 中央偵察矩形：`(7680, 7680)..=(8192, 8192)`

---

## 完整事件表

| # | 事件名稱 | 觸發時間 | 重複 | 條件 | 動作（依序執行） | 座標／目標 | 單位別名與數量 |
|---|----------|----------|------|------|------------------|------------|----------------|
| 1 | `Story_01_Briefing_And_Hostility` | `0:00`（0s） | 單次 | 無 | 1. 顯示訊息（Team 0）：`"The Last Granary: Hold the village for 8 minutes until reinforcements arrive! Defend your main house."`<br>2. 隊伍外交：Team 0 ↔ Team 1 設為敵對（`hostile=true`） | — | — |
| 2 | `Story_02_Wave_1` | `1:00`（60s） | 單次 | 無 | 1. 顯示警告訊息：`"Warning: A small enemy raiding party is approaching from the northwest!"`<br>2. 生成敵軍並命令移動至主屋 | 生成 `(1280, 1280)` → 目的地 `(10624, 10112)` | `HUN_KAVINF00` × 5（Team 1） |
| 3 | `Story_03_Wave_2` | `3:00`（180s） | 單次 | 無 | 1. 顯示警告訊息：`"Warning: A stronger enemy force with horsemen and archers is marching on the village!"`<br>2. 生成近戰騎兵並移動至主屋<br>3. 生成弓騎兵並移動至主屋 | 生成 `(1280, 1280)` → 目的地 `(10624, 10112)` | `HUN_KAVINF00` × 6（Team 1）<br>`HUN_KAVSCH00` × 3（Team 1） |
| 4 | `Story_04_Wave_3` | `5:00`（300s） | 單次 | 無 | 1. 顯示警告訊息：`"Warning: The main enemy warband has arrived! Hold the granary!"`<br>2. 生成近戰騎兵並移動至主屋<br>3. 生成弓騎兵並移動至主屋 | 生成 `(1280, 1280)` → 目的地 `(10624, 10112)` | `HUN_KAVINF00` × 8（Team 1）<br>`HUN_KAVSCH00` × 5（Team 1） |
| 5 | `Story_05_Center_Scout_Hint` | `0:00`（0s） | 單次 | 位於矩形區域（`kind=2`）：目標 `GER_INF01`（`bc6353c7-ac64-9919-f5e5-bae5e0a83464`） | 1. 顯示偵察訊息：`"Scouts report enemy riders gathering beyond the central ridge. Watch the northwest approach!"` | 矩形 `(7680, 7680)..=(8192, 8192)` | — |
| 6 | `Survive_480s` | `8:00`（480s） | 單次 | 無 | 1. 勝利（`kind=3`，末位動作） | — | — |
| 7 | `Team_0_main_house_destroyed` | `0:00`（0s） | 單次 | 死亡或已移除（`kind=1`）：目標 `GER_HAU00`（`e199b4a5-8f55-2dc5-ed82-0c4e5d1671b2`） | 1. 失敗（`kind=4`，末位動作） | 主屋位於 `(10624, 10112)` | — |

---

## 安裝至 `MAPS\ENDL_005` 與還原步驟

> [!WARNING]
> 請勿直接覆寫其他原版地圖，且操作前務必確認已完成備份。

### 1. 備份原版 `MAPS\ENDL_005`

目前安裝的 `ENDL_005` 已是故事地圖；先前的原始備份是 `%TEMP%\arm-ENDL_005-backup-20261115`。請先確認備份內容，並另存到不會被系統清除的位置。不要將目前已安裝的故事地圖誤認為原始備份。

若在其他電腦首次安裝，請在安裝之前備份原有地圖；以下指令拒絕覆寫既有備份：

```powershell
$gameMap = "C:\Program Files (x86)\Against Rome\MAPS\ENDL_005"
$backup  = Join-Path $env:TEMP "arm-ENDL_005-backup-manual"
if (-not (Test-Path -LiteralPath $gameMap -PathType Container)) { throw "找不到地圖：$gameMap" }
if (Test-Path -LiteralPath $backup) { throw "備份已存在，請先檢查：$backup" }
Copy-Item -LiteralPath $gameMap -Destination $backup -Recurse -ErrorAction Stop
```

### 2. 安裝故事地圖成品

先確認備份完整，再使用地圖編輯器或檔案總管將 `re_workspace\story-map-the-last-granary\ENDL_005` 的成品複製到目標。複製前確認來源與目標，並保留備份；不要先刪除目標資料夾。若目標原有額外檔案，先核對其用途，再決定如何處理。

### 3. 還原原版 `MAPS\ENDL_005`

測試結束後，先確認選用的是安裝前的完整備份：本機先前的備份為 `%TEMP%\arm-ENDL_005-backup-20261115`；依上述步驟新建的備份則為 `%TEMP%\arm-ENDL_005-backup-manual`。先將目前地圖另存，再以檔案總管從正確備份覆蓋目標，並逐檔核對。額外檔案須與備份清單比對後個別處理，不要直接刪除整個目標資料夾。

---

## 尚未在實際運行遊戲中驗證的項目

以下項目僅通過編輯器 API 建置、Design Check、Safe Save、重載與 `--validate-map` 離線檢查，**尚未在實際執行的 `Against_Rome.exe` 中進行遊戲內驗證**：

1. 地圖於無盡模式選單載入與開局正常啟動。
2. 開局 `0:00` 簡報訊息顯示與 Team 1 敵對外交（血條是否正確顯示紅色並可主動攻擊）。
3. `1:00`、`3:00`、`5:00` 三波警告訊息顯示、敵軍於 `(1280, 1280)` 實際生成，以及從西北方尋路移動並攻擊 `(10624, 10112)` 玩家主屋。
4. 玩家開局步兵 `GER_INF01` 進入中央矩形 `(7680, 7680)..=(8192, 8192)` 前不觸發、進入後才觸發偵察訊息。
5. 存活至 `8:00`（480 秒）正確觸發勝利結算，以及 `GER_HAU00` 被摧毀時正確觸發失敗結算。
6. 三波敵軍強度對一般玩家是否確實可守住（實際遊戲平衡與尋路通暢度）。

---

## 成品保存位置與目前安裝狀態（2026-11-15）

- 不含編輯器備份檔的成品另存於 `re_workspace\story-map-the-last-granary\ENDL_005`（56 個檔案；`re_workspace` 不進版控，內含遊戲素材故不提交）。`%TEMP%` 內的工作副本可能被系統清除，安裝時建議改用這份。
- 已安裝到 `C:\Program Files (x86)\Against Rome\MAPS\ENDL_005`，安裝後逐檔 SHA-256 與成品相符（56/56）。原本的 `ENDL_005` 備份在 `%TEMP%\arm-ENDL_005-backup-20261115`（55 個檔案）。
- 還原時除了覆蓋 55 個備份檔，還要刪除新增的 `arm_start_camera.json`。
