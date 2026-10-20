use super::catalog::ObjectiveRuleCatalog;
use super::graph::{ObjectiveDependencyGraph, ObjectiveDiagnostic, ObjectiveDiagnosticSeverity};
use super::models::*;
use against_rome_scenario::{Action, BciImage, Condition, Event, ScenarioDocument};
use serde::{Deserialize, Serialize};

/// Validated ScenarioEvents; bci_bytes is populated only by compile_to_bci.
#[derive(Debug, Clone)]
pub struct BciObjectiveCompilationResult {
    pub success: bool,
    pub compiled_events: Vec<Event>,
    pub diagnostics: Vec<ObjectiveDiagnostic>,
    pub summary: String,
    pub bci_bytes: Option<Vec<u8>>,
}

/// Conservative lowering to the existing ScenarioEvent compiler. No objective-specific bytecode.
pub struct BciObjectiveCompiler;

impl BciObjectiveCompiler {
    pub fn compile(
        graph: &ObjectiveDependencyGraph,
        scenario: Option<&ScenarioDocument>,
        known_aliases: Option<&[String]>,
    ) -> BciObjectiveCompilationResult {
        let mut diagnostics = graph.validate();

        if graph
            .objectives()
            .iter()
            .filter(|o| o.category == ObjectiveCategory::Primary)
            .count()
            != 1
        {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_PRIMARY_LIMIT",
                "Export requires exactly one primary objective; completion latches for multiple primaries are unavailable.",
            ));
        }

        if !graph.dependencies().is_empty() {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_DEPENDENCY_UNSUPPORTED",
                "Prerequisites, AND/OR parents and exclusive branches are simulation-only; no event completion primitive is exposed.",
            ));
        }

        for obj in graph.objectives() {
            for message in ObjectiveRuleCatalog::validate_for_compilation(obj).errors {
                diagnostics.push(ObjectiveDiagnostic::with_id(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_EXPORT_UNSUPPORTED",
                    message,
                    obj.id,
                ));
            }
        }

        if diagnostics
            .iter()
            .any(|d| d.severity == ObjectiveDiagnosticSeverity::Error)
        {
            return Self::failure(diagnostics);
        }

        let mut failures = vec![];
        let mut completions = vec![];

        for obj in graph.objectives() {
            if obj.category == ObjectiveCategory::FailureCriterion
                || matches!(
                    obj.kind,
                    ObjectiveKind::Survival | ObjectiveKind::EscortUnit
                )
            {
                // Defeat must precede any possible victory, including a corpse still inside the arrival area.
                let failed = Event {
                    name: format!("OBJ_FAIL_{:x}", obj.id.as_simple()),
                    delay_seconds: 0,
                    repeat: false,
                    enabled: true,
                    actions: vec![
                        Action {
                            kind: 0,
                            text: format!("Objective failed: {}", obj.title),
                            ..Default::default()
                        },
                        Action {
                            kind: 4, // Defeat
                            ..Default::default()
                        },
                    ],
                    conditions: vec![Condition {
                        kind: 1, // ObjectDeadOrRemoved
                        target_id: obj.parameters.target_guids[0],
                        ..Default::default()
                    }],
                };
                failures.push(failed);
                if obj.category == ObjectiveCategory::FailureCriterion {
                    continue;
                }
            }

            let mut completed = Event {
                name: format!("OBJ_WIN_{:x}", obj.id.as_simple()),
                delay_seconds: if obj.kind == ObjectiveKind::Survival {
                    obj.parameters.hold_duration_seconds
                } else {
                    0
                },
                repeat: false,
                enabled: true,
                actions: vec![],
                conditions: vec![],
            };

            match obj.kind {
                ObjectiveKind::DestroyBuilding | ObjectiveKind::AssassinateTarget => {
                    for &id in &obj.parameters.target_guids {
                        completed.conditions.push(Condition {
                            kind: 1, // ObjectDeadOrRemoved
                            target_id: id,
                            ..Default::default()
                        });
                    }
                }
                ObjectiveKind::Survival => {
                    completed.conditions.push(Condition {
                        kind: 0, // ObjectExists
                        target_id: obj.parameters.target_guids[0],
                        ..Default::default()
                    });
                }
                ObjectiveKind::CaptureArea | ObjectiveKind::EscortUnit => {
                    let area = obj.parameters.area.as_ref().unwrap();
                    completed.conditions.push(Condition {
                        kind: 2, // ObjectInArea
                        target_id: obj.parameters.target_guids[0],
                        min_x: area.min_x,
                        min_z: area.min_z,
                        max_x: area.max_x,
                        max_z: area.max_z,
                    });
                }
                ObjectiveKind::CustomScripted => {
                    completed
                        .conditions
                        .extend(obj.parameters.custom_conditions.clone());
                }
                _ => {
                    diagnostics.push(ObjectiveDiagnostic::new(
                        ObjectiveDiagnosticSeverity::Error,
                        "OBJ_UNSUPPORTED_KIND",
                        "Export validation must reject unsupported objective kinds.",
                    ));
                    return Self::failure(diagnostics);
                }
            }

            completed.actions.push(Action {
                kind: 0,
                text: format!("Objective completed: {}", obj.title),
                ..Default::default()
            });

            if let Some(ref reward) = obj.reward {
                if !reward.completion_message.trim().is_empty() {
                    completed.actions.push(Action {
                        kind: 0,
                        text: reward.completion_message.clone(),
                        ..Default::default()
                    });
                }
                completed.actions.extend(reward.extra_actions.clone());
            }

            if obj.category == ObjectiveCategory::Primary {
                completed.actions.push(Action {
                    kind: 3, // Victory
                    ..Default::default()
                });
            }

            completions.push(completed);
        }

        // Run bonus completions before the primary terminal event; failure always has priority.
        completions.sort_by_key(|e| e.actions.iter().any(|a| a.kind == 3) as i32);
        let mut events = failures;
        events.extend(completions);

        // Validation against scenario rules
        let mut check_doc = scenario.cloned().unwrap_or_default();
        check_doc.events = events.clone();
        if let Err(e) = check_doc.validate(known_aliases) {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_SCENARIO_INVALID",
                e,
            ));
            return Self::failure(diagnostics);
        }

        if scenario.is_none() {
            diagnostics.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Warning,
                "OBJ_BINDING_UNCHECKED",
                "Event structure validated; target bindings require a ScenarioDocument before BCI generation.",
            ));
        }

        diagnostics.push(ObjectiveDiagnostic::new(
            ObjectiveDiagnosticSeverity::Info,
            "OBJ_GAMEPLAY_UNVERIFIED",
            "Uses existing ScenarioEvent primitives; this objective composition has not been tested in game.",
        ));

        BciObjectiveCompilationResult {
            success: true,
            summary: format!(
                "Compiled {} objectives to {} validated ScenarioEvents.",
                graph.objectives().len(),
                events.len()
            ),
            compiled_events: events,
            diagnostics,
            bci_bytes: None,
        }
    }

    /// Compile a pristine raw BCI0 or PFIL original in memory.
    pub fn compile_to_bci(
        graph: &ObjectiveDependencyGraph,
        original: &[u8],
        scenario: &ScenarioDocument,
        known_aliases: &[String],
    ) -> BciObjectiveCompilationResult {
        let result = Self::compile(graph, Some(scenario), Some(known_aliases));
        if !result.success {
            return result;
        }

        let mut scenario_copy = scenario.clone();
        scenario_copy.events.extend(result.compiled_events.clone());

        if let Err(e) = scenario_copy.validate(Some(known_aliases)) {
            let mut diags = result.diagnostics;
            diags.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_SCENARIO_INVALID",
                e,
            ));
            return Self::failure(diags);
        }

        for spawn in scenario_copy.spawns.iter().filter(|s| !s.prebuilt) {
            if !known_aliases
                .iter()
                .any(|a| crate::ai::dotnet_unicode::ordinal_ignore_case_eq(a, &spawn.alias))
            {
                let mut diags = result.diagnostics;
                diags.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_BCI_INVALID",
                    format!("Unknown spawn alias: {}", spawn.alias),
                ));
                return Self::failure(diags);
            }
        }

        let is_pfil = original.starts_with(b"PFIL");
        let raw_bytes = if is_pfil {
            match against_rome_data::lzss::decompress_pfil(original, None) {
                Ok(b) => b,
                Err(e) => {
                    let mut diags = result.diagnostics;
                    diags.push(ObjectiveDiagnostic::new(
                        ObjectiveDiagnosticSeverity::Error,
                        "OBJ_BCI_INVALID",
                        e.to_string(),
                    ));
                    return Self::failure(diags);
                }
            }
        } else {
            original.to_vec()
        };

        let mut image = match BciImage::parse(&raw_bytes) {
            Ok(img) => img,
            Err(e) => {
                let mut diags = result.diagnostics;
                diags.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_BCI_INVALID",
                    e,
                ));
                return Self::failure(diags);
            }
        };

        let original_main = image.main;
        let script_spawns: Vec<_> = scenario_copy
            .spawns
            .iter()
            .filter(|s| !s.prebuilt)
            .cloned()
            .collect();

        if !script_spawns.is_empty() {
            if let Err(e) = against_rome_scenario::inject_spawns(&mut image, &script_spawns) {
                let mut diags = result.diagnostics;
                diags.push(ObjectiveDiagnostic::new(
                    ObjectiveDiagnosticSeverity::Error,
                    "OBJ_BCI_INVALID",
                    e,
                ));
                return Self::failure(diags);
            }
        }

        if let Err(e) = against_rome_scenario::inject_events(&mut image, &scenario_copy, original_main) {
            let mut diags = result.diagnostics;
            diags.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_BCI_INVALID",
                e,
            ));
            return Self::failure(diags);
        }

        let serialized = image.serialize();
        if BciImage::parse(&serialized).map(|img| img.serialize()) != Ok(serialized.clone()) {
            let mut diags = result.diagnostics;
            diags.push(ObjectiveDiagnostic::new(
                ObjectiveDiagnosticSeverity::Error,
                "OBJ_BCI_INVALID",
                "BCI round trip failed.",
            ));
            return Self::failure(diags);
        }

        let output_bytes = if is_pfil {
            match Self::pack_pfil(&serialized, original) {
                Ok(bytes) => bytes,
                Err(err) => {
                    let mut diags = result.diagnostics;
                    diags.push(ObjectiveDiagnostic::new(
                        ObjectiveDiagnosticSeverity::Error,
                        "OBJ_BCI_INVALID",
                        &err,
                    ));
                    return Self::failure(diags);
                }
            }
        } else {
            serialized
        };

        let mut res = result;
        res.bci_bytes = Some(output_bytes);
        res
    }

    /// 以 Literal-only LZSS 將已序列化的 BCI 位元碼打包回 PFIL 格式。
    fn pack_pfil(raw: &[u8], template_pfil: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = template_pfil
            .get(..64)
            .ok_or_else(|| "截斷的 PFIL 標頭".to_string())?
            .to_vec();
        out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
        for chunk in raw.chunks(8) {
            out.push(((1u16 << chunk.len()) - 1) as u8);
            out.extend_from_slice(chunk);
        }
        Ok(out)
    }

    fn failure(diagnostics: Vec<ObjectiveDiagnostic>) -> BciObjectiveCompilationResult {
        BciObjectiveCompilationResult {
            success: false,
            compiled_events: vec![],
            diagnostics,
            summary: "Objective export rejected; no events or BCI emitted.".to_string(),
            bci_bytes: None,
        }
    }

    // Serialization & Lossless Metadata

    pub fn serialize_graph(graph: &ObjectiveDependencyGraph) -> Result<String, String> {
        let dto = ObjectiveGraphDto {
            objectives: graph.objectives().into_iter().cloned().collect(),
            dependencies: graph.dependencies().to_vec(),
        };
        serde_json::to_string_pretty(&dto).map_err(|e| e.to_string())
    }

    pub fn deserialize_graph(json: &str) -> Result<ObjectiveDependencyGraph, String> {
        let dto: ObjectiveGraphDto =
            serde_json::from_str(json).map_err(|e| format!("無法解析目標依賴圖資料：{e}"))?;

        let mut graph = ObjectiveDependencyGraph::new();
        for obj in dto.objectives {
            graph.add_objective(obj)?;
        }
        for dep in dto.dependencies {
            graph.add_dependency(dep.source_objective_id, dep.target_objective_id, dep.relation);
        }
        Ok(graph)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ObjectiveGraphDto {
    objectives: Vec<ObjectiveDefinition>,
    dependencies: Vec<ObjectiveDependency>,
}
