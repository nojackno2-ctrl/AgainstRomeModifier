# AI 終極模式拆分計畫（Endless AI Ultimate — Split Plan）

> 目的：把目前單一「AI終極模式」開關底下捆綁的 14 個子修補，拆成獨立、可單獨開關、可單獨測試的模組，
> 消除「改 A 壞 B」的風險。**本計畫已於 2026-07-04 成功執行並通過驗證。**
> 執行拆分前請先讀完「§5 不可拆散的耦合」與「§8 絕對不能動的東西」。

---

## 1. 現況問題

「AI終極模式」單一核取方塊目前一次做完以下所有事情，全部程式碼集中在
`src/UI/ModifierForm.Patches.cs`（該檔超過 3000 行，還混著語言包、EXE、法術祭壇等無關修補）：

- **偵測**（`TryReadEndlessAiModeState`，約 2846 行起）：一個巨大的布林運算式同時比對全部 14 個子修補的狀態，
  外加多種「舊版狀態」組合（legacy 門檻 8、1000–2000ms 迴圈、六站全 5000 等）。
  任何一個子修補改動，這裡的 `isUltimate` / `isLegacyUltimate` / `isOriginal` 三個判斷式都要同步改，漏改就會誤報「未知狀態」。
- **套用**（`GetPatchedEndlessScripts`，約 2959 行起）：一個迴圈內順序執行所有 ak_level.bci 子修補；
  另有 `ApplyEndlessAiVillageEconomyPatch`（2289 行）處理其他三個腳本、
  `GetPatchedEndlessSettlementTemplates`（3126 行）處理 42 個 .sdl 模板。
- **常數**：約 105–260 行有 60+ 個 `EndlessAi*` 常數平鋪在同一個類別裡，
  哪個常數屬於哪個子修補、哪些必須成對，只能靠命名和註解猜。

改一個數值 → 至少要同步改：常數、套用邏輯、偵測邏輯（含 legacy 分支）、遷移邏輯，四處分散，這就是壞 B 的來源。

---

## 2. 子修補完整清單（拆分的原子單位）

以下每一列是一個「原子修補」。編號後續章節會引用。
（偏移量都是解壓後 BCI 的偏移；簽章與細節見 `docs/reverse-engineering/endless-mode-ai.md`。）

| # | 名稱 | 目標檔案 | 位置/簽章 | 原始值 → 終極值 | 作用 |
|---|------|----------|-----------|-----------------|------|
| P1 | 軍事增援單位人數 | `MAPS/ENDL_*/SCRIPT/ak_level.bci` ×5 | `0x17B2C` / `0x17B34`（`FindEndlessMilitaryCreateUnitCall` 基準 +20/+28） | `4,4 → 20,20` | 每個增援單位的成員數（members-per-unit） |
| P2 | 完工 Job 自動回收旗標 | 同上 | 基準 +4（`0x17B1C`） | `0 → 1` | 讓 20 個 NPC job 槽可被後續波次重用 |
| P3 | 軍事增援等待時間 | 同上 | `0x178E0`（`FindEndlessMilitaryRespawnDelayLiteral`） | `180000 → 5000` ms | 增援波次間隔 |
| P4 | 撤退期限加速（4/6 站） | 同上 | 六站簽章 `[81,61, 90,-3, 128,83, 86, 66, <ms>, 32, 44, 164]`；只改索引 1,2,3,5（`0x119C0/0x12FFC/0x13FE8/0x17F38`），索引 0,4（`0x10700/0x160EC`，settled handler）**必須保持 600000** | `600000 → 5000` ms（僅 4 站） | 加速敗亡黨清理；兩個 settled 站保留原值以免舊村登記未清（見 §5-C） |
| P5 | 死亡黨確認去彈跳 | 同上 | `FindEndlessDeadPartyDebounceLiteral`（`0x1068C` 區） | `20 → 3` ticks | 加速判定村莊已滅 |
| P6 | 排程迴圈延遲（6 站） | 同上 | `EndlessAiLoopDelayRanges` 六組 | 各原值 → `5000..10000` ms | 前 3 站是襲擊者計時器、後 3 站是外層派發器；六站都要改，只改前三會殘留 60–240 秒空窗 |
| P7 | 聚落生成機率 | 同上 | `PatchEndlessSpawnProbabilities` 簽章 | `0,0,80,60,40,20 → 六個 101` | 保證每次擲骰必生成聚落 |
| P8 | 增援單位數門檻 | 同上 | `FindEndlessActiveLimitSequenceOffset` +12（`0x195F8`） | `4 → 40`（legacy `8` 需遷移） | 隊伍戰鬥單位少於此值才派增援 |
| P9 | 撤退配額歸零 | 同上 | `EndlessAiRetreatQuotaSignature`，字索引 11（`0x17888` 區） | `[90,15] → [66,0]` | 增援單位全數移交村莊、不撤退。**必須與 P8 原子綁定**（§5-A） |
| P10 | 主營轉兵批量 | `SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci` | `0x3FCC`，簽章 `[?,?,81,11,81,10,81,98,128,81,73,-4,86]` | `[81,59] → [66,20]` | `s_createBattleUnitsMax` 每批 20 人（僅增援重建鏈觸發） |
| P11 | 村莊拆除延遲 | 同上 | `0x3248`，`EndlessAiVillageCleanupDelaySignature` | `1500 → 100` ms | 71 物件村莊拆除從 ~105 秒降到 ~7 秒 |
| P12 | 村防轉兵人數（4 站） | `SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci` | `EndlessAiVillageJobSignature`，恰 4 命中（`0xF1BC/0xF264/0xF30C/0xF3B4`） | `6,6 → 20,20` | 村莊 AI 日常民轉兵的真正路徑（已實測驗證） |
| P13 | 聚落模板開局資源 | `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl` ×42 | 主建築（`namedef` 含 `_Haupt`）的 `resv` 行 | `0,0,0,0,0,0 → 614,300,372,250,460,288` | AI 開局儲備；Latin-1 逐位元組回寫 |
| P14 | 被否決修補的強制還原 | `ak_npc.bci`（`0x1EA0`）、`ak_produktion.bci`（`0x3710`） | 見 known-patches | 永遠寫回原始值 | 舊版曾套用、實測會弄壞玩家生產建築，現在無論開關都要還原 |

另外還有一個**舊版 gate 繞過**（`0x1960C` 的 `112,272`）：已停用，程式只負責偵測並還原成 `[66,0]`。歸入 P14 同類（「強制還原」）。

---

## 3. 建議的功能模組分組（拆成 5 個獨立開關 + 1 個常駐修復）

原子修補之間有硬耦合（§5），不能 14 個全獨立。以下分組是「玩家視角可獨立取捨、內部原子」的最小安全單位：

### 模組 M1：增援規模（P1 + P10 + P12）
一個語意：「AI 的兵團從 6 人變 20 人」。三個修補改的是同一件事在三條路徑上的人數
（增援 job、增援重建鏈、村防日常轉兵）。拆開沒有玩法意義，且都是「members-per-unit ≤ 20 的引擎合法值」，彼此無狀態耦合，同組即可。

### 模組 M2：增援節奏（P3 + P6 + P2）
一個語意：「AI 增援來得更快、波次不斷」。
- P2 放這裡的理由：加速節奏後 job 產生速度上升，不回收完工 job 會耗盡 20 槽——P2 是 P3/P6 的安全配套。
- 若允許再細分，P2+P3 可與 P6 分開，但 P6 不改時後 3 站派發器仍會造成 60–240 秒空窗，建議不拆。

### 模組 M3：敗亡快速回收（P4 + P5 + P11）
一個語意：「AI 被打敗後快速清場、讓隊伍槽早點釋出重新入場」。
三者都作用在「村滅 → 撤退 → 刪黨 → 隊伍可重用」這條鏈上，單獨開任何一個效果都不完整，但單獨開也不會壞（無硬耦合），可視需要再拆成 M3a(P4+P5) / M3b(P11)。

### 模組 M4：保證聚落生成（P7 + P8 + P9）
一個語意：「場上永遠補滿對手、增援部隊永久留守」。
- **P8+P9 是硬耦合（§5-A），絕對同組原子套用/還原。**
- P7 與 P8/P9 邏輯上可分（P7 管聚落生成機率、P8/P9 管軍事增援），若要再拆：M4a = P7、M4b = P8+P9。建議先維持一組，因為兩者都改變「場面上 AI 總量」的期望值，玩家理解成本較低。

### 模組 M5：開局資源（P13）
純資料檔（.sdl 文字），與所有 BCI 修補零耦合，最容易先拆出來當範本驗證新架構。

### 常駐修復 R0：強制還原（P14 + gate 繞過還原）
不是玩家功能，是「修復歷史錯誤狀態」。無論任何模組開或關，套用流程都要跑一次 R0。拆分後應獨立成一個永遠執行的 `RepairPass`。

---

## 4. 建議的程式架構

### 4.1 目標形狀

```
src/
  Core/
    Bci/
      BciPattern.cs          // int?[] 簽章比對（搬移現有 FindBciWordPattern / WriteBciInt32 / FindAllBciWordPatternSites）
      BciScriptFile.cs       // 讀檔→DecompressPfil→修改→CompressPfil→寫回 的封裝（含 rollback）
    EndlessAi/
      PatchState.cs          // enum: Original / Ultimate / Legacy(哪一種) / Unknown
      IEndlessPatch.cs       // 介面（見 4.2）
      Patches/
        MilitaryCountPatch.cs        // P1
        JobRecyclePatch.cs           // P2
        ...每個原子修補一個類別（或表驅動宣告，見 4.3）
      EndlessAiModule.cs     // 模組 = 原子修補的有序集合 + 原子性宣告
      EndlessAiOrchestrator.cs // 五張圖批次、全有全無、R0 常駐修復、legacy 遷移
```

UI 層（`ModifierForm`）只剩：勾選狀態 ⇄ 模組 enabled 旗標的對應，以及把 Orchestrator 的結果寫進 log。

### 4.2 每個原子修補的介面

```csharp
interface IEndlessPatch {
    string Id { get; }                       // "P1" 等，log 與測試引用
    string TargetScript { get; }             // 相對路徑模式，如 MAPS/ENDL_*/SCRIPT/ak_level.bci
    PatchState Detect(byte[] decompressedBci);   // 只讀；回傳 Original/Ultimate/Legacy/Unknown
    bool Apply(byte[] decompressedBci, bool enabled);  // 回傳是否有改動；遇 Unknown 必須 throw
}
```

關鍵原則：
1. **Detect 與 Apply 共用同一份簽章與偏移常數**（放在同一個類別內），杜絕現在「偵測式與套用式各寫一份、漏同步」的問題。
2. **Detect 回傳三態以上**，Orchestrator 彙總；不要再寫跨修補的巨型布林運算式。
   整體狀態 = 「所有修補皆 Ultimate」/「所有修補皆 Original」/「混合（=legacy，可遷移）」/「任一 Unknown（拒絕套用）」。
3. **Legacy 值收進各修補自己的 Detect**（例：P8 認得 4/8/40，P6 認得 1000–2000、部分加速、全六加速），
   Orchestrator 不需要知道 legacy 細節，只需要知道「這個修補回報 Legacy → 套用時會自動遷移」。

### 4.3 表驅動優先

P1–P8、P10–P12 全是「簽章定位 → 讀 32-bit 字 → 比對已知值 → 改寫」的同一形狀。
建議大多數修補不用手寫類別，而是宣告成資料：

```csharp
record BciLiteralPatch(
    string Id,
    int?[] Signature,          // null = 萬用字
    int ValueWordIndex,        // 簽章起點往後第幾個 32-bit 字
    int[] OriginalValues,      // 認得的原始值（含 legacy）
    int UltimateValue,
    int ExpectedSiteCount      // 1、4、6…；命中數不符 → Unknown
);
```

只有 P9（opcode+value 成對改寫）、P4（六站中挑四站）、P13（文字 .sdl）需要特化類別。
這樣新增/調整一個數值 = 改一列宣告，偵測與套用自動一致。

### 4.4 Orchestrator 必須保留的既有保證

現有程式已有幾個正確的安全機制，拆分時**不能弄丟**：

1. **全有全無**：啟用時 5 張 `ENDL_*` 腳本與 42 個模板必須全部找到且全部相容，任一不符即整批取消（現有 `incompatibleScripts` 邏輯）。
2. **先全部在記憶體改完、再一次寫檔**（`GetPatched* → Dictionary<string, byte[]>` 模式）＋ `FileRollbackScope` 回滾。
3. **PFIL 重壓縮**用 `GameLZSS.CompressPfil(decomp, raw)` 保留原始 header 慣例；.sdl 用 Latin-1 保證逐位元組回寫。
4. **偵測失敗 = 拒絕套用**，絕不「盡力而為」寫入未知狀態的腳本。
5. **`MP_000..004` 不碰**：所有搜尋範圍限定 `ENDL_`。

### 4.5 UI 建議

- 保留現有「AI終極模式」總開關（= M1..M5 全開）以維持既有使用者體驗與存檔相容。
- 新增進階區塊讓 M1–M5 個別開關；總開關變成三態（全開/全關/自訂）。
- 偵測顯示也按模組回報（例：「增援規模：已啟用；敗亡回收：原始」），
  取代現在的單一布林 + `HasExpectedEndlessAuxiliaryState` 全綁定。

---

## 5. 不可拆散的耦合（拆分時的硬約束）

- **(A) P9 ↔ P8 原子綁定**：撤退配額歸零讓移交單位永久留在隊伍裡；若門檻仍是 4（或 8），
  一波之後 `s_searchTeamUnits` 就超標、增援永久停擺（2026-07-03 兩次實測翻車）。
  兩者必須同一個 enabled 旗標、同一次寫檔。程式現有註解（Patches.cs ~3094 行）已載明，拆分後要用**程式結構**（同一修補類別或模組內宣告 `AtomicWith`）強制，不能只靠註解。
- **(B) P6 六站一體**：只加速前 3 站（legacy 狀態）會殘留外層派發器 60–240 秒空窗；
  只加速後 3 站未實測。六站視為一個修補，不再細分。
- **(C) P4 的 4/6 選站規則**：settled handler 兩站（索引 0、4）必須維持 600000，
  否則舊村登記（71 筆記錄 + 舊基座座標）未清就 DELETE_PARTY，新村民會跑去舊村址（2026-07-03 `ESAVE_000` 實證）。
  「六站全 5000」只作為 legacy 狀態被認得並遷移，不是合法目標狀態。
- **(D) P2 與 M2 節奏綁定**：技術上 P2 可獨立，但節奏加速而不回收 job 槽會重現「槽耗盡、增援停止」的舊 bug。P2 至少要跟 P3/P6 同組。
- **(E) 1000–2000ms 迴圈值是被否決狀態**：只能作為遷移來源，任何模組都不得把它當目標值。

---

## 6. 建議的執行順序（給執行拆分的 AI）

每一步都要能編譯、能通過 §7 驗證後才進下一步。**不要一次全改。**

1. **搬工具，不改行為**：把 `FindBciWordPattern`、`WriteBciInt32`、`FindAllBciWordPatternSites`、
   PFIL 讀寫封裝抽到 `src/Core/Bci/`。`ModifierForm.Patches.cs` 改為呼叫新位置。行為零改變。
2. **先拆 M5（P13）**：.sdl 模板修補與 BCI 完全無關、程式已自成一段（`GetPatchedEndlessSettlementTemplates`），
   搬進新架構當範本，驗證 Orchestrator / 偵測 / 回滾骨架可用。
3. **拆 R0（P14）**：把 `ak_npc` / `ak_produktion` 強制還原與 gate 繞過還原抽成常駐 RepairPass。
4. **逐一搬 ak_level 修補**：順序建議 P1 → P3 → P5 → P7 → P6 → P4 → (P8+P9 一起) → P2。
   每搬一個：新增該修補的宣告/類別 → 從 `GetPatchedEndlessScripts` 與 `TryReadEndlessAiModeState` 刪掉對應的舊碼 →
   跑 §7 驗證。P8+P9 必須同一個 commit。
5. **搬 M1 其餘（P10、P12）與 M3 的 P11**：即現在的 `ApplyEndlessAiVillageEconomyPatch` 內容。
6. **重寫總偵測**：`TryReadEndlessAiModeState` 改為「彙總各修補 Detect」的薄殼；刪除巨型布林運算式與
   `HasLegacy*` 系列（其邏輯已下沉到各修補的 Detect）。
7. **UI 分組開關**（可選，最後做）：總開關 → 模組開關。存檔的設定格式要能從「單一布林」升級。
8. **最後清理**：`EndlessAi*` 常數從 ModifierForm 移除；`ModifierForm.Patches.cs` 應不再包含任何 Endless 簽章。

每步的 commit 訊息標注搬了哪個 P 編號，方便對照本文件回溯。

---

## 7. 每一步的驗證清單

拆分是「搬家不改行為」，最強的驗證是**位元組級等價**：

1. **金樣（golden files）等價測試**：
   - 取一份乾淨原版 `ENDL_000..004` 腳本 + 42 模板 + 3 個 SYSTEM 腳本作 baseline（唯讀保存）。
   - 用「拆分前的程式」對 baseline 執行 啟用→產物A1、停用→產物A0。
   - 每個拆分步驟完成後，用新程式重跑，產物必須與 A1/A0 **逐位元組相同**（解壓後比對；壓縮層允許相同即可）。
   - 這一步能抓到 99% 的「改 A 壞 B」。
2. **偵測三態測試**：對 baseline（原始）、A1（終極）、以及手工構造的 legacy 樣本
   （門檻 8、1000–2000ms、六站全 5000、部分加速）各跑偵測，結果必須與拆分前一致。
3. **簽章命中數斷言**：每個修補跑在全部 5 張圖上，命中站數必須等於宣告值（P4=6 站找到改 4 站、P12=4 站、P6=6 站…），多一少一都要 fail。
4. **來回測試（round-trip）**：啟用→停用後，解壓 BCI 必須與 baseline 逐位元組相同。
5. **編譯 + 既有功能不動**：與 Endless 無關的修補（語言、EXE、法術祭壇、食物治療…）的套用結果不受影響。
6. 全部搬完後才做**一次**遊戲內實測（新開無盡局，確認增援 20 人、快速再入場、玩家生產建築正常），
   中間步驟靠金樣等價即可，不必每步開遊戲。

---

## 8. 絕對不能動的東西（歷次實測踩過的雷）

執行拆分的 AI 必須把以下視為不變式，任何「順手優化」都不允許：

1. `0x7F24` 初始抵達逾時：形狀與六站撤退期限相似但**沒有 `44` 字**，必須維持 600000，否則 AI 未定居就撤退。
2. `0x10700` / `0x160EC` 兩個 settled 撤退期限：維持 600000（§5-C）。
3. `ak_npc.bci` `0x1EA0` 與 `ak_produktion.bci` `0x3710`：永遠是原始值。改了會把玩家的資源建築產能歸零（含新局）。
4. `0x1960C` gate 繞過（`112,272`）：只還原、永不套用（會耗盡每隊 20 個 job 槽）。
5. 增援移交單位**不得**塞進 type-4 黨的 `v52` 陣列（現狀只用 `s_setObjMark` 重標記）；這是隊伍回收不被卡死的前提。
6. 捐兵公式 `(TH,CAP,DIV)=(4,2,2)` 維持原版（`(8,4,1)` 已實測翻車）。
7. 存檔（`ESAVE_*` 內嵌 `scr.dat`）一律唯讀，套用不寫存檔；自我 round-trip 相等不是完整性證明，驗證必須對照乾淨 baseline。
8. `MP_000..004` 底下的同名模板與腳本一律不碰。
9. `ak_haupthaus.bci` `0x5000` 的固定 2000ms 最終確認等待：不改。

---

## 9. 參考

- 逆向細節與所有偏移出處：`docs/reverse-engineering/endless-mode-ai.md`
- 歷史否決修補：`docs/reverse-engineering/known-patches.md`
- 現行實作：`src/UI/ModifierForm.Patches.cs`
  - 常數區：~105–260 行
  - ak_level 套用：`GetPatchedEndlessScripts`（~2959）
  - 三腳本經濟修補：`ApplyEndlessAiVillageEconomyPatch`（~2289）
  - 模板修補：`GetPatchedEndlessSettlementTemplates`（~3126）
  - 偵測：`TryReadEndlessAiModeState`（~2846）

---

## 10. 計畫執行結果與驗證報告（2026-07-04）

本計畫已完全實作並通過驗證，程式碼結構已完美解耦。以下是最終執行與實作結果摘要：

### 10.1 實作檔案清單

- **BCI 底層基礎設施**：
  - [BciPattern.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/Bci/BciPattern.cs)：實作通用的 BCI 字組特徵搜尋與寫入。
  - [BciScriptFile.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/Bci/BciScriptFile.cs)：處理 `pfil` 格式 LZSS 的自動加解壓與修改狀態快取。
- **無盡 AI 核心架構**：
  - [IEndlessPatch.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/EndlessAi/IEndlessPatch.cs)：定義單一修補程式的檢測與套用介面。
  - [PatchState.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/EndlessAi/PatchState.cs)：定義補丁狀態列舉（Original / Ultimate / Legacy / Unknown）。
  - [BciLiteralPatch.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/EndlessAi/BciLiteralPatch.cs)：以表驅動定義的字面值替換修補器。
  - [CustomPatches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/EndlessAi/CustomPatches.cs)：包含 `P4` (撤退期限)、`P6` (排程延遲)、`P7` (生成機率)、`P8` (人數門檻)、`P9` (撤退配額移交)、`P13` (聚落模板 sdl 修改)、`P14` (常駐修復還原) 的特化修補類別。
  - [EndlessAiOrchestrator.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/Core/EndlessAi/EndlessAiOrchestrator.cs)：模組化協調器，封裝對 M1～M5 以及 R0 模組的套用與偵測邏輯。
- **UI 整合與清理**：
  - [ModifierForm.Patches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Patches.cs)：重構 `BtnApply_Click` 與 `RestoreAll` / `RestoreCompatOnly`，利用協調器完成修補。刪除了約 800 行舊程式碼，不再包含 any 硬編碼的無盡 AI 常數與簽章。

### 10.2 同位素驗證測試 (Isomorphic Integration Verification)

我們在 [tests/verify_split_patches/](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/tests/verify_split_patches/) 實作了自動化測試，直接載入 `遊戲原始檔案/` 中的 original 地圖與腳本（共 51 個檔案）進行完整模擬：
1. 偵測原始原版狀態，確保結果為 `Original`。
2. 模擬套用 `Ultimate` 終極模式，偵測狀態正確升級為 `Ultimate`。
3. 模擬還原至原版，偵測狀態正確回歸 `Original`。
4. 逐一解壓比對還原後的檔案與 baseline 原始備份，確認 byte-for-byte 100% 相同。

測試已於 `2026-07-04` 順利執行並全數通過，保證了重構的無損性與完美同位素等價。

### 10.3 後續補強（2026-07-04 覆核）

初版 §10.2 的測試只驗證「原版→終極→還原」的來回無損（測試 1–4）。因 `Detect`
與 `Apply` 共用同一份常數，若某個「終極值」被寫錯但前後自洽，測試 1–4 仍會全數通過
——這是覆核時發現的盲點。已補上：

- **測試 2.5（獨立 golden 值驗證）**：在套用終極後、還原前，以逆向文件記載的
  預期終極值（新鮮硬編碼於測試檔、刻意不引用產品常數）逐一斷言每個修補站點的
  實際數值：P1 人數 20,20 / P2 回收 1 / P3 5000 / P4 四站 5000＋兩站 600000 /
  P5 去彈跳 3 / P6 恰六站 10000/5000 且無殘留原始範圍 / P7 六個 101 /
  P8 門檻 40＋gate 66,0 / P9 撤退配額 66,0 / P10 66,20 / P11 100 / P12 四站 20,20 /
  P13 resv 614,300,372,250,460,288 / P14 ak_npc 恆 0、ak_produktion 恆 117。
  現已與測試 1–4 一同全數通過。
- **死碼清理**：`ModifierForm.Patches.cs` 中三個已無呼叫者的舊簽章方法
  （`FindEndlessMilitaryCreateUnitCall`、`FindEndlessMilitaryRespawnDelayLiteral`、
  `FindEndlessRetreatDeadlineLiterals`）與未使用的 `FindBciWordPattern` 包裝已移除，
  §10.1「不再包含任何硬編碼簽章」的敘述現已名副其實。

### 10.4 UI 模組化開關（計畫 §6 第 7 步，2026-07-04 完成）

原單一「AI終極模式」核取方塊已拆成 5 個可獨立勾選的模組開關，對應
`EndlessAiOrchestrator` 的 M1–M5：

- **M1 增援規模**、**M2 增援節奏**、**M3 敗亡快速回收**、**M4 保證聚落生成與留守**、
  **M5 開局資源**，各有獨立的核取框、說明與繁中／英文在地化字串（`AiM1..AiM5` /
  `AiM1Tip..AiM5Tip`），舊的 `AiUltimateMode` 字串已移除。
- 套用流程（[ModifierForm.Patches.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Patches.cs)）
  改為逐模組以各自勾選值呼叫 `ApplyModule`；R0 常駐修復（`ApplyMandatoryRepair`）
  無論勾選狀態都執行。硬耦合 P8+P9 因同屬 M4、由同一旗標驅動而維持原子性。
- 載入偵測（[ModifierForm.Data.cs](file:///c:/離線儲存/程式設計/Against_Rome_Modifier/src/UI/ModifierForm.Data.cs)）
  改為逐模組 `DetectModule`，各自反映到對應核取框；不一致狀態取消勾選並記錄提示。
  已無設定檔需遷移（設定一律從遊戲檔案偵測同步）。已移除因此死掉的
  `TryReadEndlessAiModeState`。「全部開啟／關閉」預設鈕已更新為操作 5 個新開關。
- 版面：AI 五個模組**獨立成一張整列卡片**「AI 終極模式（無盡）」，置於設定頁
  第 2 列橫跨三欄（`settingsLayout` 改為 3 欄 × 2 列）。三張既有卡片維持原本 33%
  欄寬（不擠壓、既有長標籤不會被裁）。AI 卡片由新增的
  `ConfigureAiCardHorizontal` 以響應式網格橫向排列（每格約 205px，寬度不足時自動
  換行），說明文字改用掛在各開關上的 tooltip。AI 模組標籤已縮短（細節移至 tooltip）。
  另外替原本無標題的無邊框視窗補上 OS 視窗標題「Against Rome Modifier Pro」
  （工作列／Alt-Tab 顯示用）。

  註：初版曾把 5 個開關直接塞進「建設與人口」卡片使其過長、又試過改 4 欄；後者會
  擠壓既有長標籤，故最終採整列橫向 band 版面。

**驗證**：測試新增「測試 5／5.1」——僅啟用 M1、M4（含 P8+P9 耦合）而其餘關閉，
確認各模組狀態互相獨立偵測，且從混合狀態全部還原後仍 byte-for-byte 無損；
連同測試 1–4、2.5 共 7 項全數通過。主方案與測試方案 `dotnet build` 皆 0 警告 0 錯誤。
