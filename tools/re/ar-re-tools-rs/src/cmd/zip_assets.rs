//! `probe_zip_assets.py`: stream-check the eight installed ZIP asset packages without extracting files.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

pub const PACKAGES: [&str; 8] = ["alr.dat", "apt.dat", "floortex.dat", "gui.dat", "mp.dat", "sfx.dat", "shad.dat", "voice.dat"];

pub fn signature(prefix: &[u8]) -> String {
    if prefix.len() >= 8 && (&prefix[..4] == b"ALRA" || &prefix[..4] == b"APAT") {
        return format!("{}:word4={}", String::from_utf8_lossy(&prefix[..4]), u32_at(prefix, 4).unwrap());
    }
    if prefix.len() >= 12 && &prefix[..4] == b"RIFF" {
        let tag: String = prefix[8..12].iter().map(|&b| if b < 0x80 { b as char } else { '\u{fffd}' }).collect();
        return format!("RIFF:{tag}");
    }
    if prefix.starts_with(b"BM") {
        return "BMP".into();
    }
    // TGA has no reliable fixed magic; do not infer its format from extension.
    "other".into()
}

fn method_number(method: zip::CompressionMethod) -> i128 {
    match method {
        zip::CompressionMethod::Stored => 0,
        zip::CompressionMethod::Deflated => 8,
        _ => -1,
    }
}

/// Inspects the given packages (names relative to `root`).
pub fn inspect_packages(root: &Path, packages: &[&str]) -> R<J> {
    let mut reports = J::arr();
    let mut paths: BTreeMap<String, Vec<J>> = BTreeMap::new();
    let mut total: u128 = 0;
    let mut verified_total = 0usize;
    for name in packages {
        let source = root.join(name);
        let source_hash = sha256_file(&source)?;
        let file = std::fs::File::open(&source).map_err(|e| io(e, &source))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let declared: u128 = (0..archive.len())
            .map(|i| archive.by_index_raw(i).map(|e| e.size() as u128).map_err(|e| e.to_string()))
            .sum::<R<u128>>()?;
        total += declared;
        for i in 0..archive.len() {
            if archive.by_index_raw(i).map_err(|e| e.to_string())?.size() > 512 * 1024 * 1024 {
                return Err("archive exceeds analysis size limit".into());
            }
        }
        req(total <= 10 * 1024u128.pow(3), "archive exceeds analysis size limit")?;
        let (mut members, mut methods, mut signatures) = (J::arr(), Counter::new(), Counter::new());
        let mut buffer = vec![0u8; 1 << 20];
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
            let (filename, is_dir) = (entry.name().to_string(), entry.is_dir());
            let (declared_size, stored, crc, method) = (entry.size(), entry.compressed_size(), entry.crc32(), method_number(entry.compression()));
            let mut digest = Sha256::new();
            let mut length = 0u64;
            let mut prefix: Vec<u8> = Vec::new();
            // ZipExtFile-style CRC validation happens when EOF is reached; directories are streamed too.
            loop {
                let n = entry.read(&mut buffer).map_err(|e| format!("{name}/{filename}: {e}"))?;
                if n == 0 {
                    break;
                }
                digest.update(&buffer[..n]);
                length += n as u64;
                let wanted = 32usize.saturating_sub(prefix.len());
                prefix.extend_from_slice(&buffer[..n.min(wanted)]);
            }
            if length != declared_size {
                return Err(format!("decoded length mismatch: {name}/{filename}"));
            }
            let sha = hex(&digest.finalize());
            let sig = signature(&prefix);
            methods.inc(method.to_string());
            signatures.inc(&sig);
            members.push(jobj! {
                "path" => filename.clone(),
                "directory" => is_dir,
                "bytes" => length,
                "stored_bytes" => stored,
                "method" => method,
                "crc32" => format!("{crc:08x}"),
                "sha256" => sha.clone(),
                "signature" => sig,
            });
            if !is_dir {
                paths.entry(filename.replace('\\', "/").to_lowercase()).or_default().push(jobj! {
                    "package" => *name, "path" => filename, "sha256" => sha,
                });
            }
        }
        verified_total += archive.len();
        reports.push(jobj! {
            "package" => *name,
            "sha256" => source_hash,
            "bytes" => std::fs::metadata(&source).map_err(|e| io(e, &source))?.len(),
            "entries_crc_verified" => members.len(),
            "decoded_bytes" => declared,
            "methods" => methods.to_json(),
            "signatures" => signatures.to_json(),
            "members" => members,
        });
    }
    let mut overlaps = J::arr();
    for (path, matches) in paths {
        if matches.len() > 1 {
            let mut hashes: Vec<&str> = matches.iter().filter_map(|m| m.get("sha256").and_then(J::as_str)).collect();
            hashes.sort_unstable();
            hashes.dedup();
            overlaps.push(jobj! {
                "normalized_path" => path,
                "same_payload" => hashes.len() == 1,
                "members" => J::Arr(matches),
            });
        }
    }
    Ok(jobj! {
        "packages" => reports,
        "entries_crc_verified" => verified_total,
        "decoded_bytes" => total,
        "normalized_path_collisions" => overlaps,
        "limitations" => vec![
            "CRC and byte counts establish ZIP decoding, not full format validity or runtime use",
            "casefold path normalization is a comparison convention, not native lookup emulation",
            "no payload extraction or execution; TGA not inferred by extension",
        ],
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-zip-assets ROOT OUTPUT.json")?;
    let root = Path::new(&p.pos[0]);
    let output = new_output(Path::new(&p.pos[1]), &[resolve(root)], "source directory")?;
    let result = inspect_packages(root, &PACKAGES)?;
    write_new(&output, &(result.pretty(false) + "\n"))?;
    let mut head = J::obj();
    if let J::Obj(items) = &result {
        for (k, v) in items {
            if k != "packages" && k != "normalized_path_collisions" {
                head.set(k, v.clone());
            }
        }
    }
    println!("{}", head.line(false));
    println!(
        "{}",
        jobj! {
            "packages" => result.get("packages").map_or(0, J::len),
            "path_collisions" => result.get("normalized_path_collisions").map_or(0, J::len),
        }
        .line(false)
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;
    use std::io::Write;

    pub fn make_zip(path: &Path, entries: &[(&str, &[u8], bool)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, data, deflate) in entries {
            let options = zip::write::FileOptions::default().compression_method(if *deflate {
                zip::CompressionMethod::Deflated
            } else {
                zip::CompressionMethod::Stored
            });
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn checks_crc_signatures_and_path_collisions() {
        let dir = TempDir::new();
        let mut alra = b"ALRA".to_vec();
        alra.extend(7u32.to_le_bytes());
        alra.extend(vec![1u8; 100]);
        make_zip(&dir.path().join("a.zip"), &[("Sprites/One.alr", &alra, true), ("x.bmp", b"BM1234567890", false)]);
        make_zip(&dir.path().join("b.zip"), &[("sprites\\one.alr", b"different", true), ("riff.wav", b"RIFF\x04\0\0\0WAVEdata", false)]);
        let report = inspect_packages(dir.path(), &["a.zip", "b.zip"]).unwrap();
        assert_eq!(report.get("entries_crc_verified"), Some(&J::Int(4)));
        let collisions = match report.get("normalized_path_collisions").unwrap() {
            J::Arr(c) => c,
            _ => panic!(),
        };
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].get("same_payload"), Some(&J::Bool(false)));
        let first = match report.get("packages").unwrap() {
            J::Arr(p) => p[0].clone(),
            _ => panic!(),
        };
        assert_eq!(first.get("signatures").unwrap().get("ALRA:word4=7"), Some(&J::Int(1)));
        assert_eq!(first.get("signatures").unwrap().get("BMP"), Some(&J::Int(1)));
        assert_eq!(first.get("methods").unwrap().get("8"), Some(&J::Int(1)));
        assert_eq!(first.get("methods").unwrap().get("0"), Some(&J::Int(1)));
    }

    #[test]
    fn corrupt_member_fails_crc() {
        let dir = TempDir::new();
        let path = dir.path().join("c.zip");
        make_zip(&path, &[("stored.bin", b"AAAAAAAAAAAAAAAA", false)]);
        let mut bytes = std::fs::read(&path).unwrap();
        let at = bytes.windows(16).position(|w| w == b"AAAAAAAAAAAAAAAA").unwrap();
        bytes[at + 3] = b'B';
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect_packages(dir.path(), &["c.zip"]).is_err());
    }

    #[test]
    fn riff_signature_includes_form_type() {
        assert_eq!(signature(b"RIFF\x00\x00\x00\x00WAVEfmt "), "RIFF:WAVE");
        assert_eq!(signature(b"xx"), "other");
    }
}
