//! Synthetic tests for particle-linked saved-light projection (no game data).

use against_rome_data::error::code;
use against_rome_data::ffi::*;
use against_rome_data::pool::WIDTHS;
use against_rome_data::projection::{CHILD_COUNT, EMITTER_RECORD_LEN};
use against_rome_data::{project_particle_batch, Error, LightPool, ParticleLightRequest, CAPACITY, PAYLOAD_LEN};

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

fn put(p: &mut [u8], col: usize, slot: usize, b: &[u8]) {
    let o = COL[col] + slot * WIDTHS[col];
    p[o..o + b.len()].copy_from_slice(b);
}

fn light(p: &mut [u8], slot: usize, xyz: [f32; 3], active: u32) {
    put(p, 0, slot, &active.to_le_bytes());
    for (i, v) in xyz.iter().enumerate() {
        put(p, 1 + i, slot, &v.to_bits().to_le_bytes());
    }
    put(p, 4, slot, &3.0f32.to_le_bytes());
    put(p, 7, slot, &300.0f32.to_le_bytes());
    put(p, 8, slot, &450.0f32.to_le_bytes());
    put(p, 12, slot, &0xDEAD_BEEFu32.to_le_bytes());
    put(p, 14, slot, &12i16.to_le_bytes());
}

fn pool_bytes() -> Vec<u8> {
    let mut p = vec![0u8; PAYLOAD_LEN];
    p[0..4].copy_from_slice(&1i32.to_le_bytes());
    p[4..8].copy_from_slice(&1024i32.to_le_bytes());
    for c in 13..16 {
        p[COL[c]..COL[c] + CAPACITY * 2].fill(0xFF);
    }
    light(&mut p, 5, [100.25, -7.5, 33.0], 1); // active child's light, saved value deliberately stale
    light(&mut p, 6, [-0.0, 1.5, f32::from_bits(0x7FC0_1234)], 1); // inactive child's light
    light(&mut p, 7, [1.0, 2.0, 3.0], 0); // light inactive in pool
    p
}

fn pool() -> LightPool {
    LightPool::from_payload(&pool_bytes()).unwrap()
}

fn set_child(rec: &mut [u8], ord: usize, active: u8, fixed: [i32; 3], link: i16) {
    let o = 12 + 35 * ord;
    rec[o] = active;
    for (a, v) in fixed.iter().enumerate() {
        rec[o + 1 + 4 * a..o + 5 + 4 * a].copy_from_slice(&v.to_le_bytes());
    }
    rec[o + 33..o + 35].copy_from_slice(&link.to_le_bytes());
}

/// Record with unknown filler bytes everywhere so "preserved" is observable. All links -1.
fn record(active: u16) -> Vec<u8> {
    let mut r: Vec<u8> = (0..EMITTER_RECORD_LEN).map(|i| (i * 7 + 3) as u8).collect();
    r[0..2].copy_from_slice(&active.to_le_bytes());
    for c in 0..CHILD_COUNT as usize {
        set_child(&mut r, c, 0, [0x1111, 0x2222, 0x3333], -1);
    }
    r
}

fn req(record_index: u32, child_ordinal: u32, light_slot: u32, d: [i32; 3]) -> ParticleLightRequest {
    ParticleLightRequest { record_index, child_ordinal, light_slot, target_guid: [light_slot as u8; 16], delta_fixed: d }
}

#[test]
fn active_child_uses_fixed_plus_delta_then_floor_not_float_delta() {
    let mut r = record(2);
    // x = 3.75 fixed; y = -3.75 fixed; z = 0.
    set_child(&mut r, 4, 1, [0x3_C000, -0x3_C000, 0], 5);
    let p = pool();
    let out = project_particle_batch(&p, &r, &[req(0, 4, 5, [0, 0, 0])]).unwrap();
    // zero delta still derives SAR16 (floor), NOT the stale saved light bits 100.25/-7.5/33.
    assert_eq!(out[0].snapshot.position(), [3.0, -4.0, 0.0]);
    assert_eq!(out[0].fixed_position, [0x3_C000, -0x3_C000, 0]);
    assert_eq!(out[0].child_active_raw, 1);
    // direct float projection of the same request keeps the stale saved bits: the two paths differ.
    let direct = against_rome_data::project_batch(
        &p,
        &[against_rome_data::LightProjectionRequest { slot: 5, target_guid: [0; 16], delta_fixed: [0; 3] }],
    )
    .unwrap();
    assert_eq!(direct[0].snapshot.position(), [100.25, -7.5, 33.0]);
    assert_ne!(direct[0].snapshot.position(), out[0].snapshot.position());
}

#[test]
fn negative_fraction_floors_toward_negative_infinity() {
    let mut r = record(1);
    set_child(&mut r, 0, 1, [0, 0, 0], 5);
    let p = pool();
    // x: -1 -> floor(-1/65536) = -1; y: +65535 -> 0; z: -65536 -> -1 (float delta would give ~-0, ~1, -1).
    let out = project_particle_batch(&p, &r, &[req(0, 0, 5, [-1, 65535, -65536])]).unwrap();
    assert_eq!(out[0].snapshot.position(), [-1.0, 0.0, -1.0]);
    // -65537 -> floor = -2
    let out = project_particle_batch(&p, &r, &[req(0, 0, 5, [-65537, 65536, 0x7FFF_FFFF])]).unwrap();
    assert_eq!(out[0].snapshot.position(), [-2.0, 1.0, 32767.0]);
    assert_eq!(out[0].fixed_position[2], 0x7FFF_FFFF);
    // truncating division (C-style) would give -1 / 0 for -65537/65536 and -1/65536; floor must differ.
    let out = project_particle_batch(&p, &r, &[req(0, 0, 5, [i32::MIN, 0, 0])]).unwrap();
    assert_eq!(out[0].snapshot.position()[0], -32768.0);
}

#[test]
fn nonzero_child_active_byte_counts_as_active() {
    let mut r = record(1);
    set_child(&mut r, 0, 0x80, [0x2_0000, 0, 0], 5);
    let out = project_particle_batch(&pool(), &r, &[req(0, 0, 5, [0, 0, 0])]).unwrap();
    assert_eq!((out[0].snapshot.position()[0], out[0].child_active_raw), (2.0, 0x80));
}

#[test]
fn inactive_child_keeps_saved_light_bits_and_fixed_unshifted() {
    let mut r = record(1);
    set_child(&mut r, 9, 0, [0x5_0000, 0x6_0000, 0x7_0000], 6);
    let p = pool();
    let out = project_particle_batch(&p, &r, &[req(0, 9, 6, [0x10_0000, -0x10_0000, 12345])]).unwrap();
    let src = p.snapshot_any(6).unwrap();
    assert_eq!(out[0].snapshot.raw_fields(), src.raw_fields()); // incl -0.0 and NaN payload
    assert_eq!(out[0].fixed_position, [0x5_0000, 0x6_0000, 0x7_0000]);
    assert_eq!(out[0].child_active_raw, 0);
}

#[test]
fn only_light_xyz_changes_and_sources_immutable() {
    let mut r = record(1);
    set_child(&mut r, 1, 1, [0x1_0000, 0x2_0000, 0x3_0000], 5);
    let p = pool();
    let (rec_before, payload_before) = (r.clone(), p.payload().to_vec());
    let out = project_particle_batch(&p, &r, &[req(0, 1, 5, [0x1_0000, 0, 0])]).unwrap();
    let src = *p.snapshot_any(5).unwrap().raw_fields();
    let got = out[0].snapshot.raw_fields();
    assert_eq!(src[..4], got[..4]);
    assert_eq!(src[16..], got[16..]);
    assert_eq!(out[0].snapshot.position(), [2.0, 2.0, 3.0]);
    assert_eq!(r, rec_before);
    assert_eq!(p.payload(), &payload_before[..]);
}

#[test]
fn overflow_is_checked_for_each_axis() {
    let mut r = record(1);
    set_child(&mut r, 2, 1, [i32::MAX, i32::MIN, 0], 5);
    let p = pool();
    let e = project_particle_batch(&p, &r, &[req(0, 2, 5, [1, 0, 0])]).unwrap_err();
    assert_eq!((e.index, e.error), (0, Error::FixedOverflow { ordinal: 2, axis: 0 }));
    let e = project_particle_batch(&p, &r, &[req(0, 2, 5, [0, -1, 0])]).unwrap_err();
    assert_eq!(e.error, Error::FixedOverflow { ordinal: 2, axis: 1 });
    // boundary values that do not overflow succeed
    assert!(project_particle_batch(&p, &r, &[req(0, 2, 5, [-1, 1, 0])]).is_ok());
    // inactive child never overflows: delta is not applied
    set_child(&mut r, 2, 0, [i32::MAX, i32::MIN, 0], 5);
    assert!(project_particle_batch(&p, &r, &[req(0, 2, 5, [i32::MAX, i32::MIN, 0])]).is_ok());
}

#[test]
fn bad_child_link_light_emitter_and_indexes() {
    let mut r = record(1);
    set_child(&mut r, 0, 1, [0, 0, 0], 5);
    set_child(&mut r, 1, 1, [0, 0, 0], -1);
    set_child(&mut r, 2, 1, [0, 0, 0], 7); // links pool-inactive light
    set_child(&mut r, 3, 1, [0, 0, 0], 2000); // link out of range
    let mut dead = record(0);
    set_child(&mut dead, 0, 1, [0, 0, 0], 5);
    let mut both = r.clone();
    both.extend_from_slice(&dead);
    let p = pool();
    let err = |reqs: &[ParticleLightRequest]| project_particle_batch(&p, &both, reqs).unwrap_err().error;
    assert_eq!(err(&[req(0, 64, 5, [0; 3])]), Error::InvalidChild(64));
    assert_eq!(err(&[req(0, 0, 6, [0; 3])]), Error::LightLinkMismatch { ordinal: 0, expected: 6, actual: 5 });
    assert_eq!(err(&[req(0, 1, 5, [0; 3])]), Error::LightLinkMismatch { ordinal: 1, expected: 5, actual: -1 });
    assert_eq!(err(&[req(0, 3, 5, [0; 3])]), Error::LightLinkMismatch { ordinal: 3, expected: 5, actual: 2000 });
    assert_eq!(err(&[req(0, 3, 2000, [0; 3])]), Error::InvalidSlot(2000));
    assert_eq!(err(&[req(0, 2, 7, [0; 3])]), Error::InactiveSlot(7));
    assert_eq!(err(&[req(1, 0, 5, [0; 3])]), Error::InactiveEmitter(1));
    assert_eq!(err(&[req(2, 0, 5, [0; 3])]), Error::InvalidRecordIndex(2));
    let e = project_particle_batch(&p, &both[..2000], &[]).unwrap_err();
    assert_eq!(e.error, Error::RecordsLength(2000));
    assert!(project_particle_batch(&p, &[], &[]).unwrap().is_empty());
}

#[test]
fn mixed_batch_is_atomic_and_duplicates_are_independent() {
    let mut r = record(1);
    set_child(&mut r, 0, 1, [0x1_0000, 0, 0], 5);
    set_child(&mut r, 1, 0, [0, 0, 0], 6);
    let p = pool();
    let ok = req(0, 0, 5, [0x1_0000, 0, 0]);
    let out = project_particle_batch(&p, &r, &[ok, ok, req(0, 1, 6, [9, 9, 9])]).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].snapshot, out[1].snapshot);
    assert_eq!(out.iter().map(|o| o.request_index).collect::<Vec<_>>(), [0, 1, 2]);
    let bad = req(0, 0, 6, [0; 3]);
    let e = project_particle_batch(&p, &r, &[ok, ok, bad, ok]).unwrap_err();
    assert_eq!((e.index, e.error.code()), (2, code::LIGHT_LINK_MISMATCH));
}

// ---- FFI ----

fn ffi_req(rec: u32, ord: u32, light: u32, d: [i32; 3]) -> ArParticleLightRequest {
    ArParticleLightRequest {
        struct_size: ar_particle_light_request_struct_size(),
        record_index: rec,
        child_ordinal: ord,
        light_slot: light,
        target_guid: [light as u8; 16],
        delta_fixed: d,
    }
}

fn blank() -> ArParticleProjectedLight {
    let mut o: ArParticleProjectedLight = unsafe { std::mem::zeroed() };
    o.struct_size = ar_particle_projected_light_struct_size();
    o.slot = 0xABCD_EF01;
    o
}

fn open() -> *mut LightPool {
    let p = pool_bytes();
    let mut h = std::ptr::null_mut();
    assert_eq!(unsafe { ar_light_pool_open(p.as_ptr(), p.len(), &mut h) }, code::OK);
    h
}

#[test]
fn ffi_layout_sizes_and_offsets() {
    assert_eq!(std::mem::size_of::<ArParticleLightRequest>(), 44);
    assert_eq!(std::mem::size_of::<ArParticleProjectedLight>(), 160);
    assert_eq!(ar_particle_light_request_struct_size(), 44);
    assert_eq!(ar_particle_projected_light_struct_size(), 160);
    // existing ABI untouched
    assert_eq!(std::mem::size_of::<ArLightSnapshot>(), 120);
    assert_eq!(ar_light_projection_request_struct_size(), 36);
    assert_eq!(ar_projected_light_struct_size(), 140);
    assert_eq!(ar_light_abi_version(), 1);
    assert_eq!(std::mem::offset_of!(ArParticleLightRequest, record_index), 4);
    assert_eq!(std::mem::offset_of!(ArParticleLightRequest, child_ordinal), 8);
    assert_eq!(std::mem::offset_of!(ArParticleLightRequest, light_slot), 12);
    assert_eq!(std::mem::offset_of!(ArParticleLightRequest, target_guid), 16);
    assert_eq!(std::mem::offset_of!(ArParticleLightRequest, delta_fixed), 32);
    assert_eq!(std::mem::offset_of!(ArParticleProjectedLight, raw_fields), 84);
    assert_eq!(std::mem::offset_of!(ArParticleProjectedLight, child_ordinal), 140);
    assert_eq!(std::mem::offset_of!(ArParticleProjectedLight, child_active_raw), 144);
    assert_eq!(std::mem::offset_of!(ArParticleProjectedLight, fixed_position), 148);
}

#[test]
fn ffi_success_atomic_failure_and_boundaries() {
    unsafe {
        let h = open();
        let mut r = record(1);
        set_child(&mut r, 0, 1, [0x3_C000, 0, 0], 5);
        set_child(&mut r, 1, 0, [7, 8, 9], 6);
        let reqs = [ffi_req(0, 0, 5, [0x1_0000, 0, 0]), ffi_req(0, 1, 6, [5, 5, 5])];
        let mut outs = [blank(), blank()];
        let (mut n, mut fi) = (99usize, 7u32);
        let rc = ar_light_pool_project_particle_batch(
            h, r.as_ptr(), 1, reqs.as_ptr(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi,
        );
        assert_eq!((rc, n, fi), (code::OK, 2, u32::MAX));
        assert_eq!(outs[0].position, [4.0, 0.0, 0.0]);
        assert_eq!((outs[0].child_ordinal, outs[0].child_active_raw, outs[0].fixed_position), (0, 1, [0x4_C000, 0, 0]));
        assert_eq!(outs[1].fixed_position, [7, 8, 9]);
        assert_eq!((outs[1].request_index, outs[1].target_guid), (1, [6u8; 16]));
        assert_eq!(outs[1].position[0].to_bits(), (-0.0f32).to_bits());
        assert_eq!(outs[1].position[2].to_bits(), 0x7FC0_1234);

        // failure at index 1: nothing written, failed_index set, count 0
        let bad = [ffi_req(0, 0, 5, [1, 0, 0]), ffi_req(0, 0, 6, [0; 3])];
        let mut outs = [blank(), blank()];
        let rc = ar_light_pool_project_particle_batch(
            h, r.as_ptr(), 1, bad.as_ptr(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi,
        );
        assert_eq!((rc, n, fi), (code::LIGHT_LINK_MISMATCH, 0, 1));
        assert!(outs.iter().all(|o| o.slot == 0xABCD_EF01 && o.raw_fields == [0u8; 56] && o.fixed_position == [0; 3]));

        let call = |rec: *const u8, rc_: usize, rq: &[ArParticleLightRequest], o: &mut [ArParticleProjectedLight],
                    cap: usize, n: *mut usize, fi: *mut u32, pool: *const LightPool| {
            ar_light_pool_project_particle_batch(pool, rec, rc_, rq.as_ptr(), rq.len(), o.as_mut_ptr(), cap, n, fi)
        };
        // capacity
        assert_eq!(call(r.as_ptr(), 1, &reqs, &mut outs, 1, &mut n, &mut fi, h), code::BUFFER_TOO_SMALL);
        assert_eq!((n, outs[0].slot), (2, 0xABCD_EF01));
        // nulls
        assert_eq!(call(r.as_ptr(), 1, &reqs, &mut outs, 2, std::ptr::null_mut(), &mut fi, h), code::NULL_ARGUMENT);
        assert_eq!(call(std::ptr::null(), 1, &reqs, &mut outs, 2, &mut n, &mut fi, h), code::NULL_ARGUMENT);
        assert_eq!(call(r.as_ptr(), 1, &reqs, &mut outs, 2, &mut n, &mut fi, std::ptr::null()), code::NULL_ARGUMENT);
        // bad struct sizes (request, then output)
        let mut sz = reqs;
        sz[1].struct_size = 3;
        assert_eq!(call(r.as_ptr(), 1, &sz, &mut outs, 2, &mut n, &mut fi, h), code::BAD_STRUCT_SIZE);
        assert_eq!(fi, 1);
        outs[0].struct_size = 0;
        assert_eq!(call(r.as_ptr(), 1, &reqs, &mut outs, 2, &mut n, &mut fi, h), code::BAD_STRUCT_SIZE);
        assert_eq!((fi, outs[1].slot), (0, 0xABCD_EF01));
        outs[0].struct_size = ar_particle_projected_light_struct_size();
        // record index / records length / overflow
        let idx = [ffi_req(1, 0, 5, [0; 3])];
        assert_eq!(call(r.as_ptr(), 1, &idx, &mut outs, 2, &mut n, &mut fi, h), code::INVALID_RECORD_INDEX);
        assert_eq!(call(std::ptr::null(), 0, &idx, &mut outs, 2, &mut n, &mut fi, h), code::INVALID_RECORD_INDEX);
        assert_eq!(call(r.as_ptr(), usize::MAX, &idx, &mut outs, 2, &mut n, &mut fi, h), code::BAD_RECORDS_LENGTH);
        set_child(&mut r, 2, 1, [i32::MAX, 0, 0], 5);
        let ov = [ffi_req(0, 2, 5, [1, 0, 0])];
        assert_eq!(call(r.as_ptr(), 1, &ov, &mut outs, 2, &mut n, &mut fi, h), code::FIXED_OVERFLOW);
        assert_eq!((n, fi), (0, 0));
        assert!(outs.iter().all(|o| o.slot == 0xABCD_EF01));
        // count 0 with null arrays is fine
        n = 5;
        let rc = ar_light_pool_project_particle_batch(
            h, std::ptr::null(), 0, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut n, &mut fi,
        );
        assert_eq!((rc, n), (code::OK, 0));
        for c in [code::INVALID_RECORD_INDEX, code::FIXED_OVERFLOW, code::BAD_RECORDS_LENGTH, code::NONFINITE_POSITION] {
            let m = std::ffi::CStr::from_ptr(ar_light_error_message(c)).to_str().unwrap();
            assert_ne!(m, "unknown error");
        }
        ar_light_pool_free(h);
    }
}
