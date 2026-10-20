//! `scan_native_scene.py`: read-only static scan of an explicit EXE copy for string references
//! (ALR/APT/texture-editor markers) plus optional disassembly ranges. Output is analysis
//! evidence, not an API contract. Ranges are virtual addresses: `--range 0x400000:0x400100`.
//!
//! Disassembly uses iced-x86 instead of Capstone (linear decoding; undecodable bytes are
//! skipped one byte at a time like Capstone's skipdata mode), so instruction text may differ
//! in spelling (`mov eax,[ebx+0xc]` vs `mov eax, dword ptr [ebx + 0xc]`) but not in addresses.

use super::Parsed;
use crate::dis::{formatter, format_instruction};
use crate::util::*;
use iced_x86::{Decoder, DecoderOptions, OpKind};
use std::collections::BTreeMap;
use std::path::Path;

const TOKENS: [&str; 5] = [".alr", ".apt", "textureeditor", "animation:", "vertexcolors"];

fn parse_address(text: &str) -> R<u64> {
    let t = text.trim();
    let value = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => t.parse::<u64>(),
    };
    value.map_err(|_| format!("invalid literal for int() with base 0: '{text}'"))
}

/// Printable ASCII runs of at least five characters (`[ -~]{5,}`) as (file offset, bytes).
fn ascii_runs(data: &[u8]) -> Vec<(usize, &[u8])> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if (b' '..=b'~').contains(&data[i]) {
            let start = i;
            while i < data.len() && (b' '..=b'~').contains(&data[i]) {
                i += 1;
            }
            if i - start >= 5 {
                runs.push((start, &data[start..i]));
            }
        } else {
            i += 1;
        }
    }
    runs
}

/// Values a Capstone `X86_OP_IMM` / `X86_OP_MEM` operand scan would see (masked to 32 bits).
fn operand_values(ins: &iced_x86::Instruction) -> Vec<u64> {
    let mut values = Vec::new();
    for i in 0..ins.op_count() {
        match ins.op_kind(i) {
            OpKind::Immediate8 | OpKind::Immediate8_2nd | OpKind::Immediate16 | OpKind::Immediate32 | OpKind::Immediate64
            | OpKind::Immediate8to16 | OpKind::Immediate8to32 | OpKind::Immediate8to64 | OpKind::Immediate32to64 => {
                values.push(ins.immediate(i) & 0xffff_ffff)
            }
            OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64 => values.push(ins.near_branch_target() & 0xffff_ffff),
            OpKind::Memory => values.push(ins.memory_displacement64() & 0xffff_ffff),
            _ => {}
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

/// Builds the report text for `exe`.
pub fn scan(exe: &Path, data: &[u8], ranges: &[String], extra_targets: &[String]) -> R<String> {
    let pe = Sections::parse(data)?;
    if pe.machine != 0x14c {
        return Err(usage("Expected an x86 PE executable"));
    }
    let base = pe.image_base as u64;
    let mut report = vec![
        format!("EXE: {}", resolve(exe).display()),
        format!("SHA256: {}", sha256_hex(data)),
        "Static candidate references; linear decoding is not function-boundary proof.".to_string(),
    ];
    let mut targets: BTreeMap<u64, String> = BTreeMap::new();
    for (start, run) in ascii_runs(data) {
        let text = String::from_utf8_lossy(run).to_string();
        let lower = text.to_lowercase();
        if !TOKENS.iter().any(|t| lower.contains(t)) {
            continue;
        }
        let address = base + pe.rva_from_offset(start as u32) as u64;
        targets.insert(address, text);
        report.push(format!("STRING 0x{address:08X} {}", targets[&address]));
    }
    for value in extra_targets {
        let address = parse_address(value)?;
        targets.insert(address, format!("requested target 0x{address:08X}"));
    }
    let mut fmt = formatter();
    for (_, va, raw_size, raw_offset, characteristics) in &pe.list {
        if characteristics & 0x2000_0000 == 0 {
            continue;
        }
        let section = data.get(*raw_offset as usize..(*raw_offset as usize + *raw_size as usize).min(data.len())).unwrap_or(&[]);
        let start_address = base + *va as u64;
        let mut decoder = Decoder::with_ip(32, section, start_address, DecoderOptions::NONE);
        while decoder.can_decode() {
            let position = decoder.position();
            let ins = decoder.decode();
            if ins.is_invalid() {
                // skipdata: advance one byte and continue
                let _ = decoder.set_position(position + 1);
                decoder.set_ip(start_address + position as u64 + 1);
                continue;
            }
            for value in operand_values(&ins) {
                if let Some(label) = targets.get(&value) {
                    let (mnemonic, operands) = format_instruction(&mut fmt, &ins);
                    report.push(format!("XREF 0x{:08X} {mnemonic} {operands} -> {label}", ins.ip()));
                }
            }
        }
    }
    for span in ranges {
        let (a, b) = span.split_once(':').ok_or_else(|| usage(format!("Range must look like START:END: {span}")))?;
        let (start, end) = (parse_address(a)?, parse_address(b)?);
        let section = pe.list.iter().find(|(_, va, raw_size, _, _)| base + *va as u64 <= start && start < base + *va as u64 + *raw_size as u64);
        let Some((_, va, raw_size, _, _)) = section else {
            return Err(usage(format!("Range must stay within one file-backed PE section: {span}")));
        };
        if end <= start || end > base + *va as u64 + *raw_size as u64 {
            return Err(usage(format!("Range must stay within one file-backed PE section: {span}")));
        }
        let offset = pe.offset_from_rva((start - base) as u32)? as usize;
        let bytes = data.get(offset..offset + (end - start) as usize).ok_or("range exceeds file")?;
        report.push(format!("RANGE 0x{start:08X}:0x{end:08X}"));
        for ins in crate::dis::decode_linear(bytes, start) {
            report.push(format!("0x{:08X} {:<30} {} {}", ins.address(), hex_spaced(&ins.bytes), ins.mnemonic, ins.operands));
        }
    }
    Ok(report.join("\n") + "\n")
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["range", "target", "output"])?;
    p.exact(1, "scan-native-scene EXE [--range 0xA:0xB]... [--target 0xADDR]... [--output FILE]")?;
    let exe = Path::new(&p.pos[0]);
    // Refuse to overwrite the input, an existing file, or anything inside the installed game.
    let output = match p.opt("output") {
        Some(path) => {
            let resolved = new_output(Path::new(path), &[installed_game()], "the installed game directory")?;
            if resolved == resolve(exe) {
                return Err(usage("Output must not overwrite the input executable"));
            }
            Some(resolved)
        }
        None => None,
    };
    let data = read(exe)?;
    let ranges: Vec<String> = p.all("range").iter().map(|s| s.to_string()).collect();
    let targets: Vec<String> = p.all("target").iter().map(|s| s.to_string()).collect();
    let text = scan(exe, &data, &ranges, &targets)?;
    match output {
        Some(path) => write_new(&path, &text)?,
        None => print!("{text}"),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    /// 32-bit PE with one executable section at RVA 0x1000 (file offset 0x200) holding `code` + `rdata` strings.
    fn tiny_exe(code: &[u8], text: &[u8]) -> Vec<u8> {
        let mut image = vec![0u8; 0x200];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        image[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        image[0x94..0x96].copy_from_slice(&224u16.to_le_bytes());
        let opt = 0x98;
        image[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        image[opt + 28..opt + 32].copy_from_slice(&0x400000u32.to_le_bytes());
        let section = opt + 224;
        image[section..section + 5].copy_from_slice(b".text");
        let size = (code.len() + text.len()).max(0x40) as u32;
        image[section + 8..section + 12].copy_from_slice(&size.to_le_bytes());
        image[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[section + 16..section + 20].copy_from_slice(&size.to_le_bytes());
        image[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        image[section + 36..section + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        image.resize(0x200, 0);
        image.extend_from_slice(code);
        image.extend_from_slice(text);
        image
    }

    #[test]
    fn finds_string_xrefs_and_disassembles_ranges() {
        // push 0x401020 ; jmp 0x401000   (string "bone.alr" at RVA 0x1020 = file 0x220)
        let mut code = vec![0x68, 0x20, 0x10, 0x40, 0x00, 0xe9, 0xf6, 0xff, 0xff, 0xff];
        code.resize(0x20, 0x90);
        let exe = tiny_exe(&code, b"bone.alr\0nothing interesting here\0");
        let report = scan(Path::new("tiny.exe"), &exe, &["0x401000:0x40100a".to_string()], &["0x401000".to_string()]).unwrap();
        assert!(report.contains("STRING 0x00401020 bone.alr"), "{report}");
        let xref = report.lines().find(|l| l.starts_with("XREF 0x00401000")).expect("xref line");
        assert!(xref.contains("push") && xref.ends_with("-> bone.alr"), "{xref}");
        assert!(report.contains("RANGE 0x00401000:0x0040100A"));
        assert!(report.contains("68 20 10 40 00"));
        let jump = report.lines().find(|l| l.starts_with("XREF 0x00401005")).expect("branch xref");
        assert!(jump.contains("jmp") && jump.ends_with("-> requested target 0x00401000"), "{jump}");
    }

    #[test]
    fn rejects_ranges_outside_sections_and_non_x86() {
        let exe = tiny_exe(&[0x90; 16], b"");
        assert!(scan(Path::new("t.exe"), &exe, &["0x500000:0x500010".to_string()], &[]).unwrap_err().starts_with("usage:"));
        assert!(scan(Path::new("t.exe"), &exe, &["0x401000:0x401000".to_string()], &[]).is_err());
        let mut arm = exe;
        arm[0x84..0x86].copy_from_slice(&0x1c0u16.to_le_bytes());
        assert!(scan(Path::new("t.exe"), &arm, &[], &[]).is_err());
    }

    #[test]
    fn refuses_to_overwrite_input_or_existing_output() {
        let dir = TempDir::new();
        let exe = write(dir.path(), "game.exe", &tiny_exe(&[0x90; 8], b""));
        let report = dir.path().join("report.txt");
        std::fs::write(&report, b"keep").unwrap();
        let a = [exe.display().to_string(), "--output".into(), exe.display().to_string()];
        assert!(run(&a).unwrap_err().starts_with("usage:"));
        let b = [exe.display().to_string(), "--output".into(), report.display().to_string()];
        assert!(run(&b).is_err());
        assert_eq!(std::fs::read(&report).unwrap(), b"keep");
    }
}
