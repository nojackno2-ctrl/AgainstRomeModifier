use against_rome_environment::{
    addition_index_from_object_index, apply_blank_terrain_nature_clearance,
    build_nature_display_objects, delete_picked_nature, is_addition_index,
    is_existing_slot_index, is_level_data_object, nature_deleted_status_message,
    nature_selected_status_message, object_index_for_addition, object_index_for_slot,
    resolve_picked_nature, slot_from_object_index, NatureAddition, NatureEditSession,
    NaturePickResult, PlacedNatureObject, ADDITION_OBJECT_INDEX_BASE,
    LEVEL_OBJECT_INDEX_BASE, NATIVE_DATA_OBJECTS_SOURCE,
};

#[test]
fn test_object_index_encoding_and_decoding_parity() {
    assert_eq!(LEVEL_OBJECT_INDEX_BASE, -100_000);
    assert_eq!(ADDITION_OBJECT_INDEX_BASE, -200_000);
    assert_eq!(NATIVE_DATA_OBJECTS_SOURCE, "DATA/objects.dat");

    // Level slot indices: -100000 - slot
    assert_eq!(object_index_for_slot(0), -100_000);
    assert_eq!(object_index_for_slot(1), -100_001);
    assert_eq!(object_index_for_slot(42), -100_042);
    assert_eq!(slot_from_object_index(-100_000), Some(0));
    assert_eq!(slot_from_object_index(-100_001), Some(1));
    assert_eq!(slot_from_object_index(-100_042), Some(42));

    // Addition indices: -200000 - index
    assert_eq!(object_index_for_addition(0), -200_000);
    assert_eq!(object_index_for_addition(1), -200_001);
    assert_eq!(object_index_for_addition(9), -200_009);
    assert_eq!(addition_index_from_object_index(-200_000), Some(0));
    assert_eq!(addition_index_from_object_index(-200_001), Some(1));
    assert_eq!(addition_index_from_object_index(-200_009), Some(9));

    // Predicates and boundaries
    assert!(is_existing_slot_index(-100_000));
    assert!(is_existing_slot_index(-199_999));
    assert!(!is_existing_slot_index(-200_000));
    assert!(!is_existing_slot_index(-99_999));
    assert!(!is_existing_slot_index(0));

    assert!(is_addition_index(-200_000));
    assert!(is_addition_index(-200_050));
    assert!(!is_addition_index(-199_999));
    assert!(!is_addition_index(0));

    assert!(is_level_data_object(-100_000, "DATA/objects.dat"));
    assert!(is_level_data_object(-200_005, "DATA/objects.dat"));
    assert!(!is_level_data_object(0, "DATA/objects.dat"));
    assert!(!is_level_data_object(10, "DATA/objects.dat"));
    assert!(!is_level_data_object(-100_000, "DATA/terrain.dat"));
}

#[test]
fn test_build_nature_display_objects_filters_removals_and_non_landscape() {
    let objects = vec![
        PlacedNatureObject::with_transform(0, 1, "LanGerLau00", 100.0, 5.0, 100.0, 0.5, 8, false),
        PlacedNatureObject::with_transform(1, 1, "LanGerLau00", 200.0, 10.0, 200.0, 1.2, 8, true),
        PlacedNatureObject::with_transform(2, 99, "BauGerHau00", 300.0, 0.0, 300.0, 0.0, 1, false),
    ];

    let mut session = NatureEditSession::new();
    session.plant(NatureAddition::new(10, "LanGerBir00", 400.0, 8.0, 400.0, 2.5));
    session.plant(NatureAddition::new(11, "LanGerEic00", 500.0, 9.0, 500.0, 3.1));
    session.commit_stroke();

    // Mark slot 0 as removed
    session.remove(&[0], &[]);

    let display = build_nature_display_objects(&objects, &session);
    assert_eq!(display.len(), 3);

    // Slot 1 is present (slot 0 was removed, slot 2 is BauGerHau00 non-landscape)
    let slot1 = display.iter().find(|d| d.object_index == -100_001).unwrap();
    assert_eq!(slot1.name, "LanGerLau00");
    assert_eq!(slot1.x, 200.0);
    assert_eq!(slot1.y, 10.0);
    assert_eq!(slot1.z, 200.0);
    assert!(!slot1.is_addition);
    assert!(slot1.is_level_data_object());
    assert!((slot1.angle_degrees.unwrap() - (1.2 * 180.0 / std::f32::consts::PI)).abs() < 1e-4);

    // Addition 0 (-200000)
    let add0 = display.iter().find(|d| d.object_index == -200_000).unwrap();
    assert_eq!(add0.name, "LanGerBir00");
    assert_eq!(add0.x, 400.0);
    assert_eq!(add0.team, 8);
    assert!(add0.is_addition);

    // Addition 1 (-200001)
    let add1 = display.iter().find(|d| d.object_index == -200_001).unwrap();
    assert_eq!(add1.name, "LanGerEic00");
    assert_eq!(add1.x, 500.0);
    assert!(add1.is_addition);
}

#[test]
fn test_csharp_oracle_delete_picked_nature_workflow_parity() {
    // Exact parity with C# MapEditorNatureHistoryTests.cs:
    // Picked_nature_objects_are_deleted_with_undo_in_move_mode_and_fixed_objects_are_kept
    let pos = 10.5 * 64.0;
    let objects = vec![
        PlacedNatureObject::with_transform(0, 1, "LanGerLau00", pos, 0.0, pos, 0.0, 8, false),
        PlacedNatureObject::with_transform(1, 1, "LanGerLau00", pos, 0.0, pos, 0.0, 8, true), // linked
    ];

    let mut session = NatureEditSession::new();
    let addition = NatureAddition::new(1, "LanGerLau00", pos + 100.0, 0.0, pos + 100.0, 0.785);
    session.plant(addition.clone());
    session.commit_stroke();

    // 1. Pick linked slot 1 (-100001) -> ProtectedLinkedObject, cannot delete
    let pick_linked = resolve_picked_nature(-100_001, "DATA/objects.dat", &objects, &session);
    assert_eq!(
        pick_linked,
        NaturePickResult::ProtectedLinkedObject {
            slot: 1,
            name: "LanGerLau00".to_string()
        }
    );

    // 2. Pick removable slot 0 (-100000) -> Success
    let pick_slot0 = resolve_picked_nature(-100_000, "DATA/objects.dat", &objects, &session);
    let target0 = match pick_slot0 {
        NaturePickResult::Success(t) => t,
        other => panic!("Expected Success, got {other:?}"),
    };
    assert_eq!(target0.name(), "LanGerLau00");

    // Delete picked slot 0
    let deleted = delete_picked_nature(&target0, &mut session);
    assert!(deleted);
    assert!(session.removed_slots().contains(&0));

    // Subsequent pick on slot 0 reports AlreadyRemoved (pick is consumed)
    let pick_again = resolve_picked_nature(-100_000, "DATA/objects.dat", &objects, &session);
    assert_eq!(pick_again, NaturePickResult::AlreadyRemoved { slot: 0 });

    // Undo restores slot 0
    assert!(session.can_undo());
    assert!(session.undo());
    assert!(!session.removed_slots().contains(&0));

    // Redo removes slot 0 again
    assert!(session.can_redo());
    assert!(session.redo());
    assert!(session.removed_slots().contains(&0));

    // 3. Pick pending planted addition (-200000) -> Success
    let pick_add = resolve_picked_nature(-200_000, "DATA/objects.dat", &objects, &session);
    let target_add = match pick_add {
        NaturePickResult::Success(t) => t,
        other => panic!("Expected Success, got {other:?}"),
    };
    assert!(target_add.is_addition());

    // Delete pending addition
    let deleted_add = delete_picked_nature(&target_add, &mut session);
    assert!(deleted_add);
    assert_eq!(session.additions().len(), 0);

    // Undo restores the addition
    assert!(session.undo());
    assert_eq!(session.additions().len(), 1);
    assert_eq!(session.additions()[0].name, "LanGerLau00");
    assert_eq!(session.additions()[0].rotation, 0.785);
}

#[test]
fn test_csharp_oracle_blank_terrain_clearance_parity() {
    // Exact parity with C# MapEditorNatureBlankTests.cs:
    // Blank_terrain_clears_stored_removable_and_pending_additions_while_preserving_linked_and_building_objects
    let pos = 10.5 * 64.0;
    let objects = vec![
        PlacedNatureObject::with_transform(0, 1, "LanGerLau00", pos, 0.0, pos, 0.5, 8, false),
        PlacedNatureObject::with_transform(1, 1, "LanGerLau00", pos + 50.0, 0.0, pos + 50.0, 1.2, 8, true), // linked
        PlacedNatureObject::with_transform(2, 2, "LanGerEic00", pos + 100.0, 0.0, pos + 100.0, 2.1, 8, false),
        PlacedNatureObject::with_transform(3, 3, "BauRomHau00", pos + 150.0, 0.0, pos + 150.0, 0.0, 1, false), // building
    ];

    let mut session = NatureEditSession::new();
    let add1 = NatureAddition::new(10, "LanGerBir00", pos + 200.0, 0.0, pos + 200.0, 0.42);
    let add2 = NatureAddition::new(11, "LanGerBus00", pos + 250.0, 0.0, pos + 250.0, 1.84);
    session.plant(add1.clone());
    session.plant(add2.clone());
    session.commit_stroke();

    assert_eq!(session.additions().len(), 2);
    assert!(session.removed_slots().is_empty());
    assert!(session.is_dirty());

    // Apply blank terrain clearance
    let report = apply_blank_terrain_nature_clearance(&objects, &mut session);

    // Removable slots 0 and 2 were cleared
    assert_eq!(report.removed_slots, vec![0, 2]);
    assert_eq!(report.removed_additions_count, 2);
    // Linked slot 1 and building slot 3 are strictly preserved
    assert_eq!(report.preserved_linked_slots, vec![1]);
    assert_eq!(report.preserved_non_landscape_slots, vec![3]);

    // Session state
    assert!(session.additions().is_empty());
    assert!(session.removed_slots().contains(&0));
    assert!(session.removed_slots().contains(&2));
    assert!(!session.removed_slots().contains(&1));
    assert!(!session.removed_slots().contains(&3));
    assert!(session.is_dirty());
    assert!(session.can_undo());

    // Undo restores the additions and removes the slot removals
    assert!(session.undo());
    assert!(session.removed_slots().is_empty());
    assert_eq!(session.additions().len(), 2);
    assert_eq!(session.additions()[0].name, "LanGerBir00");
    assert_eq!(session.additions()[0].rotation, 0.42);
    assert_eq!(session.additions()[1].name, "LanGerBus00");
    assert_eq!(session.additions()[1].rotation, 1.84);

    // Redo clears them again
    assert!(session.redo());
    assert!(session.additions().is_empty());
    assert!(session.removed_slots().contains(&0));
    assert!(session.removed_slots().contains(&2));
}

#[test]
fn test_nature_status_messages_localization_parity() {
    assert_eq!(
        nature_deleted_status_message("LanGerLau00", true),
        "Removed LanGerLau00; Undo restores it."
    );
    assert_eq!(
        nature_deleted_status_message("LanGerLau00", false),
        "已移除 LanGerLau00；可用「復原」還原。"
    );

    assert_eq!(
        nature_selected_status_message("LanGerLau00", true, true),
        "Selected LanGerLau00; press Delete to remove it (undo restores it)."
    );
    assert_eq!(
        nature_selected_status_message("LanGerLau00", true, false),
        "已選取 LanGerLau00；按 Delete 移除（可復原）。"
    );

    assert_eq!(
        nature_selected_status_message("BauGerHau00", false, true),
        "BauGerHau00 is a placed or fixed map object; edit it with the Place tool."
    );
    assert_eq!(
        nature_selected_status_message("BauGerHau00", false, false),
        "BauGerHau00 屬於放置物件或地圖固定物件，請用「放置」工具編輯。"
    );
}

#[test]
fn test_edge_cases_and_invalid_data_safety() {
    let objects = vec![
        PlacedNatureObject::new(10, 1, "LanGerNad00", 50.0, 50.0, false),
    ];
    let session = NatureEditSession::new();

    // Invalid source
    assert_eq!(
        resolve_picked_nature(-100_010, "DATA/models.dat", &objects, &session),
        NaturePickResult::NotLevelDataObject
    );

    // Positive index
    assert_eq!(
        resolve_picked_nature(5, "DATA/objects.dat", &objects, &session),
        NaturePickResult::NotLevelDataObject
    );

    // Non-existent slot
    assert_eq!(
        resolve_picked_nature(-100_099, "DATA/objects.dat", &objects, &session),
        NaturePickResult::SlotNotFound { slot: 99 }
    );

    // Non-existent addition index
    assert_eq!(
        resolve_picked_nature(-200_005, "DATA/objects.dat", &objects, &session),
        NaturePickResult::AdditionNotFound { index: 5 }
    );

    // Display object with NaN/Inf rotation handles gracefully without crashing
    let nan_obj = PlacedNatureObject::with_transform(
        20, 1, "LanGerNad00", 10.0, 0.0, 10.0, f32::NAN, 8, false,
    );
    let display = build_nature_display_objects(&[nan_obj], &session);
    assert_eq!(display.len(), 1);
    assert_eq!(display[0].angle_degrees, None);
}
