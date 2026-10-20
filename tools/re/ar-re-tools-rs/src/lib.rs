//! `ar-re`: Rust port of the reusable read-only reverse-engineering validation scripts
//! that used to live in `tools/re/*.py`. See README.md for the script -> subcommand map.
//!
//! Safety rules shared by every subcommand (as in the Python originals): inputs are only read,
//! reports are created with exclusive-create semantics (an existing file is never overwritten),
//! output paths inside the input / installed-game directories are refused, and EXE based
//! probes are gated on the SHA-256 of the one supported `Against_Rome.exe` build.

pub mod cmd;
pub mod dis;
pub mod floatconv;
pub mod json;
pub mod pools;
pub mod util;

#[cfg(test)]
pub mod tests_support;
