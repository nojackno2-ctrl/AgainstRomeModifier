//! Save/load parity for `arm_scenario.json` across events: conditions that target script spawns and
//! prebuilt DATA objects, actions that spawn/move/end the mission, legacy migration and malformed input.
//! `csharp_scenario_save_load_differential` compares every corpus case with verdicts and outputs frozen from
//! the (since removed) C# `ScenarioDocument.Load/Save` and Apply compile path
//! (tests/fixtures/scenario_roundtrip_oracle.json); no .NET or retail files required.
use against_rome_scenario::*;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::PathBuf};

const A: &str = "12345678-1122-3344-5566-778899001122"; // script squad
const B: &str = "22345678-1122-3344-5566-778899001122"; // prebuilt house, slot 9, uid u32::MAX
const C: &str = "32345678-1122-3344-5566-778899001122"; // script building
const D: &str = "42345678-1122-3344-5566-778899001122"; // prebuilt house, slot 10, raw rotation
const HEX: &str = "abcdefab-cdef-abcd-efab-cdefabcdefab"; // letters, to observe GUID case handling
const ALIASES: [&str; 2] = ["unit", "house"];

fn aliases() -> Vec<String> {
    ALIASES.iter().map(|s| s.to_string()).collect()
}

/// A v6 document whose four events reference each other's objects across condition kinds and action kinds.
fn rich() -> Value {
    json!({
        "Version": 6,
        "Spawns": [
            {"Alias":"unit","X":100.5,"Z":120.5,"Team":1,"Count":5,"Angle":45,"Y":3.25,"Id":A},
            {"Alias":"house","X":50,"Z":70,"Team":0,"Prebuilt":true,"Id":B},
            {"Alias":"house","X":200.5,"Z":300.5,"Team":-1,"Id":C},
            {"Alias":"house","X":0.1,"Z":16383,"Team":2,"Prebuilt":true,"Angle":90,"RawRotation":1.5707964,"Id":D}
        ],
        "DataSlots": [
            {"Slot":9,"Uid":4294967295u32,"SpawnId":B},
            {"Slot":10,"Uid":77,"SpawnId":D}
        ],
        "Events": [
            {"Name":"opening","DelaySeconds":3,
             "Conditions":[{"Kind":0,"TargetId":A},{"Kind":1,"TargetId":B},
                           {"Kind":2,"TargetId":B,"MinX":50,"MinZ":60,"MaxX":200,"MaxZ":300}],
             "Actions":[{"Kind":0,"Text":"Привет!"},{"Kind":1,"Team":0,"OtherTeam":2,"Hostile":false},
                        {"Kind":2,"Team":1,"Alias":"unit","Count":20,"X":200.5,"Z":300.5,
                         "MoveDestination":{"X":600,"Z":700}}]},
            {"Name":"reinforce","Repeat":true,"DelaySeconds":10,
             "Conditions":[{"Kind":0,"TargetId":C}],
             "Actions":[{"Kind":2,"Team":3,"Alias":"unit","Count":1,"X":0.1,"Z":16383},{"Kind":0,"Text":"Repeat"}]},
            {"Name":"victory","DelaySeconds":600,
             "Conditions":[{"Kind":1,"TargetId":A},{"Kind":2,"TargetId":C,"MinX":0,"MinZ":0,"MaxX":16383,"MaxZ":16383},
                           {"Kind":0,"TargetId":D}],
             "Actions":[{"Kind":0,"Text":"Win"},{"Kind":3}]},
            {"Name":"draft","Enabled":false,"DelaySeconds":1,
             "Conditions":[{"Kind":0,"TargetId":"99999999-9999-9999-9999-999999999999"}],
             "Actions":[{"Kind":4}]}
        ]
    })
}

fn mutate(f: impl FnOnce(&mut Value)) -> Value {
    let mut v = rich();
    f(&mut v);
    v
}

struct Case {
    name: String,
    bytes: Vec<u8>,
    /// Expected `from_json` verdict of the Rust port.
    rust_load: bool,
    /// Expected `to_json` verdict after a successful load.
    rust_save: bool,
}

fn case(name: &str, v: Value, rust_load: bool, rust_save: bool) -> Case {
    Case { name: name.into(), bytes: serde_json::to_vec(&v).unwrap(), rust_load, rust_save }
}
fn raw_case(name: &str, bytes: &[u8], rust_load: bool, rust_save: bool) -> Case {
    Case { name: name.into(), bytes: bytes.to_vec(), rust_load, rust_save }
}

fn corpus() -> Vec<Case> {
    let mut c = vec![];
    // ---- well-formed documents ----
    c.push(case("rich_v6", rich(), true, true));
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend(serde_json::to_vec(&rich()).unwrap());
    c.push(raw_case("rich_bom_prefixed", &bom, true, true));
    c.push(raw_case("empty_object", b"{}", true, true));
    c.push(case("unknown_fields_ignored", mutate(|v| {
        v["Extra"] = json!([1, 2]);
        v["Events"][0]["Mystery"] = json!("x");
        v["Events"][0]["Actions"][0]["Other"] = json!(null);
    }), true, true));
    c.push(case("lowercase_keys_ignored", json!({"version":6,"events":[{"name":"x"}]}), true, true));
    c.push(case("uppercase_guid_text", mutate(|v| {
        v["Spawns"][2]["Id"] = json!(HEX);
        v["Events"][1]["Conditions"][0]["TargetId"] = json!(HEX.to_uppercase());
        v["Events"][2]["Conditions"][1]["TargetId"] = json!(HEX);
    }), true, true));
    // Records encoded as positional arrays are accepted by serde derives but not by System.Text.Json.
    c.push(raw_case("json_root_array_form", b"[6,[],[],[]]", false, false));
    c.push(case("spawn_as_array", mutate(|v| v["Spawns"][0] = json!(["unit", 1.0, 2.0])), false, false));
    c.push(case("event_as_array", mutate(|v| v["Events"][0] = json!(["x"])), false, false));
    c.push(case("condition_as_array", mutate(|v| v["Events"][0]["Conditions"][0] = json!([0, A])), false, false));
    c.push(case("move_destination_as_array", mutate(|v| v["Events"][0]["Actions"][2]["MoveDestination"] = json!([1, 2])), false, false));
    c.push(case("disabled_dangling_target", mutate(|v| {
        v["Events"][3]["Conditions"][0]["TargetId"] = json!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    }), true, true));
    c.push(case("events_256", mutate(|v| {
        let one = v["Events"][1].clone();
        v["Events"] = Value::Array((0..256).map(|i| { let mut e = one.clone(); e["Name"] = json!(format!("e{i}")); e }).collect());
    }), true, true));
    c.push(case("conditions_32", mutate(|v| {
        let one = v["Events"][0]["Conditions"][0].clone();
        v["Events"][0]["Conditions"] = Value::Array(vec![one; 32]);
    }), true, true));
    c.push(case("name_100_chars", mutate(|v| { v["Events"][0]["Name"] = json!("n".repeat(100)); }), true, true));
    c.push(case("area_single_point", mutate(|v| {
        v["Events"][0]["Conditions"][2] = json!({"Kind":2,"TargetId":B,"MinX":7,"MinZ":9,"MaxX":7,"MaxZ":9});
    }), true, true));
    // Legacy v3: ids and slot owners are derived from the position/order, condition points at the derived id.
    c.push(case("legacy_v3_migrates", json!({
        "Version": 3,
        "Spawns": [{"Alias":"unit","X":1,"Z":2,"Team":0,"Count":3},
                   {"Alias":"house","X":50,"Z":70,"Team":0,"Prebuilt":true}],
        "DataSlots": [{"Slot":4,"Uid":99}],
        "Events": [{"Name":"legacy","Conditions":[{"Kind":0,"TargetId":legacy_spawn_id(1).to_string()},
                                                    {"Kind":1,"TargetId":legacy_spawn_id(0).to_string()}],
                    "Actions":[{"Kind":0,"Text":"old"}]}]
    }), true, true));
    c.push(case("legacy_v3_slot_count_mismatch_stays_unbound", json!({
        "Version": 3,
        "Spawns": [{"Alias":"house","X":5,"Z":6,"Team":0,"Prebuilt":true},
                   {"Alias":"house","X":7,"Z":8,"Team":0,"Prebuilt":true}],
        "DataSlots": [{"Slot":4,"Uid":99}]
    }), true, true));

    // ---- malformed JSON text ----
    c.push(raw_case("json_empty_file", b"", false, false));
    c.push(raw_case("json_truncated", &serde_json::to_vec(&rich()).unwrap()[..100], false, false));
    c.push(raw_case("json_trailing_garbage", b"{} x", false, false));
    c.push(raw_case("json_trailing_comma", b"{\"Version\":6,}", false, false));
    c.push(raw_case("json_comment", b"{/*c*/\"Version\":6}", false, false));
    c.push(raw_case("json_root_array", b"[]", false, false));
    c.push(raw_case("json_root_null", b"null", false, false));
    c.push(raw_case("json_duplicate_key", br#"{"Version":6,"Version":6}"#, false, false));

    // ---- structural nulls and wrong types ----
    for key in ["Spawns", "DataSlots", "Events"] {
        c.push(case(&format!("null_{key}"), mutate(|v| v[key] = Value::Null), false, false));
    }
    c.push(case("action_kind_as_string", mutate(|v| v["Events"][0]["Actions"][0]["Kind"] = json!("Message")), false, false));
    c.push(case("condition_kind_as_string", mutate(|v| v["Events"][0]["Conditions"][0]["Kind"] = json!("ObjectExists")), false, false));
    c.push(case("count_fractional", mutate(|v| v["Events"][0]["Actions"][2]["Count"] = json!(1.5)), false, false));
    c.push(case("delay_as_string", mutate(|v| v["Events"][0]["DelaySeconds"] = json!("3")), false, false));
    c.push(case("uid_overflow", mutate(|v| v["DataSlots"][0]["Uid"] = json!(4294967296u64)), false, false));
    c.push(case("float_overflow_x", mutate(|v| v["Spawns"][0]["X"] = json!(1e40)), false, false));

    // ---- GUID text forms System.Text.Json refuses ----
    let n_form = B.replace('-', "");
    c.push(case("guid_simple_form", mutate(|v| v["Spawns"][1]["Id"] = json!(n_form.clone())), false, false));
    c.push(case("guid_braced_form", mutate(|v| v["Events"][0]["Conditions"][0]["TargetId"] = json!(format!("{{{A}}}"))), false, false));
    c.push(case("guid_urn_form", mutate(|v| v["Events"][0]["Conditions"][0]["TargetId"] = json!(format!("urn:uuid:{A}"))), false, false));
    c.push(case("guid_not_hex", mutate(|v| v["Spawns"][0]["Id"] = json!("zzzzzzzz-1122-3344-5566-778899001122")), false, false));

    // ---- identity and DATA binding ----
    c.push(case("version_0", mutate(|v| v["Version"] = json!(0)), false, false));
    c.push(case("version_7", mutate(|v| v["Version"] = json!(7)), false, false));
    c.push(case("v6_spawn_without_id", mutate(|v| { v["Spawns"][0].as_object_mut().unwrap().remove("Id"); }), false, false));
    c.push(case("duplicate_spawn_id", mutate(|v| v["Spawns"][2]["Id"] = json!(A)), false, false));
    c.push(case("slot_points_at_script_spawn", mutate(|v| v["DataSlots"][0]["SpawnId"] = json!(A)), false, false));
    c.push(case("slot_points_at_unknown_spawn", mutate(|v| v["DataSlots"][0]["SpawnId"] = json!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")), false, false));
    c.push(case("duplicate_slot_number", mutate(|v| v["DataSlots"][1]["Slot"] = json!(9)), false, false));
    c.push(case("duplicate_slot_owner", mutate(|v| v["DataSlots"][1]["SpawnId"] = json!(B)), false, false));
    c.push(case("slot_14000", mutate(|v| v["DataSlots"][0]["Slot"] = json!(14000)), false, false));
    c.push(case("slot_negative", mutate(|v| v["DataSlots"][0]["Slot"] = json!(-1)), false, false));
    c.push(case("uid_zero", mutate(|v| v["DataSlots"][0]["Uid"] = json!(0)), false, false));
    c.push(case("raw_rotation_contradicts_angle", mutate(|v| v["Spawns"][3]["Angle"] = json!(91)), false, false));
    c.push(case("raw_rotation_on_script_spawn", mutate(|v| { v["Spawns"][0]["RawRotation"] = json!(0.78539816); v["Spawns"][0]["Angle"] = json!(45); }), false, false));
    c.push(case("prebuilt_target_loses_binding", mutate(|v| { v["DataSlots"].as_array_mut().unwrap().remove(0); }), false, false));
    c.push(case("prebuilt_target_zero_uid", mutate(|v| v["DataSlots"][0]["Uid"] = json!(0)), false, false));

    // ---- cross-event conditions ----
    c.push(case("condition_kind_3", mutate(|v| v["Events"][0]["Conditions"][0]["Kind"] = json!(3)), false, false));
    c.push(case("condition_kind_negative", mutate(|v| v["Events"][0]["Conditions"][0]["Kind"] = json!(-1)), false, false));
    c.push(case("condition_nil_target", mutate(|v| v["Events"][0]["Conditions"][0]["TargetId"] = json!("00000000-0000-0000-0000-000000000000")), false, false));
    c.push(case("condition_missing_target", mutate(|v| { v["Events"][0]["Conditions"][0].as_object_mut().unwrap().remove("TargetId"); }), false, false));
    c.push(case("condition_dangling_target_enabled", mutate(|v| v["Events"][0]["Conditions"][0]["TargetId"] = json!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")), false, false));
    c.push(case("condition_target_deleted_with_spawn", mutate(|v| { v["Spawns"].as_array_mut().unwrap().remove(0); }), false, false));
    for (name, field, value) in [
        ("area_min_gt_max_x", "MinX", 201), ("area_min_gt_max_z", "MinZ", 301),
        ("area_negative_min", "MinX", -1), ("area_max_out_of_range", "MaxZ", 16384),
    ] {
        c.push(case(name, mutate(|v| v["Events"][0]["Conditions"][2][field] = json!(value)), false, false));
    }
    c.push(case("conditions_33", mutate(|v| {
        let one = v["Events"][0]["Conditions"][0].clone();
        v["Events"][0]["Conditions"] = Value::Array(vec![one; 33]);
    }), false, false));
    c.push(case("events_257", mutate(|v| {
        let one = v["Events"][1].clone();
        v["Events"] = Value::Array((0..257).map(|i| { let mut e = one.clone(); e["Name"] = json!(format!("e{i}")); e }).collect());
    }), false, false));

    // ---- actions and event shape ----
    c.push(case("action_kind_9", mutate(|v| v["Events"][0]["Actions"][0]["Kind"] = json!(9)), false, false));
    c.push(case("action_kind_negative", mutate(|v| v["Events"][0]["Actions"][0]["Kind"] = json!(-1)), false, false));
    c.push(case("event_no_actions", mutate(|v| v["Events"][0]["Actions"] = json!([])), false, false));
    c.push(case("actions_33", mutate(|v| v["Events"][0]["Actions"] = Value::Array(vec![json!({"Kind":0,"Text":"x"}); 33])), false, false));
    c.push(case("disabled_event_no_actions", mutate(|v| v["Events"][3]["Actions"] = json!([])), false, false));
    c.push(case("event_name_blank", mutate(|v| v["Events"][0]["Name"] = json!("  ")), false, false));
    c.push(case("event_name_101", mutate(|v| v["Events"][0]["Name"] = json!("n".repeat(101))), false, false));
    c.push(case("event_name_surrogate_pairs_over_100_units", mutate(|v| v["Events"][0]["Name"] = json!("😀".repeat(51))), false, false));
    c.push(case("repeat_delay_zero", mutate(|v| v["Events"][1]["DelaySeconds"] = json!(0)), false, false));
    c.push(case("delay_negative", mutate(|v| v["Events"][0]["DelaySeconds"] = json!(-1)), false, false));
    c.push(case("delay_86401", mutate(|v| v["Events"][0]["DelaySeconds"] = json!(86401)), false, false));
    c.push(case("terminal_not_last", mutate(|v| v["Events"][2]["Actions"] = json!([{"Kind":3},{"Kind":0,"Text":"late"}])), false, false));
    c.push(case("terminal_in_repeat_event", mutate(|v| v["Events"][1]["Actions"] = json!([{"Kind":4}])), false, false));
    c.push(case("two_terminal_actions", mutate(|v| v["Events"][2]["Actions"] = json!([{"Kind":3},{"Kind":4}])), false, false));
    c.push(case("message_blank", mutate(|v| v["Events"][0]["Actions"][0]["Text"] = json!(" ")), false, false));
    c.push(case("message_nul", mutate(|v| v["Events"][0]["Actions"][0]["Text"] = json!("a\u{0}b")), false, false));
    c.push(case("message_over_1000_bytes", mutate(|v| v["Events"][0]["Actions"][0]["Text"] = json!("я".repeat(1001))), false, false));
    c.push(case("diplomacy_same_team", mutate(|v| v["Events"][0]["Actions"][1]["OtherTeam"] = json!(0)), false, false));
    c.push(case("diplomacy_team_8", mutate(|v| v["Events"][0]["Actions"][1]["Team"] = json!(8)), false, false));
    c.push(case("spawn_action_count_0", mutate(|v| v["Events"][0]["Actions"][2]["Count"] = json!(0)), false, false));
    c.push(case("spawn_action_count_21", mutate(|v| v["Events"][0]["Actions"][2]["Count"] = json!(21)), false, false));
    c.push(case("spawn_action_x_out_of_range", mutate(|v| v["Events"][0]["Actions"][2]["X"] = json!(16384)), false, false));
    c.push(case("spawn_action_blank_alias", mutate(|v| v["Events"][0]["Actions"][2]["Alias"] = json!("")), false, false));
    c.push(case("move_destination_on_message", mutate(|v| v["Events"][0]["Actions"][0]["MoveDestination"] = json!({"X":1,"Z":1})), false, false));
    c.push(case("move_destination_out_of_range", mutate(|v| v["Events"][0]["Actions"][2]["MoveDestination"] = json!({"X":1,"Z":16384})), false, false));
    c.push(case("move_destination_missing_axis", mutate(|v| v["Events"][0]["Actions"][2]["MoveDestination"] = json!({"X":1})), false, false));
    // Alias membership is a compile-time gate (needs the catalog), not a load/save one.
    c.push(case("spawn_action_unknown_alias", mutate(|v| v["Events"][0]["Actions"][2]["Alias"] = json!("ghost")), true, true));
    c.push(case("script_spawn_unknown_alias", mutate(|v| v["Spawns"][0]["Alias"] = json!("ghost")), true, true));
    // ---- spawn shape ----
    c.push(case("spawn_x_out_of_range", mutate(|v| v["Spawns"][0]["X"] = json!(16384)), false, false));
    c.push(case("spawn_blank_alias", mutate(|v| v["Spawns"][0]["Alias"] = json!(" ")), false, false));
    c.push(case("spawn_squad_team_8", mutate(|v| v["Spawns"][0]["Team"] = json!(8)), false, false));
    c.push(case("spawn_building_team_16", mutate(|v| v["Spawns"][2]["Team"] = json!(16)), false, false));
    c.push(case("spawn_count_21", mutate(|v| v["Spawns"][0]["Count"] = json!(21)), false, false));
    c
}

fn document(case: &Case) -> Option<ScenarioDocument> {
    ScenarioDocument::from_json(&case.bytes).ok()
}

/// Version is normalized by Save; everything else must survive a save/load cycle unchanged.
fn normalized(d: &ScenarioDocument) -> Value {
    let mut v = serde_json::to_value(d).unwrap();
    v["Version"] = json!(6);
    v
}

fn idle() -> Vec<u8> {
    BciImage::idle().serialize()
}

#[test]
fn corpus_names_are_unique_and_rust_verdicts_match_expectations() {
    let cases = corpus();
    let names: BTreeSet<_> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "duplicate case name");
    for c in &cases {
        let loaded = ScenarioDocument::from_json(&c.bytes);
        assert_eq!(loaded.is_ok(), c.rust_load, "{}: load verdict {:?}", c.name, loaded.as_ref().err());
        if let Ok(d) = loaded {
            let saved = d.to_json();
            assert_eq!(saved.is_ok(), c.rust_save, "{}: save verdict {:?}", c.name, saved.err());
        }
    }
}

#[test]
fn save_load_is_idempotent_and_lossless_for_every_cross_event_case() {
    let mut saved_cases = 0;
    for c in corpus() {
        let Some(doc) = document(&c) else { continue };
        let Ok(first) = doc.to_json() else { continue };
        let reloaded = ScenarioDocument::from_json(&first).unwrap_or_else(|e| panic!("{}: reload {e}", c.name));
        assert_eq!(normalized(&reloaded), normalized(&doc), "{}: model changed across save/load", c.name);
        assert_eq!(reloaded.version, 6, "{}", c.name);
        assert_eq!(reloaded.to_json().unwrap(), first, "{}: second save differs", c.name);
        // Everything the saved file contains is also valid JSON the editor wrote itself: no nulls from floats.
        let text = String::from_utf8(first).unwrap();
        assert!(!text.contains("NaN") && !text.contains("Infinity"), "{}", c.name);
        saved_cases += 1;
    }
    assert!(saved_cases >= 12, "only {saved_cases} saved cases");
}

#[test]
fn legacy_documents_get_stable_ids_slot_owners_and_condition_targets() {
    let c = corpus().into_iter().find(|c| c.name == "legacy_v3_migrates").unwrap();
    let doc = document(&c).unwrap();
    assert_eq!(doc.spawns[0].id, legacy_spawn_id(0));
    assert_eq!(doc.spawns[1].id, legacy_spawn_id(1));
    assert_eq!(doc.data_slots[0].spawn_id, doc.spawns[1].id, "single prebuilt owns the single legacy slot");
    assert_eq!(doc.events[0].conditions[0].target_id, doc.spawns[1].id);
    let saved = ScenarioDocument::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(saved.version, 6);
    assert_eq!(saved.spawns[0].id, doc.spawns[0].id);

    let c = corpus().into_iter().find(|c| c.name == "legacy_v3_slot_count_mismatch_stays_unbound").unwrap();
    let doc = document(&c).unwrap();
    assert!(doc.data_slots[0].spawn_id.is_nil(), "ambiguous legacy slots must never be guessed");
    let saved = ScenarioDocument::from_json(&doc.to_json().unwrap()).unwrap();
    assert!(saved.data_slots[0].spawn_id.is_nil());
}

#[test]
fn guid_text_is_written_lowercase_hyphenated_and_read_case_insensitively() {
    let c = corpus().into_iter().find(|c| c.name == "uppercase_guid_text").unwrap();
    let doc = document(&c).unwrap();
    assert_eq!(doc.events[1].conditions[0].target_id.to_string(), HEX);
    let text = String::from_utf8(doc.to_json().unwrap()).unwrap();
    assert!(text.contains(&format!("\"TargetId\": \"{HEX}\"")));
    assert!(!text.contains(&HEX.to_uppercase()));
}

#[test]
fn compile_survives_save_load_and_tracks_the_saved_binding() {
    let source = idle();
    let aliases = aliases();
    let doc = document(&corpus().into_iter().find(|c| c.name == "rich_v6").unwrap()).unwrap();
    let before = compile_script(&source, &doc, None, Some(&aliases)).unwrap();
    assert_ne!(before, source, "the cross-event scenario must actually change the script");
    let reloaded = ScenarioDocument::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(compile_script(&source, &reloaded, None, Some(&aliases)).unwrap(), before);
    BciImage::parse(&before).unwrap();

    // Not vacuous: a different saved UID for the prebuilt target changes the generated condition code.
    let mut rebound = reloaded.clone();
    rebound.data_slots[0].uid = 4242;
    let rebound = ScenarioDocument::from_json(&rebound.to_json().unwrap()).unwrap();
    assert_ne!(compile_script(&source, &rebound, None, Some(&aliases)).unwrap(), before);
}

#[test]
fn to_json_fails_closed_and_leaves_the_document_untouched() {
    let base = document(&corpus().into_iter().find(|c| c.name == "rich_v6").unwrap()).unwrap();
    let mut bad: Vec<(&str, ScenarioDocument)> = vec![];
    let mut push = |name, f: &dyn Fn(&mut ScenarioDocument)| {
        let mut d = base.clone();
        f(&mut d);
        bad.push((name, d));
    };
    push("nan spawn x", &|d| d.spawns[0].x = f32::NAN);
    push("inf spawn y", &|d| d.spawns[0].y = f32::INFINITY);
    push("nan raw rotation", &|d| d.spawns[3].raw_rotation = Some(f32::NAN));
    // Message actions never read X/Z, so only the finite pre-check stops serde from writing `null`.
    push("nan message x", &|d| d.events[0].actions[0].x = f32::NAN);
    push("inf message z", &|d| d.events[2].actions[0].z = f32::NEG_INFINITY);
    push("dangling enabled target", &|d| d.events[0].conditions[0].target_id = uuid::Uuid::from_u128(7));
    push("terminal not last", &|d| d.events[2].actions.reverse());
    push("nil spawn id", &|d| d.spawns[2].id = uuid::Uuid::nil());
    push("duplicate spawn id", &|d| d.spawns[2].id = d.spawns[0].id);
    push("unbound prebuilt target", &|d| {
        d.data_slots.remove(0);
    });
    push("blank event name", &|d| d.events[0].name = "\t".into());
    for (name, doc) in bad {
        let before = normalized_exact(&doc);
        assert!(doc.to_json().is_err(), "{name}");
        assert_eq!(normalized_exact(&doc), before, "{name}: to_json mutated its input");
    }
    // The untouched base still saves, so each failure above is attributable to its own defect.
    assert!(base.to_json().is_ok());
}

/// Like `normalized` but keeps non-finite floats visible (serde_json::to_value turns them into null).
fn normalized_exact(d: &ScenarioDocument) -> String {
    format!("{d:?}")
}

#[test]
fn malformed_input_never_reaches_compile_even_through_the_cli_entry_points() {
    let source = idle();
    for c in corpus().into_iter().filter(|c| !c.rust_load) {
        assert!(ScenarioDocument::from_json(&c.bytes).is_err(), "{}", c.name);
    }
    // Documents that pass load/save but name an alias the catalog lacks must still be refused at compile.
    for name in ["spawn_action_unknown_alias", "script_spawn_unknown_alias"] {
        let doc = document(&corpus().into_iter().find(|c| c.name == name).unwrap()).unwrap();
        assert!(compile_script(&source, &doc, None, Some(&aliases())).is_err(), "{name}");
        assert!(compile_script(&source, &doc, None, None).is_err(), "{name}: no catalog");
    }
}

// ---------------------------------------------------------------- C# differential gate

/// The Rust port validates the whole document on load, while C# `Load` only checks spawn rotation,
/// identities, DATA bindings and condition shape and defers the rest to Save/Apply. Rust refusing earlier
/// is fail-closed. These are the only inputs that C# accepts through Load, Save *and* the Apply compile
/// path although Rust refuses them; every entry must really diverge, so the list cannot go stale.
const CSHARP_ACCEPTS_RUST_REJECTS: &[&str] = &[
    "json_duplicate_key", // System.Text.Json keeps the last duplicate key; serde refuses it
    "move_destination_missing_axis", // C# record ctor defaults the absent Z to 0; Rust demands both axes
    "spawn_count_21",     // C# Inject clamps the squad size to 20; Rust refuses instead of silently changing it
    "spawn_x_out_of_range", // C# Inject clamps X/Z into 0..16383; Rust refuses instead of moving the object
];

/// Structural JSON comparison that ignores number spelling (`8000` vs `8000.0`, `1E+2`) and key order,
/// which are formatting choices of each serializer; returns the path of the first semantic difference.
fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x.as_f64() != y.as_f64()).then(|| format!("{path} ({x} vs {y})")),
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path} (length {} vs {})", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (p, q))| first_difference(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<_> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| match (x.get(k), y.get(k)) {
                (Some(p), Some(q)) => first_difference(p, q, &format!("{path}.{k}")),
                _ => Some(format!("{path}.{k} (present on one side only)")),
            })
        }
        _ => (a != b).then(|| format!("{path} ({a} vs {b})")),
    }
}

/// Frozen C# oracle results for one corpus case.
struct Frozen(Value);
impl Frozen {
    fn verdict(&self, stage: &str) -> Option<String> {
        self.0.get(stage).and_then(Value::as_str).map(str::to_string)
    }
    fn saved(&self) -> Value {
        self.0["saved"].clone()
    }
    fn bci(&self) -> Vec<u8> {
        unhex(self.0["bci_hex"].as_str().unwrap())
    }
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn frozen_oracle() -> std::collections::BTreeMap<String, Frozen> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scenario_roundtrip_oracle.json");
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value.as_object().unwrap().iter().map(|(k, v)| (k.clone(), Frozen(v.clone()))).collect()
}

#[test]
fn csharp_scenario_save_load_differential() {
    let corpus = corpus();
    let oracle = frozen_oracle();
    assert_eq!(
        oracle.keys().cloned().collect::<BTreeSet<_>>(),
        corpus.iter().map(|c| c.name.clone()).collect::<BTreeSet<_>>(),
        "corpus drifted from the frozen C# oracle fixture"
    );
    let (mut stricter_load, mut stricter_save, mut stricter_compile) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let (mut both_saved, mut both_compiled, mut both_rejected) = (0, 0, 0);
    let aliases = aliases();
    for c in &corpus {
        let n = c.name.as_str();
        let cs_load = oracle[n].verdict("load").unwrap_or_else(|| panic!("{n}: no C# load verdict"));
        let rust = ScenarioDocument::from_json(&c.bytes);
        match (cs_load.starts_with("ok"), rust.as_ref()) {
            (false, Ok(_)) => panic!("{n}: C# refused load ({cs_load}) but Rust accepted"),
            (false, Err(_)) => { both_rejected += 1; continue; }
            (true, Err(_)) => { stricter_load.insert(n.to_string()); continue; }
            (true, Ok(_)) => {}
        }
        let doc = rust.unwrap();
        let cs_save = oracle[n].verdict("save").unwrap_or_else(|| panic!("{n}: no C# save verdict"));
        let rust_saved = doc.to_json();
        match (cs_save.starts_with("ok"), rust_saved.as_ref()) {
            (false, Ok(_)) => panic!("{n}: C# refused save ({cs_save}) but Rust saved"),
            (false, Err(_)) => { both_rejected += 1; continue; }
            (true, Err(_)) => { stricter_save.insert(n.to_string()); continue; }
            (true, Ok(bytes)) => {
                let cs_value: Value = oracle[n].saved();
                let rust_value: Value = serde_json::from_slice(bytes).unwrap();
                if let Some(path) = first_difference(&rust_value, &cs_value, "$") {
                    panic!("{n}: saved JSON differs from C# Save at {path}");
                }
                both_saved += 1;
            }
        }
        // C# accepts what Rust wrote, and compiles it to the same bytes as Rust does.
        let reloaded = ScenarioDocument::from_json(rust_saved.as_ref().unwrap()).unwrap();
        let cs_compile = oracle[n].verdict("compile").unwrap_or_else(|| panic!("{n}: no C# compile verdict"));
        let rust_bytes = compile_script(&idle(), &reloaded, None, Some(&aliases));
        match (cs_compile.starts_with("ok"), rust_bytes) {
            (false, Ok(_)) => panic!("{n}: C# refused compile ({cs_compile}) but Rust compiled"),
            (false, Err(_)) => both_rejected += 1,
            (true, Err(_)) => { stricter_compile.insert(n.to_string()); }
            (true, Ok(bytes)) => {
                assert_eq!(bytes, oracle[n].bci(), "{n}: compiled BCI differs from C#");
                both_compiled += 1;
            }
        }
    }
    // Rust refuses at load what C# only refuses at Save/Apply. That is fail-closed only if C# really does
    // refuse it one stage later; the few inputs C# silently accepts (clamping, last-key-wins) are listed.
    let lenient_in_csharp: BTreeSet<String> = stricter_load
        .iter()
        .filter(|n| {
            let later_stage_rejects = ["save", "compile"]
                .iter()
                .any(|stage| oracle[n.as_str()].verdict(stage).is_some_and(|v| v.starts_with("err")));
            !later_stage_rejects
        })
        .cloned()
        .collect();
    let expected: BTreeSet<String> = CSHARP_ACCEPTS_RUST_REJECTS.iter().map(|s| s.to_string()).collect();
    println!(
        "C# parity: saved={both_saved} compiled={both_compiled} jointly-rejected={both_rejected} of {} cases\n\
         rust stricter at load: {stricter_load:?}\nrust stricter at save: {stricter_save:?}\nrust stricter at compile: {stricter_compile:?}",
        corpus.len()
    );
    assert_eq!(lenient_in_csharp, expected, "C# leniency drifted from the documented list");
    assert!(both_saved >= 12 && both_compiled >= 8, "oracle covered too little");
}
