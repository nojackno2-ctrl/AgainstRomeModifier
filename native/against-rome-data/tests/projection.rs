//! Synthetic-data tests for saved-light projection (no game data).

use against_rome_data::error::code;
use against_rome_data::ffi::*;
use against_rome_data::pool::WIDTHS;
use against_rome_data::{project_batch, Error, LightPool, LightProjectionRequest, CAPACITY, PAYLOAD_LEN};

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

fn empty() -> Vec<u8> {
    let mut p = vec![0u8; PAYLOAD_LEN];
    p[0..4].copy_from_slice(&1i32.to_le_bytes());
    p[4..8].copy_from_slice(&1024i32.to_le_bytes());
    for c in 13..16 {
        p[COL[c]..COL[c] + CAPACITY * 2].fill(0xFF);
    }
    p
}

fn put(p: &mut [u8], col: usize, slot: usize, b: &[u8]) {
    let o = COL[col] + slot * WIDTHS[col];
    p[o..o + b.len()].copy_from_slice(b);
}

fn light(p: &mut [u8], slot: usize, xyz: [u32; 3], active: u32) {
    put(p, 0, slot, &active.to_le_bytes());
    for (i, b) in xyz.iter().enumerate() {
        put(p, 1 + i, slot, &b.to_le_bytes());
    }
    put(p, 4, slot, &3.0f32.to_le_bytes()); // RGB > 1
    put(p, 5, slot, &0.5f32.to_le_bytes());
    put(p, 6, slot, &0.25f32.to_le_bytes());
    put(p, 7, slot, &300.0f32.to_le_bytes()); // unequal radii
    put(p, 8, slot, &450.0f32.to_le_bytes());
    put(p, 9, slot, &0xFFFEu16.to_le_bytes());
    put(p, 10, slot, &0.15f32.to_le_bytes());
    put(p, 11, slot, &0xFFFF_FFFFu32.to_le_bytes());
    put(p, 12, slot, &0xDEAD_BEEFu32.to_le_bytes());
    put(p, 13, slot, &(-5i16).to_le_bytes());
    put(p, 14, slot, &12i16.to_le_bytes());
    put(p, 15, slot, &39i16.to_le_bytes());
}

fn pool() -> LightPool {
    let mut p = empty();
    light(&mut p, 3, [1.5f32.to_bits(), (-0.0f32).to_bits(), 100.0f32.to_bits()], 7); // active != 1
    light(&mut p, 9, [10.0f32.to_bits(), 20.0f32.to_bits(), 30.0f32.to_bits()], 1);
    light(&mut p, 20, [f32::NAN.to_bits(), 0.0f32.to_bits(), f32::MAX.to_bits()], 1);
    light(&mut p, 30, [0.0f32.to_bits(); 3], 0); // inactive
    LightPool::from_payload(&p).unwrap()
}

fn req(slot: u32, d: [i32; 3]) -> LightProjectionRequest {
    LightProjectionRequest { slot, target_guid: [slot as u8; 16], delta_fixed: d }
}

/// Everything except bytes 4..16 must equal the saved snapshot.
fn assert_rest_preserved(pool: &LightPool, slot: u32, got: &[u8; 56]) {
    let src = *pool.snapshot_any(slot).unwrap().raw_fields();
    assert_eq!(src[..4], got[..4]);
    assert_eq!(src[16..], got[16..]);
}

#[test]
fn projects_and_preserves_other_44_bytes() {
    let p = pool();
    let out = project_batch(&p, &[req(9, [0x0001_8000, -0x0000_8000, 0])]).unwrap();
    assert_eq!(out[0].snapshot.position(), [11.5, 19.5, 30.0]);
    assert_eq!(out[0].snapshot.position_bits()[2], 30.0f32.to_bits());
    assert_rest_preserved(&p, 9, out[0].snapshot.raw_fields());
    let s = out[0].snapshot;
    assert_eq!((s.color(), s.effective_radius(), s.baseline_radius()), ([3.0, 0.5, 0.25], 300.0, 450.0));
    assert_eq!(out[0].target_guid, [9u8; 16]);
}

#[test]
fn raw_bits_kept_when_delta_zero_including_negative_zero_and_nan() {
    let p = pool();
    let out = project_batch(&p, &[req(3, [65536, 0, 0]), req(20, [0, 0, 0])]).unwrap();
    assert_eq!(out[0].snapshot.position_bits()[1], (-0.0f32).to_bits());
    assert_eq!(out[0].snapshot.position_bits()[0], 2.5f32.to_bits());
    assert_eq!(out[0].snapshot.active_raw(), 7);
    assert_eq!(out[1].snapshot.raw_fields(), p.snapshot_any(20).unwrap().raw_fields());
}

#[test]
fn nonzero_delta_on_negative_zero_axis_changes_value() {
    let p = pool();
    let out = project_batch(&p, &[req(3, [0, -1, 0])]).unwrap();
    assert_eq!(out[0].snapshot.position()[1], -1.0 / 65536.0);
}

#[test]
fn negative_fraction_and_truncation_free_division() {
    let p = pool();
    let out = project_batch(&p, &[req(9, [-1, -65535, i32::MIN])]).unwrap();
    let pos = out[0].snapshot.position();
    assert_eq!(pos[0], (10.0f64 - 1.0 / 65536.0) as f32);
    assert_eq!(pos[1], (20.0f64 - 65535.0 / 65536.0) as f32);
    assert_eq!(pos[2], (30.0f64 - 32768.0) as f32);
}

#[test]
fn copy_of_copy_does_not_accumulate_from_source() {
    let p = pool();
    let a = project_batch(&p, &[req(9, [0x10000, 0, 0])]).unwrap();
    let b = project_batch(&p, &[req(9, [0x20000, 0, 0])]).unwrap();
    assert_eq!(a[0].snapshot.position()[0], 11.0);
    assert_eq!(b[0].snapshot.position()[0], 12.0); // source slot still 10.0, not 11.0
    assert_eq!(p.snapshot_any(9).unwrap().position()[0], 10.0);
}

#[test]
fn duplicate_slot_and_guid_are_independent_instances() {
    let p = pool();
    let r = req(9, [65536, 0, 0]);
    let out = project_batch(&p, &[r, r, req(9, [0, 0, 0])]).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out.iter().map(|o| o.request_index).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(out[0].snapshot, out[1].snapshot);
    assert_eq!(out[2].snapshot.position()[0], 10.0);
}

#[test]
fn mixed_bad_batch_returns_no_partial_output() {
    let p = pool();
    let before = p.payload().to_vec();
    let cases: [(LightProjectionRequest, Error); 3] = [
        (req(30, [1, 0, 0]), Error::InactiveSlot(30)),
        (req(1024, [0, 0, 0]), Error::InvalidSlot(1024)),
        (req(20, [1, 0, 0]), Error::NonFinitePosition { slot: 20, axis: 0 }),
    ];
    for (bad, want) in cases {
        let e = project_batch(&p, &[req(9, [1, 1, 1]), bad, req(3, [1, 1, 1])]).unwrap_err();
        assert_eq!((e.index, e.error), (1, want));
    }
    assert_eq!(p.payload(), &before[..]);
}

#[test]
fn empty_batch_ok() {
    assert!(project_batch(&pool(), &[]).unwrap().is_empty());
}

// ---- FFI boundary ----

fn ffi_req(slot: u32, d: [i32; 3]) -> ArLightProjectionRequest {
    ArLightProjectionRequest {
        struct_size: ar_light_projection_request_struct_size(),
        slot,
        target_guid: [slot as u8; 16],
        delta_fixed: d,
    }
}

fn blank_out() -> ArProjectedLight {
    let mut o: ArProjectedLight = unsafe { std::mem::zeroed() };
    o.struct_size = ar_projected_light_struct_size();
    o.slot = 0xABCD_EF01; // sentinel
    o
}

fn open() -> *mut LightPool {
    let mut p = empty();
    light(&mut p, 3, [1.5f32.to_bits(), (-0.0f32).to_bits(), 100.0f32.to_bits()], 7);
    light(&mut p, 9, [10.0f32.to_bits(), 20.0f32.to_bits(), 30.0f32.to_bits()], 1);
    light(&mut p, 30, [0; 3], 0);
    let mut h = std::ptr::null_mut();
    assert_eq!(unsafe { ar_light_pool_open(p.as_ptr(), p.len(), &mut h) }, code::OK);
    h
}

#[test]
fn ffi_struct_sizes_and_existing_snapshot_layout_unchanged() {
    assert_eq!(std::mem::size_of::<ArLightSnapshot>(), 120);
    assert_eq!(ar_light_projection_request_struct_size(), 36);
    assert_eq!(ar_projected_light_struct_size(), 140);
    assert_eq!(ar_light_abi_version(), 1);
}

#[test]
fn ffi_success_and_atomic_failure() {
    unsafe {
        let h = open();
        let reqs = [ffi_req(9, [0x10000, 0, 0]), ffi_req(3, [0, 0, 0]), ffi_req(9, [0x10000, 0, 0])];
        let mut outs = [blank_out(), blank_out(), blank_out()];
        let (mut n, mut fi) = (99usize, 7u32);
        let rc = ar_light_pool_project_batch(h, reqs.as_ptr(), 3, outs.as_mut_ptr(), 3, &mut n, &mut fi);
        assert_eq!((rc, n, fi), (code::OK, 3, u32::MAX));
        assert_eq!(outs[0].position, [11.0, 20.0, 30.0]);
        assert_eq!(outs[1].position[1].to_bits(), (-0.0f32).to_bits());
        assert_eq!((outs[1].active_raw, outs[2].request_index, outs[2].target_guid), (7, 2, [9u8; 16]));
        assert_eq!(outs[0].raw_fields[16..], outs[2].raw_fields[16..]);

        // failure at index 2 (inactive): outputs untouched
        let bad = [ffi_req(9, [1, 0, 0]), ffi_req(3, [1, 0, 0]), ffi_req(30, [0, 0, 0])];
        let mut outs = [blank_out(), blank_out(), blank_out()];
        let rc = ar_light_pool_project_batch(h, bad.as_ptr(), 3, outs.as_mut_ptr(), 3, &mut n, &mut fi);
        assert_eq!((rc, n, fi), (code::INACTIVE_SLOT, 0, 2));
        assert!(outs.iter().all(|o| o.slot == 0xABCD_EF01 && o.raw_fields == [0u8; 56]));

        // out of range & null failed_index allowed
        let r = [ffi_req(5000, [0, 0, 0])];
        assert_eq!(
            ar_light_pool_project_batch(h, r.as_ptr(), 1, outs.as_mut_ptr(), 3, &mut n, std::ptr::null_mut()),
            code::INVALID_SLOT
        );
        ar_light_pool_free(h);
    }
}

#[test]
fn ffi_boundary_errors() {
    unsafe {
        let h = open();
        let reqs = [ffi_req(9, [1, 0, 0]), ffi_req(9, [2, 0, 0])];
        let mut outs = [blank_out(), blank_out()];
        let (mut n, mut fi) = (0usize, 0u32);
        let call = |pool, r: *const ArLightProjectionRequest, c, o: *mut ArProjectedLight, cap, n: *mut usize, f| {
            ar_light_pool_project_batch(pool, r, c, o, cap, n, f)
        };
        assert_eq!(call(h, reqs.as_ptr(), 2, outs.as_mut_ptr(), 1, &mut n, &mut fi), code::BUFFER_TOO_SMALL);
        assert_eq!((n, outs[0].slot), (2, 0xABCD_EF01));
        assert_eq!(call(h, reqs.as_ptr(), 2, outs.as_mut_ptr(), 2, std::ptr::null_mut(), &mut fi), code::NULL_ARGUMENT);
        assert_eq!(call(std::ptr::null(), reqs.as_ptr(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi), code::NULL_ARGUMENT);
        assert_eq!(call(h, std::ptr::null(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi), code::NULL_ARGUMENT);
        assert_eq!(call(h, reqs.as_ptr(), 2, std::ptr::null_mut(), 2, &mut n, &mut fi), code::NULL_ARGUMENT);
        // count 0 with null arrays is fine
        n = 5;
        assert_eq!(call(h, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut n, &mut fi), code::OK);
        assert_eq!(n, 0);
        // bad struct sizes (request, then output) -> nothing written
        let mut badreq = reqs;
        badreq[1].struct_size = 1;
        assert_eq!(call(h, badreq.as_ptr(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi), code::BAD_STRUCT_SIZE);
        assert_eq!(fi, 1);
        outs[1].struct_size = 0;
        assert_eq!(call(h, reqs.as_ptr(), 2, outs.as_mut_ptr(), 2, &mut n, &mut fi), code::BAD_STRUCT_SIZE);
        assert_eq!((fi, outs[0].slot), (1, 0xABCD_EF01));
        ar_light_pool_free(h);
    }
}

#[test]
fn ffi_overflow_codes() {
    unsafe {
        let mut p = empty();
        light(&mut p, 0, [f32::MAX.to_bits(), f32::NAN.to_bits(), 0], 1);
        let mut h = std::ptr::null_mut();
        assert_eq!(ar_light_pool_open(p.as_ptr(), p.len(), &mut h), code::OK);
        let mut outs = [blank_out()];
        let (mut n, mut fi) = (0usize, 0u32);
        // f32::MAX + ~32768 rounds back to f32::MAX -> finite
        let r = [ffi_req(0, [i32::MAX, 0, 0])];
        assert_eq!(ar_light_pool_project_batch(h, r.as_ptr(), 1, outs.as_mut_ptr(), 1, &mut n, &mut fi), code::OK);
        assert_eq!(outs[0].position[0], f32::MAX);
        let r = [ffi_req(0, [0, 1, 0])];
        assert_eq!(
            ar_light_pool_project_batch(h, r.as_ptr(), 1, outs.as_mut_ptr(), 1, &mut n, &mut fi),
            code::NONFINITE_POSITION
        );
        ar_light_pool_free(h);
    }
}
