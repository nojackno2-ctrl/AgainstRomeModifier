//! Retained Ghidra evidence helpers shared by `probe-native-scene-projection` and
//! `probe-native-visual-state`: body/provenance checks and spelling-normalized comparison of
//! Ghidra assembly lines with the instruction decoded from the real EXE.

use crate::dis::{decode_linear, Ins};
use crate::jobj;
use crate::json::J;
use crate::util::*;
use serde_json::Value;

/// `-?0x[0-9a-f]+` -> decimal, everywhere in `text` (lower case input).
fn hex_to_decimal(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let negative = bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'0') && bytes.get(i + 2) == Some(&b'x');
        let start = if negative { i + 1 } else { i };
        if bytes[start..].starts_with(b"0x") && bytes.get(start + 2).is_some_and(u8::is_ascii_hexdigit) {
            let mut end = start + 2;
            while end < bytes.len() && (bytes[end].is_ascii_digit() || (b'a'..=b'f').contains(&bytes[end])) {
                end += 1;
            }
            let value = u128::from_str_radix(&text[start + 2..end], 16).unwrap_or(0);
            if negative {
                out.push('-');
            }
            out.push_str(&value.to_string());
            i = end;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// `mov` with an unsized absolute memory operand: Ghidra omits the width, assume `dword ptr`.
fn size_absolute_memory(operands: &str) -> String {
    let mut out = String::new();
    let mut rest = operands;
    while let Some(at) = rest.find("[0x") {
        let after = &rest[at + 3..];
        let digits = after.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        out.push_str(&rest[..at]);
        let closes = digits > 0 && after[digits..].starts_with(']');
        if closes && !rest[..at].ends_with("ptr ") {
            out.push_str("dword ptr ");
        }
        out.push_str("[0x");
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Spelling-only normalization of a Ghidra / decoder instruction (register, width and operand order kept).
/// `scene` additionally maps `sal`, `float ptr` and unsized absolute `mov` operands (scene-projection variant).
pub fn canonical_text(assembly: &str, scene: bool) -> (String, String) {
    let lower = assembly.to_lowercase();
    let (mnemonic, operands) = match lower.trim().split_once(' ') {
        Some((m, o)) => (m.to_string(), o.to_string()),
        None => (lower.trim().to_string(), String::new()),
    };
    let mut mnemonic = match mnemonic.as_str() {
        "jnz" => "jne".to_string(),
        "jz" => "je".to_string(),
        "sal" if scene => "shl".to_string(),
        _ => mnemonic,
    };
    if mnemonic == "retn" {
        mnemonic = "ret".into();
    }
    let mut operands = operands;
    if scene {
        operands = operands.replace("float ptr", "dword ptr");
        if mnemonic == "mov" {
            operands = size_absolute_memory(&operands);
        }
    }
    let operands: String = hex_to_decimal(&operands).split_whitespace().collect::<Vec<_>>().concat().replace("+0]", "]");
    // Immediates are compared as 32-bit values (iced prints `-1` operands as 0xffffffff).
    let operands = operands
        .split(',')
        .map(|token| match token.parse::<i64>() {
            Ok(v) => (v as i128).rem_euclid(1 << 32).to_string(),
            Err(_) => token.to_string(),
        })
        .collect::<Vec<_>>()
        .join(",");
    (mnemonic, operands)
}

pub fn same_instruction(expected: &str, ins: &Ins, scene: bool) -> bool {
    canonical_text(expected, scene) == canonical_text(&format!("{} {}", ins.mnemonic, ins.operands), scene)
}

/// Decodes the single instruction at `address` using at most `available` (<= 15) file-backed bytes.
pub fn decode_one(exe: &[u8], address: u64, available: usize) -> R<Ins> {
    let bytes = read_va(exe, address, available)?;
    decode_linear(bytes, address).into_iter().next().ok_or_else(|| format!("no instruction decoded at {address:#x}"))
}

pub fn parse_hex_address(text: &str) -> R<u64> {
    let t = text.trim();
    u64::from_str_radix(t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t), 16)
        .map_err(|_| format!("invalid literal for int() with base 16: '{text}'"))
}

pub fn json_records(retained: &[u8]) -> R<Vec<Value>> {
    let value: Value = serde_json::from_slice(retained).map_err(|e| e.to_string())?;
    value["records"].as_array().cloned().ok_or_else(|| "missing records".to_string())
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut v = value;
    for key in path {
        v = v.get(*key)?;
    }
    v.as_str()
}

/// Retained `analyze_function` record whose procedure address is `entry`; exactly one must exist.
/// `need_assembly` additionally requires a non-empty assembly listing.
pub fn procedure_record(records: &[Value], entry: u64, operation: Option<&str>, need_assembly: bool) -> R<Value> {
    let wanted = format!("{entry:#x}");
    let matches: Vec<&Value> = records
        .iter()
        .filter(|r| {
            operation.is_none_or(|op| r["operation"].as_str() == Some(op))
                && str_at(r, &["normalized_result", "procedure", "address"]) == Some(wanted.as_str())
                && (!need_assembly || r["normalized_result"]["assembly"].as_array().is_some_and(|a| !a.is_empty()))
        })
        .collect();
    req(matches.len() == 1, &format!("missing/ambiguous retained {} {entry:#x}", if need_assembly { "body" } else { "procedure" }))?;
    Ok(matches[0].clone())
}

pub struct Body {
    pub ranges: Vec<(u64, u64)>,
    pub raw_ranges: J,
    pub total_bytes: u64,
    pub contiguous: bool,
}

/// Validates the Ghidra body provenance fields and returns the inclusive ranges.
pub fn body_of(record: &Value) -> R<Body> {
    let body = &record["normalized_result"]["procedure"]["body"];
    req(
        body["available"] == true && body["contains_entry"] == true && body["provenance"] == "ghidra-function-body-address-set",
        "unproven body",
    )?;
    let mut ranges = Vec::new();
    let mut raw = J::arr();
    for r in body["ranges"].as_array().ok_or("missing body ranges")? {
        let (start, end) = (str_at(r, &["start"]).ok_or("missing range start")?, str_at(r, &["end"]).ok_or("missing range end")?);
        ranges.push((parse_hex_address(start)?, parse_hex_address(end)?));
        raw.push(jobj! { "start" => start, "end" => end });
    }
    Ok(Body {
        ranges,
        raw_ranges: raw,
        total_bytes: body["total_bytes"].as_u64().unwrap_or(0),
        contiguous: body["non_contiguous"] == false,
    })
}

/// The retained assembly lines (`0xADDR: MNEMONIC operands`).
pub fn assembly_lines(record: &Value) -> Vec<String> {
    record["normalized_result"]["assembly"]
        .as_array()
        .map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

pub fn subject_sha(record: &Value) -> Option<&str> {
    str_at(record, &["subject", "digest", "sha256"])
}

pub fn evidence_id(record: &Value) -> String {
    record["evidence_id"].as_str().unwrap_or("").to_string()
}

/// Splits `0xADDR: text`.
pub fn split_line(line: &str) -> R<(u64, &str)> {
    let (address, text) = line.split_once(": ").ok_or_else(|| format!("malformed retained line: {line}"))?;
    Ok((parse_hex_address(address)?, text))
}

/// Verifies one retained line against the EXE: the address must lie in exactly one body range.
pub fn verify_line(exe: &[u8], ranges: &[(u64, u64)], line: &str, scene: bool, mismatch: &str) -> R<Ins> {
    let (address, expected) = split_line(line)?;
    let containing: Vec<_> = ranges.iter().filter(|(a, b)| *a <= address && address <= *b).collect();
    req(containing.len() == 1, &format!("instruction outside retained body: {line}"))?;
    let available = 15.min((containing[0].1 - address + 1) as usize);
    let ins = decode_one(exe, address, available)?;
    req(same_instruction(expected, &ins, scene), &format!("{mismatch} at {:#x}", address))?;
    Ok(ins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghidra_and_decoder_spellings_normalize_equally() {
        let eq = |a: &str, b: &str| canonical_text(a, true) == canonical_text(b, true);
        assert!(eq("IMUL EDI, EBX, 0x4c", "imul edi, ebx, 76"));
        assert!(eq("CMP EDI, -0x1", "cmp edi, 0xffffffff"));
        assert!(eq("JNZ 0x004b47d6", "jne 0x4b47d6"));
        assert!(eq("LEA EBX, [ESI*0x4 + 0x0]", "lea ebx, [esi*4]"));
        assert!(eq("MOV EAX, dword ptr [ESP + 0x1c]", "mov eax, dword ptr [esp+0x1c]"));
        assert!(eq("FSUB float ptr [0x64d650]", "fsub dword ptr [0x64d650]"));
        assert!(eq("MOV EAX, [0x612c70]", "mov eax, dword ptr [0x612c70]"));
        assert!(eq("SAL EAX, 0x1", "shl eax, 1"));
        assert!(!eq("MOV EAX, dword ptr [ESP + 0x1c]", "mov ebx, dword ptr [esp+0x1c]"));
        assert!(!eq("ADD ESP, 0x4", "sub esp, 0x4"));
        assert!(!eq("MOV EAX, dword ptr [ESP + 0x1c]", "mov eax, word ptr [esp+0x1c]"));
    }

    #[test]
    fn visual_variant_keeps_sal_distinct() {
        assert_ne!(canonical_text("SAL EAX, 1", false), canonical_text("SHL EAX, 1", false));
        assert_eq!(canonical_text("JZ 0x10", false), canonical_text("je 0x10", false));
    }

    #[test]
    fn numeric_normalization_handles_hex_in_memory_operands() {
        assert_eq!(hex_to_decimal("[eax + 0xa14c06]"), "[eax + 10570758]");
        assert_eq!(hex_to_decimal("-0x10"), "-16");
        assert_eq!(hex_to_decimal("ebx"), "ebx");
    }

    #[test]
    fn body_validation_requires_ghidra_provenance() {
        let record: Value = serde_json::json!({
            "evidence_id": "e1",
            "normalized_result": {"procedure": {"address": "0x10", "body": {
                "available": true, "contains_entry": true, "non_contiguous": false,
                "provenance": "ghidra-function-body-address-set", "total_bytes": 6,
                "ranges": [{"start": "0x10", "end": "0x15"}]}},
              "assembly": ["0x10: NOP"]}
        });
        let body = body_of(&record).unwrap();
        assert_eq!(body.ranges, vec![(0x10, 0x15)]);
        assert!(body.contiguous);
        let mut bad = record.clone();
        bad["normalized_result"]["procedure"]["body"]["provenance"] = serde_json::json!("guess");
        assert!(body_of(&bad).is_err());
        let found = procedure_record(&[record.clone()], 0x10, None, true).unwrap();
        assert_eq!(evidence_id(&found), "e1");
        assert!(procedure_record(&[record.clone(), record], 0x10, None, true).is_err());
    }
}
