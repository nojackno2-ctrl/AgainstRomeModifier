use against_rome_data::{error::code, particle_definition_ffi::*};
use std::ptr;

fn catalog(velocity: i32, divisor: i32, alr: i32) -> *mut DefinitionCatalog {
    catalog_with_skip(velocity, divisor, alr, 0)
}
fn catalog_with_skip(velocity: i32, divisor: i32, alr: i32, skip: i32) -> *mut DefinitionCatalog {
    let par=format!("[ParticleParameterDefault]\n;idx,activ,name,maxam,timer,reflec,birth,fadet,windf,floorc,swarm,timdie,swarad,swarmtim,heisen,refint\n22,1,P,64,15000,0,0,0,0,0,{skip},0,0,0,{velocity},0\n");
    let geo="[ParticleGeometrieDefault]\n;idx,activ,name,exsiz,ezsiz,heigh,exadd,eyadd,ezadd,exvar,eyvar,ezvar,ysub,xadd,yadd,zadd\n23,1,G,0,0,0,0,0,0,0,0,0,0,0,0,0\n";
    let gfx="[ParticleGrafikDefault]\n;idx,activ,name,lolig,obdef,expde,objpa,objpv,objpp\n24,1,X,-1,227,-1,-1,0,0\n";
    let mut fields: Vec<_> = (0..209).map(|i| format!("field{i}")).collect();
    for (i, n) in [
        (0, "idx"),
        (1, "activ"),
        (2, "alen"),
        (5, "alrid"),
        (52, "name"),
        (63, "papar"),
        (64, "pageo"),
        (65, "pagfx"),
    ] {
        fields[i] = n.into();
    }
    let mut obj = format!("[ObjectDefaults]\n;{}\n", fields.join(","));
    for (id, d, p, g, x) in [(830, 999, 22, 23, 24), (227, divisor, -1, -1, -1)] {
        let mut row = vec!["0".to_owned(); 209];
        for (i, v) in [(0, id), (1, 1), (2, d), (5, alr), (63, p), (64, g), (65, x)] {
            row[i] = v.to_string();
        }
        row[52] = "Object".into();
        obj.push_str(&row.join(","));
        obj.push('\n');
    }
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe {
            ar_particle_definition_open(
                par.as_ptr(),
                par.len(),
                geo.as_ptr(),
                geo.len(),
                gfx.as_ptr(),
                gfx.len(),
                obj.as_ptr(),
                obj.len(),
                &mut handle,
            )
        },
        code::OK
    );
    handle
}
#[test]
fn native_frame_success_uses_grafik_divisor_and_all_error_outputs_are_atomic() {
    let handle = catalog(0, 1000, 258);
    let mut frame = 77;
    assert_eq!(
        unsafe {
            ar_particle_definition_lifetime_frame(handle, 830, 14001, 258, 8, 1, 8, &mut frame)
        },
        0
    );
    assert_eq!(frame, 7); // owner divisor999 would produce0.
    for (owner, asset, cols, dirs, count, expected) in [
        (830, 257, 8, 1, 8, 248),
        (830, 258, 0, 1, 8, 240),
        (830, 258, 8, 32, 256, 242),
        (830, 258, 8, 1, 1, 246),
        (-1, 258, 8, 1, 8, 224),
    ] {
        frame = 77;
        assert_eq!(
            unsafe {
                ar_particle_definition_lifetime_frame(
                    handle, owner, 14001, asset, cols, dirs, count, &mut frame,
                )
            },
            expected
        );
        assert_eq!(frame, 77);
    }
    unsafe {
        ar_particle_definition_free(handle);
    }
}
#[test]
fn velocity_zero_divisor_and_unsupported_primary_route_do_not_guess_frame_zero() {
    for (velocity, divisor, asset, expected) in
        [(1, 1000, 258, 241), (0, 0, 258, 243), (0, 1000, -1, 247)]
    {
        let handle = catalog(velocity, divisor, asset);
        let mut frame = 77;
        assert_eq!(
            unsafe {
                ar_particle_definition_lifetime_frame(
                    handle, 830, 14001, asset, 8, 1, 8, &mut frame,
                )
            },
            expected
        );
        assert_eq!(frame, 77);
        unsafe {
            ar_particle_definition_free(handle);
        }
    }
    let mut frame = 77;
    assert_eq!(
        unsafe {
            ar_particle_definition_lifetime_frame(ptr::null(), 830, 0, 258, 8, 1, 8, &mut frame)
        },
        code::NULL_ARGUMENT
    );
    assert_eq!(frame, 77);
}

#[test]
fn saved_batch_filters_count_and_active_and_returns_signed_coordinates_and_frames() {
    let handle = catalog(0, 1000, 258);
    let mut record = [0u8; 2252];
    let state = [0xabu8; 16];
    record[0..2].copy_from_slice(&1u16.to_le_bytes());
    record[2..6].copy_from_slice(&3i32.to_le_bytes());
    for ordinal in [0usize, 2, 3] {
        let child = 12 + ordinal * 35;
        record[child] = 1;
        for (offset, value) in [
            (1, -1i32),
            (5, 65537),
            (9, i32::MIN),
            (13, -65537),
            (25, 14001),
        ] {
            record[child + offset..child + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    let before = record;
    let sentinel = ParticleFrameCandidate {
        ordinal: 99,
        ..Default::default()
    };
    let mut output = [sentinel; 64];
    let mut count = 99;
    assert_eq!(ar_particle_frame_candidate_struct_size(), 40);
    let query = |output: &mut [ParticleFrameCandidate], count: &mut u32, dirs| unsafe {
        ar_particle_definition_saved_frames(
            handle,
            830,
            7,
            record.as_ptr(),
            record.len(),
            state.as_ptr(),
            state.len(),
            258,
            8,
            dirs,
            8,
            output.as_mut_ptr(),
            output.len(),
            count,
        )
    };
    assert_eq!(query(&mut output, &mut count, 1), 0);
    assert_eq!(count, 2);
    assert_eq!(output[0].ordinal, 0);
    assert_eq!(output[1].ordinal, 2);
    assert_eq!(output[0].position_native, [-1, 1, -32768]);
    assert_eq!(output[0].velocity_high, [-2, 0, 0]);
    assert_eq!((output[0].frame, output[0].status), (7, 0));
    assert_eq!(output[2], sentinel);
    assert_eq!(query(&mut output, &mut count, 32), 0);
    assert_eq!((output[0].frame, output[0].status), (u32::MAX, 242));
    assert_eq!(record, before);
    assert_eq!(state, [0xab; 16]);
    unsafe {
        ar_particle_definition_free(handle);
    }
}

#[test]
fn saved_batch_whole_query_failures_preserve_all_outputs() {
    let handle = catalog(0, 1000, 258);
    let mut record = [0u8; 2252];
    let state = [0u8; 16];
    record[0] = 1;
    record[2..6].copy_from_slice(&65i32.to_le_bytes());
    let sentinel = ParticleFrameCandidate {
        ordinal: 99,
        ..Default::default()
    };
    let mut output = [sentinel; 64];
    let mut count = 99;
    let query = |record: &[u8], output: &mut [ParticleFrameCandidate], count: &mut u32, alr| unsafe {
        ar_particle_definition_saved_frames(
            handle,
            830,
            0,
            record.as_ptr(),
            record.len(),
            state.as_ptr(),
            state.len(),
            alr,
            8,
            1,
            8,
            output.as_mut_ptr(),
            output.len(),
            count,
        )
    };
    assert_eq!(query(&record, &mut output, &mut count, 258), 114);
    assert_eq!(query(&record[..2251], &mut output, &mut count, 258), 14);
    assert_eq!(query(&record, &mut output, &mut count, 999), 248);
    record[2..6].copy_from_slice(&2i32.to_le_bytes());
    record[12] = 1;
    record[47] = 1;
    assert_eq!(query(&record, &mut output[..1], &mut count, 258), 2);
    assert_eq!(count, 99);
    assert_eq!(output, [sentinel; 64]);
    record[0] = 0;
    assert_eq!(query(&record, &mut output[..0], &mut count, 258), 0);
    assert_eq!(count, 0);
    unsafe {
        ar_particle_definition_free(handle);
    }
}

#[test]
fn saved_batch_applies_positive_skip_first_and_rejects_invalid_snapshot_buffers() {
    let handle = catalog_with_skip(0, 1000, 258, 1);
    let mut record = [0u8; 2252];
    let state = [0u8; 16];
    record[0] = 1;
    record[2..6].copy_from_slice(&3i32.to_le_bytes());
    for ordinal in 0..3 {
        record[12 + ordinal * 35] = 1;
    }
    let mut output = [ParticleFrameCandidate::default(); 64];
    let mut count = 99;
    assert_eq!(
        unsafe {
            ar_particle_definition_saved_frames(
                handle,
                830,
                0,
                record.as_ptr(),
                record.len(),
                state.as_ptr(),
                state.len(),
                258,
                8,
                1,
                8,
                output.as_mut_ptr(),
                output.len(),
                &mut count,
            )
        },
        0
    );
    assert_eq!(count, 2);
    assert_eq!((output[0].ordinal, output[1].ordinal), (1, 2));
    use against_rome_data::particle::ParticleSnapshot;
    assert!(ParticleSnapshot::from_saved_buffers(1024, &record, &state).is_err());
    assert!(ParticleSnapshot::from_saved_buffers(0, &record[..2251], &state).is_err());
    assert!(ParticleSnapshot::from_saved_buffers(0, &record, &state[..15]).is_err());
    unsafe {
        ar_particle_definition_free(handle);
    }
}

#[test]
fn translated_batch_adds_before_sar_and_rejects_out_of_count_active_overflow_atomically() {
    let handle = catalog(0, 1000, 258);
    let mut record = [0u8; 2252];
    let state = [0xabu8; 16];
    record[0] = 1;
    record[2..6].copy_from_slice(&1i32.to_le_bytes());
    record[12] = 1;
    record[13..17].copy_from_slice(&(-65537i32).to_le_bytes());
    record[37..41].copy_from_slice(&14001i32.to_le_bytes());
    let sentinel = ParticleFrameCandidate {
        ordinal: 99,
        ..Default::default()
    };
    let mut output = [sentinel; 64];
    let mut count = 99;
    let query = |record: &[u8], output: &mut [ParticleFrameCandidate], count: &mut u32| unsafe {
        ar_particle_definition_saved_frames_translated(
            handle,
            830,
            0,
            record.as_ptr(),
            record.len(),
            state.as_ptr(),
            state.len(),
            258,
            8,
            1,
            8,
            1,
            0,
            0,
            output.as_mut_ptr(),
            output.len(),
            count,
        )
    };
    let before = record;
    assert_eq!(query(&record, &mut output, &mut count), 0);
    assert_eq!(count, 1);
    assert_eq!(output[0].position_native, [-1, 0, 0]);
    assert_eq!((output[0].frame, output[0].lifetime_raw), (7, 14001));
    assert_eq!(record, before);
    let tail = 12 + 63 * 35;
    record[tail] = 1;
    record[tail + 1..tail + 5].copy_from_slice(&i32::MAX.to_le_bytes());
    output.fill(sentinel);
    count = 99;
    assert_eq!(query(&record, &mut output, &mut count), 13);
    assert_eq!(output, [sentinel; 64]);
    assert_eq!(count, 99);
    record[tail] = 0;
    assert_eq!(query(&record, &mut output, &mut count), 0);
    use against_rome_data::particle::ParticleSnapshot;
    let snapshot = ParticleSnapshot::from_saved_buffers(0, &record, &state).unwrap();
    let translated = snapshot.translated_fixed([1, 0, 0]).unwrap();
    assert_eq!(translated.raw_state(), &state);
    assert_eq!(
        translated.child(63).unwrap().raw_bytes(),
        snapshot.child(63).unwrap().raw_bytes()
    );
    assert_eq!(snapshot.raw_record(), &record);
    unsafe {
        ar_particle_definition_free(handle);
    }
}

#[test]
fn primary_mode_query_unknown_and_bad_owner_preserve_output() {
    let handle = catalog(0, 1000, 258);
    let mut mode = 77;
    for (owner, expected) in [(830, 249), (999, 224)] {
        assert_eq!(
            unsafe { ar_particle_definition_primary_mode(handle, owner, &mut mode) },
            expected
        );
        assert_eq!(mode, 77);
    }
    assert_eq!(
        unsafe { ar_particle_definition_primary_mode(ptr::null(), 830, &mut mode) },
        1
    );
    assert_eq!(mode, 77);
    unsafe {
        ar_particle_definition_free(handle);
    }
}
