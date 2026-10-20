//! `inventory_game_directory.py`: read-only installed-game inventory.
//!
//! Never executes binaries or extracts archives. SAVE, screenshots, backups and runtime
//! logs are excluded from content analysis. PFIL inspection decodes only a bounded prefix.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn signature(data: &[u8]) -> &'static str {
    for (prefix, name) in [
        (&b"PFIL@"[..], "PFIL@"),
        (b"PK\x03\x04", "ZIP-local-header"),
        (b"MZ", "DOS-MZ"),
        (b"BCI0", "BCI0"),
        (b"ALRA", "ALRA"),
        (b"APAT", "APAT"),
        (b"ARCP", "ARCP"),
        (b"RIFF", "RIFF"),
        (b"BM", "BMP"),
    ] {
        if data.starts_with(prefix) {
            return name;
        }
    }
    "other"
}

/// Files below `root` as (relative parts, absolute path), sorted like `sorted(Path.rglob('*'))`.
fn walk(root: &Path) -> R<Vec<(Vec<String>, PathBuf)>> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), Vec::<String>::new())];
    while let Some((dir, parts)) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| io(e, &dir))? {
            let entry = entry.map_err(|e| io(e, &dir))?;
            let path = entry.path();
            let mut next = parts.clone();
            next.push(entry.file_name().to_string_lossy().into_owned());
            let meta = fs::symlink_metadata(&path).map_err(|e| io(e, &path))?;
            if meta.is_dir() && !meta.file_type().is_symlink() {
                stack.push((path, next));
            } else if meta.is_file() || meta.file_type().is_symlink() {
                out.push((next, path));
            }
        }
    }
    out.sort_by_key(|(parts, _)| parts.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>());
    Ok(out)
}

fn archive_record(path: &Path) -> R<J> {
    let file = fs::File::open(path).map_err(|e| io(e, path))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut extensions = Counter::new();
    let mut total = 0u128;
    let mut encrypted = 0u64;
    let mut samples: Vec<(String, Vec<J>)> = Vec::new();
    for index in 0..archive.len() {
        let (name, size, is_dir) = {
            let entry = archive.by_index_raw(index).map_err(|e| e.to_string())?;
            (entry.name().to_string(), entry.size(), entry.is_dir())
        };
        let ext = py_suffix(&name).to_lowercase();
        extensions.inc(&ext);
        total += size as u128;
        if samples.iter().all(|(e, _)| *e != ext) {
            samples.push((ext.clone(), Vec::new()));
        }
        let group = samples.iter_mut().find(|(e, _)| *e == ext).unwrap();
        if group.1.len() < 3 && !is_dir {
            match archive.by_index(index) {
                Ok(mut stream) => {
                    let mut head = Vec::new();
                    (&mut stream).take(64).read_to_end(&mut head).map_err(|e| e.to_string())?;
                    group.1.push(jobj! {
                        "path" => name,
                        "signature" => signature(&head),
                        "prefix_hex" => hex(&head[..head.len().min(16)]),
                    });
                }
                Err(zip::result::ZipError::UnsupportedArchive(_)) => encrypted += 1,
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    let samples = J::Obj(samples.into_iter().map(|(e, v)| (e, J::Arr(v))).collect());
    Ok(jobj! {
        "entries" => archive.len(),
        "extensions" => extensions.to_json(),
        "uncompressed_bytes" => total,
        "encrypted_entries" => encrypted,
        "samples" => samples,
        "crc_verified" => false,
    })
}

pub fn inventory(root: &Path) -> R<(J, usize)> {
    let mut records = J::arr();
    let mut errors = J::arr();
    let mut excluded = Counter::new();
    let mut ext_counter = Counter::new();
    let mut sig_counter = Counter::new();
    let mut payload_counter = Counter::new();
    let mut total_bytes = 0u128;
    let mut file_count = 0usize;
    let canonical_root = resolve(root);
    for (parts, path) in walk(root)? {
        let rel = parts.join("/");
        let meta = fs::symlink_metadata(&path).map_err(|e| io(e, &path))?;
        if meta.file_type().is_symlink() || !is_inside(&resolve(&path), &canonical_root) {
            excluded.inc("links");
            continue;
        }
        let ext = py_suffix(parts.last().unwrap()).to_lowercase();
        let first = parts[0].to_uppercase();
        if ["SAVE", "SCRNSHOT", "TOENG", ".AGAINST-ROME-MODIFIER-LANGUAGE-BACKUP"].contains(&first.as_str())
            || [".bak", ".log", ".dmp", ".arm_original"].contains(&ext.as_str())
            || ext.starts_with(".dm")
        {
            excluded.inc("save/screenshot/backup/runtime");
            continue;
        }
        let outcome: R<J> = (|| {
            let mut prefix = Vec::new();
            fs::File::open(&path)
                .and_then(|f| f.take(4096).read_to_end(&mut prefix))
                .map_err(|e| io(e, &path))?;
            let mut record = jobj! {
                "path" => rel.clone(),
                "bytes" => meta.len(),
                "extension" => ext.clone(),
                "signature" => signature(&prefix),
            };
            let mut payload_sig = None;
            if prefix.starts_with(b"PFIL@") {
                let (size, payload) = pfil_prefix(&prefix, 32)?;
                let sig = signature(&payload);
                record.set("uncompressed_bytes", size);
                record.set("payload_signature", sig);
                record.set("payload_prefix_hex", hex(&payload[..payload.len().min(16)]));
                payload_sig = Some(sig);
            }
            if ext == ".exe" || ext == ".dll" || (parts.len() == 1 && ext == ".dat") {
                record.set("sha256", sha256_file(&path)?);
            }
            if parts.len() == 1 && prefix.starts_with(b"PK\x03\x04") {
                record.set("archive", archive_record(&path)?);
            }
            ext_counter.inc(&ext);
            sig_counter.inc(signature(&prefix));
            if let Some(sig) = payload_sig {
                payload_counter.inc(sig);
            }
            total_bytes += meta.len() as u128;
            file_count += 1;
            Ok(record)
        })();
        match outcome {
            Ok(record) => records.push(record),
            Err(error) => errors.push(jobj! { "path" => rel, "error" => error }),
        }
    }
    let summary = jobj! {
        "files" => file_count,
        "bytes" => total_bytes,
        "extensions" => ext_counter.to_json(),
        "signatures" => sig_counter.to_json(),
        "payload_signatures" => payload_counter.to_json(),
    };
    let error_count = errors.len();
    let report = jobj! {
        "root" => root.display().to_string(),
        "scope" => "read-only shipped data plus custom maps; saves and backups excluded",
        "limitations" => vec![
            "PFIL prefix only, not full decompression/integrity",
            "ZIP central directory and at most 3 prefix samples per extension; no full CRC pass",
            "Signatures and filenames do not establish runtime semantics",
        ],
        "files" => records,
        "excluded" => excluded.to_json(),
        "errors" => errors,
        "summary" => summary,
    };
    Ok((report, error_count))
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "inventory-game-directory GAME OUTPUT.json")?;
    let root = resolve(Path::new(&p.pos[0]));
    if !root.is_dir() {
        return Err(usage("game directory does not exist"));
    }
    let output = new_output(Path::new(&p.pos[1]), &[root.clone()], "the game directory")?;
    let (report, errors) = inventory(&root)?;
    write_new(&output, &(report.pretty(false) + "\n"))?;
    let line = jobj! {
        "output" => output.display().to_string(),
        "summary" => report.get("summary").cloned().unwrap_or(J::Null),
        "errors" => errors,
    };
    println!("{}", line.line(false));
    Ok(if errors > 0 { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn inventories_synthetic_game() {
        let dir = TempDir::new();
        write(dir.path(), "Game.exe", b"MZ\x90\x00 fake");
        write(dir.path(), "SAVE/slot1.sav", b"secret");
        write(dir.path(), "notes.log", b"log");
        write(dir.path(), "a.dat", b"ARCPxxxx");
        write(dir.path(), "MAPS/M1/DATA/light.dat", &pfil_store(b"hello pfil payload!!"));
        let (report, errors) = inventory(dir.path()).unwrap();
        assert_eq!(errors, 0);
        let summary = report.get("summary").unwrap();
        assert_eq!(summary.get("files"), Some(&J::Int(3)));
        assert_eq!(report.get("excluded").unwrap().get("save/screenshot/backup/runtime"), Some(&J::Int(2)));
        let files = match report.get("files").unwrap() {
            J::Arr(a) => a,
            _ => panic!(),
        };
        let pfil = files.iter().find(|f| f.get("signature").and_then(J::as_str) == Some("PFIL@")).unwrap();
        assert_eq!(pfil.get("uncompressed_bytes"), Some(&J::Int(20)));
        assert_eq!(pfil.get("payload_signature").and_then(J::as_str), Some("other"));
        let exe = files.iter().find(|f| f.get("extension").and_then(J::as_str) == Some(".exe")).unwrap();
        assert!(exe.get("sha256").is_some());
        let dat = files.iter().find(|f| f.get("path").and_then(J::as_str) == Some("a.dat")).unwrap();
        assert!(dat.get("sha256").is_some(), "root-level .dat is hashed");
        assert_eq!(dat.get("signature").and_then(J::as_str), Some("ARCP"));
    }

    #[test]
    fn output_inside_game_is_refused() {
        let dir = TempDir::new();
        let inside = dir.path().join("inv.json");
        assert!(run(&[dir.path().display().to_string(), inside.display().to_string()]).unwrap_err().starts_with("usage:"));
    }
}
