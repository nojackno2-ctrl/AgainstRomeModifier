# Particle numeric contract probe

純 Rust 靜態 evidence 工具，path dependency 重用 `../particle-render-contract-rs` 的 `Pe`、`hash` 與 instruction verification；不呼叫該工具的 broad executable-section scan。沒有產品 numeric API、FFI、host 或 game execution。

限定六個完整 export function bodies、七個常數、兩個 on-disk pointers 的各 64-byte targets，以及 angle 唯一直接 CRT callee 的 128-byte raw window（僅解碼至第一個 RET，21 bytes / 7 instructions）。`src/frozen.json` 保存 raw hex、VA、size、SHA256、完整指令與完整 selected record 的 canonical JSON hashes。parent export 本身固定為 22 records，僅分析六筆指定 records。

在 repository root 使用既有 local toolchain，process-local environment：

```powershell
$env:CARGO_HOME="$PWD/re_workspace/rust-runtime/cargo"
$env:RUSTUP_HOME="$PWD/re_workspace/rust-runtime/rustup"
$env:PATH="$PWD/re_workspace/rust-runtime/cargo/bin;$env:PATH"
$env:CARGO_TARGET_DIR="$PWD/re_workspace/particle-numeric-target"
cargo fmt --manifest-path tools/re/particle-numeric-contract-rs/Cargo.toml -- --check
cargo test --offline --locked --manifest-path tools/re/particle-numeric-contract-rs/Cargo.toml
cargo run --offline --locked --manifest-path tools/re/particle-numeric-contract-rs/Cargo.toml --bin particle-numeric-contract -- 'C:/Program Files (x86)/Against Rome/Against_Rome.exe' re_workspace/particle-direction-palette-20261012-evidence.json re_workspace/particle-numeric-contract-20261012.json
```

已建立的 output 不可覆寫：`create_new(true)`，CLI 要求既存 repository `re_workspace` parent；重跑須使用事先尚不存在的 output。若 Windows policy 阻擋 executable，停止並報告，禁止換 executable filename 繞過。

`examples/freeze.rs` 是一次性 baseline capture 工具，不是替代 CLI；同樣只以 create_new 建立 `src/frozen.json`，既有 baseline 時應拒絕。首次 baseline 經主 CLI 與 tests 獨立核對。`serde_json/float_roundtrip` 保留 finite f64 的 bit-exact JSON round trip。

2026-10-12：fmt、8 tests 與 actual CLI 通過；700 assertions（693 body + 7 CRT）。限制、精確整數操作與後續工作見 `docs/reverse-engineering/particle-numeric-contract.md`。source bytes 與 export 於 collect 前後相同，寫入前再次核對 EXE；沒有執行 EXE 指令或遊戲。
