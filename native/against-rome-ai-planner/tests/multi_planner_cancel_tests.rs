use against_rome_ai_planner::multi_planner::{
    AiMapDesignRole, AiMapRoleStatus, CancelToken, MultiAiMapPlanner, MultiAiMapRoleRequest,
};
use against_rome_ai_planner::ollama::AiMaterialOption;
use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn hill_plan() -> AiMapPlan {
    AiMapPlan {
        base_height: Some(70),
        features: vec![AiMapFeature {
            feature_type: Some("hill".into()),
            location: Some("north".into()),
            radius: 4,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn requests() -> Vec<MultiAiMapRoleRequest> {
    vec![
        MultiAiMapRoleRequest::new(AiMapDesignRole::Terrain, "m"),
        MultiAiMapRoleRequest::new(AiMapDesignRole::Water, "m"),
    ]
}

#[test]
fn cancel_before_start_runs_no_provider_and_reports_cancelled() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let planner = MultiAiMapPlanner::new(Arc::new(move |_, _, _, _| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok((hill_plan(), "{}".into()))
    }));
    let token = CancelToken::new();
    token.cancel();
    let mut progress_count = 0;
    let mut cb = |_p| progress_count += 1;
    let r = planner
        .generate_plan_cancellable(&requests(), "desc", &[AiMaterialOption::new("G", "g")], 0.0, &token, Some(&mut cb))
        .unwrap();
    assert!(r.is_cancelled);
    assert!(r.plan.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(progress_count, 2, "only finish events are reported once cancelled");
    assert!(r.roles.iter().all(|r| r.status == AiMapRoleStatus::Cancelled));
}

#[test]
fn cancel_during_provider_discards_that_roles_result_and_skips_the_rest() {
    let token = CancelToken::new();
    let t = token.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let planner = MultiAiMapPlanner::new(Arc::new(move |_, _, _, _| {
        c.fetch_add(1, Ordering::SeqCst);
        t.cancel(); // cancelled while the provider is working; it still "succeeds"
        Ok((hill_plan(), serde_json::to_string(&hill_plan()).unwrap()))
    }));
    let r = planner
        .generate_plan_cancellable(&requests(), "desc", &[AiMaterialOption::new("G", "g")], 0.0, &token, None)
        .unwrap();
    assert!(r.is_cancelled);
    assert!(r.plan.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(r.roles.iter().all(|r| r.status == AiMapRoleStatus::Cancelled));
}

#[test]
fn uncancelled_token_matches_plain_generate_plan() {
    let planner = MultiAiMapPlanner::new(Arc::new(|_, _, _, _| {
        Ok((hill_plan(), serde_json::to_string(&hill_plan()).unwrap()))
    }));
    let mats = [AiMaterialOption::new("G", "g")];
    let a = planner.generate_plan(&requests(), "desc", &mats, 0.0, None).unwrap();
    let b = planner
        .generate_plan_cancellable(&requests(), "desc", &mats, 0.0, &CancelToken::new(), None)
        .unwrap();
    assert_eq!(a, b);
    assert!(!b.is_cancelled);
}
