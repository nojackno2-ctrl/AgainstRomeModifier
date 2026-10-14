# 波次生成後的群組命令

2026-10-14，Codex。目標是設計者的地圖編輯器；不重製遊戲，也不進行渲染工作。安裝目錄全程唯讀，未執行遊戲、未 commit/push。

## 結論與實作範圍

**已有原版腳本證據支持群組移動到世界位置。** 實作 `SpawnUnit.MoveDestination`（整數世界 X/Z，0–16383），Campaign wave 降階及現有波次對話框。生成原生回傳值必須等於 1，才向該次輸出的 index/UID 發送訊息 8；每次建立前清空輸出，避免失敗後沿用別組身份。未指定目的地維持原有生成 bytes。沒有增加遊戲邏輯或尋路算法。

**指定物件攻擊仍未完成證據鏈。** `s_setObjTarget`、`s_setObjAttackDest`、`s_initAttack` 的註冊／內部腳本使用，不足以確定關卡向新建群組發送攻擊命令的完整 ABI。`TargetObjectId`、`AssignedPathId`、非零 `RelativeDelaySeconds` 明確拒絕，不把移動命令描述為 attack-move。沒有假裝目標建築座標就是物件攻擊。

## 原始檔與搜索邊界

來源：`C:\Program Files (x86)\Against Rome`。唯讀解析 MAPS/SYSTEM 全部 110 個 loose `.bci`（PFIL 解壓或 raw BCI0），依已記錄 opcode 寬度逐指令掃描 `pushsym`，而非任意 DWORD 搜尋。未掃 ZIP 內可能另外存在的腳本。引用 `docs/reverse-engineering/script-natives.md`、`bci0-opcodes.md`、`endless-mode-ai.md`、`spawn-formations.md`；未將既有 gameplay 記錄當本次 runtime 驗收。

SHA256：

| 原始檔 | SHA256 |
|---|---|
| `Against_Rome.exe` | `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf` |
| `MAPS/ENDL_000/SCRIPT/ak_level.bci` | `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27` |
| `SYSTEM/CLAK/SCRIPT/ak_kampfverband.bci` | `7cab09a8bf149fd49d82b73963c0a867251d21892b3ecc35647081d8804e7e30` |

掃描結果：`s_conMoveTo` 46 呼叫，全部在單位腳本；`s_setObjAttackDest` 14 呼叫；`s_setObjTarget` 2 呼叫（priester/zivilist）；`s_setObjTargetPos` 1 呼叫（priester）；`s_setObjDestPos` 0 呼叫。這些是此集合的呼叫數，不是指令不存在的證明。

## 可核對的移動模式

下列 offset 均為 **code-stream relative**；raw BCI0 檔偏移需加 `0x24`。

ENDL_000 function `0xB0E8` 從 obj array 讀出 recipient index/UID（locals 3/4），檢查位置／到出口距離，在 `0xB374..0xB3D4` 推入十個參數並呼叫 symbol #94 `s_sendMsg`。逆向 push 順序：

```text
66 2        // priority
66 0
66 0
90 -5       // destination Z
90 -4       // destination X
66 8        // move message
90 4        // recipient UID
90 3        // recipient index
81 6        // sender UID (original level globals)
81 5        // sender index
128 94      // s_sendMsg
73 -10
```

原始 96 bytes（不含原腳本接著取回／丟棄回傳值的 `86,71`）：

```text
4200000002000000420000000000000042000000000000005A000000FBFFFFFF5A000000FCFFFFFF42000000080000005A000000040000005A0000000300000051000000060000005100000005000000800000005E00000049000000F6FFFFFF
```

`ak_kampfverband.bci`：`0x8B50..0x8BB4` 透過 `s_getMsgExt` 讀入訊息；`0x8EE8..0x8F10` 比較 message=8，`callint` 的相對位移 -31620 解析至 `0x138C`。該 handler 檢查 `s_tstMoveAllowed`／`s_cCombat`、crew/load 等條件，再於 `0x151C..0x1580` 把訊息參數轉交 `s_setObjActivity`。移動 controller `0x15D0` 於 `0x1688..0x16E0` 推入目的地、identity 與控制參數，呼叫 `s_conMoveTo`。這是群組自身腳本接受／執行移動的靜態流程證據；命令仍可能受遊戲狀態條件拒絕。

EXE 既有註冊證據：`s_sendMsg i(iiiiiiiiii)` callback `0x521BA0`；`s_conMoveTo i(iiiiiiiii)` callback `0x5345C0`。本次確認 EXE hash 對齊，未重新 decompile native callback。REA MCP 不在工具清單；`npx.cmd ... doctor` 因 npm registry EACCES 未成功。Python entry point 被 application-control 阻擋；使用 shell 內 inline 唯讀 PFIL/BCI 解析，未調整 policy。TEMP `.ps1` 載入也被 ExecutionPolicy 阻擋，不能當已成功的工具執行。

## 重定位與未知邊界

編譯器保留原始 payload/message/priority/argc，以 literal 目的地替代原 local 座標，creation locals 0/1 替代 recipient locals 3/4。sender 使用同一新建群組的 index/UID，避免借用任意原 level global；這是編輯器的明確身份選擇，**原 ENDL 範例 sender 是 level object，self-sender 的實際 queue／confirmation 行為尚未遊戲內驗證**。不宣稱原始 96 bytes 原封不動；測試明確套用以上重定位再逐 byte 比較。

新增核心邏輯只有既有 C# scenario compiler 的必要整合，沒有新增獨立 parser/core subsystem；保留 repository Rust 優先規則與現有 C# 宿主。

Scenario action 編輯保留既有目的地，event summary／wave preview 顯示目的地；本次 `MapEditorForm.cs` hook **0 行**。未改 user guide（既有未提交修改）或其他代理檔案。

## 驗證

- `dotnet build AgainstRomeModifier.slnx -c Release --nologo`：sandbox NuGet.Config access denied，9 errors；正式提權首重跑成功，0 errors／47 warnings。新增最後兩測試後重建成功，0 errors／9 warnings（incremental 結果，不取代首 build 警告數）。
- host focused：72 pass／0 fail／0 skip；Modules focused：15 pass／0 fail／0 skip。合計87。涵蓋原版 bytes 重定位、建立失敗／多組及重複事件身份、條件 locals、目的地邊界、Scenario JSON 存讀、Campaign 降階、dialog preview／編輯保留。
- `git diff --check` exit0；EXE 及上列兩 BCI 結束時 hash 與起始相同。
- 未執行遊戲內驗收；不得將測試 VM 或 byte pattern 當作實機群組已移動。

最終測試指令（經正式提權執行）：

```powershell
dotnet test tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj -c Release --no-build --no-restore --filter "FullyQualifiedName~ScenarioEventsTests|FullyQualifiedName~CampaignWaveDialogTests|FullyQualifiedName~MapEditorCampaignWaveTests|FullyQualifiedName~ScenarioEventDialogTests" --logger "console;verbosity=minimal"
dotnet test tests/AgainstRomeMapEditor.Modules.Tests/AgainstRomeMapEditor.Modules.Tests.csproj -c Release --no-build --no-restore --filter "FullyQualifiedName~AiArchetypeTests|FullyQualifiedName~ScenarioEventSessionTests" --logger "console;verbosity=minimal"
```

本次修改檔案（其他 pre-existing／其他代理修改保留）：

- `src.Shared/Scripting/ScenarioEvents.cs`
- `src.Shared/Scripting/ScenarioEventCompiler.cs`
- `src.MapEditor.Modules/AI/CampaignMissionModels.cs`
- `src.MapEditor.Modules/AI/CampaignMissionCompiler.cs`
- `src.MapEditor/CampaignWaveDialog.cs`
- `src.MapEditor/ScenarioEventDialog.cs`
- `tests/AgainstRomeModifier.Tests/ScenarioWaveOrdersTests.cs`（新增）
- `tests/AgainstRomeModifier.Tests/ScenarioEventsTests.cs`
- `tests/AgainstRomeModifier.Tests/CampaignWaveDialogTests.cs`
- `tests/AgainstRomeModifier.Tests/ScenarioEventDialogTests.cs`
- `docs/agent-reports/wave-orders-re.md`（新增）
- `AI_HANDOFF.md`（目標與狀態一筆 concise entry）
