# 十路 Agy 功能逆向複核

2026-10-09，使用者要求十個 Agy 子代理，重點為地圖編輯器與遊戲修改器。batch `edfa7bac1533` 實際以 Agy、max_concurrency=10 派送十路，禁止改檔、Git 寫入、執行遊戲及共用 REA session。八路回傳 exit 0 報告；物件池及儲存交易兩路在 toolbox 初始化時 exit 3，REA `trace_dylib_resolution` schema 的 negative lookahead 不受 AGY regex engine 支援。此為工具相容性問題；未修改全域 MCP。限縮兩路檔案範圍補 batch `584d352bbc34` succeeded，兩路分別 exit 0／62秒、exit 0／46秒，十個分工均已取得報告。初批仍為 failed，不改寫其歷史狀態。

## 十個分工的核對

本表區分原始報告與父代理用目前 source／既有原生證據的複核，不把文件重述算作本輪新反組譯。

| 分工 | 接受的觀察／後續目標 | 修正或限制 |
| --- | --- | --- |
| 物件關聯池 | Add 將 +71/+73/+77 寫為 self，Remove 未同步 anim／gfxtype／action；原生各有獨立 allocator/reset，產品跨池契約待建立。 | 補報稱 +73 尚未確認，是同期閱讀舊版本結果；父代理本輪已確認 gfxtype reader／allocator／validator。也未證明原版空圖必然1:1。 |
| 儲存交易 | store.Save、scenario.Save、script Apply 共用 rollback；AcceptChanges／events baseline 在 Commit 之後。新增 pools 應納入同一 scope 並在提交前驗證。 | 駁回「完整保證磁碟與記憶體不脫節」：Commit 後仍有 reload／UI 操作可能失敗；RestoreAll 的每檔例外會被捕捉，並非任何環境都能保證成功還原。本輪未新增故障注入或變更交易行為。 |
| UID／slot | LevelObjectStore.Add 使用首個空槽及現存最大 UID+1；移除最大 UID 後可能再用同一組 slot／UID。應追查跨存檔及歷史引用。 | 駁回「slot+1 只有 C# 模擬器證據」：scenario-event-conditions.md 已列 handler 0x5194f0／implementation 0x50f270 的 index-1、UID compare，以及 DATA reader 0x48bae0 保持槽順序。目標是 Against_Rome.exe；報告混稱 ar.exe 不當新證據。重用本身不證明 bug，持久編輯身分與當次 binding 已存在。 |
| 空白地圖 | BlankMapBuilder.Create 透過 ClonePrepared 建立副本，保留 SDL／派系／其他池；BlankMapContent 重設已辨識資料。零物件 runtime 與 SDL consumer 仍需追查。 | 不能拿「只重設部分池」推論目前產品缺其餘檔：它從完整範本複製。SDL 清空卡死為 source 記載的先前實測，本輪未重做；33 inactive targets 不能排除或證明卡死原因。 |
| 高度快取 | Persistence 只在 HeightsDirty 且高度層存在時寫高度並清四快取；TerrainLayerFiles 先 TrackFile 再刪，納入 rollback。 | 高度總和相同仍會觸發目前的 HeightsDirty 刪除；Agy 所述未刪除風險不是已重現產品 bug。HandleSkyDensMap／HandleVisibleMap 開關的原生覆蓋範圍待查。 |
| 尋路／visible | NavMeshPassabilityGrid 使用 collision!=0 及水位，未套用 visible bit0；map-formats.md 既有原生 0x4d8830 會將 bit0 未設的 tile 寫255。預覽與引擎通行判定可能不同。 | 不能將 visible bit0 完整定義為「世界幾何視野」，也未證明 FOW 絕不影響其他尋路路徑。灰階值是代價的說法仍是假說，需追 consumer；原有文件的 >0 阻擋證據不能省略。 |
| 事件生命週期 | Compiler entry 初始化 deadline／seen／mission state，輪詢已接入原等待點；遊戲存讀檔是否重跑 entry 及保留 ScriptVarL 尚未知。 | 「缺乏狀態暫存原語」與既有 s_get/setScriptVarL 及 SEEN keys 矛盾。一般 AND 是目前設計；ObjectDeadOrRemoved 已跨輪詢保留曾見狀態，不能籠統說多目標摧毀完全無法記憶。 |
| 運鏡 | CompileCameraCalls 有已核對位置／縮放 ABI，IsWiredToLevelScript=false；時間排程、姿態映射、控制權恢復仍未完成。 | GenerateBciScriptText 的 Pitch／Yaw／distance 是明示預覽註解，不是未支援原生呼叫。缺少已辨識原語不證明引擎完全沒有內插或旋轉能力，也不證明必须 native hook。 |
| AI 時鐘 | P6 scheduler v16 有 now/deadline rollback 分支；P4 及其他 loops 為不同範圍。需查各 consumer 與 save/load rebase 順序。 | 未證明原版時鐘常態回退或其他 deadlines 實機停止；scalar reader 不等於持續時鐘。新候選 caller 需原 bytes 複核。 |
| 屍體／容量 | 正確 opcode 0x510906 的500→50記憶體補丁已獨立驗證。維護函式 caller、第二個 call 回傳語意、allocator 耗盡處理為下一步。 | 不能由40-byte window推出「50槽一定溢出／效果反轉」。0x5108fc getter0x4ae4a0讀0x771ca8已有證據；0x51090b 的另一個 call 與完整清除控制流需追查。實機效果未驗證。 |

## 父代理同期新原生證據

[map-data-pools.md](map-data-pools.md) 新增 gfxtype reader／record／平行陣列、objects +73 關聯及非全零 reset 預設。三個 pool 共222檔精確長度驗證通過，74對 gfxtype links 的唯一33筆 inactive targets 與 anim／action 物件槽集合相同。本輪只改分析工具與文件，未改產品跨池儲存或遊戲原檔。

## 下一步

先建立 objects／position／objdata／anim／gfxtype／action／hirarchy 的新增、移除與交易契約，再決定產品修改；不可直接將 object slot 當成所有 pool slot。後續原生 consumer 及受控實機驗證仍未完成。兩批均 terminal，十路均已返回，不重派已完成工作。
