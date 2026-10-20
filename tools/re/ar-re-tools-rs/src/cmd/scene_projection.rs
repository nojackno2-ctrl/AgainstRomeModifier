//! `probe_native_scene_projection.py`: read-only, fingerprinted native XYZ projection and
//! optional 74-map audit.
//!
//! Usage: `ar-re probe-native-scene-projection EXE OUTPUT [--game-root ROOT] [--evidence FILE]`.
//! Only OUTPUT is created (exclusive creation). No game execution, REA invocation, extraction,
//! installation or runtime FPU claim. The retained ten-record Ghidra bundle
//! (`re_workspace/native-family-height-20261011-evidence.json`) is hash-gated.

use super::evidence::*;
use super::Parsed;
use crate::dis::Ins;
use crate::floatconv::conditional_conversion;
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use iced_x86::{Mnemonic, OpKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const EVIDENCE_SHA256: &str = "3caf2767783718d16f8a19e4c93119a0e9034e81a7df4fe5265d056557b62e5e";
pub const EVIDENCE_NAME: &str = "native-family-height-20261011-evidence.json";
pub const CONTROLS: [u16; 6] = [0x037f, 0x007f, 0x027f, 0x077f, 0x0b7f, 0x0f7f];

// This whole contiguous caller slice proves the packed index, stride, stack accounting,
// three separate conversions, and five projection arguments.
pub const CALLER: [&str; 29] = [
    "0x4b48ad: IMUL EDI, EBX, 0x4c",
    "0x4b48b0: MOV EDI, dword ptr [EDI + 0xa14c06]",
    "0x4b48b6: SAR EDI, 0x10",
    "0x4b48b9: CMP EDI, -0x1",
    "0x4b48bc: JLE 0x004b47d6",
    "0x4b48c2: LEA EAX, [ESP + 0x8]",
    "0x4b48c6: PUSH EAX",
    "0x4b48c7: LEA EAX, [ESP + 0x8]",
    "0x4b48cb: PUSH EAX",
    "0x4b48cc: IMUL EAX, EDI, 0x14",
    "0x4b48cf: PUSH dword ptr [EAX + 0xb18848]",
    "0x4b48d5: MOV dword ptr [ESP + 0x1c], EAX",
    "0x4b48d9: CALL 0x00419c80",
    "0x4b48de: ADD ESP, 0x4",
    "0x4b48e1: PUSH EAX",
    "0x4b48e2: MOV EAX, dword ptr [ESP + 0x1c]",
    "0x4b48e6: PUSH dword ptr [EAX + 0xb18844]",
    "0x4b48ec: CALL 0x00419c80",
    "0x4b48f1: ADD ESP, 0x4",
    "0x4b48f4: PUSH EAX",
    "0x4b48f5: MOV EAX, dword ptr [ESP + 0x20]",
    "0x4b48f9: PUSH dword ptr [EAX + 0xb18840]",
    "0x4b48ff: CALL 0x00419c80",
    "0x4b4904: ADD ESP, 0x4",
    "0x4b4907: PUSH EAX",
    "0x4b4908: CALL 0x00491b50",
    "0x4b490d: IMUL EAX, EBP, 0x12",
    "0x4b4910: MOV EAX, dword ptr [EAX + 0xc2706a]",
    "0x4b4916: ADD ESP, 0x14",
];

fn caller_lines() -> Vec<String> {
    CALLER.iter().map(|s| s.to_string()).collect()
}

#[derive(Clone, Debug, PartialEq)]
enum V {
    Pointer(i64),
    Position(i64),
    Float(char),
    Converted(char),
}

impl V {
    fn to_json(&self) -> J {
        match self {
            V::Pointer(n) => J::Arr(vec!["pointer_to_original_local".into(), (*n).into()]),
            V::Position(n) => J::Arr(vec!["primary_position_byte_offset".into(), (*n).into()]),
            V::Float(a) => J::Arr(vec!["float32".into(), a.to_string().into()]),
            V::Converted(a) => J::Arr(vec!["converted_signed32".into(), a.to_string().into()]),
        }
    }
}

fn contiguous(index: &BTreeMap<u64, Ins>, start: u64, end: u64) -> R<()> {
    let mut cursor = start;
    for (address, ins) in index {
        req(*address == cursor, &format!("missing/overlapping instruction at {cursor:#x}"))?;
        cursor += ins.bytes.len() as u64;
    }
    req(cursor == end + 1, "instruction coverage does not match inclusive body")
}

fn displacement(ins: &Ins) -> i64 {
    ins.ins.memory_displacement32() as i32 as i64
}

/// Replays only the verified straight-line argument preparation, not code execution.
fn caller_stack(index: &BTreeMap<u64, Ins>) -> R<J> {
    let mut stack: BTreeMap<i64, V> = BTreeMap::new();
    let (mut esp, mut eax) = (0i64, None::<V>);
    let mut calls = J::arr();
    let mut conversion_axes = Vec::new();
    for (address, ins) in index.range(0x4b48c2..=0x4b4908) {
        match ins.ins.mnemonic() {
            Mnemonic::Lea => eax = Some(V::Pointer(esp + displacement(ins))),
            Mnemonic::Imul => eax = Some(V::Position(ins.ins.immediate(2) as i64)),
            Mnemonic::Push => {
                let value = if ins.ins.op0_kind() == OpKind::Memory {
                    req(eax == Some(V::Position(20)), "float source lost position offset")?;
                    let axis = match ins.ins.memory_displacement32() {
                        0xb18840 => 'X',
                        0xb18844 => 'Y',
                        0xb18848 => 'Z',
                        _ => return Err("unexpected position axis address".into()),
                    };
                    V::Float(axis)
                } else {
                    eax.clone().ok_or("push of undefined register")?
                };
                esp -= 4;
                stack.insert(esp, value);
            }
            Mnemonic::Mov => {
                if ins.ins.op0_kind() == OpKind::Memory {
                    stack.insert(esp + displacement(ins), eax.clone().ok_or("store of undefined register")?);
                } else {
                    eax = Some(stack.get(&(esp + displacement(ins))).cloned().ok_or("load of unwritten stack slot")?);
                }
            }
            Mnemonic::Add => esp += ins.ins.immediate(1) as i64,
            Mnemonic::Call => {
                let target = ins.ins.near_branch_target();
                if target == 0x419c80 {
                    let Some(V::Float(axis)) = stack.get(&esp).cloned() else {
                        return Err("conversion input is not an individual float".into());
                    };
                    calls.push(jobj! { "site" => format!("{address:#x}"), "float_axis" => axis.to_string() });
                    conversion_axes.push(axis);
                    eax = Some(V::Converted(axis));
                } else {
                    req(target == 0x491b50, "unexpected argument-preparation call")?;
                    let arguments: Vec<V> = (0..5).map(|i| stack.get(&(esp + i * 4)).cloned().ok_or("missing call argument")).collect::<Result<_, _>>()?;
                    req(
                        arguments == [V::Converted('X'), V::Converted('Y'), V::Converted('Z'), V::Pointer(4), V::Pointer(8)],
                        "actual caller stack order mismatch",
                    )?;
                    req(conversion_axes == ['Z', 'Y', 'X'] && esp == -20, "conversion order or stack balance mismatch")?;
                    let listed: Vec<J> = arguments.iter().map(V::to_json).collect();
                    let mut pushed = listed.clone();
                    pushed.reverse();
                    return Ok(jobj! {
                        "provenance" => "symbolic replay of exact Capstone-verified caller slice",
                        "conversion_calls" => calls,
                        "cdecl_arguments_at_call" => J::Arr(listed),
                        "push_order" => J::Arr(pushed),
                        "ESP_delta_before_CALL" => esp,
                    });
                }
            }
            other => return Err(format!("unhandled caller preparation instruction {address:#x} ({other:?})")),
        }
    }
    Err("missing projection call in caller stack replay".into())
}

fn verified_json(ins: &Ins, retained: &str) -> J {
    jobj! {
        "address" => format!("{:#x}", ins.address()),
        "bytes" => hex(&ins.bytes),
        "retained" => retained,
        "capstone" => format!("{} {}", ins.mnemonic, ins.operands),
    }
}

pub fn inspect_code(exe: &[u8], retained: &[u8]) -> R<J> {
    req(sha256_hex(exe) == EXE_SHA256, "unsupported EXE fingerprint")?;
    req(sha256_hex(retained) == EVIDENCE_SHA256, "retained ten-record bundle fingerprint changed")?;
    let records = json_records(retained)?;
    req(records.len() == 10, "expected latest ten-record bundle")?;
    let caller_all = caller_lines();
    let mut functions = J::arr();
    let mut caller_index: BTreeMap<u64, Ins> = BTreeMap::new();
    let mut instruction_total = 0usize;
    for (entry, end) in [(0x491b50u64, 0x491bc1u64), (0x419c80, 0x419cb6), (0x4b4790, 0x4b50b4)] {
        let record = procedure_record(&records, entry, Some("analyze_function"), false)?;
        req(subject_sha(&record) == Some(EXE_SHA256), "retained subject EXE mismatch")?;
        let body = body_of(&record)?;
        req(body.contiguous && body.ranges == [(entry, end)] && body.total_bytes == end - entry + 1, "retained inclusive body/provenance mismatch")?;
        let mut lines = assembly_lines(&record);
        let is_caller = entry == 0x4b4790;
        if is_caller {
            req(caller_all.iter().all(|l| lines.contains(l)), "caller stack/index contract changed")?;
            lines = caller_all.clone();
        }
        let mut index = BTreeMap::new();
        let mut verified = J::arr();
        for line in &lines {
            let ins = verify_line(exe, &body.ranges, line, true, "original EXE/retained assembly mismatch")?;
            req(!index.contains_key(&ins.address()), "duplicate retained instruction address")?;
            verified.push(verified_json(&ins, split_line(line)?.1));
            index.insert(ins.address(), ins);
        }
        contiguous(&index, if is_caller { 0x4b48ad } else { entry }, if is_caller { 0x4b4918 } else { end })?;
        instruction_total += verified.len();
        functions.push(jobj! {
            "entry" => format!("{entry:#x}"),
            "evidence_id" => evidence_id(&record),
            "body_ranges_inclusive" => body.raw_ranges.clone(),
            "body_bytes" => body.total_bytes,
            "coverage" => if is_caller { "caller slice only" } else { "complete body" },
            "instructions" => verified,
        });
        if is_caller {
            caller_index = index;
        }
    }
    req(read_va(exe, 0x612c70, 4)? == [0xfd, 0xff, 0xff, 0x3e], "signed near-half conversion bias changed")?;

    // Independently verify the original object validator. Its active-byte base avoids inventing a
    // relative offset from the caller's unaligned load.
    let validator = crate::cmd::evidence::decode_one(exe, 0x4ab7ce, 15)?;
    req(
        crate::dis::same(&validator.mnemonic, &validator.operands, "cmp", "byte ptr [eax*4 + 0xa14bfc], 0"),
        "original object validator base changed",
    )?;
    let object_base = validator.ins.memory_displacement32() as u64;
    let packed_load = caller_index.get(&0x4b48b0).ok_or("caller load missing")?;
    req(
        packed_load.ins.op1_kind() == OpKind::Memory
            && packed_load.ins.memory_size().size() == 4
            && caller_index.get(&0x4b48b6).is_some_and(|i| i.ins.immediate(1) == 16),
        "not a signed packed high-half index",
    )?;
    let packed_address = packed_load.ins.memory_displacement32() as u64;
    let position_offset = packed_address + 2 - object_base;
    req(position_offset == 0x0c, "unexpected derived primary-position offset")?;
    let xyz: Vec<u64> = [0x4b48f9u64, 0x4b48e6, 0x4b48cf]
        .iter()
        .map(|a| caller_index.get(a).map(|i| i.ins.memory_displacement32() as u64).ok_or("caller XYZ instruction missing"))
        .collect::<Result<_, _>>()?;
    req(xyz == [0xb18840, 0xb18844, 0xb18848], "caller XYZ source changed")?;
    let position_base = xyz[0] - 4;
    req(position_base == 0xb1883c && caller_index.get(&0x4b48cc).is_some_and(|i| i.ins.immediate(2) == 20), "position runtime base/stride changed")?;
    let hex_list = |values: &[u64]| -> Vec<String> { values.iter().map(|v| format!("{v:#x}")).collect() };
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "evidence_sha256" => EVIDENCE_SHA256,
        "instruction_assertions" => instruction_total + 1,
        "functions" => functions,
        "caller_stack_validation" => caller_stack(&caller_index)?,
        "object_base_validator" => jobj! {
            "address" => format!("{:#x}", validator.address()),
            "bytes" => hex(&validator.bytes),
            "assembly" => format!("{} {}", validator.mnemonic, validator.operands),
            "provenance" => "original EXE Capstone; separate from retained bodies",
        },
        "mapping" => jobj! {
            "object_runtime_base" => format!("{object_base:#x}"),
            "object_runtime_stride" => 76,
            "packed_primary_load" => format!("{packed_address:#x}"),
            "packed_high_half_byte_delta" => 2,
            "derived_primary_position_runtime_offset" => format!("{position_offset:#x}"),
            "derivation" => format!("{packed_address:#x} + 2 - {object_base:#x} = {position_offset:#x}"),
            "object_primary_position_disk_offset" => 67,
            "object_disk_stride" => 79,
            "parent_link_disk_column_zero_based" => 13,
            "position_runtime_base" => format!("{position_base:#x}"),
            "position_runtime_stride" => 20,
            "XYZ_runtime_addresses" => hex_list(&xyz),
            "XYZ_runtime_offsets" => vec![4, 8, 12],
            "position_disk_stride" => 17,
            "XYZ_disk_offsets" => vec![1, 5, 9],
        },
        "call_chain" => J::Arr(vec![
            jobj! { "caller" => "0x4b4790", "site" => "0x4b48d9", "callee" => "0x419c80", "input" => "primary Z float32" },
            jobj! { "caller" => "0x4b4790", "site" => "0x4b48ec", "callee" => "0x419c80", "input" => "primary Y float32" },
            jobj! { "caller" => "0x4b4790", "site" => "0x4b48ff", "callee" => "0x419c80", "input" => "primary X float32" },
            jobj! {
                "caller" => "0x4b4790", "site" => "0x4b4908", "callee" => "0x491b50",
                "arguments_cdecl" => vec!["converted X", "converted Y", "converted Z", "screenX pointer", "screenY pointer"],
                "push_order" => vec!["screenY pointer (original ESP+8)", "screenX pointer (original ESP+4)", "converted Z", "converted Y", "converted X"],
                "cleanup" => "0x4b4916 ADD ESP,0x14; each conversion separately ADD ESP,4",
            },
        ]),
        "projection" => jobj! {
            "globals" => jobj! { "zoom" => "0x771854", "cameraX" => "0x771858", "cameraY" => "0x77185c" },
            "screenX" => "SAR32(wrap32(X-Z), zoom) - cameraX, wrapped signed32",
            "screenY" => "SAR32(wrap32(X+Z), wrap32(zoom+1)) - cameraY - heightTerm, wrapped signed32",
            "heightTerm" => "d=wrap32(zoom-1); d>=0 ? SAR32(Y,d) : SHL32(Y,wrap32(-d))",
            "normal_zoom_height" => "zoom>=1: Y SAR (zoom-1); zoom=0: Y SHL 1",
            "worldY_used_static" => true,
            "Y_reads" => vec!["0x491b8e", "0x491baa"],
            "Y_subtractions" => vec!["0x491b94", "0x491bb0"],
            "shift_count" => "x86 CL masked with 31; arithmetic inputs/results signed32 wrap",
        },
        "float_conversion" => jobj! {
            "function" => "0x419c80",
            "bias_bits" => "3efffffd",
            "operation" => "FSUB signed near-half bias then FISTP",
            "model" => "sibling probe_native_float_conversion.conditional_conversion",
            "gameplay_fpu_observed" => false,
        },
    })
}

// ------------------------------------------------------------------ integer projection model

fn wrap32(value: i64) -> i32 {
    value as i32
}
fn sar32(value: i32, count: i32) -> i32 {
    value >> (count & 31)
}
fn shl32(value: i32, count: i32) -> i32 {
    wrap32((value as i64) << (count & 31))
}

pub struct Projection {
    pub screen_x: i32,
    pub screen_y: i32,
    pub height_term: i32,
    pub height_branch: &'static str,
    pub masked_counts: [i32; 3],
}

pub fn project(x: i32, y: i32, z: i32, zoom: i32, camera_x: i32, camera_y: i32) -> Projection {
    let sx = wrap32(sar32(wrap32(x as i64 - z as i64), zoom) as i64 - camera_x as i64);
    let sy = wrap32(sar32(wrap32(x as i64 + z as i64), wrap32(zoom as i64 + 1)) as i64 - camera_y as i64);
    let decrement = wrap32(zoom as i64 - 1);
    let height = if decrement >= 0 { sar32(y, decrement) } else { shl32(y, wrap32(-(decrement as i64))) };
    let y_count = if decrement >= 0 { decrement as i64 } else { -(decrement as i64) };
    Projection {
        screen_x: sx,
        screen_y: wrap32(sy as i64 - height as i64),
        height_term: height,
        height_branch: if decrement >= 0 { "SAR" } else { "SHL" },
        masked_counts: [zoom & 31, wrap32(zoom as i64 + 1) & 31, (y_count & 31) as i32],
    }
}

impl Projection {
    fn to_json(&self) -> J {
        jobj! {
            "screenX" => self.screen_x,
            "screenY" => self.screen_y,
            "height_term" => self.height_term,
            "height_branch" => self.height_branch,
            "masked_counts" => jobj! { "X" => self.masked_counts[0], "XZ_sum" => self.masked_counts[1], "Y" => self.masked_counts[2] },
        }
    }
}

pub fn model_cases() -> R<J> {
    type Case = (&'static str, [i32; 6], (i32, i32));
    let fixed: [Case; 14] = [
        ("negative-Y zoom0", [10, -3, 2, 0, 0, 0], (8, 12)),
        ("negative-Y zoom1", [10, -3, 2, 1, 0, 0], (4, 6)),
        ("negative-Y zoom2", [10, -3, 2, 2, 0, 0], (2, 3)),
        ("negative XYZ zoom0", [-10, -3, -2, 0, 0, 0], (-8, 0)),
        ("negative XYZ zoom1", [-10, -3, -2, 1, 0, 0], (-4, 0)),
        ("negative XYZ zoom2", [-10, -3, -2, 2, 0, 0], (-2, 0)),
        ("sum overflow", [2147483647, 0, 1, 0, 0, 0], (2147483646, -1073741824)),
        ("difference overflow", [-2147483648, 0, 1, 0, 0, 0], (2147483647, -1073741824)),
        ("camera subtraction wrap", [0, 0, 0, 0, -2147483648, 2147483647], (-2147483648, -2147483647)),
        ("height SHL overflow", [0, 1073741824, 0, 0, 0, 0], (0, -2147483648)),
        ("shift mask zoom32", [10, -3, 2, 32, 0, 0], (8, 7)),
        ("shift mask zoom33", [10, -3, 2, 33, 0, 0], (4, 6)),
        ("negative zoom SHL branch", [10, -3, 2, -1, 0, 0], (0, 24)),
        ("DEC INT_MIN wraps to positive", [10, -3, 2, -2147483648, 0, 0], (8, 7)),
    ];
    let mut cases = J::arr();
    let ints = |a: &[i32; 6]| J::Arr(a.iter().map(|v| J::from(*v)).collect());
    for (label, a, expected) in fixed {
        let result = project(a[0], a[1], a[2], a[3], a[4], a[5]);
        req((result.screen_x, result.screen_y) == expected, &format!("functional boundary mismatch: {label}"))?;
        cases.push(jobj! {
            "case" => label,
            "input_X_Y_Z_zoom_cameraX_cameraY" => ints(&a),
            "expected_screenX_screenY" => vec![expected.0, expected.1],
            "result" => result.to_json(),
        });
    }
    for zoom in [-2147483648i32, -33, -32, -1, 0, 1, 2, 31, 32, 33, 63, 2147483647] {
        for xyz in [[-2147483648i32, -2147483648, 2147483647], [2147483647, 2147483647, -2147483648]] {
            let a = [xyz[0], xyz[1], xyz[2], zoom, 2147483647, -2147483648];
            cases.push(jobj! {
                "case" => "signed32 extrema and masked shifts",
                "input_X_Y_Z_zoom_cameraX_cameraY" => ints(&a),
                "result" => project(a[0], a[1], a[2], a[3], a[4], a[5]).to_json(),
            });
        }
    }
    let bits = [0u32, 0x8000_0000, 0x3f00_0000, 0xbf00_0000, 0x3f7f_ffff, 0xbf7f_ffff, 0x3f80_0000, 0xbf80_0000, 0x4eff_ffff, 0x4f00_0000, 0xcf00_0000];
    let mut conversions = J::arr();
    for b in bits {
        let mut controls = J::obj();
        for cw in CONTROLS {
            controls.set(&format!("{cw:04x}"), conversion_json(b, cw)?);
        }
        conversions.push(jobj! { "float32_bits" => format!("{b:08x}"), "conditional_controls" => controls });
    }
    Ok(jobj! {
        "integer_projection_cases" => cases,
        "conditional_float_cases" => conversions,
        "controls_are_hypothetical" => true,
        "runtime_rounding_mode_observed" => false,
        "overflow_control_note" => "DEC zoom wraps before signed branch, including INT_MIN; not a zoom-range claim",
    })
}

fn conversion_json(bits: u32, control: u16) -> R<J> {
    let c = conditional_conversion(bits, control)?;
    Ok(jobj! {
        "integer" => c.integer,
        "out_of_signed32_range" => c.out_of_signed32_range,
        "precision_bits" => c.precision_bits,
        "rounding_bits" => c.rounding_bits,
    })
}

// ------------------------------------------------------------------ saved-map audit

struct State {
    json: J,
    active: bool,
    position_status: &'static str,
    finite: bool,
    y_bits: u32,
    y: f64,
}

fn state(objs: &Objects, positions: &[u8], pcount: usize, slot: usize) -> R<State> {
    let record = 16 + slot * 79;
    let position = i16_at(&objs.data, record + 67)? as i64;
    let active = objs.data[record] != 0;
    let mut json = jobj! {
        "slot" => slot,
        "active" => active,
        "uid" => u32_at(&objs.data, record + 3)?,
        "type" => u16_at(&objs.data, record + 75)?,
        "primary_position_index_signed16" => position,
    };
    if !(0..pcount as i64).contains(&position) {
        let status = if position == -1 { "missing" } else { "out_of_range" };
        json.set("position_status", status);
        return Ok(State { json, active, position_status: status, finite: false, y_bits: 0, y: 0.0 });
    }
    let start = 8 + position as usize * 17;
    let bits = [u32_at(positions, start + 1)?, u32_at(positions, start + 5)?, u32_at(positions, start + 9)?];
    let xyz: Vec<f32> = bits.iter().map(|b| f32::from_bits(*b)).collect();
    let status = if positions[start] != 0 { "active" } else { "inactive" };
    let finite = xyz.iter().all(|v| v.is_finite());
    json.set("position_status", status);
    json.set("XYZ_float32_bits", bits.iter().map(|b| format!("{b:08x}")).collect::<Vec<_>>());
    json.set("XYZ", J::Arr(xyz.iter().map(|v| if v.is_finite() { J::Float(*v as f64) } else { J::Null }).collect()));
    json.set("finite_XYZ", finite);
    let mut conditional = J::obj();
    for cw in CONTROLS {
        let mut values = J::arr();
        for b in bits {
            values.push(if b & 0x7f80_0000 != 0x7f80_0000 {
                conversion_json(b, cw)?
            } else {
                jobj! { "integer" => J::Null, "nonfinite_input" => true }
            });
        }
        conditional.set(&format!("{cw:04x}"), values);
    }
    json.set("conditional_XYZ_integer", conditional);
    Ok(State { json, active, position_status: status, finite, y_bits: bits[1], y: xyz[1] as f64 })
}

pub fn inspect_pair(objects_source: &[u8], positions_source: &[u8]) -> R<J> {
    let objs = Objects::from_decoded(decode(objects_source)?, false, "unsupported objects/position layout")?;
    let positions = decode(positions_source)?;
    req(positions.len() >= 8, "short pool header")?;
    let (pversion, pcount) = (u32_at(&positions, 0)?, u32_at(&positions, 4)? as usize);
    req(pversion == 1 && pcount <= 33000, "unsupported objects/position layout")?;
    req(positions.len() == 8 + pcount * 17, "position length mismatch")?;
    let parent_offset = objs.column(13);
    let mut counts = Counter::new();
    let mut children = J::arr();
    let mut first_valid = J::Null;
    let mut first_unequal = J::Null;
    for slot in 0..objs.count {
        let active = objs.active(slot);
        counts.inc(if active { "active_objects" } else { "inactive_objects" });
        let parent_slot = u16_at(&objs.data, parent_offset + slot * 2)? as usize;
        if parent_slot == 0xffff {
            continue;
        }
        counts.inc("declared_children");
        let child = state(&objs, &positions, pcount, slot)?;
        let parent = if parent_slot < objs.count { Some(state(&objs, &positions, pcount, parent_slot)?) } else { None };
        let status = match &parent {
            None => "out_of_range",
            Some(_) if parent_slot == slot => "self_link",
            Some(p) if !p.active => "inactive_parent",
            _ => "active_parent",
        };
        counts.inc(format!("parent_{status}"));
        // "valid" is an audit predicate, not proof of native render eligibility.
        let valid = active
            && status == "active_parent"
            && child.position_status == "active"
            && parent.as_ref().is_some_and(|p| p.position_status == "active")
            && child.finite
            && parent.as_ref().is_some_and(|p| p.finite);
        let mut edge = jobj! {
            "child" => child.json.clone(),
            "parent_slot" => parent_slot,
            "parent_status" => status,
            "parent" => parent.as_ref().map_or(J::Null, |p| p.json.clone()),
            "valid_for_height_comparison" => valid,
        };
        if valid {
            let p = parent.as_ref().unwrap();
            counts.inc("valid_children");
            let dy = child.y - p.y;
            edge.set("child_minus_parent_Y", dy);
            edge.set("unequal_Y_numeric", dy != 0.0);
            edge.set("unequal_Y_bits", child.y_bits != p.y_bits);
            counts.inc(if dy != 0.0 { "valid_children_unequal_Y" } else { "valid_children_equal_Y" });
            counts.add("valid_children_negative_Y", child.y < 0.0);
            if first_valid == J::Null {
                first_valid = edge.clone();
            }
            if dy != 0.0 && first_unequal == J::Null {
                first_unequal = edge.clone();
            }
        } else {
            counts.inc("invalid_for_height_comparison");
        }
        children.push(edge);
    }
    let first_child = match &children {
        J::Arr(a) => a.first().cloned().unwrap_or(J::Null),
        _ => J::Null,
    };
    Ok(jobj! {
        "object_slots" => objs.count,
        "position_slots" => pcount,
        "counts" => counts.to_json(),
        "decoded_sha256" => jobj! { "objects" => sha256_hex(&objs.data), "position" => sha256_hex(&positions) },
        "PFIL_wrapped" => jobj! {
            "objects" => objects_source.starts_with(b"PFIL@"),
            "position" => positions_source.starts_with(b"PFIL@"),
        },
        "all_child_records" => children,
        "representatives" => jobj! {
            "first_child" => first_child,
            "first_valid_child" => first_valid,
            "first_unequal_Y_child" => first_unequal,
        },
        "valid_definition" => "distinct in-range parent, both objects active, both primary positions active/in-range, finite XYZ",
    })
}

/// Tracks every source file read so unchanged-ness can be proven at the end.
#[derive(Default)]
pub struct Sources {
    pub list: Vec<(PathBuf, String)>,
}

impl Sources {
    pub fn read(&mut self, path: &Path) -> R<Vec<u8>> {
        let path = std::fs::canonicalize(path).map(|p| resolve(&p)).map_err(|e| io(e, path))?;
        let data = read(&path)?;
        let digest = sha256_hex(&data);
        match self.list.iter_mut().find(|(p, _)| *p == path) {
            Some((_, before)) => req(*before == digest, &format!("source changed between reads: {}", display(&path)))?,
            None => self.list.push((path, digest)),
        }
        Ok(data)
    }
    pub fn finish(&self) -> R<J> {
        let mut out = J::arr();
        for (path, before) in &self.list {
            let after = sha256_hex(&read(path)?);
            req(*before == after, &format!("source changed during probe: {}", display(path)))?;
            out.push(jobj! { "path" => display(path), "before" => before.clone(), "after" => after, "unchanged" => true });
        }
        Ok(out)
    }
}

pub fn scan(root: &Path, sources: &mut Sources) -> R<J> {
    let directories = map_dirs(root)?;
    let mut reports = J::arr();
    let (mut missing, mut errors) = (J::arr(), J::arr());
    let mut totals = Counter::new();
    for directory in &directories {
        req(is_inside(&resolve(directory), root), "map directory escapes source root")?;
        let mut packed = Vec::new();
        let mut absent = Vec::new();
        for name in ["objects.dat", "position.dat"] {
            let path = directory.join("DATA").join(name);
            req(is_inside(&resolve(&path), root), "pool path escapes source root")?;
            if path.is_file() {
                packed.push(sources.read(&path)?);
            } else {
                absent.push(name);
            }
        }
        let name = file_name(directory);
        if !absent.is_empty() {
            missing.push(jobj! { "map" => name, "missing" => absent });
            continue;
        }
        match inspect_pair(&packed[0], &packed[1]) {
            Ok(report) => {
                if let Some(J::Obj(counts)) = report.get("counts") {
                    for (k, v) in counts {
                        totals.add(k, v.as_i128().unwrap_or(0));
                    }
                }
                let mut entry = jobj! { "map" => name };
                crate::cmd::common::merge(&mut entry, report);
                reports.push(entry);
            }
            Err(error) => errors.push(jobj! { "map" => name, "error" => error }),
        }
    }
    let all = directories.len() == reports.len() && reports.len() == 74 && missing.is_empty() && errors.is_empty();
    Ok(jobj! {
        "expected_native_map_count" => 74,
        "map_directories_discovered" => directories.len(),
        "all_74_maps_audited" => all,
        "maps" => reports,
        "missing_pool_pairs" => missing,
        "errors" => errors,
        "totals" => totals.to_json(),
        "originality_verified" => false,
        "originality_note" => "All supplied MAPS directories audited; no ISO/baseline comparison. Installed files may have prior edits.",
    })
}

pub fn default_evidence() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap_or(Path::new(".")).join("re_workspace").join(EVIDENCE_NAME)
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["game-root", "evidence"])?;
    p.exact(2, "probe-native-scene-projection EXE OUTPUT [--game-root ROOT] [--evidence FILE]")?;
    let exe = std::fs::canonicalize(&p.pos[0]).map(|x| resolve(&x)).map_err(|e| e.to_string())?;
    let evidence = std::fs::canonicalize(p.opt("evidence").map(PathBuf::from).unwrap_or_else(default_evidence))
        .map(|x| resolve(&x))
        .map_err(|e| e.to_string())?;
    let root = match p.opt("game-root") {
        Some(r) => Some(std::fs::canonicalize(r).map(|x| resolve(&x)).map_err(|e| e.to_string())?),
        None => None,
    };
    let output = resolve(Path::new(&p.pos[1]));
    let mut forbidden = vec![exe.parent().unwrap().to_path_buf(), evidence.parent().unwrap().to_path_buf(), installed_game()];
    forbidden.extend(root.clone());
    req(std::fs::symlink_metadata(&p.pos[1]).is_err() && !output.exists(), "output must be a new explicit path")?;
    req(output.parent().is_some_and(Path::is_dir), "output parent must already exist")?;
    req(forbidden.iter().all(|f| !is_inside(&output, &resolve(f))), "output must be outside EXE/evidence input directories and all game directories")?;
    let mut sources = Sources::default();
    let exe_bytes = sources.read(&exe)?;
    let evidence_bytes = sources.read(&evidence)?;
    let mut result = inspect_code(&exe_bytes, &evidence_bytes)?;
    result.set("evidence_path", display(&evidence));
    result.set("functional_models", model_cases()?);
    if let Some(root) = &root {
        result.set("map_audit", scan(root, &mut sources)?);
    }
    result.set("source_hashes", sources.finish()?);
    result.set(
        "limitations",
        vec![
            "Static instruction and disk observations only; game/render/FPU behavior was not executed or observed.",
            "World Y is consumed by this native projection, but parent inheritance, visibility, terrain units and editor equivalence are unproven.",
            "Float conversions are conditional on explicitly hypothetical x87 controls; no gameplay rounding mode claim.",
            "Conversion model excludes traps/exception masks and indefinite out-of-range integer results.",
            "Map audit validity is an explicit diagnostic predicate, not native culling or render eligibility.",
            "Pool version/capacity/length checks are stricter than native; PFIL wrapper/trailing integrity is unproven.",
            "Source hashes compare endpoints; transient concurrent edits and original-media identity are not established.",
        ],
    );
    write_new(&output, &(result.compact(true) + "\n"))?;
    let audit = result.get("map_audit");
    let len_of = |key: &str| audit.and_then(|a| a.get(key)).map_or(0, J::len);
    let all = audit.and_then(|a| a.get("all_74_maps_audited")).cloned().unwrap_or(J::Null);
    let summary = jobj! {
        "output" => display(&output),
        "instruction_assertions" => result.get("instruction_assertions").cloned().unwrap_or(J::Null),
        "maps" => len_of("maps"),
        "all_74_maps_audited" => all.clone(),
        "errors" => len_of("errors"),
        "missing_pool_pairs" => len_of("missing_pool_pairs"),
        "totals" => audit.and_then(|a| a.get("totals")).cloned().unwrap_or_else(J::obj),
        "sources_unchanged" => sources.list.len(),
        "worldY_used_static" => true,
        "runtime_rounding_mode_observed" => false,
    };
    println!("{}", summary.line(true));
    Ok(if audit.is_some() && all != J::Bool(true) { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn integer_projection_matches_the_documented_boundaries() {
        // model_cases() asserts every fixed boundary from the Python script internally.
        let cases = model_cases().unwrap();
        assert_eq!(cases.get("integer_projection_cases").unwrap().len(), 14 + 12 * 2);
        assert_eq!(cases.get("conditional_float_cases").unwrap().len(), 11);
        let p = project(10, -3, 2, -1, 0, 0);
        assert_eq!((p.screen_x, p.screen_y, p.height_branch), (0, 24, "SHL"));
        let wrapped = project(i32::MAX, 0, 1, 0, 0, 0);
        assert_eq!(wrapped.screen_x, 2147483646);
        assert_eq!(project(10, -3, 2, 32, 0, 0).masked_counts, [0, 1, 31]);
    }

    #[test]
    fn caller_slice_is_a_contiguous_listing() {
        let lines = caller_lines();
        assert_eq!(lines.len(), 29);
        assert!(lines.first().unwrap().starts_with("0x4b48ad"));
        assert!(lines.last().unwrap().starts_with("0x4b4916"));
    }

    fn position_blob(entries: &[(bool, [f32; 3])]) -> Vec<u8> {
        let mut data = u32s(&[1, entries.len() as u32]);
        for (active, xyz) in entries {
            data.push(*active as u8);
            for v in xyz {
                data.extend(v.to_le_bytes());
            }
            data.extend([0u8; 4]);
        }
        data
    }

    #[test]
    fn audits_parent_child_height_pairs() {
        let mut objs = objects(3);
        for slot in 0..3 {
            obj_activate(&mut objs, slot, 1);
            obj_record_i16(&mut objs, slot, 67, slot as i16);
        }
        obj_unlink_all(&mut objs, 3, 13);
        obj_column(&mut objs, 3, 13, 1, 0); // 1 -> parent 0 (Y differs)
        obj_column(&mut objs, 3, 13, 2, 2); // self link
        let positions = position_blob(&[(true, [1.0, 5.0, 2.0]), (true, [1.0, 2.5, 2.0]), (true, [0.0, 0.0, 0.0])]);
        let report = inspect_pair(&objs, &positions).unwrap();
        let counts = report.get("counts").unwrap();
        assert_eq!(counts.get("declared_children"), Some(&J::Int(2)));
        assert_eq!(counts.get("valid_children"), Some(&J::Int(1)));
        assert_eq!(counts.get("valid_children_unequal_Y"), Some(&J::Int(1)));
        assert_eq!(counts.get("parent_self_link"), Some(&J::Int(1)));
        assert_eq!(counts.get("invalid_for_height_comparison"), Some(&J::Int(1)));
        let rep = report.get("representatives").unwrap();
        assert_eq!(rep.get("first_valid_child").unwrap().get("child_minus_parent_Y"), Some(&J::Float(-2.5)));
        assert_eq!(rep.get("first_unequal_Y_child"), rep.get("first_valid_child"));
    }

    #[test]
    fn nonfinite_positions_are_reported_not_compared() {
        let mut objs = objects(2);
        for slot in 0..2 {
            obj_activate(&mut objs, slot, 1);
            obj_record_i16(&mut objs, slot, 67, slot as i16);
        }
        obj_unlink_all(&mut objs, 2, 13);
        obj_column(&mut objs, 2, 13, 1, 0);
        let positions = position_blob(&[(true, [0.0, f32::NAN, 0.0]), (true, [0.0, 1.0, 0.0])]);
        let report = inspect_pair(&objs, &positions).unwrap();
        assert_eq!(report.get("counts").unwrap().get("valid_children"), None);
        assert_eq!(report.get("counts").unwrap().get("invalid_for_height_comparison"), Some(&J::Int(1)));
    }

    #[test]
    fn sources_detect_changes_between_reads() {
        let dir = TempDir::new();
        let path = write(dir.path(), "a.bin", b"one");
        let mut sources = Sources::default();
        sources.read(&path).unwrap();
        std::fs::write(&path, b"two").unwrap();
        assert!(sources.read(&path).is_err());
    }

    #[test]
    fn evidence_hash_gate_rejects_other_bundles() {
        assert!(inspect_code(&[0u8; 8], b"{}").unwrap_err().contains("fingerprint"));
    }
}
