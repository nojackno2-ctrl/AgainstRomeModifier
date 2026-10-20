//! `probe_native_visual_state.py`: fingerprint-checked static visual-state assertions and a
//! read-only map audit. Requires the retained eight-record Ghidra evidence
//! (`re_workspace/native-child-render-contract-20261011-evidence.json`, or `--evidence FILE`).
//! Does not execute the game or infer visibility/tick behavior.

use super::evidence::*;
use super::scene_projection::Sources;
use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const EVIDENCE_NAME: &str = "native-child-render-contract-20261011-evidence.json";
/// Offsets below are integers; the report also includes explicit hex to prevent ambiguity.
pub const AXES_DISK: [usize; 4] = [13, 17, 15, 19];
pub const AXES_RUNTIME: [usize; 4] = [0x12, 0x16, 0x14, 0x18];

pub const ANCHORS: [(u64, &[&str]); 4] = [
    (
        0x4b53c0,
        &[
            "0x4b5441: CMP EBX, -0x1", "0x4b5444: JNZ 0x004b5473",
            "0x4b5446: LEA EBX, [ESI*0x4 + 0x0]", "0x4b544d: ADD EBX, ESI",
            "0x4b544f: SHL EBX, 0x2", "0x4b5452: SUB EBX, ESI",
            "0x4b5454: MOV EBX, dword ptr [EBX*0x4 + 0xa14c0c]",
            "0x4b545b: SAR EBX, 0x10", "0x4b545e: CMP EBX, -0x1",
            "0x4b5461: JZ 0x004b5551", "0x4b5467: SHL EBX, 0x5",
            "0x4b546a: MOV EBX, dword ptr [EBX + 0xbb9a62]",
            "0x4b5470: SAR EBX, 0x10",
        ],
    ),
    (
        0x48c250,
        &[
            "0x48c2b5: CALL 0x00414df0", "0x48c315: MOV byte ptr [ESI], AL",
            "0x48c31b: CALL 0x00414e90", "0x48c32b: MOV dword ptr [ESI + 0x4], EAX",
            "0x48c332: CALL 0x00414e40", "0x48c342: MOV word ptr [ESI + 0x8], AX",
            "0x48c34a: CALL 0x00414e90", "0x48c35e: MOV dword ptr [ESI + 0xc], EAX",
            "0x48c365: CALL 0x00414e40", "0x48c379: MOV word ptr [ESI + 0x10], AX",
            "0x48c381: CALL 0x00414e40", "0x48c395: MOV word ptr [ESI + 0x12], AX",
            "0x48c39d: CALL 0x00414e40", "0x48c3b1: MOV word ptr [ESI + 0x14], AX",
            "0x48c3b9: CALL 0x00414e40", "0x48c3cd: MOV word ptr [ESI + 0x16], AX",
            "0x48c3d5: CALL 0x00414e40", "0x48c2e0: MOV word ptr [ESI + 0x18], AX",
            "0x48c2e8: ADD ESI, 0x20",
        ],
    ),
    (
        0x4b4790,
        &[
            "0x4b4a7e: IMUL EAX, EBX, 0x4c",
            "0x4b4a81: MOV EAX, dword ptr [EAX + 0xa14c0c]",
            "0x4b4a87: SAR EAX, 0x10", "0x4b4a8a: CMP EAX, -0x1",
            "0x4b4a8d: JZ 0x004b4ddb", "0x4b4a93: SHL EAX, 0x5",
            "0x4b4a96: MOV EDX, dword ptr [EAX + 0xbb9a70]",
            "0x4b4a9c: SAR EDX, 0x10",
            "0x4b4a9f: MOV EBP, dword ptr [EAX + 0xbb9a6c]",
            "0x4b4aa5: MOV dword ptr [ESP + 0x44], EDX",
            "0x4b4aa9: MOV EDX, dword ptr [EAX + 0xbb9a6e]",
            "0x4b4aaf: MOV EAX, dword ptr [EAX + 0xbb9a72]",
            "0x4b4ab5: SAR EAX, 0x10",
            "0x4b4ab8: MOV dword ptr [ESP + 0x40], EAX",
            "0x4b4ac1: SAR EDX, 0x10", "0x4b4ac5: SAR EBP, 0x10",
            "0x4b4ac8: MOV dword ptr [ESP + 0x44], EDX",
            "0x4b4b0d: MOV ECX, dword ptr [ESP + 0x48]", "0x4b4b11: PUSH ECX",
            "0x4b4b12: MOV EDI, dword ptr [ESP + 0x54]", "0x4b4b16: PUSH EDI",
            "0x4b4b17: MOV EAX, dword ptr [ESP + 0x54]", "0x4b4b1b: PUSH EAX",
            "0x4b4b1c: MOV EDX, dword ptr [ESP + 0x60]", "0x4b4b20: PUSH EDX",
            "0x4b4b21: PUSH EBP", "0x4b4b2a: CALL 0x004cbf70",
        ],
    ),
    (
        0x4cbf70,
        &[
            "0x4cbf77: MOV EDI, dword ptr [ESP + 0x9c]",
            "0x4cbfd8: LEA EAX, [ESP + 0x8]", "0x4cbfdc: PUSH EAX",
            "0x4cbfdd: LEA EAX, [ESP + 0x8]", "0x4cbfe1: PUSH EAX",
            "0x4cbfe2: MOV EBP, dword ptr [ESP + 0xa8]", "0x4cbfe9: PUSH EBP",
            "0x4cbfea: PUSH EDI",
            "0x4cbfeb: MOV EAX, dword ptr [ESP + 0xa8]", "0x4cbff2: PUSH EAX",
            "0x4cbff3: MOV EDX, dword ptr [ESP + 0xa8]", "0x4cbffa: PUSH EDX",
            "0x4cbffb: MOV ECX, dword ptr [ESP + 0xa8]", "0x4cc002: PUSH ECX",
            "0x4cc003: CALL 0x004e6b80",
        ],
    ),
];

pub fn default_evidence() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap_or(Path::new(".")).join("re_workspace").join(EVIDENCE_NAME)
}

pub fn inspect_code(exe: &[u8], retained: &[u8]) -> R<J> {
    req(sha256_hex(exe) == EXE_SHA256, "unsupported EXE fingerprint")?;
    let records = json_records(retained)?;
    req(records.len() == 8, "expected eight retained Ghidra records")?;
    let mut functions = J::arr();
    let mut total = 0usize;
    for (entry, anchors) in ANCHORS {
        let record = procedure_record(&records, entry, None, true)?;
        req(subject_sha(&record) == Some(EXE_SHA256), "evidence EXE mismatch")?;
        let body = body_of(&record)?;
        // Use provider instruction starts within the complete provider body, not a guessed linear
        // region. Embedded data need not be decoded as code.
        let lines = assembly_lines(&record);
        let starts: BTreeSet<u64> = lines.iter().map(|l| split_line(l).map(|(a, _)| a)).collect::<R<_>>()?;
        let mut verified = J::arr();
        for anchor in anchors {
            req(lines.iter().any(|l| l == anchor), &format!("retained assembly changed: {anchor}"))?;
            let (address, expected) = split_line(anchor)?;
            req(starts.contains(&address), "anchor outside Ghidra body")?;
            let ins = verify_line(exe, &body.ranges, anchor, false, "EXE instruction differs from retained anchor")?;
            verified.push(jobj! {
                "address" => anchor.split(": ").next().unwrap(),
                "bytes" => hex(&ins.bytes),
                "assembly" => expected,
                "capstone" => format!("{} {}", ins.mnemonic, ins.operands),
            });
        }
        total += verified.len();
        functions.push(jobj! {
            "entry" => format!("{entry:#x}"),
            "evidence_id" => evidence_id(&record),
            "body_ranges_inclusive" => body.raw_ranges.clone(),
            "anchors" => verified,
        });
    }
    let hex_list = |values: &[usize]| -> Vec<String> { values.iter().map(|v| format!("{v:#x}")).collect() };
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => total,
        "functions" => functions,
        "mapping" => jobj! {
            "objects_animidx_disk" => 71,
            "objects_animidx_runtime" => "0x12",
            "anim_alrframe_disk" => 5,
            "anim_alrframe_runtime" => "0x08",
            "anim_disk_stride" => 21,
            "anim_runtime_stride" => 32,
            "apt_axes_disk" => AXES_DISK.to_vec(),
            "apt_axes_disk_hex" => hex_list(&AXES_DISK),
            "apt_axes_runtime_hex" => hex_list(&AXES_RUNTIME),
            "signed_fields" => vec!["animidx", "ALRFrame", "APT axes"],
            "disk_to_runtime_hex" => jobj! {
                "0x00" => "0x00", "0x01" => "0x04", "0x05" => "0x08", "0x07" => "0x0c", "0x0b" => "0x10",
                "0x0d" => "0x12", "0x0f" => "0x14", "0x11" => "0x16", "0x13" => "0x18",
            },
            "apt_argument_order" => "axis0, axis1, axis2, axis3 (cdecl right-to-left pushes)",
            "frame_fallback" => "argument -1 -> object animidx -> signed ALRFrame",
        },
    })
}

/// One object's state: (json in Python key order, active, (ALRFrame, APT axes) when the anim link is usable, link kind).
type ObjectState = (J, bool, Option<(i16, [i16; 4])>, &'static str);

fn state(objs: &Objects, anim: &[u8], acount: usize, slot: usize) -> R<ObjectState> {
    let offset = 16 + slot * 79;
    let active = objs.data[offset] != 0;
    let animidx = i16_at(&objs.data, offset + 71)? as i64;
    let mut json = jobj! {
        "slot" => slot,
        "active" => active,
        "uid" => u32_at(&objs.data, offset + 3)?,
        "team" => u16_at(&objs.data, offset + 1)?,
        "type" => u16_at(&objs.data, offset + 75)?,
        "animidx" => animidx,
    };
    if animidx == -1 || !(0..acount as i64).contains(&animidx) {
        let link = if animidx == -1 { "missing" } else { "invalid" };
        json.set("anim_link", link);
        return Ok((json, active, None, link));
    }
    let start = 8 + animidx as usize * 21;
    let anim_active = anim[start] != 0;
    let link = if anim_active { "active" } else { "inactive" };
    let frame = i16_at(anim, start + 5)?;
    let mut axes = [0i16; 4];
    for (value, offset) in axes.iter_mut().zip(AXES_DISK) {
        *value = i16_at(anim, start + offset)?;
    }
    json.set("anim_active", anim_active);
    json.set("anim_link", link);
    json.set("ALRFrame", frame);
    json.set("APT_axes", axes.to_vec());
    Ok((json, active, Some((frame, axes)), link))
}

pub fn inspect_pair(objects_source: &[u8], anim_source: &[u8]) -> R<J> {
    let objs = Objects::from_decoded(decode(objects_source)?, false, "unsupported pool layout")?;
    let anim = decode(anim_source)?;
    req(anim.len() >= 8, "short pool header")?;
    let (aversion, acount) = (u32_at(&anim, 0)?, u32_at(&anim, 4)? as usize);
    req(aversion == 1 && acount <= 14000, "unsupported pool layout")?;
    req(anim.len() == 8 + acount * 25, "anim length mismatch")?;
    let parent_offset = objs.column(13);
    let mut counts = Counter::new();
    let mut frames: BTreeSet<i16> = BTreeSet::new();
    let mut axes: [BTreeSet<i16>; 4] = Default::default();
    let (mut roots, mut child_states) = (J::arr(), J::arr());
    for slot in 0..objs.count {
        let (mut child, active, extra, link) = state(&objs, &anim, acount, slot)?;
        let parent = u16_at(&objs.data, parent_offset + slot * 2)? as usize;
        if !active && parent == 0xffff {
            continue;
        }
        counts.inc("object_states");
        counts.inc(if active { "active_objects" } else { "inactive_objects" });
        counts.inc(format!("animlinks_{link}"));
        if let Some((frame, values)) = extra {
            frames.insert(frame);
            counts.add("negative_ALRFrame", frame < 0);
            for (i, value) in values.iter().enumerate() {
                axes[i].insert(*value);
                counts.add(format!("negative_axis{i}"), *value < 0);
            }
        }
        child.set("parent_slot", if parent == 0xffff { J::Null } else { J::from(parent) });
        if parent == 0xffff {
            child.set("parent_link", "root");
            roots.push(child);
            continue;
        }
        counts.inc("children");
        let status = if parent >= objs.count {
            "invalid"
        } else if parent == slot {
            "self"
        } else if objs.data[16 + parent * 79] != 0 {
            "active"
        } else {
            "inactive"
        };
        child.set("parent_link", status);
        counts.inc(format!("parentlinks_{status}"));
        if parent < objs.count {
            child.set("parent_state", state(&objs, &anim, acount, parent)?.0);
        }
        child_states.push(child);
    }
    let frame_list: Vec<i16> = frames.iter().copied().collect();
    let axis_lists: Vec<Vec<i16>> = axes.iter().map(|s| s.iter().copied().collect()).collect();
    Ok(jobj! {
        "object_slots" => objs.count,
        "anim_slots" => acount,
        "active_anim_slots" => (0..acount).filter(|&i| anim[8 + i * 21] != 0).count(),
        "decoded_sha256" => jobj! { "objects" => sha256_hex(&objs.data), "anim" => sha256_hex(&anim) },
        "pfil_wrapped" => jobj! { "objects" => objects_source.starts_with(b"PFIL@"), "anim" => anim_source.starts_with(b"PFIL@") },
        "counts" => counts.to_json(),
        "distinct_ALRFrame" => frame_list.clone(),
        "distinct_APT_axes" => axis_lists.clone(),
        "distinct_counts" => jobj! { "ALRFrame" => frame_list.len(), "APT_axes" => axis_lists.iter().map(Vec::len).collect::<Vec<_>>() },
        "root_states" => roots,
        "all_child_states" => child_states,
    })
}

pub fn scan(root: &Path, sources: &mut Sources) -> R<J> {
    let directories = map_dirs(root)?;
    let (mut reports, mut missing, mut errors) = (J::arr(), J::arr(), J::arr());
    let mut totals = Counter::new();
    let mut frames: BTreeSet<i128> = BTreeSet::new();
    let mut axes: [BTreeSet<i128>; 4] = Default::default();
    for directory in &directories {
        req(is_inside(&resolve(directory), root), "map directory escapes source root")?;
        let paths: Vec<PathBuf> = ["objects.dat", "anim.dat"].iter().map(|n| directory.join("DATA").join(n)).collect();
        let name = file_name(directory);
        let absent: Vec<String> = paths.iter().filter(|p| !p.is_file()).map(|p| file_name(p)).collect();
        if !absent.is_empty() {
            for path in paths.iter().filter(|p| p.is_file()) {
                req(is_inside(&resolve(path), root), "pool path escapes source root")?;
                sources.read(path)?;
            }
            missing.push(jobj! { "map" => name, "missing" => absent });
            continue;
        }
        for path in &paths {
            req(is_inside(&resolve(path), root), "pool path escapes source root")?;
        }
        let packed = paths.iter().map(|p| sources.read(p)).collect::<R<Vec<_>>>()?;
        match inspect_pair(&packed[0], &packed[1]) {
            Ok(report) => {
                if let Some(J::Obj(c)) = report.get("counts") {
                    for (k, v) in c {
                        totals.add(k, v.as_i128().unwrap_or(0));
                    }
                }
                if let Some(J::Arr(list)) = report.get("distinct_ALRFrame") {
                    frames.extend(list.iter().filter_map(J::as_i128));
                }
                if let Some(J::Arr(lists)) = report.get("distinct_APT_axes") {
                    for (set, list) in axes.iter_mut().zip(lists) {
                        if let J::Arr(values) = list {
                            set.extend(values.iter().filter_map(J::as_i128));
                        }
                    }
                }
                let mut entry = jobj! { "map" => name };
                super::common::merge(&mut entry, report);
                reports.push(entry);
            }
            Err(error) => errors.push(jobj! { "map" => name, "error" => error }),
        }
    }
    let frame_list: Vec<i128> = frames.iter().copied().collect();
    let axis_lists: Vec<Vec<i128>> = axes.iter().map(|s| s.iter().copied().collect()).collect();
    Ok(jobj! {
        "map_directories" => directories.len(),
        "expected_baseline_maps" => 74,
        "matches_74_map_baseline" => directories.len() == 74,
        "maps" => reports,
        "missing_pool_pairs" => missing,
        "errors" => errors,
        "totals" => totals.to_json(),
        "distinct_ALRFrame" => frame_list.clone(),
        "distinct_APT_axes" => axis_lists.clone(),
        "distinct_counts" => jobj! { "ALRFrame" => frame_list.len(), "APT_axes" => axis_lists.iter().map(Vec::len).collect::<Vec<_>>() },
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["game-root", "evidence"])?;
    p.exact(2, "probe-native-visual-state EXE OUTPUT [--game-root ROOT] [--evidence FILE]")?;
    let exe = resolve(Path::new(&p.pos[0]));
    let root = p.opt("game-root").map(|r| resolve(Path::new(r)));
    let evidence_path = p.opt("evidence").map(PathBuf::from).unwrap_or_else(default_evidence);
    let evidence_dir = resolve(&evidence_path).parent().map(Path::to_path_buf).unwrap_or_default();
    let output = resolve(Path::new(&p.pos[1]));
    req(std::fs::symlink_metadata(&p.pos[1]).is_err() && !output.exists(), "output must be a new explicit path")?;
    req(!is_inside(&output, &installed_game()) && root.as_ref().is_none_or(|r| !is_inside(&output, r)), "output must be outside source game and installed directories")?;
    req(!is_inside(&output, exe.parent().unwrap_or(Path::new(""))) || is_inside(&output, &evidence_dir), "output must be outside EXE directory (repository re_workspace is allowed)")?;
    req(output.parent().is_some_and(Path::is_dir), "output parent must already exist")?;
    let mut sources = Sources::default();
    let exe_bytes = sources.read(&exe)?;
    let evidence_bytes = sources.read(&evidence_path)?;
    let mut result = inspect_code(&exe_bytes, &evidence_bytes)?;
    result.set("evidence_path", display(&resolve(&evidence_path)));
    if let Some(root) = &root {
        result.set("map_audit", scan(root, &mut sources)?);
    }
    result.set("source_hashes", sources.finish()?);
    result.set(
        "limitations",
        vec![
            "Static EXE assertions and disk snapshots only; no game or runtime execution.",
            "No matrix semantics, parent visibility inheritance, culling, sorting or tick advancement claimed.",
            "animidx is objects disk +71 signed16, never the object slot; frame and axes are signed16.",
            "UID is disk u32 +3 and team u16 +1; parent column13 is a disk link only.",
            "Pool capacity/version/length validation is stricter than the native reader.",
            "PFIL decoded to declared size; wrapper/trailing compressed integrity not established.",
            "Negative frame/axes and inactive/shared links are observations, not corruption diagnoses.",
            "Hash equality checks endpoints only; concurrent transient changes cannot be excluded.",
        ],
    );
    write_new(&output, &(result.compact(true) + "\n"))?;
    let audit = result.get("map_audit");
    let len_of = |key: &str| audit.and_then(|a| a.get(key)).map_or(0, J::len);
    let summary = jobj! {
        "output" => display(&output),
        "instruction_assertions" => result.get("instruction_assertions").cloned().unwrap_or(J::Null),
        "maps" => len_of("maps"),
        "errors" => len_of("errors"),
        "missing_pool_pairs" => len_of("missing_pool_pairs"),
        "totals" => audit.and_then(|a| a.get("totals")).cloned().unwrap_or_else(J::obj),
        "sources_unchanged" => sources.list.len(),
    };
    println!("{}", summary.line(true));
    Ok(if len_of("errors") > 0 || len_of("missing_pool_pairs") > 0 { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn anim_blob(count: usize, entries: &[(usize, bool, i16, [i16; 4])]) -> Vec<u8> {
        let mut data = u32s(&[1, count as u32]);
        data.resize(8 + count * 25, 0);
        for (i, active, frame, axes) in entries {
            let start = 8 + i * 21;
            data[start] = *active as u8;
            data[start + 5..start + 7].copy_from_slice(&frame.to_le_bytes());
            for (offset, value) in AXES_DISK.iter().zip(axes) {
                data[start + offset..start + offset + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        data
    }

    #[test]
    fn collects_states_parent_links_and_distinct_values() {
        let mut objs = objects(3);
        for slot in 0..3 {
            obj_activate(&mut objs, slot, 5 + slot as u16);
            obj_record_i16(&mut objs, slot, 71, -1);
        }
        obj_record_i16(&mut objs, 0, 71, 0);
        obj_record_i16(&mut objs, 1, 71, 1);
        obj_unlink_all(&mut objs, 3, 13);
        obj_column(&mut objs, 3, 13, 1, 0); // child 1 -> parent 0
        obj_column(&mut objs, 3, 13, 2, 9); // child 2 -> invalid parent
        let anim = anim_blob(2, &[(0, true, -3, [1, 2, 3, 4]), (1, false, 7, [0, 0, 0, -9])]);
        let report = inspect_pair(&objs, &anim).unwrap();
        let counts = report.get("counts").unwrap();
        assert_eq!(counts.get("object_states"), Some(&J::Int(3)));
        assert_eq!(counts.get("animlinks_active"), Some(&J::Int(1)));
        assert_eq!(counts.get("animlinks_inactive"), Some(&J::Int(1)));
        assert_eq!(counts.get("animlinks_missing"), Some(&J::Int(1)));
        assert_eq!(counts.get("negative_ALRFrame"), Some(&J::Int(1)));
        assert_eq!(counts.get("negative_axis3"), Some(&J::Int(1)));
        assert_eq!(counts.get("parentlinks_active"), Some(&J::Int(1)));
        assert_eq!(counts.get("parentlinks_invalid"), Some(&J::Int(1)));
        assert_eq!(report.get("distinct_ALRFrame"), Some(&J::Arr(vec![J::Int(-3), J::Int(7)])));
        assert_eq!(report.get("root_states").unwrap().len(), 1);
        let children = match report.get("all_child_states").unwrap() {
            J::Arr(c) => c,
            _ => panic!(),
        };
        assert_eq!(children[0].get("parent_state").unwrap().get("slot"), Some(&J::Int(0)));
        assert_eq!(children[0].get("anim_link").and_then(J::as_str), Some("inactive"));
    }

    #[test]
    fn inactive_unlinked_objects_are_skipped() {
        let mut objs = objects(2);
        obj_activate(&mut objs, 0, 1);
        obj_unlink_all(&mut objs, 2, 13);
        obj_record_i16(&mut objs, 0, 71, -1);
        let report = inspect_pair(&objs, &anim_blob(0, &[])).unwrap();
        assert_eq!(report.get("counts").unwrap().get("object_states"), Some(&J::Int(1)));
    }

    #[test]
    fn rejects_bad_layouts() {
        assert!(inspect_pair(&objects(1), &u32s(&[1, 2])).is_err());
        assert!(inspect_pair(&objects(1), &u32s(&[2, 0])).is_err());
    }

    #[test]
    fn anchor_tables_are_well_formed() {
        for (entry, anchors) in ANCHORS {
            assert!(!anchors.is_empty(), "{entry:#x}");
            for anchor in anchors {
                let (address, _) = split_line(anchor).unwrap();
                assert!(address >= entry - 0x1000 && address < entry + 0x1000, "{anchor}");
            }
        }
        assert_eq!(ANCHORS.iter().map(|(_, a)| a.len()).sum::<usize>(), 13 + 19 + 27 + 15);
    }

    #[test]
    fn code_gate_rejects_other_exe_and_bundle_sizes() {
        assert!(inspect_code(&[0u8; 8], b"{}").unwrap_err().contains("fingerprint"));
    }
}
