# 真實大型地圖效能驗證

2026-10-08 Codex；使用現有授權 TEMP 副本，未存取遊戲安裝目錄。

來源 `ArmGameCompare_20261007/ENDL_000`：6,618 DATA 物件、7,231 個完整場景物件、7,143 原生 sprite、2,050 陰影、9 個可播放動畫物件、50 個場景光源。新 catalog 219 種名稱／隊伍／角度组合中有212種可解碼 sprite。不是合成7000物件。

OpenGL 3.3、NVIDIA GeForce RTX 4080、driver 617.14。本機數字是特定負載與裝置的量測，不能推論所有硬體或遊戲內效能。

## 方法與證據

- `ARM_MAP_PERFORMANCE=1` 明確啟用；來源及全新輸出只允許TEMP，遞迴拒絕reparse points。全來源每個檔案以stream SHA256在前後比對，表單僅讀官方圖、dirty為false。
- 各模式12幀暖身、120幀量測，實際 `Refresh→OnPaint→RenderScene→SwapBuffers→GL.Finish`。`PaintFrameCount` 必須恰好增加120。持續使用與視圖尺寸相同的color/depth FBO，確保offscreen視窗仍真正GPU渲染；每case末幀讀回，拒絕全單色結果，保存PNG。讀回／PNG寫檔不計入幀時間。
- 相機先對焦圖集真正接受動畫的武器工坊 `BauGerWaf00_Waffenschmiede`（world11104,15072），原生1:1縮放。中鍵拖曳走真實 `OnMouseDown/Move/Up`，包括hover、地形picking及小地圖回呼。動畫固定時鐘逐幀前進33.33ms；光照時刻12:00。
- 這是完成渲染的吞吐FPS，包含同步GPU完成，**不是螢幕呈現FPS**。正式動畫timer仍為33ms；未清OS檔案快取、各case共用資產快取。CPU／managed／process記憶體不含VRAM。
- 原路徑量測起始commit `a9a3799` 加相同benchmark，暫用原 `BuildSpriteVertices/BuildShadowVertices`；最終量測加入重用緩衝／增量動畫。兩次源檔hash相同、10張量測末幀與overview共11張PNG SHA256全部相同，`comparison.json`保存逐圖hash與逐case結果。靜態與動畫PNG不同，證明可見動畫確實切格。

最終證據：`%TEMP%/ArmMapPerformance_20261008_settlement_before` 與 `..._settlement_exact`，各有 `report.json`、`stages.json`、`frames.json`、`source-hashes.json`、量測PNG；exact另有 `comparison.json`。已目視工坊、圍牆、地景與陰影。較早 `_before/_after/_final`、`_fbo_*` 是開發中量測；`_settlement_final` 的38個邊緣像素差異已修，不能當作像素一致證據。

## 原路徑與最終結果

| 模式 | 動作 | 原平均 ms | 最終平均 ms | 最終 p95 ms | 原 bytes/幀 | 最終 bytes/幀 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 地形 | 靜止 | 0.28 | 0.22 | 0.37 | 136 | 136 |
| 地形 | 拖曳 | 0.37 | 0.21 | 0.39 | 778 | 778 |
| 全物件靜態 | 靜止 | 3.23 | 0.62 | 0.70 | 2,626,507 | 200 |
| 全物件靜態 | 拖曳 | 2.63 | 1.97 | 2.15 | 2,627,050 | 794 |
| 動畫 | 靜止 | 2.62 | 0.48 | 0.53 | 2,626,488 | 232 |
| 動畫 | 拖曳 | 2.69 | 1.91 | 2.09 | 2,627,082 | 826 |
| 光照 | 靜止 | 2.43 | 0.49 | 0.55 | 2,626,546 | 240 |
| 光照 | 拖曳 | 2.51 | 2.00 | 2.25 | 2,628,242 | 1,986 |
| 動畫＋光照 | 靜止 | 2.54 | 0.62 | 0.67 | 2,626,528 | 272 |
| 動畫＋光照 | 拖曳 | 2.51 | 2.07 | 2.25 | 2,628,274 | 2,018 |

原帶物件模式120幀會觸發GC；最終所有case的Gen0/1/2均為0。各case很短且存在暖身／排程波動，不能將動畫或光照個別case稍快視為負成本；配置量與逐像素一致證據較強。

本次最終載入／解碼記錄（不計source hash與PNG輸出）：

| 階段 | 時間 |
| --- | ---: |
| 構造catalog與UI | 1,631 ms |
| 顯示、載圖、首次decode／atlas／首幀 | 2,515 ms |
| 暖資產快取重新載圖 | 665 ms |
| 獨立新sprite catalog開啟 | 22 ms |
| 新catalog首次解碼219種候選的sprite與idle動畫（OS cache暖） | 990 ms |

載圖後managed約388 MiB、working set約1,172 MiB、private bytes約1,424 MiB，當時peak working set約1,219 MiB。首次載圖累計配置約1.96 GB，這是短期與保留配置的總和，不等於resident記憶體；分開的首次decode探測又配置約1.73 GB，且探測時原編輯器仍開著，因此它的process peak不能當作單一編輯器載入需求。仍需依不同硬體實測，這次修正主要處理反覆重繪配置，未宣稱cold disk載入或VRAM降低。

## 實際修正

`SceneSpriteGeometryBuffer` 重用排序清單、頂點陣列與物件quad索引；場景、圖集、高度、地勢比例或深度變換改變時重新排序，動畫只更新變動物件的quad。`SceneShadowGeometryCache` 只在場景、陰影、圖集或地形改變時重建地面陰影。

曾嘗試以yaw/pitch作平移快取鍵，聚落對照發現38個重疊地景像素不同。最終將view.M43也納入深度cache鍵，保留原浮點排序；拖曳仍會重新排序但不每幀配置大陣列。這保留原顯示行為，沒有以改排序規則換取更漂亮的基準。

純測試比對舊頂點算法，覆蓋動畫、旋轉、平移、地形、地勢比例、物件移動／移除與陰影失效；7000物件切格及旋轉暖身後零配置。實際GPU測試及全套回歸另外執行，不能以純測試替代完整圖量測。

## 重跑

在repository的PowerShell：

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
$env:ARM_COMPARE_GAME = Join-Path $env:TEMP 'ArmGameCompare_20261007'
$env:ARM_MAP_PERFORMANCE = '1'
$env:ARM_PERFORMANCE_OUTPUT = Join-Path $env:TEMP ('ArmMapPerformance_' + [guid]::NewGuid().ToString('N'))
$env:ARM_PERFORMANCE_REVISION = (git rev-parse HEAD)
dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build --no-restore --filter 'FullyQualifiedName~Real_large_map_performance' --logger 'console;verbosity=normal'
Remove-Item Env:ARM_MAP_PERFORMANCE
```

來源須是使用者授權的現有TEMP素材副本，有ENDL_000、SYSTEM及原生dat檔；不足1000 DATA、缺少可播放動畫／陰影／光源、GL不可用或未真正畫完，都會拒絕通過。一般回歸不啟用此gate。
