use against_rome_data::{error::code, particle_definition_ffi::*};
use std::ptr;

fn headers() -> [Vec<u8>; 4] {
    let par=b"[ParticleParameterDefault]\n;idx,activ,name,maxam,timer,reflec,birth,fadet,windf,floorc,swarm,timdie,swarad,swarmtim,heisen,refint\n".to_vec();
    let geo=b"[ParticleGeometrieDefault]\n;idx,activ,name,exsiz,ezsiz,heigh,exadd,eyadd,ezadd,exvar,eyvar,ezvar,ysub,xadd,yadd,zadd\n".to_vec();
    let gfx =
        b"[ParticleGrafikDefault]\n;idx,activ,name,lolig,obdef,expde,objpa,objpv,objpp\n".to_vec();
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
    [
        par,
        geo,
        gfx,
        format!("[ObjectDefaults]\n;{}\n", fields.join(",")).into_bytes(),
    ]
}
fn output() -> ArParticleDefinition {
    ArParticleDefinition {
        struct_size: 44,
        owner_type: 77,
        parameter_id: 77,
        geometry_id: 77,
        grafik_id: 77,
        grafik_object: 77,
        alr_id: 77,
        lifetime: 77,
        type_divisor: 77,
        velocity_branch_raw: 77,
        skip_first_raw: 77,
    }
}
#[test]
fn owned_catalog_and_atomic_query_errors_keep_outputs_unchanged() {
    assert_eq!(ar_particle_definition_abi_version(), 1);
    assert_eq!(ar_particle_definition_struct_size(), 44);
    let mut inputs = headers();
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe {
            ar_particle_definition_open(
                inputs[0].as_ptr(),
                inputs[0].len(),
                inputs[1].as_ptr(),
                inputs[1].len(),
                inputs[2].as_ptr(),
                inputs[2].len(),
                inputs[3].as_ptr(),
                inputs[3].len(),
                &mut handle,
            )
        },
        code::OK
    );
    for input in &mut inputs {
        input.fill(0);
    }
    let mut sources = 99;
    assert_eq!(
        unsafe { ar_particle_definition_source_count(handle, &mut sources) },
        code::OK
    );
    assert_eq!(sources, 0);
    let mut out = output();
    let before = out;
    assert_eq!(
        unsafe { ar_particle_definition_query(handle, 0, &mut out) },
        224
    );
    assert_eq!(out, before);
    out.struct_size = 0;
    let before = out;
    assert_eq!(
        unsafe { ar_particle_definition_query(handle, 0, &mut out) },
        code::BAD_STRUCT_SIZE
    );
    assert_eq!(out, before);
    unsafe {
        ar_particle_definition_free(handle);
        ar_particle_definition_free(ptr::null_mut());
    }
    let inputs = headers();
    for invalid in 0..4 {
        let mut lengths = inputs.each_ref().map(|b| b.len());
        lengths[invalid] = 0;
        let mut failed = 1usize as *mut DefinitionCatalog;
        assert_eq!(
            unsafe {
                ar_particle_definition_open(
                    inputs[0].as_ptr(),
                    lengths[0],
                    inputs[1].as_ptr(),
                    lengths[1],
                    inputs[2].as_ptr(),
                    lengths[2],
                    inputs[3].as_ptr(),
                    lengths[3],
                    &mut failed,
                )
            },
            220 + invalid as i32
        );
        assert!(failed.is_null());
    }
}
#[test]
fn null_and_impossible_input_lengths_clear_handle_without_dereferencing() {
    let mut handle = 1usize as *mut DefinitionCatalog;
    assert_eq!(
        unsafe {
            ar_particle_definition_open(
                ptr::null(),
                1,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut handle,
            )
        },
        code::NULL_ARGUMENT
    );
    assert!(handle.is_null());
    assert_eq!(
        unsafe {
            ar_particle_definition_open(
                ptr::null(),
                usize::MAX,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut handle,
            )
        },
        code::BUFFER_TOO_SMALL
    );
    assert!(handle.is_null());
    assert_eq!(
        unsafe {
            ar_particle_definition_open(
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut handle,
            )
        },
        220
    );
    assert!(handle.is_null());
    let mut out = output();
    let before = out;
    assert_eq!(
        unsafe { ar_particle_definition_query(ptr::null(), 0, &mut out) },
        code::NULL_ARGUMENT
    );
    assert_eq!(out, before);
}

#[test]
fn root_api_rejects_invalid_utf8_or_relative_root_and_count_null_is_atomic() {
    let mut handle = 1usize as *mut DefinitionCatalog;
    assert_eq!(
        unsafe { ar_particle_definition_load_utf8([255].as_ptr(), 1, &mut handle) },
        226
    );
    assert!(handle.is_null());
    assert_eq!(
        unsafe { ar_particle_definition_load_utf8(b"relative".as_ptr(), 8, &mut handle) },
        226
    );
    assert!(handle.is_null());
    assert_eq!(
        unsafe { ar_particle_definition_load_utf8(ptr::null(), 1, &mut handle) },
        code::NULL_ARGUMENT
    );
    let mut count = 99;
    assert_eq!(
        unsafe { ar_particle_definition_source_count(ptr::null(), &mut count) },
        code::NULL_ARGUMENT
    );
    assert_eq!(count, 99);
}
