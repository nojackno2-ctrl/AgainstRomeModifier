#[derive(Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub timestamp: u32,
    pub preferred_base: u32,
    pub image_size: u32,
    pub entry: u32,
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(i..i.checked_add(4)?)?.try_into().ok()?,
    ))
}
pub fn fingerprint(b: &[u8]) -> Option<Fingerprint> {
    if b.get(..2)? != b"MZ" {
        return None;
    }
    let nt = u32_at(b, 0x3c)? as usize;
    if b.get(nt..nt.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    if b.get(nt + 4..nt + 6)? != [0x4c, 1] || b.get(nt + 24..nt + 26)? != [0x0b, 1] {
        return None;
    }
    Some(Fingerprint {
        timestamp: u32_at(b, nt + 8)?,
        preferred_base: u32_at(b, nt + 52)?,
        image_size: u32_at(b, nt + 80)?,
        entry: u32_at(b, nt + 40)?,
    })
}
pub fn build_matches(expected: u32, actual: u32) -> bool {
    expected == 0 || expected == actual
}
pub fn rebase(va: u32, base: u32) -> u32 {
    va.wrapping_sub(0x400000).wrapping_add(base)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Export {
    pub ordinal: u32,
    pub name: Option<String>,
}
/// Parse an on-disk PE export directory (PE32 or PE32+). Used to compare the
/// generated DLL with the C++ definition without loading it into a process.
pub fn exports(b: &[u8]) -> Option<Vec<Export>> {
    fn word(b: &[u8], i: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            b.get(i..i.checked_add(2)?)?.try_into().ok()?,
        ))
    }
    if b.get(..2)? != b"MZ" {
        return None;
    }
    let nt = u32_at(b, 0x3c)? as usize;
    if b.get(nt..nt.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let nsec = word(b, nt + 6)? as usize;
    let opt = nt.checked_add(24)?;
    let opt_size = word(b, nt + 20)? as usize;
    let dir = match word(b, opt)? {
        0x10b => 96,
        0x20b => 112,
        _ => return None,
    };
    if opt_size < dir + 8 {
        return None;
    }
    let sections = opt.checked_add(opt_size)?;
    b.get(sections..sections.checked_add(nsec.checked_mul(40)?)?)?;
    let offset = |rva: u32| -> Option<usize> {
        for i in 0..nsec {
            let s = sections + i * 40;
            let va = u32_at(b, s + 12)?;
            let raw_size = u32_at(b, s + 16)?;
            if let Some(delta) = rva.checked_sub(va) {
                if delta < raw_size {
                    return Some(u32_at(b, s + 20)?.checked_add(delta)? as usize);
                }
            }
        }
        None
    };
    let rva = u32_at(b, opt + dir)?;
    if rva == 0 {
        return Some(vec![]);
    }
    let d = offset(rva)?;
    let base = u32_at(b, d + 16)?;
    let count = u32_at(b, d + 20)? as usize;
    let names = u32_at(b, d + 24)? as usize;
    if count > b.len() / 4 || names > count {
        return None;
    }
    let funcs = offset(u32_at(b, d + 28)?)?;
    let strings = if names == 0 {
        0
    } else {
        offset(u32_at(b, d + 32)?)?
    };
    let ords = if names == 0 {
        0
    } else {
        offset(u32_at(b, d + 36)?)?
    };
    let mut by_index = std::collections::BTreeMap::new();
    for i in 0..names {
        let index = word(b, ords + i * 2)? as usize;
        if index >= count {
            return None;
        }
        let s = offset(u32_at(b, strings + i * 4)?)?;
        let tail = b.get(s..)?;
        let end = tail.iter().position(|v| *v == 0)?;
        let name = std::str::from_utf8(&tail[..end]).ok()?.to_owned();
        if by_index.insert(index, name).is_some() {
            return None;
        }
    }
    let mut out = Vec::new();
    for i in 0..count {
        if u32_at(b, funcs + i * 4)? != 0 {
            out.push(Export {
                ordinal: base.checked_add(i as u32)?,
                name: by_index.remove(&i),
            });
        }
    }
    Some(out)
}
