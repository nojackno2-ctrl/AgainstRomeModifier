# 道路自動選片開發狀態

## 2026-10-08 Claude：真實圖塊分析與土路材質

依授權的唯讀 TEMP 副本 `ArmGameCompare_20261007/floortex.dat`（未存取安裝目錄）以邊緣 8–10px 石塊（低飽和）比例分析：

- `H_WEG1–5` 開口只在東西（E/W 中段 55–77%，N/S < 10%），`V_WEG1–3` 只在南北；**沒有任何轉角或路口片**。`WEG_*ROM` 是不規則鵝卵石塊，開口不一致，不適合自動選片。`weg1–3`、`PFAD1–3` 是均勻地表。
- `PFAD<九宮格><A|B><變體>`／`PFAD_Erde…` 是路面區域的外圈邊界（A＝邊與外角、B＝內角），與 4U 數字鍵盤慣例一致，涵蓋 16 種四角組合中的 14 種（缺兩種對角）。原版地圖用量 PFAD 4266 格，遠多於 H_WEG 68、V_WEG 28。
- 因此改將 PFAD 註冊為可直接繪製的「土路」材質（`FloorMaterialCatalog.PathMaterialId`）：角點由名稱決定，外側材質以顏色推得（草地組 → L2:07 色差 3.3–3.5、Erde 組 → L2:08 色差 5.3）。其他色差 ≤16 的材質為近似外側，每種只用最吻合的邊界組（容差 2），避免草地與土地邊界混用。
- 驗證：真實副本中每張已登記邊界的四角顏色都與名稱推斷一致；ENDL_005（全 B8）36/36 處筆刷成功且只用 Erde 邊界，ENDL_000（混合 4U）5/36。B8 上首版曾混用綠草邊界形成明顯綠框，已以「每外側單一組」修正並目視拼貼確認。遊戲內外觀尚未驗證。

H_WEG/V_WEG 直線道路工具（`RoadStrokePlanner`）仍未接 UI：沒有轉角片，任何轉彎都會整段拒絕，需另找轉彎做法後再接。

2026-10-07，Codex。道路核心已實作，尚未接入玩家操作介面，不能視為可用道路工具。

## 已有證據

- 先前 TEMP 素材系列報表列出 `weg1–3`、`WEG_H1ROM`、`WEG_H2ROM`、`WEG_V1ROM–4ROM`，以及 `PFAD1`、`PFAD2A1`、`PFAD3A1` 等。
- 名稱只能證明候選素材存在，不能證明其實際出入口。未把 H/V、PFAD 數字直接硬編碼為連接方向。
- 最新 AGENTS.md 禁止讀寫安裝目錄。已請使用者確認唯讀素材分析權限；本輪只讀先前 TEMP 報表，未存取安裝目錄。

## 道路核心

`RoadStrokePlanner.Plan` 接收目前材質、完整游標路徑及已驗證的原生道路圖塊連接表。連接使用 North/East/South/West（對應材質格 Y−1/X+1/Y+1/X−1）。

- 補齊稀疏游標點；斜向路徑改成共邊相接的階梯，不用只有角點接觸的格子。
- 合併折返、交叉的連接需求，保留被覆蓋道路與同系列鄰格間已存在的雙向連接。
- 轉彎／路口要求精確的原生片；單邊端點可使用直線片，缺片整份拒絕。
- 規劃不修改輸入。`TerrainBlendEditSession.PaintRoadPath` 成功才印章寫入，缺片撤回整段待提交筆畫。
- 使用既有材質歷史，支援整筆 undo/redo、成功儲存基準。呼叫端開始道路筆畫前須先提交其他工具的待提交筆畫，避免一起撤回。

`RoadStrokePlannerTests` 的十項合成測試驗證上述行為與缺片後重試；不是實際遊戲素材或外觀證據。

## 2026-10-08 Antigravity：道路自動選片接入 UI 與地區智能篩選完成

- **道路圖塊目錄與連接映射 (`RoadTileCatalog`)**：
  - 標準石道（Steinweg）：水平直線 `H_WEG1..5`、垂直直線 `V_WEG1..3`、交叉路口／轉角／廣場 `weg1..3`。
  - 羅馬道路（Römerstraße）：水平直線 `WEG_H1..2ROM`、垂直直線 `WEG_V1..4ROM`、轉角與交叉地磚 `Pflaster_braun1..3`（回退 `weg1..3`）。
  - 風格隔離：選用羅馬道路時優先配對羅馬組件，標準石道優先配對標準組件，避免風格混雜；缺特定方向直線片時才安全回退至萬用路口片。端點在有直路時優先由直線片平滑收尾。
- **介面整合 (2D/3D 統一筆畫流程)**：
  - 印章模式下新增雙語核取方塊「自動選路（拖曳自動轉向與路口）」／"Auto road (auto direction & junctions)"。
  - 2D 畫布（`MapCanvasControl`）與 3D 視圖（`Map3DViewControl`）拖曳繪製時即時收集軌跡點，調用 `RoadStrokePlanner.Plan` 與 `TerrainBlendEditSession.PaintRoadPath` 即時預覽材質變化。
  - 滑鼠放開時觸發 `CommitStroke`，整段道路筆畫合併為單次 Undo/Redo 交易並同步至 `BodenTexturesDocument`。
  - 按下 `Escape` 鍵即時呼叫 `CancelRoadStroke()`，乾淨取消未提交的筆畫並還原地圖材質顯示。
- **印章調色盤地區智能篩選**：
  - 當地圖貼圖本身未包含 `L` 系列（如純地景地圖）時，自動依地圖上原有地景物件地區代碼（如日耳曼樹木 `LanGer...` → 關聯 `L2`）進行智慧過濾，隱藏其他地區圖塊；勾選「顯示其他地區圖塊」或進行關鍵字搜尋時開放全部圖塊。
- **驗證**：
  - `AgainstRomeMapEditor.Modules.Tests` 新增 3 項 `RoadTileCatalogTests`（標準道路轉彎規劃、羅馬道路偏好挑選、缺轉向片回退），全套 293 通過。
  - `AgainstRomeModifier.Tests` 新增 3 項整合測試（`Auto_road_stroke_plans_straights_and_turns_in_views_and_undoes_as_one_step`、`Auto_road_escape_cancels_pending_stroke`、`Stamp_palette_filters_by_nature_region_when_map_has_no_L_tiles`），全套 689 通過、22 略過、0 失敗。
  - 完整 Solution 回歸（合計 982 項測試）全數通過，0 錯誤 0 警告。

