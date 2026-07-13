# 功能規範：無盡模式玩家陣營改為羅馬人（實驗性功能）

> 狀態：規範定稿，待實作。
> 本文件為完整實作規範，交付對象為未參與前期逆向工程的實作者（AI 或人類）。
> 逆向工程證據見文末附錄；實作時不需要重做任何逆向分析。

---

## 1. 功能定義

| 項目 | 內容 |
|---|---|
| 功能 ID | `RomanEndless` |
| UI 分組 | **實驗性功能**（`pnlExperimentalCard`） |
| UI 名稱（zh-TW） | `無盡模式羅馬陣營 (實驗性)` |
| UI 名稱（en） | `Play as Romans in Endless Mode (Experimental)` |
| 修改目標 | `MAPS/ENDL_000..004/DATA/team.dat`（共 5 個檔案，PFIL 壓縮文字檔） |
| 修改內容 | `[teamdata]` 區段第 0 列（玩家隊伍）第 1 欄 faction token 改為 `ROM` |
| 生效範圍 | 僅無盡模式（ENDL_* 地圖）；戰役、歷史戰役、多人地圖一律不碰 |
| 存檔相容 | 只影響**新開局**；舊存檔內含自己的 team 資料，不受影響也不會壞檔 |

### 1.1 原理（一段話版本）

無盡模式**沒有**玩家陣營選擇 UI，玩家部族由每張地圖的 `team.dat` `[teamdata]`
第 0 列決定（原版：ENDL_000=GER、001=HUN、002=HUN、003=GER、004=KEL）。
遊戲載入時 EXE 把 team 0 的 faction 寫入「當前部族」全域變數，驅動建造選單、
旗幟、技能樹與開局生成。玩家開局開拓隊**不是**預置在地圖裡的，而是執行期依
`cl_scint.ini` 的 per-tribe `FigType` 表動態生成（羅馬齊全），建村起始資源也有
羅馬專屬的 `ResTpVillage=ROM` 行。因此**只改 team.dat 一處**即可得到完整一致的
羅馬開局，不會出現「日耳曼單位 + 羅馬科技」的混搭。

### 1.2 已知設計面限制（要寫進 UI tooltip，不是 bug）

1. 羅馬**沒有祭司**（`cl_scint.ini` `[ObjTypes]` 的 ROM 沒有 `Pri` 行）。
2. 羅馬**沒有榮耀技能樹**（`[TribeData]` `SkillList` 只有 GER/HUN/KEL）——
   榮耀系統是蠻族專屬機制，玩羅馬時技能樹預期為空。
3. 無盡腳本的羅馬建城者（type-4）與羅馬援軍（type-5）是 CPU 隊伍邏輯，
   以 team 編號運作、不看部族，玩家（team 0）變羅馬不影響它們；
   CPU 羅馬隊伍**仍會敵對**羅馬玩家（敵對關係是 per-team 不是 per-tribe）。

---

## 2. 修改規格（資料層）

### 2.1 檔案格式

`team.dat` 為 PFIL@ LZSS 壓縮的純文字 INI。專案已有完整讀寫管線：
`PatchText.Read(byte[])` 解壓成文字行、`PatchText.Write(...)` 回寫並重壓
（會更新 PFIL header 的解壓長度欄位，允許長度改變——現有的人口上限功能
「35→1600」已證明長度變更安全；本功能 `GER→ROM` 等長，更無風險）。

### 2.2 `[teamdata]` 區段格式

每列：`隊伍編號,faction,未知,未知,人口上限,旗幟版本(bver)`，共 8 列（team 0..7）。
原版五張 ENDL 地圖的第 0 列：

```
ENDL_000: 0,GER,35,35,1600,0
ENDL_001: 0,HUN,35,35,1600,0
ENDL_002: 0,HUN,35,35,1600,9
ENDL_003: 0,GER,35,35,1600,1
ENDL_004: 0,KEL,35,35,1600,0
```

### 2.3 修改規則

- **啟用**：找到 `[teamdata]` 區段中第 0 欄為 `0` 的那一列，把第 1 欄改為 `ROM`
  （保留欄位間原有空白格式；實務上原檔無空白，直接替換 token 即可）。
  其餘欄位（含 bver 第 5 欄）**一律不動**——`SYSTEM/banner.ini` 已驗證
  volk03（ROM）的 bver00..09 全部存在，任何 bver 值都合法。
- **停用/還原**：從 `BackupManager` 的原版備份重建（與其他 team.dat 功能相同策略，
  不要嘗試「反向 patch」）。
- **範圍**：只處理 key 符合 `MAPS/ENDL_*/DATA/team.dat` 的備份項目。
  注意現有 `MaxPopulationFeature` 是掃 `MAPS/` 下**所有** team.dat；
  本功能必須加上 `ENDL_` 過濾。
- faction token 對照（EXE 解析器 `0x432620`）：`GER=0, KEL=1, HUN=2, ROM=3`。
  本功能只需寫 token 字串 `ROM`，不需要碰數字 id。

---

## 3. 實作規格（程式層，依專案既有架構）

本專案對「單一檔案、多個功能」的既有模式是：一個 Patcher 收全部選項、
一個 Feature builder 決定範圍、PatchEngine 統一組裝。**本功能必須與
`MaxPopulation`（同樣寫 team.dat）共存於同一次寫入**，不能各寫各的。

### 3.1 `src/Core/Patches/TeamDatPatcher.cs`

1. 擴充選項：
   ```csharp
   public readonly record struct TeamDatOptions(
       bool MaxPopulation,
       int PopulationLimit = TeamDatPatcher.DefaultPopulationLimit,
       bool RomanPlayer = false);   // 新增：把 team 0 faction 改為 ROM
   ```
2. `GetPatchedBytes` 開頭的早退條件改為
   `if (!options.MaxPopulation && !options.RomanPlayer) return original.ToArray();`
3. 在 `[teamdata]` 行迴圈內新增：當 `options.RomanPlayer` 且該列
   `cols[0].Trim() == "0"` 時，`cols[1] = "ROM"`。
4. 新增常數 `public const string RomanFactionToken = "ROM";`
   偵測與 log 引用此常數，勿散落魔術字串（比照 `DefaultPopulationLimit` 慣例）。

### 3.2 `src/Core/Features/Map/MaxPopulationFeature.cs` → 改造為共用 builder

現有 `Build(backup, enabled)` 只吃人口上限。改為（或新增同檔案的多載）：

```csharp
internal static Dictionary<string, byte[]> Build(BackupManager backup, bool maxPopulation, bool romanEndless)
{
    var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
    foreach (var item in backup.BackupFiles.Where(item =>
        item.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) &&
        item.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)))
    {
        bool isEndless = item.Key.StartsWith("MAPS/ENDL_", StringComparison.OrdinalIgnoreCase);
        results[item.Key] = TeamDatPatcher.GetPatchedBytes(item.Value,
            new TeamDatOptions(maxPopulation, RomanPlayer: romanEndless && isEndless));
    }
    return results;
}
```

重點：`RomanPlayer` 只在 ENDL 地圖為 true；HIST/其他地圖只受 MaxPopulation 影響。

### 3.3 `src/Core/Services/PatchEngine.cs`

- 步驟 G（team.dat）改為傳入兩個布林：
  `GetPatchedTeamDatBytes(backupManager, options.MaxPopulation, options.RomanEndless)`。
- log：新增鍵 `SvcLogTeamDatRoman`（見 §3.6），在 romanEndless 為 true 時輸出
  「已將 {n} 張無盡地圖的玩家陣營改為羅馬」。既有
  `SvcLogTeamDatApplied/Restored` 邏輯保持。

### 3.4 `src/Core/Features/PatchProfile.cs` 與 `FeatureRegistry.cs`

- `PatchProfile` 新增：
  `public bool RomanEndless { get => Bool("RomanEndless"); set => Bool("RomanEndless", value); }`
- `FeatureRegistry.All` 新增 `Bool("RomanEndless", FeatureCategory.Stats)`
  （與 MaxPopulation 同類；「實驗性」是 UI 分組概念，Registry 無此分類）。

### 3.5 `src/Core/Features/FeatureDetector.cs` — **必改，有既有偵測衝突**

現況：區段 E 的偵測邏輯是「任何 team.dat 與備份不一致 ⇒ MaxPopulation=true」。
加入本功能後這會誤判（只開 RomanEndless 時 MaxPopulation 也會被判為開啟）。

改為**重合成比對**（byte-exact，重用 patcher，不需解析內容）：

```
對每個 team.dat 備份項目，讀取遊戲目錄現況 bytes；
依序嘗試四種組合 (maxPop, roman) ∈ {(F,F),(T,F),(F,T),(T,T)}：
    candidate = TeamDatPatcher.GetPatchedBytes(backup, new(maxPop, RomanPlayer: roman && isEndless))
    若 current == candidate ⇒ 記錄該檔案的組合，break。
彙總：MaxPopulation = 任一檔案命中 maxPop=T 的組合；
      RomanEndless  = 任一 ENDL 檔案命中 roman=T 的組合。
任何檔案四種組合都不符 ⇒ 維持舊行為（視為已修改，MaxPopulation=true），並 log。
```

注意 (F,F) 組合下 `GetPatchedBytes` 會走早退路徑回傳原 bytes，等同「未修改」判定。

### 3.6 `src/Core/Localization.cs`

新增四鍵（zh-TW 與 en 兩份字典都要）：

| 鍵 | zh-TW | en |
|---|---|---|
| `RomanEndless` | `無盡模式羅馬陣營 (實驗性)` | `Play as Romans in Endless Mode (Experimental)` |
| `RomanEndlessTip` | `將五張無盡地圖的玩家陣營強制改為羅馬人：羅馬開拓隊、羅馬建築與旗幟、羅馬起始資源。注意：羅馬依原版設計沒有祭司與榮耀技能樹；只影響新開局，舊存檔不變。` | `Forces the player faction to Roman on all five endless maps: Roman trek, buildings, banners and starting resources. Note: by original design Romans have no priest and no glory skill tree; affects new games only.` |
| `SvcLogTeamDatRoman` | `已將 {0} 張無盡地圖 (team.dat) 的玩家陣營改為羅馬。` | `Set the player faction to Roman in {0} endless maps (team.dat).` |
| `SvcLogTeamDatRomanRestored` | `已將無盡地圖的玩家陣營還原為原版。` | `Restored the original player faction in endless maps.` |

### 3.7 UI（`src/UI/ModifierForm*.cs`）

比照 `chkRangedAccuracy`（最近一個實驗性開關）的完整接線，共五處：

1. `ModifierForm.cs`：宣告 `chkRomanEndless`（`ModernToggle`），建立於實驗性卡片
   區塊並 `pnlExperimentalCard.Controls.Add(chkRomanEndless)`。
2. `ModifierForm.Patches.cs` `BuildFeatureToggleMap()`：加 `["RomanEndless"] = chkRomanEndless`。
3. `ModifierForm.Layout.cs`：把 `chkRomanEndless` 加進實驗性卡片的排版清單，
   卡片高度若不足需一併調整（`ConfigureSettingsCard(pnlExperimentalCard, ..., 398, ...)`）。
4. `ModifierForm.Localization.cs`：`chkRomanEndless.Text = Loc.Get("RomanEndless");`
   與 `myToolTip.SetToolTip(chkRomanEndless, Loc.Get("RomanEndlessTip"));`
5. `ModifierForm.Presets.cs`：
   - 「一鍵全開」**不得**開啟本功能（實驗性功能既有慣例，見檔內註解）。
   - 「全部關閉/還原」要把 `chkRomanEndless.Checked = false`。

### 3.8 備份

`BackupManager` 既有備份清單已涵蓋 `MAPS/**/team.dat`（人口上限功能依賴同一組
備份，且有 `SvcLogTeamDatHealed` 自動修復機制），**不需要**新增備份項目。

---

## 4. 測試規格（`tests/AgainstRomeModifier.Tests`）

新增 `RomanEndlessPatchTests.cs`（比照 `PatcherRoundTripTests.cs` 用
`SyntheticFixture` 合成 team.dat）：

1. **啟用**：合成含 8 列 `[teamdata]` 的 team.dat（第 0 列 `0,GER,35,35,1600,0`），
   `GetPatchedBytes(original, new(false, RomanPlayer: true))` 後解壓驗證：
   第 0 列變 `0,ROM,35,35,1600,0`，其餘 7 列與所有其他區段
   （`[teamskills_*]`、`[maxteamobjgenerell]`）逐字元不變。
2. **停用等冪**：`new(false, RomanPlayer: false)` 回傳 bytes 與原檔完全相同。
3. **與 MaxPopulation 疊加**：`new(true, RomanPlayer: true)` 同時改 faction 與人口欄。
4. **只改 team 0**：其他列即使 faction 是 GER/HUN/KEL 也不得被改。
5. **偵測回圈**：四種組合各自 `GetPatchedBytes` 後餵給偵測邏輯，
   驗證還原出正確的 (MaxPopulation, RomanEndless) 組合（尤其「只開 Roman」
   不得誤報 MaxPopulation——這是 §3.5 要修的既有缺陷的回歸測試）。
6. **範圍**：builder 層測試——合成一個 `MAPS/HIST_000/DATA/team.dat` 備份項目，
   開啟 RomanEndless 後其輸出必須與原檔相同（未被修改）。
7. **真實檔案 round-trip**（若測試基建可取得 Backup.zip fixture，比照
   `BackupZipGameFixture`）：對五張真實 ENDL team.dat 各驗證恰好一列被改。

---

## 5. 進遊戲驗收清單（實作完成後由使用者手動驗證）

依專案規範，**一律透過修改器套用**，不得手改遊戲目錄。

1. 開啟功能 → 啟動遊戲 → 無盡模式任選一張地圖開新局：
   - [ ] 開局開拓隊是羅馬單位（羅馬首領模型 + 平民 + 馱馬）
   - [ ] 建村後主建築為羅馬 Hauptzelt、建造選單為羅馬建築清單
   - [ ] 小地圖/旗幟顯示羅馬樣式
   - [ ] 起始資源約為 食20/木120/石20/金0/裝備0（`ResTpVillage=ROM`）
   - [ ] 首領無榮耀技能樹或為空（預期行為）、無祭司可用（預期行為）
2. 存檔再讀檔，確認正常。
3. 關閉功能 → 開新局回到該地圖原版部族；讀取步驟 2 的羅馬存檔仍為羅馬（存檔自帶 team 資料）。
4. 開啟「狀態偵測」確認 UI 開關狀態與實際檔案一致（含只開 Roman 不誤報人口上限）。
5. 迴歸：MaxPopulation 單獨開啟仍正常、與本功能同開仍正常。

---

## 附錄 A：逆向工程證據（不需重做，僅供查證）

VA = `Against_Rome.exe` 檔案偏移 + 0x400000（PE 節區已驗證線性對映）。

| 項目 | 位址/位置 | 內容 |
|---|---|---|
| faction token 解析 | EXE `0x432620` | `GER=0, KEL=1, HUN=2, ROM=3`（volk id） |
| `[teamdata]` 回呼 | EXE `0x468fcc` | 第 1 欄 token → team 結構 `0x73ca88 + team*0x84 + 0x00` |
| 無盡啟動 | EXE `0x43b5a0` 一帶 | mscr_endlosspiel → gameMode=5 → `SetPlayerTeam(0)` |
| SetPlayerTeam | EXE `0x44ceb0` | 讀 team faction（`0x46a7f0`）→ 寫 AktVolk 全域 `0x68b688` |
| 無盡對話框 | `dlg_endl` 控制項 | 只有地圖格/載入/簡報/戰霧，無 nation 欄位（玩家不能選陣營） |
| 開拓隊生成 | `SYSTEM/CLAK/cl_scint.ini` `[ObjTypes]` | per-tribe `FigType` 表，ROM 齊全（Anf/Inf/KavInf/PackPf/Sch + 共用 ALL_ZIV*/ALL_PACKPF） |
| 建村起始資源 | `cl_scint.ini` `[TribeData]` `ResTpVillage` | ROM=20,120,20,0,0,0；EXE key 表 `0x61bb40`（ResTpFill=key3、ResTpVillage=key4） |
| 開拓隊非預置證明 | ESAVE_000 vs ENDL_002 物件池 diff | 地圖 3222 個 active 物件全為場景（slot 0..3221 連續、objdata 全預設值）；存檔新增 4667 物件（slot 3222+ 依序分配）= 所有單位/建物皆執行期建立 |
| 旗幟資源 | `SYSTEM/banner.ini` | volk03（ROM）bver00..09 全存在，team.dat 第 5 欄任何值皆合法 |
| MP 大廳 nation | EXE `0x503a90`/`0x503ac2` | nation 0..3 全合法（`err unknown nation` 僅 >3 觸發）→ 人類玩羅馬是引擎支援路徑 |

## 附錄 B：明確不採用的替代方案

1. **EXE patch（`0x44cecf` 的 `call 0x46a7f0` → `mov eax,3`）**：會影響戰役/教學/
   多人所有模式，且只改 AktVolk 不改 team faction，兩者不一致。否決。
2. **改 objects.dat 預置單位**：開拓隊根本不是預置的（附錄 A），無此需求。
3. **改 cl_scint.ini**：會影響全部遊戲模式與 CPU 羅馬隊伍，且 PatchEngine
   現行策略是「cl_scint.ini 一律還原為原版備份」（步驟 D），不可衝突。
