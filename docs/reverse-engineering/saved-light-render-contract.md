# Saved light → runtime → render 靜態合約

2026-10-11，Codex。僅新增指定 probe、本報告及 `re_workspace` 唯一結果檔。已讀 AGENTS、AI_HANDOFF、分支／status／diff／近期 commits；現有整合修改保持原狀，handoff 由 parent 更新。本次未 commit、build/test、啟動遊戲、寫 game/ISO 或使用 shared REA session。

## 結論與驗證

`0x49FFE0` **不直接讀任何 saved radius**：`0x4A00B0` 比較 runtime **+0x3C**（`0x7F9F2C`）的半徑平方；`0x4A00C3` 乘 runtime **+0x40**（`0x7F9F30`）的平方倒數。兩者由 `0x49F810` 內 `49F8E2..49F904` 使用 **+0x1C** 計算。saved column 7／snapshot offset 28 → +0x1C 是 effective radius；column 8／snapshot offset 32 → +0x20 是更新計算使用的 baseline radius。這是指令角色命名，不是還原原始 C 結構名稱。

實際執行指定 Python，exit 0，**746 個唯一 instruction assertions 通過**（原 121 個，淨增 625 個）；EXE 讀取前後相同，SHA256 綁定：

`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`

結果：[`saved-light-render-strengthened-21a4d1c02ba04f3b8e368280af9209fe.json`](../../re_workspace/saved-light-render-strengthened-21a4d1c02ba04f3b8e368280af9209fe.json)。包含逐指令 bytes、VA、operands、9 個限定視窗的 bytes hash／反組譯、16 欄映射、probe hash、工具版本及重用 evidence hash/IDs。這是純靜態驗證，**不是 runtime/tick/FPU 驗收**。

重跑（每次使用新 GUID；拒絕既有檔；output 必須直接位於本 repo `re_workspace`）：

```powershell
$env:PYTHONPATH="$env:TEMP\arm-re-python"
& 'C:\Users\nojac\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe' -B `
  tools/re/probe_saved_light_render_contract.py `
  'C:\Program Files (x86)\Against Rome\Against_Rome.exe' `
  ('re_workspace/saved-light-render-contract-' + [guid]::NewGuid().ToString('N') + '.json')
```

## 證據來源與範圍

優先重用下列現存檔；probe 依明確 schema 驗其 source fingerprint 並保存 SHA256，未重新開 REA 或遍掃地圖／全部檔案：

- `re_workspace/native-light-pool-20261010.json`：reader/create/reset 原始 Capstone 證據與既有 saved pool layout。
- `native-light-pool-20261010-evidence.json`：reader `ev_7b55fde313501b05d35a118cf1267f7138228a0abeb6d16460f6638eab456af8`；create `ev_7f08c2ec7fbec93f449c7e7364c3bedfeac8307fd949591753f358e7a8000a2a`。
- `direct-light-runtime-update-20261010-evidence.json`：updater body `49F810..49FA79` 的 `ev_013f3fce256458d0dce96bac935d3cbecdfb92a5e1b5bdb15584cb3e741011f6`；renderer body `ev_2b4e4388990bafab21587cbe6b7fe4ccb8e075b5e200de739e7e6825ea70c68f`；spatial builder `ev_835b7601f59022b7d316ac3159639e4539437e4fc235d16be1d4bfd77bc86e22`。

- `direct-light-transform-final-20261010.json`：重用既有 symbolic XYZ、callback fields 67/70/71 與 owner position getter 證據；本 probe 將其中 `49F8FA..49F9D7` 的每一條指令 bytes／mnemonic／operands 對照新結果，並要求地址集合完整一致。未重新執行原 probe 的地圖掃描或 symbolic evaluator。

schema 明確區分 static result 與 REA bundle：static result 必須含 `exe_sha256`、非空 instructions、正數 instruction_assertions，並驗欄寬／transform metadata；bundle 必須含非空 records，每筆必須含 subject.digest.sha256、evidence_id、operation、parameters。合法 bundle 沒有 root `exe_sha256` 仍可由每筆 subject hash 綁定；若 root key 存在就必須吻合。不再用空 records 或預設 EXE hash 默默接受缺件。錯誤含檔名、record index 與 `evidence schema` 原因，寫結果之前 fail。

已在記憶體中實際驗證 9 個 schema 情境：合法無 root hash bundle 接受；缺／空 records、缺 subject、缺／錯 subject hash、缺 parameters、錯 optional root hash、static 缺 root hash 均明確拒絕。診斷：[`saved-light-render-schema-648792c078e54ad28148c59eea4ce5ac.json`](../../re_workspace/saved-light-render-schema-648792c078e54ad28148c59eea4ce5ac.json)。同時重新核對 EXE、probe 與四個重用 evidence hashes，及 746 個 assertion 地址無重複；未執行專案 build/test。

在此基礎上由 pefile 映射 VA，Capstone 重新解析 installed EXE 的限定 reader／validator／create／reset／update／spatial builders／renderer。不是全 EXE writer inventory，也不宣稱已排除其他 aliases、間接寫入或 caller。

## Saved pool 與 detached snapshot 映射

版本 1，capacity 1024，decoded payload 57352 bytes，header 8 bytes。磁碟是平行欄位；`NativeLightSnapshot.Fields` 是 Capture 將各欄串接後的 56 bytes，**不是 runtime memcpy**。第 c 欄槽 s 的 payload offset：`8 + capacity * sum(widths[:c]) + s * width[c]`。runtime base `0x7F9EF0`，stride **0x44**。

| column | snapshot offset | width／讀值 | runtime offset | 已證實角色 |
| --- | --- | --- | --- | --- |
| 0 | 0 | u32 raw | +00 | 非零為 active |
| 1 | 4 | float32 | +04 | X |
| 2 | 8 | float32 | +08 | Y |
| 3 | 12 | float32 | +0C | Z |
| 4 | 16 | float32 | +10 | render R |
| 5 | 20 | float32 | +14 | render G |
| 6 | 24 | float32 | +18 | render B |
| 7 | 28 | float32 | +1C | effective radius |
| 8 | 32 | float32 | +20 | baseline radius |
| 9 | 36 | 16-bit raw | +24 | 更新 type（以 unsigned short 分支） |
| 10 | 38 | float32 | +28 | type parameter |
| 11 | 42 | u32 raw | +2C | 累加欄；type 2 以 signed FILD 解讀 |
| 12 | 46 | u32 raw | +30 | special-fx raw；本次未追 consumer |
| 13 | 50 | 16-bit signed link | +34 | owner |
| 14 | 52 | 16-bit signed link | +36 | parent |
| 15 | 54 | 16-bit signed index | +38 | definition |
| 無 | 無 | float32 衍生 | +3C | effective radius² |
| 無 | 無 | float32 衍生 | +40 | 1／effective radius² |

probe 對全部 16 欄分別 assert reader call、`add edi,0x44`、實際 store；本輪再逐指令 assert 整個限定 `48AB70` reader（1350 bytes），包括每欄 loop 的初值／界限計算、cmp／branch。這確認目的地 record stride 與迴圈，不等於 helper 消耗來源 byte width 的新證明。radius 關鍵：column 7 call `48AE18`／store `48AE3F [edi-28h]`；column 8 call `48AE68`／store `48AE93 [edi-24h]`。store 前 edi 已增加 44h，所以目的地分別 +1C／+20。

RGB 對應由 reader store 與 renderer `4A00CD/4A00F5/4A0117` 的三通道 loads 共同確認。沒有證據允許 loader 把 saved RGB 夾在 0..1，或以 definition RGB 取代它。reader call 指向 `414F10`，但本 probe **沒有讀／assert reader helper body**，現存本輪重用證據亦不足以直接確認 helper 的 FLD/FSTP／讀入 byte width；不將此說成新證明，也不保證非有限值／NaN payload 的 bit preservation。saved 欄寬來自既有 pool layout，與 reader stores／各欄 loops 的目的地 stride 對照。

## Radius 的寫入者與使用者

1. `49F500` 建立：`49F590` 從同一參數載入 ebx；`49F594`、`49F59B` 分別寫 +1C、+20，兩 radius **建立時相同**。不能推廣成 saved snapshot 永遠相同。
2. `48AB70` 讀檔：兩 radius 分別載入，沒有把 column 8 覆蓋回 column 7。
3. `49F700` reset：`49F789/49F790` 清 +1C／+20；未在這個 reset body 清 +3C／+40。inactive slot 的衍生 cache 不能直接拿來照明。
4. `49F810` 更新：`49F82E/49F835` 僅讓 active 非零的槽進入更新，`49FA23` 將 caller 傳入的整數加到 +2C，單位／呼叫頻率未驗證。
   - type 1：`49F864..49F898` 讀 baseline → `5C5AF2/FISTP` → 加 1 作 IDIV divisor；`49B6B0` 的回傳值經 **signed IDIV remainder**，轉浮點後乘 type parameter，再加 baseline，寫 effective。符號式：`effective = baseline + remainder * parameter`。未分析 RNG helper、輸出分布或 divisor exceptional cases，不称均勻亂數，也不在本次斷言轉換／FPU 模式等價。
   - type 2：`49F9E4..49FA0D` 以 signed +2C 比較 parameter；小於時寫 `effective = (1 - age / parameter) * baseline`。大於等於時 `49FA13` 呼叫 reset。時間單位、負值／NaN／overflow 行為不以普通 C# 公式替代原生。
   - type 0 或其他值：此 body 的 type 分支保留既有 effective，不強制改成 baseline。
5. `49F8E2..49F904`：active gate 後讀 effective，平方並 FSTP 到 +3C，再以 `1 / stored_squared` 存 +40；後者使用已落地 float32 的平方。兩衍生欄未出現在 16 saved 欄中。
6. `49FA90` spatial builder：`49FBE2/49FBEA/49FBF2/49FBFA` 以 effective 對 X/Z 加減形成範圍；`49FD30` 的篩選也比較 +3C。baseline 沒有在這兩個限定 body 充當距離半徑。

7. **owner-linked XYZ 更新**：`49F8FA` 從跨欄 dword `+32` 讀取，`49F900 SAR 16` 取得 signed owner `+34`；`49F903..49F914` 呼叫 `4AB7B0`，回傳零則跳往下一槽，保留原 XYZ。非零時 `49F91A..49F92D` 取 `4BD320` 的 position，`49F930..49F939` 取 `4BD5F0` 的 rotation；兩 getter 的內部意義沿用 existing direct-light-transform evidence，非本輪新讀 body。
   - `49F93E..49F955` 取 owner record 的 definition 並呼叫 `4AFD80`。其回傳零會跳到 `49F9CB`，以 owner position 直接寫 XYZ；這不是保留 saved XYZ 的分支。
   - gate 非零時 `49F957..49F9C8` 使用 definition `lihei`、`lightrad`、`lightang` 的條件 transform。既有 symbolic 證據：`theta = (rotation - 90 - lightang) * float32(1/360) * 2 * float32(pi)`；`X'=ownerX+sin(theta)*lightrad`、`Y'=ownerY+lihei`、`Z'=ownerZ+cos(theta)*lightrad`。本輪完整指令對照不等於 x87 實機舍入驗收。
   - `49F9CB..49F9D7` 按 stack 變化依序傳 Z／Y／X／slot 到 `49F660`；新讀範圍嚴格限 **`49F660..49F6AF`（0x50 bytes）**，不讀相鄰 helper。setter 呼叫 `49F4D0` 檢查槽有效；成功才由 `slot*17*4` 定址，在 `49F68A/49F695/49F6A0` 分別寫 runtime `+04/+08/+0C`。整段逐指令 assert，不能再把 saved XYZ 永遠固定說成原生正確。

上述 updater 沒有改寫 RGB。這只證明該 body 的行為，不是全程 RGB 恆定或無其他 writer 的證明。objdef 的 `lightrad` 是先前 direct-light 位置轉換的偏移參數，不能因名稱相似用來替代 pool radius。

## Render active、RGB、距離與邊界

`49FFE0` 使用的列表路徑只沿用既有 renderer body 證據；本輪未完整追蹤所有 list producers／callers，不能把列表名稱或所有 active membership 說成全域已證明。`49F810` 的 active 非零 gate 與 `49F83F` capacity 1024、`49F845` signed loop branch 已 assert；reset 後 `49F89E/A5` 再檢查 active。`49F8B3/BB` 將槽 index 寫入 `0x1EFEB06` 並更新計數 `0x1EFDB00`；`49F8C0..C9`、`49FA51..5A`、`49FA66..6F` 比較 RGB 與 globals `771870/74/78`，普通有序數值下任一通道較大才在 `49F8D5/DD` 寫另一槽列表 `0x1EFDB06`／計數 `0x1EFDAFC`。本輪 **只稱 RGB 比較 globals，ambient 語義／來源未知**。`49FBB2..49FBCD` 已 assert spatial builder 消費後者的計數與 signed slot；`49FBD0/D8` 明確載入 X/Z，再用 effective radius 作四個加減界限。不可把這兩個列表擅自同名為 active/render 全域列表，也不宣稱 RGB gate 的所有 native 系統用途。

`49F9F4/F6` 的 FNSTSW／SAHF 及 `49F878..49F884` divisor／sign extension／EDX remainder store 已 assert。render `4A00B6/B8` flags、`4A00CD..4A0132` 三通道 compare／flags／條件替換、`4A0142..4A0170` 三組 signed cmp／JLE／1.0 store 都已逐指令 assert。

`4A006C..4A00AC` 使用 runtime +04／+08／+0C 與本函式第 1／2／3 float 參數做差及平方和，得到 `(lightX-queryX)²+(lightY-queryY)²+(lightZ-queryZ)²`。**Y 確實參與距離**；不是 X/Z 平面或 screen distance。`4A00B0/B9` 對 +3C 做 FCOMP，有限普通數值下 d² ≥ r² 跳過；`4A00C3..CB` 得 `1-d²*inverse_squared`。三通道乘 saved RGB，候選較大才替換當前值（逐通道 max，不累加）。末尾 `4A0142..4A0170` 以 signed integer compare 比較 float bit pattern 與 `0x3F800000`，普通正有限值時表現為上限 1；不將它說成通用的 IEEE/NaN `Math.Clamp`。

本次證明的是 caller 傳入的三個 native scalar 與 saved light XYZ 比較。沒有重新審所有 terrain/sprite caller 或 shader coordinate 變換，**不能聲稱 preview 的 tile*256、HeightScale／heightmapstep／relief 換算後 Y 與此距離空間等價**。原生 read→update→spatial rebuild→render 的執行順序與 tick 頻率也沒有 live 驗收。

## 給 parent 的精確整合下一步

1. 在 parent 擁有的 `NativeLightSnapshot` 加 immutable typed getters：`ActiveRaw` offset 0 u32、`Color` 16/20/24 float32、`EffectiveRadius` 28、`BaselineRadius` 32、`TypeRaw` 36 u16、`TypeParameter` 38 float32、`AgeRaw` 42 u32（另需 signed 解讀時用明確 accessor）、`SpecialFxRaw` 46 u32、`Definition` 54 i16；保留現有 XYZ／owner／parent 與完整 raw bytes。不要將 raw 陣列公開為可變引用，不依 definition 重建 saved 欄。
2. 建獨立 saved-light preview instance，在 **saved snapshot preview** 以 **saved effective radius** 呈現保存時狀態；type 1/2 也不能擅自改用 baseline 當 render radius（那只是近似，不接受作目標實作）。saved effective 是已保存狀態，並非未來 tick 的 effective；baseline 僅供 metadata／另驗的 native update model。不要把兩者命名成 radius／radiusSquared，也不要在 snapshot 內假裝儲存 +3C/+40。預覽可明確排除非有限 RGB/XYZ/radius 及非正 radius，保留原 bytes 並提供診斷；這是 preview policy，不能反寫 saved pool 或宣稱原生 policy。
3. `SceneLightingContext` 增加接收已捕捉 saved instances 的路徑，從 map-owned detached store／family projection 提供實際池資料；有 saved family 證據的物件不要再合成其 APT/lidef 光，避免 double-count。其他 synthetic fallback 必須顯式區分。不要因 lightdef inactive 或不可讀而抹除已 active 的 saved light；本次 updater 未以 definition active gate saved slot。
4. 共用 family member/effect 投影：retained instance 用 poolslot，pending copy 用 root UID＋targetScenarioId＋source poolslot／child ordinal 分辨，保留同座標的不同實例。位移使用既有已驗的 native family FixedDelta／XYZ 合約，先保持 saved light native Y；**saved snapshot preview** 不再加 lihei、重採 terrain 或再次套 lightrad，否則會 double-transform 已保存位置。若產品要呈現 **post-update/tick runtime 的 dynamic mode**，則必須另外使用當時 owner position／rotation／definition gate 重算 transform，不能因 snapshot 模式保留 XYZ 而把動態原生也說成固定；需另驗 tick 順序／頻率、type 1/2 radius update、FPU/RNG。此 identity 是產品整合設計，並非 EXE 中發現同名 key。
5. 照明距離先保留 native XYZ；parent 需追 terrain/sprite 的 query XYZ 來源並明確設計 native-space 距離與 preview-space 顯示換算。sprite marker 高度畫對，不足以證明 shader 距離正確。不要在同一 vector 內混用 native X/Z 與縮放後 preview Y。
6. 後續 parent 驗證應覆蓋 unequal radii 的 snapshot／copy／reload、active raw 非 1、RGB >1 原樣讀取、saved 與 synthetic suppression、不同 instance 同座標、含 Y 的距離及 HeightScale 改變。這些是後續工作，本次依限制**未新增或執行 build/test**。只有另行取得 runtime/tick/FPU 證據後才能接 native type animation／聲稱原生視覺等價。

此輪解決了 renderer radius 來源與兩 saved radius 的靜態用途；完整 writer 覆蓋、native tick／RNG／浮點模式、所有距離 caller／shader 等價以及遊戲內整合仍未知。

## Review 77ceef4b2564 修正與未證明項

已補 owner 條件 transform／49F660 XYZ stores、type 2 flags、type 1 signed remainder store、capacity loop、spatial X/Z loads、render compare/max/clamp；修正 snapshot 與 post-tick 分界、double-transform、baseline 近似及 ambient/list 命名；schema 不接受缺 records／subject hash，也不強迫合法 bundle 加 root EXE key。此次實際重跑 exit 0、746 assertions、EXE 與四個 evidence 讀取前後未變。

首次增補執行因我誤設 owner instruction 數量而 fail，未產生結果；已改為新斷言與既有 evidence 的完整地址集合比對，第二次通過。仍未新證明 reader helper body/source read width、owner/definition validity helper body、getter body、symbolic evaluator 舍入、RGB globals 的 ambient 語義、全部列表來源與用途、所有 render query caller、native tick/RNG/FPU/pixel equivalence。整合 UI、game／ISO、build/test／commit／handoff 寫入皆未執行。
