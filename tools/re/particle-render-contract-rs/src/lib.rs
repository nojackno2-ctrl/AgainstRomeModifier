use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter, Mnemonic, OpKind};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const HASH: &str = "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf";
pub const BASE: u64 = 0x12355d4;
pub const WINDOWS: &[(u64, usize)] = &[
    (0x48dd90, 0x475),
    (0x4db800, 0x130),
    (0x4db960, 0x4e4),
    (0x4dcfc0, 0x651),
    (0x4dd630, 0x225),
    (0x4b53c0, 0x66b),
    (0x4c31d0, 0x300),
    (0x4af200, 0x26),
    (0x4af230, 0x26),
    (0x4dc685, 0x90),
    (0x4dcef3, 0x6d),
    (0x4ca010, 0x160),
];
pub const GLOBALS: &[u64] = &[
    BASE,
    BASE + 4,
    BASE + 8,
    BASE + 12,
    BASE + 16,
    BASE + 20,
    BASE + 24,
    BASE + 28,
    BASE + 0x5c,
    BASE + 0x15c,
    BASE + 0x25c,
    BASE + 0x35c,
    BASE + 0x45c,
    BASE + 0x55c,
    BASE + 0x65c,
    BASE + 0x75c,
    BASE + 0x7dc,
    BASE + 0x85c,
    BASE + 0x8dc,
    BASE + 6,
    BASE + 0x7da,
    BASE + 0x85a,
];
pub fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn need(b: bool, s: &str) -> Result<()> {
    if b {
        Ok(())
    } else {
        Err(s.into())
    }
}
fn u16at(b: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(o..o.checked_add(2).ok_or("overflow")?)
            .ok_or("truncated u16")?
            .try_into()?,
    ))
}
fn u32at(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(o..o.checked_add(4).ok_or("overflow")?)
            .ok_or("truncated u32")?
            .try_into()?,
    ))
}
pub struct Section {
    pub va: u64,
    pub offset: usize,
    pub size: usize,
    pub executable: bool,
}
pub struct Pe<'a> {
    pub source: &'a [u8],
    pub sections: Vec<Section>,
}
impl<'a> Pe<'a> {
    pub fn parse(b: &'a [u8]) -> Result<Self> {
        need(b.get(..2) == Some(b"MZ"), "MZ")?;
        let p = u32at(b, 0x3c)? as usize;
        need(b.get(p..p + 4) == Some(b"PE\0\0"), "PE signature")?;
        need(u16at(b, p + 4)? == 0x14c, "x86 only")?;
        let n = u16at(b, p + 6)? as usize;
        let opt = p + 24;
        need(u16at(b, opt)? == 0x10b, "PE32 only")?;
        let base = u32at(b, opt + 28)? as u64;
        let table = opt + u16at(b, p + 20)? as usize;
        let mut sections = Vec::new();
        for k in 0..n {
            let o = table + k * 40;
            let s = Section {
                va: base + u32at(b, o + 12)? as u64,
                offset: u32at(b, o + 20)? as usize,
                size: u32at(b, o + 16)? as usize,
                executable: u32at(b, o + 36)? & 0x20000000 != 0,
            };
            need(
                s.offset
                    .checked_add(s.size)
                    .is_some_and(|end| end <= b.len()),
                "section bounds",
            )?;
            sections.push(s);
        }
        Ok(Self {
            source: b,
            sections,
        })
    }
    pub fn read(&self, va: u64, n: usize) -> Result<&'a [u8]> {
        for s in &self.sections {
            if va >= s.va {
                let r = (va - s.va) as usize;
                if r.checked_add(n).is_some_and(|e| e <= s.size) {
                    return Ok(&self.source[s.offset + r..s.offset + r + n]);
                }
            }
        }
        Err(format!("unmapped {va:#x}").into())
    }
    pub fn decode(&self, va: u64, n: usize) -> Result<Vec<Instruction>> {
        let raw = self.read(va, n.checked_add(15).ok_or("window overflow")?)?;
        let mut d = Decoder::with_ip(32, raw, va, DecoderOptions::NONE);
        let mut out = Vec::new();
        while d.can_decode() {
            let i = d.decode();
            if i.next_ip() > va + n as u64 {
                break;
            }
            if i.is_invalid() {
                return Err(format!(
                    "invalid instruction {:#x} in focused {va:#x}+{n:#x}",
                    i.ip()
                )
                .into());
            }
            out.push(i);
        }
        Ok(out)
    }
    pub fn item(&self, i: &Instruction) -> Result<Value> {
        let mut f = IntelFormatter::new();
        f.options_mut().set_hex_prefix("0x");
        f.options_mut().set_hex_suffix("");
        f.options_mut().set_uppercase_hex(false);
        let mut text = String::new();
        f.format(i, &mut text);
        Ok(
            json!({"va":format!("{:#x}",i.ip()),"bytes":hex(self.read(i.ip(),i.len())?),"instruction":text}),
        )
    }
}
pub fn verify(pe: &Pe, fixture: &Value) -> Result<usize> {
    let a = fixture.as_array().ok_or("fixture array")?;
    let mut seen = BTreeSet::new();
    for row in a {
        let va = u64::from_str_radix(row["va"].as_str().ok_or("va")?.trim_start_matches("0x"), 16)?;
        need(seen.insert(va), "duplicate VA")?;
        let h = row["bytes"].as_str().ok_or("bytes")?;
        need(h.len() % 2 == 0 && !h.is_empty(), "hex width")?;
        let b = (0..h.len())
            .step_by(2)
            .map(|p| u8::from_str_radix(&h[p..p + 2], 16))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        need(pe.read(va, b.len())? == b, "frozen bytes mismatch")?;
        let mut d = Decoder::with_ip(32, &b, va, DecoderOptions::NONE);
        let i = d.decode();
        need(
            !i.is_invalid() && i.len() == b.len(),
            "instruction boundary",
        )?;
    }
    Ok(a.len())
}
// Saved interleaved child -> runtime parallel arrays. No semantic field names.
pub const CHILD_MAP: &[(usize, usize, usize, usize)] = &[
    (0, 1, 0x1c, 1),
    (1, 4, 0x5c, 4),
    (5, 4, 0x15c, 4),
    (9, 4, 0x25c, 4),
    (13, 4, 0x35c, 4),
    (17, 4, 0x45c, 4),
    (21, 4, 0x55c, 4),
    (25, 4, 0x65c, 4),
    (29, 2, 0x75c, 2),
    (31, 2, 0x7dc, 2),
    (33, 2, 0x85c, 2),
];
pub struct MappedSavedFields {
    pub bytes: Vec<u8>,
    pub defined: Vec<bool>,
}
// Unmapped runtime bytes are placeholders: defined=false, never native defaults.
pub fn unpack_record(saved: &[u8]) -> Result<MappedSavedFields> {
    need(saved.len() == 2252, "record must be 2252")?;
    let mut r = vec![0; 0x8e0];
    let mut defined = vec![false; 0x8e0];
    for (s, n, t) in [(0, 2, 0), (2, 4, 4), (6, 2, 8), (8, 4, 12)] {
        r[t..t + n].copy_from_slice(&saved[s..s + n]);
        defined[t..t + n].fill(true);
    }
    for c in 0..64 {
        for &(s, n, t, stride) in CHILD_MAP {
            r[t + c * stride..t + c * stride + n]
                .copy_from_slice(&saved[12 + c * 35 + s..12 + c * 35 + s + n]);
            defined[t + c * stride..t + c * stride + n].fill(true);
        }
    }
    Ok(MappedSavedFields { bytes: r, defined })
}
pub fn run(exe: &Path, reference: &Path, windows: &[(u64, usize)]) -> Result<Value> {
    let b = fs::read(exe)?;
    need(hash(&b) == HASH, "unsupported EXE SHA256")?;
    let pe = Pe::parse(&b)?;
    let frozen: Value = serde_json::from_str(include_str!("assertions.json"))?;
    let count = verify(&pe, &frozen)?;
    let refbytes = fs::read(reference)?;
    let reference: Value = serde_json::from_slice(&refbytes)?;
    need(reference["exe_sha256"] == HASH, "reference hash mismatch")?;
    // Cross-check every native audit instruction independently against source bytes.
    let mut refs = Vec::new();
    for line in reference["instructions"]
        .as_array()
        .ok_or("reference instructions")?
    {
        let s = line.as_str().ok_or("instruction string")?;
        let mut fields = s.split_whitespace();
        refs.push(json!({"va":fields.next().ok_or("va")?,"bytes":fields.next().ok_or("bytes")?}));
    }
    let rc = verify(&pe, &Value::Array(refs))?;
    let mut xrefs = Vec::new();
    let mut scan = Vec::new();
    for s in pe.sections.iter().filter(|s| s.executable) {
        let raw = &b[s.offset..s.offset + s.size];
        let mut d = Decoder::with_ip(32, raw, s.va, DecoderOptions::NONE);
        let mut invalid = 0usize;
        let mut instructions = 0usize;
        while d.can_decode() {
            let i = d.decode();
            instructions += 1;
            if i.is_invalid() {
                invalid += 1;
                continue;
            }
            if (0..i.op_count()).any(|k| i.op_kind(k) == OpKind::Memory)
                && GLOBALS.contains(&i.memory_displacement64())
            {
                let mut v = pe.item(&i)?;
                v["global_operand"] = json!(format!("{:#x}", i.memory_displacement64()));
                xrefs.push(v);
            }
        }
        scan.push(json!({"va":format!("{:#x}",s.va),"raw_bytes":s.size,"decoded":instructions,"invalid":invalid}));
    }
    let mut focused = Vec::new();
    for &(va, n) in windows {
        let ins = pe.decode(va, n)?;
        let mut rows = Vec::new();
        let mut calls = BTreeSet::new();
        for i in &ins {
            rows.push(pe.item(i)?);
            if i.mnemonic() == Mnemonic::Call && i.op_kind(0) == OpKind::NearBranch32 {
                calls.insert(format!("{:#x}", i.near_branch_target()));
            }
        }
        focused.push(
            json!({"va":format!("{va:#x}"),"length":n,"instructions":rows,"direct_callees":calls}),
        );
    }
    need(fs::read(exe)? == b, "source changed")?;
    Ok(
        json!({"exe_sha256":HASH,"source_unchanged":true,"frozen_assertions":count,"reference_assertions":rc,"reference_sha256":hash(&refbytes),"child_mapping":CHILD_MAP.iter().map(|&(s,n,t,stride)|json!({"saved_child_offset":s,"width":n,"runtime_emitter_offset":format!("{t:#x}"),"ordinal_stride":stride})).collect::<Vec<_>>(),"saved_child_bytes":35,"runtime_child_sum_bytes":35,"runtime_emitter_bytes":0x8e0,"known_global_operands":GLOBALS.iter().map(|v|format!("{v:#x}")).collect::<Vec<_>>(),"scan":scan,"xrefs":xrefs,"focused":focused,"unknowns":["linear executable-section scan is candidate discovery, not CFG/function ownership","indirect or register-materialized globals outside exact operand list are not covered","no frame0 fallback or owner ALR substitution","runtime tick, FPU, pixels and asset resolution unverified"]}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_all_ordinals() {
        let s: Vec<u8> = (0..2252).map(|i| (i * 13 % 251) as u8).collect();
        let r = unpack_record(&s).unwrap();
        for c in 0..64 {
            for &(o, n, t, k) in CHILD_MAP {
                assert_eq!(
                    &s[12 + c * 35 + o..12 + c * 35 + o + n],
                    &r.bytes[t + c * k..t + c * k + n]
                );
                assert!(r.defined[t + c * k..t + c * k + n].iter().all(|v| *v));
            }
        }
        assert_eq!(r.bytes.len(), 2272);
        assert_eq!(r.defined.iter().filter(|v| **v).count(), 2252);
        assert!(!r.defined[2]);
        assert!(!r.defined[0x8dc]);
    }
    #[test]
    fn mapping_rejects_wrong_record() {
        assert!(unpack_record(&[0; 2251]).is_err());
        assert!(unpack_record(&[0; 2253]).is_err());
    }
    #[test]
    fn malformed_pe_rejected() {
        assert!(Pe::parse(&[]).is_err());
        assert!(Pe::parse(&[0; 64]).is_err());
    }
    #[test]
    fn frozen_bytes_and_va_mutations() {
        let raw = [0x83, 0xc0, 0x04];
        let pe = Pe {
            source: &raw,
            sections: vec![Section {
                va: 0x400000,
                offset: 0,
                size: 3,
                executable: true,
            }],
        };
        let good = json!([{"va":"0x400000","bytes":"83c004"}]);
        assert_eq!(verify(&pe, &good).unwrap(), 1);
        assert!(verify(&pe, &json!([{"va":"0x400000","bytes":"83c005"}])).is_err());
        assert!(verify(&pe, &json!([{"va":"0x400001","bytes":"83c004"}])).is_err());
        assert!(verify(&pe, &json!([{"va":"0x400000","bytes":"83c0"}])).is_err());
        assert!(pe.read(0x3fffff, 1).is_err());
    }
    #[test]
    fn wrong_exe_fails_before_reference() {
        assert!(run(
            Path::new("Cargo.toml"),
            Path::new("nonexistent-reference"),
            &[]
        )
        .unwrap_err()
        .to_string()
        .contains("SHA256"));
    }
    #[test]
    fn child_map_has_no_overlap() {
        let mut seen = BTreeSet::new();
        for c in 0..64 {
            for &(o, n, t, k) in CHILD_MAP {
                assert!(o + n <= 35);
                for j in 0..n {
                    assert!(seen.insert(t + c * k + j));
                }
            }
        }
        assert_eq!(seen.len(), 2240);
        assert_eq!(seen.iter().next_back(), Some(&0x8db));
    }
}
