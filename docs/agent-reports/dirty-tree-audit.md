# 工作區未提交變更審查報告 (Dirty Working Tree Audit)

**報告日期**：2026-10-14  
**工作目錄**：`D:\Github\AgainstRomeModifier`  
**分支狀態**：主要開發分支 (HEAD: `140b76d`)  
**變更規模**：45 個已追蹤修改檔案 (Modified)、85 項未追蹤檔案或目錄 (Untracked，不含本報告自身)，共計 130 項變更。

---

## 1. 邏輯變更集分組 (Logical Change Sets)

本工作區中累積的未提交變更源於多個 Agent 與使用者的前續開發，涵蓋原生資料池逆向、跨池交易複製、3D 場景光影渲染、動畫視覺狀態、粒子系統及安裝媒體驗證。以下將所有 dirty 檔案歸納為 9 個具體的業務邏輯群組，外加 1 個應排除的本機產物群組。

---

### 群組 1：Agent 協同規範與交接紀錄 (Agent Collaboration & Handoff History)
- **變更一覽**：
  - `AGENTS.md` (Modified)
  - `AI_HANDOFF.md` (Modified)
  - `docs/ai-handoff-history-2026-10-11-before-rust-pause.md` (Untracked)
- **變更簡述**：更新自動提交與 GitHub 推送政策規則、精簡活頁交接紀錄，並將 Rust 轉向前的舊交接紀錄完整封存至專屬歷史文件。
- **建議 Commit 訊息**：
  ```
  docs(agent): update collaboration guidelines and archive handoff history
  ```

---

### 群組 2：原生光源與 3D 場景渲染整合 (Native Lighting & 3D Scene Rendering)
- **變更一覽**：
  - **核心與資產模組**：
    - `src.Shared/Maps/NativeLightPool.cs` (Untracked)
    - `src.Shared/Maps/NativeLightDefinitions.cs` (Untracked)
    - `src.Shared/Maps/RustLightPool.cs` (Untracked)
    - `src.MapEditor.Modules/NativeAssets/NativeAptDocument.cs` (Modified)
    - `src.MapEditor.Modules/NativeAssets/NativeAptLightPoints.cs` (Modified)
    - `src.MapEditor.Modules/NativeAssets/NativeAptAttachmentGeometry.cs` (Untracked)
    - `src.MapEditor.Modules/NativeAssets/NativeLightSource.cs` (Modified)
    - `src.MapEditor.Modules/NativeAssets/SceneLightingContext.cs` (Modified)
  - **編輯器宿主與著色器**：
    - `src.MapEditor/Map3DViewControl.cs` (Modified)
    - `src.MapEditor/SceneGeometryBuffers.cs` (Modified)
    - `src.MapEditor/SceneObjectRenderer.cs` (Modified)
    - `src.MapEditor/TerrainHeightField.cs` (Modified)
    - `src.MapEditor/MapEditorForm.SavedLights.cs` (Untracked)
  - **單元與整合測試**：
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeLightTests.cs` (Modified)
    - `tests/AgainstRomeMapEditor.Modules.Tests/SceneLightingContextTests.cs` (Modified)
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeAptAttachmentTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/LevelObjectDirectLightTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/NativeLightPoolTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/NativeLightDefinitionsTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/RustLightPoolTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/RustLightProjectionTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/SavedLightRenderingTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorSavedLightPreviewTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/NativeSceneElevationTests.cs` (Untracked)
  - **逆向文件與探針工具**：
    - `docs/reverse-engineering/map-lighting.md` (Modified)
    - `docs/reverse-engineering/saved-light-render-contract.md` (Untracked)
    - `tools/re/apt-probe/Program.cs` (Modified)
    - `tools/re/apt-probe/apt-probe.csproj` (Modified)
    - `tools/re/saved-light-contract-rs/` (Untracked directory)
    - `tools/re/probe_apt_attachment_projection.py` (Untracked)
    - `tools/re/probe_direct_light_transform.py` (Untracked)
    - `tools/re/probe_native_light_pool.py` (Untracked)
    - `tools/re/probe_native_light_definitions.py` (Untracked)
    - `tools/re/probe_saved_light_render_contract.py` (Untracked)
    - `tools/re/probe_native_scene_projection.py` (Untracked)
- **變更簡述**：解析 `light.dat` 原生光源池快照與 `lightdef.dau`，實作 APT 附件光源幾何投影，透過 Rust FFI 進行光源位置投影，並在 OpenGL 著色器中分離原生單位高程與編輯器地形浮雕。
- **建議 Commit 訊息**：
  ```
  feat(lighting): integrate native light pool, APT attachment projection, and 3D scene lighting
  ```

---

### 群組 3：物件池管理、資料池診斷與地圖載入契約 (Object Store, Native Pools & Diagnostics)
- **變更一覽**：
  - **核心邏輯**：
    - `src.Shared/Maps/LevelObjectStore.cs` (Modified)
    - `src.Shared/Maps/NativePoolDiagnostics.cs` (Modified)
    - `src.Shared/Maps/BlankMapContent.cs` (Modified)
    - `src.Shared/Maps/NativeObjectDefinitions.cs` (Untracked)
    - `src.Shared/Maps/NativeMapLoadContract.cs` (Untracked)
    - `src.Shared/Maps/NativeLagerPool.cs` (Untracked)
    - `src.MapEditor/MapEditorForm.Diagnostics.cs` (Modified)
  - **測試套件**：
    - `tests/AgainstRomeModifier.Tests/LevelObjectStoreTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/LevelObjectStoreGameTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/NativePoolDiagnosticsTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/NativeObjectDefinitionsTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/NativeLagerPoolTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorNativeLoadContractTests.cs` (Untracked)
  - **逆向文件與探針工具**：
    - `docs/reverse-engineering/map-data-pools.md` (Modified)
    - `docs/reverse-engineering/map-load-pipeline.md` (Modified)
    - `docs/reverse-engineering/objdata-contract.md` (Modified)
    - `docs/reverse-engineering/objdef-fields.csv` (Modified)
    - `docs/reverse-engineering/object-create-contract.md` (Modified)
    - `tools/re/probe_map_load_contract.py` (Untracked)
    - `tools/re/probe_native_lager_pool.py` (Untracked)
    - `tools/re/probe_native_object_parents.py` (Untracked)
    - `tools/re/probe_object_position_links.py` (Untracked)
    - `tools/re/probe_optional_creation_contract.py` (Untracked)
    - `tools/re/probe_type_position_contract.py` (Untracked)
    - `tools/re/probe_typus_creation_contract.py` (Untracked)
    - `tools/re/probe_uid_generation_contract.py` (Untracked)
- **變更簡述**：於 `LevelObjectStore` 導入物件父子雙向反向連結 (Parent Backlink)、直接光源欄位、第三位置 (Column 10/afstr) 校驗與營地池 (lager/biglager)，並完善地圖資料完整性診斷與空白地圖初始化契約。
- **建議 Commit 訊息**：
  ```
  feat(store): enhance cross-pool diagnostics, parent-child links, and camp pool support
  ```

---

### 群組 4：群組物件交易式複製、平移與放置 (Supported Family Transaction & Placement Copy)
- **變更一覽**：
  - **核心與腳本契約**：
    - `src.Shared/Maps/LevelObjectStore.Families.cs` (Untracked)
    - `src.Shared/Maps/SdlObjectCatalog.cs` (Modified)
    - `src.Shared/Scripting/LevelScriptInjector.cs` (Modified)
    - `src.Shared/Scripting/ScenarioLevelObjects.cs` (Modified)
    - `src.MapEditor.Modules/Placement/PlacementEditSession.cs` (Modified)
  - **地圖編輯器介面與生命週期**：
    - `src.MapEditor/MapEditorForm.FamilyPreview.cs` (Untracked)
    - `src.MapEditor/MapEditorForm.NativeFamilies.cs` (Untracked)
    - `src.MapEditor/MapEditorForm.Nature.cs` (Modified)
    - `src.MapEditor/MapEditorForm.Persistence.cs` (Modified)
    - `src.MapEditor/MapEditorForm.Placement.cs` (Modified)
    - `src.MapEditor/MapEditorForm.Scene.cs` (Modified)
    - `src.MapEditor/MapEditorForm.cs` (Modified)
  - **測試套件**：
    - `tests/AgainstRomeModifier.Tests/LevelObjectStoreCrossPoolTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/MapEditorBuildingFallbackTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/MapEditorSavePlacementRegressionTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/MapEditorTribeTeamIntegrationTests.cs` (Modified)
    - `tests/AgainstRomeModifier.Tests/MapEditorFamilyPreviewTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorNativeFamilyCopyTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorSavedFamilyCopyTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorSavedFamilyDiagnosticsTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/PlacementDirectionTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/FamilyCopyFailureTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeRootFamilyCopyIntentTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeSourceFamilyCandidateTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeSourceMetadataGuardTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/PlacementFamilyCopyIntentTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/PlacementDirectionTests.cs` (Untracked)
  - **逆向探針**：
    - `tools/re/probe_family_dependency_frontier.py` (Untracked)
    - `tools/re/probe_family_movement_contract.py` (Untracked)
- **變更簡述**：支援以獨立閉包為單位的物件群組 (Family) 交易式深複製、定點數平移與旋轉方向管理，嚴格保留原生精確旋轉弧度 (`RawRotation`) 與未綁定原圖來源身份。
- **建議 Commit 訊息**：
  ```
  feat(placement): implement transactional family copy, rotation retention, and editor UI integration
  ```

---

### 群組 5：原生動畫視覺狀態解碼與預覽 (Native Animation & Visual State Preservation)
- **變更一覽**：
  - **核心與資產解析**：
    - `src.Shared/Maps/LevelObjectStore.VisualState.cs` (Untracked)
    - `src.Shared/Maps/NativeAnimationVisualState.cs` (Untracked)
    - `src.Shared/Maps/SdlSceneCatalog.cs` (Modified)
    - `src.MapEditor.Modules/NativeAssets/NativeAptDocument.cs` (Modified)
    - `src.MapEditor.Modules/NativeAssets/NativeSpriteCatalog.cs` (Modified)
  - **測試套件**：
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeAnimationVisualStateTests.cs` (Untracked)
    - `tests/AgainstRomeMapEditor.Modules.Tests/NativeSpriteVisualStateTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorNativeVisualStateTests.cs` (Untracked)
  - **文件與探針**：
    - `docs/reverse-engineering/native-scene-rendering.md` (Modified)
    - `tools/re/probe_native_visual_state.py` (Untracked)
- **變更簡述**：自 `anim.dat` 解讀已儲存之 ALR 線性影格與 APT 四軸姿態狀態，傳遞至精靈渲染器進行真實幀位預覽。
- **建議 Commit 訊息**：
  ```
  feat(rendering): restore native animation visual state and APT multi-axis preview
  ```

---

### 群組 6：粒子系統逆向、Rust 粒子池與 ALR 標頭規格化 (Particle Systems & ALR Metadata)
- **變更一覽**：
  - **Rust Crates**：
    - `native/against-rome-data/src/lib.rs` (Modified)
    - `native/against-rome-data/src/alr_metadata.rs` (Untracked)
    - `native/against-rome-data/tests/alr_metadata.rs` (Untracked)
  - **C# 橋接與編輯器支援**：
    - `src.Shared/Maps/NativeParticlePool.cs` (Untracked)
    - `src.Shared/Maps/RustParticlePool.cs` (Untracked)
    - `src.Shared/Maps/RustParticleLightProjection.cs` (Untracked)
    - `src.MapEditor/MapEditorForm.SavedParticles.cs` (Untracked)
  - **測試套件**：
    - `tests/AgainstRomeModifier.Tests/NativeParticlePoolTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/RustParticlePoolTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/RustParticleLightProjectionTests.cs` (Untracked)
    - `tests/AgainstRomeModifier.Tests/MapEditorSavedParticlePreviewTests.cs` (Untracked)
  - **文件與探針**：
    - `docs/reverse-engineering/particle-alr-metadata.md` (Untracked)
    - `tools/re/probe_native_particle_pool.py` (Untracked)
    - `tools/re/probe_particle_creation_contract.py` (Untracked)
- **變更簡述**：解析 `particle.dat` 發射器與子光源連動，實作 Rust 粒子坐標 SAR16 投影橋接，並在 Rust 中規範化解析 ALR v4..6 標頭中繼資料。
- **建議 Commit 訊息**：
  ```
  feat(particle): add particle pool parsing, Rust projection bridge, and ALR metadata validation
  ```

---

### 群組 7：安裝媒體基準比對與 Wise 安裝包探索 (Install Media Baseline & Wise Probing)
- **變更一覽**：
  - `docs/reverse-engineering/install-media.md` (Untracked)
  - `tests/AgainstRomeModifier.Tests/InstallMediaBaselineTests.cs` (Untracked)
  - `tools/re/probe_wise_media.py` (Untracked)
- **變更簡述**：直接對使用者提供之 ISO 映像檔建立虛擬資料流解析 Wise 安裝包多卷結構，建立乾淨的原廠資產指紋比對基準。
- **建議 Commit 訊息**：
  ```
  docs(re): add install media baseline verification and Wise installer probe
  ```

---

### 群組 8：專案建構目標與使用者手冊更新 (Build Targets & User Guide)
- **變更一覽**：
  - `Directory.Build.targets` (Untracked)
  - `docs/map-editor-user-guide.md` (Modified)
- **變更簡述**：新增 MSBuild 自訂目標以自動部署 Rust `against_rome_data.dll` 至輸出與發布目錄，並更新編輯器操作指引。
- **建議 Commit 訊息**：
  ```
  build: automate native Rust DLL deployment and update user guide
  ```

---

### 群組 9：獨立逆向探索探針與旗幟類型表 (Standalone RE Probes & Native Tables)
- **變更一覽**：
  - `src.Shared/Maps/NativeMarkerTypeTable.cs` (Untracked)
  - `tests/AgainstRomeModifier.Tests/NativeMarkerTypeTableTests.cs` (Untracked)
  - `tools/re/probe_native_banner_types.py` (Untracked)
  - `tools/re/probe_native_float_conversion.py` (Untracked)
  - `tools/re/replay_x87_conversion.py` (Untracked)
  - `tools/re/probe_graphics_fpu_frontier.py` (Untracked)
- **變更簡述**：包含部族旗幟類型 (Banner Types) 選擇表格之獨立解析與測試，以及 x87 浮點指令與圖形投影探索探針。
- **建議 Commit 訊息**：
  ```
  re(probes): add banner type table and floating-point conversion probes
  ```

---

### 群組 10：本機執行期產物、暫存檔案與本機設定 (Local Runtime Artifacts & Config - 不應提交)
- **變更一覽**：
  - `.claude/` (`.claude/settings.local.json`) (Untracked)
  - `TEMP/` (包含 `ArmAgy*QA/`, `RunTerrainTests.dll`, `TestRunner.cs`, xunit DLLs 等) (Untracked directory)
  - `native/against-rome-data/target/` (包含 `debug/`, `tmp/`, `.rustc_info.json` 等) (Untracked directory)
- **說明**：詳見第 3 節。

---

## 2. 變更檔案分組對照清單 (Summary Table)

| 群組名稱 | 包含檔案數 (M/??) | 涵蓋核心領域 | 建議 Commit 類型 |
|---|---|---|---|
| **1. Agent 協同規範與紀錄** | 3 (2 / 1) | 協同規則、交接狀態、歷史封存 | `docs(agent)` |
| **2. 原生光源與 3D 渲染** | 34 (13 / 21) | light.dat、APT 幾何投影、Rust FFI、GL 雙座標著色器 | `feat(lighting)` |
| **3. 物件池管理與資料診斷** | 26 (11 / 15) | LevelObjectStore、Parent Backlink、第三位置、營地池 | `feat(store)` |
| **4. 群組物件複製與放置** | 29 (12 / 17) | Family 閉包交易式複製、RawRotation 保留、方向旋轉與 Form UI | `feat(placement)` |
| **5. 原生動畫與視覺狀態** | 10 (4 / 6) | anim.dat 線性影格、APT 4 軸解碼、精靈預覽 | `feat(rendering)` |
| **6. 粒子系統與 ALR 標頭** | 14 (1 / 13) | particle.dat、Rust 投影、ALR 23-word 標頭解析 | `feat(particle)` |
| **7. 安裝媒體基準比對** | 3 (0 / 3) | ISO 虛擬流、Wise 多卷解析、基線指紋測試 | `docs(re)` |
| **8. 建構整合與使用者手冊** | 2 (1 / 1) | Directory.Build.targets、user guide 更新 | `build` |
| **9. 獨立探針與旗幟類型** | 6 (0 / 6) | Banner 標記表、x87 浮點轉換探針 | `re(probes)` |
| **10. 本機產物 (排除)** | 3 (0 / 3) | TEMP、Rust target、Claude settings | *(不提交)* |
| **總計** | **130 (45 / 85)** | 全工作區髒檔案與未追蹤項目完整覆蓋 | — |

---

## 3. 執行期產物、機密資料與意外檔案審查 (Runtime Data, Secrets & Accidental Artifacts)

經逐檔與檔案樹檢查，未發現任何 API Key、密碼、Token、私鑰等敏感機密資料。但有以下**屬於本機建構產物、暫存檔案或本機工具設定**的項目，嚴禁提交至 Git 倉庫：

1. **`TEMP/` 目錄 (強烈建議加入 `.gitignore` 並清除)**：
   - 內含先前的 QA 自動化測試遺留目錄：`ArmAgyNatureQA/`, `ArmAgyPlacementQA/`, `ArmAgySaveQA/`, `ArmAgyTerrainQA/`，其中含有大量編譯過程的中間檔案 (`.dll`, `.pdb`, `.cache`, `project.assets.json`)。
   - 包含編譯後的測試執行檔與依賴：`RunTerrainTests.exe`, `RunTerrainTests.dll`, `TerrainSculptTest.dll`, `xunit.*.dll` 及 `TestRunner.cs`。
   - 這些均為一次性執行期暫存檔案，根目錄 `.gitignore` 未排除 `TEMP/`，導致其暴露為未追蹤狀態。
2. **`native/against-rome-data/target/` (Rust 建構輸出目錄)**：
   - 包含編譯 Rust crate 所產生的 `debug/`, `tmp/`, `.rustc_info.json`, `CACHEDIR.TAG`。
   - 根目錄 `.gitignore` 僅有 `[Bb]in/` 與 `[Oo]bj/`，未涵蓋 Rust 標準的 `target/`；且該子 crate 目錄下缺乏獨立的 `.gitignore`。
3. **`.claude/settings.local.json` (Agent 本機環境授權設定)**：
   - 內容為本地命令權限允許規則 (`"Bash(DOTNET_ROLL_FORWARD=Major ARM_GAME_PATH=*)"`)。
   - 根目錄 `.gitignore` 忽略了 `.agents/` 與 `.codex/`，但遺漏了 `.claude/`。此為使用者端本地設定，不應進入共用程式庫。

---

## 4. 可疑不一致性與潛在斷鏈分析 (Suspicious Inconsistencies)

在審查現有修改與未追蹤檔案的對應關係時，發現以下幾處顯著的設計不一致性、未完成的實作斷鏈或潛在風險：

1. **`NativeMarkerTypeTable` 與主專案處於孤立狀態 (Orphaned Feature)**：
   - `src.Shared/Maps/NativeMarkerTypeTable.cs` 實作了依據部族、旗幟版本與 engine 進行旗幟選取的邏輯，並配有完整測試 `tests/AgainstRomeModifier.Tests/NativeMarkerTypeTableTests.cs` 與探針 `tools/re/probe_native_banner_types.py`。
   - **不一致點**：在 `src.Shared`、`src.MapEditor` 或主修改器中，**完全沒有任何呼叫端引用 `NativeMarkerTypeTable`**。這是一組有程式碼與測試但未被編輯器或場景載入串接的孤立模組。
2. **Rust `alr_metadata.rs` 模組導出但缺乏 FFI 與宿主接線 (Incomplete Rust Integration)**：
   - 在 `native/against-rome-data/src/lib.rs` 中加入了 `pub mod alr_metadata;`，並編寫了完整整合測試 `tests/alr_metadata.rs`。
   - **不一致點**：該模組並未像 `alr_scanlines` 一樣提供任何 C-ABI / FFI 導出 (`extern "C"`)，C# 端也沒有相應的 P/Invoke 橋接。查閱 `AI_HANDOFF.md` 紀錄可知，該測試先前曾受 Windows CodeIntegrity 4551 阻擋，因此該功能僅停留在 Rust 內部 crate 階段，未真正部署至 DLL 或整合進地圖編輯器。
3. **`Directory.Build.targets` 為未追蹤狀態 (Fragile Build Pipeline)**：
   - `Directory.Build.targets` 定義了將 `re_workspace/native-data-deploy/against_rome_data.dll` 自動複製至 C# 專案輸出與發布目錄的 MSBuild 邏輯。
   - **不一致點**：此檔案目前為未追蹤狀態。若其他開發者或 CI 環境在乾淨環境下 clone 本 repo，發布時將因缺少此 targets 而無法正確複製 Native DLL，造成發布產物斷鏈。
4. **`tools/re/apt-probe` 擴充功能未受自動化測試覆蓋**：
   - `tools/re/apt-probe/Program.cs` 新增了 `--attachments-only` 參數與附件幾何驗證，並在 `.csproj` 中鏈結了 `NativeAptAttachmentGeometry.cs`。
   - **不一致點**：此工具變更僅透過手動執行指令驗證，`tests/` 中並未包含針對 `apt-probe` 本身的測試案例。
5. **探索性逆向探針缺少對應實作 (Exploratory Probes without Implementation)**：
   - `tools/re/replay_x87_conversion.py` 與 `tools/re/probe_graphics_fpu_frontier.py` 探索了遊戲 EXE 內部的 x87 浮點指令與投影邊界。
   - **不一致點**：工作區中尚未有對應的 C# 或 Rust 模組承接此項逆向成果（`AI_HANDOFF.md` 亦指出 velocity/direction x87 轉換仍屬 unknown 狀態）。
6. **`AI_HANDOFF.md` 大幅刪減與歷史檔案未提交之斷代風險**：
   - `AI_HANDOFF.md` 刪減了 1027 行歷史，並將其複製至未追蹤檔案 `docs/ai-handoff-history-2026-10-11-before-rust-pause.md`。
   - **不一致點**：若未來只提交部分修改而漏掉此未追蹤的歷史檔案，將造成 2026-10-11 以前詳細的逆向推導與驗證歷程永久遺失。

---

## 5. 總結與維護建議

- **保留未提交工作樹**：本輪稽核確認所有 130 項修改均為先前任務保留下來的有效成果與相應測試，符合 `AGENTS.md` 所述之「保留既有 dirty 狀態」。
- **`.gitignore` 補強建議**：在後續的適當維護階段，應將 `.claude/`、`TEMP/` 以及 `native/against-rome-data/target/` 納入 `.gitignore`，避免本地產物干擾原始碼狀態。
- **後續提交建議順序**：若使用者授權進行分段提交，建議依上述群組順序（群組 8 建構目標 → 群組 3 核心池 → 群組 2 光源 → 群組 4 群組複製 → 群組 5 動畫 → 群組 6 粒子 → 群組 1 協同文件）分別獨立進行 Verification、Commit 與 Push。
