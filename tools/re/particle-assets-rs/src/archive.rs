use flate2::{Decompress, FlushDecompress, Status};
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub method: u16,
    pub flags: u16,
    pub crc: u32,
    pub packed: usize,
    pub size: usize,
    pub local: usize,
}
fn u16at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        b.get(p..p.checked_add(2).ok_or("overflow")?)
            .ok_or("truncated ZIP word")?
            .try_into()
            .unwrap(),
    ))
}
fn u32at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(p..p.checked_add(4).ok_or("overflow")?)
            .ok_or("truncated ZIP dword")?
            .try_into()
            .unwrap(),
    ))
}
pub fn entries(b: &[u8]) -> Result<Vec<Entry>, String> {
    if b.len() < 22 {
        return Err("truncated ZIP EOCD".into());
    }
    let p = (b.len().saturating_sub(65557)..=b.len() - 22)
        .rev()
        .find(|&p| {
            b[p..p + 4] == *b"PK\x05\x06"
                && u16at(b, p + 20).is_ok_and(|n| p + 22 + n as usize == b.len())
        })
        .ok_or("missing ZIP EOCD")?;
    if u16at(b, p + 4)? != 0 || u16at(b, p + 6)? != 0 || u16at(b, p + 8)? != u16at(b, p + 10)? {
        return Err("unsupported multi-disk ZIP".into());
    }
    let count = u16at(b, p + 10)? as usize;
    let len = u32at(b, p + 12)? as usize;
    let mut c = u32at(b, p + 16)? as usize;
    if count == 65535 || len == u32::MAX as usize || c == u32::MAX as usize {
        return Err("ZIP64 unsupported".into());
    }
    let end = c.checked_add(len).ok_or("ZIP overflow")?;
    if end > p {
        return Err("central directory out of range".into());
    }
    let mut result = Vec::new();
    for _ in 0..count {
        if u32at(b, c)? != 0x02014b50 {
            return Err("invalid ZIP central signature".into());
        }
        let n = u16at(b, c + 28)? as usize;
        let extra = u16at(b, c + 30)? as usize;
        let comment = u16at(b, c + 32)? as usize;
        let next = c
            .checked_add(46 + n + extra + comment)
            .ok_or("ZIP overflow")?;
        if next > end {
            return Err("truncated ZIP central entry".into());
        }
        if u16at(b, c + 34)? != 0 {
            return Err("multi-disk entry".into());
        }
        let name_bytes = &b[c + 46..c + 46 + n];
        if !name_bytes.is_ascii() {
            return Err("non-ASCII ZIP name encoding unresolved".into());
        }
        result.push(Entry {
            name: String::from_utf8(name_bytes.to_vec()).unwrap(),
            method: u16at(b, c + 10)?,
            flags: u16at(b, c + 8)?,
            crc: u32at(b, c + 16)?,
            packed: u32at(b, c + 20)? as usize,
            size: u32at(b, c + 24)? as usize,
            local: u32at(b, c + 42)? as usize,
        });
        c = next;
    }
    if c != end {
        return Err("unexpected central directory trailing bytes".into());
    }
    Ok(result)
}
pub fn resolve<'a>(entries: &'a [Entry], name: &str) -> Result<&'a Entry, String> {
    if name.contains('/') || name.contains('\\') || name == ".." {
        return Err("ALR name contains path; resolution unresolved".into());
    }
    let key = format!("SYSTEM/DATA/ALR/{name}");
    let matches: Vec<_> = entries
        .iter()
        .filter(|e| e.name.replace('\\', "/").eq_ignore_ascii_case(&key))
        .collect();
    match matches.as_slice() {
        [entry] => Ok(entry),
        [] => Err(format!("missing archive member {key}")),
        _ => Err(format!("ambiguous archive member {key}")),
    }
}
pub fn read(b: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
    if e.flags & 1 != 0 || e.size > 50 * 1024 * 1024 || e.packed > 50 * 1024 * 1024 {
        return Err("encrypted/oversized ZIP member".into());
    }
    let p = e.local;
    if u32at(b, p)? != 0x04034b50 || u16at(b, p + 8)? != e.method || u16at(b, p + 6)? != e.flags {
        return Err("invalid/inconsistent local ZIP header".into());
    }
    let n = u16at(b, p + 26)? as usize;
    let extra = u16at(b, p + 28)? as usize;
    let start = p.checked_add(30 + n + extra).ok_or("ZIP overflow")?;
    let end = start.checked_add(e.packed).ok_or("ZIP overflow")?;
    let name = b.get(p + 30..p + 30 + n).ok_or("truncated local name")?;
    if name != e.name.as_bytes() {
        return Err("local/central name mismatch".into());
    }
    let data = b.get(start..end).ok_or("truncated ZIP payload")?;
    let out = match e.method {
        0 => data.to_vec(),
        8 => {
            let mut decoder = Decompress::new(false);
            let mut v = Vec::with_capacity(e.size + 1);
            let status = decoder
                .decompress_vec(data, &mut v, FlushDecompress::Finish)
                .map_err(|e| e.to_string())?;
            if status != Status::StreamEnd || decoder.total_in() != data.len() as u64 {
                return Err("truncated or trailing compressed member bytes".into());
            }
            v
        }
        m => return Err(format!("unsupported ZIP method {m}")),
    };
    if out.len() != e.size {
        return Err("ZIP decoded length mismatch".into());
    }
    if crc32fast::hash(&out) != e.crc {
        return Err("ZIP CRC32 mismatch".into());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Entry {
        Entry {
            name: "SYSTEM/DATA/ALR/a.alr".into(),
            method: 0,
            flags: 0,
            crc: 0,
            packed: 0,
            size: 0,
            local: 0,
        }
    }
    #[test]
    fn missing_duplicate_and_path() {
        let e = entry();
        assert!(resolve(&[], "a.alr").is_err());
        assert!(resolve(&[e.clone(), e], "a.alr").is_err());
        assert!(resolve(&[], "../a.alr").is_err());
    }
    #[test]
    fn invalid_truncated_outofrange() {
        assert!(entries(&[]).is_err());
        assert!(entries(&[0; 22]).is_err());
        let mut b = vec![0; 22];
        b[..4].copy_from_slice(b"PK\x05\x06");
        b[16..20].copy_from_slice(&99u32.to_le_bytes());
        assert!(entries(&b).is_err());
        assert!(read(&[], &entry()).is_err());
    }
    #[test]
    fn stored_crc_and_local_mismatch() {
        let mut e = entry();
        let mut b = vec![0; 30];
        b[..4].copy_from_slice(b"PK\x03\x04");
        b[26..28].copy_from_slice(&(e.name.len() as u16).to_le_bytes());
        b.extend(e.name.as_bytes());
        assert!(read(&b, &e).is_ok());
        e.crc = 1;
        assert!(read(&b, &e).is_err());
        e.crc = 0;
        b[30] = b'X';
        assert!(read(&b, &e).is_err());
    }
    #[test]
    fn deflate_payload_limits_crc_and_truncation() {
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"abcabcabc").unwrap();
        let packed = encoder.finish().unwrap();
        let mut e = entry();
        e.method = 8;
        e.size = 9;
        e.packed = packed.len();
        e.crc = crc32fast::hash(b"abcabcabc");
        let mut b = vec![0; 30];
        b[..4].copy_from_slice(b"PK\x03\x04");
        b[8..10].copy_from_slice(&8u16.to_le_bytes());
        b[26..28].copy_from_slice(&(e.name.len() as u16).to_le_bytes());
        b.extend(e.name.as_bytes());
        b.extend(&packed);
        assert_eq!(read(&b, &e).unwrap(), b"abcabcabc");
        for n in 0..b.len() {
            assert!(read(&b[..n], &e).is_err());
        }
        // Also update packed size to exercise the deflate parser itself, not only ZIP slicing.
        let start = b.len() - packed.len();
        for n in 0..packed.len() {
            let mut truncated = e.clone();
            truncated.packed = n;
            assert!(read(&b[..start + n], &truncated).is_err());
        }
        e.size = 8;
        assert!(read(&b, &e).is_err());
        e.size = 9;
        e.crc = 0;
        assert!(read(&b, &e).is_err());
        e.crc = crc32fast::hash(b"abcabcabc");
        b.push(0);
        e.packed += 1;
        assert!(read(&b, &e).is_err());
    }
    #[test]
    fn central_directory_boundaries() {
        let name = b"SYSTEM/DATA/ALR/a.alr";
        let mut b = vec![0; 46];
        b[..4].copy_from_slice(b"PK\x01\x02");
        b[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend(name);
        let end = b.len();
        let mut eocd = vec![0; 22];
        eocd[..4].copy_from_slice(b"PK\x05\x06");
        eocd[8..10].copy_from_slice(&1u16.to_le_bytes());
        eocd[10..12].copy_from_slice(&1u16.to_le_bytes());
        eocd[12..16].copy_from_slice(&(end as u32).to_le_bytes());
        b.extend(eocd);
        assert_eq!(entries(&b).unwrap().len(), 1);
        for n in 0..b.len() {
            assert!(entries(&b[..n]).is_err());
        }
        b[28..30].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(entries(&b).is_err());
    }
}
