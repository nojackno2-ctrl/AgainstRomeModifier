//! `probe_wise_media.py`: read-only audit of the known Wise split installer inside two ISO9660 images.
//!
//! InstallFile candidates use the published SabreTools layout, not a full Wise VM parser. Every
//! accepted file must pass compressed bounds, EOF, length and CRC. No installer actions are
//! executed. Only explicitly selected files are extracted (into the new output directory).

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::util::*;
use flate2::{Decompress, FlushDecompress, Status};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const ISO_HASHES: [&str; 2] = [
    "b48bcd4675bb20ae782c35480c40ffc53e3bb663414e2aa7a70517851d285484",
    "d66b47df8d62bc0ea2ee5af3309eafb6a828871ec085e9ef2e831b8c4b2e99d2",
];
pub const BASE: u64 = 0x30fcf;

/// A random-access byte source (an ISO member or the concatenation of volumes).
pub trait Source {
    fn size(&self) -> u64;
    fn read(&mut self, offset: u64, count: u64) -> R<Vec<u8>>;
}

pub struct IsoMember {
    file: File,
    offset: u64,
    size: u64,
}

fn iso_fields(record: &[u8]) -> R<(u64, u64)> {
    req(record.len() >= 18, "invalid directory record")?;
    let (extent, size) = (u32_at(record, 2)?, u32_at(record, 10)?);
    let (be_extent, be_size) = (u32::from_be_bytes(record[6..10].try_into().unwrap()), u32::from_be_bytes(record[14..18].try_into().unwrap()));
    req(extent == be_extent && size == be_size, "ISO endian fields disagree")?;
    Ok((extent as u64, size as u64))
}

impl IsoMember {
    /// Finds `target` (case-insensitive, `;version` stripped) in the root directory of the ISO.
    pub fn open(path: &Path, target: &str) -> R<Self> {
        let mut file = File::open(path).map_err(|e| io(e, path))?;
        let image_size = file.metadata().map_err(|e| io(e, path))?.len();
        file.seek(SeekFrom::Start(16 * 2048)).map_err(|e| io(e, path))?;
        let mut pvd = vec![0u8; 2048];
        file.read_exact(&mut pvd).map_err(|_| "not primary ISO9660".to_string())?;
        req(&pvd[..7] == b"\x01CD001\x01", "not primary ISO9660")?;
        let (extent, size) = iso_fields(&pvd[156..190])?;
        file.seek(SeekFrom::Start(extent * 2048)).map_err(|e| io(e, path))?;
        let mut directory = vec![0u8; size as usize];
        let n = file.read(&mut directory).map_err(|e| io(e, path))?;
        directory.truncate(n);
        let mut offset = 0usize;
        while offset < directory.len() {
            let count = directory[offset] as usize;
            if count == 0 {
                offset = (offset / 2048 + 1) * 2048;
                continue;
            }
            let record = directory.get(offset..offset + count).ok_or("invalid directory record")?;
            req(count >= 34, "invalid directory record")?;
            let name_bytes = &record[33..33 + record[32] as usize];
            let name = String::from_utf8(name_bytes.to_vec()).map_err(|_| "invalid ISO name")?;
            let name = name.split(';').next().unwrap_or("");
            if name.to_lowercase() == target.to_lowercase() {
                req(record[25] & 0x82 == 0, "directory or multi-extent source not supported")?;
                let (extent, size) = iso_fields(record)?;
                let offset = extent * 2048;
                req(offset + size <= image_size, "ISO member exceeds image")?;
                return Ok(Self { file, offset, size });
            }
            offset += count;
        }
        Err(format!("missing ISO member: {target}"))
    }
}

impl Source for IsoMember {
    fn size(&self) -> u64 {
        self.size
    }
    fn read(&mut self, offset: u64, count: u64) -> R<Vec<u8>> {
        req(offset <= self.size && count <= self.size - offset, "member read outside bounds")?;
        self.file.seek(SeekFrom::Start(self.offset + offset)).map_err(|e| e.to_string())?;
        let mut result = vec![0u8; count as usize];
        self.file.read_exact(&mut result).map_err(|_| "short source read".to_string())?;
        Ok(result)
    }
}

/// Split media: the logical file is the concatenation of the members.
pub struct Composite<S: Source> {
    pub members: Vec<S>,
}

impl<S: Source> Composite<S> {
    pub fn new(members: Vec<S>) -> Self {
        Self { members }
    }
}

impl<S: Source> Source for Composite<S> {
    fn size(&self) -> u64 {
        self.members.iter().map(Source::size).sum()
    }
    fn read(&mut self, mut offset: u64, mut count: u64) -> R<Vec<u8>> {
        req(offset <= self.size() && count <= self.size() - offset, "split read outside bounds")?;
        let mut result = Vec::with_capacity(count as usize);
        for member in &mut self.members {
            if offset >= member.size() {
                offset -= member.size();
                continue;
            }
            let part = count.min(member.size() - offset);
            result.extend(member.read(offset, part)?);
            count -= part;
            offset = 0;
            if count == 0 {
                break;
            }
        }
        req(count == 0, "incomplete split read")?;
        Ok(result)
    }
}

/// Raw-deflate decode of a complete stream; it must end exactly at the end of `data`.
fn inflate_exact(data: &[u8]) -> R<Vec<u8>> {
    let mut decoder = Decompress::new(false);
    let mut out = Vec::with_capacity(data.len() * 3 + 64);
    loop {
        let (in_before, out_before) = (decoder.total_in() as usize, out.len());
        out.reserve(1 << 16);
        let status = decoder.decompress_vec(&data[in_before..], &mut out, FlushDecompress::Finish).map_err(|e| e.to_string())?;
        if status == Status::StreamEnd {
            break;
        }
        if decoder.total_in() as usize == in_before && out.len() == out_before {
            return Err("invalid bootstrap stream".into());
        }
    }
    req(decoder.total_in() as usize == data.len(), "invalid bootstrap stream")?;
    Ok(out)
}

pub struct Bootstrap {
    pub script: Vec<u8>,
    pub blocks: Vec<J>,
}

/// Native entry 4021af reads these blocks; 401edf strips the trailing CRC.
pub fn bootstrap(source: &mut dyn Source, base: u64, cursor_start: usize, sizes: &[usize], script_index: usize, script_len: usize) -> R<Bootstrap> {
    let header = source.read(0, 1_048_576.min(source.size()))?;
    let (mut cursor, mut script, mut blocks) = (cursor_start, None, Vec::new());
    for (index, &packed) in sizes.iter().enumerate() {
        let part = header.get(cursor..(cursor + packed).min(header.len())).unwrap_or(&[]);
        req(part.len() >= 4, "invalid bootstrap stream")?;
        let data = inflate_exact(&part[..part.len() - 4])?;
        let crc = u32_at(part, packed.checked_sub(4).ok_or("invalid bootstrap stream")?)?;
        req(crc32(&data) == crc, "bootstrap CRC mismatch")?;
        blocks.push(jobj! {
            "offset" => cursor, "compressed_bytes" => packed, "bytes" => data.len(), "sha256" => sha256_hex(&data),
        });
        if index == script_index {
            req(data.len() == script_len, "unexpected script size")?;
            script = Some(data);
        }
        cursor += packed;
    }
    req(cursor as u64 == base, "unexpected file data base")?;
    Ok(Bootstrap { script: script.ok_or("unexpected script size")?, blocks })
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub script_offset: usize,
    pub path: String,
    pub start: u64,
    pub end: u64,
    pub bytes: u64,
    pub crc32: u32,
}

/// Finds `%NAME%\path\0` strings with a valid 43-byte InstallFile prefix inside the Wise script.
pub fn candidates(script: &[u8], source_size: u64, base: u64) -> R<Vec<Candidate>> {
    let mut rows = Vec::new();
    let mut i = 0usize;
    while i < script.len() {
        if script[i] != b'%' {
            i += 1;
            continue;
        }
        // %[A-Z_]+%\\[^\x00\r\n]+\x00
        let mut j = i + 1;
        while j < script.len() && (script[j].is_ascii_uppercase() || script[j] == b'_') {
            j += 1;
        }
        let matched = j > i + 1 && script.get(j) == Some(&b'%') && script.get(j + 1) == Some(&b'\\') && {
            let mut k = j + 2;
            while k < script.len() && !matches!(script[k], 0 | b'\r' | b'\n') {
                k += 1;
            }
            k > j + 2 && script.get(k) == Some(&0) && {
                j = k;
                true
            }
        };
        if !matched {
            i += 1;
            continue;
        }
        let (start_match, end_match) = (i, j + 1);
        i = end_match;
        if start_match < 43 {
            continue;
        }
        let offset = start_match - 43;
        let (op, flags) = (script[offset], u16_at(script, offset + 1)?);
        let (start, end) = (u32_at(script, offset + 3)? as u64, u32_at(script, offset + 7)? as u64);
        let size = u32_at(script, offset + 15)? as u64;
        if op != 0 || flags != 128 || !(start < end && end <= source_size.saturating_sub(base)) || !(0 < size && size <= 1_073_741_824) {
            continue;
        }
        let path = decode_cp1251(&script[start_match..end_match - 1])?;
        let crc = u32_at(script, offset + 39)?;
        rows.push(Candidate { script_offset: offset, path, start, end, bytes: size, crc32: crc });
    }
    req(!rows.is_empty(), "no InstallFile candidates found")?;
    Ok(rows)
}

/// Streams one member through raw inflate, checking EOF, length, padding and CRC.
/// Returns (sha256 hex, padding bytes). Optionally writes the data to `destination` (exclusive create).
pub fn validate_file(source: &mut dyn Source, row: &Candidate, base: u64, destination: Option<&Path>) -> R<(String, usize)> {
    let (begin, finish) = (base + row.start, base + row.end);
    req(finish - begin >= 4, "file stream too short")?;
    let mut output = match destination {
        Some(path) => Some(OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| io(e, path))?),
        None => None,
    };
    let mut decoder = Decompress::new(false);
    let (mut hasher, mut crc, mut length) = (Sha256::new(), 0u32, 0u64);
    let mut padding: Vec<u8> = Vec::new();
    let mut finished = false;
    let mut cursor = begin;
    let mut chunk_out = vec![0u8; 1 << 20];
    while cursor < finish - 4 {
        let part = source.read(cursor, 262_144.min(finish - 4 - cursor))?;
        cursor += part.len() as u64;
        let mut pending: &[u8] = &part;
        loop {
            if finished {
                padding.extend_from_slice(pending);
                break;
            }
            let (in_before, out_before) = (decoder.total_in(), decoder.total_out());
            let status = decoder.decompress(pending, &mut chunk_out, FlushDecompress::None).map_err(|e| e.to_string())?;
            let consumed = (decoder.total_in() - in_before) as usize;
            let produced = (decoder.total_out() - out_before) as usize;
            pending = &pending[consumed..];
            length += produced as u64;
            req(length <= row.bytes, "inflated data exceeds declared size")?;
            crc = crc32_update(crc, &chunk_out[..produced]);
            hasher.update(&chunk_out[..produced]);
            if let Some(file) = output.as_mut() {
                file.write_all(&chunk_out[..produced]).map_err(|e| e.to_string())?;
            }
            if status == Status::StreamEnd {
                finished = true;
            } else if pending.is_empty() && produced < chunk_out.len() {
                break;
            } else if consumed == 0 && produced == 0 {
                return Err("invalid deflate stream".into());
            }
        }
        // Some media records (including ar.exe) have a zero pad before CRC.
        req(padding.len() <= 3 && padding.iter().all(|&b| b == 0), "nonzero/excess stream padding")?;
    }
    let trailer = u32_at(&source.read(finish - 4, 4)?, 0)?;
    req(finished && length == row.bytes, "file EOF/size mismatch")?;
    req(crc == trailer && trailer == row.crc32, "file CRC mismatch")?;
    Ok((hex(&hasher.finalize()), padding.len()))
}

fn safe_relative(path: &str) -> Option<String> {
    let relative = path.strip_prefix("%MAINDIR%\\")?;
    relative.split('\\').all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':')).then(|| relative.to_string())
}

pub struct Options {
    pub installed: Option<PathBuf>,
    pub extract: Vec<String>,
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["installed", "extract"])?;
    p.exact(3, "probe-wise-media DISC1 DISC2 OUTPUT_DIR [--installed DIR] [--extract MAINDIR_REL]...")?;
    let discs = [resolve(Path::new(&p.pos[0])), resolve(Path::new(&p.pos[1]))];
    let output = resolve(Path::new(&p.pos[2]));
    req(!output.exists(), "output already exists")?;
    for disc in &discs {
        req(!is_inside(&output, disc.parent().unwrap_or(Path::new(""))), "output in media directory")?;
    }
    let installed = p.opt("installed").map(|d| resolve(Path::new(d)));
    if let Some(dir) = &installed {
        req(!is_inside(&output, dir), "output in installed game")?;
    }
    let hashes = [sha256_file(&discs[0])?, sha256_file(&discs[1])?];
    req(hashes.iter().map(String::as_str).eq(ISO_HASHES), "unsupported media fingerprint")?;
    let selected: BTreeSet<String> = p.all("extract").iter().map(|n| n.replace('/', "\\").to_lowercase()).collect();
    for name in &selected {
        req(!name.starts_with('\\') && name.split('\\').all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':')), "unsafe extraction path")?;
    }
    fs::create_dir(&output).map_err(|e| io(e, &output))?;
    let first = IsoMember::open(&discs[0], "Setup.exe")?;
    let second = IsoMember::open(&discs[1], "Setup.W02")?;
    let first_size = first.size();
    let mut source = Composite::new(vec![first, second]);
    let boot = bootstrap(&mut source, BASE, 0x3a9e, &[0x130c, 0x14ad0, 0x152b9, 0x2397, 0x105], 1, 441_016)?;
    fs::write(output.join("script.bin"), &boot.script).map_err(|e| e.to_string())?;
    let rows = candidates(&boot.script, source.size(), BASE)?;
    let (mut retained, mut compared) = (BTreeSet::new(), 0usize);
    let mut files = J::arr();
    let mut identical = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let relative = safe_relative(&row.path);
        let destination = match &relative {
            Some(rel) if selected.contains(&rel.to_lowercase()) => {
                let mut path = output.join("files");
                for part in rel.split('\\') {
                    path.push(part);
                }
                fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                retained.insert(rel.to_lowercase());
                Some(path)
            }
            _ => None,
        };
        let (sha, padding) = validate_file(&mut source, row, BASE, destination.as_deref())?;
        let mut entry = jobj! {
            "script_offset" => row.script_offset,
            "path" => row.path.clone(),
            "start" => row.start,
            "end" => row.end,
            "bytes" => row.bytes,
            "crc32" => row.crc32,
            "padding_bytes" => padding,
            "sha256" => sha.clone(),
            "crosses_volume_boundary" => BASE + row.start < first_size && first_size < BASE + row.end,
        };
        if let (Some(dir), Some(rel)) = (&installed, &relative) {
            let mut path = dir.clone();
            for part in rel.split('\\') {
                path.push(part);
            }
            req(is_inside(&resolve(&path), dir), "comparison path escaped source")?;
            if path.is_file() {
                let installed_hash = sha256_file(&path)?;
                entry.set("installed_sha256", installed_hash.clone());
                entry.set("installed_identical", installed_hash == sha);
                identical += (installed_hash == sha) as usize;
                compared += 1;
            }
        }
        files.push(entry);
        if (index + 1) % 500 == 0 {
            println!("Validated {}/{} files", index + 1, rows.len());
        }
    }
    req(retained == selected, "requested file not found")?;
    req(sha256_file(&discs[0])? == hashes[0] && sha256_file(&discs[1])? == hashes[1], "source media changed")?;
    let report = jobj! {
        "iso_sha256" => ISO_HASHES.to_vec(),
        "file_data_base" => BASE,
        "bootstrap" => J::Arr(boot.blocks),
        "candidate_files" => rows.len(),
        "compared" => compared,
        "installed_identical" => identical,
        "limitations" => vec![
            "InstallFile candidate scan, not full Wise VM/action coverage",
            "all extracted streams validated; no installer actions executed",
        ],
        "files" => files,
    };
    fs::write(output.join("report.json"), report.pretty(true) + "\n").map_err(|e| e.to_string())?;
    println!("PASS {} CRC/EOF/size checks; {compared} installed comparisons; sources unchanged", rows.len());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;
    use flate2::write::DeflateEncoder;
    use flate2::Compression;

    struct Mem(Vec<u8>);
    impl Source for Mem {
        fn size(&self) -> u64 {
            self.0.len() as u64
        }
        fn read(&mut self, offset: u64, count: u64) -> R<Vec<u8>> {
            req(offset <= self.size() && count <= self.size() - offset, "member read outside bounds")?;
            Ok(self.0[offset as usize..(offset + count) as usize].to_vec())
        }
    }

    fn deflate(data: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    /// Minimal ISO9660: PVD at sector 16 and a root directory with `members` as files.
    fn make_iso(path: &Path, members: &[(&str, &[u8])]) {
        let mut image = vec![0u8; 20 * 2048];
        let mut directory = Vec::new();
        let mut extent = 20u32;
        let mut payload = Vec::new();
        for (name, data) in members {
            let id = format!("{name};1");
            let mut record = vec![0u8; 33 + id.len()];
            record[0] = record.len() as u8;
            record[2..6].copy_from_slice(&extent.to_le_bytes());
            record[6..10].copy_from_slice(&extent.to_be_bytes());
            record[10..14].copy_from_slice(&(data.len() as u32).to_le_bytes());
            record[14..18].copy_from_slice(&(data.len() as u32).to_be_bytes());
            record[32] = id.len() as u8;
            record[33..].copy_from_slice(id.as_bytes());
            if record.len() % 2 == 1 {
                record.push(0);
                record[0] += 1;
            }
            directory.extend(record);
            let padded = data.len().div_ceil(2048).max(1) * 2048;
            let mut chunk = data.to_vec();
            chunk.resize(padded, 0);
            payload.extend(chunk);
            extent += (padded / 2048) as u32;
        }
        let dir_extent = 17u32;
        directory.resize(2048, 0);
        image[17 * 2048..18 * 2048].copy_from_slice(&directory);
        let mut pvd = vec![0u8; 2048];
        pvd[..7].copy_from_slice(b"\x01CD001\x01");
        pvd[156 + 2..156 + 6].copy_from_slice(&dir_extent.to_le_bytes());
        pvd[156 + 6..156 + 10].copy_from_slice(&dir_extent.to_be_bytes());
        pvd[156 + 10..156 + 14].copy_from_slice(&2048u32.to_le_bytes());
        pvd[156 + 14..156 + 18].copy_from_slice(&2048u32.to_be_bytes());
        image[16 * 2048..17 * 2048].copy_from_slice(&pvd);
        image.extend(payload);
        std::fs::write(path, image).unwrap();
    }

    #[test]
    fn reads_iso_members_and_composes_volumes() {
        let dir = TempDir::new();
        make_iso(&dir.path().join("a.iso"), &[("Setup.exe", b"0123456789")]);
        make_iso(&dir.path().join("b.iso"), &[("Other.bin", b"zz"), ("Setup.W02", b"abcdefghij")]);
        let first = IsoMember::open(&dir.path().join("a.iso"), "setup.EXE").unwrap();
        let second = IsoMember::open(&dir.path().join("b.iso"), "Setup.W02").unwrap();
        let mut both = Composite::new(vec![first, second]);
        assert_eq!(both.size(), 20);
        assert_eq!(both.read(8, 4).unwrap(), b"89ab");
        assert_eq!(both.read(0, 20).unwrap(), b"0123456789abcdefghij");
        assert!(both.read(15, 6).is_err());
        assert!(IsoMember::open(&dir.path().join("a.iso"), "missing.bin").is_err());
        std::fs::write(dir.path().join("junk.iso"), vec![0u8; 40000]).unwrap();
        assert!(IsoMember::open(&dir.path().join("junk.iso"), "x").is_err());
    }

    fn script_with(path: &str, start: u32, end: u32, size: u32, crc: u32) -> Vec<u8> {
        let mut script = vec![0xAAu8; 10];
        let mut prefix = vec![0u8; 43];
        prefix[0] = 0;
        prefix[1..3].copy_from_slice(&128u16.to_le_bytes());
        prefix[3..7].copy_from_slice(&start.to_le_bytes());
        prefix[7..11].copy_from_slice(&end.to_le_bytes());
        prefix[15..19].copy_from_slice(&size.to_le_bytes());
        prefix[39..43].copy_from_slice(&crc.to_le_bytes());
        script.extend(prefix);
        script.extend(path.as_bytes());
        script.push(0);
        script
    }

    #[test]
    fn finds_candidates_and_rejects_bad_records() {
        let script = script_with("%MAINDIR%\\DATA\\a.bin", 0, 100, 50, 7);
        let rows = candidates(&script, 1000, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "%MAINDIR%\\DATA\\a.bin");
        assert_eq!((rows[0].start, rows[0].end, rows[0].bytes, rows[0].crc32), (0, 100, 50, 7));
        assert!(candidates(&script_with("%MAINDIR%\\a", 0, 100, 50, 7), 50, 10).is_err(), "end beyond media");
        assert!(candidates(&script_with("%MAINDIR%\\a", 5, 5, 50, 7), 1000, 10).is_err(), "empty range");
        assert!(candidates(&script_with("%MAINDIR%\\a", 0, 9, 0, 7), 1000, 10).is_err(), "zero size");
        assert!(candidates(b"nothing here", 1000, 0).is_err());
    }

    #[test]
    fn validates_streams_with_padding_and_crc() {
        let payload: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let compressed = deflate(&payload);
        let crc = crc32(&payload);
        let mut media = vec![9u8; 7];
        media.extend(&compressed);
        media.push(0); // zero pad before the trailer
        media.extend(crc.to_le_bytes());
        let end = (media.len() - 7) as u64;
        let row = Candidate { script_offset: 0, path: "%MAINDIR%\\x".into(), start: 0, end, bytes: payload.len() as u64, crc32: crc };
        let mut source = Mem(media.clone());
        let (sha, padding) = validate_file(&mut source, &row, 7, None).unwrap();
        assert_eq!(sha, sha256_hex(&payload));
        assert_eq!(padding, 1);

        // extraction writes a new file and never overwrites
        let dir = TempDir::new();
        let target = dir.path().join("out.bin");
        validate_file(&mut Mem(media.clone()), &row, 7, Some(&target)).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), payload);
        assert!(validate_file(&mut Mem(media.clone()), &row, 7, Some(&target)).is_err());

        let wrong_crc = Candidate { crc32: crc ^ 1, ..row.clone() };
        assert!(validate_file(&mut Mem(media.clone()), &wrong_crc, 7, None).unwrap_err().contains("CRC"));
        let wrong_size = Candidate { bytes: 10, ..row.clone() };
        assert!(validate_file(&mut Mem(media.clone()), &wrong_size, 7, None).is_err());
        let mut bad_pad = media;
        let pad_at = bad_pad.len() - 5;
        bad_pad[pad_at] = 1;
        assert!(validate_file(&mut Mem(bad_pad), &row, 7, None).is_err());
    }

    #[test]
    fn bootstrap_checks_every_block() {
        let blocks: Vec<Vec<u8>> = vec![b"first block".to_vec(), vec![5u8; 300], b"third".to_vec()];
        let mut header = vec![0u8; 16];
        let mut sizes = Vec::new();
        for block in &blocks {
            let mut part = deflate(block);
            part.extend(crc32(block).to_le_bytes());
            sizes.push(part.len());
            header.extend(part);
        }
        let base = header.len() as u64;
        header.resize(2048, 0);
        let boot = bootstrap(&mut Mem(header.clone()), base, 16, &sizes, 1, 300).unwrap();
        assert_eq!(boot.script, vec![5u8; 300]);
        assert_eq!(boot.blocks.len(), 3);
        assert!(bootstrap(&mut Mem(header.clone()), base + 1, 16, &sizes, 1, 300).is_err());
        assert!(bootstrap(&mut Mem(header.clone()), base, 16, &sizes, 1, 301).is_err());
        let mut corrupted = header;
        corrupted[16 + sizes[0] - 1] ^= 1;
        assert!(bootstrap(&mut Mem(corrupted), base, 16, &sizes, 1, 300).is_err());
    }

    #[test]
    fn extraction_paths_must_stay_relative() {
        assert_eq!(safe_relative("%MAINDIR%\\DATA\\a.bin").as_deref(), Some("DATA\\a.bin"));
        assert!(safe_relative("%MAINDIR%\\..\\a").is_none());
        assert!(safe_relative("%MAINDIR%\\C:\\a").is_none());
        assert!(safe_relative("%SYS%\\a").is_none());
    }
}
