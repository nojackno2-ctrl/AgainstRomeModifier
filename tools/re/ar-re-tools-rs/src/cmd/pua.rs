//! `probe_pua.py`: verify ARCP/PUA metadata and decoded CRCs without extracting members.
//!
//! The XOR table is read from the caller-supplied, fingerprint-checked game EXE. No bundled
//! game data or key table; inputs are never modified.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::path::Path;

pub fn xor_table(exe: &[u8]) -> R<Vec<u8>> {
    require_exe(exe)?;
    // The original reads the table with its own bounded section walk (VA 0x62f060, 64 bytes).
    Ok(read_va(exe, 0x62f060, 64).map_err(|_| "XOR table is not file-backed".to_string())?.to_vec())
}

pub fn inspect(data: &[u8], key: &[u8]) -> R<J> {
    req(data.len() >= 58, "short PUA header")?;
    let words: Vec<u32> = (0..14).map(|i| u32_at(data, i * 4)).collect::<R<_>>()?;
    let version_bytes = [data[56], data[57]];
    req(words[0] == 0x5043_5241, "not an ARCP archive")?;
    req(words[4] == 58 && words[6] == 28, "unsupported header or record layout")?;
    let (count, table, names_offset) = (words[8] as usize, words[4] as usize, words[10] as usize);
    req(count <= (data.len() - table) / 28, "entry table out of bounds")?;
    let table_end = table + count * 28;
    let names_end = names_offset + words[11] as usize + words[12] as usize;
    req(table_end <= names_offset && names_offset <= names_end && names_end <= data.len(), "name table out of bounds")?;
    let mut checksum_header = data[..58].to_vec();
    checksum_header[20..24].fill(0);
    let header_crc = crc32_update(crc32(&checksum_header), &data[table..table_end]);
    req(header_crc == words[5], "header/entry-table CRC mismatch")?;
    let names_blob = &data[names_offset..names_end];
    req(crc32(names_blob) == words[13], "names CRC mismatch")?;
    let strings: Vec<&[u8]> = names_blob.split(|&b| b == 0).collect();
    req(strings.len() == count + 3 && strings[strings.len() - 1].is_empty(), "name count/terminator mismatch")?;
    let mut records = J::arr();
    let mut extensions = Counter::new();
    let mut flag_counter = Counter::new();
    for (index, name) in strings[2..strings.len() - 1].iter().enumerate() {
        let base = table + index * 28;
        let f: Vec<u32> = (0..6).map(|i| u32_at(data, base + i * 4)).collect::<R<_>>()?;
        let (flags, extra_flags) = (u16_at(data, base + 24)?, u16_at(data, base + 26)?);
        let (offset, size, stored_size, crc) = (f[0] as usize, f[3] as usize, f[4] as usize, f[5]);
        req(size <= 64 * 1024 * 1024, "member exceeds analysis limit")?;
        req(names_end <= offset && offset + stored_size <= data.len(), "payload out of bounds")?;
        req(flags & !3 == 0 && extra_flags == 0, "unsupported member flags")?;
        let mut payload = data[offset..offset + stored_size].to_vec();
        if flags & 2 != 0 {
            let mut wrapper = vec![0u8; 64];
            wrapper[..5].copy_from_slice(b"PFIL@");
            wrapper[16..20].copy_from_slice(&(size as u32).to_le_bytes());
            wrapper.extend_from_slice(&payload);
            payload = pfil_prefix(&wrapper, size)?.1;
        }
        if flags & 1 != 0 {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= key[i & 63];
            }
        }
        req(payload.len() == size, "decoded length mismatch")?;
        req(crc32(&payload) == crc, &format!("decoded CRC mismatch at entry {index}"))?;
        let path = decode_cp1251(name)?;
        extensions.inc(py_suffix(&path).to_lowercase());
        flag_counter.inc(flags.to_string());
        records.push(jobj! {
            "index" => index,
            "path" => path,
            "path_bytes_hex" => hex(name),
            "offset" => offset,
            "bytes" => size,
            "stored_bytes" => stored_size,
            "flags" => flags,
            "crc32" => format!("{crc:08x}"),
            "sha256" => sha256_hex(&payload),
            "unknown_record_words" => vec![f[1], f[2]],
        });
    }
    let mut header_words: Vec<J> = words.iter().map(|&w| J::from(w)).collect();
    header_words.extend(version_bytes.iter().map(|&b| J::from(b)));
    Ok(jobj! {
        "sha256" => sha256_hex(data),
        "bytes" => data.len(),
        "header_words_and_bytes" => J::Arr(header_words),
        "header_crc_verified" => true,
        "names_crc_verified" => true,
        "decoded_member_crcs_verified" => records.len(),
        "extensions" => extensions.to_json(),
        "flags" => flag_counter.to_json(),
        "members" => records,
        "limitations" => vec![
            "cp1251 is a display convention; path bytes retained separately",
            "unknown record words, version bytes and archive lookup/runtime semantics not inferred",
            "no archive payloads extracted or executed",
        ],
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(3, "probe-pua PUA EXE OUTPUT.json")?;
    let (pua, exe) = (Path::new(&p.pos[0]), Path::new(&p.pos[1]));
    let dirs: Vec<_> = [pua, exe].iter().filter_map(|p| resolve(p).parent().map(Path::to_path_buf)).collect();
    let output = new_output(Path::new(&p.pos[2]), &dirs, "both input directories")?;
    let key = xor_table(&read(exe)?)?;
    let result = inspect(&read(pua)?, &key)?;
    write_new(&output, &(result.pretty(false) + "\n"))?;
    let mut summary = J::obj();
    if let J::Obj(items) = &result {
        for (k, v) in items {
            if k != "members" {
                summary.set(k, v.clone());
            }
        }
    }
    println!("{}", summary.line(false));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    /// Builds a minimal two-member ARCP archive (one plain, one XOR-coded) with valid CRCs.
    pub fn synthetic_pua(key: &[u8]) -> Vec<u8> {
        let plain = b"hello archive".to_vec();
        let secret = b"xor coded payload!".to_vec();
        let coded: Vec<u8> = secret.iter().enumerate().map(|(i, b)| b ^ key[i & 63]).collect();
        let names: Vec<&[u8]> = vec![b"", b"", b"DIR\\one.txt", b"two.BIN", b""];
        let blob: Vec<u8> = names.iter().flat_map(|n| n.iter().copied().chain(std::iter::once(0))).collect();
        let blob = &blob[..blob.len() - 0];
        // strings split on NUL must be count + 3 = 5 pieces with an empty final piece:
        // "" "" "DIR\one.txt" "two.BIN" ""  => joined with NUL and *no* trailing NUL after the last empty piece
        let joined: Vec<u8> = {
            let mut v = Vec::new();
            for (i, n) in names.iter().enumerate() {
                v.extend_from_slice(n);
                if i + 1 < names.len() {
                    v.push(0);
                }
            }
            v
        };
        let _ = blob;
        let table = 58usize;
        let names_offset = table + 2 * 28;
        let payload_offset = names_offset + joined.len();
        let mut entries = Vec::new();
        for (offset, plain_len, stored, crc, flags) in [
            (payload_offset, plain.len(), plain.len(), crc32(&plain), 0u16),
            (payload_offset + plain.len(), secret.len(), coded.len(), crc32(&secret), 1u16),
        ] {
            for v in [offset as u32, 7, 9, plain_len as u32, stored as u32, crc] {
                entries.extend(v.to_le_bytes());
            }
            entries.extend(flags.to_le_bytes());
            entries.extend(0u16.to_le_bytes());
        }
        let mut header = vec![0u8; 58];
        let words = [0x5043_5241u32, 0, 0, 0, 58, 0, 28, 0, 2, 0, names_offset as u32, joined.len() as u32, 0, crc32(&joined)];
        for (i, w) in words.iter().enumerate() {
            header[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let crc = crc32_update(crc32(&header), &entries);
        header[20..24].copy_from_slice(&crc.to_le_bytes());
        let mut out = header;
        out.extend(entries);
        out.extend(&joined);
        out.extend(&plain);
        out.extend(&coded);
        out
    }

    fn key() -> Vec<u8> {
        (0..64).map(|i| (i * 7 + 3) as u8).collect()
    }

    #[test]
    fn verifies_synthetic_archive() {
        let report = inspect(&synthetic_pua(&key()), &key()).unwrap();
        assert_eq!(report.get("decoded_member_crcs_verified"), Some(&J::Int(2)));
        let members = match report.get("members").unwrap() {
            J::Arr(m) => m,
            _ => panic!(),
        };
        assert_eq!(members[0].get("path").unwrap().as_str(), Some("DIR\\one.txt"));
        assert_eq!(members[1].get("sha256").unwrap().as_str(), Some(sha256_hex(b"xor coded payload!").as_str()));
        assert_eq!(report.get("extensions").unwrap().get(".bin"), Some(&J::Int(1)));
    }

    #[test]
    fn detects_corruption() {
        let archive = synthetic_pua(&key());
        let mut bad_payload = archive.clone();
        let last = bad_payload.len() - 1;
        bad_payload[last] ^= 1;
        assert!(inspect(&bad_payload, &key()).unwrap_err().contains("decoded CRC mismatch"));
        let mut bad_header = archive.clone();
        bad_header[40] ^= 1;
        assert!(inspect(&bad_header, &key()).is_err());
        assert!(inspect(&archive, &[0u8; 64]).unwrap_err().contains("CRC mismatch"));
        assert!(inspect(&archive[..30], &key()).is_err());
        let mut not_arcp = archive;
        not_arcp[0] = b'X';
        assert_eq!(inspect(&not_arcp, &key()).unwrap_err(), "not an ARCP archive");
    }

    #[test]
    fn xor_table_requires_known_exe() {
        assert!(xor_table(b"MZ").unwrap_err().contains("fingerprint"));
        let _ = TempDir::new();
    }

    #[test]
    fn crc32_matches_zlib_reference() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
