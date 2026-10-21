# Rust 修改器功能與反編譯證據稽核（2026-10-21）

## 結論

父端完成後補充：本分工原先的「未執行」僅描述分工自身。父端已完成當日新反編譯、完整 workspace check/test/build 與零售記憶體往返，並以 Rust 探針實際重現 A2、A3、A7。尤其 A3 要求 2／3／5／10 倍，P20 helper 實際全為 3，安裝版 BCI 來源不變。最終結果及新增 callback 證據見 [本輪索引](feature-audit-2026-10-21.md)，不能只採用下方較早的靜態證據標籤判定整輪涵蓋率。

**現行 Rust registry 與舊矩陣的 ID 集合完全一致，皆為 49 項；功能行為與驗證結論並非全部一致。** 本次以原始碼逐項追查套用、偵測、還原入口，發現原生寬螢幕行為與矩陣相反、舊別名輸入正規化缺口、駐軍倍率未傳入實際補丁且登錄鍵與執行鍵不同，以及多個偵測器不足以證明完整套用。不得把舊矩陣的「verified」直接視為 Rust 實機驗證。

- Registry：`native/against-rome-modifier/src/features.rs:70–365`，49 個不同 ID，31 個 Stats、17 個 Compat、1 個 Language；42 個 Toggle、7 個 Specialized。
- 舊矩陣：`docs/reverse-engineering/feature-verification-matrix.md:33–86`，49 個不同 ID。唯讀抽取 `id: "..."` 與表格首欄後以 `Compare-Object` 比較，無集合差異。
- 明確矛盾：矩陣 `:77` 說啟用寬螢幕會還原六處 EXE；Rust `exe_features.rs:766–794` 啟用時寫入六處原生補丁，並在完整安裝組合中另外調整 dgVoodoo。
- 文件落差：矩陣 `:76` 的追蹤 DLL 是 `version.dll`，現行安裝／偵測使用 `winmm.dll`；`version.dll` 僅是舊受管理檔案的遷移清除目標。
- 反編譯缺口：矩陣所依賴的完整 `ghidra_inventory` 在本機指定路徑不存在；仍有歷史定點 REA JSON。稽核期間另讀到父端產出的當日 REA JSON，以下分開標示來源，不宣稱由本稽核重新反編譯。
- 本次**沒有執行遊戲、apply／restore、Cargo 建置／測試、安裝／移除或 DLL 載入**。遊戲目錄僅讀取 EXE 雜湊與定點位元組。本報告的缺陷為靜態可追查結果，沒有修正產品碼。

## 範圍、版本與方法

已先讀 `AGENTS.md`，分段搜尋 `AI_HANDOFF.md`，檢查分支、工作樹、未暫存／已暫存差異與近期提交。分支為 `主要開發`，稽核起點 HEAD 為 `4ef8282`；起始既存變更為 `AGENTS.md`、`README.md`。後續觀察到父端／其他工作新增 `AI_HANDOFF.md` 變更與 `native/against-rome-modifier/tests/retail_exe_feature_audit.rs`，均未修改、暫存或驗證。未發現根目錄 `CLAUDE.md`、`GEMINI.md` 或 `.agents/rules`。此分工範圍限定只寫本報告；交接與整合驗證由父端負責，分工不操作 Git 或再委派。

採用 `reverse-engineer-anything` 技能的證據分層原則；本次只讀現有檔案，REA 新反編譯由父端另做。既有文件、Java 分析腳本、測試原始碼只能證明「先前宣稱／設計／測試範圍」，不能證明本次反編譯或執行成功。

方法：以 `rg -n` 定位所有 registry ID、`PatchEngine` 組合路徑、各 patcher／detector／restore；讀取相關程式區段；解析本機證據 JSON 的 `records`、`parameters`、`normalized_result`、`subject.digest`；對安裝 EXE 執行唯讀 `Get-FileHash` 與定點位元組讀取。沒有網路查詢、匯入新的反編譯工程或完整輸出／提交遊戲偽碼。

安裝檔：`C:\Program Files (x86)\Against Rome\Against_Rome.exe`，本次實讀 SHA-256：

`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`

此 SHA 與下列本機 REA records 的 subject 一致。這只綁定分析版本，不能證明其他版本、資料檔、DLL 或 BCI 相同。主要來源 SHA-256：

| 原始碼 | 本次讀取雜湊 |
|---|---|
| `native/against-rome-modifier/src/features.rs` | `5daaf5235e9c8f8f82b961aa34eb837c7797d3ce2a0447319af344a11038ceac` |
| `native/against-rome-modifier/src/exe_features.rs` | `b9ac12803fd7f3aab0c33e36f90942bb55c04fd57583e8559e869718b6f5e16b` |
| `native/against-rome-modifier/src/services.rs` | `d434c3d0f742676693ead60478c2f735827c327c0a612303a67993aa759b63a6` |

## 表格讀法與共同路徑

下表縮寫全部指向 `native/against-rome-modifier/src/`：`F=features.rs`、`S=services.rs`、`D=detectors.rs`、`X=exe_features.rs`、`C=cl_script.rs`、`O=objdef.rs`、`R=ress.rs`、`T=team_dat.rs`、`B=bci_features.rs`、`E=endless_ai.rs`、`I=install.rs`、`P=partgeo.rs`、`Q=epara.rs`。例如 `O:142` 即該 crate 的 `src/objdef.rs` 第 142 行。`M:數字` 為舊矩陣的精確列行號；`F` 欄是 registry ID 的精確行號。**矩陣全部行號以本次起始讀取／`git show 4ef8282:docs/reverse-engineering/feature-verification-matrix.md` 為準**：父端稍後在檔頭增加兩行歷史告示，工作樹對應行號已整體加2；本報告不改該檔。欄位編號沿用程式的零起算索引；raw 是 EXE 檔案 offset，VA 是載入位址，BCI offset 是解壓後位置，三者不可混用。

共同還原／組合代碼：

- **RS**：`S:877–920`，Stats 還原資料檔備份；BCI `S:941–964`，EXE `X:916–961`。資料檔關閉功能時也由備份重新組合，並非對目前值反向除倍率。缺備份時分類還原會拒絕或無該項寫入，不能保證整個遊戲目錄還原。
- **RC**：`S:854–875,927–940`，Compat EXE `X:840–911`、BCI 模組 `E:1831–1876`、受管理安裝檔移除。未知 EXE／BCI 狀態有保留不動的路徑；「還原成功回傳」不必然等於位元組全為原版。
- **RI**：完整安裝組合 `S:755–775 → I:552–571`；僅 `S:627` 的 core compose 不包含外部部署。偵測總入口 `D:940–962`，其中 `I:576` 偵測部署。
- **RB**：`S:780–822` 先關閉所有使用者 BCI 模組，執行 R0 修復與舊補丁遷移，再依正規化 profile 套用；`E:1879` 只回傳重壓縮檔案計畫。不能把產生計畫稱為落盤成功。分類還原仍可能執行強制修復，不是任意原始輸入的逐位元組倒轉。

證據標籤：**H**＝舊矩陣／文件／分析腳本所述鏈，完整本機輸出缺失，本次未重新證實；**L**＝實讀的歷史本機 REA record；**N**＝父端另產出的當日 REA record，本稽核只讀；**W**＝本次唯讀讀到安裝 EXE 的局部原始位元組；**外部**＝不適用原遊戲反編譯。各 N/L 細項見後方證據索引。所有列的 runtime 狀態均為**本次未驗證**；「同向」只指寫入目標／常數與矩陣大致同向。

## 全部 49 個現行 ID 對照

| ID（F 行）／舊矩陣 | apply：實際目標與入口 | detect／restore | 反編譯對照、結果與未知 |
|---|---|---|---|
| `FastCiviProduction`（72）M:33 | `S:645–663 → C:267–281`；`cl_script.ini CiviDelay=500` | `D:130–135` 所有找到值在 `(0,500]`；RS | 同向；N `0x540dc0` 的 field 5 clamp 為 500…100000000；完整 key→field 鏈仍 H。偵測允許小於 500，非精確 500 驗證。 |
| `InfiniteMorale`（78）M:34 | `C:298–309`；LostMem/Flee=0、OverPop=99999999、Idle=500 | `D:137–145` 與 `:225`，讀 GER 代表值；RS | H 部落 parser／consumer；四值可追查，但不能從 GER 推定其他 faction 全正確。 |
| `NoRunHpLoss`（84）M:35 | `S:725 → O:142–158`；只改支援列且原 `lpsub` 約為 1.00 的欄 208→0.00 | `D:838–854` 只查羅馬槍盾；RS | 矩陣摘要未寫原值限制。H lpsub consumer，完整輸出缺；單列偵測不能保證所有單位。 |
| `Balance`（90）M:36 | `S:583–608,702–727 → O:127–129,199–409`；TroopConfig 九值基底組合 HP／傷害／VW／AW／視野／reload／range，另影響法術半徑 | `D:563–603`：任一非 leader HP/VW/AW 與原值不同即 true；RS | 欄位層 H＋L 型別初始化＋N objdef serializer；偵測不是 preset 比對，任意外部修改可被判 Balance。整組 preset 效果未知。 |
| `FreeProduction`（96）M:37 | `S:692 → R:59–93`；Bau 欄1–6、Fig13–18、攻城1–6→0，保留 pack horse | `D:389–395` 只看羅馬槍盾13–18；RS | 同向；H `0x46bd00` objres loader。其他列／建築成本和騎乘村民副作用未驗證。 |
| `FreeUpgrade`（102）M:38 | `R:65–67,98–111`；Bau7–12、volkres8/10/12/14、24–263偶數、264–295→0 | `D:397–403` 只查 Ger_Kampf264–295；RS | 同向；H `0x46b200` volkres callback，僅存 Java 選點，不是本次輸出；其餘升級群未知。 |
| `NoSpellCost`（108）M:39 | `R:80–92`；Pri/Dru25–28→0 | `D:405–411` 只查德魯伊／祭司代表列 FigGerPri00；RS | N `0x44a010` 法術按鈕函式存在；H 成本 loader 鏈。不是傷害欄位，其他 faction 成本未驗證。 |
| `HousingCapacity20x`（114）M:40 | `O:82–90`；欄156 `wohnwer` 正原值×20 | `D:509–517,660` 備份相對比較；RS | 同向；N `0x4b0300` 欄位 serializer，H 容量求和 consumer；非 runtime 人口驗收。 |
| `StorageCapacity10x`（120）M:41 | `O:93–105`；Bau 且含 Hau/Lag，欄42×10 | `D:518–521,691`；RS | 同向；H storage consumer，N serializer 只補欄位鏈，不能直接證明儲存玩法。 |
| `HqHp10x`（126）M:42 | `O:107–116`；Bau 且含 Hau，欄19×10 | `D:522–525,721`；RS | 同向；L `0x4a9740/0x4afe10` HP 初始化／型別鏈；N serializer。受名稱集合與原值限制。 |
| `FastBuildUpgradeRepair`（132）M:43 | `O:118–120`；Bau73/74除10，正值至少1 | `D:526–529,751–788`；RS | 建造／升級同向；H repair 與建造時間連動，未取得該完整 consumer 的本次重證，不能宣稱修理必為精確10倍。 |
| `FoodHealing10x`（138）M:44 | `B:15–27,84–149`；12個 BCI 唯一 signature 的 amount word `site+4`，1→10 | `B:50–81,278–280` 全12檔一致；RS／RB 關閉→1 | H VM／s_addLP 鏈；N `0x512aa0` 存在但非全部12檔回溯。未解碼本次安裝 BCI，不宣稱命中。 |
| `MaxPopulation`（144）M:45 | `S:729–744 → T:68–74`；只處理備份列舉且磁碟仍存在的 team.dat，正欄4→1600 | `D:422–469` 候選整檔比較，無匹配反而 true；RS | H team loader，N `0x487f70` 可供續查；「MAPS/**」受備份集合限縮。未知內容被判開啟是缺陷，見 A5。 |
| `CiviProduce20`（150）M:46 | `X:116–125,698–709`；raw `0x4fc00` 區塊 push1→push20（literal在+14） | `X:427–438`、`D:56–60`；RS；`B:155` 清舊ak_npc | N 父端該raw所在函式＋W前8位元組；與矩陣同向。未驗證實際點擊、住房 clamp 及輸出20人。 |
| `UnitRecruit20`（156）M:47 | `X:127–129,712–723`；raw `0x4c7dd`，`29 FD 01 FE → 6A 14 5E 90` | `X:439–450`、`D:61–65`；RS | N 該raw所在函式＋W原4位元組；同向；保留上限的完整控制流程與點擊未測。 |
| `IdleSelect999`（162）M:48 | `X:131–156,726–737`；raw `0x51dc0`／VA `0x451dc0`，含padding共0xB0替換 | `X:451–462`、`D:66–70`；RS | N `0x451dc0` 原40欄陣列、N `0x44d5a0` master cap；W原prologue。同向，但新機器碼跳轉／999全選未執行。 |
| `CorpseRetention`（168）M:49 | `X:158–160,740`；raw `0x110906`／VA `0x510906`，`BB F4 01 00 00 → BB 32 00 00 00` | `X:463–474`、`D:71–75`；RS | L corpse-offset＋N `0x5108f0` 的14000 pool／500 reserve＋W吻合修正後offset；非提高pool上限。效能／長時穩定未知。 |
| `RomanEndless`（174）M:50 | `X:106–114,684–695` 強制faction3；`S:739–740 → T:63–65` ENDL team0→ROM | `D:947–955` 優先EXE，未知才採team；RS | N `0x45bd60` setter＋W原prologue；同向但EXE可遮蔽team不一致，不能證明兩端皆已套用。 |
| `RomanReinforcementGarrison`（180）M:51 | `F:517–519 → E:1516–1519` M6=P8/P9；`E:691–703` 閾值70；`:896–968` quota0、jz+0與release helper | `B:258–263` Ultimate/Legacy→true；RC／RB | 目標同向；H ENDL解壓offset `0x195f8/0x17888` 與state49/50；helper本次只讀Rust，未核對實際5檔VM符號，不承接舊runtime成功。 |
| `VillageGarrisonQuota3x`（186）M:52 | `S:805–807 → E:978–1176` Dorfverteidigung四call→尾端helper；word14位於`helper+0x38` | `B:264–271` 只回填 **VillageGarrisonQuotaMultiplier**；RC／RB | H VM opcode34；registry整數鍵無直連，且orchestrator設定倍率未傳給P20物件，實際組合仍用預設3，見A3；實際BCI命中未知。 |
| `SpellDamage5x`（192）M:53 | `C:224–228` 六個DAMAGE_SPELL_KEYS的Value×5 | `D:154–161` 六鍵相對備份；RS | H `0x41cba0` parser、`:exe-functions.md:300–314` table；N `0x541440` setter可續查，未補齊各傷害consumer。 |
| `SpellHealing10x`（198）M:54 | `C:230–235` KEL Spell1 Value原值×10，原65才是650 | `D:163–165` 相對備份×10；RS | 同向；H 欄5／heal鏈。修改過的備份不保證650。 |
| `SpellResurrection`（204）M:55 | `C:237–242` KEL Spell3 Value/Value2=100 | `D:167–169,346`；RS | 同向；H ak_priester、s_specialEffektCreateUnit。未重解碼該BCI及確認100/100執行語意。 |
| `GeneralSkills`（210）M:56 | `C:248–257` SAbility的Value*×5 | `D:176–182` 原備份ability鍵相對×5；RS | 同向；H shared parser及BCI setter鏈。未對每個技能consumer重證。 |
| `LeaderGlory`（216）M:57 | `O:10–18,69–79` 四leader欄148/149/150/161正值×5 | `D:606–608,890`；RS | 同向；N serializer＋H逐glory consumer。L檔名moves-maxruhm中的maxruhm欄152不是這四欄，不能充作全部glory效果證據。 |
| `RangedRange3x`（222）M:58 | `O:230–242,285–301,414`：rad2×3、Sirad同步、drad×2；`Q:9–32` 移動散布0且初速factor×1.08 | `D:552–560,627` 只看羅馬弓手range；RS含cl_epara | 目標大致同向；矩陣漏列初速×1.08。N `0x4c0e60/0x4e0bd0/0x542730` 為range鏈部分證據；不檢查Q或全單位，見A4。 |
| `UnitMovementSpeed2x`（228）M:59 | `O:230–281` 支援unit的欄4/23/191正值×2 | `D:612–626` 只看羅馬槍盾moves；RS | 同向；L `0x4bb5d0` 群組最慢成員值、N `0x4bb480` 速度選擇；單位範圍、其他兩欄及運行效果未知。 |
| `VillagerMovementSpeed5x`（234）M:60 | `O:131–139` 男女村民與pack horse三速度欄×5 | `D:629–647` 只看男性村民moves；RS | 同向；同movement證據，未檢查女村民／馬。 |
| `SpellEntireMap`（240）M:61 | `O:285–301` priest Sirad=30000 | `D:649–657` 只看KEL priest range約30000；RS | 同向；H priest acquisition／casting gate。AllUnitsEntireMapVision也寫同值，可造成此開關被推定開啟；不是效果半徑證明。 |
| `SpellRange3x`（246）M:62 | `C:180–205` Radius×3，再乘Balance提供的各族基底半徑倍率（`S:638–659`） | `D:171–174,289` 與備份比較；RS | 同向但有倍率疊加；H欄4→效果搜索鏈。多功能組合下「恰為3倍」判斷不足，未做組合測試。 |
| `ProjectileArcHeight`（252）M:63 | `O:123–124,431` emit×2；`S:671–685 → P:6–18,29–99` 七種ysub欄12×2 | `D:530–533,791–835` 只看objdef emit；RS含partgeo | 雙檔目標同向；L particle serializer不等於完整重力積分器，H後半鏈；漏改partgeo仍可被偵測true，見A4。 |
| `AllUnitsEntireMapVision`（258）M:64 | `O:45–50,161–169` 支援單位／男女村民／馬，最後覆寫Sirad欄24=30000 | `D:857–887` 備份列集合皆需命中；RS | 同向；N sight/type鏈部分，H完整acquisition consumer。不是所有未知／新增objdef都覆蓋；長時AI成本未知。 |
| `CustomUnitStats`（264）M:65 | `F:411` 的custom_unit_stats資料，非同名String值；`S:589–608` 必須Balance=true才merge；`O:199–409`寫欄位 | 沒有從objdef重建custom_unit_stats的detector；RS | Specialized輸入／保存契約與布林功能不同；H欄位consumer、L初始化／N serializer僅部分；未有獨立可逆偵測，見A6。 |
| `FocusLoss`（270）M:71 | `X:102–104,619`；raw `0x161a88`／VA `0x561a88` 6-byte store→NOP | `D:35–38`、`X:392`；RC | N父端該raw所在函式＋W `89 15 C4 7D 9E 02`；同向。失焦事件未觸發。 |
| `VillageBuildRange`（276）M:72 | `X:308–341,635–664`；raw hook `0x1364c1`、cave `0x16258f`，bounds30000 | `D:40–45`、`X:528–548`；RC並清舊四點補丁 | N父端hook所在函式、N `0x4c0900`寫objdata bounds＋W原hook/零cave；同向。紅框／實際建造未測。 |
| `NoSpellAltar`（282）M:73 | `X:162–223,666`；12處raw `0x4a0e3…0x4a414` 比較門檻→0 | `X:400–418` 所有sites；`D:48–51`；RC | N `0x44a010` 與H三族分支同向；未逐點重新反組譯全部12條。 |
| `GameSpeed`（288）M:74 | `X:358–389,822–830`；raw `0x204214/0x20424c` doubles基底1e9/1e6×整數倍率 | `D:53–54`；RC=1；未知cur=0時apply靜默略過 | L兩constant原bytes、N QPC初始化／TGT path＋W doubles吻合；同向但沒有上限且未知不報錯，見A7。 |
| `DgVoodoo`（294）M:75 | RI `I:371–449` 部署DLL/設定與管理marker | `I:349–356` 只查marker及D3D8/DDraw存在；RC | 外部；同向。偵測不重算DLL hash；存在不等於載入成功／GPU相容。 |
| `ArgmTrace`（300）M:76 | RI `I:462–528` 部署 **winmm.dll**＋argm_trace.ini；舊version.dll按hash遷移清除 | `I:454–460`；RC按hash保護移除 | 與矩陣DLL名稱矛盾；外部部署不證明hook已安裝。INI timestamp僅接受known `0x404d1710`，否則0，不能宣稱自動支援任意build。 |
| `NativeWidescreen1920x1080`（306）M:77 | `X:766–794` **啟用六處原生EXE補丁**；RI另外`I:310,560–565`更新dgVoodoo | `D:77–86` 原生三種狀態均true；`I:582–584`置中亦true；RC | **與矩陣相反**。raw `0x246b3/0x249aa/0x1dcc76/0x24bd0/0x24b80/0x41e25`；W局部原始碼吻合，見A1。雙來源偵測無法分辨原生／置中。 |
| `CameraZoomOut1`（312）M:78 | `X:284–306,796–819`；raw `0x81388/0x8d880/0x14c1fa → cave0x1625c0`，helper最小1.0 | `X:506–527`、`D:88–95`；RC | N `0x498a30` setter／W三原call；同向。實際縮放、讀檔／任務script入口未測。 |
| `EndlessAi.M1`（318）M:79 | `E:1499` P1/P10/P12；P1 count words索引5/7的4→20，Dorf conversion亦調整 | `B:256–275 → E:1778`；RC／RB | H BCI raw-decompressed `0x17b2c/0x17b34`、Dorf `0x3fcc`；signature實作可查，實際五圖未解碼重驗。 |
| `EndlessAi.Core`（324）M:80 | `E:1532–1543` P3/P6/P2/P4/P5/P11/P7/P17/P18/P19，另RB強制R0 | `B:258–274` Legacy也選中；RC／RB | H週期、退場、settle-place鏈；H `0x178e0/0x119c0/0x1068c/0xaf0/0x9a10`等。不是單點patch，Legacy不代表每個subpatch成功。 |
| `EndlessAi.M2`（330）M:81 | 預期Core別名，`F:466–535`；非user_modules獨立apply | `B:256–275`不直接填此ID；RC | **僅legacy map M2可導向Core**；直接`set_bool("EndlessAi.M2",true)`被正規化覆為false，見A2。 |
| `EndlessAi.M3`（336）M:82 | 同M2，legacy map M3→Core | 同M2；RC | 同A2；無獨立byte target，不能按49項算49種補丁。 |
| `EndlessAi.M4`（342）M:83 | 同M2，legacy map M4→Core | 同M2；RC | 同A2；來源H Core鏈，不另重複計算證據。 |
| `EndlessAi.M5`（348）M:84 | `E:1515 → CustomPatch(13)`，`:705–724` SDL resv=`614,300,372,250,460,288` | `E:625` SDL原/目標比較；`B:273`；RC／RB | 同向；H settlement模板欄位；pattern `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`，未知值拒絕。未確認現行地圖全命中。 |
| `EndlessAi.M6`（354）M:85 | `F:517–519` 與RomanReinforcementGarrison OR並鏡射；`E:1516` | `B:261–263`兩鍵皆填；RC／RB | 別名成立，與M2/M3/M4直接值缺口不同；H P8/P9來源，不是另一獨立功能。 |
| `ToEnglish`（360）M:86 | RI `I:197–281` 語言overlay與manifest備份計畫 | `I:177–195,576–579`；`S:974–978`還原 | 外部；同向。有manifest／marker的檔案契約不證明全部字型、編碼、遊戲內文字正常。 |

## 缺陷與矛盾明細

### A1：高優先，寬螢幕恢復了舊矩陣明示已被實機否決的原生實驗

矩陣 `:16–19,77` 明確把原生修改列為 runtime-rejected，現行安全展示路徑應「還原EXE＋dgVoodoo置中」。`X:774–783` 卻在開啟時寫六個 `WS_*_PATCHED`；只有關閉才寫original（`:784–793`）。`S:633–636` 真正使用該函式生成EXE計畫，完整compose再接安裝計畫（`S:761–764`），因此不是孤立未使用函式。靜態可確定組合計畫同時包含原生patch與wrapper設定。

六點常數位於 `X:245–281`：identify、create、文字、force、actmode、ingame menu。`D:77–86` 會把原生Legacy／Patched判開；`I:582–584`又可因wrapper置中判開，單一旗標無法證明採用哪條路徑。現有測試 `tests/patch_engine_install_compose.rs:251–312` 有wrapper組合檢查，不足以推翻這條直接原始碼矛盾；本次未執行測試。應由父端決定預期行為並補「開啟後EXE六點為original」的驗證，本報告未修正。

### A2：中優先，M2／M3／M4 直接 ID 輸入被丟失，回讀亦未提供鏡射

`F:499–515` 的 `legacy_core` 僅從 `endless_ai_modules` map蒐集；`:520` 只把 `get_bool("EndlessAi.Core") | legacy_core` 作為Core，沒有讀同名直接M2/M3/M4值；`:532–534` 將三個直接值覆蓋成Core。因此空profile只設 `EndlessAi.M2=true` 再normalize，結果Core=false且M2=false；M3/M4同理。這是對函式逐步求值，**沒有執行程式**。

CLI `cli.rs:218–229` 允許 `Features` 物件載入該直接鍵，`:287` 呼叫normalize，因此 `{"Features":{"EndlessAi.M2":true}}` 具有同一靜態遺失路徑；反之 `{"EndlessAiModules":{"M2":true}}` 走 `cli.rs:240–249` 會保留legacy意圖。矩陣 `:81–83` 未限制別名只接受legacy map，不能籠統判全部別名一致。偵測 `B:256` 僅遍歷 `E:1545–1551` 的五個user_modules，Core回填後不再normalize（`D:958–962`），M2/M3/M4回讀仍為預設false。

### A3：高優先，駐軍倍率未傳入P20物件，另有registry鍵不一致

`F:186–189` 將 `VillageGarrisonQuota3x` 宣告為Int(1)，但執行只讀未登錄的 `VillageGarrisonQuotaMultiplier`（`F:476–477`、`S:805–806`），偵測只寫該後者（`B:264–271`）。直接對registry ID設Int(5)沒有消費者；讀registry ID也不會回傳剛偵測的實際5倍。

CLI有特例：`cli.rs:253–269` 讀獨立Multiplier整數，舊布林 `VillageGarrisonQuota3x=true` 轉成3；GUI `apps/against-rome-editor/src/modifier_ui.rs:58` 同樣採用Multiplier鍵。這些橋接能表達啟用意圖，不能宣稱generic registry Int契約可用。

**更直接的功能缺陷：設定2／5／10不會傳入實際P20 patch。** `E:1524` 建立 `P20VillageGarrisonQuotaPatch::default()`，其內部multiplier固定初值3（`:982–985`）；`E:1571–1574` setter只更新orchestrator的另一個 `village_quota_multiplier` 欄位。逐一搜尋該欄位，除宣告、預設值、setter外沒有讀取。`E:1606–1608` 直接將module交給apply，沒有複製倍率；實際helper寫入使用的是P20物件的 `self.multiplier`（`:1144,1147`）。因此 `S:805–810` 雖先設orchestrator為5，接著啟用P20時仍以3判斷／寫入；已有5倍helper也可能被當Legacy後改回3。此為完整靜態資料流發現，未執行BCI修改。舊矩陣所稱可切換x2/x3/x5/x10，與目前此組合入口不一致；單獨建立自訂P20物件可支援倍率，不能證明產品入口有傳入。

### A4：中優先，多目標功能的 detector 不是完整驗證器

- 彈道：`D:791–835` 只比較objdef的emit，完全不讀partgeo；`partgeo.dau`仍原版或損毀時，emit×2仍足以true。實際apply需雙檔（`S:671–685`、`O:123–124`）。
- 射程：`D:554–560` 只看一個羅馬弓手range，沒有檢查所有rad2／Sirad／drad、`cl_epara.ini`的移動散布與初速×1.08。矩陣 `:58` 本身也漏列初速因子。
- 速度：`D:612–647` 只看羅馬槍盾／男性村民的moves，不驗證movsf／bmovs與其他列。成本三開關也各取一個代表列（`D:389–411`）。
- Balance：`D:593–599` 將任一非leader HP/VW/AW差異當作Balance，不比較平衡preset全部欄位。其他工具或Custom值也能命中。
- RomanEndless：`D:949–955` 只要EXE可辨識就優先其狀態，即使team與其不一致。

以上是實作可確認的涵蓋限制，不是本次觀察到安裝資料已損毀。稽核應以獨立逐檔逐欄驗證補足，不能拿單一detected=true證明整項正確。

### A5：中優先，MaxPopulation 將不認識的 team.dat 狀態標為開啟

`D:451–469` 以原備份生成四種pop/roman候選，只有整檔bytes相等才回傳候選。`D:443–445` 卻在四種都不匹配時直接 `max_population=true`。因此只改旗幟／其他合法欄位、或無關格式差異，也可能回報MaxPopulation，而非Unknown。又以跨圖OR（`:441`）彙總，不能推論每圖每隊欄4皆1600。apply範圍僅備份中的檔案，與矩陣「MAPS/**」字面全覆蓋不同。

### A6：中優先，CustomUnitStats 沒有獨立 detect，且有 Balance 相依

`F:264–267` 是String型Specialized登錄值；真正資料在 `F:411` 的map，`cli.rs:279–284` 另行反序列化。`S:594–598` 在Balance=false時直接回原值，custom map不會套用；GUI `modifier_ui.rs:1735` 有提示此相依，矩陣 `:65` 沒有寫出。整個 `D:940–962` 及其子偵測器沒有重建該map，也沒有設定同名String值；GUI `modifier_ui.rs:943–945` 可能保留記憶體舊map，不能視為從磁碟偵測成功。

分類還原可回復objdef備份，但不代表能辨識／重建任意自訂設定。故舊矩陣 `:9–11`「每項都有對應detect/restore」對此項需要限定：有資料檔還原，無獨立磁碟反推設定能力。

### A7：較低優先，GameSpeed 未知時可靜默略過，輸入與辨識界線須另驗

`X:367–389` 要求兩處double比例皆為同一正整數，否則0；`X:824–830` 只有cur>0才寫入，不像其他requested unknown EXE特徵使用 `require_known` 拒絕。請求2倍而cur=0時可回傳未修改bytes的Ok。關閉／Compat還原同樣只處理已知倍率（`X:879–884`）。這保留未知內容但缺少「請求未達成」訊號；不是全功能執行成功的證據。輸入只以max(1)下限處理，沒有上限；本次未測極大倍率、時鐘溢位或遊戲可玩性。

### A8：文件落差，ArgmTrace 部署身分與驗證範圍已改變

`I:454–460,484,520–527` 明確使用winmm.dll，`I:476–482` 是按marker/hash清舊version.dll。矩陣 `:76` 應被標為舊部署說明；不能依它檢查目前檔名。`I:509–517` 未匹配known timestamp時產生0，且保留使用者改過的ini；「自動鎖定build」須按實際ini判斷。`is_installed`只查marker名稱及檔案存在，不驗證PE架構／exports／hook簽章／執行中的載入；原遊戲EXE不被修改不代表部署後不會在程序內inline hook。

## 本機反編譯與位元組證據索引

### 缺失與歷史文件界線

`docs/reverse-engineering/exe-functions.md:13–14` 引用的：

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

本次指定路徑不存在，且以 `rg --files --hidden re_workspace` 限定上述檔名、排除target/Rust工具鏈後未找到替代位置；未搜尋其他磁碟。沒有完整輸出或提交decompiled_functions.c。矩陣 `:94–103` 所述2026-07-17 headless成功是歷史記錄，本次未重跑。`tools/re/GhidraScriptIniAnalysis.java`、`GhidraRessAnalysis.java:150–163`、`GhidraUnitRangeSpeedAnalysis.java:79–98` 仍在，但腳本中的選點本身不等於已取得反編譯結果。尤其tribe／ress loader、各法術consumer、BCI VM乘法／release符號、修理連動等完整證據鏈仍有缺口。

### L：實際讀取的歷史本機資料

這些JSON多為單行；可引用位置均為該檔`:1`，再以record evidence_id定位。未將整份偽碼抄入報告。

| 本機檔案（均在re_workspace） | record／地址 | 本次可支持與不能支持的內容 |
|---|---|---|
| `corpse-offset-20261009.json:1` | opcode raw `0x110906`、VA `0x510906`、bytes `bb f4 01 00 00`；舊configured `0x110907` 的window為 `f4 01 00 00 e8` | 支持off-by-one修正；不證明屍體保留效能。 |
| `clock-20261009-evidence-final.json:1` | `ev_8d56f89f48f317cb134effbc80fa7ac0e290d30f77b260ff38908e7765bd35b3` VA`0x604214` bytes`0000000065cdcd41`；`ev_947efcad56784f597037814cf5a20144bda447bde021e39ff7f0203d2e293dbf` VA`0x60424c` bytes`0000000080842e41` | 綁定1e9／1e6常數；`ev_94b9696d5e93f1e7e93b2eacdaf0087aca6c4c11b8feae2d1ad3d1f295764cff` 的`0x55e530`顯示timeGetTime差值乘第二常數；非倍率執行測試。 |
| `objdef-tokenizer-20261009-evidence.json:1` | `ev_1be74682a49638c4119752d7809db59f19526d8fb1c0a3599a9dc9ce3e3037a7` `0x42a230`；`ev_80e4574b4b6f1899809c6f6a38f0ef9ce2a5f9bb6f6a5bd7076249b6671d91ea` `0x567d50` | loader／分隔掃描基礎；不能單憑tokenizer證實每個欄位consumer。 |
| `object-moves-maxruhm-20261009-evidence.json:1` | `ev_2e89501dfa6e21f16031bb9203f9432d2602df193381e73bde43f464042b95bf` `0x4bb5d0` | 群組掃描`objdata` stride0x90、moves位址`0xf5ffa8`，採成員最小值再乘global；未證明每個移動入口。 |
| `object-vitals-init-20261009-evidence.json:1` | `ev_f183b7c6d1a48960c2442467041aec4e1dfd16b7761307d7741dfbd68422d785` `0x4a9740`；`ev_8cefa10a066f6514d3576a1f87a10913f507113d65124aba833edb7f62b25e17` `0x4afe10` | 型別／物件初始化部分鏈；不證明所有平衡、glory或玩法結果。 |
| `particle-geometry-object-20261013-evidence.json:1` | `ev_dc0a390c3185fc3d30a102ffeb8ed576a2e457f3174f77d4a74a642f72e1e4d0` `0x4b04c0` | objdef欄名序列包括moves/movsf/wohnwer/bmovs，屬serializer／layout；不足以單獨證明emit＋ysub完整積分。 |

### N：父端另做的2026-10-21 REA結果，本稽核僅讀

稽核進行中可見 `re_workspace/feature-audit-20261021-rea-evidence.json`（讀取時395079 bytes），records的subject SHA與本次安裝EXE一致。它是父端新產物，並非本稽核呼叫REA所得；以下只採實際讀到的定點內容。該檔與任何新測試皆未由本稽核寫入／提交。單行JSON位置同為`:1`，以下ID可精確定位。

| 函式／位置 | evidence_id | 已讀觀察與限制 |
|---|---|---|
| `0x5108f0` | `ev_034d8e7a2e7895acae769fd169a3fe3633212bdff125f8d41e6ea256b50fc461` | 有14000 pool與500 reserve運算，支持CorpseRetention常數機制。 |
| `0x45bd60` | `ev_3cdd6f3b3a81f805f61c325d613e1a79744d114cf5d640464d579c8bd1340461` | 將參數交給active faction並寫`DAT_00737478`、team setter；未測強制ROM後開局。 |
| `0x451dc0`／`0x44d5a0` | `ev_ea16d3efba3912cbf2aac528235ffac4252cf3889cd67be6192b6b5a9eb01a17`／`ev_75c97b8899bf4a507fafb3c371dda7d8d31beb71cc45106316d8cb5f3205c21a` | 原版40欄stack buffer與selection adder；patched新程式碼未執行。 |
| `0x498a30` | `ev_84a39fcb4dc98417e7de4ee7f19632b0d7c2a3e1954ce16a3ea64cf191f74132` | 原生zoom setter／clamp，不能代替三入口新cave呼叫測試。 |
| `0x4c0900` | `ev_e7364ed3de2c82aa02cfc782e62d759586c9c9483e1ce5324493221c04c99b69` | 寫`0xf5fff8/0xf5fffa`的每物件bounds；不能單獨證實新hook及紅框。 |
| `0x540dc0` | `ev_1e479c1d41596ceba39a35b9cc4aff70eb5c59f651817e54c1caff3dbc5fe938` | field5 clamp最低500，尚需cl_script key→欄位完整綁定。 |
| `0x4b0300` | `ev_5c9943184de853bef9b73b4d86509edadc614d6cdef67e2ea80459ce9a3f211a` | objdef serializer，可支持欄位定位；不把serializer等同每個consumer。 |
| `0x4bb480` | `ev_148d658e10ed0a6e0bf158fe87c6f0d2e3f08ac62148fdcba77bcbbf8fa30688` | 原生movement選擇邏輯，僅部分速度證據。 |
| `0x4c0e60`／`0x4e0bd0`／`0x542730` | `ev_6ac3f5a942234ee1890ee06cd9b8c40d685e3e3f19c22a6db8b2f54e1945c972`／`ev_d5a8ed5cbaa91a66a48ece2eb98fe4ec233351fe090db51938d72357f60d5cae`／`ev_75f33ba80c87d96656b73fde5a65cd999c49a558705da472ab6cdd040710a621` | range getter／距離／script bridge；不含對全部patched資料的實際命中。 |
| `0x55df60`請求（回傳函式標頭`FUN_0055df20`）／`0x55e550` | `ev_3d58a8b32953b80f873484ca9c948ddc3e6f981d633a1083b6245cceda93d2a6`／`ev_6f72ff77fd89cf3e3e397bd554e843d6171eec871e1784b300424dbae1a213f0` | QPC frequency初始化使用`0x604214`；TGT乘`0x60424c`。請求位址與containing function標頭不同須保留，不能另算成兩個函式。 |
| raw `0x4fc00`／`0x4c7dd`／`0x161a88`／`0x1364c1` | `ev_8712b4d14e21494304ca8082be0cc4f6280ed5ea182ce8b0d3a0b67e0674b30b`／`ev_c1e41e3ec1ffd3a73d9ca46d6d5012cac4752d0100ccf4fd2cde6eb96a8d92e4`／`ev_b65e1c9f9d08f71a8a65603a69e0d4042fae4863788b57fb7e4c7a61ec278d17`／`ev_c9a386e3fb0cb81bc315b20df641a83d3a481056f5b8c962f33ba5cdd563bd8c` | 父端以file-offset選取所在函式，僅原版靜態輸出；不是補丁後runtime。 |

另可見N `0x44a010`（`ev_d3fab007d32daf4a111e7b68984fbc7da0dd020d136ebf71343950c0360a431c`）、`0x487f70`、`0x541440`、`0x512aa0`等records。只確認其存在／有限片段，未完整重建所有成本、team、spell、BCI符號鏈，故表格保留H與未知，不將「有某函式」升級成全部功能已驗證。

### W：本次安裝 EXE 定點唯讀觀察

| raw offset | 本次原始bytes／數值 | 支持範圍 |
|---|---|---|
| `0x110906` | `BB F4 01 00 00` | corpse正確opcode window，沒有寫入50。 |
| `0x161a88` | `89 15 C4 7D 9E 02` | focus原始6bytes。 |
| `0x4c7dd` | `29 FD 01 FE` | recruit原始4bytes。 |
| `0x4fc00`／`0x51dc0`／`0x5bd60` | 前8bytes分別`83 7C 24 18 01 0F 95 C0`／`53 56 57 81 EC 40 01 00`／`53 8B 5C 24 08 53 E8 25` | 僅區塊前綴，不是全部32／176／21bytes簽章檢查。 |
| `0x1364c1`／`0x16258f` | `85 F6 7C A6 85 FF 7C A2`／前8bytes為0 | hook與cave前綴；非完整cave長度證明。 |
| `0x246b3`／`0x249aa` | `81 3E 40 06 00 00 75 12`／`6A 50 6A 20 68 B0 04 00` | 原寬螢幕identify/create前綴；未驗證全部六段。 |
| `0x24bd0`／`0x24b80`／`0x41e25` | `8B 15 B0 80 65 00 52`／`A1 B0 80 65 00 C3`／`83 F8 21 74 D4` | 原force/getter/menu window。文字點未另讀。 |
| `0x81388`／`0x8d880`／`0x14c1fa` | 原CALL分別`E8 A3 76 01 00`／`E8 AB B1 00 00`／`E8 31 C8 F4 FF` | 三zoom入口原位元組。 |
| `0x204214`／`0x20424c` | little-endian f64為1000000000／1000000 | 原時鐘1倍常數；非調速實驗。 |

## 驗證涵蓋與下一步

本次完成：49/49 ID集合、49/49程式路徑盤點、歷史矩陣逐列對照、既有本機證據身分核對、安裝EXE有限定點讀取。沒有任何一項被宣告本次runtime成功；沒有以歷史交接的測試總數代替本次驗證。未執行`cargo check/test/build`，以遵守僅能寫本報告的限制，避免寫target／cache；也未執行會建立log或bak的modifier CLI。

現有 `tests/feature_registry_parity.rs:5–13,17–41` 已改用frozen C# JSON，只保證ID集合／不重複；不是反編譯測試。`tests/category_restore_parity.rs:223,299,366`、`tests/patch_engine_install_compose.rs:251,314`、`tests/services_install_detectors.rs:171,203,230` 等涵蓋synthetic資料與檔案契約，這次僅讀原始碼；不引用為本次通過。父端新 `retail_exe_feature_audit.rs` 在本次盤點期間出現，其執行結果由父端另行報告。結尾工作樹另可見父端新增驗證索引、editor/save稽核報告與 `tools/re/GhidraFeatureVerification.java`，本稽核未修改這些檔案。

建議父端後續順序（均未由本稽核執行）：先決定A1的正確產品路徑，再以不落盤的記憶體plan驗證原生六點應為original或patched；補M2/M3/M4 direct ID與legacy map各種正規化／回讀驗證；統一P20 registry與Multiplier契約；對MaxPopulation未知及各多檔partial state建立獨立判準；以REA定點補齊tribe／ress parser、BCI VM符號與完整consumer鏈。最後完成遊戲內驗收，才能更新各功能的 runtime 結論。
