use against_rome_scene::{
    scene_object::{SceneObject, SceneObjectKind},
    Vec3,
};

#[test]
fn scene_object_kind_categorization() {
    let building = SceneObject::new("BauHaus", Vec3::ZERO, Vec3::ZERO, 0, "test.sdl", 0, None);
    assert_eq!(building.kind(), SceneObjectKind::Building);

    let unit = SceneObject::new("FigLegion", Vec3::ZERO, Vec3::ZERO, 1, "test.sdl", 1, None);
    assert_eq!(unit.kind(), SceneObjectKind::Unit);

    let misc = SceneObject::new("Tree01", Vec3::ZERO, Vec3::ZERO, 0, "test.sdl", 2, None);
    assert_eq!(misc.kind(), SceneObjectKind::Object);
}

#[test]
fn scene_object_marker_visibility_rules() {
    // Ordinary objects in custom map files are visible
    let custom_obj = SceneObject::new("Tree", Vec3::ZERO, Vec3::ZERO, 0, "Maps/test.sdl", 0, None);
    assert!(custom_obj.marker_visible());

    // Objects from DATA/objects.dat are invisible in game unless they are Skriptmark
    let data_obj = SceneObject::new("Tree", Vec3::ZERO, Vec3::ZERO, 0, "DATA/objects.dat", 1, None);
    assert!(!data_obj.marker_visible());

    let script_mark = SceneObject::new("SkriptmarkTarget", Vec3::ZERO, Vec3::ZERO, 0, "DATA/objects.dat", 2, None);
    assert!(script_mark.marker_visible());
}

#[test]
fn scene_object_safe_move_and_clamp() {
    let obj = SceneObject::new(
        "Tree",
        Vec3::new(100.0, 10.0, 200.0),
        Vec3::new(50.0, 10.0, 50.0),
        0,
        "test.sdl",
        0,
        Some(45.0),
    );

    let moved = obj.move_to(300.0, 400.0).expect("finite coords move");
    assert_eq!(moved.world.x, 300.0);
    assert_eq!(moved.world.z, 400.0);
    assert_eq!(moved.local.x, 50.0 + (300.0 - 100.0));
    assert_eq!(moved.local.z, 50.0 + (400.0 - 200.0));

    // Clamps to [0, 16384]
    let clamped = obj.move_to(-50.0, 20000.0).expect("clamps to map");
    assert_eq!(clamped.world.x, 0.0);
    assert_eq!(clamped.world.z, 16384.0);

    // Rejects non-finite coordinates
    assert!(obj.move_to(f32::NAN, 100.0).is_err());
    assert!(obj.move_to(100.0, f32::INFINITY).is_err());
}
