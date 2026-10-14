# Particle integer palette/pixel kernel contract

## 2026-10-13 parent verification and additive ABI

前次修正後的測試已在原 target 實際執行：`cargo test --offline --locked --manifest-path native/against-rome-data/Cargo.toml --test particle_pixels --test particle_pixel_ffi`，12 kernel + 4 ABI tests 全通過。另執行 `--lib --test light_pool --test particle_pool --test projection --test particle_projection --test particle_pixels --test particle_pixel_ffi` 共62 passed／0 failed；沒有執行受 Windows policy 封鎖的 ALR metadata test。

新增 `ar_particle_pixel_abi_version()` = 1、`ar_particle_pixel_row(indices, count, palette, palette_len, destination, mode, skip_zero, output, output_capacity)`。長度以元素為單位；mode 0 replacement／1 half-average，skip_zero 0 always-write／1 skip-zero。回傳0成功、1 null、2長度或容量錯誤、6panic、200無效選項、201索引越界。結果完整暫存後才寫caller output，允許output與destination/palette重疊；成功只寫count words，失敗全output不變。空row接受null；非空storage有效性與u32 alignment由caller保證。

`tools/build-native-data.ps1 -OutputDir re_workspace/particle-pixel-abi-check` 成功，Release DLL SHA256 `b2e86862993efc20f0250e34b64b1d070df086dcc4a33301fe021bed792a35f0`。script透過LoadLibrary/GetProcAddress及C# delegate真正執行half-average／skip-zero／tail保留／late-invalid-index原子失敗，並檢查既有light/particle ABI sizes。僅獨立candidate目錄，未替換預設部署DLL；metadata module雖參與編譯，沒有metadata exports或runtime驗證。

Solution `dotnet build AgainstRomeModifier.slnx -c Release --no-restore`：0 errors／41 warnings。此ABI尚未接入MapEditor sprite renderer；frame selection、ALR完整pixel decoding、endpoint palette dispatch及遊戲像素對照仍未完成。下列2026-10-12紀錄保留為原交付歷程，最終kernel執行狀態以上述parent結果為準。

2026-10-12 Codex。純 Rust immutable primitives，僅 supplied palette buffers／已選取的 indexed row span；無 FFI、C#、host、build script 或原遊戲修改。沒有遊戲 runtime pixels 等價主張。

## 證據綁定與範圍

- Parent export `re_workspace/particle-pixel-composite-20261012-evidence.json`：27 records，SHA256 `c445279e80240aa6977d9482b4615195e035f024a52d20d11b2a7befaf2057a4`。只選以下七個 `analyze_function` records；未開啟 REA 或重新讀 EXE。
- Export subject EXE SHA256：`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。這是 parent exported source identity，非本工作重新驗證磁碟 EXE 或 runtime 行為。
- `particle-numeric-contract.md` 讀取時 SHA256 `26543e09b08cf056279234714f939b09924e6ccf3fca4584beb9e11e23ce57e1`；其40e5c0公式及已驗 body hash `4ceb01bd7a6ca461b0017ebab22233064e1600ad5693a7ccc758ab67f6c6b6b6` 與本 export assembly 符合。文件原本不開放產品 numerical API 的後續工作建議，由本次使用者明確授權的 bounded supplied-buffer kernel 取代；dispatch／caller-domain未知仍保留。

| VA / inclusive end | support evidence identifier | 已證 primitive / context |
|---|---|---|
| 40e5c0 / 40e748 | `ev_2b8680ea046a1f4d2458f8cb767e804f56b474d73e72179dc398da1888c8f0d2` | raw palette integer kernel |
| 40e750 / 40e799 | `ev_707e547d4f5485ac774358df3bdcbd598e9f8c8d6e1bafa868028d0d49160a1d` | indexed replacement, always write |
| 40e7b0 / 40e80b | `ev_4b7de1ecd84886167d3b1d3d1ec9d2057d30392f9137cd6a8c50aa2e702ff20d` | indexed replacement, skip zero |
| 40e900 / 40e965 | `ev_db18472887f97534872ee64b8891a6b93e04cf4f0faf337af9b858d895bf0424` | indexed half-average, always write |
| 40e980 / 40e9f0 | `ev_3512283015b7399a7a570d52b9a653e5f3cbc9ba119675efce6b6223d0e3f5f0` | indexed half-average, skip zero |
| 4ca340 / 4ca73b | `ev_5693cabb1da55696707699108b5bcae39fcbf85e11b5d2c13c42964602c1c375` | replacement helper selection / palette route context |
| 4cab80 / 4caf7b | `ev_926a4634872896dbf4b2bd89cbf9c70b78e3c55d5fc8073ad561c5aefb3a66b6` | half-average helper selection / palette route context |

4ca340/4cab80 select the zero gate from row-segment metadata (`local_64==0` means skip-zero variant). They call **40e4d0** for endpoint/primary cases and **40e5c0** for the interpolation branch, with native count256 and alpha flag0. The 40e4d0 indirect gain/endpoint dispatch remains unresolved. This implementation does **not** use 40e5c0 as an endpoint fallback, recreate those callers, or infer ALR metadata, frame selection, definition mapping, lazy loading, clipping, pointer aliasing, strides, or a complete particle blitter.

## Immutable Rust APIs

Module `against_rome_data::particle_pixels`:

- `palette_kernel_40e5c0(a: &[u32], b: &[u32], supplied_count: usize, args: PaletteKernel40e5c0) -> Result<Vec<u32>, PixelError>`.
- `PaletteKernel40e5c0`: `weight: i32`, `gain_low/gain_middle/gain_high/tint/alpha_flag: u32`. Gains retain raw32 bits; alpha flag tests any nonzero value. No implicit defaults or bounded gain conversions.
- `half_average(source: u32, destination: u32) -> u32`.
- `indexed_pixel_row(indices: &[u8], palette: &[u32], destination: &[u32], zero_policy: IndexZeroPolicy, mode: PixelWriteMode) -> Result<Vec<u32>, PixelError>`.
- `IndexZeroPolicy::{AlwaysWrite, SkipZero}` and `PixelWriteMode::{Replacement, HalfAverage}` are explicit caller choices.

Errors distinguish invalid count, insufficient A/B buffer, unequal span lengths, and required palette index out of range (with first failing position/index/length). All inputs are shared immutable slices; outputs own new allocations. Validation of the entire operation precedes output allocation. No caller-visible partial output is returned on error, and no input can be mutated through these APIs. Caller-preselected spans are contiguous; empty row spans are valid.

Palette count **1..=256** and both buffers length>=count are **product-domain restrictions**, not claims that native validates count or bounds. Buffer lengths are raw-word counts, not byte counts. Extra entries are ignored. We conservatively require B length>=count even for count1, although native does not read B[0]. Native unconditionally reads A[0]/writes output[0], then wrapping DEC and signed JLE control the loop; native count0/out-of-bounds is dangerous, and signed extreme counts do not generalize to this restricted API.

## Exact 40e5c0 arithmetic

`out[0]=A[0]` unchanged, including alpha. For j>=1, let `W(x)` keep low32; every ADD/SUB/IMUL below uses W. SHR is logical unsigned; there are no floats, clamps, saturating operations, or per-channel rounding substitutions.

```text
w = raw32(weight); u = W(256-w)
Sb = W((A[j]&ff)*u + (B[j]&ff)*w)
Sg = W(((A[j]&ff00)>>8)*u + ((B[j]&ff00)>>8)*w)
Sr = W(((A[j]&ff0000)>>16)*u + ((B[j]&ff0000)>>16)*w)
L  = W((Sb>>8)*gainLow)>>8
M  = (W((Sg&ff00)*gainMiddle)>>8)&ff00
H  = W((Sr&ff00)*gainHigh)&ff0000
C  = W(W(L+M)+H)
if tint != 0:
    C = W(((C&00fefefe)>>1) + ((tint&00fefefe)>>1))
if alphaFlag != 0:
    alpha = ((C&ff) + 2*((C>>8)&ff) + ((C>>16)&ff))>>2
    C = W((C&ffffff)+(alpha<<24))
out[j] = C
```

Assembly anchors: interpolation IMUL 40e628/63d/651/666/67a/68b, gain IMUL 691/69e/6a8, SHR 68e/6ad/6b0, masks 696/6a3/6b3/6bb, tint TEST6c6 then mask/SHR/ADD6ca/6d3/6d5, alpha TEST6fd then weighted sum/SHR/mask/SHL6701..717. The tinted operand is masked before the native SAR at601, so it is nonnegative; logical SHR1 gives identical bits here. Alpha's extracted-byte sum is at most1020, so ordinary Rust u32 addition cannot overflow at that step.

Low gain has **no final &ff**. Thus output may cross channels or carry into the high byte with alpha disabled. Example: A[j]=ffffff, weight0, gains=(ffffffff,100,100) yields `01fffeff`; alpha synthesis yields `fefffeff`, not a clamped white. A nonzero high-only tint still enables halving with zero tint RGB. Tint precedes alpha synthesis. Source palette alpha is ignored after entry zero.

## Pixel row operations

Replacement copies the whole raw palette word, including alpha. HalfAverage is exactly:

```text
((src & 0x00fefefe) >> 1) + ((dst & 0x00fefefe) >> 1)
```

Each RGB operand truncates its own half before addition: white255+white255=>254; odd1+odd1=>0. This differs from dividing the sum by2. The output high byte is zero, irrespective of source/destination alpha. Masks avoid cross-byte carries.

SkipZero leaves the entire destination word unchanged and does not dereference palette[0]. All-zero skipped rows therefore permit an empty palette. AlwaysWrite requires palette[0] if index0 occurs. Every required nonzero index out of range rejects the entire row before any output allocation; a late bad index cannot expose partially processed output.

## Verification and limits

- Process-local Rust environment: `re_workspace/rust-runtime/{cargo,rustup}`, original target `re_workspace/against-rome-data-target`, manifest `native/against-rome-data/Cargo.toml`. No full suite, metadata test executable, policy change, alternative executable name/path/target, or runtime pixel comparison.
- `cargo build --offline --locked --manifest-path native/against-rome-data/Cargo.toml`: exit0.
- New source/tests formatted with `rustfmt --edition 2021`; manifest `cargo fmt --manifest-path native/against-rome-data/Cargo.toml -- --check` reports existing formatting differences (e.g. ar-light-json.rs and projection.rs/tests). Those files were left unchanged.
- The single authorized `cargo test --offline --locked --manifest-path native/against-rome-data/Cargo.toml --test particle_pixels` executed normally: **11 passed / 1 failed**; not CodeIntegrity-blocked. Failure was the test's incorrect manual expected row `00020406` for odd source010305 and destination030507. Correct divide-each-before-add result is `00010305` (observed implementation result), consistent with the independent exhaustive test that passed.
- Corrected that expectation afterward; **final test source has not been executed again** because only one attempt was authorized. Do not report this as a final 12-pass suite or authorize parent success-only integration on that basis. Final `cargo check --offline --locked --manifest-path native/against-rome-data/Cargo.toml --test particle_pixels` exit0; `rustfmt --check --edition 2021` on both new Rust files exit0; scoped lib diff whitespace check passed. These checks do not execute tests.
- Tests cover first-entry/raw alpha retention, count1/256/index255, weight0/128/256/512/negative/MIN/MAX, wrapping complement and gain multiplication, low-byte spill and high-byte carry, gain masks, tint order and high-only tint gate, any-nonzero alpha flag, replacement alpha, skipped-zero full bits, insufficient buffers/count/index/span errors and immutable inputs. Exhaustive 65,536 single-channel input pairs are checked independently using `src/2+dst/2`, at all three channel positions with independent high-alpha bits.
- This is static-evidence-bound integer computation with Rust test evidence, not original EXE execution, native runtime differential, mapeditor integration, or complete particle rendering. Existing `alr_metadata` export/work, FFI/header/C#/host/handoff and Git index/history were preserved.
