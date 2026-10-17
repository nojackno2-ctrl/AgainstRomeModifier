# Against Rome Modifier v1.3.0 發布說明

本版提供最新可驗證的 legacy Windows x64 主程式 `AgainstRomeModifier.exe`，整合既有 Modifier、MapEditor 與 SaveManager。使用 self-contained single-file 發行方式，無須另外安裝 .NET runtime；不包含遊戲本體、地圖素材或專有 `Backup.zip`，仍需合法取得的 Against Rome 遊戲資料。

## 相較 v1.2.2 的既有累積功能

- 地圖設計：地形高度、通行／碰撞、水域、區域材質與道路，以及自然物件、建築和部隊放置。
- 場景編輯：物件角度與群組操作、計時事件及條件、開局相機、地圖檢查，以及保存交易與重載安全處理。
- 作者工具：侵蝕、懸崖、天候、聚落、目標／過場預覽、控制台巨集與地圖打包；AI 製圖透過編輯器筆刷操作，所需 Ollama／模型須另外設定。
- 3D 檢視使用原生素材的編輯器近似預覽，包含陰影、光照、動畫與粒子；不代表原遊戲像素等價或完整遊戲執行。

上述為既有已提交功能的累積整理；本輪只做發布警告修正、實際測試失敗修正、版本更新與封裝，沒有重寫整個程式。

## 本輪修正

- 修正阻擋 `-warnaserror` 的 XML 參數文件、未使用程式碼、分析器與測試斷言問題，未新增診斷抑制或降低嚴重度。
- 侵蝕運算重用固定大小堆疊緩衝區，避免迴圈重複配置造成堆疊溢位。
- GLFW 回報沒有顯示器時，安全退回既有 2D 編輯模式，避免建立 OpenGL 視窗造成原生存取違規。
- 修正放置面板的高 DPI 捲動，以及 3D fallback 橫幅保留素材缺失原因。

## 驗證狀態

- .NET SDK 10.0.402；Release solution strict full rebuild：0 warnings／0 errors。
- Host：1292 passed／33 skipped／0 failed，總計 1325；最終完整重跑 exit 0。
- MapEditor.Modules：1152 passed／2 skipped／0 failed。
- `Test-ReleaseArtifacts.ps1`：最終重包通過，ZIP 內僅 EXE 且內容 hash 一致、checksum 逐項重算相符；Core assembly/file/product 版本須為 1.3.0.0／1.3.0.0／1.3.0。
- 最終資產與 SHA256 以同目錄 `SHA256SUMS.txt` 及 `D:/Github/audit-arm-release-2026-10-17.md` 為準。

## Rust 與執行期限制

Rust migration 仍為 Partial。此版是 legacy C# Windows host，使用既有 Rust data DLL 處理部分原生資料；不是完整 Rust 桌面替代品。獨立 Rust app、其未完成整合及 cancelled-job dirty source/lockfiles 不屬本版已驗證變更。

本輪工作階段沒有 GLFW 可用顯示器；OpenGL 相關測試可能走既有 fallback，測試成功不能代表 GPU 畫面、context recreation 或實體高 DPI 螢幕已驗證。高 DPI 測試是程式模擬。封裝 EXE 的 `--version` smoke 在啟動前因既有 `requireAdministrator` 要求而被 OS 拒絕（需要提升權限），`--help` 未啟動；未改權限或安全政策。本輪未啟動或修改已安裝遊戲、未做遊戲內地圖驗收、長時間遊玩或完整真素材功能驗收。部分測試因缺少選擇性零售基準而略過；opt-in 接受測試未啟用。

獨立 Rust app stale lockfile、ignored texture CLI source 及 i686 trace parity 等問題維持稽核狀態；本輪未修復或重驗整體 migration，也未變更安全政策。

## 來源身分

- 發布來源：`v1.3.0` 標籤對應的不可變提交；GitHub Release 同時記錄完整 SHA。
- 建置起點：`412a2bb02ee4aea01c54833c664e16f27c8333b9` 加本輪經審查的 C#／版本修正；此起點不是最終發布 commit。
- 發布流程先從相同來源建置 Rust data DLL，再驗證匯出／ABI／SHA256；實際隨包 DLL 雜湊記錄於 GitHub Release 附註。先前本機舊 DLL 候選不作為本次發布資產。
- 完整 Rust 桌面改寫仍未完成；本版本只發布已驗證的現有 Windows 主程式。

開發者相容性：公開模組型別 RollbackLayerFlags 更名為 RollbackLayers，ObjectiveStateChangedEventArgs 更名為 ObjectiveStateChange；外部來源使用者需更新名稱並重新編譯，使用者設定與檔案格式不變。
