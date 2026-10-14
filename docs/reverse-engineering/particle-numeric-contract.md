# Particle numeric contract：bounded Rust 靜態證據

Parent 補驗（2026-10-12 Codex）：最終 crate `cargo test --locked --offline` 8 pass / 0 fail；實際 CLI 新輸出 `re_workspace/particle-numeric-parent-20261012.json`，700 assertions、EXE 前後不變。這些是 source bytes／數值位元與探針執行驗證，不是原遊戲 FPU 或像素驗證。

2026-10-12 Codex。只新增 `tools/re/particle-numeric-contract-rs/`、本報告與 ignored `re_workspace` output/target。未更動 core、C# host、既有 tests、handoff 或 Git index/history；未啟動遊戲、未寫 game/ISO、未呼叫 REA。metadata worker 的 `alr_metadata.rs`／`lib.rs`／tests／report ownership 保留。

## 來源、界線及驗證

- installed `C:\Program Files (x86)\Against Rome\Against_Rome.exe` SHA256：`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
- parent export `re_workspace/particle-direction-palette-20261012-evidence.json`：22 records；SHA256 `c4e18e979d184d81ce33551ee603a228e5c793482c7aea860306287f1077ed4f`。解析完整 export，只選六筆 `analyze_function`；每筆完整 record 經 Rust canonical JSON 序列化 hash，沒有印出完整 ledger。
- `tools/re/particle-numeric-contract-rs/src/frozen.json` SHA256：`4733461f18f3e7018fd9aa0bcd14b6130f60e06ebc71a1029923e9ee61ea46c8`。包括完整 body raw hex／decoded assembly／parent assembly／pseudocode／evidence IDs／complete-record hashes。
- actual CLI 新 output：`re_workspace/particle-numeric-contract-20261012.json`，SHA256 `8acae76c11f1b9450e8ea740ab2da95038a92bf9549b6c07cd6e1e06c372e97e`。`create_new(true)`；EXE 與 export 前後 byte-identical，寫出前 EXE 再核對。
- target：`re_workspace/particle-numeric-target`，已確認被 Git ignore。重用 Rust `particle-render-contract` 的 PE／hash／instruction verifier，不呼叫它的 `run`／全 executable-section xrefs。
- `cargo fmt` 成功；`cargo test --offline --locked`：8 passed / 0 failed / 0 ignored；主 CLI `cargo run --offline --locked --bin particle-numeric-contract ...` exit 0。700 instruction assertions＝693 body + 7 bounded CRT；7 finite constants、9 declared operand widths、2 default pointer slots + 2×64-byte targets。沒有 Windows policy rejection，沒有 variant executable workaround。
- tests：raw-byte／VA／hash／size mutations、instruction mutation/partial tail、mapped bounds/overflow、f32/f64 exact widths及 nonfinite rejection、decoder operand widths、create_new 保護既有 output、installed frozen differential。首次 test 7 pass / 1 fail，原因是 JSON f64 parse 的一個 ULP 差異；啟用 `serde_json/float_roundtrip` 後完整 8 tests 通過。未放寬 constant equality。

六個連續 body 的 inclusive end 均來自 selected normalized record，並以 EXE decode 確認每條指令起點及完整邊界：

| VA / inclusive end | bytes | instructions | raw body SHA256 |
|---|---:|---:|---|
| 419c80 / 419cb6 | 55 | 13 | `b2bfaca9b9f489428e8170665b5e78c543bf51780d1590f7985b8a754de2331a` |
| 419cc0 / 419ce2 | 35 | 8 | `70430e2cb163325a45b71da21ab28548b3811d4cb64abab6099be37d4890f65b` |
| 49be90 / 49bf0d | 126 | 38 | `b8b848ea24360f67409daba635150bd1383470bdf208031d7b393971a48ff9cd` |
| 40e4d0 / 40e5a8 | 217 | 87 | `844017df77da743611aea62bcafdfa964cdaaa48f091dfed8a6fcaee9d64212e` |
| 40e5c0 / 40e748 | 393 | 121 | `4ceb01bd7a6ca461b0017ebab22233064e1600ad5693a7ccc758ab67f6c6b6b6` |
| 4dcfc0 / 4dd610 | 1617 | 426 | `685de7bb88f41bd8c42f5d568351ed3683ab4ddaec3e05927006a8ca3f783b1f` |

## 常數：先確認 operand width，再 decode

以下均 little-endian 且 finite。f32 值顯示為精確升至 f64 的數值，沒有把相鄰四 bytes 當成 double：

| VA | bytes/type | raw hex | finite value | raw SHA256 |
|---|---|---|---:|---|
| 612c70 | 4 / f32 | `fdffff3e` | 0.49999991059303284 | `409c4f6f2c09cb8c76bce28bc3eb403198334ecaaeaff12d37da5c6751c615d4` |
| 5f4ba0 | 8 / f64 | `0000000000806640` | 180 | `fab4c4415b50601e97f6b89f2f92021b61bf2dd3bdb9d1f5ffb6e8de92046e01` |
| 5f4b98 | 8 / f64 | `e3a9471adca54c40` | 57.2957795 | `a926e2cb4b91157641a4cff1806eb1df711d79920a28348fd5114720e8887076` |
| 5f6826 | 4 / f32 | `610b363b` | 0.0027777778450399637 | `3d47ff31041026af7fd4bd62824516df8ea55e06a2371e53e3eb9bdba208a0b0` |
| 5f682a | 4 / f32 | `0000b443` | 360 | `82a72eeef4f720c75f627f5ccf9f2963cceb6210f66318ec6362722e29a564f9` |
| 5f682e | 4 / f32 | `000034c2` | -45 | `4be682e8be02506cb8c55e1a850db407338bbf4613dad23ca6d62e70780bb187` |
| 5f6832 | 4 / f32 | `0000b4c3` | -360 | `172e0b30dbefc8bc91184fb1bf0b47029cf68e65396f991041ca5a26ef466a2a` |

width 證據：419c9a dword OR／419cd1 float FSUB＝4；49bef8 double FMUL／49befe double FSUBR＝8；4dd41f FLD、4dd42f/44a/462 FADD、4dd47c FMUL＝float 4。九個 operand records 全保存在 frozen/output，decoder `memory_size().size()` 必須相符。

## FPU conversion 419cc0 / 419c80

419cc0 完整 raw 35 bytes：`8b153cd6640042d944240489153cd66400d825702c6100db1d50d66400a150d66400c3`。指令序列為 MOV/INC counter、FLD `[ESP+4]` f32、MOV counter、FSUB `[612c70]` f32、FISTP `[64d650]` signed dword、MOV EAX、RET。關鍵 raw：419cd1 `d825702c6100`；419cd7 `db1d50d66400`。這裡減的是上述 bias，不是恰好 0.5。

419c80 先從輸入 raw bits 取 sign `& 0x80000000`，OR bias bits，存 scratch 再 FSUB；FISTP 同樣輸出 signed dword。419c95 `2500000080`、419c9a `0b05702c6100`、419ca5 `d82550d66400`、419cab `db1d50d66400`。兩函式改寫 global counters/scratch，不能假設可重入。

decompiler 的 `ROUND` 不是 round-to-nearest、floor 或 truncation policy 證據。兩 body 都沒有 FLDCW；x87 control word RC/PC、precision、exception masks、invalid conversion、runtime scratch races 未知。不提供 Rust 浮點近似 conversion API。

## Angle 49be90 與 renderer direction

只對 finite inputs 的 axis 情形有下列精確分支結果。令 `(a,b)` 為函式兩個 f32 arguments；`TEST bits,0x7fffffff` 將 +0/-0 都視為零，return 是 EAX 裡的 f32 bits：

| a | b | 結果 / bits |
|---|---|---|
| ±0 | 正數 | 90 / 42b40000 |
| ±0 | 負數 | 270 / 43870000 |
| ±0 | ±0 | +0 / 00000000，四種零向量均同 |
| 正數 | ±0 | 180 / 43340000 |
| 負數 | ±0 | +0 / 00000000 |

49bea4/49bed2 的 FLDZ 與 FCOMP、FNSTSW/SAHF 後分別使用 JBE/JC 與 JNC。非零兩 arguments：FLD a、FLD b、CALL 5c5f4b，FMUL f64 57.2957795，FSUBR f64 180，最後 FSTP f32。NaN、infinity、exception／unordered 情形不提升為已證 finite contract。

49be90 完整126-byte raw：`83ec0431d28b4c2408891424f7c1ffffff7f7524d9eed85c240cdfe09e7607c704240000874372078b042483c404c3c704240000b442ebf0f744240cffffff7f7519d9eed85c2408dfe09e73dbc70424000034438b042483c404c3d9442408d944240ce853a01200dc0d984b5f00dc2da04b5f00d91c248b042483c404c3`。

因 angle assembly 留有 CRT numerical boundary，唯一追加 callee 為 `5c5f4b`，raw 讀取上限128 bytes，hash `fbe22f9da529fb5ee3844a7e2b4ce05547053ca11d6492bb40ffd803000bb402`。僅解碼 21 bytes / 7 instructions 至 5c5f5f RET；其餘107 bytes 是 bounded raw lookahead，沒有函式歸屬主張。raw prefix `d9c9f60594c56300017504d9f3eb05e875650000c3`：FXCH、TEST byte `[63c594],1`，flag clear 走 FPATAN，flag set CALL 5cc4d4。未讀 runtime flag、未追另一 callee；FPATAN route 的 stack 對應 angle(a,b) 為 atan2(b,a)，一般 atan/quadrant 邊界、CRT selected route及 rounding 仍未知。

renderer 4dd3dd/3e4 讀 runtime child X/Z velocity arrays（emitter+0x35c/+0x55c），先 **signed integer SAR 16**，再 FILD signed dword → FSTP f32，才呼叫 angle。故 low16 fraction 已捨棄；例如 raw -1 SAR16＝-1，raw 65535 SAR16＝0；不能用原 fixed-point float 向量取 atan。range signed high16＝[-32768,32767]，至 f32 可精確表示。

後續 direction 路徑在 x87／f32 store boundaries 下算 `(360 - angle) + (-45)`，若負則加360；再對 stored f32 **signed bit pattern** 與 `0x43b40000` 比較，達門檻加-360。單次 wrap，不是任意 modulo API。乘 direction count、stored angle及 f32 5f6826，再 FSTP f32 → 419c80；結果 IMUL frame count 後加 base frame。先前 vertical-velocity bucket 呼叫419cc0，或 lifetime 路徑兩次 signed IDIV，divisor validity與 overflow traps 未證。不能只用 axis 結果宣稱整體 frame index 已可精確計算。

## Palette 40e5c0：精確 32-bit 操作

完整 body 截止 normalized inclusive end **40e748**。參數依 stack 為 palette A、palette B、output、unused p4、weight w=p5、count n=p6、gains p7/p8/p9、tint p10、alpha flag p11。首 entry **無條件** `out[0]=A[0]`，包含原 alpha，未插值、未gain、未tint、未重新算alpha；B/output 都先 advance 4。DEC n 採32-bit wrapping，再 signed TEST/JLE；只在 `i32(wrap32(n-1))>0` 時處理其餘 entries。不能以 count<=1 簡化所有 signed extreme cases；零count 仍讀/寫第一 entry，caller bounds 未證。

下列是 assembly 的符號說明，不是新增可呼叫產品 API。令 W(x) 取 low32，SHR 為 unsigned logical shift；所有 ADD/SUB/IMUL 均 W，IMUL 只留 low32。`u=W(256-w)`；對 entries j≥1：

```text
Sb = W((A[j]&ff)*u + (B[j]&ff)*w)
Sg = W(((A[j]&ff00)>>8)*u + ((B[j]&ff00)>>8)*w)
Sr = W(((A[j]&ff0000)>>16)*u + ((B[j]&ff0000)>>16)*w)
L  = W((Sb>>8)*gainLow)>>8
M  = (W((Sg&ff00)*gainMiddle)>>8)&ff00
H  = W((Sr&ff00)*gainHigh)&ff0000
C  = W(W(L+M)+H)
if tint != 0:
    C = W(((C&fefefe)>>1) + ((tint&fefefe)>>1))
if alphaFlag != 0:
    lo=C&ff; mid=(C>>8)&ff; hi=(C>>16)&ff
    alpha=(lo+2*mid+hi)>>2
    C = W((C&ffffff)+(alpha<<24))
out[j] = C
```

證據：40e628/63d/651/666/67a/68b interpolation IMUL；40e68e SHR8、691 gain IMUL、6b0 SHR8；40e696/6a3 `AND ...,ff00`、6b3 `AND ...,ff00`、6bb `AND ...,ff0000`；6d3 SHR1；701/708/70a/70c 算 `(low+2*middle+high)>>2`，70f mask24、714 SHL24。低通道第二個 SHR 後**沒有**額外 AND ff，超出未證參數範圍時可能跨 channel；不能將式子改為每通道 float interpolation／clamp。tint 先 mask fefefe，避免每 byte half 跨位；alphaFlag==0 不做顯式 alpha synthesis，亦不能在未知 gain 範圍下宣称 output 高byte 必為零。沒有浮點指令。

## Palette 40e4d0：間接 packed gain

首 entry 是否保留由 `(*source &ffffff)==(p3&ffffff)` 決定，忽略 alpha 比較；相等則原32-bit entry copy、count DEC、兩 pointers advance。gains 用 `W(p5 + (p6<<8) + (p7<<16))` **ADD** packed，沒有輸入 byte masks；CALL `[618b54]` 傳 count/destination/source/packed gain。runtime implementation 未證，不推演完整 gain output。

optional tint 明確採 `(entry&fefefe)>>1 + (tint&fefefe)>>1`；nonzero p9 再呼叫 40186a(count,destination,destination)，本 probe 不追此 helper、不能把它的 alpha policy 當作40e5c0的已證式子。最後 CALL `[618b44]`；多條 return branches 都在 frozen。

磁碟 default slots 均讀4 bytes；各 target 僅讀64 bytes：

| slot VA | slot raw / SHA256 | target VA / 64-byte raw SHA256 |
|---|---|---|
| 618b54 | `50fd4000` / `8d4f29e52a7a263af31c98d6e2afcccbd7c7113985fe88834ecfc94b7658b901` | 40fd50 / `40350f2513f45c507fc18246e5a098595fc96caec01980f7753842788149ca25` |
| 618b44 | `60fc4000` / `72936ec3f6f213d7a2c919f7954670b572369ec45c322d52683e0b96fadf74aa` | 40fc60 / `4c6fe83ad8d7b59c150602e8537dd8513c2532db7d7ee4129541c2c939e087c4` |

兩個64-byte完整 raw hex 在 frozen/output。40fc60 第 byte c3 是磁碟 RET；後續 bytes 不代表同一函式。這些不是 runtime-chosen implementation 證據，未掃描 pointer writers或 broad xrefs。

## 下一個 Rust direction/frame/palette 工作

先由 parent 採集合法 runtime x87 control word／CRT flag／palette dispatch evidence，並限定取得40e5c0 caller 的 weight/gain/count ranges及 buffer alias/bounds。未取得前，Rust core 不增加產品 numerical API。direction/frame 另需 frame/direction metadata、IDIV divisor/overflow constraints、native float store boundaries；palette route需 selected gain implementation與40186a alpha證據。保留 integer mask/shift/wrapping，不用 approximate float替代。之後在新增 Rust module 以已證參數與邊界差分驗證，再做必要 host 薄接線；static byte verification 不等同 native game/frame/pixel驗收。

## Parent 後續像素寫入證據（2026-10-12）

`particle-pixel-composite-20261012-evidence.json`／同名 snapshot 已保存，REA session 已關閉。這些新增函式不包含在上述700個probe assertions中。`40E750`（ev_707e547d4f5485ac774358df3bdcbd598e9f8c8d6e1bafa868028d0d49160a1d）直接覆寫palette DWORD；`40E7B0`（ev_4b7de1ecd84886167d3b1d3d1ec9d2057d30392f9137cd6a8c50aa2e702ff20d）略過index0，其餘同樣覆寫。

`40E900`（ev_db18472887f97534872ee64b8891a6b93e04cf4f0faf337af9b858d895bf0424）及index0gate版 `40E980`（ev_3512283015b7399a7a570d52b9a653e5f3cbc9ba119675efce6b6223d0e3f5f0）寫入 `((source & 0xFEFEFE) >> 1) + ((destination & 0xFEFEFE) >> 1)`：每個channel兩邊分別截位，255與255得到254，high byte清為0；skip0時目的DWORD所有bits保留。這不同於一般floating alpha=0.5 blend，亦不是additive。

`4CAB80`（ev_926a4634872896dbf4b2bd89cbf9c70b78e3c55d5fc8073ad561c5aefb3a66b6）使用40E900/980；既有4B53C0 dispatch的mode2/7/8走這條路，mode1/5/6走4CA340的40E750/7B0，mode0走4CA010直接寫入。其他mode尚未閉合，不能把所有粒子都設成同一blend。這仍是靜態body證據，未實測destination pixels／quality aliases／GPU等價。
