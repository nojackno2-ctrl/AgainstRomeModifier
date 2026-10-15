use against_rome_textures::{
    NativeTerrainMaterialResolver, TerrainBlendAuthoringMap, TerrainBlendEditSession,
};
use std::sync::Arc;

struct Resolver;
impl NativeTerrainMaterialResolver for Resolver {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let parts: Vec<&str> = texture.split('/').collect();
        if parts.len() == 4 {
            Some([
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
            ])
        } else {
            None
        }
    }

    fn resolve_native_tile(&self, corners: &[&str; 4], _x: usize, _y: usize) -> Option<String> {
        if corners.iter().any(|&c| c.eq_ignore_ascii_case("unsupported")) {
            None
        } else {
            Some(corners.join("/"))
        }
    }
}

struct ReloadedResolver;
impl NativeTerrainMaterialResolver for ReloadedResolver {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let clean = texture.strip_prefix("new:").unwrap_or(texture);
        let parts: Vec<&str> = clean.split('/').collect();
        if parts.len() == 4 {
            Some([
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
            ])
        } else {
            None
        }
    }

    fn resolve_native_tile(&self, corners: &[&str; 4], _x: usize, _y: usize) -> Option<String> {
        Some(format!("new:{}", corners.join("/")))
    }
}

#[test]
fn protected_circle_preserves_adjacent_tiles_and_corners_through_bake_and_undo() {
    let resolver = Arc::new(Resolver);
    let source: Vec<String> = vec!["grass/grass/grass/grass".to_string(); 64];
    let import = TerrainBlendAuthoringMap::import(8, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    let allowed = |x: usize, y: usize| -> bool {
        x >= 2 && y >= 2 && x < 7 && y < 7 && !(x == 4 && y == 4)
    };

    let result = session
        .paint_circle(4.0, 4.0, 8.0, "sand", true, false, Some(&allowed))
        .unwrap();

    assert!(result.succeeded);
    assert!(!result.texture_changes.is_empty());
    for change in &result.texture_changes {
        assert!(allowed(change.x, change.y));
    }

    for y in 0..8 {
        for x in 0..8 {
            if !allowed(x, y) {
                assert_eq!(
                    session.get_texture(x, y).unwrap(),
                    source[y * 8 + x],
                    "Tile ({}, {}) should remain unchanged",
                    x,
                    y
                );
            }
        }
    }

    for &(cx, cy) in &[(4, 4), (5, 4), (4, 5), (5, 5), (2, 3), (7, 3)] {
        assert_eq!(session.authoring_map().get_corner(cx, cy).unwrap(), "grass");
    }

    let edited = session.current_textures().to_vec();
    assert!(session.commit_stroke());
    session.undo().unwrap();
    assert_eq!(session.current_textures(), source.as_slice());

    session.redo().unwrap();
    assert_eq!(session.current_textures(), edited.as_slice());

    // Auto-bridge on protected circle must error
    assert!(session
        .paint_circle(4.0, 4.0, 8.0, "sand", true, true, Some(&allowed))
        .is_err());
}

#[test]
fn rebinding_resource_resolver_keeps_saved_baseline_history_and_uses_new_resolver_for_future_strokes() {
    let resolver = Arc::new(Resolver);
    let source = vec!["grass/grass/grass/grass".to_string()];
    let import = TerrainBlendAuthoringMap::import(1, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    assert!(session.paint_circle(0.0, 0.0, 0.0, "sand", true, false, None).unwrap().succeeded);
    assert!(session.commit_stroke());
    let first = session.current_textures().to_vec();

    session.undo().unwrap();
    session.rebind_material_resolver(Arc::new(ReloadedResolver));
    assert!(!session.is_dirty());
    assert!(session.can_redo());

    session.redo().unwrap();
    assert_eq!(session.current_textures(), first.as_slice());

    assert!(session.paint_circle(1.0, 1.0, 0.0, "water", true, false, None).unwrap().succeeded);
    assert!(session.commit_stroke());
    let second = session.current_textures().to_vec();
    assert_eq!(second[0], "new:sand/grass/water/grass");

    session.undo().unwrap();
    assert_eq!(session.current_textures(), first.as_slice());

    session.undo().unwrap();
    assert_eq!(session.current_textures(), source.as_slice());
    assert!(!session.is_dirty());

    session.redo().unwrap();
    session.redo().unwrap();
    assert_eq!(session.current_textures(), second.as_slice());

    session.reset_to_baseline();
    assert_eq!(session.current_textures(), source.as_slice());
}

#[test]
fn rejected_independent_area_preserves_earlier_areas_in_one_undo_step() {
    let resolver = Arc::new(Resolver);
    let source = vec!["grass/grass/grass/grass".to_string()];
    let import = TerrainBlendAuthoringMap::import(1, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    assert!(session.paint_circle(0.0, 0.0, 0.0, "sand", false, false, None).unwrap().succeeded);
    let rejected = session.paint_circle(0.5, 0.5, 2.0, "unsupported", false, false, None).unwrap();
    assert!(!rejected.succeeded);
    assert!(rejected.texture_changes.is_empty());
    assert_eq!(session.current_textures()[0], "sand/grass/grass/grass");

    assert!(session.paint_circle(1.0, 1.0, 0.0, "water", false, false, None).unwrap().succeeded);
    assert!(session.commit_stroke());
    assert_eq!(session.current_textures()[0], "sand/grass/water/grass");

    session.undo().unwrap();
    assert!(!session.is_dirty());
    assert!(!session.can_undo());

    session.redo().unwrap();
    assert_eq!(session.current_textures()[0], "sand/grass/water/grass");
}

#[test]
fn blend_module_import_paint_undo_redo_and_baseline_need_no_host() {
    let resolver = Arc::new(Resolver);
    let source = vec!["grass/grass/grass/grass".to_string()];
    let import = TerrainBlendAuthoringMap::import(1, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    assert!(!session.is_dirty());
    let result = session.paint_circle(0.0, 0.0, 0.0, "sand", true, false, None).unwrap();
    assert!(result.succeeded);
    assert_eq!(session.current_textures()[0], "sand/grass/grass/grass");
    assert!(session.commit_stroke());

    let undone = session.undo().unwrap();
    assert_eq!(undone.len(), 1);
    assert!(!session.is_dirty());
    assert_eq!(session.authoring_map().get_corner(0, 0).unwrap(), "grass");

    let redone = session.redo().unwrap();
    assert_eq!(redone.len(), 1);
    assert_eq!(session.authoring_map().get_corner(0, 0).unwrap(), "sand");

    session.commit_baseline();
    assert!(!session.is_dirty());

    session.paint_circle(1.0, 1.0, 0.0, "water", true, false, None).unwrap();
    session.reset_to_baseline();
    assert_eq!(session.current_textures()[0], "sand/grass/grass/grass");
    assert!(!session.is_dirty());
    assert!(!session.can_undo());
}

#[test]
fn unsupported_junction_rolls_back_entire_pending_stroke_without_losing_committed_history() {
    let resolver = Arc::new(Resolver);
    let source = vec!["grass/grass/grass/grass".to_string()];
    let import = TerrainBlendAuthoringMap::import(1, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    session.paint_circle(0.0, 0.0, 0.0, "sand", true, false, None).unwrap();
    session.commit_stroke();
    session.paint_circle(1.0, 0.0, 0.0, "water", true, false, None).unwrap();

    let rejected = session.paint_circle(1.0, 1.0, 0.0, "unsupported", true, false, None).unwrap();
    assert!(!rejected.succeeded);
    assert_eq!(rejected.issues.len(), 1);
    assert_eq!(session.current_textures()[0], "sand/grass/grass/grass");
    assert_eq!(
        session.authoring_map().corner_materials(),
        &["sand", "grass", "grass", "grass"]
    );
    assert!(session.can_undo());

    session.undo().unwrap();
    assert!(!session.is_dirty());
}

#[test]
fn stamp_sets_one_exact_tile_and_is_undoable_and_redoable() {
    let resolver = Arc::new(Resolver);
    let source: Vec<String> = vec!["grass/grass/grass/grass".to_string(); 9];
    let import = TerrainBlendAuthoringMap::import(3, &source, resolver.as_ref(), "grass").unwrap();
    let mut session = TerrainBlendEditSession::new(import, &source, resolver).unwrap();

    assert!(session.stamp_texture(5, 0, "weg1").is_none());
    let change = session.stamp_texture(1, 1, "weg1").unwrap();
    assert_eq!(change.before, "grass/grass/grass/grass");
    assert_eq!(change.after, "weg1");
    assert_eq!(session.get_texture(1, 1).unwrap(), "weg1");

    assert!(session.commit_stroke());
    session.undo().unwrap();
    assert_eq!(session.get_texture(1, 1).unwrap(), "grass/grass/grass/grass");

    session.redo().unwrap();
    assert_eq!(session.get_texture(1, 1).unwrap(), "weg1");
}
