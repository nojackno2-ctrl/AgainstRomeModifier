# Particle render contract probe

唯讀 PE32 / iced-x86 probe；EXE SHA256 固定
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
不開 REA、不執行遊戲、不改原遊戲池，不連結既有 editor core。

在 repository root 執行：

```powershell
$env:CARGO_HOME="$PWD\re_workspace\rust-runtime\cargo"
$env:RUSTUP_HOME="$PWD\re_workspace\rust-runtime\rustup"
$env:CARGO_TARGET_DIR="$PWD\re_workspace\particle-render-contract-target"
& "$env:CARGO_HOME\bin\cargo.exe" build --offline --locked --manifest-path tools/re/particle-render-contract-rs/Cargo.toml
& "$env:CARGO_HOME\bin\cargo.exe" test --offline --locked --manifest-path tools/re/particle-render-contract-rs/Cargo.toml
$output=Join-Path $PWD ('re_workspace\particle-render-contract-'+[guid]::NewGuid().ToString('N')+'.json')
& "$env:CARGO_HOME\bin\cargo.exe" run --offline --locked --manifest-path tools/re/particle-render-contract-rs/Cargo.toml -- 'C:\Program Files (x86)\Against Rome\Against_Rome.exe' $output "$PWD\re_workspace\native-particle-audit-20261010.json"
```

可追加 `VA:HEX_LENGTH` focused windows；長度為 exclusive upper bound，只輸出完整落在 window 的指令。decoder 最多唯讀額外15 bytes判定最後指令boundary；不自動擴展callee。

`assertions.json` 凍結2330 VA/bytes（既有audit/creation加本輪consumer）；逐次對EXE重驗bytes及instruction boundary。`sources.json` 綁定五個既有evidence/probe SHA；不符即失敗。不能當runtime證明。

Whole-code discovery 只針對 `GLOBALS` 已知粒子displacement。Linear executable-section candidates包含invalid count，不是CFG；詳見 [報告](../../../docs/reverse-engineering/particle-render-contract.md)。

`unpack_record` 只接受2252-byte saved emitter，回傳 `MappedSavedFields { bytes, defined }`。20個未映射runtime bytes的 `defined=false`，零值是storage placeholder，不能當native預設或完整runtime object。helper不處理pool尾四個columns／frame resolver。

以cache已有 `serde_core` 作 `serde` dependency alias，透過 `serde::Serialize` 寫JSON；沒下載derive facade。dependencies由Cargo.lock鎖定，無Python/dotnet。

最終build/test成功，6tests通過。先前run成功產生`particle-render-contract-final-20261012-f.json`，2330assertions；整理CLI後的最新版重跑被Windows application control `os error 4551`擋下，未執行／未產生g.json，也未驗CLI覆寫拒絕。未改安全policy。初次cargo fmt因stable toolchain缺rustfmt失敗；parent補裝workspace-local rustfmt後fmt成功，重跑6tests全pass。
