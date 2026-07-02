# Against Rome Modifier 技術文件

> 更新日期：2026-07-02
>
> 編碼：UTF-8
> 對象：開發者、逆向工程研究者與 AI 維護代理

本文件描述目前程式與補丁契約。完整的修改時間線、失敗案例、代理操作規範與除錯 playbook，請先讀 [`docs/AI_AGENT_HANDOFF.md`](docs/AI_AGENT_HANDOFF.md)。機器可讀的欄位與 offset 位於 `data/game_schema.json`；精確 EXE／BCI bytes 位於 `docs/reverse-engineering/known-patches.md`。

## 1. 證據與修改原則

- **穩定**：程式路徑與 runtime 行為都已核對。
- **靜態驗證**：bytes、格式、反編譯或 round-trip 已核對，但仍缺足夠遊戲測試。
- **候選**：只能研究，不應由修改器自動寫入。
- **已否決**：runtime 已證明錯誤或造成回歸；保留紀錄以防重新加入。
- Ghidra 的 `FUN_*` 只是導航點，必須有 call path、資料流、字串註冊或實機證據，才能命名其意義。
- 每個寫入功能必須能偵測原版／目前版／舊版／未知狀態，且有明確回復路徑。

## 2. 技術架構

| 檔案 | 責任 |
|---|---|
| `Program.cs` | WinForms 入口、DPI、UAC |
| `ModifierForm.cs` | 手工 UI、事件 wiring、文件頁 |
| `ModifierForm.Data.cs` | 備份載入、目前資料讀取、狀態偵測、顯示 |
| `ModifierForm.Patches.cs` | 套用、回復、rollback、遊戲檔補丁 |
| `ModifierForm.Presets.cs` | `.arpreset` 匯入／匯出 |
| `ModifierForm.SaveManager.cs` | 存檔瀏覽、備份、回復、刪除 |
| `ModifierForm.DgVoodoo.cs` | dgVoodoo2 受管安裝／移除 |
| `TroopConfig.cs` | 欄位 enum、單位 metadata、平衡規則 |
| `TroopPresetForm.cs` | 9 欄單位 preset 編輯 |
| `GameLZSS.cs` | 遊戲 LZSS 與 `PFIL@` 包裝 |
| `Localization.cs` | 中英文 UI／log |
| `data/game_schema.json` | 機器可讀的欄位、offset 與 patch metadata |

目標框架為 .NET 8 Windows、WinForms、x64、nullable enabled、PerMonitorV2 DPI。程式 manifest 要求管理員權限，因為正常遊戲安裝位於 `Program Files (x86)`。

## 3. 遊戲根目錄與備份基線

`GetGamePath()` 回傳 UI 中選定的遊戲路徑。套用、回復、啟動、SAVE、語言 overlay、dgVoodoo2 都必須從這個根目錄推導。任何會寫入或刪除的入口，都應要求 `<gamePath>\Against_Rome.exe` 存在。

原版資料來源順序：

1. 內嵌 `Backup.zip`。
2. 執行檔旁的本機 `Backup.zip`。
3. 使用者所選遊戲安裝中的檔案，載入成記憶體基線。

公開版本不包含原始遊戲資料，因此第三種方式是正式支援路徑。補丁從原版基線重新產生，避免在已修改資料上累積倍率。

## 4. 寫入與回復安全

`FileRollbackScope` 追蹤每個目標在第一次修改前的 bytes 或「原先不存在」狀態。`SafeWriteAllBytes` 使用暫存檔完成單檔替換；流程成功後才 `Commit()`，否則回復所有已追蹤目標。

這不是多檔案的檔案系統原子交易。2026-07-02 工作樹雖已把 EXE、INI、DAU、team.dat、ENDL BCI 的候選內容先產生為 `byte[]`，實際仍逐檔寫入，語言與 dgVoodoo2 也有各自 I/O。安全性來自預先驗證加 rollback，不可描述成真正的單次 atomic commit。

commit 後要先 Dispose／清空 rollback scope，再更新 UI；UI refresh 例外不應回滾已完成的遊戲檔。

## 5. `PFIL@` 與 CSV-like 格式

- `DecompressPfil` 讀取 64-byte PFIL header 與 LZSS payload；無 PFIL header 時回傳原始 bytes。
- `CompressPfil` 要求原始 header 至少 64 bytes，並重寫解壓大小。
- 遊戲文字使用 Windows-1251；專案文件使用 UTF-8。
- 修改後至少驗證 `decompress(compress(payload)) == payload`。
- `objdef.dau` 還要求解壓文字總長不變。
- 資料是簡單逗號分隔，現行相容契約為 `Split(',')`／`Join`；不要換成 RFC 4180 quote parser。

## 6. `objdef.dau`

路徑：`SYSTEM/DATA_MP/DEFAULTS/objdef.dau`。

主要 zero-based 欄位：

| Index | 名稱 | 狀態 |
|---:|---|---|
| 4 | Moves | stable |
| 19 | Hp | stable |
| 23 | Movsf | stable |
| 24 | Sirad | stable |
| 52 | Name | candidate |
| 78..84 | Weapon 1 | stable；angle 82 保留 |
| 86..92 | Weapon 2 | stable |
| 94..100 | Weapon 3 | stable |
| 142 | Aw | stable |
| 146 | Vw | stable |
| 156 | HousingCapacity / `wohnwer` | stable |
| 191 | Bmovs | stable |
| 199 | Weapon 1 damage/type base | candidate |

### 6.1 單位屬性層級

原版／平衡數值先形成 fallback，自訂 preset 再逐欄覆蓋。舊版短 preset 只覆蓋存在的欄位；缺欄位繼承 fallback。不支援 spell radius 的單位，第 9 欄固定為 0。

### 6.2 人口建築容量 20 倍

- 對原版 `wohnwer > 0` 的所有 row 生效；已觀察到 22 rows。
- 寫入 `original * 20`，不是 `current * 20`。
- 保留欄位寬度與整個解壓 payload 長度。
- `LoadCurrentData` 以所有正值 row 對比 `original * 20` 來偵測狀態。
- 整合 UI、preset、apply、restore 與 state detection。

## 7. `ress.ini`

路徑：`SYSTEM/ress.ini`。

`[objres]` 主要欄位：

| 範圍 | 用途 | 修改規則 |
|---|---|---|
| 1..6 | 建造成本 | free construction 時清零 |
| 7..12 | 升級成本 | free upgrade 時清零 |
| 13..18 | 單位生產成本 | free production 時清零，保留明確例外 |
| 19..24 | 裝備／卸裝退還 | 必須保留 |
| 25..28 | 祭司法術成本 | no spell cost 時清零 |

`FigTiePac00_Packpferd` 完全排除於免費模式，保留原始成本與退還。不得重新加入 healing-food／healing-speed 修改，也不得為修 UI quota 問題而清除 19..24；騎乘村民與戰鬥單位的 reservation count 涉及 EXE/UI runtime 語意。

## 8. `cl_script.ini`

路徑：`SYSTEM/cl_script.ini`。

目前管理：

- `Radius = <faction>, SpellN, value`
- `CiviDelay = <faction>, value`
- `MoralsDecLostMem`
- `MoralsDecFlee`
- `MoralsDecOverPop`
- `MoralsIncIdle`

修改時保留註解和未管理內容，依 faction + key 定位。Balance toggle 的事件只應更新預覽，不應偷偷重讀所有 live game files。強制英文 toggle 也不應由 live overlay 自動勾選。

## 9. `team.dat`

路徑：`MAPS/**/team.dat`。

最大人口補丁只修改 `[teamdata]` zero-based column 4 為 1600；`maxteamobjgenerell` 保留。column 5 是 banner version。`team.dat` 不負責無盡模式增援 count。

## 10. AI Ultimate Mode

路徑：`MAPS/ENDL_000..004/SCRIPT/ak_level.bci`。

目前目標狀態：

- 增援 count：4 → 20；EXE runtime clamp 為 1..20。
- respawn cooldown：180000 ms → 5000 ms。
- active-party limit：4 → 8。
- completed-job recycle：0 → 1。
- gate 保持原版 `66,0`。
- 只把前三個軍事增援 polling loops 改為 `5000..10000 ms`；其他 loops 回原始範圍。

舊版 `112,272` gate bypass 已否決：它會配合過快 loops 耗盡每隊 20-slot NPC job table，造成後期不再補兵。現行 migration 一律恢復 `66,0`。

全域 CLAK 經濟 patch 也已否決：`ak_npc.bci`、`ak_produktion.bci`、`ak_haupthaus.bci` 的舊修改會讓玩家資源建築停止生產；現行程式只還原，不啟用。

2026-07-01 曾從 `ESAVE_000`／`ENDL_002` 的 `CLAK\scr.dat` 取出內嵌 `ak_level`，與 live BCI 做 exact SHA-256 比對，兩者相同，且讀到 20／5000／8／recycle 1／gate `66,0`。因此主觀「沒有變」可能是存檔已排程 job／timer，不代表 bytes 未寫入。

目前仍需五張 ENDL 地圖的長時間 late-wave regression test；短期成功不能標成完整 runtime verified。

## 11. EXE 補丁

### 11.1 失去焦點時繼續執行

- offset `0x161a88`
- original `89 15 C4 7D 9E 02`
- patched `90 90 90 90 90 90`

只接受已知 signature；Unknown EXE 不寫入。

### 11.2 村莊建造範圍與紅色虛線框

現行 patch 使用 setter trampoline：

- hook `005364c1`／file offset `0x1364c1`
- cave `0056258f`／file offset `0x16258f`
- 以 `(value * 5) >> 1` 產生 2.5x
- 保留負值檢查、呼叫 `004c0900`、回到 `005364d1`

狀態包含 Original、Legacy2x、Expanded2Point5x、Unknown；Legacy2x 必須可偵測、升級與回復。

舊四處 shift-6 → shift-7 patch 已否決：

- `0x1366c4`, `0x1366cd`
- `0x0d722c`, `0x0d723b`

現行程式不再寫入舊 `07` bytes，只偵測並還原。2x setter 曾實機確認建造範圍與紅框同步；2.5x 目前為靜態驗證、待新的遊戲內確認。

## 12. 強制英文與語言回復

強制英文開關是手動、預設關閉。語言 overlay 原版基線位於 `<gamePath>\.against-rome-modifier-language-backup`。

回復合約：

- overlay 存在且 baseline/manifest 缺失：中止並明確報錯。
- baseline 完整：精確回復原檔。
- 原版不存在的 overlay-only 檔案：回復時刪除。

不得把 active `ToEng` 檔當成原版。真實安裝曾有 332 個 overlay 檔但無 baseline，而當時 `Backup.zip` 的 146 entries 不含這些目標。修復工具為 `tools/Repair-LanguageBackup.ps1`。

## 13. 存檔與 VirtualStore

唯一 live save root 是 `<gamePath>\SAVE`。Windows 在非提升狀態啟動舊遊戲時，可能把寫入導向 `%LOCALAPPDATA%\VirtualStore\Program Files (x86)\Against Rome\SAVE`；修改器本身要求管理員並把遊戲 `WorkingDirectory` 設為所選根目錄，但外部非提升捷徑仍可能重建 VirtualStore。

ZIP 備份先建立 `.tmp`，加入修改器產生的 `manifest.json`，成功後再 move。ZIP entry 禁止 absolute path、leading slash 與 `..` traversal。存檔 restore 成功後先 commit，再清理 staging；cleanup 失敗只記錄。

## 14. dgVoodoo2

修改器內嵌 x86 `D3D8.dll`、`DDraw.dll`、`dgVoodooCpl.exe`、`dgVoodoo.conf`。它不下載 runtime dependency，也不覆蓋非受管 DLL。遊戲根目錄的 manifest 記錄受管檔與 hash；使用者改過的受管檔不會被無聲刪除。來源、版本與 SHA-256 見 `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`。

## 15. UI 與 preset

- `mainTabControl` 的 header 故意隱藏，左側按鈕負責導航。
- `StyleNavButton` 綁定前必須先建立對應 `TabPage`。
- `pnlSwitchesCard` 是核心開關區，手工座標不可大範圍自動重排。
- 新 toggle 必須同步 UI field、localization、apply、restore、state detection、preset save/load 與文件。
- `.arpreset` 使用簡單 INI-like 格式與 invariant culture；保留舊 `PopLimit`／`CiviSpeed` 相容。
- `.artroop` 目前有 9 個屬性；舊短格式缺欄位時使用 fallback。

## 16. 逆向工程工作流

查詢順序：

1. `docs/reverse-engineering/`
2. `data/game_schema.json`
3. `re_workspace/ghidra_inventory/against_rome_function_index.csv`
4. `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`
5. 必要時新增 focused script 到 `tools/re/`

`re_workspace/` 是本機證據與產物，禁止上傳；`tools/re/` 的可重現 scripts 可以公開。完整反編譯不等於取得原始碼，不能還原原始識別字、註解或 build system。

## 17. 未完成項目

- AI Ultimate 五張 ENDL 地圖的長時間回歸。
- 2.5x village range／red frame 新倍率的實機確認。
- `apt.dat` 的安全格式與用途。
- BCI opcode 的完整解碼。
- `[volkres]` 多個 candidate 欄位。
- 自動化測試；目前主要依賴 build、schema、round-trip、bytes 與實機驗證。
- 公開發佈前的頂層 `LICENSE` 決策。

## 18. 驗證

基本驗證：

```powershell
dotnet build .\AgainstRomeModifier.csproj -c Release --no-restore
Get-Content .\data\game_schema.json -Raw | ConvertFrom-Json | Out-Null
git diff --check
```

依修改類型追加：

- PFIL：壓縮／解壓 round-trip。
- `objdef.dau`：解壓長度完全相等、短 row bounds。
- EXE：original/current/legacy/unknown 四種 state。
- ENDL：五張 map、enable/disable/migration、長時間 waves、save embedded script。
- 語言：manifest、數量、path safety、SHA-256、缺 baseline abort。
- 存檔：完整 `.tmp` ZIP、manifest、path traversal、commit 後 cleanup。

## 19. 公開版本邊界

不得提交：原始遊戲資料、`Backup.zip`、`MAPS/`、`SYSTEM/`、`SAVE/`、`ToEng/`、原始遊戲封存、本機語言 baseline、`.codex/`、`.agents/`、`re_workspace/`、內部 AI 稽核、工具下載 cache。

可以提交：C# source、README、技術文件、schema、`tools/re/` scripts、已記錄來源與 hash 的 dgVoodoo2 檔。未獲使用者明確要求時，不 commit、不 push、不 rewrite history、不 force-push；本 repo 也不應使用 `git push --mirror`。
