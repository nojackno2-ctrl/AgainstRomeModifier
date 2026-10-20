# Rust 桌面入口進度

`apps/against-rome-editor` 使用 eframe/egui 0.27.2 建立獨立 Rust 桌面程式，不啟動 .NET。除地形／通行圖外，現有入口還包含材質、放置、scenario/event、environment/Nature/wildlife、ambience、navigation、AI planning、modifier、save manager、diff 與 packaging 等面板；各面板的 parity 與完整工作流程仍不等同於整體改寫完成。完整功能範圍見 [rust-rewrite.md](rust-rewrite.md)。

## 資料與行為

- 高度讀取 boden.bmp 綠通道；碰撞依現有 C# 讀取 RGB 平均值。尺寸以實際 BMP 為準，不能從 Heightmapstep 推導頂點數；Heightmapstep 是高度換算參數。193/192 與257/256配對需分別驗證。
- 地形運算已共用 native/against-rome-editor 經 C# differential 驗證核心。Agy 初稿的獨立 domain 已替換為 adapter，量化、歷史與 collision RGB 語意差異已修正並測試。
- 未修改像素、BMP header／padding 與原有碰撞色彩應保留；跨圖層保存必須對一般 I/O 失敗回滾。兩個檔案無法以兩次 rename 宣稱斷電原子性。
- 原遊戲安裝目錄只讀取；編輯結果存至工作副本。空白高度圖是地形圖層起點，尚不是可在遊戲開局的完整地圖。

## 執行與驗證

- `--map DIRECTORY --validate-map` 實際解析圖層並輸出尺寸，唯讀。
- `--map DIRECTORY --smoke-test` 建立原生視窗、實際跑三次 UI update 後關閉；不寫入圖層。這只驗證桌面啟動，不代表所有互動驗收。
- `--help`／`--version` 輸出使用方式與版本。
- 已移除初稿只印 OK 的 --check，避免將空檢查當成視窗驗證。

2026-10-15 Parent 驗證：桌面22 tests通過；含既有資料核心、新核心與獨立 differential 的整合指令共193 pass／0 fail。TUTOR_01（193／192）與 ENDL_000（257／256）唯讀解析成功，TUTOR_01 原生視窗跑三次 UI update 後正常退出，四個來源 BMP SHA-256 前後不變。這不是完整 UI 操作或原遊戲驗收。紀錄見 AI_HANDOFF.md 與 re_workspace/rust-rewrite-* logs。

## 尚未完成或尚未驗收

仍需逐項完成各 Rust 面板與 C# 工作流程的 parity、完整地圖與零售地圖往返、GUI 實際操作、GPU/3D 場景呈現、Rust 發行流程取代 .NET 發行，以及授權範圍內的遊戲接受測試。headless 測試通過只能證明測試覆蓋的程式行為，不能作為這些項目的完成證據。

2026-10-19 核對：Rust app 已有 Nature Plant/Remove brush，包含 map overlay、stroke painting、undo/redo、dirty guard 與 Safe Save 的 DATA transaction。`cargo test --manifest-path apps/against-rome-editor/Cargo.toml --locked --offline --target-dir re_workspace/rust-editor-app-target nature`：8 passed、0 failed；完整 `tools/test-rust-rewrite.ps1 -Offline -IncludeDesktop` exit 0。尚未做 GUI/retail map/game 驗收。

2026-10-19 新增：Neutral Wildlife 面板可依 map template catalog 將已觀察到的野生動物 clone 到 Objects session，採樣地形高度並以 neutral team 8 寫入；模板與 cloneable type 必須完整解析、最多 96 個一個 undo batch，否則保留草稿並拒絕套用。Safe Save/reload app test 通過。這只落實現存 animal object placement，不宣稱有已驗證 lair structure、guard、respawn 或 spawn waves。
