# `against-rome-ai-planner` Handoff

## 概覽與任務定義
- **工作包**：AI 地圖規劃與放置排版移植（AI Map Planning & Placement Layouts）
- **獨佔路徑**：`native/against-rome-ai-planner/**`
- **相依 crate**：`native/against-rome-editor`、`native/against-rome-textures`
- **目標範圍**：
  1. `AiEditorOperator`: AI 筆刷操作執行器，將 AI 規劃的地圖特徵轉換為編輯器筆刷指令（`/brush height/collision/material`），維持 commit bdf3b57 的「AI 驅動編輯器筆刷」閉環操作設計，而不是直接寫入檔案。
  2. `AiMapEditScope`: 地圖編輯區域與鎖定矩形防護合約，嚴格保護指定 tile 與頂點邊界不被修改。
  3. `AiMapPlan`: AI 地圖計畫模型，包含地圖摘要、baseHeight、baseMaterial、特徵陣列（hill, mountain, valley, plateau, lake, ridge, river, material, blocked, passable），方位名稱（Locations）到 tile 座標的確定性換算，正規化與 Ollama JSON Schema 輸出。
  4. `AiMapPlanPreview`: 獨立記憶體預覽影像建構器，不改動地圖與實體檔案，精確烘焙地形、水面、材質變化、拒絕區域與阻擋標記。
  5. `MultiAiMapPlanner`: 多 AI 專家協調器（地形 Terrain、水系 Water、材質 Materials），支援個別模型指派、進度回報、角色範圍約束、特徵配額截斷（8 個/角色）與重複項移除。
  6. `OllamaMapPlanner`: 本機 Ollama HTTP 用戶端，支援抽象 `HttpTransport`（提供 `MockHttpTransport` 與 `TcpHttpTransport`），在無 Ollama 服務時可離線執行單元測試。
  7. `FormationLayout` & `FormationLayoutDefinition`: 原生 `formdef.dau` 線段取樣、id 0 `All_Haufen` 9 條有效線段嵌入、中點間隔採樣、世界 X/Z 鏡像與角度旋轉。
  8. `LayoutTypeResolver`: 別名解析器，將簡寫名稱（如 `BauGerHau00`）解析為目錄實存別名或模板鍵。
- **狀態**：完成 (Completed)

## 完成進度記錄
- [x] 建立 `native/against-rome-ai-planner/Cargo.toml`
- [x] 實作 `src/scope.rs`（`Rectangle`, `AiMapEditScope`）
- [x] 實作 `src/plan.rs`（`AiMapPlan`, `AiMapFeature`, `AiMapApplyResult`, 方位換算, 正規化, JSON Schema）
- [x] 實作 `src/brush.rs`（`EditorBrushStroke`, `EditorBrushTool`, `EditorHeightBrush`, `EditorBrushResult`, `EditorBrushExecutor`, `TerrainStrokePath`）
- [x] 實作 `src/operator.rs`（`AiEditorOperator`，閉環筆刷調用、蛇形與圓形路徑規劃、`/brush` 指令與文字記錄）
- [x] 實作 `src/preview.rs`（`AiMapPlanPreview`, `AiMapPlanPreviewBuilder`, `RgbaImage` 預覽烘焙）
- [x] 實作 `src/multi_planner.rs`（`MultiAiMapPlanner`, `AiMapDesignRole`, `MultiAiMapPlanResult`）
- [x] 實作 `src/ollama.rs`（`OllamaMapPlanner`, `AiMaterialOption`, `HttpTransport`, `MockHttpTransport`, `TcpHttpTransport`）
- [x] 實作 `src/formation.rs`（`FormationLayout`, `FormationLayoutDefinition`, `FormationLayoutSegment`）
- [x] 實作 `src/layout_resolver.rs`（`LayoutTypeResolver`, `SdlObjectType`）
- [x] 實作 `src/lib.rs` 匯出所有公開介面與型別
- [x] 完整測試覆蓋（8 組測試檔案，共 37 項測試全部通過，0 失敗，0 略過）：
  - `tests/scope_tests.rs`: 3 passed
  - `tests/plan_tests.rs`: 7 passed
  - `tests/operator_tests.rs`: 4 passed
  - `tests/preview_tests.rs`: 1 passed
  - `tests/ollama_tests.rs`: 6 passed
  - `tests/multi_planner_tests.rs`: 5 passed
  - `tests/formation_tests.rs`: 6 passed
  - `tests/layout_resolver_tests.rs`: 5 passed

## 驗證指令與結果
- **指令**：
  ```powershell
  $env:CARGO_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo"
  $env:RUSTUP_HOME="D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup"
  $env:PATH="$env:CARGO_HOME\bin;$env:PATH"
  cargo test --offline --manifest-path native/against-rome-ai-planner/Cargo.toml --target-dir re_workspace/target-ai-planner
  ```
- **測試結果**：
  `test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
