# 2026-10-21 全功能反編譯驗證索引

## 範圍與判定

本輪核對目前 Rust 修改器的完整功能 ID 清單，另將地圖編輯器與存檔管理器列為工具箱補充範圍。**完整盤點不等於所有功能通過**：每項的寫入目標、遊戲讀取證據、還原能力與未驗證行為分開判定。

- [修改器逐項矩陣](modifier-rust-feature-audit-2026-10-21.md)：對照目前 registry、套用、偵測、還原。
- [地圖編輯器與存檔管理器矩陣](editor-save-feature-audit-2026-10-21.md)：格式與消費路徑，以及工具自身測試邊界。
- 舊 [49 項矩陣](feature-verification-matrix.md) 為歷史證據，不能直接當成本輪 Rust 通過紀錄。
- 本輪未執行遊戲、未修改安裝遊戲；未重新驗收 GUI、dgVoodoo 畫面、AI 長期運作或任意使用者組態。

## 本輪直接分析的原版

安裝版 `Against_Rome.exe`，x86 PE，2,486,272 bytes，SHA-256：

`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`

REA 使用 Ghidra 12.1.4 與預設分析；analysis profile digest：

`3c06738e45e144d5f073edbd3244c6bff46222f389525fe659bc92dca2ccdece`

33 筆成功 Evidence（31 個反編譯查詢、2 個 xref）保留於 ignored 的 `re_workspace/feature-audit-20261021-rea-evidence.json`。完整反編譯與原版 EXE 不提交。Evidence 的 `unknowns=0` 只代表未向 REA 登錄未知事項，**不代表本專案沒有驗證缺口**。

## 新證據與對應功能

### 已確認的問題與涵蓋限制

1. **原生寬螢幕與歷史安全方案相反**：現行 `PatchEngine::compose` 確實呼叫會寫入六個原生補丁的 `apply_profile`；舊紀錄要求還原 EXE 並使用 dgVoodoo 置中。零售記憶體測試確認開啟此 ID 會改 EXE。這是實作與歷史驗收政策的矛盾；本輪未觀察新版遊戲畫面，未改產品行為。
2. **三個直接別名輸入消失**：對 `EndlessAi.M2/M3/M4` 各設 true 再 normalize，實際全變成 false，Core 也為 false；透過舊 modules map 的輸入另有橋接，不能混為一談。
3. **駐軍倍率未傳入實際補丁**：讀取安裝版 `Dorfverteidigung.bci`，在 orchestrator 的記憶體快取中要求 2／3／5／10 倍，實際 helper 全為 **3 倍**，來源 BCI 完全不變。另把 registry `VillageGarrisonQuota3x` 設 5，實際觀察 registry=5、`VillageGarrisonQuotaMultiplier`=0、enabled=false；GUI／CLI 的特例橋接也沒有補上 P20 物件的倍率傳遞。
4. **未知速度狀態會回報 Ok 但不套用**：只在記憶體把 QPC 常數改為 0，再要求 2 倍，實際回傳 Ok、位元組完全未變、偵測倍率仍 0。這保護未知資料，但缺少「要求未達成」訊號。
5. **偵測涵蓋不足**：若干速度、成本、射程僅抽樣代表單位；彈道未驗證 partgeo；MaxPopulation 的未知 team.dat 可能被判開啟；CustomUnitStats 依賴 Balance 且無完整磁碟反推設定。完整來源定位與界限見逐項報告 A4..A6。
6. **部署文件過時**：ArgmTrace 現行是 `winmm.dll`，舊矩陣的 `version.dll` 為遷移清除目標。DLL 存在與 marker 不能證明載入／hook 成功。

第 2..4 項已由父端 Rust 探針直接呼叫目前 crate API 重現，log：`re_workspace/feature-audit-20261021-contract-probe.log`；probe 位於 ignored `re_workspace/feature-audit-probe/`，指令：`cargo run --manifest-path re_workspace/feature-audit-probe/Cargo.toml --offline --target-dir re_workspace/target-w20`。probe exit 0 表示觀察程式完成，**不是三項功能通過**；未執行遊戲或寫入安裝版。這補充了分工報告原先的靜態推導。

### 遊戲消費路徑

| 功能群 | 本輪觀察 | 判定邊界 |
|---|---|---|
| 屍體保留 | `0x5108f0` 以 14,000 pool 與 500 保留值計算本輪清理數量 | 證實補丁常數用途；50 的效能與長期穩定性未實測 |
| 閒置選取 | `0x451dc0` 原版兩個 40-entry 陣列；`0x44d5a0` 的全域選取數量受 999 限制 | 支持 999 上限；本輪未實際點擊 UI |
| 村莊建造範圍 | file offset `0x1364c1` 所屬 `0x536450` 同時呼叫 `0x4c0900` 並寫入 per-object 的兩個 short | 支持兩份邊界同步，不是僅改紅框 |
| 羅馬無盡派系 | `0x45bd60` 使用同一參數更新派系、全域選擇與玩家隊伍 | 支持 setter 入口；不證明 AI 生命週期 |
| 平民生產／轉職批量 | file offsets `0x4fc00`、`0x4c7dd` 分別解析到 `0x44fbd0`、`0x44c6b0` | 確認 UI 生產／選擇呼叫鏈；保留原有條件與上限 |
| 失焦暫停 | file offset `0x161a88` 所屬 `0x561a70` 寫入 pause-state 並要求重繪 | 支持 NOP 的目標；視窗失焦效果未重測 |
| 遊戲速度 | `0x604214` xref `0x55df8d`、`0x60424c` xref `0x55e586`；新反編譯含 QPC frequency 與 timeGetTime 換算 | 支持兩條 clock 路徑同步；不代表任意倍速穩定 |
| 鏡頭距離 | `0x498a30` 設定／限制 zoom 並觸發兩個更新函式 | Ghidra 對兩個末端 call 提供 flow-model 限制，不能省略 |
| 移速／戰鬥／視野相關欄位 | `0x4b0300` 重建 objdef 欄位輸出次序；`0x4bb480` 讀取多種移速；`0x4c0e60`、`0x4e0bd0`、`0x542730` 為武器距離鏈；`0x4e0a70` 消費傷害與榮耀值 | serializer 與 consumer 支持欄位對應；不把 serializer 單獨當 loader 證明 |
| 回血 | `0x512aa0` → `0x4ad190`，`0x4ad1e0` 執行 LP delta 與原有有效性檢查 | BCI 實際呼叫參數與各腳本仍須逐項對照 |
| 法術／族群參數 | `0x41bce0` 註冊 TribeData、Spells、SpecialAbilities；`0x541440` 有 tribe/spell/field 範圍與 clamp；`0x540dc0`／`0x540fe0` 為族群 setter/getter | `CiviDelay` 的 500 下限與新設定相容；具體效果不能只靠 section 名稱推論 |
| 法術費用與祭壇 | `0x44a010` 同時比較資源成本與 1/2/3/4 祭壇門檻；`0x46e1e0` 取出四個成本值 | 費用與祭壇是不同 gate，必須分別套用 |
| 投射物弧線／範圍 | `0x4bb770` 發射、`0x4dbe60` 飛行積分、`0x4e0a70` 傷害 | 支持機制；命中率、碰撞與視覺效果需實機 |
| 復活／建立單位 | `0x5245d0` 保留隊伍／類型／數量限制並建立物件 | 不能把 Value=100 解讀為不受 engine clamp 的保證 |
| 地圖物件／高度 | `0x487f70` 序列化多段物件資料，`0x49bd50` 組合地形高度與附加高度 | 支持多池與地形格式驗證；不能推定空圖或所有組合可遊玩 |

屍體函式 Evidence：`ev_034d8e7a2e7895acae769fd169a3fe3633212bdff125f8d41e6ea256b50fc461`。
兩個速度 xref Evidence：`ev_0f4595c9e4c83eac6c42cbb92b48207a9c6b877efaaa65b860d31653f92267c1`、
`ev_d0617d59c026b9874f093d0c3a3bd2ad5f3225a5663d83ed1a050f8f179bdd19`。

### 其餘新反編譯 Evidence 定位

長度為完整 pseudocode 的字元數，只有定位資訊提交至 Git。

| 查詢（VA 或明確 file-offset） | Evidence ID | 字元數 |
|---|---|---:|
| `0x00451dc0` | `ev_ea16d3efba3912cbf2aac528235ffac4252cf3889cd67be6192b6b5a9eb01a17` | 776 |
| `0x0044d5a0` | `ev_75c97b8899bf4a507fafb3c371dda7d8d31beb71cc45106316d8cb5f3205c21a` | 748 |
| `0x00498a30` | `ev_84a39fcb4dc98417e7de4ee7f19632b0d7c2a3e1954ce16a3ea64cf191f74132` | 385 |
| `0x004c0900` | `ev_e7364ed3de2c82aa02cfc782e62d759586c9c9483e1ce5324493221c04c99b69` | 438 |
| `0x0045bd60` | `ev_3cdd6f3b3a81f805f61c325d613e1a79744d114cf5d640464d579c8bd1340461` | 198 |
| `0x0041bce0` | `ev_ec11359149f4b5b759471e6e9aed9bbddedead724ad5e8d8a764e6f8031044e3` | 2494 |
| `0x0044a010` | `ev_d3fab007d32daf4a111e7b68984fbc7da0dd020d136ebf71343950c0360a431c` | 4433 |
| `0x004b0300` | `ev_5c9943184de853bef9b73b4d86509edadc614d6cdef67e2ea80459ce9a3f211a` | 13521 |
| `0x004bb480` | `ev_148d658e10ed0a6e0bf158fe87c6f0d2e3f08ac62148fdcba77bcbbf8fa30688` | 1349 |
| `0x004c0e60` | `ev_6ac3f5a942234ee1890ee06cd9b8c40d685e3e3f19c22a6db8b2f54e1945c972` | 471 |
| `0x004e0bd0` | `ev_d5a8ed5cbaa91a66a48ece2eb98fe4ec233351fe090db51938d72357f60d5cae` | 1586 |
| `0x00542730` | `ev_75f33ba80c87d96656b73fde5a65cd999c49a558705da472ab6cdd040710a621` | 343 |
| `0x00512aa0` | `ev_8ccfc08593c4c75e91713244bf2ed95ea09a6a1d364bc7214e19ddfa65e69075` | 281 |
| `0x004ad1e0` | `ev_72d571436be61f4afe390899f1580e8690cacc6626b7e2d708b524437379035b` | 4486 |
| `0x0046e1e0` | `ev_6fe2c01873447c0508b30613b23543d4039385b483db0c02894f70a4ec3519d1` | 668 |
| `file-offset:0x161a88` | `ev_b65e1c9f9d08f71a8a65603a69e0d4042fae4863788b57fb7e4c7a61ec278d17` | 194 |
| `file-offset:0x4fc00` | `ev_8712b4d14e21494304ca8082be0cc4f6280ed5ea182ce8b0d3a0b67e0674b30b` | 328 |
| `file-offset:0x4c7dd` | `ev_c1e41e3ec1ffd3a73d9ca46d6d5012cac4752d0100ccf4fd2cde6eb96a8d92e4` | 1688 |
| `file-offset:0x1364c1` | `ev_c9a386e3fb0cb81bc315b20df641a83d3a481056f5b8c962f33ba5cdd563bd8c` | 775 |
| `0x00541440` | `ev_72bb9223f331892632c738b6c6065209beaf632bb5f72cf3aeefe05e28da6365` | 1892 |
| `0x004bb770` | `ev_af36214268ae04a15db85aebad323026176f4496a941429f75ab4fe5848104ea` | 10669 |
| `0x004dbe60` | `ev_ee66d78e2faf545176cda08de9b073fd0c3d999af46ba64fb7cdf97b37b077dd` | 25434 |
| `0x004e0a70` | `ev_5835d397383ab6cbed7c9daacf65f48bd981bb7230ec6eff0840b8f28b761195` | 1581 |
| `0x00540fe0` | `ev_bb05497fd4df4f0f420d8734819764a84b442dc863e8f87a450c5128050b8739` | 1343 |
| `0x0055df60` | `ev_3d58a8b32953b80f873484ca9c948ddc3e6f981d633a1083b6245cceda93d2a6` | 910 |
| `0x0055e550` | `ev_6f72ff77fd89cf3e3e397bd554e843d6171eec871e1784b300424dbae1a213f0` | 353 |
| `0x00540dc0` | `ev_1e479c1d41596ceba39a35b9cc4aff70eb5c59f651817e54c1caff3dbc5fe938` | 2182 |
| `0x005245d0` | `ev_31a3f07fd9a8dfae89c2887404f076b398272d2f768fbd31036f6659464df45e` | 2458 |
| `0x00487f70` | `ev_29be34b8bae6bf1445efcfe527cedfbebd0aeb19f11daa88de3199230485f164` | 43410 |
| `0x0049bd50` | `ev_1e477607c39d3022c231dc8d184a08623a470bc4935ccf864c152af6c4c72dd9` | 354 |

## 失敗與補充方法

REA 預設分析未建立 `0x46bd00`（objres）與 `0x46b200`（volkres）callback 函式。兩次查詢得到找不到 procedure 的錯誤；工具後續嘗試 file-offset fallback 仍失敗，不能解讀為該 VA 無效或程式不存在。改以既有 `GhidraRessAnalysis.java` 的明確 createFunction 流程補充，並同次執行 `GhidraScriptIniAnalysis.java`。

第一次 headless 直接傳入含 `Program Files (x86)` 的來源路徑，批次檔解析錯誤、exit 255。第二次改為 ignored 目錄唯一副本，核對 SHA-256 一致；輸出 `re_workspace/feature-audit-20261021-callbacks-copy.log`。結果 exit 0、Post-analysis succeeded／Import succeeded；兩個 callback 均成功建立與反編譯。

`objres` callback 的 case 0..30 對應 31 欄，case 25..28 寫入 `0x73eb0c..0x73eb18`，與新反編譯的 `0x46e1e0` 四成本 getter 相符。`0x46ce60` 也逐一命名 `bau/upg/aus/auf` 與四個 `spruch`；支持保留獨立 `auf` 欄位，而非一律歸零。`volkres` callback 的八層群組與大型 runtime row 使用不同型別指標，不能從 decompiler 的 `*0x129`／`*0x4a4` 顯示形式誤判為不同行大小。Script INI 腳本新增的是 parser 的字串 xref；本輪未強制建立全部 key callback，具體 key setter/getter 證據另外列於表中。

## 自動驗證

### 碰撞與存檔入口補查

新增 `tools/re/GhidraFeatureVerification.java`，明確 VA 參數、缺失函式建立、逐項反編譯失敗報錯。以相同 SHA-256 副本執行 `004d87a0 004d8830 004ae320 00431410 0048e960`，exit 0、`功能驗證全部完成：5`、Post-analysis／Import succeeded。raw log：`re_workspace/feature-audit-20261021-collision-save.log`，不提交偽碼。

- `0x4d87a0` **只清空** 256×256 short 碰撞格，不能稱為 BMP 解碼器或門檻 consumer。
- `0x4d8830` 確實將 visible bit0 未設的格子標 255。
- `0x4ae320` 是按 object/type 重建每物件 byte 狀態，**未消費**碰撞格；不能用此函式支持 `>0` 或 `>128` 的門檻。
- 因此，`environment_ui.rs:98` 的 `>128` 與舊 `map-formats.md` 的 `>0` 仍是**未解疑點**，不是已確認產品 Bug；需補實際碰撞 reader 與尋路／放置 consumer，包含 1/127/128/129/255。
- `0x431410` 確實在組合 USER／level 路徑後還原 `DATA/svar.dat`，呼叫 `0x474b00/0x44e1b0/0x469370` 並進入 `0x4870e0`；支持存檔與地圖共用部分載入鏈，尚不證明全部 metadata／script tasks 已安全恢復。
- `0x48e960` 重新確認 24 pools 的巢狀開檔鏈；缺檔可中止後續 reader。這不是「檔案可省略」證據，亦非所有 pool record／tail 的逐欄位證明。

### 測試結果

新增 `native/against-rome-modifier/tests/retail_exe_feature_audit.rs`，以 `ARM_AUDIT_EXE` 明確指定唯讀來源，10 個 EXE 切換功能（含原生寬螢幕低階路徑）與速度 2/5/10，在記憶體驗證：

- 原版簽章與預設組態不改位元組；
- 單項套用實際改變位元組、維持檔案長度；
- 重複套用冪等、關閉與分類還原完全恢復；
- 合併套用與還原、最後來源檔逐 byte 相同。

此 gate 僅驗證位元組轉換安全性。原生寬螢幕即使通過，也不能推翻歷史實機否決。

執行方式（依本機工具鏈設定 CARGO_HOME／RUSTUP_HOME）：

```powershell
$env:ARM_AUDIT_EXE = 'C:\Program Files (x86)\Against Rome\Against_Rome.exe'
cargo test -p against-rome-modifier --locked --offline --target-dir re_workspace/target-w20 --test retail_exe_feature_audit -- --ignored --nocapture
```

完整 workspace 依序執行 `cargo check`、`cargo test -- --test-threads=1`、`cargo build`，均加 `--workspace --locked --offline --target-dir re_workspace/target-w20`，三階段皆 exit 0。紀錄為 `re_workspace/feature-audit-20261021-validation.log`；276 個 test-result groups，**2,315 passed／0 failed／16 ignored**。host 為 `x86_64-pc-windows-msvc`、rustc／cargo 1.99.0；無 MSVC 連結錯誤，未測 Visual Studio 互動除錯。

最終新測試另依序執行 modifier `cargo check --tests`、上述明確 ignored gate、`cargo build`，三階段皆 exit 0；**1 passed／0 failed／0 ignored**，10 個 EXE 切換功能與 3 種速度及組合往返成功。紀錄 `re_workspace/feature-audit-20261021-retail-exe-final.log`，不與早先 workspace 計數混算。新檔 rustfmt check 通過；安裝版 EXE 最後讀回 SHA-256 與起始值相同。
