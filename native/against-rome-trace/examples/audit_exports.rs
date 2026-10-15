//! Verify a built DLL's export table without executing or deploying it.
use winmm::pe::{exports, Export};
fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: audit_exports PATH_TO_WINMM_DLL");
    let b = std::fs::read(path).expect("read DLL");
    assert!(
        winmm::pe::fingerprint(&b).is_some(),
        "must be an x86 PE32 image"
    );
    let actual = exports(&b).expect("parse export directory");
    let expected: Vec<_> = include_str!("../winmm.def")
        .lines()
        .filter_map(|l| {
            let (left, ordinal) = l.split_once('@')?;
            let ordinal = ordinal.split_whitespace().next()?.parse::<u32>().ok()?;
            Some(Export {
                ordinal,
                name: if l.contains("NONAME") {
                    None
                } else {
                    Some(left.split('=').next()?.trim().to_owned())
                },
            })
        })
        .collect();
    assert_eq!(
        actual, expected,
        "DLL exports differ from the C++ winmm definition"
    );
    println!(
        "PASS: {} x86 exports, exact names/ordinals/NONAME parity",
        actual.len()
    );
}
