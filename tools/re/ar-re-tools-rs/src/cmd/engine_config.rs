//! `probe_engine_config.py`: reads the fingerprinted game's encoded engine section table;
//! never executes it.

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::util::*;
use std::path::Path;

fn ascii(bytes: &[u8]) -> R<String> {
    req(bytes.is_ascii(), "'ascii' codec can't decode byte")?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

/// Decodes one encoded section prefix (`((c ^ 0x39) >> 3) | ((c ^ 0x39) << 5)`).
pub fn decode_section(encoded: &[u8]) -> R<Vec<u8>> {
    let decoded: Vec<u8> = encoded.iter().map(|&c| (c ^ 0x39).rotate_right(3)).collect();
    let again: Vec<u8> = decoded.iter().map(|&c| c.rotate_left(3) ^ 0x39).collect();
    req(again == encoded, "section roundtrip failed")?;
    Ok(decoded)
}

pub fn inspect(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let table = read_va(exe, 0x632b98, 36 * 22)?;
    let mut sections = J::arr();
    let mut prefixes = Vec::new();
    for index in 0..36 {
        let record = &table[index * 22..(index + 1) * 22];
        let end = record.iter().position(|&b| b == 0x39).ok_or("unterminated encoded section")?;
        let decoded = decode_section(&record[..end])?;
        let prefix = ascii(&decoded)?;
        sections.push(jobj! { "index" => index, "prefix" => prefix.clone() });
        prefixes.push(prefix);
    }
    // Extract only header metadata from the embedded defaults, never full game data.
    let default_block = read_va(exe, 0x615e40, 4096)?;
    let default = ascii(default_block.split(|&b| b == 0).next().unwrap_or(&[]))?;
    let mut recognized = J::arr();
    for line in splitlines(&default) {
        let header = line.trim().to_lowercase();
        if !header.starts_with('[') {
            continue;
        }
        let matches: Vec<usize> = (0..36).filter(|&i| header.starts_with(prefixes[i].as_str())).collect();
        req(!matches.is_empty() && matches[0] != 35, "unrecognized default header")?;
        recognized.push(jobj! { "header" => header, "section_index" => matches[0] });
    }
    let cstring = |address: u64| -> R<String> {
        let data = read_va(exe, address, 100)?;
        req(data.contains(&0), "unterminated option text")?;
        ascii(data.split(|&b| b == 0).next().unwrap_or(&[]))
    };
    let mut options = J::arr();
    for index in 0..40u64 {
        let record = read_va(exe, 0x61a444 + index * 12, 12)?;
        let (case, name, alias) = (u32_at(record, 0)?, u32_at(record, 4)?, u32_at(record, 8)?);
        options.push(jobj! {
            "index" => index,
            "parser_case" => case,
            "long_name" => cstring(name as u64)?,
            "short_alias" => cstring(alias as u64)?,
        });
    }
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "table_va" => "0x632b98",
        "record_stride" => 22,
        "sections" => sections,
        "embedded_default_headers" => recognized,
        "game_option_table_va" => "0x61a444",
        "game_option_record_stride" => 12,
        "game_options" => options,
        "registry_selector_resources" => string_resources(exe, &[9998, 9999])?,
        "limitations" => vec![
            "static metadata only; no current process or effective configuration observed",
            "index 35 is the generic opening-bracket fallback; prefixes are not corrected",
            "not a full configuration parser or a runtime emulator",
        ],
    })
}

/// Specified RT_STRING entries, including empty entries and every language.
pub fn string_resources(exe: &[u8], ids: &[u32]) -> R<J> {
    let nt = u32_at(exe, 0x3c)? as usize;
    let opt = nt + 24;
    let base = u32_at(exe, opt + 28)? as u64;
    let (rva, size) = (u32_at(exe, opt + 112)? as u64, u32_at(exe, opt + 116)? as usize);
    let root = read_va(exe, base + rva, size)?;
    let entries = |offset: usize| -> R<Vec<(u32, u32)>> {
        req(offset + 16 <= root.len(), "resource directory out of bounds")?;
        let (named, numbered) = (u16_at(root, offset + 12)? as usize, u16_at(root, offset + 14)? as usize);
        let count = named + numbered;
        req(offset + 16 + count * 8 <= root.len(), "resource entries out of bounds")?;
        (0..count).map(|i| Ok((u32_at(root, offset + 16 + i * 8)?, u32_at(root, offset + 20 + i * 8)?))).collect()
    };
    let types: Vec<u32> = entries(0)?.into_iter().filter(|(key, _)| *key == 6).map(|(_, v)| v).collect();
    req(types.len() == 1 && types[0] & 0x8000_0000 != 0, "missing RT_STRING directory")?;
    let blocks = entries((types[0] & 0x7fff_ffff) as usize)?;
    let mut result = J::arr();
    for &resource_id in ids {
        let matches: Vec<u32> = blocks.iter().filter(|(key, _)| *key == resource_id / 16 + 1).map(|(_, v)| *v).collect();
        req(matches.len() == 1 && matches[0] & 0x8000_0000 != 0, "missing string block")?;
        for (language, leaf) in entries((matches[0] & 0x7fff_ffff) as usize)? {
            req(leaf & 0x8000_0000 == 0 && leaf as usize + 16 <= root.len(), "invalid resource leaf")?;
            let (data_rva, length) = (u32_at(root, leaf as usize)? as u64, u32_at(root, leaf as usize + 4)? as usize);
            let data = read_va(exe, base + data_rva, length)?;
            let mut offset = 0usize;
            for index in 0..16u32 {
                req(offset + 2 <= data.len(), "truncated string length")?;
                let chars = u16_at(data, offset)? as usize;
                offset += 2;
                req(offset + chars * 2 <= data.len(), "truncated string data")?;
                if index == resource_id % 16 {
                    let units: Vec<u16> = (0..chars).map(|i| u16_at(data, offset + i * 2)).collect::<R<_>>()?;
                    let text = String::from_utf16(&units).map_err(|_| "invalid UTF-16 string")?;
                    result.push(jobj! { "id" => resource_id, "language" => language, "text" => text });
                }
                offset += chars * 2;
            }
        }
    }
    Ok(result)
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-engine-config EXE OUTPUT.json")?;
    let exe = Path::new(&p.pos[0]);
    let exe_dir = resolve(exe).parent().map(Path::to_path_buf).unwrap_or_default();
    let output = new_output(Path::new(&p.pos[1]), &[exe_dir], "input directory")?;
    let result = inspect(&read(exe)?)?;
    write_new(&output, &(result.pretty(true) + "\n"))?;
    println!(
        "{}",
        jobj! {
            "sections" => result.get("sections").map_or(0, J::len),
            "default_headers" => result.get("embedded_default_headers").map_or(0, J::len),
            "game_options" => result.get("game_options").map_or(0, J::len),
            "registry_selector_entries" => result.get("registry_selector_resources").map_or(0, J::len),
        }
        .line(true)
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(decoded: &[u8]) -> Vec<u8> {
        decoded.iter().map(|&c| c.rotate_left(3) ^ 0x39).collect()
    }

    #[test]
    fn section_codec_round_trips() {
        for text in [&b"[engine"[..], b"[sound", b"[", b"a_b-1"] {
            assert_eq!(decode_section(&encode(text)).unwrap(), text);
        }
    }

    #[test]
    fn gate_rejects_foreign_exe() {
        assert!(inspect(b"MZ").unwrap_err().contains("fingerprint"));
    }

    /// Builds an in-memory PE32 with an RT_STRING resource: block 625 (ids 9984..9999),
    /// holding strings at positions 14 and 15, in one language (0x409).
    fn exe_with_strings() -> Vec<u8> {
        let mut image = vec![0u8; 0x800];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        image[0x94..0x96].copy_from_slice(&224u16.to_le_bytes());
        let opt = 0x98;
        image[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        image[opt + 28..opt + 32].copy_from_slice(&0x400000u32.to_le_bytes());
        image[opt + 112..opt + 116].copy_from_slice(&0x1000u32.to_le_bytes()); // resource RVA
        image[opt + 116..opt + 120].copy_from_slice(&0x100u32.to_le_bytes());
        let section = opt + 224;
        image[section + 8..section + 12].copy_from_slice(&0x400u32.to_le_bytes());
        image[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[section + 16..section + 20].copy_from_slice(&0x400u32.to_le_bytes());
        image[section + 20..section + 24].copy_from_slice(&0x400u32.to_le_bytes());
        let put32 = |image: &mut Vec<u8>, at: usize, v: u32| image[0x400 + at..0x400 + at + 4].copy_from_slice(&v.to_le_bytes());
        let put16 = |image: &mut Vec<u8>, at: usize, v: u16| image[0x400 + at..0x400 + at + 2].copy_from_slice(&v.to_le_bytes());
        // root dir: one id entry (type 6) -> dir at 0x18
        put16(&mut image, 14, 1);
        put32(&mut image, 16, 6);
        put32(&mut image, 20, 0x8000_0018);
        // type dir: one id entry (block id 9998/16+1 = 625) -> dir at 0x30
        put16(&mut image, 0x18 + 14, 1);
        put32(&mut image, 0x18 + 16, 625);
        put32(&mut image, 0x18 + 20, 0x8000_0030);
        // language dir: one entry (0x409) -> leaf at 0x48
        put16(&mut image, 0x30 + 14, 1);
        put32(&mut image, 0x30 + 16, 0x409);
        put32(&mut image, 0x30 + 20, 0x48);
        // leaf: data RVA 0x1060, size
        let mut block = Vec::new();
        for index in 0..16u16 {
            let text: Vec<u16> = match index {
                14 => "ab".encode_utf16().collect(),
                15 => "\u{e9}z".encode_utf16().collect(),
                _ => vec![],
            };
            block.extend((text.len() as u16).to_le_bytes());
            for unit in text {
                block.extend(unit.to_le_bytes());
            }
        }
        put32(&mut image, 0x48, 0x1060);
        put32(&mut image, 0x48 + 4, block.len() as u32);
        image[0x400 + 0x60..0x400 + 0x60 + block.len()].copy_from_slice(&block);
        image
    }

    #[test]
    fn reads_rt_string_entries() {
        let exe = exe_with_strings();
        let result = string_resources(&exe, &[9998, 9999]).unwrap();
        match result {
            J::Arr(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].get("text").unwrap().as_str(), Some("ab"));
                assert_eq!(items[1].get("text").unwrap().as_str(), Some("\u{e9}z"));
                assert_eq!(items[1].get("language"), Some(&J::Int(0x409)));
            }
            _ => panic!(),
        }
        assert!(string_resources(&exe, &[16]).is_err(), "missing block is an error");
    }
}
