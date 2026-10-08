# 平坦與真正空白地圖的建立條件

2026-10-08 Codex；已實作獨立「空白場景（實驗）」選項，完成編輯器保存／重開驗證；遊戲內啟動驗收與其他 DATA 池的相容性仍待完成。平坦建立流程另在輸入名稱時列出保留的來源內容；讀取失敗的項目明確列為未知。

## TEMP副本的新證據

唯讀分析 `%TEMP%/ArmGameCompare_20261007/ENDL_000`；這是現有副本，數量不代表所有原版版本。`SCRIPT/ak_level.bci` SHA256為 `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27`，PFIL解壓120657 bytes、CODE115244 bytes。

`python tools/bcitool.py syms <TEMP script>` 列出 INIT_UNITSCIV、INIT_UNITSMIL、四族Endlos_*_Siedlung1..5、placesSettle／placesSpawn／placesWaypoint；`calls` 找到以下code-relative位置（加0x24為解壓後offset）：

| 位置 | 原生呼叫 |
| --- | --- |
| 0xa5b0、0xa75c、0x1a52c | s_createUnitAndMems |
| 0xe560 | s_setVillageTemplate |
| 0x17b3c | s_addNPCJob_createUnit |

副本8份聚落SDL解壓後共有613個object區段，其中5個onload=1。這是靜態來源證據，不能直接推算遊戲實際出生數量或斷言所有分支必定執行。

## 真正空白的合格條件

- 清除源圖的可見地景、預放建築／部隊、編輯器事件與自動放置來源；不能只整平材質。
- 對啟動腳本、聚落藍圖、script markers與linked物件建立明確處置契約：保留會自動生成內容的原無盡腳本、或刪除它引用的藍圖，均不能直接宣稱空白且可玩。
- 原生DATA應保留有效格式／空槽，不以刪檔代替初始化。高度、材質、碰撞、輔助層與依賴快取需一致。
- 提供獨立建立選項，告知起始隊伍／開局資料如何產生。用新輸出做儲存與重開驗證；最後實際遊戲開局、單位與尋路正常，才能完成此項。

建立清單並不證明上述啟動契約。AGENTS.md的安裝目錄限制目前仍有效；程式與資料分析只使用TEMP副本，遊戲內測試待已提出的例外授權或使用者手動驗收。

## 獨立空白場景的目前契約

`BlankMapBuilder` 透過 `EndlessMapCloner.ClonePrepared` 在尚未登錄的新 staging 目錄初始化。全部完成才移到正式槽位並寫 manifest；來源圖不寫入。失敗則回滾／移除本次新建目錄，保留既有槽位。

- 全部原生 objects／objdata 槽位改用未啟用槽位範本；位置池除標頭外全清零，含 orphan 與無效位置。SDL 聚落範本保留不動：2026-10-08 遊戲內實測，把 SDL 清成沒有 object 區段會讓遊戲載入時卡死；保留原範本搭配閒置腳本可正常載入（空白平坦場景，起點無物件）。編輯器放置、事件與 DATA 綁定清空。
- anim／gfxtype／action／hirarchy／formatio／lager／biglager／light／particle／explos／hitex／flash 所有記錄與 parallel columns 改成各池的空槽範本；保留版本、容量、必要 sentinel 與 PFIL 標頭。way 路徑與狀態全部清零，保留標頭。共13份 runtime 資料池；版本、長度、欄寬不符或沒有空槽則拒絕建立。空槽狀態依格式讀完整1／2／4 bytes，不以低位元組為零誤判。
- 刪除 staging 的舊 BCI 與原來源 backup，以新的 idle `ak_level` 及相同的 `ak_level.arm_original` 取代。新 CODE 只有 frame、10 tick 等待迴圈與 destructor cleanup，沒有來源初始化、聚落／NPC 生成或其他 native calls。未刪除引擎需要的關卡腳本入口。
- 64×64 單一原生材質；257×257 高度固定為水面高度圖單位加20、vertex RGB 全白、smooth／emboss 全黑；256×256 collision 全黑。水面太高或尺寸不符則拒絕。新 minimap 與地表材質一致，四個高度快取清除。這些檔案在登錄前就已寫好，初次開圖不再執行舊平坦範本的待儲存流程。
- marker 與 manifest 記錄 `StandaloneLevel=true`，複製此圖也繼承。原無盡 AI 修補只掃描一般無盡圖，避免 idle BCI 令探測變 Unknown；原廠槽位誤放標記不會被排除。既有一般自製圖仍納入原修補。
- 保留派系／人口預設、環境及其餘 DATA 池。沒有預設聚落、部隊、可採集資源或敵方 AI；需要在編輯器放置起始物件與勝利事件。engine 的相機及未確認欄位、天氣／FOW／stat 等仍保留來源值，須分析及遊戲內確認。因此 UI 明確標成實驗，**不宣稱已可開局或可玩**。

## 原生格式證據

唯讀 `re_workspace/Against_Rome.exe` SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，`tools/re/formation-probe/probe.py --range` 反組譯以下 loader。下表是檔案 record 與平行欄寬，不能拿 in-memory stride 代替。TEMP ENDL_000 各解壓長度與表列計算完全相符。

| DATA | loader | header bytes | record bytes | 每槽 parallel columns bytes |
| --- | --- | ---: | ---: | --- |
| anim | 0x48c250 | 8 | 21 | 2, 2 |
| gfxtype | 0x48c460 | 8 | 15 | 2 |
| action | 0x48c630 | 12（含動作欄6） | 25 | 無 |
| hirarchy | 0x48cf70 | 12（含連結欄50） | 103 | 2 |
| formatio | 0x48d0f0 | 8 | 15 | 4, 2, 4 |
| lager | 0x48d2d0 | 16（含欄數6、10） | 43 | 2 |
| biglager | 0x48d590 | 12（含容量800） | 1601 | 無 |
| light | 0x48ab70 | 8 | 4（int32 state） | 13×4 |
| particle | 0x48dd90 | 12（含容量64） | 2252（uint16 state） | 4, 4, 4, 4 |
| explos | 0x48e210 | 8 | 46（uint16 state） | 4, 4 |
| hitex | 0x48e4d0 | 8 | 28（uint16 state） | 4, 4, 4 |
| flash | 0x48b910 | 12（含容量16） | 201 | 32 |
| way | 0x48dba0 | 12（含點數256） | 1026（usedPoints u16 + 256對XZ u16） | 4 |

曾以錯誤的 lager 41B record 猜測出1490active；native reader證明43B record＋2B column，修正後3200槽全部inactive。這個失敗假設已淘汰，不以總長匹配作為唯一格式證據。BCI 原 main 的等待位於0x1bec0（frame local→131），結尾0x1bfa8..0x1bfbc為 frame cleanup／return result／130；新 idle 用相同整數等待與cleanup ABI，仍不是 runtime evidence。

## 編輯器驗證及重現

最後 Release solution build 零警告／零錯誤；完整 solution test 啟用真實 TEMP 空白驗收與強制 OpenGL，host685／modules290通過，共975通過、22略過、零失敗。git diff --check 通過。

新增10例合成 raw／PFIL：連結物件、orphan位置、重複初始化、錯誤版本、無空槽（含 light 的 int32 state=256）、寫入後缺腳本失敗的完整回滾；新idle VM例執行100輪，放置／事件僅生成一次，frame stack不累積。另驗證原 P1 修補 apply／save／fresh detect／restore 保留獨立圖 bytes。這些不是遊戲VM實測。

`ARM_BLANK_ACCEPTANCE=1` 的真實TEMP驗收使用一份新副本，先加入來源放置／事件、注入腳本與來源backup、額外BCI及活動特效槽，確認新空白槽全部移除；檢查 native 空槽、全像素灰階與快取失效；真表單保存→新表單重開→加入10人部隊／訊息事件→再重開→移除全部編輯內容，確認最後回復新bootstrap而非原main。新槽準備途中失敗與實際損壞anim版本也拒絕登錄；複製空白圖保留獨立模式與 bootstrap，原 AI 修補數量不增加。完整來源SHA256不變，中英900×600選單及名稱內容清單截圖已目視。最後證據目錄 `TEMP/ArmBlankAcceptance_20261008_06`；`_01..04` 是較早範圍，`_05` 為最後定向55例驗證。

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
$env:ARM_COMPARE_GAME = Join-Path $env:TEMP 'ArmGameCompare_20261007'
$env:ARM_OPENGL_REQUIRED = '1'
$env:ARM_BLANK_ACCEPTANCE = '1'
$env:ARM_BLANK_OUTPUT = Join-Path $env:TEMP ('ArmBlankAcceptance_' + [Guid]::NewGuid().ToString('N'))
Remove-Item Env:ARM_GAME_PATH -ErrorAction SilentlyContinue
dotnet test tests/AgainstRomeModifier.Tests -c Release --no-build --no-restore --filter 'FullyQualifiedName~Blank_|FullyQualifiedName~Idle_level_has_no_source_calls|FullyQualifiedName~MapSelection_form_constructs'
```

未設 opt-in 變數時真實驗收直接返回，不能當成驗收成功。輸入與全新輸出均限TEMP、來源hash與reparse checks沿用示範驗收的隔離保護。
