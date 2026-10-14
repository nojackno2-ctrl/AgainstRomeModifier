//! Synthetic-data tests only; no original game data is embedded.

use against_rome_data::error::code;
use against_rome_data::ffi::*;
use against_rome_data::lzss;
use against_rome_data::pool::{SNAPSHOT_OFFSETS, WIDTHS};
use against_rome_data::{Error, LightPool, Source, CAPACITY, PAYLOAD_LEN};

const COL: [usize; 16] = {
    let mut o = [0usize; 16];
    let mut i = 0;
    let mut acc = 8;
    while i < 16 {
        o[i] = acc;
        acc += CAPACITY * WIDTHS[i];
        i += 1;
    }
    o
};

fn empty_payload() -> Vec<u8> {
    let mut p = vec![0u8; PAYLOAD_LEN];
    p[0..4].copy_from_slice(&1i32.to_le_bytes());
    p[4..8].copy_from_slice(&1024i32.to_le_bytes());
    for c in 13..16 {
        p[COL[c]..COL[c] + CAPACITY * 2].fill(0xFF);
    }
    p
}

fn put(p: &mut [u8], col: usize, slot: usize, bytes: &[u8]) {
    let o = COL[col] + slot * WIDTHS[col];
    p[o..o + bytes.len()].copy_from_slice(bytes);
}

fn lit_pfil(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    out[..4].copy_from_slice(b"PFIL");
    out[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    for chunk in payload.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8); // low bits set = literals
        out.extend_from_slice(chunk);
    }
    out
}

fn sample() -> Vec<u8> {
    let mut p = empty_payload();
    put(&mut p, 0, 3, &7u32.to_le_bytes());
    put(&mut p, 1, 3, &1.5f32.to_le_bytes());
    put(&mut p, 2, 3, &(-2.0f32).to_le_bytes());
    put(&mut p, 3, 3, &f32::from_bits(0x7fa0_0001).to_bits().to_le_bytes()); // signaling NaN payload
    put(&mut p, 4, 3, &2.5f32.to_le_bytes());
    put(&mut p, 5, 3, &0.25f32.to_le_bytes());
    put(&mut p, 6, 3, &0.5f32.to_le_bytes());
    put(&mut p, 7, 3, &300.0f32.to_le_bytes());
    put(&mut p, 8, 3, &450.0f32.to_le_bytes());
    put(&mut p, 9, 3, &0xFFFEu16.to_le_bytes());
    put(&mut p, 10, 3, &0.15f32.to_le_bytes());
    put(&mut p, 11, 3, &0xFFFF_FFFFu32.to_le_bytes());
    put(&mut p, 12, 3, &0xDEAD_BEEFu32.to_le_bytes());
    put(&mut p, 13, 3, &(-5i16).to_le_bytes());
    put(&mut p, 14, 3, &12i16.to_le_bytes());
    put(&mut p, 15, 3, &39i16.to_le_bytes());
    p
}

#[test]
fn snapshot_offsets_match_contract() {
    assert_eq!(SNAPSHOT_OFFSETS, [0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 38, 42, 46, 50, 52, 54]);
    assert_eq!(PAYLOAD_LEN, 57352);
}

#[test]
fn typed_fields_and_bit_preservation() {
    let payload = sample();
    let pool = LightPool::from_payload(&payload).unwrap();
    assert_eq!(pool.active_slots().collect::<Vec<_>>(), vec![3]);
    let s = pool.capture(3).unwrap();
    assert_eq!(s.active_raw(), 7);
    assert_eq!(s.position()[0], 1.5);
    assert_eq!(s.position()[1], -2.0);
    assert_eq!(s.position_bits()[2], 0x7fa0_0001);
    assert_eq!(s.color(), [2.5, 0.25, 0.5]);
    assert_eq!(s.effective_radius(), 300.0);
    assert_eq!(s.baseline_radius(), 450.0);
    assert_eq!(s.type_raw(), 0xFFFE);
    assert_eq!(s.type_parameter(), 0.15);
    assert_eq!(s.age_raw(), 0xFFFF_FFFF);
    assert_eq!(s.age_signed(), -1);
    assert_eq!(s.special_fx_raw(), 0xDEAD_BEEF);
    assert_eq!((s.owner(), s.parent(), s.definition()), (-5, 12, 39));
    assert_eq!(pool.payload(), &payload[..]);
    assert!(pool.capture(2).is_none() && pool.capture(1024).is_none());
    assert_eq!(pool.snapshot_any(1024), Err(Error::InvalidSlot(1024)));
    // the 56 raw bytes are the exact concatenation of the slot's columns
    let mut expect = Vec::new();
    for c in 0..16 {
        let o = COL[c] + 3 * WIDTHS[c];
        expect.extend_from_slice(&payload[o..o + WIDTHS[c]]);
    }
    assert_eq!(&s.raw_fields()[..], &expect[..]);
}

#[test]
fn inactive_slot_state_is_preserved_by_snapshot_any() {
    let mut p = empty_payload();
    put(&mut p, 7, 9, &123.0f32.to_le_bytes()); // radius in an inactive slot
    let pool = LightPool::from_payload(&p).unwrap();
    assert!(pool.capture(9).is_none());
    assert_eq!(pool.snapshot_any(9).unwrap().effective_radius(), 123.0);
}

#[test]
fn payload_validation_is_exact() {
    let good = empty_payload();
    assert!(matches!(LightPool::from_payload(&good[..good.len() - 1]), Err(Error::PayloadLength { .. })));
    let mut longer = good.clone();
    longer.push(0);
    assert!(matches!(LightPool::from_payload(&longer), Err(Error::PayloadLength { .. })));
    let mut v = good.clone();
    v[0] = 2;
    assert_eq!(LightPool::from_payload(&v), Err(Error::BadVersion(2)));
    let mut c = good.clone();
    c[4] = 0;
    c[5] = 8;
    assert_eq!(LightPool::from_payload(&c), Err(Error::BadCapacity(2048)));
}

#[test]
fn pfil_roundtrip_keeps_header_and_decoded_bytes() {
    let payload = sample();
    let mut file = lit_pfil(&payload);
    file[8] = 0xAB; // unknown header byte must survive
    let pool = LightPool::from_file_bytes(&file).unwrap();
    assert_eq!(pool.payload(), &payload[..]);
    match pool.source() {
        Source::Pfil { header } => assert_eq!(&header[..], &file[..64]),
        _ => panic!("expected PFIL source"),
    }
}

#[test]
fn pfil_strict_bounds() {
    let payload = empty_payload();
    let file = lit_pfil(&payload);
    // truncated body
    assert!(matches!(
        LightPool::from_file_bytes(&file[..file.len() - 1]),
        Err(Error::LzssTruncated { .. })
    ));
    // trailing byte
    let mut trailing = file.clone();
    trailing.push(0);
    assert!(matches!(LightPool::from_file_bytes(&trailing), Err(Error::LzssTrailingInput { unused: 1 })));
    // wrong declared length (also checked before allocating)
    let mut wrong = file.clone();
    wrong[16..20].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
    assert!(matches!(LightPool::from_file_bytes(&wrong), Err(Error::DeclaredSize { .. })));
    // truncated header
    assert!(matches!(
        LightPool::from_file_bytes(b"PFIL\0\0"),
        Err(Error::PfilHeaderTruncated { len: 6 })
    ));
}

#[test]
fn lzss_ring_initial_state_matches_game() {
    // flags 0 -> two back-references. offset 0xFF3 (inside the 18 NUL tail, untouched by the
    // first copy's writes at 0xFEE..) => NUL; offset 0 => spaces (0x20).
    let body = [0x00, 0xF3, 0xF2, 0x00, 0x02];
    assert_eq!(lzss::decode(&body, 10).unwrap(), b"\0\0\0\0\0     ");
    // literal 'A' lands at ring[0xFEE]; a len-3 copy from 0xFEE overlaps its own writes => "AAAA".
    let body = [0b0000_0001, b'A', 0xEE, 0xF0];
    assert_eq!(lzss::decode(&body, 4).unwrap(), b"AAAA");
}

#[test]
fn lzss_overshoot_is_rejected() {
    let body = [0x00, 0x00, 0x02]; // 5-byte copy into a 4-byte output
    assert!(matches!(lzss::decode(&body, 4), Err(Error::LzssOvershoot { .. })));
}

#[test]
fn ffi_roundtrip_and_error_codes() {
    unsafe {
        let payload = sample();
        let file = lit_pfil(&payload);
        let mut handle = std::ptr::null_mut();
        assert_eq!(ar_light_pool_open(file.as_ptr(), file.len(), &mut handle), code::OK);
        assert!(!handle.is_null());

        let mut count = 0u32;
        assert_eq!(ar_light_pool_active_count(handle, &mut count), code::OK);
        assert_eq!(count, 1);

        let size = ar_light_snapshot_struct_size();
        let mut snap: ArLightSnapshot = std::mem::zeroed();
        assert_eq!(ar_light_pool_snapshot(handle, 3, &mut snap), code::BAD_STRUCT_SIZE);
        snap.struct_size = size;
        assert_eq!(ar_light_pool_snapshot(handle, 3, &mut snap), code::OK);
        assert_eq!((snap.slot, snap.active_raw, snap.effective_radius, snap.baseline_radius), (3, 7, 300.0, 450.0));
        assert_eq!((snap.type_raw, snap.age_raw, snap.owner, snap.parent, snap.definition), (0xFFFE, u32::MAX, -5, 12, 39));
        assert_eq!(ar_light_pool_snapshot(handle, 4, &mut snap), code::INACTIVE_SLOT);
        assert_eq!(ar_light_pool_snapshot_any(handle, 4, &mut snap), code::OK);
        assert_eq!(ar_light_pool_snapshot(handle, 1024, &mut snap), code::INVALID_SLOT);
        assert_eq!(ar_light_pool_snapshot(std::ptr::null(), 0, &mut snap), code::NULL_ARGUMENT);

        // two-call buffer protocol
        let mut need = 0usize;
        assert_eq!(ar_light_pool_copy_payload(handle, std::ptr::null_mut(), 0, &mut need), code::BUFFER_TOO_SMALL);
        assert_eq!(need, PAYLOAD_LEN);
        let mut small = vec![0u8; 16];
        assert_eq!(ar_light_pool_copy_payload(handle, small.as_mut_ptr(), 16, &mut need), code::BUFFER_TOO_SMALL);
        let mut buf = vec![0u8; need];
        assert_eq!(ar_light_pool_copy_payload(handle, buf.as_mut_ptr(), need, &mut need), code::OK);
        assert_eq!(buf, payload);
        let mut hdr = [0u8; 64];
        let mut hlen = 0usize;
        assert_eq!(ar_light_pool_copy_pfil_header(handle, hdr.as_mut_ptr(), 64, &mut hlen), code::OK);
        assert_eq!((hlen, &hdr[..]), (64, &file[..64]));
        ar_light_pool_free(handle);
        ar_light_pool_free(std::ptr::null_mut());

        // failure nulls the out handle and reports a data error
        let mut bad = std::ptr::null_mut();
        let junk = [1u8, 2, 3];
        assert_eq!(ar_light_pool_open(junk.as_ptr(), junk.len(), &mut bad), code::PAYLOAD_LENGTH);
        assert!(bad.is_null());
        assert_eq!(ar_light_pool_open(std::ptr::null(), 0, &mut bad), code::NULL_ARGUMENT);
        let msg = std::ffi::CStr::from_ptr(ar_light_error_message(code::LZSS_TRUNCATED));
        assert_eq!(msg.to_str().unwrap(), "LZSS input truncated");
    }
}
