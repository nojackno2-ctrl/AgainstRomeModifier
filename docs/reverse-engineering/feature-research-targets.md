# 功能導向逆向目標

> 2026-10-09 使用者指定：以地圖編輯器及遊戲修改器功能為逆向目標。Agy job `41014a22bd74` 完成唯讀缺口盤點（exit 0，91 秒）；Codex 複核 registry、驗證矩陣及物件文件。以下是待辦優先序，不是新功能已完成的宣告。

本次直接讀取 `src.Core/Core/Features/FeatureRegistry.cs`，All 陣列有 **49 項**。與 [功能驗證矩陣](feature-verification-matrix.md) 相符；矩陣中的既有實機結論沿用文件紀錄，本輪未重做實機驗證。

| 優先 | 產品功能 | 逆向目標與完成判準 |
| --- | --- | --- |
| 1 | 地圖物件放置、清空及載入診斷 | 追 objects／position／objdata 與 anim／action／hirarchy 的 slot、UID、初始化及釋放約束。先建立唯讀跨池一致性檢查，再以受控測試證明新增／移除不破壞載入。最新成果見 [地圖資料池](map-data-pools.md)。 |
| 2 | 地形、碰撞、光照及預覽 | 以 [地圖格式](map-formats.md) 的高度及快取靜態證據為起點，確認改高度後原生快取重建與尋路同步；驗證編輯器預覽和遊戲畫面的一致性。 |
| 3 | 任務事件、巢穴波次及運鏡 | 追 VM 原語、`src.Shared/Scripting/LevelScriptInjector.cs` 注入流程及存讀檔後的事件狀態。以 [任務規則](objective-rules.md)、[巢穴](wild-lairs.md)、[運鏡](cinematic-camera.md) 區分已接線功能與實驗模型，逐項驗證觸發、重複執行及存讀檔。 |
| 4 | 修改器無盡 AI（EndlessAi.Core） | 補 deadline repair 的實機證據及長時間循環的生命週期檢查。M2/M3/M4 為 Core 相容別名，不另算原生補丁。參考 [AI 逆向](endless-mode-ai.md)。 |
| 5 | 修改器鏡頭（CameraZoomOut1） | 確認啟動／任務／存讀檔 setter 一致，量測擴大視野後的可見範圍、裁剪及效能；既有第一步縮放生效紀錄不能推廣為任意縮放安全。 |
| 6 | 修改器屍體保留（CorpseRetention） | 追 14,000-slot 物件池 reserve／回收機制與新增失敗路徑。補大規模戰鬥容量及招募／建造持續可用的驗證；目前仍為 Experimental。 |

所有新發現須連到 consumer／writer／reader／runtime 證據，再決定是否成為產品能力。靜態布局、精確 patch 偵測及單元測試各有用途，均不能單獨證明遊戲效果。原始安裝檔維持唯讀；ar.exe 保護／歷史研究暫緩。
