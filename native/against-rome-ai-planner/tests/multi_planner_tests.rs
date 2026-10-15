use std::sync::Arc;
use against_rome_ai_planner::multi_planner::{
    AiMapDesignRole, AiMapRoleProgress, AiMapRoleStatus, MultiAiMapPlanner, MultiAiMapRoleRequest,
    FEATURES_PER_ROLE,
};
use against_rome_ai_planner::ollama::AiMaterialOption;
use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};

fn create_sample_plan(role: AiMapDesignRole) -> AiMapPlan {
    match role {
        AiMapDesignRole::Terrain => AiMapPlan {
            base_height: Some(80),
            features: vec![AiMapFeature {
                feature_type: Some("hill".to_string()),
                location: Some("north".to_string()),
                radius: 4,
                ..Default::default()
            }],
            ..Default::default()
        },
        AiMapDesignRole::Water => AiMapPlan {
            features: vec![AiMapFeature {
                feature_type: Some("lake".to_string()),
                location: Some("south".to_string()),
                radius: 4,
                ..Default::default()
            }],
            ..Default::default()
        },
        AiMapDesignRole::Materials => AiMapPlan {
            base_material: Some("Grass".to_string()),
            features: vec![AiMapFeature {
                feature_type: Some("material".to_string()),
                location: Some("east".to_string()),
                radius: 4,
                material: Some("Sand".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        },
    }
}

fn role_from_prompt(prompt: &str) -> AiMapDesignRole {
    if prompt.contains("Terrain specialist") {
        AiMapDesignRole::Terrain
    } else if prompt.contains("Water specialist") {
        AiMapDesignRole::Water
    } else {
        AiMapDesignRole::Materials
    }
}

#[test]
fn test_progress_reports_role_start_and_finish_in_order() {
    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];

    let planner = MultiAiMapPlanner::new(Arc::new(|_model, prompt, _mats, _water| {
        let role = role_from_prompt(prompt);
        if role == AiMapDesignRole::Water {
            Err("water failed".to_string())
        } else {
            let plan = create_sample_plan(role);
            let raw = serde_json::to_string(&plan).unwrap();
            Ok((plan, raw))
        }
    }));

    let mut updates = Vec::new();
    let requests = vec![
        MultiAiMapRoleRequest::new(AiMapDesignRole::Terrain, "terrain-model"),
        MultiAiMapRoleRequest::new(AiMapDesignRole::Water, "water-model"),
        MultiAiMapRoleRequest::new(AiMapDesignRole::Materials, "materials-model"),
    ];

    let result = planner
        .generate_plan(
            &requests,
            "test",
            &materials,
            60.0,
            Some(&mut |p: AiMapRoleProgress| updates.push(p)),
        )
        .unwrap();

    assert!(result.has_failures());
    assert_eq!(vec![0, 1, 1, 2, 2, 3], updates.iter().map(|u| u.completed_roles).collect::<Vec<_>>());
    assert_eq!(vec![false, true, false, true, false, true], updates.iter().map(|u| u.finished).collect::<Vec<_>>());
    assert_eq!(Some(AiMapRoleStatus::Failed), updates[3].status);
}

#[test]
fn test_calls_merge_in_role_order() {
    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];

    let planner = MultiAiMapPlanner::new(Arc::new(|_model, prompt, _mats, _water| {
        let role = role_from_prompt(prompt);
        let plan = create_sample_plan(role);
        let raw = serde_json::to_string(&plan).unwrap();
        Ok((plan, raw))
    }));

    let result = planner
        .generate_plan_single_model("local", "山谷河流草地", &materials, 60.0)
        .unwrap();

    assert!(!result.has_failures());
    let types: Vec<&str> = result
        .plan
        .as_ref()
        .unwrap()
        .features
        .iter()
        .map(|f| f.feature_type.as_deref().unwrap())
        .collect();
    assert_eq!(vec!["hill", "lake", "material"], types);
    assert_eq!(Some(80), result.plan.as_ref().unwrap().base_height);
    assert_eq!(Some("Grass".to_string()), result.plan.as_ref().unwrap().base_material);
}

#[test]
fn test_partial_failure_retains_preview_and_reports_failed_role() {
    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];

    let planner = MultiAiMapPlanner::new(Arc::new(|_model, prompt, _mats, _water| {
        let role = role_from_prompt(prompt);
        if role == AiMapDesignRole::Water {
            Err("water unavailable".to_string())
        } else {
            let plan = create_sample_plan(role);
            let raw = serde_json::to_string(&plan).unwrap();
            Ok((plan, raw))
        }
    }));

    let result = planner
        .generate_plan_single_model("local", "test", &materials, 60.0)
        .unwrap();

    assert!(result.has_failures());
    assert!(!result.is_cancelled);
    let types: Vec<&str> = result
        .plan
        .as_ref()
        .unwrap()
        .features
        .iter()
        .map(|f| f.feature_type.as_deref().unwrap())
        .collect();
    assert_eq!(vec!["hill", "material"], types);
    assert_eq!(AiMapRoleStatus::Failed, result.roles[1].status);
    assert!(result.roles[1].error.as_ref().unwrap().contains("water unavailable"));
}

#[test]
fn test_scope_and_budget_are_enforced_with_visible_warnings() {
    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];

    let planner = MultiAiMapPlanner::new(Arc::new(|_model, prompt, _mats, _water| {
        let role = role_from_prompt(prompt);
        let mut plan = create_sample_plan(role);
        plan.base_height = Some(999);
        plan.base_material = Some("Grass".to_string());
        let template = plan.features[0].clone();
        plan.features = (0..20)
            .map(|idx| AiMapFeature {
                feature_type: template.feature_type.clone(),
                x: idx as i32,
                radius: 2,
                material: Some("Sand".to_string()),
                ..Default::default()
            })
            .collect();
        plan.features.push(AiMapFeature {
            feature_type: Some(if role == AiMapDesignRole::Terrain {
                "lake".to_string()
            } else {
                "hill".to_string()
            }),
            x: 63,
            ..Default::default()
        });
        let raw = serde_json::to_string(&plan).unwrap();
        Ok((plan, raw))
    }));

    let result = planner
        .generate_plan_single_model("local", "test", &materials, 60.0)
        .unwrap();

    let plan = result.plan.unwrap();
    assert_eq!(24, plan.features.len());
    assert_eq!(Some(255), plan.base_height);
    for role_res in &result.roles {
        assert_eq!(FEATURES_PER_ROLE, role_res.accepted_features);
        assert!(role_res.warnings.iter().any(|w| w.contains("截斷 12")));
        assert!(role_res.warnings.iter().any(|w| w.contains("範圍")));
    }
}

#[test]
fn test_role_prompts_constrain_each_role() {
    let terrain_prompt = MultiAiMapPlanner::build_role_prompt(AiMapDesignRole::Terrain, "測試要求");
    assert!(terrain_prompt.contains("baseHeight"));
    assert!(terrain_prompt.contains("Do not set baseMaterial"));
    assert!(terrain_prompt.contains("at most 8 features"));

    let water_prompt = MultiAiMapPlanner::build_role_prompt(AiMapDesignRole::Water, "測試要求");
    assert!(water_prompt.contains("lake and river"));
    assert!(water_prompt.contains("Do not set baseHeight"));

    let mat_prompt = MultiAiMapPlanner::build_role_prompt(AiMapDesignRole::Materials, "測試要求");
    assert!(mat_prompt.contains("material features"));
    assert!(mat_prompt.contains("Do not set baseHeight"));
}
