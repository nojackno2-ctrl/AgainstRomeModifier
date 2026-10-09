# 野外動物、巢穴與資源再生：實證與能力邊界

2026-10-08，Codex，`wt/lair`。來源只限使用者指定 TEMP 副本與 repo。
未存取／啟動安裝目錄，未新增任何 BCI opcode 或 native 呼叫。

## 可重現的來源

執行 `python tools/re/probe_wild_lairs.py`。此工具只讀：

- `%TEMP%/ArmGameCompare_20261007/{ENDL_000,ENDL_005,SYSTEM}`。
- `%TEMP%/ArmNativeAssets_20261007/objdef.txt`。
- repo 既有 `tools/bcitool.py` 的 PFIL 解壓與 BCI 解析。

它輸出名稱／索引、SDL 的 team／onload、DATA 啟用物件統計、BCI 呼叫上下文與 SHA256；
不寫入來源，不匯出原始資產。`objdef.txt` 的逗號分隔欄位 52（0-based）是物件名稱。
`cl_scint.ini` 實際是 PFIL；不可當純文字讀。正式模組以 `ScriptObjectAliases` 的
`[ObjDefName]` 解析結果作綁定檢查；`[ObjDefScript]` 只作原生預設腳本證據。

| 來源 | SHA256 |
|---|---|
| `SYSTEM/CLAK/cl_scint.ini` | `a15ad8c484490bdac34d8f257de5169a6fefb9c1a8383f6e8d448439bf457358` |
| `objdef.txt` | `a393fdf49da72ec37332574bfa718596b36f97aa31a72d9c36c9cdd4bb1e84fe` |
| `ENDL_000/SCRIPT/ak_level.bci` | `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27` |
| `ENDL_005/SCRIPT/ak_level.bci` | `faaba1e6653327c04610ac0ab4a728d1faadde4cf750eb9598f33e0504fd769e` |

**來源限制**：ENDL_005 副本已含先前編輯器測試內容，不能把它當未修改的原版。
本次另見新增 30 株 `LanBriLau00_Laubbaum_gross`、3 個 team 0 物件與腳本注入尾部。
ENDL_000 作原始行為比對基線；兩者未做遊戲內測試。

## 真實動物與所有 ALL_* 別名（高信心：檔案內容）

| objdef 索引 | 真實 NameDef | ScriptObjectAlias | ObjDefScript |
|---:|---|---|---|
| 36 | `FigTiePfe00_Pferd` | 無 | 未在此 alias 區段指定 |
| 41 | `FigTiePac00_Packpferd` | `ALL_PACKPF00` | `ak_packpferd` |
| 2022 | `FigTieWol00_Wilder_Wolf` | `ALL_WOL00` | `ak_landtier` |
| 2023 | `FigTieAdl00_Adler` | 無 | 未在此 alias 區段指定 |
| 2024 | `FigTieBae00_Baer` | `ALL_BAE00` | `ak_landtier` |
| 2025 | `FigTieEbe00_Wildschwein` | `ALL_EBE00` | `ak_landtier` |
| 2026 | `FigTieRau00_Raubkatze` | `ALL_RAU00` | `ak_landtier` |
| 2027 | `FigTieMoe00_Moewe` | 無 | 未在此 alias 區段指定 |
| 2028 | `FigTieRab00_Rabe` | 無 | 未在此 alias 區段指定 |
| 2029 | `FigTieSpa00_Spatz` | 無 | 未在此 alias 區段指定 |

另外兩個 ALL_* 是 `ALL_ZIVMAN00=FigZivMan00_Zivilist`、
`ALL_ZIVWEI00=FigZivWei00_Zivilistin`，兩者預設 `ak_zivilist`。
以上是該副本 `[ObjDefName]` 的全部七個 ALL_*。
`FigGerWol00_Wolf`／`FigGerWol01_Wolf` 也存在（245／246），
不能和 `FigTieWol00_Wilder_Wolf` 混同，更不能把 `GER_INF01` 劍士當作狼。
此 TEMP SYSTEM 不含 `CLAK/SCRIPT/ak_landtier.bci`，所以沒有動物 AI 的實作證據。

## 樹、石、田、礦與營地候選

名稱存在不等於可採集。以下是具體名稱／名稱族，不推導資源量、成長率或採掘後再生。
`ResourceRegenerationPlanner.KnownResources` 目前刻意只收 64 個已核對名稱：

- 33 種針葉樹：`LanGerNad00..05_Tanne_gross`、`06..11_Tanne_klein`、
  `12..17_Tanne_gross`、`18..23_Tanne_klein`、`24..32_Tanne_mittel`。
  範例索引 46=`LanGerNad00_Tanne_gross`、80=`LanGerNad01_Tanne_gross`。
- 13 種闊葉樹：`LanBriLau00..06_Laubbaum_gross`、`07..09_Laubbaum_mittel`、
  `10..12_Laubbaum_klein`（717–725，另 1719–1722）。
- 3 個石頭樣本：`LanGerSte00_1Stein`、`LanGerSte01_1Stein`、`LanGerSte02_1Stein`（0–2）。
- 5 種田：`LanItaWei00..04_Weizenfeld`（529–533）。
- 4 個礦場建築：`BauGerMin00_Mine`（43）、`BauKelMin00_Mine`（387）、
  `BauRomMin00_Mine`（715）、`BauHunMin00_Mine`（1010），
  別名依序 `GER_MIN00`／`KEL_MIN00`／`ROM_MIN00`／`HUN_MIN00`。
- 6 個金工建築：`BauGerGol00_Goldschmiede`（234）、`BauKelGol00/01_Goldschmiede`（392/391）、
  `BauRomGol00/01_Goldschmiede`（705/706）、`BauHunGol00_Goldschmiede`（998），
  別名為相應部族 `*_GOL00/01`。這些是 Goldschmiede（金工坊），**不是金礦床**。

更廣的石頭候選含 `LanGerSte00..11_1Stein`、`LanGerSte13..20_Hinkelstein`、
`LanGer2St/3St/4St/6St*`、`LanGerSteMoo*`／`LanGer3StMoo*`／`LanGer4StMoo*`、
`LanItaSte*`、`LanBriSteGro/SteHau/SteKle/SteMau/SteMit*`、`LanBriHin*`。
原始資料亦有拼字例外 `LagGerSteMoo03_1Stein_Moos`、`LanItsSte48_1Stein`；
不能依名稱族憑空製造不存在的 ID。探查工具列出每個確實存在的匹配項及索引。

營地／巢穴名稱掃描（Zelt/Lager/Hoehl/Nest/Camp 等）找到的可靠候選是
`BauRomHau00_Hauptzelt`（368，`ROM_HAU00`）、`BauRomLag00_Lagerzelt`（381，`ROM_LAG00`）、
`BauRomWoh00_Wohnzelt`（243，`ROM_WOH00`），以及各部族 `Bau*Lag*_Lagerhaus`。
這些是聚落建築，不構成中立強盜營或動物巢穴機制的證據。
未找到舊目錄的 `BauGerZelt01`／`BauHunZelt01`／`BauGerBar00`／`BauRomWac00`／
`LanGerStein01`／`LanGerFelsen01`／`LanGerGest01`／`LanKelStein01`。
**未找到只是此資料集的否定證據，不能證明全遊戲或其他版本沒有巢穴。**

## ENDL 如何配置與使用（高信心：靜態格式／統計）

兩個副本各 8 個 SDL、613 個 object 區段，分組相同：

| team | onload | 筆數 | 內容 |
|---:|---:|---:|---|
| 8 | 0 | 528 | 聚落建築／牆／門等藍圖 |
| 8 | 1 | 5 | 4 個草物件、1 個 `FX_KelGol00Rauch` |
| 1 | 0 | 80 | 羅馬第二聚落藍圖 |

SDL 沒有 FigTie 動物、樹林、麥田或動物巢穴。兩個日耳曼、匈奴、凱爾特 SDL 各含一個礦場與金工坊，
均 `team=8,onload=0`。例 `Endlos_Ger_Siedlung1.sdl/object0116`：
`BauGerMin00_Mine`，相對位置 `(-192,0,-512)`；object0031 金工坊相對位置 `(-576,0,-320)`。
世界位置要加 `[settlement] refpos`，不能把 SDL 的 team 8 直接改成事件 team 7。

SDL 為 AI 建村藍圖；`onload=1` 的檔案欄位不保證遊戲開局自動建立。
既有 [map-formats.md](map-formats.md) 的實機反例比 `SdlObjectCatalog` 舊註解更可靠。
ENDL_000 BCI 在 code offset `0x0E560` 推入 `s_setVillageTemplate`（argc=-2），
引用 `Endlos_*_Siedlung*`，配合既有 [endless-mode-ai.md](endless-mode-ai.md) 的建村狀態鏈。
該舊文件的 `0x0E584` 等使用解壓後容器位移；本次工具顯示 code-stream 位移，需加 `0x24`。

預放自然物件在 DATA。讀取 `objects.dat`：16-byte header，14000 槽，79 bytes/record；
active=+0，team u16=+1，type_id u16=+75（對應 objdef 索引）。
ENDL_000 啟用 6618 筆，全部 team 8；33 種 Tanne 實例共 875 株。
ENDL_005 啟用 6651 筆（6648 team 8，3 team 0），相對基線含前述 30 株新闊葉樹。
兩者 DATA 也均沒有 FigTie 動物。資源可採集量與耗盡狀態未由本次統計解碼。

原 ENDL_000 `ak_level.bci` 的分離生成路徑：

- `0x0A414`：`s_createObj`，argc=-7；`0x0A5B0`／`0x0A75C`／`0x1A52C`：`s_createUnitAndMems`，argc=-15。
- `0x1A5D0`：另一個 `s_createObj`。符號／上下文含 `ALL_ZIVMAN00`，而不是四個野獸別名。
- `CIVRECREATE`／`CIVRECREATE_WAIT` 屬無盡模式軍民增援邏輯；不能按字面解讀成樹林或動物再生。
- 沒有四個野獸 alias 的引用，也未建立森林 CA、生長階段、資源耗盡後重生或原生巢穴波次的證據。
  僅看符號缺席不能排除引擎內部動態行為；本次不作此推論。

## 模組改動與安全邊界

`NeutralLairCatalog.Default` 現在只列四種真實陸生野獸，`ScriptAlias` 指向上述 ALL_*。
沒有虛構軍隊代替野獸、巢穴核心、波次、獎勵、難度或 biome。`Unrated` 與 radius=1 僅為
相容模型的編輯器 metadata（radius 不是原生碰撞尺寸）。`FilteredBy` 必須同時核對 alias 與 NameDef。
舊分類／Blueprint API 保留供使用者顯式建立劇本，不宣稱原版存在那些營地。舊危險度係數同樣移除；有波次／守衛的劇本不會套用猜測公式。

`ResourceRegenerationPlanner` 移除猜測的 `LanGerTanne01` 與整套 CA／殘樁腐朽／幼木成長。
`PlanReplacement` 僅記錄顯式要求，不模擬 native resource 量，不自動偵測採伐。
`CreateEditorAddition` 只允許有現成原生範本的中立 tree/stone/field 立即編輯；
同時核對範本 TypeId 與已查證 objdef 索引。這不是運行中再生，需由宿主經 `NatureEditSession` 提交。建築沿用既有配置流程。

`WildLairScenarioEventBinder` 只回傳既有 ScenarioEvent，供既有 ScenarioEventCompiler 編譯：

| 請求 | 狀態／原因 |
|---|---|
| 顯式劇本的 faction infantry 定時生成 | 支援 real alias 的 SpawnUnit，team 0–7，count 1–20，世界座標 0–16383 |
| 存在核心才執行波次／已見過核心死亡後通知 | 沿用 ObjectExists／ObjectDeadOrRemoved；核心仍需有效 ScenarioSpawn／DATA 綁定 |
| 不同首波延遲／週期、最大活躍波次、巡邏／仇恨命令 | 拒絕；現有事件只有單一計時且不能表達此類狀態 |
| 戰利品木材／食物／金／榮譽 | 拒絕；沒有已驗證的 reward action |
| 野獸 timer 或任何 team 8 SpawnUnit | 拒絕；需要 single-object／neutral ownership 路徑，不把動物包成軍隊 |
| 樹／石／田／礦場／金工坊 regeneration timer | 拒絕；目前 ScenarioActionKind 沒有 single-object、耗盡或補充資源動作 |

SpawnUnit 在現有 compiler 固定呼叫 `s_createUnitAndMems`，以 infantry container 加 member alias；
本 adapter 僅開放 NameDef 為 `Fig{Ger,Hun,Kel,Rom}Inf*` 的實際別名，未擴大 native ABI。
現有 LevelScriptInjector 的開局 `ScenarioSpawn.Count=0` 可走 `s_createObj`，
但它不是 ScenarioEvent timer action，故本任務不將它冒充再生支持。
沒有自動外交、team clamping、座標 clamping 或未知別名替換。

波次使用完整 instance GUID，移除／同步不會混淆相同前八碼的不同實例。
先產生並驗證所有新事件與 session 容量才刪除舊事件；失敗維持原 session。
舊 8 字元 LAIR_* 名稱會被 ValidateLairBindings 報告為 legacy，**不自動刪除**，需明確遷移。
`playerTeam` 參數僅保留呼叫相容性，不再推導／改寫外交。

## 驗證與信心

原有 8 個測試中的虛構巢穴／軍隊代替動物／CA 生長／自動外交斷言已替換，理由是它們只證明
猜測模型自洽，未證明原版相容。新增 real alias 的最小 INI fixture（只含名稱映射，無遊戲資產），
驗證目錄、負向能力、完整 GUID、sync 容量／錯誤保留、世界比例及現有 compiler BCI round-trip。
所有 64 個資源 NameDef、10 個資源 alias 另與 TEMP 原資料逐筆交叉比對一致。

- 高信心：上述名稱、別名、索引、檔案欄位、統計、現有 compiler 的操作邊界。
- 未確定：石頭／田的實際可採集性、native 資源量、野獸 AI／中立外交、原生再生、巢穴機制。
- 未在遊戲驗證：本次新增的劇本 timer 與核心條件組合、編輯器立即替換的各個資源型別。
  既有 spawn/message/victory 的驗證來自先前 handoff，本次沒有啟動遊戲。
- 完整 build/test 結果見本分支 AI_HANDOFF.md；此工作樹缺少未追蹤／忽略的原始 EXE／Backup assets，
  依賴這些 fixture 的宿主測試會依既有機制 skip，不能把 skip 當作遊戲驗收。
