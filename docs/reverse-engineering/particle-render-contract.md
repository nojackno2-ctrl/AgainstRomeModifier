# 原遊戲 particle render contract：bounded 靜態證據

2026-10-12，Codex。只新增本報告、`tools/re/particle-render-contract-rs/`及獨立ignored outputs。未改handoff／Rustcore／Shared／host／既有tests，未commit、game/ISO writes、launch game、shared REA calls。Parent new renderer export尚未提供，以下consumer結論來自本輪EXE bytes，沒有冒充parent REA結論。

## 結論／驗證邊界

saved child **35 bytes**；runtime emitter **0x8e0=2272 bytes，child使用SoA**，不是連續stride34。`0x4DCFC0`具有child gate、position、圖像索引計算及draw dispatch；`0x4DD630`另一路呼叫`0x4C31D0`。sprite/frame/blend契約尚未閉合，不能用frame0或owner ALR替代未知粒子frame。

新Rust probe通過 **2330 frozen VA/bytes/instruction-boundary assertions**，另對既有native audit **538 instructions** cross-check；本crate **6tests pass**，EXE再讀相同。2330不是2330個獨立行為證明。whole executable-section exact-global-operand搜尋得到237 candidates，不證明CFG、runtime／tick／FPU／pixels。

## Evidence與來源

- EXE：唯讀 `C:\Program Files (x86)\Against Rome\Against_Rome.exe`；實測SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
- 既有 `re_workspace/native-particle-audit-20261010.json`：reader/reset bytes及74-map audit。1714 active／owner-valid／backlink-match、172 active child lights是 **2026-10-10留存資料**，本輪沒有重跑map audit。
- `particle-layout-20261010-evidence.json`：`ev_23a4300f3583e7d370134f353f8ef0f108055d03341200b3c9493e8ebe4d1eba`（415320/4152F0）、`ev_e80f61f8098704eccce0be87c89da559cdef48d1ffa5e59b178ecc22d91d2fb6`（48DD90/4DB800）。只讀既有export。
- `particle-creation-contract-20261010-evidence.json`：`ev_d986f416ac0b03933cc70db47c149c7357256cc200f56bed92ea3d08935edcab`（xrefs）、`ev_dc95c6478ede4ec4f0c03e8c101427b4a6ff3c7411e62d3cf1a6eec6a8392ab8`（4DB960）。另讀reviewed creation JSON與`probe_native_particle_pool.py`，未跑／新增Python。
- 最新輸出：`re_workspace/particle-render-contract-final-20261012-f.json`；包括VA/bytes、direct callees、mapping、xrefs及五個來源digests。重跑命令見 [README](../../tools/re/particle-render-contract-rs/README.md)；CLI對`src/sources.json`任一source hash不符即拒絕。

限定whole-code coverage：`0x401000` raw1929216 bytes／615283 candidates／151 invalid；`0x2AD5000` raw5784／2582 candidates／0 invalid。只擷取已知粒子memory displacement（含既知base−2 unaligned DWORD+SAR16），不全repo掃描／擴展其他globals。linear decode可能失步；register-materialized／indirect refs可能漏失，不得把零xref當全域不存在。

## Saved → runtime mapping

Parent 補驗（2026-10-12 Codex）：獨立 REA 分析確認 `0x4DCFC0` 完整 body 1617 bytes（Evidence `ev_af0d6b826e20f7c857c52868bf473845cfb3a5bbdcbbcef3fcde4a64787a6963`）與 `0x4B53C0` 完整 body 1643 bytes（`ev_4e9598580cc9863a719b1a43a7649f97da08ae5f0100413bb797f7120efa3c72`）；pseudocode 支持下述壽命／Y速度／XZ方向與 mode dispatch，但不證明 x87 rounding 或像素。Export：`re_workspace/particle-render-consumers-20261012-evidence.json`，snapshot：`particle-render-20261012.snapshot.json`，session 已關閉。Parent 重跑最終 crate `cargo test --offline --locked`：6 pass / 0 fail；沒有重試被 policy 拒絕的 CLI，也沒有執行遊戲。

emitter E的runtime base `B=0x12355D4+E*0x8e0`；saved record=`12+E*2252`，child c的saved base=`record+12+c*35`，c∈[0,63]。

| saved emitter offset | width | runtime offset | writer VA／bytes |
|---|---:|---|---|
| 0 | 2 | 0 | 48DEB4 `66894500` |
| 2 | 4 | 4 | 48DEE0 `894504` |
| 6 | 2 | 8 | 48DF07 `66894508` |
| 8 | 4 | 0x0c | 48DF1F `89450c` |

runtime+2..3／+10..11不是這四次read的資料。stride直接support：48DE5E `81c5e0080000` ADD EBP,0x8e0；48DEA8 `697c240ce0080000` IMUL...,0x8e0。

| saved child offset | width | runtime relative B | writer VA／bytes |
|---|---:|---|---|
| 0 | 1 | 0x1c+c | 48DF93 `88442f1c` |
| 1 | 4 | 0x5c+4c | 48DFC3 `89475c` |
| 5 | 4 | 0x15c+4c | 48DFEA `89875c010000` |
| 9 | 4 | 0x25c+4c | 48E014 `89875c020000` |
| 13 | 4 | 0x35c+4c | 48E03E `89875c030000` |
| 17 | 4 | 0x45c+4c | 48E068 `89875c040000` |
| 21 | 4 | 0x55c+4c | 48E092 `89875c050000` |
| 25 | 4 | 0x65c+4c | 48E0BC `89875c060000` |
| 29 | 2 | 0x75c+2c | 48E0EE `6689875c070000` |
| 31 | 2 | 0x7dc+2c | 48E119 `668987dc070000` |
| 33 | 2 | 0x85c+2c | 48DF69 `6689875c080000` |

reader序列是u8、7×u32、3×u16（最後reader在48E124），加上store widths／cursor +1/+4/+2證據，總和 **35**。runtime child arrays共2240 bytes，結束於+0x8db；沒有runtime child34。

saved pool尾四個columns各1024×4 bytes，415320／4152F0以emitter stride0x8e0寫+0x10/+0x14/+0x18/+0x8dc：48DE7F `83c010`、48E156 `83c014`、48E185 `83c018`、48E1B4 `05dc080000`。runtime accounting=header28+child2240+tail4=2272；saved record12+64×35=2252，差20為padding4及columns16。probe mapper用defined mask保留未知，不是完整runtime建構器。

## Render consumer：直接讀取與unknown

| 問題 | 直接support | 結論／限制 |
|---|---|---|
| emitter gate | 4DCFE2 `6683bfd455230100`、4DD64C CMP word[B],0 | 非零才續行，不能只接受1；不是全部visibility。 |
| count／ordinal | 4DD0B7 `8b87d8552301`、4DD176/1F5；4DD6C7 | count控制ordinal。definition[80EFEC]>0則從1開始，否則0（4DD15C/167/16B、4DD72A/735/737）；不可固定draw64。 |
| child gate | 4DD19B `80bff055230100`、4DD273 `80b8f055230100`、4DD76D `80bdf055230100` | saved child0非零才加入／消費，仍受definition／asset／callee gates。 |
| position | 4DD2C1 `8b8330582301`、4DD2CB `8b8330572301`、4DD2D5 `8b8330562301`，各SAR16→491B50；4DD495/4AD/4C5再FILD→draw | saved child1/5/9直接作native position components。signed SAR16不能換成float除65536；display axes／地形政策留整合層。 |
| type／asset | 4DCFF4 owner→objects→type*0x2a4；4DD02A/044/05A type refs→4DDE40/4DE540/4DE9E0；4DD09C讀824FD4→4AFD80；4DD0FE與4B53EE讀C648CA | asset/type來自object/definition鏈，不是child29／31。validator語意／完整asset identity待parent。 |
| frame/index | 4DD5EF `2b93305c2301` SUB child+0x65c；4DD5F5 IMUL；4DD601/608 IDIV；4DD60A `89d3` remainder→EBX；4DD4DD `53` PUSH index；4DD4FF `e8bc7efdff` CALL4B53C0 | child25參與一條index算式；還有方向分支。尚非editor frame resolver。 |
| child29/31 | 4DC685 `66ff88305d2301` DEC child29；4DCF0B `66ff88b05d2301` DEC child31；4DCEF3/4DCF12 unaligned DWORD+SAR16讀child31 | updater counters的直接support。上述兩renderer body沒有直接讀它們；不能命名sprite/type/frame，negative僅限此coverage。 |
| light link | 4DB8E4 `8b862e5e2301` +SAR16→49F700；4DCF2A同讀／4DCF34 release／4DCF43寫ffff | child33由reader/create/reset/consumer共同支持為light link，不是sprite/frame。 |
| blend／mode | 4DD0CE/0DF gfxtype mode；99另取type；4DD4E8/4EC傳4B53C0；4B54B3/BC/C5、4B58DF/E8、4B597A/5980/5989分流 | 可保留raw mode及dispatch trace，leaf alpha/additive/depth尚未知；不能全部設additive。 |
| size | 4AF222 `8b4110`／4AF252 `8b4114`讀asset+0x10/+0x14；4CA102 `8b532c`／4CA124 `8b5330`讀resource欄並shift | 前兩metadata用於index，不可憑offset叫pixel size。尺寸/anchor/variant解析尚需lower reader，沒有已驗saved child size欄。 |

4DCFC0 body最後4DD60C jump，exclusive4DD611；4DD630 body最後4DD850 jump，exclusive4DD855：bytes/control-flow尾端觀察，函式ownership/callers待parent。探索b/d JSON曾穿padding或由partial instruction開始；**本報告只用e/f consumer windows**，不把4DD611後padding或4DD855後table當code。

### Index算式、方向分支與callee

普通分支：A=`4AF200(asset)`，L=`definition[80EFD4]`，C=`child runtime+0x65c`，D=`type[C648C0]`，index為 `((L-C)*A / D) rem A`。4DD5EF–4DD60C支持native signed SUB/IMUL/CDQ/IDIV順序；32-bit overflow與IDIV例外不能悄改無限精度。A/D為0、negative/out-of-range、asset未知時API應回unknown/error。

另一分支由definition[80EFFC]選入（4DD2EB），用child17 SAR16、區間係數、419CC0產生index；metadata B=`4AF230(asset)`>1時另用child13/21 SAR16→49BE90方向角→419C80量化，乘A後加進index。不能只讀child25就宣稱完整frame；x87 rounding/FPU未驗，也不能將區間係數當幾何scale。A/B資源metadata正式語意尚待lower reader，這裡只有offset與運算用途。

4B53C0在4B53CF `8b5c2440`接caller index；**只有index=−1**才進owner anim fallback（4B5441 `83fbff`、4B5454 objects anim、4B546A anim值）。下一Rust API不以owner ALR或4B5551 native zero fallback替代未知粒子frame。

完整4B53C0 window `[4B53C0,4B5A2B)` direct callees：419CC0、49A490、4AEBC0、4BD8D0、4CA010、4CA340、4CA740、4CAB80、4CAF80、4CB380、4CBCC0。0/1/2/3/4/5/6/7/8/9/49 mode可定位，不能僅憑分支名判定blend。

4CA010只看前0x160 bytes：4CA06D index與resource+4作unsigned bounds gate；4CA076/07D限制variant；4CA0CA→4E48F0；4CA0D2檢查format8。4E48F0／其他leaf未閉合，故實際asset frame/mask/alpha未完成。

4DD630另有secondary index[0,2000)、相關type欄>0 gates，child13/21→49BE90，child1/9與definition欄→4C31D0（4DD848 `e88359feff`）。4C31D0只看前0x300 bytes數值／geometry準備，不一律視為billboard sprite。

## 驗證／失敗／next integration API

### Parent 後續 leaf 證據（2026-10-12）

`re_workspace/particle-leaf-20261012-evidence.json`／同名 snapshot 已保存並關閉 session。`4CA010` 完整 body 793 bytes，Evidence `ev_7b4c1a2516573e8449c8d02a2ae0837c47f8afe2bad42e5f0fb670dde1ce51a5`：先驗 frame unsigned bound，限制 palette variant，呼叫 `4E48F0`，僅處理 format8；由 `4E49C0` 取得列段，寫入 palette DWORD。其中一種列段略過 index0，另一種直接覆寫。這條 leaf 本身沒有 destination alpha 混合；不能推廣至所有 mode，亦未驗像素等價。

`4CA340` 完整 body 1020 bytes，Evidence `ev_5693cabb1da55696707699108b5bcae39fcbf85e11b5d2c13c42964602c1c375`：處理兩個 palette variant 與最多256的插值參數，`40E4D0`／`40E5C0` 建立工作調色盤，再交 `40E750`／`40E7B0` 寫列。RGB 數值、FPU 與這四個 helper 尚未完整驗證；不可據此指定 additive 或 GPU blend。原生縮放使用 shift 與單點步進，編輯器既有插值濾波不自動視為等價。

`cargo build --offline`產生lock成功；最終`cargo build/test --offline --locked`成功，6pass/0fail。先前成功`cargo run --offline --locked`產生f.json＝2330assertions／538reference／237candidates／source unchanged；之後整理CLI並直接使用serde::Serialize寫JSON，最新版CLI重跑被Windows application control `os error 4551`拒絕，**never executed**，g.json不存在，也沒有完成CLI覆寫拒絕檢查。核心probe/frozen數據未因CLI整理改動；不得稱最新版CLI已執行。未改安全policy／重試繞過。target僅`re_workspace/particle-render-contract-target`；無dotnet或existing core tests。初次cargo fmt因本機stable缺rustfmt失敗；parent後續補裝workspace-local rustfmt並fmt成功，格式化後6tests仍pass。初次任意window截斷被拒絕；修為最多15-byte唯讀lookahead，只輸出完整落在window指令，並將authoritative windows縮至有效body，沒放寬hash/bytes。

已實作 `unpack_record(&[u8]) -> Result<MappedSavedFields>`，2252→2272 sparse mapping、defined=false不能當native defaults；六測試覆蓋64ordinals不重疊／mapping／wrong lengths／malformed PE／wrong EXE hash／bytes-VA-boundary mutations。

以下是 **下一階段API提案，未接core/host**：

```rust
parse_particle_snapshot(decoded: &[u8]) -> Result<ParticleSnapshot>;
// strict header/version/capacity/length，保留2252 records及四columns。
snapshot.child(emitter: u16, ordinal: u8) -> Result<OpaqueChild35>;
resolve_particle_render_inputs(snapshot: &ParticleSnapshot,
    definitions: &VerifiedParticleDefinitions,
    assets: &VerifiedSpriteMetadata) -> Result<ParticleRenderResolution>;
// Resolved(ParticleDrawInputs) / Unknown { reason, support }
// frame/index: Option<VerifiedFrameIndex>；mode: raw；position: fixed_i32。
```

保留完整35bytes／64children，不因inactive或count小刪links。snapshot與native simulation分離；signed SAR16與display conversion分層。count超64、ordinal超界拒絕，nonzero flags保留raw；缺definition/asset、zero division或unknown route不產生draw，不猜frame0，不用owner ALR補粒子frame。

Parent下一步：export4DCFC0/4DD630函式與callers、4B53C0 index/mode參數、4AF200/230metadata、4E48F0 reader及4CA010/340/740/CAB80/CAF80/CB380/CBCC0 leaf的frame bounds／size／blend。new JSON先核對subject SHA、status／coverage，再擴充frozen assertions，才決定Rustcore DTO/frame resolver。tick/FPU/ordering/原遊戲pixels須另驗；本static任務沒有完成它們。
