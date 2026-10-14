# against-rome-data

Parent 驗證（2026-10-12）：最終 Rust core `cargo test --locked` 46 pass / 0 fail；新版 DLL 已實際載入並驗 ABI1 與粒子 24/2252/16/35/2322444-byte sizes，SHA256 `5d1bc57adc29f76331da139dbdf6b02074603633c52592bd8f180f28f1f09850`。C# 真 FFI 55 pass / 0 fail / 0 skip（粒子12、既有光源43），包括 installed 75 個粒子檔與既有光源檔的完整 payload／保存欄位逐位元比對，來源 hash/length/mtime 不變；TRX `re_workspace/particle-bridge-20261012/*20_31_23*.trx`。移除 `ARM_NATIVE_DATA_DLL` 後 AppContext.BaseDirectory 的粒子 ABI gate 1 pass。這是資料／interop 證據，尚非原遊戲粒子畫面或完整 editor 驗收。

## Immutable saved particle pool（2026-10-12 milestone）

新增 `particle` 與 `particle_ffi`，既有 light/projection ABI1、exports 與 layout 保持原樣。
依據 committed `tools/re/particle-render-contract-rs` 的 saved mapping、
`docs/reverse-engineering/particle-render-contract.md` 及目前 `NativeParticlePool.cs`。

- `ParticlePool::{from_payload, from_file_bytes, load}`：精確 2322444 bytes，header 三個 LE i32 為
  version1／capacity1024／children64；PFIL 重用既有 strict LZSS，宣告長度先檢查。
  `payload/source/input_sha256/source_path` 提供唯讀來源；SHA256 是原輸入（包含 PFIL wrapper），
  `load` 只讀檔案並保留呼叫者提供的 path，bytes constructor 的 path 為 None。
  SHA 是讀入那份 bytes 的識別，不是防止其他 process 同時改檔的 filesystem transaction。
- `snapshot_any(slot)` 保留 inactive；`capture(slot)` 只接受 nonzero emitter active；
  `active_slots()` 不把 flag 限定為 1。每個 detached `ParticleSnapshot` 保留 record2252
  和四個尾欄各一個 LE i32（`raw_record/raw_state/state_words`），不刪除 inactive fields/links。
- emitter：u16 active@0、i32 count_raw@2、i16 owner@6、i32 age_raw@8，child64×35。
  `child(ordinal)` 只檢查 ordinal<64，不套 count/active gate。
  child：u8 active@0；signed fixed XYZ i32@1/5/9；velocity_raw i32@13/17/21；
  lifetime_raw i32@25；opaque u16@29/31；light_link i16@33。
  velocity/lifetime 名稱只代表已觀察的靜態 consumer，不承諾 runtime tick 語意。
- `validate_renderable_count()` 是可選 0..=64 診斷，回報 slot/count；不影響 decode，
  也不代表 definition/asset/visibility 通過。整池 decode 不拒絕 unused garbage。
- `ParticleError::code()` 重用既有錯誤碼並新增 113（children header）、114（renderable count）、
  115（Rust load IO）；不更改舊錯誤碼與 light error messages。

新增 C ABI 全部使用 `ar_particle_pool_*`，完整宣告在 `include/against_rome_data.h`：

- `abi_version`=1；`snapshot_struct_size`=24；`record_size`=2252；`state_size`=16；
  `child_size`=35；`payload_size`=2322444；`error_message` 為 static string。
- `open/free`：輸入複製、不透明 handle、free exactly once、null free no-op；失敗 handle=null。
  可接 SafeHandle；double-free/use-after-free 或無效 pointer 仍是呼叫端錯誤。
- `active_count`；`snapshot/snapshot_any`；`child_any`；`validate_renderable_count`。
  `ArParticlePoolSnapshot`：u32 struct_size/slot/active_raw、i32 count_raw、i16 owner、
  u16 reserved（output0）、i32 age_raw。呼叫者初始化 struct_size 並提供各自獨立的
  record2252/state16 buffers；容量、slot、active、struct_size 全部預檢後才一起寫入。
  child_any 接 caller-owned35-byte buffer，不套 count 或 active。
  所有 error 均保留 snapshot/child/count 輸出原值；沒有 batch API。
- `copy_payload/copy_pfil_header/copy_input_sha256`：caller-owned byte buffers 與 required out_len，
  null/short buffer 回 BUFFER_TOO_SMALL；raw input 的 PFIL header 長度為0。
  輸出 buffers/metadata/out_len 必須互不重疊，pointer 必須有效。所有 status calls catch panic。

驗證使用獨立 `CARGO_TARGET_DIR=<repo>\re_workspace\particle-core-target`：
`cargo fmt`、`cargo test --locked`、`cargo test --release --locked`。
新增 `tests/particle_pool.rs` 使用合成 fixtures；load 測試僅建立／刪除 target 下的測試檔，
檢查 source bytes/hash/length/mtime 沒有被 parser 改動。
本輪 `cargo fmt` 成功（保留舊 modules/tests 原始格式，僅留下新增檔與 lib module declarations 的格式化）；
debug/release `cargo test --locked` 均 exit0、46 passed／0 failed／0 ignored：
SHA2561、light_pool9、projection12、particle_projection10、particle_pool14。
新增測試涵蓋全部1024 records／四columns／64 ordinals的 bitwise保留、strict headers／PFIL
truncation／overshoot／trailing、inactive/non1 flags、獨立 corrupt-count 診斷、FFI null／length／
caller-buffer容量／輸入複製後失效／單次釋放／多輸出錯誤原子性與來源 path/hash/metadata。
既有 ABI version1 與120/36/140/44/160-byte結構大小仍通過測試。
不部署 DLL、不 build .NET、不接 host；不解析猜測 sprite/frame/blend、不模擬 tick，
不宣稱原遊戲 runtime 或 pixel 等價。

Rust 核心：唯讀解析 Against Rome 的 saved `light.dat` 池，供編輯器（C# host 以薄膠水層）採用。
欄位語意依 `docs/reverse-engineering/saved-light-render-contract.md`（靜態逆向證據）。

## 範圍

- 輸入：PFIL 包裝檔（64-byte header + LZSS，自 `src.Shared/Core/GameLZSS.cs` port）或 57352-byte 原始 payload。
- 嚴格驗證：PFIL 宣告長度必須 = 57352（分配前檢查）；LZSS 輸入截斷、back-reference 超出輸出、輸出滿後仍有殘餘輸入皆報錯（C# 版本會靜默容忍這三種）；version = 1、capacity = 1024、長度精確。
- `LightPool`：保留解碼後 payload 與 PFIL header 原 bytes（`payload()`、`source()`），不提供可變參考。
- `LightSnapshot`（56 bytes，16 欄串接，非 runtime memcpy）：`active_raw` u32、XYZ／RGB／effective／baseline／type parameter 以 f32 讀出，並提供 `*_bits` 原 bit；`type_raw` u16；`age_raw` u32 與明確的 `age_signed` i32；`special_fx_raw` u32；`owner`／`parent`／`definition` i16；`raw_fields()` 為權威原始 bytes。`capture(slot)` 與 C# 相同（inactive 回 `None`）；`snapshot_any(slot)` 可讀 inactive 槽，保留未知狀態。
- 不做：不以 definition 重建欄位、不夾值、不驗 NaN/範圍策略、不推導 runtime +0x3C/+0x40、不模擬 tick/RNG/FPU、不寫檔。

## C ABI（`include/against_rome_data.h`，`against_rome_data.dll`）

- `ar_light_pool_open` 複製輸入並配置不透明 handle，`ar_light_pool_free` 恰好釋放一次（null 為 no-op；重複釋放／釋放後使用屬 host 錯誤）。失敗時 `*out = NULL`。
- 輸出結構與 buffer 皆由呼叫端配置；Rust 不交出內部指標。`ArLightSnapshot.struct_size` 須先設為 `ar_light_snapshot_struct_size()`。
- `ar_light_pool_copy_payload` / `copy_pfil_header`：`*out_len` 永遠回報所需長度，buffer 為 NULL 或過小回 `AR_BUFFER_TOO_SMALL`（兩段式）。
- 所有函式回傳 `int32` 錯誤碼（`error::code`／header enum），內部 panic 轉為 `AR_PANIC`；`ar_light_error_message` 回傳靜態字串。
- ABI 版本 `ar_light_abi_version() == 1`。

## CLI（唯讀）

```
ar-light-json dump  <light.dat> [--all-slots]      # 每槽 JSON：位置／RGB／半徑（值＋bits）、type、age、links、raw hex
ar-light-json audit <安裝根目錄|MAPS|light.dat>... [--out <新檔>]   # --out 為 create-new，拒絕覆寫
```

`audit` 於解析前後各計算檔案 SHA-256 並比對檔長與 mtime，輸出 saved 非 1 active、effective≠baseline（以 bit 比較）、非有限值、RGB>1 等計數。

## 建置與測試

```
$env:CARGO_HOME  = '<repo>\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME = '<repo>\re_workspace\rust-runtime\rustup'
& "$env:CARGO_HOME\bin\cargo.exe" test
& "$env:CARGO_HOME\bin\cargo.exe" build --release
```

`.cargo/config.toml` 將 target 導向 `re_workspace/against-rome-data-target`。無外部相依。測試僅用合成資料，crate 不含任何原遊戲資料。

## 驗證界限

- 已驗證：欄位 offset／寬度與合成資料一致、位元保留（含 NaN payload）、嚴格邊界、LZSS ring 初值（0x20×4078＋0×18）、FFI 錯誤碼與 buffer 協定、安裝檔解析結果與唯讀 hash。
- 未驗證：遊戲 runtime 行為、tick 動態、視覺或像素結果；欄位 12 的 consumer；owner/parent/definition 的有效範圍。C# `GameLZSS` 與完整保存池逐位元對照已由檔首 parent 驗證取代初期未驗狀態。
- audit 的檔案 hash 僅是「觀察當下的安裝狀態」，不是原版／來源認證。

## 保存光源投影（projection，供 editor family copy/move）

`project_batch(pool, &[LightProjectionRequest])` / C ABI `ar_light_pool_project_batch`（ABI 仍為 1，僅追加函式與結構，既有 `ArLightSnapshot` 120 bytes layout 不變）。

- 輸入為**保存狀態**：不套 owner dynamic transform、不以 baseline 代替 effective、不模擬 tick。
- 每軸：delta==0 → 保留原 bits（含 `-0.0`、NaN payload）；否則 `(saved_f32 as f64 + delta_i32 as f64 / 65536.0) as f32`，須為有限值。
- 其餘 bytes（active raw、RGB>1、兩 radius 不等、type/param/age/special/owner/parent/definition）原樣保留，僅 offset 4/8/12 可能改動。
- 原子：整批預檢（slot 範圍、inactive、changed 軸的非有限來源、結果非有限、struct_size、buffer 容量）後才寫輸出；任何錯誤均無 partial output，pool/來源不變。錯誤時 `*out_count=0`、`*failed_index`=出錯 request（BUFFER_TOO_SMALL 時 `*out_count`=所需 count）。
- 同 slot 可多次投影，各自獨立 instance；`target_guid` 原樣帶回，(guid, slot) 不去重；copy-of-copy 一律由保存來源重投影，不累計。
- 注意：`ResultOverflow` 對有限 f32 + 最大 |delta|(2^15) 實際不可達（四捨五入回 f32::MAX），僅為防禦；實務錯誤為 `INVALID_SLOT/INACTIVE_SLOT/NONFINITE_POSITION`。
- **非 particle 等價**：particle 連結光需「active child fixed + delta 再 SAR16」，此 API 只做保存光 XYZ 的直接 float 投影；particle light 使用另有驗證的 `ar_light_pool_project_particle_batch`，不可拿 directfloat 投影替代。
