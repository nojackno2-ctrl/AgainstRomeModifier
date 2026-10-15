//! Rust port of native/argm-trace. Runtime is deliberately x86 Windows only.
pub mod config;
pub mod detour;
pub mod lde;
pub mod pe;
#[cfg(all(windows, any(target_arch = "x86", feature = "check-runtime")))]
mod runtime;
pub mod targets;
pub mod tracelog;
