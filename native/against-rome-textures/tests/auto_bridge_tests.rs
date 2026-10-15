use against_rome_textures::{
    SimpleRuleResolver, TerrainBlendAuthoringMap, TerrainBlendEditSession,
};
use std::sync::Arc;

fn make_uniform_session(
    resolver: SimpleRuleResolver,
    size: usize,
    material: &str,
) -> TerrainBlendEditSession {
    let resolver_arc = Arc::new(resolver);
    let source: Vec<String> = vec![format!("{m}/{m}/{m}/{m}", m = material); size * size];
    let import =
        TerrainBlendAuthoringMap::import(size, &source, resolver_arc.as_ref(), material).unwrap();
    TerrainBlendEditSession::new(import, &source, resolver_arc).unwrap()
}

#[test]
fn without_bridge_a_missing_transition_is_rejected_and_rolled_back() {
    let resolver = SimpleRuleResolver::new()
        .with_materials(&["grass", "mud", "rock", "water"])
        .with_chain_pairs(&["grass|mud", "mud|rock"]);

    let mut session = make_uniform_session(resolver, 12, "grass");
    let result = session.paint_circle(6.0, 6.0, 2.0, "rock", true, false, None).unwrap();

    assert!(!result.succeeded);
    assert!(!result.issues.is_empty());
    assert!(session
        .current_textures()
        .iter()
        .all(|t| t == "grass/grass/grass/grass"));
}

#[test]
fn auto_bridge_inserts_a_mud_ring_between_rock_and_grass_using_only_supported_tiles() {
    let resolver = SimpleRuleResolver::new()
        .with_materials(&["grass", "mud", "rock", "water"])
        .with_chain_pairs(&["grass|mud", "mud|rock"]);

    let mut session = make_uniform_session(resolver, 12, "grass");
    let result = session.paint_circle(6.0, 6.0, 2.0, "rock", true, true, None).unwrap();

    assert!(result.succeeded, "Auto bridge should succeed, issues: {:?}", result.issues);

    // All current textures must resolve and no tile can directly connect rock with grass
    for tex in session.current_textures() {
        let parts: Vec<&str> = tex.split('/').collect();
        assert_eq!(parts.len(), 4);
        assert!(
            !(parts.contains(&"rock") && parts.contains(&"grass")),
            "rock should not directly transition to grass: {}",
            tex
        );
    }

    assert!(session.current_textures().iter().any(|t| t.contains("mud")));
    assert!(session.current_textures().iter().any(|t| t == "rock/rock/rock/rock"));

    session.commit_stroke();
    session.undo().unwrap();
    assert!(session
        .current_textures()
        .iter()
        .all(|t| t == "grass/grass/grass/grass"));
    assert!(!session.is_dirty());
}

#[test]
fn auto_bridge_can_chain_two_rings_and_still_rejects_when_no_bridge_exists() {
    // rock -> water -> mud -> grass
    let chained_resolver = SimpleRuleResolver::new()
        .with_materials(&["grass", "mud", "rock", "water"])
        .with_chain_pairs(&["grass|mud", "mud|water", "water|rock"]);

    let mut chained_session = make_uniform_session(chained_resolver, 16, "grass");
    let result = chained_session
        .paint_circle(8.0, 8.0, 2.0, "rock", true, true, None)
        .unwrap();
    assert!(result.succeeded, "Two-ring bridge rock->water->mud->grass should succeed");

    // Isolated: no bridge to rock
    let isolated_resolver = SimpleRuleResolver::new()
        .with_materials(&["grass", "mud", "rock", "water"])
        .with_chain_pairs(&["grass|mud"]);

    let mut isolated_session = make_uniform_session(isolated_resolver, 12, "grass");
    let isolated_result = isolated_session
        .paint_circle(6.0, 6.0, 2.0, "rock", true, true, None)
        .unwrap();
    assert!(!isolated_result.succeeded);
    assert!(isolated_session
        .current_textures()
        .iter()
        .all(|t| t == "grass/grass/grass/grass"));
}
