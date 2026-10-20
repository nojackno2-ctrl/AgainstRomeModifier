//! Shared helpers: error type, byte readers, PFIL prefix decoder, hashing, code pages,
//! output-path safety gates and the PE32 reader used by the EXE probes.

use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub type R<T> = Result<T, String>;

/// SHA-256 of the one supported `Against_Rome.exe` build (same gate as `probe_pua.EXE_SHA256`).
pub const EXE_SHA256: &str = "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf";
/// Installed game directory; every probe refuses to write below it.
pub const INSTALLED_GAME: &str = r"C:\Program Files (x86)\Against Rome";
/// objects.dat per-object extra column widths (`probe_object_anim_links.COLUMN_WIDTHS`).
pub const COLUMN_WIDTHS: [usize; 18] = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];

pub fn req(condition: bool, message: &str) -> R<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

/// Marks an error as a command-line usage error (exit code 2, like `argparse.error`).
pub fn usage(message: impl Into<String>) -> String {
    format!("usage: {}", message.into())
}

pub fn io(error: std::io::Error, path: &Path) -> String {
    format!("{}: {error}", path.display())
}

// ---------------------------------------------------------------- byte readers

fn slice(b: &[u8], off: usize, n: usize) -> R<&[u8]> {
    off.checked_add(n)
        .and_then(|end| b.get(off..end))
        .ok_or_else(|| format!("unpack_from requires a buffer of at least {} bytes", off.saturating_add(n)))
}
pub fn u8_at(b: &[u8], off: usize) -> R<u8> {
    Ok(slice(b, off, 1)?[0])
}
pub fn u16_at(b: &[u8], off: usize) -> R<u16> {
    Ok(u16::from_le_bytes(slice(b, off, 2)?.try_into().unwrap()))
}
pub fn i16_at(b: &[u8], off: usize) -> R<i16> {
    Ok(i16::from_le_bytes(slice(b, off, 2)?.try_into().unwrap()))
}
pub fn u32_at(b: &[u8], off: usize) -> R<u32> {
    Ok(u32::from_le_bytes(slice(b, off, 4)?.try_into().unwrap()))
}
pub fn i32_at(b: &[u8], off: usize) -> R<i32> {
    Ok(i32::from_le_bytes(slice(b, off, 4)?.try_into().unwrap()))
}
pub fn f32_at(b: &[u8], off: usize) -> R<f32> {
    Ok(f32::from_bits(u32_at(b, off)?))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn hex_spaced(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- hashing

pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn sha256_file(path: &Path) -> R<String> {
    let mut stream = File::open(path).map_err(|e| io(e, path))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = stream.read(&mut buffer).map_err(|e| io(e, path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Running CRC-32 (zlib polynomial); `crc32_update(0, data)` equals `zlib.crc32(data)`.
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    });
    let mut c = !crc;
    for &b in data {
        c = table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

// ---------------------------------------------------------------- PFIL

/// Port of `inventory_game_directory.pfil_prefix`: decodes at most `limit` bytes of a
/// `PFIL@` body (tolerates trailing input, unlike the strict `against_rome_data::lzss`).
/// Returns the declared uncompressed size and the decoded prefix.
pub fn pfil_prefix(data: &[u8], limit: usize) -> R<(i32, Vec<u8>)> {
    if data.len() < 64 || &data[..5] != b"PFIL@" {
        return Err("missing PFIL@ header".into());
    }
    let size = i32_at(data, 16)?;
    if size < 0 {
        return Err("negative PFIL length".into());
    }
    let wanted = (size as usize).min(limit);
    let mut ring = [b' '; 4096];
    ring[4078..].fill(0);
    let (mut pos, mut cursor) = (64usize, 4078usize);
    let mut out = Vec::with_capacity(wanted.min(1 << 24));
    while out.len() < wanted {
        if pos >= data.len() {
            return Err("truncated PFIL prefix".into());
        }
        let flags = data[pos];
        pos += 1;
        for bit in 0..8 {
            if out.len() >= wanted {
                break;
            }
            if flags & (1 << bit) != 0 {
                if pos + 1 > data.len() {
                    return Err("truncated PFIL token".into());
                }
                let value = data[pos];
                pos += 1;
                out.push(value);
                ring[cursor] = value;
                cursor = (cursor + 1) & 4095;
            } else {
                if pos + 2 > data.len() {
                    return Err("truncated PFIL token".into());
                }
                let (lo, hi) = (data[pos] as usize, data[pos + 1] as usize);
                pos += 2;
                let offset = lo | ((hi & 240) << 4);
                let length = (hi & 15) + 3;
                for k in 0..length {
                    let value = ring[(offset + k) & 4095];
                    out.push(value);
                    ring[cursor] = value;
                    cursor = (cursor + 1) & 4095;
                    if out.len() >= wanted {
                        break;
                    }
                }
            }
        }
    }
    Ok((size, out))
}

/// `probe_object_anim_links.decode`: PFIL payloads (bounded to 4 MiB) or raw pools.
pub fn decode(data: &[u8]) -> R<Vec<u8>> {
    if data.starts_with(b"PFIL@") {
        req(data.len() >= 64, "short PFIL header")?;
        let size = i32_at(data, 16)?;
        req((0..=4 * 1024 * 1024).contains(&size), "decoded pool exceeds analysis limit")?;
        Ok(pfil_prefix(data, size as usize)?.1)
    } else {
        Ok(data.to_vec())
    }
}

/// `decode_file` used by the 24-pool scripts: any PFIL size, no analysis ceiling.
pub fn decode_unbounded(data: &[u8]) -> R<(bool, Vec<u8>)> {
    if data.starts_with(b"PFIL@") {
        let size = i32_at(data, 16)?;
        let limit = if size < 0 { 0 } else { size as usize };
        Ok((true, pfil_prefix(data, limit)?.1))
    } else {
        Ok((false, data.to_vec()))
    }
}

// ---------------------------------------------------------------- objects.dat

/// Decoded `objects.dat` with the shared layout checks.
pub struct Objects {
    pub data: Vec<u8>,
    pub count: usize,
}

impl Objects {
    /// version 1, `count <= 14000`, both name widths 30 and exact length. `nonzero` also demands count > 0.
    pub fn parse(raw: &[u8], nonzero: bool, layout: &str) -> R<Self> {
        let data = decode(raw)?;
        Self::from_decoded(data, nonzero, layout)
    }
    pub fn from_decoded(data: Vec<u8>, nonzero: bool, layout: &str) -> R<Self> {
        req(data.len() >= 16, "short objects header")?;
        let (version, count, n0, n1) = (u32_at(&data, 0)?, u32_at(&data, 4)?, u32_at(&data, 8)?, u32_at(&data, 12)?);
        req(
            version == 1 && count <= 14000 && (!nonzero || count > 0) && n0 == 30 && n1 == 30,
            layout,
        )?;
        let count = count as usize;
        req(data.len() == 16 + count * (79 + COLUMN_WIDTHS.iter().sum::<usize>()), "objects length mismatch")?;
        Ok(Self { data, count })
    }
    pub fn active(&self, slot: usize) -> bool {
        self.data[16 + slot * 79] != 0
    }
    /// Offset of zero-based extra column `column` (`16 + count * (79 + sum(widths[:column]))`).
    pub fn column(&self, column: usize) -> usize {
        16 + self.count * (79 + COLUMN_WIDTHS[..column].iter().sum::<usize>())
    }
}

// ---------------------------------------------------------------- code pages

const CP1252_HIGH: [u32; 32] = [
    0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d, 0,
    0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178,
];

/// Strict Python-compatible cp1252 decoder (rejects the five undefined bytes).
pub fn decode_cp1252(bytes: &[u8]) -> R<String> {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let code = if (0x80..0xa0).contains(&b) {
            match CP1252_HIGH[(b - 0x80) as usize] {
                0 => return Err(format!("'charmap' codec can't decode byte {b:#04x}")),
                c => c,
            }
        } else {
            b as u32
        };
        out.push(char::from_u32(code).unwrap());
    }
    Ok(out)
}

const CP1251_HIGH: [u32; 64] = [
    0x0402, 0x0403, 0x201a, 0x0453, 0x201e, 0x2026, 0x2020, 0x2021, 0x20ac, 0x2030, 0x0409, 0x2039, 0x040a, 0x040c, 0x040b, 0x040f,
    0x0452, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0, 0x2122, 0x0459, 0x203a, 0x045a, 0x045c, 0x045b, 0x045f,
    0x00a0, 0x040e, 0x045e, 0x0408, 0x00a4, 0x0490, 0x00a6, 0x00a7, 0x0401, 0x00a9, 0x0404, 0x00ab, 0x00ac, 0x00ad, 0x00ae, 0x0407,
    0x00b0, 0x00b1, 0x0406, 0x0456, 0x0491, 0x00b5, 0x00b6, 0x00b7, 0x0451, 0x2116, 0x0454, 0x00bb, 0x0458, 0x0405, 0x0455, 0x0457,
];

/// Strict Python-compatible cp1251 decoder (byte 0x98 is undefined).
pub fn decode_cp1251(bytes: &[u8]) -> R<String> {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let code = match b {
            0..=0x7f => b as u32,
            0x80..=0xbf => match CP1251_HIGH[(b - 0x80) as usize] {
                0 => return Err(format!("'charmap' codec can't decode byte {b:#04x}")),
                c => c,
            },
            _ => 0x0410 + (b - 0xc0) as u32,
        };
        out.push(char::from_u32(code).unwrap());
    }
    Ok(out)
}

/// Python `str.splitlines()` for text that cannot contain U+0085/U+2028/U+2029 line breaks of its own.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let (mut start, mut i) = (0usize, 0usize);
    while i < bytes.len() {
        match bytes[i] {
            b'\r' | b'\n' | 0x0b | 0x0c | 0x1c | 0x1d | 0x1e => {
                lines.push(&text[start..i]);
                if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < bytes.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Python `PurePath.suffix` for the final component of a `/`- or `\`-separated name.
pub fn py_suffix(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("");
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[i..].to_string(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------- paths and output gates

/// Drops the Windows verbatim prefix (`\\?\C:\...`) so canonicalized paths compare and print
/// like Python's `Path.resolve()`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy().into_owned();
    match text.strip_prefix("\\\\?\\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

pub fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).map(strip_verbatim).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

/// Python `Path.resolve()` (non-strict): resolves the longest existing prefix, then re-appends the rest.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    let mut existing = absolute.clone();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(&existing) {
            let mut result = strip_verbatim(real);
            for part in tail.iter().rev() {
                match Path::new(part).components().next() {
                    Some(Component::ParentDir) => {
                        result.pop();
                    }
                    Some(Component::CurDir) | None => {}
                    _ => result.push(part),
                }
            }
            return result;
        }
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent().map(|p| p.to_path_buf())) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

/// Case-insensitive (Windows-style) `path.is_relative_to(directory)` on already resolved paths.
pub fn is_inside(path: &Path, directory: &Path) -> bool {
    let a: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    let b: Vec<String> = directory.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    a.len() >= b.len() && a[..b.len()] == b[..]
}

/// Validates a report path: it must not exist and must lie outside every `forbidden` directory.
/// Returns the resolved output path.
pub fn new_output(output: &Path, forbidden: &[PathBuf], what: &str) -> R<PathBuf> {
    let resolved = resolve(output);
    if resolved.exists() || fs::symlink_metadata(output).is_ok() {
        return Err(usage(format!("output must be new and outside {what}: {} exists", output.display())));
    }
    for directory in forbidden {
        if is_inside(&resolved, &resolve(directory)) {
            return Err(usage(format!("output must be new and outside {what}")));
        }
    }
    Ok(resolved)
}

pub fn installed_game() -> PathBuf {
    PathBuf::from(INSTALLED_GAME)
}

/// Exclusive-create write (`open('x')`): never overwrites.
pub fn write_new(path: &Path, text: &str) -> R<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| io(e, path))?;
    file.write_all(text.as_bytes()).map_err(|e| io(e, path))
}

pub fn read(path: &Path) -> R<Vec<u8>> {
    fs::read(path).map_err(|e| io(e, path))
}

/// `MAPS/*` sub-directories (non-symlink), sorted case-insensitively like `pathlib` on Windows.
pub fn map_dirs(root: &Path) -> R<Vec<PathBuf>> {
    let maps = root.join("MAPS");
    req(maps.is_dir(), "missing MAPS directory")?;
    let mut dirs = Vec::new();
    for entry in fs::read_dir(&maps).map_err(|e| io(e, &maps))? {
        let entry = entry.map_err(|e| io(e, &maps))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| io(e, &path))?;
        if meta.is_dir() && !meta.file_type().is_symlink() {
            dirs.push(path);
        }
    }
    dirs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(dirs)
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

// ---------------------------------------------------------------- PE32

/// `probe_engine_config.read_va`: bytes at a virtual address, entirely inside one file-backed section.
pub fn read_va(exe: &[u8], address: u64, size: usize) -> R<&[u8]> {
    let nt = u32_at(exe, 0x3c)? as usize;
    req(exe.get(..2) == Some(b"MZ") && exe.get(nt..nt + 4) == Some(b"PE\0\0"), "not PE")?;
    let count = u16_at(exe, nt + 6)? as usize;
    let opt_size = u16_at(exe, nt + 20)? as usize;
    let opt = nt + 24;
    req(u16_at(exe, opt)? == 0x10b, "expected PE32")?;
    let rva = address as i64 - u32_at(exe, opt + 28)? as i64;
    for index in 0..count {
        let section = opt + opt_size + index * 40;
        let (va, raw_size, raw_offset) =
            (u32_at(exe, section + 12)? as i64, u32_at(exe, section + 16)? as i64, u32_at(exe, section + 20)? as i64);
        if va <= rva && rva + size as i64 <= va + raw_size {
            let offset = (raw_offset + rva - va) as usize;
            req(offset + size <= exe.len(), "read exceeds source")?;
            return Ok(&exe[offset..offset + size]);
        }
    }
    Err("address is not file-backed".into())
}

pub fn require_exe(exe: &[u8]) -> R<()> {
    req(sha256_hex(exe) == EXE_SHA256, "unsupported EXE fingerprint")
}

/// Minimal PE32 section view used by `scan-native-scene` (the `pefile` subset it relied on).
pub struct Sections {
    pub image_base: u32,
    pub machine: u16,
    /// (name, virtual_address, raw_size, raw_offset, characteristics)
    pub list: Vec<(String, u32, u32, u32, u32)>,
}

impl Sections {
    pub fn parse(data: &[u8]) -> R<Self> {
        req(data.len() >= 64 && data.get(..2) == Some(b"MZ"), "not MZ")?;
        let nt = u32_at(data, 60)? as usize;
        req(data.get(nt..nt + 4) == Some(b"PE\0\0"), "not PE")?;
        let machine = u16_at(data, nt + 4)?;
        let count = u16_at(data, nt + 6)? as usize;
        let opt_size = u16_at(data, nt + 20)? as usize;
        let opt = nt + 24;
        let image_base = u32_at(data, opt + 28)?;
        let mut list = Vec::new();
        for index in 0..count {
            let s = opt + opt_size + index * 40;
            let name = data.get(s..s + 8).ok_or("truncated section header")?;
            let name = String::from_utf8_lossy(name).trim_end_matches('\0').to_string();
            list.push((name, u32_at(data, s + 12)?, u32_at(data, s + 16)?, u32_at(data, s + 20)?, u32_at(data, s + 36)?));
        }
        Ok(Self { image_base, machine, list })
    }
    /// `pefile.get_rva_from_offset`.
    pub fn rva_from_offset(&self, offset: u32) -> u32 {
        for (_, va, raw_size, raw_offset, _) in &self.list {
            if *raw_offset <= offset && offset < raw_offset.saturating_add(*raw_size) {
                return offset - raw_offset + va;
            }
        }
        offset
    }
    /// `pefile.get_offset_from_rva`.
    pub fn offset_from_rva(&self, rva: u32) -> R<u32> {
        for (_, va, raw_size, raw_offset, _) in &self.list {
            if *va <= rva && rva < va.saturating_add(*raw_size) {
                return Ok(rva - va + raw_offset);
            }
        }
        Err("data at RVA can't be fetched. Corrupt header?".into())
    }
}

/// Path as printed by Python (`str(Path)`): no verbatim prefix.
pub fn display(path: &Path) -> String {
    strip_verbatim(path.to_path_buf()).display().to_string()
}
