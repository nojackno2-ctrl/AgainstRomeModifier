use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=winmm.def");
    let def = fs::read_to_string("winmm.def").unwrap();
    let mut names = vec!["Ordinal2".to_owned()];
    for l in def.lines() {
        if let Some((_, rhs)) = l.split_once("= Thunk_") {
            names.push(rhs.split_whitespace().next().unwrap().to_owned());
        }
    }
    let mut out = format!("pub const EXPORT_COUNT: usize = {};\n", names.len());
    out.push_str("pub const EXPORT_NAMES: &[&str] = &[\n");
    for n in &names {
        out.push_str(&format!("{n:?},\n"));
    }
    out.push_str("];\n");
    out.push_str("#[cfg(target_arch=\"x86\")] static mut WINMM_POINTERS: [usize; EXPORT_COUNT] = [0; EXPORT_COUNT];\n");
    for (i, n) in names.iter().enumerate() {
        out.push_str(&format!("#[cfg(target_arch=\"x86\")] core::arch::global_asm!(\".global _Thunk_{n}\\n_Thunk_{n}:\\njmp dword ptr [{{table}} + {}]\", table = sym WINMM_POINTERS);\n", i * 4));
    }
    let dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(dir.join("exports.rs"), out).unwrap();
    if env::var("TARGET").unwrap() == "i686-pc-windows-msvc" {
        println!(
            "cargo:rustc-cdylib-link-arg=/DEF:{}",
            fs::canonicalize("winmm.def").unwrap().display()
        );
    }
}
