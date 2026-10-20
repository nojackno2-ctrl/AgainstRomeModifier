//! `probe_pe_interfaces.py`: PE32 import/export metadata; nothing is loaded or executed.

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::util::*;
use std::path::Path;

fn ascii(bytes: Vec<u8>) -> R<String> {
    String::from_utf8(bytes).ok().filter(|s| s.is_ascii()).ok_or_else(|| "'ascii' codec can't decode byte".to_string())
}

pub fn inspect(data: &[u8]) -> R<J> {
    req(data.len() >= 64 && data.get(..2) == Some(b"MZ"), "not MZ")?;
    let nt = u32_at(data, 60)? as usize;
    req(nt + 24 <= data.len() && data.get(nt..nt + 4) == Some(b"PE\0\0"), "not PE")?;
    let machine = u16_at(data, nt + 4)?;
    let count = u16_at(data, nt + 6)? as usize;
    let opt_size = u16_at(data, nt + 20)? as usize;
    let flags = u16_at(data, nt + 22)?;
    let opt = nt + 24;
    req(opt_size >= 224 && opt + opt_size + count * 40 <= data.len(), "truncated PE32 headers")?;
    req(u16_at(data, opt)? == 0x10b, "expected PE32")?;
    let base = u32_at(data, opt + 28)? as u64;
    let rd = |rva: u64, size: usize| read_va(data, base + rva, size).map_err(|e| format!("{e} (rva {rva:#x} +{size})"));
    let string = |rva: u64| -> R<String> {
        let mut result = Vec::new();
        for offset in 0..1024u64 {
            let byte = rd(rva + offset, 1)?[0];
            if byte == 0 {
                return ascii(result);
            }
            result.push(byte);
        }
        Err("unterminated interface string".into())
    };
    let mut sections = J::arr();
    let mut raw_end = opt + opt_size + count * 40;
    for index in 0..count {
        let offset = opt + opt_size + index * 40;
        let name_bytes = data[offset..offset + 8].to_vec();
        let name = ascii(name_bytes.into_iter().take_while(|&b| b != 0).collect::<Vec<_>>()).or_else(|_: String| -> R<String> {
            Err("'ascii' codec can't decode section name".into())
        })?;
        let (virtual_size, rva, raw_size, raw_offset) =
            (u32_at(data, offset + 8)?, u32_at(data, offset + 12)?, u32_at(data, offset + 16)? as usize, u32_at(data, offset + 20)? as usize);
        req(raw_size == 0 || raw_offset + raw_size <= data.len(), "section exceeds source bytes")?;
        raw_end = raw_end.max(raw_offset + raw_size);
        sections.push(jobj! {
            "name" => name, "rva" => rva, "virtual_bytes" => virtual_size,
            "raw_bytes" => raw_size, "raw_offset" => raw_offset,
        });
    }
    let mut imports = J::arr();
    let (rva, size) = (u32_at(data, opt + 104)? as u64, u32_at(data, opt + 108)? as usize);
    if rva != 0 {
        let mut terminated = false;
        for index in 0..(size / 20).min(4096) {
            let d = rd(rva + index as u64 * 20, 20)?;
            let fields: Vec<u32> = (0..5).map(|i| u32_at(d, i * 4)).collect::<R<_>>()?;
            let (lookup, name, iat) = (fields[0] as u64, fields[3] as u64, fields[4] as u64);
            if fields.iter().all(|&f| f == 0) {
                terminated = true;
                break;
            }
            let mut members = J::arr();
            let mut member_end = false;
            for slot in 0..65536u64 {
                let value = u32_at(rd((if lookup != 0 { lookup } else { iat }) + slot * 4, 4)?, 0)?;
                if value == 0 {
                    member_end = true;
                    break;
                }
                let iat_va = base + iat + slot * 4;
                if value & 0x8000_0000 != 0 {
                    members.push(jobj! { "iat_va" => iat_va, "ordinal" => value & 65535 });
                } else {
                    members.push(jobj! { "iat_va" => iat_va, "name" => string(value as u64 + 2)? });
                }
            }
            req(member_end, "unterminated import table")?;
            imports.push(jobj! { "library" => string(name)?, "members" => members });
        }
        req(terminated, "unterminated import directory")?;
    }
    let mut exports = J::arr();
    let (rva, size) = (u32_at(data, opt + 96)? as u64, u32_at(data, opt + 100)? as u64);
    if rva != 0 {
        // '<IIHH7I': Characteristics, Timestamp, Major, Minor, Name, Base, NumberOfFunctions,
        // NumberOfNames, AddressOfFunctions, AddressOfNames, AddressOfNameOrdinals (40 bytes).
        let header = rd(rva, 40)?;
        let field = |i: usize| u32_at(header, 16 + i * 4).map(u64::from);
        let (ordinal_base, functions, names, eat, name_table, ordinals) =
            (field(0)?, field(1)?, field(2)?, field(3)?, field(4)?, field(5)?);
        req(names <= functions && functions <= 65536, "export count exceeds analysis limit")?;
        let mut labels: std::collections::BTreeMap<u64, Vec<String>> = Default::default();
        for index in 0..names {
            let name = u32_at(rd(name_table + index * 4, 4)?, 0)? as u64;
            let slot = u16_at(rd(ordinals + index * 2, 2)?, 0)? as u64;
            req(slot < functions, "export ordinal outside EAT")?;
            labels.entry(slot).or_default().push(string(name)?);
        }
        for slot in 0..functions {
            let address = u32_at(rd(eat + slot * 4, 4)?, 0)? as u64;
            if address != 0 {
                let mut record = jobj! {
                    "ordinal" => ordinal_base + slot,
                    "names" => labels.get(&slot).cloned().unwrap_or_default(),
                    "va" => base + address,
                };
                if rva <= address && address < rva + size {
                    record.set("forwarder", string(address)?);
                }
                exports.push(record);
            }
        }
    }
    Ok(jobj! {
        "sha256" => sha256_hex(data),
        "bytes" => data.len(),
        "machine" => machine,
        "dll" => flags & 0x2000 != 0,
        "image_base" => base,
        "entry_va" => base + u32_at(data, opt + 16)? as u64,
        "sections" => sections,
        "imports" => imports,
        "exports" => exports,
        "unmapped_tail_bytes" => data.len() as i64 - raw_end as i64,
        "limitations" => vec![
            "PE32 metadata only; dynamic loads and runtime interfaces require separate tracing",
            "unmapped tail may contain certificates or other data, not an inferred payload",
        ],
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-pe-interfaces SOURCE OUTPUT.json")?;
    let source = Path::new(&p.pos[0]);
    let source_dir = resolve(source).parent().map(Path::to_path_buf).unwrap_or_default();
    let output = new_output(Path::new(&p.pos[1]), &[source_dir], "source directory")?;
    let result = inspect(&read(source)?)?;
    write_new(&output, &(result.pretty(true) + "\n"))?;
    let mut summary = J::obj();
    if let J::Obj(items) = &result {
        for (k, v) in items {
            if !["sections", "imports", "exports", "limitations"].contains(&k.as_str()) {
                summary.set(k, v.clone());
            }
        }
    }
    println!("{}", summary.line(true));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a tiny PE32 DLL with one section holding an import table and an export table.
    pub fn synthetic_dll() -> Vec<u8> {
        let mut image = vec![0u8; 0x600];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        let nt = 0x80;
        image[nt..nt + 4].copy_from_slice(b"PE\0\0");
        image[nt + 4..nt + 6].copy_from_slice(&0x14cu16.to_le_bytes());
        image[nt + 6..nt + 8].copy_from_slice(&1u16.to_le_bytes());
        image[nt + 20..nt + 22].copy_from_slice(&224u16.to_le_bytes());
        image[nt + 22..nt + 24].copy_from_slice(&0x2000u16.to_le_bytes());
        let opt = nt + 24;
        image[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        image[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // entry
        image[opt + 28..opt + 32].copy_from_slice(&0x1000_0000u32.to_le_bytes()); // base
        // export directory at RVA 0x1100 (size 0x30), import directory at 0x1200 (size 40)
        image[opt + 96..opt + 100].copy_from_slice(&0x1100u32.to_le_bytes());
        image[opt + 100..opt + 104].copy_from_slice(&0x30u32.to_le_bytes());
        image[opt + 104..opt + 108].copy_from_slice(&0x1200u32.to_le_bytes());
        image[opt + 108..opt + 112].copy_from_slice(&40u32.to_le_bytes());
        let section = opt + 224;
        image[section..section + 5].copy_from_slice(b".text");
        image[section + 8..section + 12].copy_from_slice(&0x400u32.to_le_bytes());
        image[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[section + 16..section + 20].copy_from_slice(&0x400u32.to_le_bytes());
        image[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        let put = |image: &mut Vec<u8>, rva: usize, bytes: &[u8]| image[0x200 + rva - 0x1000..0x200 + rva - 0x1000 + bytes.len()].copy_from_slice(bytes);
        // exports: base 5, 2 functions, 1 name
        let mut export: Vec<u8> = Vec::new();
        for v in [0u32, 0, 0, 0x1180, 5, 2, 1, 0x1130, 0x1138, 0x113c] {
            export.extend(v.to_le_bytes());
        }
        put(&mut image, 0x1100, &export);
        put(&mut image, 0x1130, &0x1150u32.to_le_bytes()); // EAT[0]
        put(&mut image, 0x1134, &0u32.to_le_bytes()); // EAT[1] unused
        put(&mut image, 0x1138, &0x1160u32.to_le_bytes()); // name RVA
        put(&mut image, 0x113c, &0u16.to_le_bytes()); // ordinal index
        put(&mut image, 0x1160, b"Hello\0");
        put(&mut image, 0x1180, b"test.dll\0");
        // imports: kernel32!Sleep and ordinal 7
        let mut descriptor = Vec::new();
        for v in [0x1230u32, 0, 0, 0x1280, 0x1240] {
            descriptor.extend(v.to_le_bytes());
        }
        put(&mut image, 0x1200, &descriptor);
        put(&mut image, 0x1280, b"kernel32.dll\0");
        put(&mut image, 0x1230, &0x1250u32.to_le_bytes());
        put(&mut image, 0x1234, &0x8000_0007u32.to_le_bytes());
        put(&mut image, 0x1250, &[0, 0]);
        put(&mut image, 0x1252, b"Sleep\0");
        image
    }

    #[test]
    fn reads_imports_exports_and_sections() {
        let report = inspect(&synthetic_dll()).expect("synthetic dll");
        assert_eq!(report.get("machine"), Some(&J::Int(0x14c)));
        assert_eq!(report.get("dll"), Some(&J::Bool(true)));
        assert_eq!(report.get("entry_va"), Some(&J::Int(0x1000_1000)));
        let imports = match report.get("imports").unwrap() {
            J::Arr(a) => a,
            _ => panic!(),
        };
        assert_eq!(imports[0].get("library").unwrap().as_str(), Some("kernel32.dll"));
        match imports[0].get("members").unwrap() {
            J::Arr(m) => {
                assert_eq!(m[0].get("name").unwrap().as_str(), Some("Sleep"));
                assert_eq!(m[1].get("ordinal"), Some(&J::Int(7)));
            }
            _ => panic!(),
        }
        match report.get("exports").unwrap() {
            J::Arr(e) => {
                assert_eq!(e.len(), 1);
                assert_eq!(e[0].get("ordinal"), Some(&J::Int(5)));
                assert_eq!(e[0].get("va"), Some(&J::Int(0x1000_1150)));
            }
            _ => panic!(),
        }
        assert_eq!(report.get("unmapped_tail_bytes"), Some(&J::Int(0x600 - 0x600)));
    }

    #[test]
    fn rejects_truncated_and_non_pe_input() {
        assert!(inspect(b"not a pe").is_err());
        let mut dll = synthetic_dll();
        dll.truncate(0x100);
        assert!(inspect(&dll).is_err());
    }
}
