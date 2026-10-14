use against_rome_data::{
    particle::{ParticlePool, PAYLOAD_LEN, RECORD_LEN},
    particle_frames::*,
};

fn context() -> FrameContext {
    FrameContext {
        definition_lifetime: 100,
        type_divisor: 100,
        velocity_branch_raw: 0,
        skip_first_raw: 0,
        columns: 8,
        directions: 1,
        frame_count: 8,
    }
}

#[test]
fn lifetime_progress_and_wrap_do_not_use_rounded_or_unbounded_math() {
    let ctx = context();
    for (life, frame) in [
        (100, 0),
        (88, 0),
        (87, 1),
        (50, 4),
        (1, 7),
        (0, 0),
        (-25, 2),
    ] {
        assert_eq!(lifetime_frame(life, &ctx), Ok(frame));
    }
    let ctx = FrameContext {
        definition_lifetime: i32::MAX,
        columns: 4,
        type_divisor: 2,
        frame_count: 4,
        ..context()
    };
    assert_eq!(lifetime_frame(-2, &ctx), Ok(2)); // SUB wraps to MIN+1, IMUL wraps to 4.
}

#[test]
fn unresolved_routes_division_traps_and_invalid_indices_are_distinct() {
    for (ctx, expected) in [
        (
            FrameContext {
                velocity_branch_raw: -1,
                ..context()
            },
            FrameUnavailable::VelocityRoundingUnverified,
        ),
        (
            FrameContext {
                directions: 32,
                ..context()
            },
            FrameUnavailable::DirectionRoundingUnverified,
        ),
        (
            FrameContext {
                columns: 0,
                ..context()
            },
            FrameUnavailable::InvalidLayout,
        ),
        (
            FrameContext {
                type_divisor: 0,
                ..context()
            },
            FrameUnavailable::ZeroDivisor,
        ),
        (
            FrameContext {
                definition_lifetime: i32::MIN,
                type_divisor: -1,
                columns: 1,
                ..context()
            },
            FrameUnavailable::DivisionOverflow,
        ),
        (
            FrameContext {
                definition_lifetime: -100,
                ..context()
            },
            FrameUnavailable::NegativeIndex,
        ),
        (
            FrameContext {
                frame_count: 1,
                ..context()
            },
            FrameUnavailable::FrameOutOfRange,
        ),
    ] {
        let life = if expected == FrameUnavailable::DivisionOverflow {
            0
        } else {
            50
        };
        assert_eq!(lifetime_frame(life, &ctx), Err(expected));
    }
}

fn pool(count: i32, active: u16) -> ParticlePool {
    let mut payload = vec![0; PAYLOAD_LEN];
    for (offset, value) in [(0, 1i32), (4, 1024), (8, 64)] {
        payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    payload[12..14].copy_from_slice(&active.to_le_bytes());
    payload[14..18].copy_from_slice(&count.to_le_bytes());
    for ordinal in [0, 1, 63] {
        let offset = 12 + 12 + ordinal * 35;
        payload[offset] = 7;
        for (relative, value) in [
            (1, -1i32),
            (5, 65535),
            (9, -65537),
            (13, -1),
            (17, 65535),
            (21, i32::MAX),
            (25, 50),
        ] {
            payload[offset + relative..offset + relative + 4].copy_from_slice(&value.to_le_bytes());
        }
        payload[offset + 33..offset + 35].copy_from_slice(&17i16.to_le_bytes());
    }
    assert_eq!(RECORD_LEN, 2252);
    ParticlePool::from_payload(&payload).unwrap()
}

#[test]
fn candidate_ordinals_respect_count_flags_and_sar_without_losing_saved_links() {
    let pool = pool(64, 2);
    let snapshot = pool.capture(0).unwrap();
    let raw = snapshot.clone();
    let mut ctx = context();
    ctx.skip_first_raw = 1;
    let candidates = saved_frame_candidates(&snapshot, Some(&ctx)).unwrap();
    assert_eq!(
        candidates.iter().map(|c| c.ordinal).collect::<Vec<_>>(),
        vec![1, 63]
    );
    assert_eq!(candidates[0].position_native, [-1, 0, -2]);
    assert_eq!(candidates[0].velocity_high, [-1, 0, 32767]);
    assert_eq!(candidates[0].frame, Ok(4));
    assert_eq!(snapshot, raw);
    assert_eq!(snapshot.child(0).unwrap().light_link(), 17);
    let unknown = saved_frame_candidates(&snapshot, None).unwrap();
    assert_eq!(unknown.len(), 3);
    ctx.skip_first_raw = -7;
    assert_eq!(
        saved_frame_candidates(&snapshot, Some(&ctx)).unwrap().len(),
        3
    );
    assert!(unknown
        .iter()
        .all(|c| c.frame == Err(FrameUnavailable::MissingContext)));
}

#[test]
fn malformed_count_rejects_emitter_and_inactive_and_count_zero_do_not_draw() {
    assert!(
        saved_frame_candidates(&pool(65, 2).snapshot_any(0).unwrap(), Some(&context())).is_err()
    );
    assert!(
        saved_frame_candidates(&pool(-1, 2).snapshot_any(0).unwrap(), Some(&context())).is_err()
    );
    assert!(
        saved_frame_candidates(&pool(64, 0).snapshot_any(0).unwrap(), Some(&context()))
            .unwrap()
            .is_empty()
    );
    assert!(
        saved_frame_candidates(&pool(0, 2).snapshot_any(0).unwrap(), Some(&context()))
            .unwrap()
            .is_empty()
    );
}

#[cfg(target_arch = "x86_64")]
#[test]
fn valid_lifetime_indices_match_real_cpu_sub_imul_cdq_idiv_sequences() {
    let mut seed = 0x491b50u32;
    for _ in 0..10000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let lifetime = seed as i32;
        seed = seed.rotate_left(13).wrapping_add(77);
        let definition = seed as i32;
        let columns = (seed % 2047 + 1) as i32;
        let magnitude = (seed.rotate_left(7) % 9999 + 2) as i32;
        let divisor = if seed & 1 == 0 { magnitude } else { -magnitude };
        let ctx = FrameContext {
            definition_lifetime: definition,
            columns,
            type_divisor: divisor,
            frame_count: columns as u32,
            ..context()
        };
        let remainder: i32;
        unsafe {
            std::arch::asm!("sub eax, {life:e}", "imul eax, {columns:e}", "cdq",
            "idiv {divisor:e}", "cdq", "idiv {columns:e}",
            life=in(reg) lifetime, columns=in(reg) columns, divisor=in(reg) divisor,
            inout("eax") definition => _, out("edx") remainder, options(nostack,nomem));
        }
        let expected = if remainder < 0 {
            Err(FrameUnavailable::NegativeIndex)
        } else {
            Ok(remainder as u32)
        };
        assert_eq!(lifetime_frame(lifetime, &ctx), expected);
    }
}

#[test]
fn native_mode_dispatch_is_distinct_from_pixel_abi_modes_and_rejects_unknown_modes() {
    for (mode, leaf) in [
        (0, 0x4ca010),
        (1, 0x4ca340),
        (2, 0x4cab80),
        (3, 0x4caf80),
        (4, 0x4cb380),
        (5, 0x4ca340),
        (6, 0x4ca340),
        (7, 0x4cab80),
        (8, 0x4cab80),
        (9, 0x4cbcc0),
        (49, 0x4ca740),
    ] {
        assert_eq!(native_sprite_leaf_address(mode), Some(leaf));
    }
    for mode in [10, 48, 50, 99, u32::MAX] {
        assert_eq!(native_sprite_leaf_address(mode), None);
    }
}

#[test]
fn caller_mode99_substitution_is_once_signed_and_requires_primary_object_context() {
    assert_eq!(
        resolve_particle_mode(3, None),
        Some(ResolvedParticleMode {
            mode: 3,
            leaf_address: Some(0x4caf80)
        })
    );
    assert_eq!(
        resolve_particle_mode(99, Some(3)),
        resolve_particle_mode(3, None)
    );
    assert_eq!(resolve_particle_mode(99, None), None);
    assert_eq!(
        resolve_particle_mode(99, Some(99)),
        Some(ResolvedParticleMode {
            mode: 99,
            leaf_address: None
        })
    );
    assert_eq!(
        resolve_particle_mode(99, Some(-1)),
        Some(ResolvedParticleMode {
            mode: -1,
            leaf_address: None
        })
    );
    for mode in i16::MIN..=i16::MAX {
        if mode == 99 {
            continue;
        }
        let resolved = resolve_particle_mode(mode, Some(3)).unwrap();
        assert_eq!(resolved.mode, mode as i32);
        assert_eq!(
            resolved.leaf_address,
            native_sprite_leaf_address(mode as i32 as u32)
        );
    }
}

#[test]
fn saved_gfx_mode_uses_separate_parallel_word_not_record_tail() {
    let mut record = [0u8; 15];
    record[0] = 1;
    record[13..15].copy_from_slice(&3i16.to_le_bytes());
    assert_eq!(saved_gfxtype_mode(&record, &99i16.to_le_bytes()), Some(99));
    assert_eq!(
        saved_gfxtype_mode(&record, &(-1i16).to_le_bytes()),
        Some(-1)
    );
    assert_eq!(
        resolve_particle_mode(
            saved_gfxtype_mode(&record, &99i16.to_le_bytes()).unwrap(),
            Some(3)
        ),
        resolve_particle_mode(3, None)
    );
    assert_eq!(saved_gfxtype_mode(&record[..14], &[99, 0]), None);
    assert_eq!(saved_gfxtype_mode(&record, &[99]), None);
    record[0] = 0;
    assert_eq!(saved_gfxtype_mode(&record, &[99, 0]), None);
}

#[test]
fn complete_saved_gfx_payload_indexes_records_and_parallel_modes_independently() {
    let mut payload = vec![0u8; 8 + 2 * 17];
    payload[..4].copy_from_slice(&1u32.to_le_bytes());
    payload[4..8].copy_from_slice(&2u32.to_le_bytes());
    payload[8] = 1;
    payload[23] = 255;
    payload[21..23].copy_from_slice(&3i16.to_le_bytes());
    payload[36..38].copy_from_slice(&4i16.to_le_bytes());
    payload[38..40].copy_from_slice(&99i16.to_le_bytes());
    payload[40..42].copy_from_slice(&(-1i16).to_le_bytes());
    let before = payload.clone();
    assert_eq!(saved_gfxtype_mode_at(&payload, 0), Some(99));
    assert_eq!(saved_gfxtype_mode_at(&payload, 1), Some(-1));
    assert_eq!(saved_gfxtype_mode_at(&payload, 2), None);
    assert_eq!(saved_gfxtype_mode_at(&payload, u32::MAX), None);
    assert_eq!(payload, before);
    payload[23] = 0;
    assert_eq!(saved_gfxtype_mode_at(&payload, 1), None);
    assert_eq!(saved_gfxtype_mode_at(&payload[..41], 0), None);
    payload.push(0);
    assert_eq!(saved_gfxtype_mode_at(&payload, 0), None);
    payload.pop();
    payload[..4].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(saved_gfxtype_mode_at(&payload, 0), None);
    payload[..4].copy_from_slice(&1u32.to_le_bytes());
    payload[4..8].copy_from_slice(&14001u32.to_le_bytes());
    assert_eq!(saved_gfxtype_mode_at(&payload, 0), None);
}

#[test]
fn installed_gfx_mode_payload_smoke_preserves_input_and_resolves_active_slots() {
    let Ok(root) = std::env::var("ARM_GAME_PATH") else {
        return;
    };
    let path = std::path::Path::new(&root).join("MAPS/ENDL_005/DATA/gfxtype.dat");
    let source = std::fs::read(&path).unwrap();
    let payload = if source.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(&source, Some(238008)).unwrap()
    } else {
        source.clone()
    };
    assert_eq!(payload.len(), 238008);
    let mut active = 0;
    for slot in 0..14000u32 {
        let mode = saved_gfxtype_mode_at(&payload, slot);
        assert_eq!(mode.is_some(), payload[8 + slot as usize * 15] != 0);
        if mode.is_some() {
            active += 1;
        }
    }
    assert!(active > 0);
    assert_eq!(source, std::fs::read(&path).unwrap());
    println!("installed ENDL_005 gfx slots14000, active{active}, source unchanged");
}

#[test]
fn saved_object_mode_follows_link_not_object_slot_and_preserves_uid() {
    let mut gfx = vec![0u8; 42];
    gfx[..4].copy_from_slice(&1u32.to_le_bytes());
    gfx[4..8].copy_from_slice(&2u32.to_le_bytes());
    gfx[8] = 1;
    gfx[23] = 1;
    gfx[38..40].copy_from_slice(&3i16.to_le_bytes());
    gfx[40..42].copy_from_slice(&99i16.to_le_bytes());
    let mut object = [0u8; 79];
    object[0] = 1;
    object[3..7].copy_from_slice(&42u32.to_le_bytes());
    object[73..75].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        saved_object_gfxtype_mode(&object, &gfx),
        Some(SavedObjectGfxMode {
            object_uid: 42,
            gfx_slot: 1,
            mode: 99
        })
    );
    gfx[23] = 0;
    assert_eq!(saved_object_gfxtype_mode(&object, &gfx), None);
    gfx[23] = 1;
    object[73..75].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(saved_object_gfxtype_mode(&object, &gfx), None);
    object[73..75].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(saved_object_gfxtype_mode(&object[..78], &gfx), None);
    object[0] = 0;
    assert_eq!(saved_object_gfxtype_mode(&object, &gfx), None);
}

#[test]
fn object_mode_ffi_uid_context_and_errors_are_atomic() {
    use against_rome_data::particle_ffi::{
        ar_particle_saved_object_mode, ar_particle_saved_object_mode_struct_size, SavedObjectMode,
    };
    let mut object = [0u8; 79];
    object[0] = 1;
    object[3..7].copy_from_slice(&42u32.to_le_bytes());
    let mut gfx = [0u8; 25];
    gfx[..4].copy_from_slice(&1u32.to_le_bytes());
    gfx[4..8].copy_from_slice(&1u32.to_le_bytes());
    gfx[8] = 1;
    gfx[23..25].copy_from_slice(&99i16.to_le_bytes());
    let sentinel = SavedObjectMode {
        struct_size: 24,
        object_uid: 77,
        gfx_slot: 77,
        gfx_mode: 77,
        resolved_mode: 77,
        leaf_address: 77,
    };
    let mut out = sentinel;
    assert_eq!(std::mem::size_of::<SavedObjectMode>(), 24);
    assert_eq!(ar_particle_saved_object_mode_struct_size(), 24);
    for (uid, primary, has, expected) in [
        (42, 0, 0, 249),
        (43, 3, 1, 251),
        (42, 32768, 1, 250),
        (42, 3, 2, 250),
    ] {
        assert_eq!(
            unsafe {
                ar_particle_saved_object_mode(
                    object.as_ptr(),
                    79,
                    gfx.as_ptr(),
                    25,
                    uid,
                    primary,
                    has,
                    &mut out,
                )
            },
            expected
        );
        assert_eq!(out, sentinel);
    }
    assert_eq!(
        unsafe {
            ar_particle_saved_object_mode(object.as_ptr(), 79, gfx.as_ptr(), 25, 42, 3, 1, &mut out)
        },
        0
    );
    assert_eq!(
        (
            out.object_uid,
            out.gfx_slot,
            out.gfx_mode,
            out.resolved_mode,
            out.leaf_address
        ),
        (42, 0, 99, 3, 0x4caf80)
    );
    out = sentinel;
    object[0] = 0;
    assert_eq!(
        unsafe {
            ar_particle_saved_object_mode(object.as_ptr(), 79, gfx.as_ptr(), 25, 42, 3, 1, &mut out)
        },
        252
    );
    assert_eq!(out, sentinel);
}
