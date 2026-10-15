use crate::lde::{decode_length, is_prefix};
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct RegBank {
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
}
pub fn verify_signature(code: &[u8], sig: &[u8], mask: &[u8]) -> bool {
    sig.len() == mask.len()
        && code.len() >= mask.len()
        && mask
            .iter()
            .enumerate()
            .all(|(i, b)| *b != b'x' || code[i] == sig[i])
}
pub fn is_relative(code: &[u8]) -> bool {
    let p = code.iter().position(|b| !is_prefix(*b));
    match p.and_then(|i| code.get(i..)) {
        Some([0xe8 | 0xe9 | 0xeb | 0x70..=0x7f | 0xe0..=0xe3, ..]) => true,
        Some([0x0f, 0x80..=0x8f, ..]) => true,
        _ => false,
    }
}
pub fn stolen_length(code: &[u8]) -> Result<usize, &'static str> {
    let mut n = 0;
    while n < 5 {
        let p = code.get(n..).ok_or("truncated prologue")?;
        if is_relative(p) {
            return Err("relative instruction");
        }
        n += decode_length(p).ok_or("undecodable prologue")?;
        if n > 32 {
            return Err("prologue too long");
        }
    }
    Ok(n)
}
fn relative(op: u8, address: u32, dest: u32) -> Vec<u8> {
    let mut v = vec![op];
    v.extend(dest.wrapping_sub(address.wrapping_add(5)).to_le_bytes());
    v
}
pub fn trampoline(code: &[u8], target: u32, address: u32) -> Result<Vec<u8>, &'static str> {
    let n = stolen_length(code)?;
    let mut v = code[..n].to_vec();
    v.extend(relative(
        0xe9,
        address.wrapping_add(n as u32),
        target.wrapping_add(n as u32),
    ));
    Ok(v)
}
pub fn patch(target: u32, stub: u32, stolen: usize) -> Vec<u8> {
    let mut v = relative(0xe9, target, stub);
    v.resize(stolen.max(5), 0x90);
    v
}
pub fn stub(address: u32, descriptor: u32, callback: u32, trampoline: u32) -> Vec<u8> {
    let mut v = vec![0x9c, 0x60, 0x89, 0xe0, 0x50, 0x68];
    v.extend(descriptor.to_le_bytes());
    v.extend(relative(
        0xe8,
        address.wrapping_add(v.len() as u32),
        callback,
    ));
    v.extend([0x83, 0xc4, 8, 0x61, 0x9d]);
    v.extend(relative(
        0xe9,
        address.wrapping_add(v.len() as u32),
        trampoline,
    ));
    v
}
