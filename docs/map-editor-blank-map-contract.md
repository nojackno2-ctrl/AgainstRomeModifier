# 平坦與真正空白地圖的建立條件

2026-10-08 Codex；仍待實作獨立空白生成與遊戲內啟動驗收。平坦建立流程現在在輸入名稱時列出來源、保留的聚落藍圖／開局放置物件、啟動腳本、編輯器放置／事件、受保護物件與將清除的地景數量；讀取失敗的項目明確列為未知。

## TEMP副本的新證據

唯讀分析 `%TEMP%/ArmGameCompare_20261007/ENDL_000`；這是現有副本，數量不代表所有原版版本。`SCRIPT/ak_level.bci` SHA256為 `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27`，PFIL解壓120657 bytes、CODE115244 bytes。

`python tools/bcitool.py syms <TEMP script>` 列出 INIT_UNITSCIV、INIT_UNITSMIL、四族Endlos_*_Siedlung1..5、placesSettle／placesSpawn／placesWaypoint；`calls` 找到以下code-relative位置（加0x24為解壓後offset）：

| 位置 | 原生呼叫 |
| --- | --- |
| 0xa5b0、0xa75c、0x1a52c | s_createUnitAndMems |
| 0xe560 | s_setVillageTemplate |
| 0x17b3c | s_addNPCJob_createUnit |

副本8份聚落SDL解壓後共有613個object區段，其中5個onload=1。這是靜態來源證據，不能直接推算遊戲實際出生數量或斷言所有分支必定執行。

## 真正空白的合格條件

- 清除源圖的可見地景、預放建築／部隊、編輯器事件與自動放置來源；不能只整平材質。
- 對啟動腳本、聚落藍圖、script markers與linked物件建立明確處置契約：保留會自動生成內容的原無盡腳本、或刪除它引用的藍圖，均不能直接宣稱空白且可玩。
- 原生DATA應保留有效格式／空槽，不以刪檔代替初始化。高度、材質、碰撞、輔助層與依賴快取需一致。
- 提供独立建立選項，告知起始隊伍／開局資料如何產生。用新輸出做儲存與重開驗證；最後實際遊戲開局、單位與尋路正常，才能完成此項。

建立清單並不證明上述啟動契約。AGENTS.md的安裝目錄限制目前仍有效；程式與資料分析只使用TEMP副本，遊戲內測試待已提出的例外授權或使用者手動驗收。
