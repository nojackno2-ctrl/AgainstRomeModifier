# 地形製圖缺口審查（2026-10-14，Codex）

## 優先缺口與本次實作

1. **水域筆刷未同步通行區域（本次完成）**：原 `PaintTerrainTile` 只用 Flatten 下挖；設計者需再手畫 collision。新增 `TerrainHeightEditSession.PaintWater`，沿用高度筆刷，在筆刷內以像素中心雙線性高度低於水面判定水下，只將碰撞值 0 改為 255。高度與碰撞共用同一筆畫 Undo／Redo／取消／既有儲存交易。乾岸、筆刷外區域及原非零碰撞保留；缺 collision.bmp 時只改高度並提示。這是明示的編輯器製圖規則，不是推定原生水域／導航演算法。
2. **全域高度／水位／材質修改未普遍同步 collision（仍缺）**：一般升降、改全圖水位及材質繪製尚未自動更新通行區域。坡度與材質的原生導航門檻未在本次核對，不能猜測；也不能無來源標記就清除人工阻擋。此次只涵蓋水域筆刷。
3. **Ramp 未提供（仍缺）**：已有 Raise／Lower／Smooth／Flatten／Roughen 與拖曳補點；直線雙端坡道仍缺。相較水域與通行的直接落差，列為後續。
4. **材質／過渡已有自動化**：`TerrainBlendEditSession` 已具 native corner bake、autoBridge（最多兩圈）及 unsupported junction 原子回滾；應繼續擴充真實圖塊覆蓋，不能合成不存在的 texture 名稱。自然物件已有地區篩選與散播。

## 格式證據與界線

- 已讀 `AGENTS.md`、`AI_HANDOFF.md`、`docs/map-editor-user-guide.md` §2.1，以及地形／水位自動化記憶檢索。記憶 registry 未找到指定「自動處理水位、相容與過渡」的原句；現行程式本身確認已有 WaterBedHeight 與 autoBridge，判斷以 repository 為準。
- `docs/reverse-engineering/map-formats.md`：LDFLOORHMAP（0x483410）讀 boden.bmp 綠通道 × Heightmapstep；LDFLOORCMAP（0x486ec0）與 0x4d87a0 讀 collision 三通道平均，0 可通行、>0 阻擋。沿用既有 TerrainLayerFiles 編碼與儲存，沒有新增磁碟格式。
- 本機 installed `MAPS/TUTOR_01` 僅讀：System.Drawing.Bitmap 解得 boden.bmp **193×193、24bpp**、綠通道 9–167（RGB 並非全灰）；collision.bmp **192×192、24bpp 灰階**、值域 0–255。不可假定所有圖都是 257／256。
- boden SHA-256：`8BE97A51A3058FFB8BDDEE3A578EA4D7051F2AEC291C5CC5868FD7077ACC6E5B`；collision SHA-256：`DFF72D013CDF3044F8BAA72AA959B0B3D129D42C468AE0FECECEC191C2AABEF9`；唯讀檢查前後一致。
- 不改原版檔案；未啟動遊戲，未宣稱原生 pathfinding／遊戲內驗收。此項為既有 C# terrain session／WinForms 整合，未另建 parser 或 Rust core。

## 驗證

- 最終 build **0 errors／8 warnings**；Modules **24 pass／0 fail／0 skip**，Host **21 pass／0 fail／0 skip**，合計 **45 pass**。Host 的兩個真實來源案例已設定唯讀 opt-in 實際執行：TUTOR_01、ENDL_000 的高度／碰撞在 TEMP 編碼重讀一致，未變高度像素保留原 RGB，來源 bytes 不變；未設定環境變數時這兩例不讀安裝目錄。
- 首次 sandbox build 因使用者 NuGet.Config 拒讀而失敗；提權首 build 發現新測試 CS8604，修正 nullable 註記後通過。無檔案鎖重試需求。Logs：`TEMP/terrain-water-{build,modules,host}.log`。
- 指令：

```powershell
dotnet build AgainstRomeModifier.slnx -c Release --nologo
dotnet test tests/AgainstRomeMapEditor.Modules.Tests -c Release --no-build --no-restore --filter "FullyQualifiedName~TerrainWaterPassabilityTests|FullyQualifiedName~TerrainHeightEditSessionTests|FullyQualifiedName~TerrainAutoBridgeTests|FullyQualifiedName~TerrainBlendModuleTests" --nologo
$env:ARM_TERRAIN_READONLY_PATH='C:\Program Files (x86)\Against Rome'
dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build --no-restore --filter "FullyQualifiedName~Water_collision_roundtrip|FullyQualifiedName~Water_brush_updates_collision|FullyQualifiedName~Water_operation_carves|FullyQualifiedName~MapEditorFormTerrainIntegrationTests|FullyQualifiedName~MapEditorTerrainHeightTests" --nologo
```

- 保留已有 TerrainHeightField.cs／MapEditorForm.Nature.cs dirty；未修改其內容或指定禁止檔案。不 commit／push。
