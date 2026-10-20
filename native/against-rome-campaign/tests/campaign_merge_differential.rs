//! Differential test for the campaign wave merge: the Rust `CampaignMissionCompiler::compile_for_merge`
//! versus the real C# path (`CampaignWaveDialog.CompileForMerge` + `MapEditorForm` guard / `UniqueEventNames` /
//! 256-event cap), driven by the same fixed cases in `tests/fixtures/campaign_merge_cases.json`.
//!
//! What is real vs replicated on the C# side (oracle project since removed; output frozen in the fixture): `CompileForMerge` and
//! `MapEditorForm.UniqueEventNames` are the real C# methods (reflection); the `ApplyCampaignWaves` unsupported-field guard
//! and the `MergeScenarioEvents` 256-event cap are replicas pinned to the C# source text by SHA-256 (`sourcePins`).
//!
//! * `committed_oracle_output_matches_rust` compares against the committed C# output
//!   (`campaign_merge_oracle_expected.json`), no .NET needed.
//!
//! Event JSON is compared structurally with an order-preserving parser: same keys in the same order,
//! same array order, strings exact, numbers by value (C# writes `8000`, serde_json writes `8000.0` for f32).

use against_rome_campaign::ai::campaign::{
    CampaignMissionPlan, CampaignObjective, CampaignObjectiveType, ReinforcementDefinition,
    WaveAttackDefinition, WaveSquadDefinition,
};
use against_rome_campaign::ai::archetype::FactionAiProfile;
use against_rome_campaign::ai::compiler::CampaignMissionCompiler;
use against_rome_scenario::{Action, Event, ScenarioDocument, Spawn, WorldPosition};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

// ---------------------------------------------------------------- order-preserving JSON

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn expect(&mut self, c: u8) {
        self.ws();
        assert_eq!(self.s[self.i], c, "json: expected {:?} at {}", c as char, self.i);
        self.i += 1;
    }
    fn value(&mut self) -> Json {
        self.ws();
        match self.s[self.i] {
            b'n' => self.lit("null", Json::Null),
            b't' => self.lit("true", Json::Bool(true)),
            b'f' => self.lit("false", Json::Bool(false)),
            b'"' => Json::Str(self.string()),
            b'[' => {
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.s[self.i] == b']' {
                    self.i += 1;
                    return Json::Arr(v);
                }
                loop {
                    v.push(self.value());
                    self.ws();
                    match self.s[self.i] {
                        b',' => self.i += 1,
                        b']' => {
                            self.i += 1;
                            return Json::Arr(v);
                        }
                        c => panic!("json: bad array separator {:?}", c as char),
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.s[self.i] == b'}' {
                    self.i += 1;
                    return Json::Obj(v);
                }
                loop {
                    self.ws();
                    let k = self.string();
                    self.expect(b':');
                    v.push((k, self.value()));
                    self.ws();
                    match self.s[self.i] {
                        b',' => self.i += 1,
                        b'}' => {
                            self.i += 1;
                            return Json::Obj(v);
                        }
                        c => panic!("json: bad object separator {:?}", c as char),
                    }
                }
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                Json::Num(std::str::from_utf8(&self.s[st..self.i]).unwrap().parse().expect("json number"))
            }
        }
    }
    fn lit(&mut self, word: &str, v: Json) -> Json {
        assert!(self.s[self.i..].starts_with(word.as_bytes()));
        self.i += word.len();
        v
    }
    fn hex4(&mut self) -> u16 {
        let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).unwrap();
        self.i += 4;
        u16::from_str_radix(h, 16).unwrap()
    }
    fn string(&mut self) -> String {
        assert_eq!(self.s[self.i], b'"');
        self.i += 1;
        let mut units: Vec<u16> = vec![];
        loop {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf16(&units).expect("json: lone surrogate"),
                b'\\' => {
                    let e = self.s[self.i];
                    self.i += 1;
                    match e {
                        b'"' => units.push(b'"' as u16),
                        b'\\' => units.push(b'\\' as u16),
                        b'/' => units.push(b'/' as u16),
                        b'b' => units.push(8),
                        b'f' => units.push(12),
                        b'n' => units.push(10),
                        b'r' => units.push(13),
                        b't' => units.push(9),
                        b'u' => units.push(self.hex4()),
                        _ => panic!("json: bad escape"),
                    }
                }
                _ => {
                    // copy one UTF-8 scalar
                    let st = self.i - 1;
                    let len = match c {
                        0..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    self.i = st + len;
                    units.extend(std::str::from_utf8(&self.s[st..st + len]).unwrap().encode_utf16());
                }
            }
        }
    }
}

fn parse(text: &str) -> Json {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    let v = p.value();
    p.ws();
    assert_eq!(p.i, p.s.len(), "json: trailing data");
    v
}

/// First difference as a path, or `None` when equal (key order, array order, strings exact, numbers by value).
fn diff(path: &str, a: &Json, b: &Json) -> Option<String> {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => {
            let kx: Vec<_> = x.iter().map(|(k, _)| k.as_str()).collect();
            let ky: Vec<_> = y.iter().map(|(k, _)| k.as_str()).collect();
            if kx != ky {
                return Some(format!("{path}: key order/set differs: {kx:?} vs {ky:?}"));
            }
            x.iter().zip(y).find_map(|((k, va), (_, vb))| diff(&format!("{path}.{k}"), va, vb))
        }
        (Json::Arr(x), Json::Arr(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: array length {} vs {}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (va, vb))| diff(&format!("{path}[{i}]"), va, vb))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: {a:?} vs {b:?}")),
    }
}

// ---------------------------------------------------------------- fixture -> Rust plan

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn guid(v: &Value) -> Uuid {
    Uuid::parse_str(v.as_str().unwrap()).unwrap()
}
fn opt_guid(v: &Value, key: &str) -> Option<Uuid> {
    v.get(key).map(guid)
}
fn int(v: &Value, key: &str, default: i32) -> i32 {
    v.get(key).map_or(default, |x| x.as_i64().unwrap() as i32)
}
fn float(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap() as f32
}
fn text(v: &Value, key: &str) -> String {
    v.get(key).map_or(String::new(), |x| x.as_str().unwrap().to_string())
}

fn squads(v: &Value) -> Vec<WaveSquadDefinition> {
    v["squads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let mut sq = WaveSquadDefinition::new(s["alias"].as_str().unwrap(), int(s, "count", 0), int(s, "team", 0));
            sq.relative_delay_seconds = int(s, "relativeDelay", 0);
            sq
        })
        .collect()
}

fn build_plan(p: &Value) -> CampaignMissionPlan {
    let mut plan = CampaignMissionPlan::new(p["title"].as_str().unwrap(), text(p, "briefing"));
    for o in p.get("objectives").and_then(Value::as_array).into_iter().flatten() {
        let ty = match o["type"].as_str().unwrap() {
            "SurviveTime" => CampaignObjectiveType::SurviveTime,
            "DefendTarget" => CampaignObjectiveType::DefendTarget,
            "DestroyTarget" => CampaignObjectiveType::DestroyTarget,
            "WaveSurvival" => CampaignObjectiveType::WaveSurvival,
            "ReachArea" => CampaignObjectiveType::ReachArea,
            other => panic!("unknown objective type {other}"),
        };
        let mut obj = CampaignObjective::new(guid(&o["id"]), ty, o["title"].as_str().unwrap(), text(o, "description"));
        obj.target_id = opt_guid(o, "targetId");
        obj.target_min_x = int(o, "minX", 0);
        obj.target_min_z = int(o, "minZ", 0);
        obj.target_max_x = int(o, "maxX", 16383);
        obj.target_max_z = int(o, "maxZ", 16383);
        obj.required_seconds = int(o, "requiredSeconds", 0);
        plan.objectives.push(obj);
    }
    for w in p.get("waves").and_then(Value::as_array).into_iter().flatten() {
        let mut wave = WaveAttackDefinition::new(
            int(w, "index", 0),
            int(w, "delay", 0),
            text(w, "announcement"),
            float(w, "x"),
            float(w, "z"),
            squads(w),
        );
        wave.assigned_path_id = opt_guid(w, "pathId");
        wave.target_object_id = opt_guid(w, "targetObjectId");
        wave.move_destination = w.get("moveDestination").map(|d| WorldPosition {
            x: int(d, "x", 0),
            z: int(d, "z", 0),
        });
        plan.waves.push(wave);
    }
    for r in p.get("reinforcements").and_then(Value::as_array).into_iter().flatten() {
        let mut re = ReinforcementDefinition::new(
            guid(&r["id"]),
            r["name"].as_str().unwrap(),
            int(r, "delay", 0),
            int(r, "team", 0),
            float(r, "x"),
            float(r, "z"),
            squads(r),
        );
        re.notification_text = text(r, "notification");
        re.assigned_path_id = opt_guid(r, "pathId");
        plan.reinforcements.push(re);
    }
    for f in p.get("factionProfiles").and_then(Value::as_array).into_iter().flatten() {
        let mut profile = FactionAiProfile::new(int(f, "team", 0), f["archetypeId"].as_str().unwrap());
        profile.enabled = f.get("enabled").map_or(true, |e| e.as_bool().unwrap());
        plan.faction_profiles.push(profile);
    }
    plan
}

struct RustOutcome {
    success: bool,
    diagnostics: Vec<String>,
    events: Vec<Event>,
}

/// Rust side of `ApplyCampaignWaves` for one case: `compile_for_merge` (which carries the unsupported-field
/// guard, event-name de-duplication and 256-event cap).
fn run_rust(case: &Value) -> RustOutcome {
    let aliases: Vec<String> = case["aliases"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
    let placeholder = |name: String| Event {
        name,
        actions: vec![Action { kind: 0, text: "existing".into(), ..Default::default() }],
        ..Default::default()
    };
    let mut scenario = ScenarioDocument::default();
    for n in case.get("existingEventNames").and_then(Value::as_array).into_iter().flatten() {
        scenario.events.push(placeholder(n.as_str().unwrap().to_string()));
    }
    for i in 0..int(case, "padExistingEvents", 0) {
        scenario.events.push(placeholder(format!("Pad_{i}")));
    }
    let spawn_alias = case.get("spawnAlias").map_or_else(|| aliases[0].clone(), |a| a.as_str().unwrap().to_string());
    for id in case.get("spawnIds").and_then(Value::as_array).into_iter().flatten() {
        scenario.spawns.push(Spawn { alias: spawn_alias.clone(), x: 100.0, z: 100.0, id: guid(id), ..Default::default() });
    }
    let plan = build_plan(&case["plan"]);
    let r = CampaignMissionCompiler::compile_for_merge(&plan, &aliases, &scenario);
    RustOutcome {
        success: r.success,
        diagnostics: r.diagnostics,
        events: if r.success { r.compiled_events } else { vec![] },
    }
}

fn to_json(o: &RustOutcome, name: &str) -> Json {
    let events = serde_json::to_string(&o.events).unwrap();
    Json::Obj(vec![
        ("name".into(), Json::Str(name.into())),
        ("success".into(), Json::Bool(o.success)),
        ("diagnostics".into(), Json::Arr(o.diagnostics.iter().cloned().map(Json::Str).collect())),
        ("events".into(), parse(&events)),
    ])
}

/// Compares one case. Event lists must be identical (order, keys, values); diagnostics only when the case opts in
/// (C# exception messages for some failures are .NET-specific text).
fn compare(case: &Value, oracle: &Json, label: &str) -> String {
    let name = case["name"].as_str().unwrap();
    let compare_diag = case["compareDiagnostics"].as_bool().unwrap();
    let rust = to_json(&run_rust(case), name);
    let (Json::Obj(r), Json::Obj(o)) = (&rust, oracle) else { panic!("not objects") };
    if case.get("expectGuardRejection").and_then(Value::as_bool) == Some(true) {
        // The C# ApplyCampaignWaves guard rejected this case (a replica in the oracle, pinned by sourcePins); Rust must too.
        let Json::Obj(ov) = oracle else { panic!("not objects") };
        assert_eq!(ov.iter().find(|(k, _)| k == "guardRejected").unwrap().1, Json::Bool(true), "{name}: oracle guard should reject");
    }
    let mut fields = 0;
    for key in ["name", "success", "events"].into_iter().chain(compare_diag.then_some("diagnostics")) {
        let rv = &r.iter().find(|(k, _)| k == key).unwrap().1;
        let ov = &o.iter().find(|(k, _)| k == key).unwrap().1;
        if let Some(d) = diff(&format!("{name}.{key}"), rv, ov) {
            panic!("[{label}] Rust != C# oracle: {d}
rust diagnostics: {:?}", run_rust(case).diagnostics);
        }
        fields += 1;
    }
    format!("{name}: success={} events={} compared_fields={fields}", matches!(r[1].1, Json::Bool(true)), match &r[3].1 { Json::Arr(a) => a.len(), _ => 0 })
}

fn load_cases() -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_cases.json")).unwrap()).unwrap()
}

fn oracle_results(text: &str) -> Vec<Json> {
    let Json::Obj(top) = parse(text) else { panic!("oracle output is not an object") };
    let Json::Arr(results) = top.into_iter().find(|(k, _)| k == "results").unwrap().1 else { panic!("results") };
    results
}

fn compare_all(oracle_text: &str, label: &str) -> usize {
    let cases = load_cases();
    let cases = cases["cases"].as_array().unwrap();
    let results = oracle_results(oracle_text);
    assert_eq!(cases.len(), results.len(), "case count differs");
    for (case, res) in cases.iter().zip(&results) {
        if case.get("expectRustDivergence").and_then(Value::as_bool) == Some(true) {
            // Documented gap: pin the current behaviour so it cannot change silently.
            let name = case["name"].as_str().unwrap();
            let Json::Obj(o) = res else { panic!("not an object") };
            assert_eq!(o.iter().find(|(k, _)| k == "success").unwrap().1, Json::Bool(true), "{name}: C# should merge");
            let rust = run_rust(case);
            assert!(!rust.success, "{name}: divergence closed - move this case to a normal comparison");
            println!("[{label}] {name}: KNOWN DIVERGENCE C# success=true, Rust success=false ({:?})", rust.diagnostics);
            continue;
        }
        println!("[{label}] {}", compare(case, res, label));
    }
    cases.len()
}

// ---------------------------------------------------------------- tests

const CASE_COUNT: usize = 27;

/// Every case in the fixture that sets `expectGuardRejection` must be a guard rejection in the oracle output, and every
/// oracle `guardRejected` row must be flagged in the fixture (so the guard replica cannot drift unnoticed in either direction).
#[test]
fn guard_rejection_cases_match_oracle_guard_flag() {
    let text = std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_oracle_expected.json")).unwrap();
    let cases = load_cases();
    let results = oracle_results(&text);
    let mut guarded = 0;
    for (case, res) in cases["cases"].as_array().unwrap().iter().zip(&results) {
        let Json::Obj(o) = res else { panic!() };
        let flag = o.iter().find(|(k, _)| k == "guardRejected").map(|(_, v)| v == &Json::Bool(true)).unwrap();
        let expected = case.get("expectGuardRejection").and_then(Value::as_bool) == Some(true);
        assert_eq!(flag, expected, "{}: fixture expectGuardRejection vs oracle guardRejected", case["name"]);
        if flag {
            guarded += 1;
            assert!(!run_rust(case).success, "{}: Rust must reject what the C# guard rejects", case["name"]);
        }
    }
    assert!(guarded >= 10, "guard coverage shrank ({guarded})");
}

/// The faction_profiles-only plan is rejected by the guard alone (real C# `CompileForMerge` accepts it), proving the guard
/// - not the compiler - is what Rust must replicate for that field.
#[test]
fn faction_profiles_are_rejected_only_by_the_apply_guard() {
    let text = std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_oracle_expected.json")).unwrap();
    let rows = oracle_results(&text);
    for name in ["faction_profiles_alone_is_rejected", "faction_profiles_disabled_entry_still_rejected", "faction_profiles_with_valid_wave_is_rejected"] {
        let Json::Obj(o) = rows.iter().find(|r| matches!(r, Json::Obj(o) if o.iter().any(|(k, v)| k == "name" && *v == Json::Str(name.into())))).unwrap() else { panic!() };
        assert_eq!(o.iter().find(|(k, _)| k == "helperSucceedsWithoutGuard").unwrap().1, Json::Bool(true), "{name}");
    }
}

/// The oracle pins the C# fragments it replicates; the committed file must carry all seven pins (a missing/renamed pin
/// means the oracle was edited without re-auditing the Rust port).
#[test]
fn committed_oracle_records_source_pins() {
    let text = std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_oracle_expected.json")).unwrap();
    let Json::Obj(top) = parse(&text) else { panic!() };
    let Json::Arr(pins) = &top.iter().find(|(k, _)| k == "sourcePins").expect("sourcePins").1 else { panic!() };
    let names: Vec<_> = pins
        .iter()
        .map(|p| match p {
            Json::Obj(o) => match &o.iter().find(|(k, _)| k == "name").unwrap().1 { Json::Str(s) => s.clone(), _ => panic!() },
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        names,
        [
            "apply_campaign_waves_preconditions",
            "apply_campaign_waves_guard",
            "apply_campaign_waves_flow",
            "merge_scenario_events_cap",
            "can_merge_scenario_events",
            "unique_event_names_reads_session_only",
            "compile_for_merge_body"
        ]
    );
}

#[test]
fn committed_oracle_output_matches_rust() {
    let text = std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_oracle_expected.json")).unwrap();
    assert_eq!(compare_all(&text, "committed"), CASE_COUNT);
}

#[test]
fn committed_oracle_covers_success_and_failure_paths() {
    let text = std::fs::read_to_string(root().join("tests/fixtures/campaign_merge_oracle_expected.json")).unwrap();
    let ok = oracle_results(&text)
        .iter()
        .filter(|r| matches!(r, Json::Obj(o) if o.iter().any(|(k, v)| k == "success" && *v == Json::Bool(true))))
        .count();
    assert!(ok >= 7 && ok < CASE_COUNT, "fixture must exercise both successful merges and rejections (ok={ok})");
}
