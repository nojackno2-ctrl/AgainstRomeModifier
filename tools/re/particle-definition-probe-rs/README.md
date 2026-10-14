# Particle definition probe

Read-only source-bound byte-window inventory plus base parameter/gfx/geometry and selected object-field audit.
Build with the repository-local Rust environment and a dedicated target directory.

```powershell
cargo run --offline --locked --manifest-path tools/re/particle-definition-probe-rs/Cargo.toml -- `
  "C:\Program Files (x86)\Against Rome\Against_Rome.exe" `
  re_workspace/new-windows.json 0x4dd8a0:0x46d 0x4ddd10:0x5c 0x4de690:0x235 0x4de8d0:0x5c

cargo run --offline --locked --manifest-path tools/re/particle-definition-probe-rs/Cargo.toml -- `
  --definitions "C:\Program Files (x86)\Against Rome" `
  re_workspace/particle-assets-parent-20261012.json re_workspace/new-definitions.json
```

Outputs must be new `.json` files inside this repository's `re_workspace`; existing files are refused.
The static mode checks the exact admitted EXE SHA256 and compares source bytes again before output.
VA windows are linear disassembly only: callers must establish entry, boundaries and reachability.
The definitions mode compares every parameter/gfx/geometry raw numeric word and name to the prior inventory,
plus six selected numeric fields and names for its object subset. It parses all base object rows and checks
the total row count, reports explicit ID chains, and verifies all four source files' bytes/length/mtime.
An object outside the oracle subset is parsed but has no independent word comparison in this mode.
It does not execute the game's loaders,
apply overlays, infer geometry/asset residency, or render particles.
