# 地圖編輯器需求與證據核對

2026-10-07，Codex。依 `map-editor-roadmap.md` 第 1 階段核對。這是缺口清單，不是完整完成宣告。

## 需求來源與已變更項目

- 原任務 `map-editor-spec.md` §0 是從範本建立、編輯 ENDL 無盡地圖。其他地圖可列出、檢視，不代表多人同步或劇情編輯已成為正式支援範圍。
- 原 §1 的獨立 EXE／修改器分頁敘述，已由同文件 Phase 1 的統一程式與 `--game`／`--map` 增補更新。現況：MapEditor 是 library，`src.Modifier/Program.cs` 處理入口；不為符合舊段落再拆 EXE。
- `map-editor-3d-view-spec.md` 的「不新增寫入面」是當時 3D 檢視階段限制；後續整體規格已增補高度／通行與 AI 編輯。保留後續功能，但不能省略原版輸出及遊戲內驗收。
- 3D Phase D 的 billboard 是選做且需另行同意；`.alr` 模型逆向不在該規格範圍。不能把選做項目當成目前完成門檻。
- 使用者已澄清開發用本地 AI 與產品 AI 不同；產品 AI 現在明確要求保留並完善。

## 核對結果

| 需求 | 目前證據 | 判定／下一步 |
| --- | --- | --- |
| 啟動與直接開圖 | `src.Modifier/Program.cs` 的參數檢查與 MapEditorForm 入口 | 有實作；需補入口驗收證據 |
| 原版唯讀、自製槽位 005–999 | `CustomMapAccess` 統一兩個catalog、SDL service與Save；真正表單鎖briefing測試證明拒絕先於檔案開啟 | 已修：原廠帶marker仍唯讀；stale selection／移除marker／錯誤root拒存，bytes與dirty保留，合法marker恢復可retry |
| 完整複製未知檔案、header、SDL 路徑 | `EndlessMapCloner`、Phase1 Clone / Documents 測試 | 合成測試涵蓋；遊戲內載入仍缺 |
| 複製任何階段失敗無半成品 | 新 manifest Load／Save 失敗測試先重現正式槽位殘留 | 本輪修正：manifest 交易與本次目標回滾；成功可原槽位重試 |
| 不覆蓋既有正式／暫存目錄 | Clone 在 copy 前檢查；新增 sentinel 測試 | 本輪測試涵蓋，既有內容不得刪除 |
| 刪除三重防護與原廠槽位拒絕 | 原 Deleter 缺 manifest membership／slot guard；新 slots 0/4/5/999 測試重現 | 本輪補回登記＋marker＋受控槽位／路徑條件，在 rename 前拒絕 |
| CP1251、文字常值限制、多行簡報 | `MapTextDocuments.cs` 與 Phase1 PutTextDocument 測試 | 有合成證據；需核對特殊字元與 UI 回饋 |
| 環境參數與未知行保留 | 真正表單三地區（en-US/de-DE/fr-FR）七值、Heightmapstep、雨滴、dirty、儲存／重開與重存bytes相同測試 | 修正依Windows地區讀錯數值及寫出逗號問題；未知行保留。合成小數案例不證明遊戲接受所有小數參數 |
| 聚落移動、複製、刪除、重新編號 | SdlSceneEditService／SdlDocument，Phase1 多項測試 | 合成路徑已有證據；遊戲內仍缺 |
| 自製圖不進備份基準 | `GameDirectoryBackupLoader`、`BackupAutoHealer` 排除 marker-backed 目錄 | 有實作；核對測試及完整還原保留／刪除行為 |
| AI 修補涵蓋新 ENDL 圖 | `EndlessAiOrchestrator` 動態列舉 ENDL_??? | 有實作；需核對額外槽位 Detect／Apply 的專屬測試 |
| 地形、材質、通行、歷史／儲存交易 | 驗收矩陣列出的 session、真正表單及 AI 跨模式測試 | 合成路徑已驗證；原版輸出仍有遊戲內 gate |
| 自然物件／人物／部隊／建築與事件 | 已有單兵與 nature fail/retry、身份、VM／dialog 測試 | 補部族／隊伍與多功能共同儲存矩陣，不推定多人同步已支援 |
| 空白地圖語意 | MapSelectionForm 仍標示 New Blank Map，實際複製範本並保留聚落；原 §0 要求驗證前不得宣稱真空白圖 | 名稱／支援邊界尚待收斂；不能沿用 UI 名稱證明需求完成 |
| 3D 真貼圖、水面、光照、標記、筆刷與 fallback | 純計算測試與 2D DrawToBitmap 不證明 GL 畫面 | 真正 OpenGL、資源失敗診斷及互動效能尚待驗收 |
| UI 中英／尺寸／DPI | 96 DPI 主畫面與 AI／事件預覽已有證據 | 高 DPI 尚缺；按 roadmap 排在核心整合後 |
| AI 正式生成／預覽／套用 | 已有序列鎖、取消、部分失敗、套用與儲存測試；預覽 WIP 尚未接 UI | 第 4 階段續作，不能以舊 live 結果當成本輪完整驗收 |
| 遊戲載入、存讀檔、原版品質 | 本輪沒有執行遊戲 | 尚未完成；仍遵守 AGENTS.md 禁止存取安裝目錄 |

## 下一批優先工作

1. 本輪建立／刪除修正已提交 `b9b9965`；獨立 git archive 快照 build 0 警告/0 錯誤，完整測試 536 通過/21 略過/0 失敗，已確認不依賴 AI 預覽 WIP。
2. 原廠圖 catalog 與 SaveMap 前置驗證已補9項測試；最新完整build 0警告/0錯誤，545通過/21略過/0失敗。
3. 核對地圖文字、環境、備份／完整還原與額外槽位 AI 修補的專屬證據。
4. 收斂「空白地圖」命名與支援範圍，再進入第 3 階段物件／事件整合。

尚未完成的項目保留在清單中，不以目前可通過的測試重新定義成功。所有執行驗證只使用 repository／TEMP 合成資料。
