# WP-07: 地圖打包、差異比對與預算 (against-rome-packaging)

## 狀態摘要
- 建立日期：2026-10-15
- 代理：Antigravity (WP-07)
- 檔案所有權：`native/against-rome-packaging/**`
- 目前狀態：**已全數完成移植與驗證** (28/28 測試通過，0 failed，0 warning)。

## 模組範圍與進度
1. [x] 架構規劃與 C# 原始程式盤點 (`Cargo.toml`)
2. [x] `contract.rs` (嚴格 Payload 驗證、DOS 裝置字元過濾、Windows Reparse Points / Symlink 防護)
3. [x] `manifest.rs` (MapPackageManifest、嚴格 SHA-256 Checksum 計算、.NET 布林字串大寫格式對齊、briefing.put & boden.ini 自動推導)
4. [x] `thumbnail.rs` (MinimapThumbnailRenderer：24-bit 未壓縮 Windows BMP、Horn's 3x3 山體陰影算法、Lambertian 光照、水體漸層/海岸線、Bresenham 道路厚線、隊伍 0..7 軍旗圓盾標記與軟陰影)
5. [x] `preflight.rs` (MapExportPreflightChecker：BMP 54-byte 結構體檢、語意診斷、出生點/SDL 檢查、勝利失敗動作檢查、快取排除排查)
6. [x] `exporter.rs` (ModBundleExporter：匯出至 ZIP、從 ZIP 安裝、自動槽位重映射、SDL 內部路徑改寫、briefing 標題替換、原廠槽位嚴格拒絕、回滾清理防護)
7. [x] `diff/` (MapDiffEngine、四階段物件配對、材質轉移矩陣、碰撞格阻擋通行、事件差異比對、SelectiveRollbackController 餘弦邊界羽化、MapVisualDiffOverlay 遮罩與小地圖渲染)
8. [x] `profiler/` (MapBudgetLimits、MapBudgetReport、MapBudgetProfiler：物件池分類、32x32 光源網格與 NMS 聚集、繪圖呼叫/圖集估算、尋路 Flood Fill、隘口收縮聚類、死胡同計算與整體健康評分)
9. [x] 單元與整合測試套件（`tests/packaging_tests.rs`、`tests/diff_tests.rs`、`tests/profiler_tests.rs`，完整對齊 C# 測試）
10. [x] 驗證測試執行與記錄（測試指令與通過數如下）

## 驗證測試指令與結果
- **測試指令**：
  ```powershell
  $env:CARGO_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo"
  $env:RUSTUP_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup"
  $env:PATH="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo\bin;$env:PATH"
  cargo test --manifest-path native/against-rome-packaging/Cargo.toml --target-dir re_workspace/target-against-rome-packaging
  ```
- **測試通過數據**：
  - `diff_tests`: **9 passed**, 0 failed
  - `packaging_tests`: **10 passed**, 0 failed
  - `profiler_tests`: **9 passed**, 0 failed
  - **總計：28 passed, 0 failed, 0 warnings**
