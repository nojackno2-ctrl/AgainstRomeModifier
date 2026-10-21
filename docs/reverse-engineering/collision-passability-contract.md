# 碰撞值、通行門檻與聚落選點契約

2026-10-21，Codex。結論：**原生 collision 不是以 128 為界的布林遮罩，也不能把除錯繪色的 `>0` 當成所有原生尋路的阻擋條件。** 聚落選點原本接受 6～128，超出已確認的原生空地選點必要條件；本次修正為拒絕 `>=6`。通行診斷的非零阻擋保留為未指定單位能力時的保守策略。

## 證據身分與查詢範圍

- 唯讀輸入：`re_workspace/feature-audit-input-0b4482432e794d7392816166aacfdae1/Against_Rome.exe`。
- SHA-256：`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，本輪已用 `Get-FileHash` 核對。未修改或執行安裝版遊戲。
- Ghidra 12.1.4 PUBLIC、REA JDK 21.0.12.1+1；本輪直接執行 headless 腳本，沒有使用 REA Evidence ledger。
- `tools/re/GhidraFeatureVerification.java` 新增 `data-xref:VA[:十六進位長度]` 及 `listing:VA`；保留原有明確 VA 模式。查 xrefs 後補掃指令的 scalar 位移，避免 Ghidra 未建立索引式存取的 data reference。
- 只查精確 base 會漏掉 reader：它們使用 **base-2 的 dword 讀取，再 `SAR 16` 取高半字**。因此查 `data-xref:01e21c50:21000`，含 base 以下 0x1000 與完整 0x20000 格區；不是全專案盤點。
- 首次 `data-xref=...` 被 Windows 批次檔拆成兩個參數，腳本失敗；改用冒號。Ghidra 即使腳本失敗也可能 exit 0／Import succeeded，必須另查 `ERROR` 與「功能驗證全部完成」。新增 listing 時一次 Java 區域變數命名衝突，已更名後重跑成功。

成功 raw log 全部留在 ignored `re_workspace/`，不提交遊戲或大量偽碼：

| 檔案 | 用途／成功函式數 |
|---|---|
| `collision-contract-offset.log` | array 範圍、偏移指令與 reader，7 |
| `collision-contract-callers-pass2.log` | reader caller、getter 與判斷指令，5 |
| `collision-contract-consumers.log` | 尋路門檻來源、選點 max，6 |
| `collision-contract-placement.log` | 選點 caller、max 呼叫堆疊與 `CMP 5; JG`，9 |

## loader → reader → consumer

| VA | 已觀察的原生行為 |
|---|---|
| `0x486ec0` | 真正 `collision.bmp` loader：先呼叫清零；成功解碼為 32bpp 後，RGB 非全零的像素以整數 `(R+G+B)/3` 寫入 short array `0x01e22c50`，列 stride 0x200。故 1／127／128／129／255 都原值保留，沒有二值化或除以 128。這是灰階值；彩色像素須先取平均，非零 RGB 也可能平均為 0。 |
| `0x4d87a0` | 只清空 256×256 short；不是 loader 或門檻 consumer。 |
| `0x4d8830` | visible array `0x018bb614` 以列 stride 0x202、欄 stride 2 取 bit0；未設定時把對應 collision 強制寫成 255；已設定時保留原值，不清回 0。 |
| `0x4d87e0` | 有界 collision getter。`0x4d8807` 讀 base-2 dword，`0x4d880e SAR EAX,0x10` 取得 signed short；越界回 0。不能單憑此低階越界回值宣稱越界可放置。 |
| `0x4d8240` | `collision >0` 時以紅色繪除錯覆蓋層。這只證明繪色，不能推廣成通用尋路門檻。 |
| `0x4da370` | 邊通行判斷：`0x4da386` 讀格值，`0x4da390 SAR`，`0x4da393 CMP EAX,ESI`、`0x4da395 JGE`，即 **collision >= 第5參數 T 時拒絕**。另有物件與另一格區／邊差檢查，通過 collision 並不保證整體通行。 |
| `0x4d8880` | 尋路主體，多個 caller 將第8參數傳給上述判斷；後段另用 `(第8參數 >> 8)+1` 作細化門檻。不能將單一固定值假裝成完整原生尋路。 |
| `0x4da5c0`、`0x4b6f30→0x4b8860` | 通行參數來自物件定義的 short 欄位（實際地址 `0x00c64918 + 定義索引*0x2a4`），前者也取群組成員最小值。沒有固定 128 的依據；本輪不擴大追該欄位的名稱、預設值或所有單位。 |
| `0x51f7d0` | 空地代價 getter：先做界限及 `0x49c3c0` 檢查，失敗回 0x7fff；成功取 `max(collision getter, 0x4d84d0 getter)`。後者讀另一 short 格區 `0x01e02c50`。 |
| `0x51f790` | 空地選點接受上述 max **<=5**。`0x51f7bd CMP EAX,0x5`，`0x51f7c0 JG` 拒絕；不是 128。由 `0x453810`、`0x53e5f0`、`0x5101f0` 等選點／位置調整路徑呼叫，不等於完整建築 footprint 驗證。 |

`0x51f7d0` 的 Ghidra 自動偽碼把延後清理堆疊的引數畫錯：看起來 `0x4d84d0` 有第三參數，`0x51dc60` 只收一個參數。已以 listing 排除：collision 回值在 `0x51f816 PUSH EAX` 暫留堆疊；第二 getter 回值在 `0x51f821 PUSH EAX`；`0x51f822 CALL 0x51dc60` 實際收到兩個值，後者取 max。不能照偽碼表面簽章解讀。

`0x4ae320` 未消費 collision，與這次門檻結論無關。

## 指定邊界值與 visible

以下「通過」僅指 collision／選點門檻，其他條件假設均通過；原生 signed short 的 0～255 不會變負數。

| 灰階值 | visible bit0=1 的有效值 | 尋路 T=1 | 尋路 T=128 | 尋路 T=129 | 空地選點 max<=5 |
|---|---|---|---|---|---|
| 0 | 0 | 通過 | 通過 | 通過 | 通過 |
| 1 | 1 | 拒絕 | 通過 | 通過 | 通過 |
| 5 | 5 | 拒絕 | 通過 | 通過 | 通過 |
| 6 | 6 | 拒絕 | 通過 | 通過 | 拒絕 |
| 127 | 127 | 拒絕 | 通過 | 通過 | 拒絕 |
| 128 | 128 | 拒絕 | 拒絕 | 通過 | 拒絕 |
| 129 | 129 | 拒絕 | 拒絕 | 拒絕 | 拒絕 |
| 255 | 255 | 拒絕 | 拒絕 | 拒絕 | 拒絕 |

T=128／129 是比較公式的邊界代入，**不是聲稱零售單位預設使用這些值**。對任一列，只要 visible bit0=0，有效值就是 255（visible 的其他 bits 不會取代 bit0），所以空地選點拒絕，所有 T<=255 的這項尋路檢查也拒絕。若 T>255，僅此比較不會拒絕；不能無條件宣稱 visible=0 永遠阻擋所有原生路徑。

## 本次產品修正與限制

- `placement_collision_blocked` 共用原生空地選點的 collision 必要條件 `>=6`；`SettlementTerrain::is_blocked` 改用它，修正原 `>128` 對 6～128 的錯誤放行，保留 0～5。
- `PassabilityGrid`／`navigation::diagnose` 保留非零拒絕，等同 collision 項採 T=1 的保守診斷。它未選定實際單位／群組門檻，不應標示為原生路徑保證；也不應為追求表面一致改成 `>128`。
- 聚落、診斷目前輸入沒有原生 visible 格區，也沒有 `0x4d84d0` 對應圖層。未宣稱完整重現原生選點，或已將原生 visible 快取整合進編輯器；不從高度臆造 visible、不改寫 collision。
- 本次未驗證建築 footprint、動態占用、GUI 或遊戲內可走／可建造。表格是本輪零售 EXE 靜態指令所支持的條件推導，測試是 Rust 產品回歸，兩者證據分開。

## 定向驗證

repo-local `re_workspace/rust-runtime`；target-dir 一律 `re_workspace/target-fix-collision`。只驗證 `against-rome-navigation`、`against-rome-editor-app`，沒有執行全 workspace。

- `cargo check --tests --locked --offline -p against-rome-navigation -p against-rome-editor-app --target-dir re_workspace/target-fix-collision`。
- `cargo test --locked --offline -p against-rome-navigation -p against-rome-editor-app --test collision_native_contract --target-dir re_workspace/target-fix-collision`：實際聚落生成在 127／128 必須與 255 同樣避開原選點；1／5 保留原選點；目前 session 診斷保守阻擋非零且保留 raw 值。
- 測試另涵蓋 0／6、原生 visible 強制 255 的載入後 fixture；fixture 不等於執行原生 visible loader。
- 最終 check／test／build 結果於驗證完成後更新本節。
