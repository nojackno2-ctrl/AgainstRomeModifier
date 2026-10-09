# 五路 Agy 功能逆向複核

2026-10-09，使用者指定五個 Agy 子代理。第一批 `c4d7dddb3397` 派送物件池、地形尋路、任務運鏡、無盡 AI、鏡頭／容量五路，限制唯讀且禁止占用父代理 REA。結果：物件池／AI／鏡頭容量三路返回報告；地形路 309 秒 print timeout（雖 exit 0，未視為完成）；任務路 toolbox 初始化 exit 3，原因是 REA `trace_dylib_resolution` schema 的 negative lookahead 不受 AGY regex engine 支援。沒有據此判定目標遊戲有問題。

地形／任務兩路用較小檔案範圍、不使用或 discover MCP 的新提示補跑，batch `1c57ecaa005d`：兩路 exit 0，分別 55／51 秒返回報告。五個分工均取得分析報告；初批失敗及逾時仍保留記錄。未修改全域 MCP 設定。

## 已返回報告的父代理複核

| 項目 | 可接受的證據 | 限制／駁回的說法 |
| --- | --- | --- |
| 物件新增同步 | `src.Shared/Maps/LevelObjectStore.cs` Add 將 record+71/+73/+77 寫為 self，實際只持有 objects／objdata／position；新確認的 +71 anim、+77 action 沒有相應 pool 分配。與 ENDL_005 的 33 對 inactive targets 及 self-index pattern 相符。 | 可確認程式契約缺口及相符 pattern，尚不能只靠現在檔案歸因全部歷史修改。CHECK 對無效 anim 會跳過，不接受「此 CHECK 已證明非法存取／卡死」。 |
| 物件移除／hirarchy | IsLinked 只使用 column10／objdata segment2，Remove 沒有更新 anim/action/hirarchy；原生群組 backlink 是 column4。需建立跨池受控編輯契約。 | 具體可到達案例與遊戲影響待驗證。Agy 說「4張歷史/戰役地圖異常」錯誤：manifest 是 **MP_016 一張圖的四個群組** duplicate-members；也不能宣稱這些樣本本輪實機不崩潰。 |
| AI deadline coverage | `src.Core/Core/EndlessAi/CustomPatches.cs` Apply 非 scheduler loop 使用字面量改寫；P6 rollback predicate 對 v16 scheduler。這是應逐 consumer 追查的範圍差異。 | 不接受「其他計時器已確定停滯數十分鐘／小時」。需確認各時鐘來源及存讀檔調用順序；Agy 提出的新 caller/serializer 定位先作候選，不冒充本輪原生追查成果。 |
| Trace 觀測範圍 | `native/argm-trace/src/targets.cpp` 現有 hook 表未提供 clock capture/rebase 的專用 hook。補充時鐘觀測有功能價值。 | 任意安裝 trace DLL 不是本輪行動；只提出 fingerprint／ABI／生命週期驗證後的候選工作。 |
| 屍體容量與補丁位置 | 原 EXE 0x5108f7 `BB B0 36 00 00`，0x5108fc call 0x4ae4a0；getter `A1 A8 1C 77 00 C3` 返回 0x771ca8。**原指令 VA/raw offset 0x510906/0x110906 才是 `BB F4 01 00 00`**。`ExePatchModel.CorpseRetentionPatchOffset=0x110907` 處實際為 `F4 01 00 00 E8`，與宣告 OriginalBytes 不同。 | 駁回 Agy 的 0x510907 原指令簽章與「契約全匹配」。本次直接 read_va／原 file bytes／PE section mapping 三者確認 off-by-one；後續限定修正已將 C# offset 改為0x110906；獨立native window與實際EXE記憶體套用/還原通過，矩陣已更新。遊戲運行仍未驗證。500→50 不代表最大屍體數或保證保留50空槽。 |
| 鏡頭 cave | 既有 cave 以 IEEE float bits 夾住下限，tail-jump 至原生 setter；大於等於1的值走既有 setter。 | 非負有限 float 的 bits 排序與數值排序一致，僅此不構成「浮點／整數矛盾」。放行2.0也不能單獨證明裁剪漏洞。consumer 的實際整數化與 picking／裁剪仍需逐路驗證。 |
| 地形補跑 | `CliffFacePlanner.ApplyPlan` 做貼圖 stamp／碰撞，未直接改高度；原生 visible bit0 與 collision 的聯動值得追。 | 未直接改高度不等於違反功能契約，也不能推導必須清高度快取或陰影已錯位；須追宿主 transaction。不接受把 visible.dat 一概解釋成玩家尚未揭露的 fog-of-war。 |
| 任務／運鏡補跑 | `CinematicBciCompiler.IsWiredToLevelScript=false`；binder 明確封鎖野獸 timer／team8 SpawnUnit／資源再生與部分獎勵；Injector 對 Events 呼叫 ScenarioEventCompiler。 | 「僅開局生成」只適用 ScriptSpawns shim，不能套用整個 Injector 或已支援的定時 Events。不從指定文件的 bounded search 推論所有 DATA 都沒有野獸。 |

這些報告用於安排下一步逆向與產品契約修正，不是遊戲卡死已修復或實機功能已驗證的證明。父代理新增 [action／hirarchy 證據與唯讀診斷](map-data-pools.md) 已提交 `0242924`；後續已修正 CorpseRetention opcode offset；Agy修正job `bfeb5873352b` exit0，父代理完整Release build31warnings/0errors，dotnet test 1,576passed/24skip/0fail，ARM_GAME_PATH安裝EXE唯讀focused tests 3passed。僅memory變更兩個立即值bytes，disk SHA256不變；未啟動遊戲。
