//! `verify_objective_bci.py`: verify a raw/PFIL objective injection (static, read-only).
//!
//! The PFIL wrapper is unpacked with `against_rome_data::lzss`; the relaxed BCI0 container
//! reader mirrors `tools/bcitool.py` (and `ar-bcitool`), which ignore version/locals.

use super::Parsed;
use crate::util::*;
use against_rome_data::lzss::decompress_pfil;
use std::collections::BTreeSet;
use std::path::Path;

pub struct Bci {
    pub code: Vec<u8>,
    pub tail: Vec<u8>,
    pub names: Vec<String>,
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

impl Bci {
    pub fn parse(raw: &[u8]) -> R<Self> {
        let data = if raw.starts_with(b"PFIL") { decompress_pfil(raw, None).map_err(|e| e.to_string())? } else { raw.to_vec() };
        req(data.starts_with(b"BCI0"), "not a BCI0 container")?;
        let code_size = u32_at(&data, 8)? as usize;
        let blob_len = u32_at(&data, 0x10)? as usize;
        let sym_count = u32_at(&data, 0x14)? as usize;
        let code = data.get(0x24..0x24 + code_size).ok_or("truncated CODE")?.to_vec();
        let p = 0x24 + code_size;
        req(data.get(p..p + 8) == Some(b"SYMBCONS"), "missing SYMBCONS")?;
        let blob = data.get(p + 8..p + 8 + blob_len).ok_or("truncated SYMBCONS")?;
        let q = p + 8 + blob_len;
        req(data.get(q..q + 4) == Some(b"CIDX"), "missing CIDX")?;
        let mut names = Vec::new();
        for i in 0..sym_count {
            let start = u32_at(&data, q + 4 + i * 4)? as usize;
            let end = blob.get(start..).and_then(|b| b.iter().position(|&c| c == 0)).map(|n| start + n).unwrap_or(blob.len().saturating_sub(1));
            names.push(latin1(blob.get(start..end).unwrap_or_default()));
        }
        let tail = data.get(q + 4 + 4 * sym_count..).ok_or("truncated CIDX")?.to_vec();
        Ok(Self { code, tail, names })
    }
    pub fn words(&self) -> Vec<i32> {
        self.code.chunks_exact(4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).collect()
    }
}

const OP1: [u32; 30] = [
    64, 65, 66, 68, 69, 70, 73, 76, 77, 78, 80, 81, 82, 83, 84, 90, 91, 92, 93, 112, 113, 114, 115, 116, 117, 118, 120, 128, 129, 160,
];

fn step(op: u32) -> usize {
    if op == 67 {
        3
    } else if OP1.contains(&op) {
        2
    } else {
        1
    }
}

/// Linear disassembly rows `(byte offset, opcode)`; operand reads past CODE are an error.
pub fn disasm(words: &[i32]) -> R<Vec<(usize, u32)>> {
    let (mut i, mut rows) = (0usize, Vec::new());
    while i < words.len() {
        let op = words[i] as u32;
        let width = step(op);
        req(i + width <= words.len(), "list index out of range")?;
        rows.push((i * 4, op));
        i += width;
    }
    Ok(rows)
}

/// `(byte offset, symbol index, name)` of every `pushsym` (op 128).
pub fn scan_calls(bci: &Bci) -> R<Vec<(usize, i32, String)>> {
    let words = bci.words();
    let (mut i, mut out) = (0usize, Vec::new());
    while i < words.len() {
        let op = words[i];
        if op == 128 {
            let a = *words.get(i + 1).ok_or("list index out of range")?;
            let name = usize::try_from(a).ok().and_then(|n| bci.names.get(n)).cloned().unwrap_or_else(|| "??".to_string());
            out.push((i * 4, a, name));
            i += 2;
        } else {
            i += step(op as u32);
        }
    }
    Ok(out)
}

/// Returns the report lines on success; failures carry the Python assertion message.
pub fn verify(original: &[u8], injected: &[u8]) -> R<Vec<String>> {
    let original = Bci::parse(original)?;
    let injected_bci = Bci::parse(injected)?;
    let words = injected_bci.words();
    let rows = disasm(&words)?;
    let boundaries: BTreeSet<usize> = rows.iter().map(|(offset, _)| *offset).collect();
    for &(offset, op) in &rows {
        if (112..119).contains(&op) || op == 120 {
            let target = offset as i64 + 8 + words[offset / 4 + 1] as i64;
            req(target >= 0 && boundaries.contains(&(target as usize)), &format!("unaligned/invalid branch {offset:#x} -> {target:#x}"))?;
        }
    }
    // Original wait push becomes the only changed eight-byte trampoline.
    let changed: Vec<usize> = original.code.iter().zip(&injected_bci.code).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
    req(!changed.is_empty(), "no original wait hook replaced")?;
    let hook = changed[0] / 4 * 4;
    req(i32_at(&injected_bci.code, hook)? == 112, "hook is not a jmp")?;
    req(injected_bci.code[..hook] == original.code[..hook], "code before hook changed")?;
    req(injected_bci.code[hook + 8..original.code.len()] == original.code[hook + 8..], "code after hook changed")?;
    req(injected_bci.code.len() > original.code.len(), "no code added")?;
    let (it, ot) = (&injected_bci.tail, &original.tail);
    req(it.len() >= 4 && ot.len() >= 4 && it[..it.len() - 4] == ot[..ot.len() - 4], "variables or destructor changed")?;
    let main = i32_at(it, it.len() - 4)?;
    req(main >= original.code.len() as i32 && main >= 0 && boundaries.contains(&(main as usize)), "main entry is not a new instruction boundary")?;
    req(injected_bci.names.starts_with(&original.names), "symbol table prefix changed")?;
    let calls = scan_calls(&injected_bci)?;
    let added: Vec<&(usize, i32, String)> = calls.iter().filter(|(off, _, _)| *off >= original.code.len()).collect();
    for native in ["s_getTime", "s_getObjPos", "s_setScriptVarL", "s_quitGame"] {
        req(added.iter().any(|(_, _, name)| name == native), &format!("missing {native}"))?;
    }
    req(injected_bci.names.iter().any(|n| n == "GLOBAL_MISSION_RESULT"), "GLOBAL_MISSION_RESULT missing")?;
    req(!added.iter().any(|(_, _, name)| name == "s_lgcSetMissionResult"), "s_lgcSetMissionResult must not be called")?;
    let mut lines = vec![format!(
        "PASS: {} instructions; all branches valid; hook {hook:#x}; original code/metadata preserved",
        rows.len()
    )];
    for (off, _, native) in added {
        if native == "s_getObjPos" || native == "s_quitGame" {
            lines.push(format!("{off:#08x}: {native}"));
        }
    }
    Ok(lines)
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "verify-objective-bci ORIGINAL INJECTED")?;
    let injected_bytes = read(Path::new(&p.pos[1]))?;
    let lines = verify(&read(Path::new(&p.pos[0]))?, &injected_bytes)?;
    for line in lines {
        println!("{line}");
    }
    println!("injected SHA256: {}", sha256_hex(&injected_bytes));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn container(words: &[i32], names: &[&str], main: i32) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut offsets = Vec::new();
        for n in names {
            offsets.push(blob.len() as u32);
            blob.extend(n.as_bytes());
            blob.push(0);
        }
        let mut data = vec![0u8; 0x24];
        data[..4].copy_from_slice(b"BCI0");
        data[8..12].copy_from_slice(&((words.len() * 4) as u32).to_le_bytes());
        data[0x10..0x14].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        data[0x14..0x18].copy_from_slice(&(names.len() as u32).to_le_bytes());
        data.extend(words.iter().flat_map(|w| w.to_le_bytes()));
        data.extend(b"SYMBCONS");
        data.extend(&blob);
        data.extend(b"CIDX");
        data.extend(offsets.iter().flat_map(|o| o.to_le_bytes()));
        data.extend(b"VAR ");
        data.extend(main.to_le_bytes());
        data
    }

    // original: [pushlit 5][jmp -> end] ... hook word pair (push 66,0) replaced by jmp to new code
    fn original_words() -> Vec<i32> {
        vec![95, 66, 5, 66, 9, 121]
    }

    fn injected_words() -> Vec<i32> {
        // hook at word 3: jmp (112) to new block at byte 24 (offset + 8 + a => 12 + 8 + a = 24 -> a = 4)
        let mut w = vec![95, 66, 5, 112, 4, 121];
        // new code at byte 24: pushsym getTime, getObjPos, setScriptVarL, quitGame, then jmp back to ret
        for sym in 0..4 {
            w.extend([128, sym, 86]);
        }
        w.extend([112, 0]); // back-jump operand patched by the tests
        w
    }

    const NAMES: [&str; 5] = ["s_getTime", "s_getObjPos", "s_setScriptVarL", "s_quitGame", "GLOBAL_MISSION_RESULT"];

    #[test]
    fn accepts_a_well_formed_injection() {
        let original = container(&original_words(), &[], 0);
        // fix the back jump so it lands on the original ret (byte 20): offset + 8 + a = 20
        let mut words = injected_words();
        let last = words.len() - 2;
        words[last + 1] = 20 - (last as i32 * 4 + 8);
        let injected = container(&words, &NAMES, 24);
        let report = verify(&original, &injected).unwrap();
        assert!(report[0].starts_with("PASS: "));
        assert!(report.iter().any(|l| l.ends_with("s_getObjPos")));
        assert!(report.iter().any(|l| l.ends_with("s_quitGame")));
    }

    #[test]
    fn rejects_unaligned_branch_missing_native_and_wrong_hook() {
        let original = container(&original_words(), &[], 0);
        let mut words = injected_words();
        let last = words.len() - 2;
        words[last + 1] = 20 - (last as i32 * 4 + 8);
        // 1: branch into the middle of an instruction
        let mut bad_branch = words.clone();
        bad_branch[last + 1] += 1;
        let injected = container(&bad_branch, &NAMES, 24);
        assert!(verify(&original, &injected).unwrap_err().contains("unaligned/invalid branch"));
        // 2: missing quitGame symbol
        let injected = container(&words, &NAMES[..3], 24);
        assert!(verify(&original, &injected).is_err());
        // 3: no change at all
        assert_eq!(verify(&original, &original).unwrap_err(), "no original wait hook replaced");
        // 4: hook is not a jmp
        let mut not_jmp = words.clone();
        not_jmp[3] = 66;
        assert!(verify(&original, &container(&not_jmp, &NAMES, 24)).is_err());
    }

    #[test]
    fn pfil_wrapped_images_are_unpacked() {
        let raw = container(&original_words(), &["a"], 0);
        // decompress_pfil is strict about the declared size, so a store-mode wrapper must be exact
        let wrapped = pfil_store(&raw);
        let bci = Bci::parse(&wrapped).unwrap();
        assert_eq!(bci.names, vec!["a".to_string()]);
        assert_eq!(bci.words(), original_words());
        assert!(Bci::parse(b"BCI1....").is_err());
    }

    #[test]
    fn scan_calls_skips_two_word_literals() {
        let bci = Bci::parse(&container(&[67, 128, 128, 128, 0, 66, 128], &["x"], 0)).unwrap();
        let calls = scan_calls(&bci).unwrap();
        assert_eq!(calls, vec![(12, 0, "x".to_string())]);
    }
}
