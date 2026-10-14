//! Synthetic saved bytes; no game data and no runtime/visual claims.
use against_rome_data::{error::code, particle::*, particle_ffi::*, sha256::sha256, Source};
use std::ptr;
fn payload() -> Vec<u8> {
    let mut p = vec![0; PAYLOAD_LEN];
    for (o, v) in [(0, 1i32), (4, 1024), (8, 64)] {
        p[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    p
}
fn patterned() -> Vec<u8> {
    let mut p = payload();
    for (i, b) in p[12..].iter_mut().enumerate() {
        *b = (i.wrapping_mul(73).wrapping_add(i / 17)) as u8;
    }
    p
}
fn put(p: &mut [u8], o: usize, v: i32) {
    p[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn pfil(p: &[u8]) -> Vec<u8> {
    let mut b = vec![0x5a; 64];
    b[..4].copy_from_slice(b"PFIL");
    b[16..20].copy_from_slice(&(p.len() as u32).to_le_bytes());
    for chunk in p.chunks(8) {
        b.push(((1u16 << chunk.len()) - 1) as u8);
        b.extend_from_slice(chunk);
    }
    b
}
#[test]
fn strict_payload_lengths_and_header() {
    let p = payload();
    assert_eq!(PAYLOAD_LEN, 2322444);
    for n in [0, 1, 4, 8, 11, 12, PAYLOAD_LEN - 1] {
        assert_eq!(
            ParticlePool::from_payload(&p[..n]).unwrap_err().code(),
            code::PAYLOAD_LENGTH
        );
    }
    let mut extra = p.clone();
    extra.push(0);
    assert_eq!(
        ParticlePool::from_payload(&extra).unwrap_err().code(),
        code::PAYLOAD_LENGTH
    );
    for (offset, value, code) in [
        (0, 2, code::BAD_VERSION),
        (4, -1, code::BAD_CAPACITY),
        (8, 63, BAD_CHILDREN),
    ] {
        let mut b = p.clone();
        put(&mut b, offset, value);
        assert_eq!(ParticlePool::from_payload(&b).unwrap_err().code(), code);
    }
}
#[test]
fn strict_pfil_roundtrip_header_hash() {
    let p = patterned();
    let b = pfil(&p);
    let pool = ParticlePool::from_file_bytes(&b).unwrap();
    assert_eq!(pool.payload(), p);
    assert_eq!(pool.input_sha256(), &sha256(&b));
    match pool.source() {
        Source::Pfil { header } => assert_eq!(&header[..], &b[..64]),
        _ => panic!("expected PFIL"),
    }
    assert!(pool.source_path().is_none());
}
#[test]
fn strict_pfil_malformed() {
    let b = pfil(&payload());
    for n in [4, 16, 63] {
        assert_eq!(
            ParticlePool::from_file_bytes(&b[..n]).unwrap_err().code(),
            code::PFIL_HEADER_TRUNCATED
        );
    }
    for n in [64, 65, 66, b.len() - 1] {
        assert_eq!(
            ParticlePool::from_file_bytes(&b[..n]).unwrap_err().code(),
            code::LZSS_TRUNCATED
        );
    }
    let mut wrong = b.clone();
    put(&mut wrong, 16, 2322443);
    assert_eq!(
        ParticlePool::from_file_bytes(&wrong).unwrap_err().code(),
        code::DECLARED_SIZE
    );
    let mut trailing = b.clone();
    trailing.push(0);
    assert_eq!(
        ParticlePool::from_file_bytes(&trailing).unwrap_err().code(),
        code::LZSS_TRAILING_INPUT
    );
    let mut over = b[..b.len() - 5].to_vec();
    over.extend_from_slice(&[0, 0, 15]);
    assert_eq!(
        ParticlePool::from_file_bytes(&over).unwrap_err().code(),
        code::LZSS_OVERSHOOT
    );
    let mut bad_magic = b;
    bad_magic[0] = b'X';
    assert_eq!(
        ParticlePool::from_file_bytes(&bad_magic)
            .unwrap_err()
            .code(),
        code::PAYLOAD_LENGTH
    );
}
#[test]
fn every_record_and_four_columns_bitwise() {
    let p = patterned();
    let pool = ParticlePool::from_payload(&p).unwrap();
    assert_eq!(pool.payload(), p);
    assert_eq!(pool.source(), &Source::RawPayload);
    for slot in 0..1024u32 {
        let s = pool.snapshot_any(slot).unwrap();
        let o = 12 + slot as usize * 2252;
        assert_eq!(&s.raw_record()[..], &p[o..o + 2252]);
        for c in 0..4 {
            let o = 12 + 1024 * 2252 + c * 4096 + slot as usize * 4;
            assert_eq!(&s.raw_state()[c * 4..c * 4 + 4], &p[o..o + 4]);
            assert_eq!(
                s.state_words()[c],
                i32::from_le_bytes(p[o..o + 4].try_into().unwrap())
            );
        }
        for ordinal in 0..64 {
            assert_eq!(
                &s.child(ordinal).unwrap().raw_bytes()[..],
                &p[o + 12 + ordinal as usize * 35..o + 12 + (ordinal as usize + 1) * 35]
            );
        }
    }
}
#[test]
fn first_last_slot_child_bounds() {
    let pool = ParticlePool::from_payload(&payload()).unwrap();
    for slot in [0, 1023] {
        let s = pool.snapshot_any(slot).unwrap();
        assert_eq!(s.slot(), slot);
        for c in [0, 63] {
            assert!(s.child(c).is_ok());
        }
        for c in [64, u32::MAX] {
            assert_eq!(s.child(c).unwrap_err().code(), code::INVALID_CHILD);
        }
    }
    for slot in [1024, u32::MAX] {
        assert_eq!(
            pool.snapshot_any(slot).unwrap_err().code(),
            code::INVALID_SLOT
        );
        assert!(pool.capture(slot).is_none());
    }
}
#[test]
fn inactive_nonone_flags_corrupt_count_separate_diagnostic() {
    let mut p = patterned();
    p[12..14].fill(0);
    put(&mut p, 14, i32::MIN);
    p[12 + 2252..14 + 2252].copy_from_slice(&0x8001u16.to_le_bytes());
    let pool = ParticlePool::from_payload(&p).unwrap();
    assert!(pool.capture(0).is_none());
    let s = pool.snapshot_any(0).unwrap();
    assert_eq!(s.count_raw(), i32::MIN);
    assert!(s.child(63).is_ok());
    assert!(s
        .validate_renderable_count()
        .unwrap_err()
        .to_string()
        .contains("-2147483648"));
    assert_eq!(pool.capture(1).unwrap().active_raw(), 0x8001);
    assert!(!pool.active_slots().any(|s| s == 0));
    assert!(pool.active_slots().any(|s| s == 1));
    for count in [-1, 0, 64, 65, i32::MAX] {
        let mut p = payload();
        put(&mut p, 14, count);
        let s = ParticlePool::from_payload(&p)
            .unwrap()
            .snapshot_any(0)
            .unwrap();
        assert_eq!(
            s.validate_renderable_count().is_ok(),
            (0..=64).contains(&count)
        );
    }
}
#[test]
fn child_and_emitter_typed_views_preserve_raw_words() {
    let mut p = payload();
    p[12..14].copy_from_slice(&7u16.to_le_bytes());
    put(&mut p, 14, 1);
    p[18..20].copy_from_slice(&(-123i16).to_le_bytes());
    put(&mut p, 20, i32::MIN);
    let o = 24;
    p[o] = 255;
    for (offset, v) in [
        (1, i32::MIN),
        (5, -65537),
        (9, i32::MAX),
        (13, -1),
        (17, 0x12345678),
        (21, -123),
        (25, -456),
    ] {
        put(&mut p, o + offset, v);
    }
    p[o + 29..o + 31].copy_from_slice(&0xffffu16.to_le_bytes());
    p[o + 31..o + 33].copy_from_slice(&0x8000u16.to_le_bytes());
    p[o + 33..o + 35].copy_from_slice(&(-1i16).to_le_bytes());
    let s = ParticlePool::from_payload(&p).unwrap().capture(0).unwrap();
    assert_eq!(s.owner(), -123);
    assert_eq!(s.age_raw(), i32::MIN);
    let c = s.child(0).unwrap();
    assert_eq!(c.active_raw(), 255);
    assert_eq!(c.position_fixed(), [i32::MIN, -65537, i32::MAX]);
    assert_eq!(c.velocity_raw(), [-1, 0x12345678, -123]);
    assert_eq!(c.lifetime_raw(), -456);
    assert_eq!(c.opaque_shorts(), [65535, 32768]);
    assert_eq!(c.light_link(), -1);
}
fn meta() -> ArParticlePoolSnapshot {
    ArParticlePoolSnapshot {
        struct_size: 24,
        slot: 99,
        active_raw: 99,
        count_raw: 99,
        owner: 99,
        reserved: 99,
        age_raw: 99,
    }
}
unsafe fn open(p: &[u8]) -> *mut ParticlePool {
    let mut h = ptr::null_mut();
    assert_eq!(ar_particle_pool_open(p.as_ptr(), p.len(), &mut h), 0);
    assert!(!h.is_null());
    h
}
#[test]
fn ffi_owned_input_snapshot_copy_free_once_and_layout_stability() {
    unsafe {
        assert_eq!(ar_particle_pool_abi_version(), 1);
        assert_eq!(ar_particle_pool_snapshot_struct_size(), 24);
        assert_eq!(
            (
                ar_particle_pool_record_size(),
                ar_particle_pool_state_size(),
                ar_particle_pool_child_size(),
                ar_particle_pool_payload_size()
            ),
            (2252, 16, 35, 2322444)
        );
        assert_eq!(against_rome_data::ffi::ar_light_abi_version(), 1);
        assert_eq!(against_rome_data::ffi::ar_light_snapshot_struct_size(), 120);
        assert_eq!(
            against_rome_data::ffi::ar_light_projection_request_struct_size(),
            36
        );
        assert_eq!(
            against_rome_data::ffi::ar_projected_light_struct_size(),
            140
        );
        assert_eq!(
            against_rome_data::ffi::ar_particle_light_request_struct_size(),
            44
        );
        assert_eq!(
            against_rome_data::ffi::ar_particle_projected_light_struct_size(),
            160
        );
        let mut p = patterned();
        let expected = p.clone();
        let h = open(&p);
        p.fill(0);
        drop(p);
        let mut record = [0; 2252];
        let mut state = [0; 16];
        let mut m = meta();
        for slot in [0, 1023] {
            assert_eq!(
                ar_particle_pool_snapshot_any(
                    h,
                    slot,
                    &mut m,
                    record.as_mut_ptr(),
                    2252,
                    state.as_mut_ptr(),
                    16
                ),
                0
            );
            let o = 12 + slot as usize * 2252;
            assert_eq!(&record[..], &expected[o..o + 2252]);
            assert_eq!(m.slot, slot);
            assert_eq!(m.reserved, 0);
            for c in 0..4 {
                let o = 12 + 1024 * 2252 + c * 4096 + slot as usize * 4;
                assert_eq!(&state[c * 4..c * 4 + 4], &expected[o..o + 4]);
            }
        }
        let mut copied = vec![0; PAYLOAD_LEN];
        let mut n = 0;
        assert_eq!(
            ar_particle_pool_copy_payload(h, copied.as_mut_ptr(), copied.len(), &mut n),
            0
        );
        assert_eq!(copied, expected);
        ar_particle_pool_free(h);
        ar_particle_pool_free(ptr::null_mut());
    }
}
#[test]
fn ffi_null_length_open_failure() {
    unsafe {
        let p = payload();
        let mut h = ptr::dangling_mut::<ParticlePool>();
        assert_eq!(
            ar_particle_pool_open(ptr::null(), 0, &mut h),
            code::NULL_ARGUMENT
        );
        assert!(h.is_null());
        assert_eq!(
            ar_particle_pool_open(p.as_ptr(), p.len(), ptr::null_mut()),
            code::NULL_ARGUMENT
        );
        for n in [0, 11, p.len() - 1, usize::MAX] {
            h = ptr::dangling_mut();
            assert_ne!(ar_particle_pool_open(p.as_ptr(), n, &mut h), 0);
            assert!(h.is_null());
        }
        let mut m = meta();
        let mut r = [77; 2252];
        let mut s = [77; 16];
        let mut n = 77;
        assert_eq!(
            ar_particle_pool_snapshot_any(
                ptr::null(),
                0,
                &mut m,
                r.as_mut_ptr(),
                r.len(),
                s.as_mut_ptr(),
                s.len()
            ),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_active_count(ptr::null(), ptr::null_mut()),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_child_any(ptr::null(), 0, 0, r.as_mut_ptr(), 35),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_validate_renderable_count(ptr::null(), 0, ptr::null_mut()),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_copy_payload(ptr::null(), r.as_mut_ptr(), 2252, &mut n),
            code::NULL_ARGUMENT
        );
        assert_eq!(n, 77);
        assert_eq!(
            ar_particle_pool_copy_pfil_header(ptr::null(), r.as_mut_ptr(), 2252, &mut n),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_copy_input_sha256(ptr::null(), r.as_mut_ptr(), 2252, &mut n),
            code::NULL_ARGUMENT
        );
    }
}
#[test]
fn ffi_snapshot_errors_leave_all_outputs_intact() {
    unsafe {
        let h = open(&payload());
        let mut m = meta();
        let mut r = [77; 2252];
        let mut s = [77; 16];
        let saved = m;
        for (slot, rc, sc, expected) in [
            (0, 2251, 16, code::BUFFER_TOO_SMALL),
            (0, 2252, 15, code::BUFFER_TOO_SMALL),
            (1024, 2252, 16, code::INVALID_SLOT),
        ] {
            assert_eq!(
                ar_particle_pool_snapshot_any(
                    h,
                    slot,
                    &mut m,
                    r.as_mut_ptr(),
                    rc,
                    s.as_mut_ptr(),
                    sc
                ),
                expected
            );
            assert_eq!(m, saved);
            assert_eq!(r, [77; 2252]);
            assert_eq!(s, [77; 16]);
        }
        assert_eq!(
            ar_particle_pool_snapshot(h, 0, &mut m, r.as_mut_ptr(), 2252, s.as_mut_ptr(), 16),
            code::INACTIVE_SLOT
        );
        assert_eq!(m, saved);
        m.struct_size = 0;
        let saved = m;
        assert_eq!(
            ar_particle_pool_snapshot_any(h, 0, &mut m, r.as_mut_ptr(), 2252, s.as_mut_ptr(), 16),
            code::BAD_STRUCT_SIZE
        );
        assert_eq!(m, saved);
        assert_eq!(
            ar_particle_pool_snapshot_any(
                h,
                0,
                ptr::null_mut(),
                r.as_mut_ptr(),
                2252,
                s.as_mut_ptr(),
                16
            ),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_snapshot_any(h, 0, &mut m, ptr::null_mut(), 2252, s.as_mut_ptr(), 16),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_snapshot_any(h, 0, &mut m, r.as_mut_ptr(), 2252, ptr::null_mut(), 16),
            code::NULL_ARGUMENT
        );
        assert_eq!(r, [77; 2252]);
        assert_eq!(s, [77; 16]);
        ar_particle_pool_free(h);
    }
}
#[test]
fn ffi_child_and_count_diagnostic_safe_outputs() {
    unsafe {
        let mut p = patterned();
        p[12..14].fill(0);
        put(&mut p, 14, 65);
        let h = open(&p);
        let mut b = [88; 35];
        for (slot, ordinal, cap, expected) in [
            (0, 0, 34, code::BUFFER_TOO_SMALL),
            (1024, 0, 35, code::INVALID_SLOT),
            (0, 64, 35, code::INVALID_CHILD),
            (0, u32::MAX, 35, code::INVALID_CHILD),
        ] {
            assert_eq!(
                ar_particle_pool_child_any(h, slot, ordinal, b.as_mut_ptr(), cap),
                expected
            );
            assert_eq!(b, [88; 35]);
        }
        for ordinal in [0, 63] {
            assert_eq!(
                ar_particle_pool_child_any(h, 0, ordinal, b.as_mut_ptr(), 35),
                0
            );
            let o = 24 + ordinal as usize * 35;
            assert_eq!(&b[..], &p[o..o + 35]);
        }
        assert_eq!(
            ar_particle_pool_child_any(h, 0, 0, ptr::null_mut(), 35),
            code::NULL_ARGUMENT
        );
        let mut n = 99;
        assert_eq!(
            ar_particle_pool_validate_renderable_count(h, 0, &mut n),
            BAD_RENDERABLE_COUNT
        );
        assert_eq!(n, 99);
        assert_eq!(
            ar_particle_pool_validate_renderable_count(h, 1024, &mut n),
            code::INVALID_SLOT
        );
        assert_eq!(n, 99);
        assert_eq!(
            ar_particle_pool_validate_renderable_count(h, 0, ptr::null_mut()),
            code::NULL_ARGUMENT
        );
        assert_eq!(
            ar_particle_pool_active_count(h, ptr::null_mut()),
            code::NULL_ARGUMENT
        );
        assert_eq!(ar_particle_pool_active_count(h, &mut n), 0);
        assert_eq!(n, (*h).active_slots().count() as u32);
        ar_particle_pool_free(h);
        let mut p = payload();
        put(&mut p, 14, 64);
        p[12..14].copy_from_slice(&9u16.to_le_bytes());
        let h = open(&p);
        assert_eq!(ar_particle_pool_validate_renderable_count(h, 0, &mut n), 0);
        assert_eq!(n, 64);
        let mut m = meta();
        let mut r = [0; 2252];
        let mut s = [0; 16];
        assert_eq!(
            ar_particle_pool_snapshot(h, 0, &mut m, r.as_mut_ptr(), 2252, s.as_mut_ptr(), 16),
            0
        );
        assert_eq!(m.active_raw, 9);
        ar_particle_pool_free(h);
    }
}
#[test]
fn ffi_copy_queries_short_buffers_pfil_and_hash() {
    unsafe {
        let p = payload();
        let packed = pfil(&p);
        for b in [&p, &packed] {
            let h = open(b);
            let mut n = 99;
            let mut out = [88; 64];
            assert_eq!(
                ar_particle_pool_copy_payload(h, ptr::null_mut(), 0, &mut n),
                code::BUFFER_TOO_SMALL
            );
            assert_eq!(n, PAYLOAD_LEN);
            assert_eq!(
                ar_particle_pool_copy_payload(h, out.as_mut_ptr(), 64, &mut n),
                code::BUFFER_TOO_SMALL
            );
            assert_eq!(out, [88; 64]);
            assert_eq!(
                ar_particle_pool_copy_payload(h, out.as_mut_ptr(), 64, ptr::null_mut()),
                code::NULL_ARGUMENT
            );
            assert_eq!(
                ar_particle_pool_copy_input_sha256(h, ptr::null_mut(), 0, &mut n),
                code::BUFFER_TOO_SMALL
            );
            assert_eq!(n, 32);
            assert_eq!(
                ar_particle_pool_copy_input_sha256(h, out.as_mut_ptr(), 31, &mut n),
                code::BUFFER_TOO_SMALL
            );
            assert_eq!(out, [88; 64]);
            assert_eq!(
                ar_particle_pool_copy_input_sha256(h, out.as_mut_ptr(), 64, &mut n),
                0
            );
            assert_eq!(&out[..32], &sha256(b));
            assert_eq!(
                ar_particle_pool_copy_input_sha256(h, out.as_mut_ptr(), 64, ptr::null_mut()),
                code::NULL_ARGUMENT
            );
            let wrapped = b.len() != PAYLOAD_LEN;
            assert_eq!(
                ar_particle_pool_copy_pfil_header(h, ptr::null_mut(), 0, &mut n),
                if wrapped { code::BUFFER_TOO_SMALL } else { 0 }
            );
            assert_eq!(n, if wrapped { 64 } else { 0 });
            assert_eq!(
                ar_particle_pool_copy_pfil_header(h, out.as_mut_ptr(), 63, &mut n),
                if wrapped { code::BUFFER_TOO_SMALL } else { 0 }
            );
            assert_eq!(
                ar_particle_pool_copy_pfil_header(h, out.as_mut_ptr(), 64, &mut n),
                0
            );
            if wrapped {
                assert_eq!(&out[..], &b[..64]);
            }
            assert_eq!(
                ar_particle_pool_copy_pfil_header(h, out.as_mut_ptr(), 64, ptr::null_mut()),
                code::NULL_ARGUMENT
            );
            ar_particle_pool_free(h);
        }
    }
}
#[test]
fn read_only_load_keeps_path_digest_bytes_and_metadata() {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../re_workspace/particle-core-target")
        });
    std::fs::create_dir_all(&target).unwrap();
    let path = target.join(format!("particle-fixture-{}.dat", std::process::id()));
    let b = pfil(&patterned());
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&b).unwrap();
    }
    let before = std::fs::metadata(&path).unwrap();
    let pool = ParticlePool::load(&path).unwrap();
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(pool.source_path(), Some(path.as_path()));
    assert_eq!(pool.input_sha256(), &sha256(&b));
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(ParticlePool::load(&path).unwrap_err().code(), IO_ERROR);
}

#[test]
fn ffi_data_failures_clear_handle_and_particle_diagnostics() {
    unsafe {
        let mut p = payload();
        for (offset, value, expected) in [
            (0, -1, code::BAD_VERSION),
            (4, 1023, code::BAD_CAPACITY),
            (8, 65, BAD_CHILDREN),
        ] {
            let old: [u8; 4] = p[offset..offset + 4].try_into().unwrap();
            put(&mut p, offset, value);
            let mut h = ptr::dangling_mut::<ParticlePool>();
            assert_eq!(ar_particle_pool_open(p.as_ptr(), p.len(), &mut h), expected);
            assert!(h.is_null());
            p[offset..offset + 4].copy_from_slice(&old);
        }
        let mut b = pfil(&p);
        b.push(0);
        let mut h = ptr::dangling_mut::<ParticlePool>();
        assert_eq!(
            ar_particle_pool_open(b.as_ptr(), b.len(), &mut h),
            code::LZSS_TRAILING_INPUT
        );
        assert!(h.is_null());
        for (code, fragment) in [
            (code::PAYLOAD_LENGTH, "2322444"),
            (BAD_CHILDREN, "64"),
            (BAD_RENDERABLE_COUNT, "count"),
            (IO_ERROR, "read failed"),
        ] {
            let message = std::ffi::CStr::from_ptr(ar_particle_pool_error_message(code))
                .to_str()
                .unwrap();
            assert!(message.contains(fragment));
        }
        assert_eq!(
            std::ffi::CStr::from_ptr(ar_particle_pool_error_message(code::OK)).to_bytes(),
            b"ok"
        );
        assert_eq!(
            std::ffi::CStr::from_ptr(ar_particle_pool_error_message(-1)).to_bytes(),
            b"unknown error"
        );
    }
}
