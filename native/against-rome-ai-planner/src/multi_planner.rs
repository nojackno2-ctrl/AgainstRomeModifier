//! MultiAiMapPlanner: orchestrates multiple AI model roles (Terrain, Water, Materials).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serde_json::Value;
use crate::ollama::AiMaterialOption;
use crate::plan::{AiMapFeature, AiMapPlan, MAX_FEATURES};

pub const FEATURES_PER_ROLE: usize = MAX_FEATURES / 3; // 8

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiMapDesignRole {
    Terrain,
    Water,
    Materials,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMapRoleStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiMapRoleProgress {
    pub role: AiMapDesignRole,
    pub model: String,
    pub completed_roles: usize,
    pub finished: bool,
    pub status: Option<AiMapRoleStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiAiMapRoleRequest {
    pub role: AiMapDesignRole,
    pub model: String,
}

impl MultiAiMapRoleRequest {
    pub fn new(role: AiMapDesignRole, model: impl Into<String>) -> Self {
        Self {
            role,
            model: model.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiMapRoleResult {
    pub role: AiMapDesignRole,
    pub model: String,
    pub status: AiMapRoleStatus,
    pub accepted_features: usize,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub raw_response: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiAiMapPlanResult {
    pub plan: Option<AiMapPlan>,
    pub roles: Vec<AiMapRoleResult>,
    pub is_cancelled: bool,
}

impl MultiAiMapPlanResult {
    pub fn has_failures(&self) -> bool {
        self.roles
            .iter()
            .any(|r| r.status != AiMapRoleStatus::Succeeded)
    }

    pub fn warnings(&self) -> Vec<String> {
        self.roles.iter().flat_map(|r| r.warnings.clone()).collect()
    }

    pub fn raw_response(&self) -> String {
        self.roles
            .iter()
            .map(|r| {
                format!(
                    "[{} / {} / {:?}]\n{}",
                    MultiAiMapPlanner::role_name(r.role),
                    r.model,
                    r.status,
                    r.error
                        .as_deref()
                        .or(r.raw_response.as_deref())
                        .unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Cooperative cancellation flag shared between the UI thread and a generation worker
/// (the Rust counterpart of C# `CancellationToken`). Clones observe the same flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub type GenerateRolePlanFn = Arc<
    dyn Fn(&str, &str, &[AiMaterialOption], f32) -> Result<(AiMapPlan, String), String>
        + Send
        + Sync,
>;

pub struct MultiAiMapPlanner {
    generate: GenerateRolePlanFn,
}

impl MultiAiMapPlanner {
    pub const ROLE_ORDER: &'static [AiMapDesignRole] = &[
        AiMapDesignRole::Terrain,
        AiMapDesignRole::Water,
        AiMapDesignRole::Materials,
    ];

    pub fn new(generate: GenerateRolePlanFn) -> Self {
        Self { generate }
    }

    pub fn role_name(role: AiMapDesignRole) -> &'static str {
        match role {
            AiMapDesignRole::Terrain => "地形",
            AiMapDesignRole::Water => "水系",
            AiMapDesignRole::Materials => "材質",
        }
    }

    pub fn build_role_prompt(role: AiMapDesignRole, description: &str) -> String {
        let scope = match role {
            AiMapDesignRole::Terrain => "Design ONLY hill, mountain, valley, plateau, ridge, blocked and passable features. You alone may set baseHeight. Do not set baseMaterial or any material field. Do not design rivers or lakes.",
            AiMapDesignRole::Water => "Design ONLY lake and river features. Do not set baseHeight, baseMaterial or any material field. Preserve dry settlement space.",
            AiMapDesignRole::Materials => "Design ONLY material features. You alone may set baseMaterial. Do not set baseHeight or create height, water or collision features. Use materials available in the schema.",
        };
        format!(
            "You are the {:?} specialist in a concurrent map design team. Other specialists work from the SAME map request and compass locations. {}\nReturn at most {} features. Your role restrictions override general suggestions about feature count or attached materials.\nMap request:\n{}",
            role,
            scope,
            FEATURES_PER_ROLE,
            description.trim()
        )
    }

    pub fn generate_plan_single_model(
        &self,
        model: &str,
        description: &str,
        materials: &[AiMaterialOption],
        water_level_sample: f32,
    ) -> Result<MultiAiMapPlanResult, String> {
        if model.trim().is_empty() {
            return Err("請選擇 AI 模型。".to_string());
        }
        let requests: Vec<MultiAiMapRoleRequest> = Self::ROLE_ORDER
            .iter()
            .map(|&role| MultiAiMapRoleRequest::new(role, model))
            .collect();

        self.generate_plan(&requests, description, materials, water_level_sample, None)
    }

    pub fn generate_plan(
        &self,
        requests: &[MultiAiMapRoleRequest],
        description: &str,
        materials: &[AiMaterialOption],
        water_level_sample: f32,
        progress: Option<&mut dyn FnMut(AiMapRoleProgress)>,
    ) -> Result<MultiAiMapPlanResult, String> {
        self.generate_plan_cancellable(
            requests,
            description,
            materials,
            water_level_sample,
            &CancelToken::new(),
            progress,
        )
    }

    /// Same as [`Self::generate_plan`] but honours `cancel` like the C# `CancellationToken`:
    /// no start progress once cancelled, remaining roles report `Cancelled`, and the result
    /// carries `is_cancelled = true` with no plan.
    pub fn generate_plan_cancellable(
        &self,
        requests: &[MultiAiMapRoleRequest],
        description: &str,
        materials: &[AiMaterialOption],
        water_level_sample: f32,
        cancel: &CancelToken,
        mut progress: Option<&mut dyn FnMut(AiMapRoleProgress)>,
    ) -> Result<MultiAiMapPlanResult, String> {
        if requests.is_empty() {
            return Err("請選擇至少一個角色，每個角色只能指定一個模型。".to_string());
        }

        let mut seen_roles = std::collections::HashSet::new();
        for r in requests {
            if !Self::ROLE_ORDER.contains(&r.role) || !seen_roles.insert(r.role) {
                return Err("請選擇至少一個角色，每個角色只能指定一個模型。".to_string());
            }
            if r.model.trim().is_empty() {
                return Err("每個角色都必須選擇 AI 模型。".to_string());
            }
        }

        if description.trim().is_empty() {
            return Err("請輸入地圖描述。".to_string());
        }

        let mut results = Vec::new();
        let material_ids: Vec<&str> = materials.iter().map(|m| m.id.as_str()).collect();

        for &role in Self::ROLE_ORDER {
            let req = match requests.iter().find(|r| r.role == role) {
                Some(r) => r,
                None => continue,
            };

            if !cancel.is_cancelled() {
                if let Some(cb) = progress.as_deref_mut() {
                    cb(AiMapRoleProgress {
                        role,
                        model: req.model.clone(),
                        completed_roles: results.len(),
                        finished: false,
                        status: None,
                    });
                }
            }

            let role_res = self.generate_role(
                cancel,
                role,
                &req.model,
                description,
                materials,
                &material_ids,
                water_level_sample,
            );

            let status = role_res.1.status;
            results.push(role_res);

            if let Some(cb) = progress.as_deref_mut() {
                cb(AiMapRoleProgress {
                    role,
                    model: req.model.clone(),
                    completed_roles: results.len(),
                    finished: true,
                    status: Some(status),
                });
            }
        }

        let reports: Vec<AiMapRoleResult> = results.iter().map(|(_, rep)| rep.clone()).collect();
        if cancel.is_cancelled() {
            return Ok(MultiAiMapPlanResult {
                plan: None,
                roles: reports,
                is_cancelled: true,
            });
        }
        if results.iter().all(|(plan, _)| plan.is_none()) {
            return Ok(MultiAiMapPlanResult {
                plan: None,
                roles: reports,
                is_cancelled: false,
            });
        }

        let mut merged = AiMapPlan::default();
        for (plan_opt, rep) in &results {
            if let Some(p) = plan_opt {
                if rep.role == AiMapDesignRole::Terrain {
                    merged.base_height = p.base_height;
                }
                if rep.role == AiMapDesignRole::Materials {
                    merged.base_material = p.base_material.clone();
                }
                merged.features.extend(p.features.clone());
            }
        }

        let summaries: Vec<String> = results
            .iter()
            .filter_map(|(p, rep)| {
                p.as_ref().map(|p| {
                    format!(
                        "{}：{}",
                        Self::role_name(rep.role),
                        p.summary.as_deref().unwrap_or("完成")
                    )
                })
            })
            .collect();
        merged.summary = Some(summaries.join("；"));

        let merged_json = serde_json::to_string(&merged).map_err(|e| e.to_string())?;
        let final_plan = AiMapPlan::parse(&merged_json, &material_ids)?;

        Ok(MultiAiMapPlanResult {
            plan: Some(final_plan),
            roles: reports,
            is_cancelled: false,
        })
    }

    fn generate_role(
        &self,
        cancel: &CancelToken,
        role: AiMapDesignRole,
        model: &str,
        description: &str,
        materials: &[AiMaterialOption],
        material_ids: &[&str],
        water: f32,
    ) -> (Option<AiMapPlan>, AiMapRoleResult) {
        let name = Self::role_name(role);
        let mut warnings = Vec::new();
        let prompt = Self::build_role_prompt(role, description);

        let cancelled = |raw: Option<String>, warnings: Vec<String>| {
            (
                None,
                AiMapRoleResult {
                    role,
                    model: model.to_string(),
                    status: AiMapRoleStatus::Cancelled,
                    accepted_features: 0,
                    error: Some("已取消。".to_string()),
                    warnings,
                    raw_response: raw,
                },
            )
        };
        if cancel.is_cancelled() {
            return cancelled(None, warnings);
        }
        let gen_res = (self.generate)(model, &prompt, materials, water);
        if cancel.is_cancelled() {
            // A provider may finish after Cancel; its result must never become applicable.
            return cancelled(gen_res.ok().map(|(_, raw)| raw), warnings);
        }
        let (generated_plan, raw) = match gen_res {
            Ok(pair) => pair,
            Err(e) => {
                let status = if e.contains("timeout") || e.contains("cancel") {
                    AiMapRoleStatus::Cancelled
                } else {
                    AiMapRoleStatus::Failed
                };
                return (
                    None,
                    AiMapRoleResult {
                        role,
                        model: model.to_string(),
                        status,
                        accepted_features: 0,
                        error: Some(e),
                        warnings,
                        raw_response: None,
                    },
                );
            }
        };

        let mut original_count = generated_plan.features.len();
        if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
            if end > start {
                if let Ok(doc) = serde_json::from_str::<Value>(&raw[start..=end]) {
                    if let Some(features) = doc.get("features").and_then(|f| f.as_array()) {
                        original_count = original_count.max(features.len());
                    }
                }
            }
        }

        let plan_json = match serde_json::to_string(&generated_plan) {
            Ok(j) => j,
            Err(e) => {
                return (
                    None,
                    AiMapRoleResult {
                        role,
                        model: model.to_string(),
                        status: AiMapRoleStatus::Failed,
                        accepted_features: 0,
                        error: Some(e.to_string()),
                        warnings,
                        raw_response: Some(raw),
                    },
                );
            }
        };

        let normalized = match AiMapPlan::parse(&plan_json, material_ids) {
            Ok(p) => p,
            Err(e) => {
                return (
                    None,
                    AiMapRoleResult {
                        role,
                        model: model.to_string(),
                        status: AiMapRoleStatus::Failed,
                        accepted_features: 0,
                        error: Some(e),
                        warnings,
                        raw_response: Some(raw),
                    },
                );
            }
        };

        if original_count > normalized.features.len() {
            warnings.push(format!(
                "{}：正規化移除或截斷 {} 個無效、重複或超量特徵。",
                name,
                original_count - normalized.features.len()
            ));
        }

        let allowed: &[&str] = match role {
            AiMapDesignRole::Terrain => &[
                "hill", "mountain", "valley", "plateau", "ridge", "blocked", "passable",
            ],
            AiMapDesignRole::Water => &["lake", "river"],
            AiMapDesignRole::Materials => &["material"],
        };

        let mut scoped: Vec<AiMapFeature> = normalized
            .features
            .into_iter()
            .filter(|f| {
                f.feature_type
                    .as_deref()
                    .map_or(false, |t| allowed.contains(&t))
            })
            .collect();

        let rejected = original_count.max(scoped.len()) - scoped.len();
        if rejected > 0 {
            warnings.push(format!("{}：忽略 {} 個超出角色範圍的特徵。", name, rejected));
        }

        if role != AiMapDesignRole::Terrain && normalized.base_height.is_some() {
            warnings.push(format!("{}：忽略超出角色範圍的 baseHeight。", name));
        }
        if role != AiMapDesignRole::Materials && normalized.base_material.is_some() {
            warnings.push(format!("{}：忽略超出角色範圍的 baseMaterial。", name));
        }

        if role != AiMapDesignRole::Materials {
            let attached = scoped.iter().filter(|f| f.material.is_some()).count();
            if attached > 0 {
                warnings.push(format!(
                    "{}：忽略 {} 個附帶材質，材質由材質角色負責。",
                    name, attached
                ));
                for f in &mut scoped {
                    f.material = None;
                }
            }
        }

        let before_dedup = scoped.len();
        let mut seen = std::collections::HashSet::new();
        scoped.retain(|f| {
            let key = (
                f.feature_type.clone(),
                f.x,
                f.y,
                f.x2,
                f.y2,
                f.radius,
                f.amount,
                f.material.clone(),
            );
            seen.insert(key)
        });

        if scoped.len() < before_dedup {
            warnings.push(format!(
                "{}：移除角色範圍處理後重複的 {} 個特徵。",
                name,
                before_dedup - scoped.len()
            ));
        }

        if scoped.len() > FEATURES_PER_ROLE {
            warnings.push(format!(
                "{}：超過角色上限 {}，截斷 {} 個特徵。",
                name,
                FEATURES_PER_ROLE,
                scoped.len() - FEATURES_PER_ROLE
            ));
            scoped.truncate(FEATURES_PER_ROLE);
        }

        let role_plan = AiMapPlan {
            edit_scope: None,
            summary: normalized.summary,
            base_height: if role == AiMapDesignRole::Terrain {
                normalized.base_height
            } else {
                None
            },
            base_material: if role == AiMapDesignRole::Materials {
                normalized.base_material
            } else {
                None
            },
            features: scoped,
        };

        if role_plan.features.is_empty()
            && role_plan.base_height.is_none()
            && role_plan.base_material.is_none()
        {
            return (
                None,
                AiMapRoleResult {
                    role,
                    model: model.to_string(),
                    status: AiMapRoleStatus::Failed,
                    accepted_features: 0,
                    error: Some("角色未產生任何可套用的範圍內內容。".to_string()),
                    warnings,
                    raw_response: Some(raw),
                },
            );
        }

        let count = role_plan.features.len();
        (
            Some(role_plan),
            AiMapRoleResult {
                role,
                model: model.to_string(),
                status: AiMapRoleStatus::Succeeded,
                accepted_features: count,
                error: None,
                warnings,
                raw_response: Some(raw),
            },
        )
    }
}
