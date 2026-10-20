use against_rome_environment::nature::{
    map_nature_regions, nature_category, nature_region, NatureAddition, NatureBrushPainter,
    NatureBrushRemover, NatureCatalogItem, NatureCategory, NatureDensity, NatureEditSession,
    NatureOperation, NaturePaletteFilter, NatureSpeciesMixer, PlacedNatureObject,
};
use against_rome_environment::random::DotNetRandom;
use std::collections::BTreeSet;

#[test]
fn test_nature_region_extraction() {
    assert_eq!(nature_region("LanGerNad00_Tanne_gross"), "Ger");
    assert_eq!(nature_region("LanItaZyp01_Zypresse"), "Ita");
    assert_eq!(nature_region("LanKelBir02_Birke"), "Kel");
    assert_eq!(nature_region("LanDanEic01"), "Dan");
    assert_eq!(nature_region("Short"), "");
    assert_eq!(nature_region(""), "");
    assert_eq!(nature_region("Lan"), "");
}

#[test]
fn test_nature_category_classification_and_localization() {
    // Trees
    for code in &["Nad", "Lau", "Bau", "Pal", "Bir", "Eic", "Buc", "Kie", "Tan", "Fic", "Obs", "Zyp", "Pin"] {
        let name = format!("LanGer{code}01_Test");
        assert_eq!(nature_category(&name), NatureCategory::Tree, "Failed on {code}");
    }
    // Grass
    for code in &["Gra", "Bli", "Blu"] {
        let name = format!("LanGer{code}01_Test");
        assert_eq!(nature_category(&name), NatureCategory::Grass, "Failed on {code}");
    }
    // Bushes
    for code in &["Dor", "Bod", "Bus", "Str", "Ger", "Hec"] {
        let name = format!("LanGer{code}01_Test");
        assert_eq!(nature_category(&name), NatureCategory::Bush, "Failed on {code}");
    }
    // Water
    for code in &["Sch", "Ent", "Wei", "Ser"] {
        let name = format!("LanGer{code}01_Test");
        assert_eq!(nature_category(&name), NatureCategory::Water, "Failed on {code}");
    }
    // Rock
    for code in &["Ste", "Fel", "Sto"] {
        let name = format!("LanGer{code}01_Test");
        assert_eq!(nature_category(&name), NatureCategory::Rock, "Failed on {code}");
    }
    // Other
    assert_eq!(nature_category("LanGerXyz01_Unknown"), NatureCategory::Other);
    assert_eq!(nature_category("BauGerHau00_Haupthaus"), NatureCategory::Other);

    // Localization and key mapping
    assert_eq!(NatureCategory::Tree.key(), "tree");
    assert_eq!(NatureCategory::from_key("tree"), Some(NatureCategory::Tree));
    assert_eq!(NatureCategory::Tree.display_name(true), "Trees");
    assert_eq!(NatureCategory::Tree.display_name(false), "樹木");
    assert_eq!(NatureCategory::Grass.display_name(true), "Grass & flowers");
    assert_eq!(NatureCategory::Grass.display_name(false), "草叢與花");

    assert_eq!(NatureOperation::Plant.display_name(true), "Plant");
    assert_eq!(NatureOperation::Plant.display_name(false), "種植");
    assert_eq!(NatureOperation::Remove.display_name(true), "Remove");
    assert_eq!(NatureOperation::Remove.display_name(false), "移除");
}

#[test]
fn test_map_nature_regions_dominant_threshold() {
    // Empty input returns empty
    let empty: Vec<&str> = Vec::new();
    assert!(map_nature_regions(empty).is_empty());

    // Non-landscape objects do not count
    let non_landscape = vec!["BauGerHau00_Haupthaus", "FigGerInf01_Schwert", "BauRomHau00"];
    assert!(map_nature_regions(non_landscape).is_empty());

    // Single dominant region (85% Ger, 10% Ita, 5% Kel)
    // Only Ger is >= 15%
    let mut names = Vec::new();
    for _ in 0..85 {
        names.push("LanGerNad01_Tanne");
    }
    for _ in 0..10 {
        names.push("LanItaZyp01_Zypresse");
    }
    for _ in 0..5 {
        names.push("LanKelBir01_Birke");
    }
    let dominant = map_nature_regions(names);
    assert_eq!(dominant.len(), 1);
    assert!(dominant.contains("GER"));
    assert!(!dominant.contains("ITA"));
    assert!(!dominant.contains("KEL"));

    // Multi-region map (60% Ger, 30% Ita, 10% Kel)
    // Both Ger (60%) and Ita (30%) are >= 15%
    let mut mixed = Vec::new();
    for _ in 0..60 {
        mixed.push("langerNad01");
    }
    for _ in 0..30 {
        mixed.push("LanItaZyp01");
    }
    for _ in 0..10 {
        mixed.push("LanKelBir01");
    }
    let dominant_mixed = map_nature_regions(mixed);
    assert_eq!(dominant_mixed.len(), 2);
    assert!(dominant_mixed.contains("GER"));
    assert!(dominant_mixed.contains("ITA"));
    assert!(!dominant_mixed.contains("KEL"));
}

#[test]
fn test_nature_palette_filtering_and_sorting() {
    let items = vec![
        NatureCatalogItem::new(1, "LanGerNad02_Tanne"),
        NatureCatalogItem::new(2, "LanGerNad01_Tanne"),
        NatureCatalogItem::new(3, "LanGerBus01_Busch"),
        NatureCatalogItem::new(4, "LanItaZyp01_Zypresse"),
        NatureCatalogItem::new(5, "LanItaBus01_Busch"),
        NatureCatalogItem::new(6, "LanKelSte01_Stein"),
    ];

    // Filter by dominant region GER, show_other_regions = false, all categories
    let mut dominant_ger = BTreeSet::new();
    dominant_ger.insert("GER".to_string());
    let filter = NaturePaletteFilter::new()
        .with_dominant_regions(dominant_ger.clone())
        .with_show_other_regions(false);

    let res = filter.filter_and_sort(&items);
    let names: Vec<&str> = res.iter().map(|i| i.name.as_str()).collect();
    // Only GER items, sorted case-insensitively
    assert_eq!(names, vec![
        "LanGerBus01_Busch",
        "LanGerNad01_Tanne",
        "LanGerNad02_Tanne",
    ]);

    // Filter by dominant region GER AND category Tree
    let tree_filter = filter.clone().with_category(Some(NatureCategory::Tree));
    let tree_res = tree_filter.filter_and_sort(&items);
    let tree_names: Vec<&str> = tree_res.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(tree_names, vec![
        "LanGerNad01_Tanne",
        "LanGerNad02_Tanne",
    ]);

    // Show other regions = true: Italian and Celtic items appear, sorted alphabetically
    let all_regions_filter = filter.with_show_other_regions(true);
    let all_res = all_regions_filter.filter_and_sort(&items);
    assert_eq!(all_res.len(), 6);
    let all_names: Vec<&str> = all_res.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(all_names, vec![
        "LanGerBus01_Busch",
        "LanGerNad01_Tanne",
        "LanGerNad02_Tanne",
        "LanItaBus01_Busch",
        "LanItaZyp01_Zypresse",
        "LanKelSte01_Stein",
    ]);
}

#[test]
fn test_nature_species_mixing_isolates_region_and_category() {
    let items = vec![
        NatureCatalogItem::new(1, "LanGerNad01_Tanne"),
        NatureCatalogItem::new(2, "LanGerEic01_Eiche"),
        NatureCatalogItem::new(3, "LanGerBir01_Birke"),
        NatureCatalogItem::new(4, "LanGerBus01_Busch"),       // Ger Bush (different category)
        NatureCatalogItem::new(5, "LanItaZyp01_Zypresse"),   // Ita Tree (different region)
        NatureCatalogItem::new(6, "LanKelBir01_Birke"),      // Kel Tree (different region)
    ];

    let selected_tree = &items[0]; // LanGerNad01

    // Matching species must only be Germanic Trees
    let candidates = NatureSpeciesMixer::matching_species(selected_tree, &items);
    assert_eq!(candidates.len(), 3);
    for cand in &candidates {
        assert_eq!(cand.region(), "Ger");
        assert_eq!(cand.category(), NatureCategory::Tree);
    }

    // When mix_species is false, always returns selected
    let mut rng = DotNetRandom::new(42);
    let unmixed = NatureSpeciesMixer::pick_species(selected_tree, &items, false, &mut rng);
    assert_eq!(unmixed.type_id, selected_tree.type_id);

    // When mix_species is true, randomly samples only from candidates
    let mut sampled_ids = BTreeSet::new();
    for _ in 0..50 {
        let picked = NatureSpeciesMixer::pick_species(selected_tree, &items, true, &mut rng);
        assert_eq!(picked.region(), "Ger");
        assert_eq!(picked.category(), NatureCategory::Tree);
        sampled_ids.insert(picked.type_id);
    }
    // Should have sampled from the available 3 species and never picked 4, 5, or 6
    assert!(sampled_ids.contains(&1));
    assert!(sampled_ids.contains(&2) || sampled_ids.contains(&3));
    assert!(!sampled_ids.contains(&4));
    assert!(!sampled_ids.contains(&5));
    assert!(!sampled_ids.contains(&6));
}

#[test]
fn test_nature_brush_remover_guards_linked_and_non_landscape() {
    let level_objects = vec![
        // Normal landscape tree within radius
        PlacedNatureObject::new(101, 1, "LanGerNad01_Tanne", 100.0, 100.0, false),
        // Linked tree within radius (must NOT be removed)
        PlacedNatureObject::new(102, 1, "LanGerNad01_Tanne", 102.0, 102.0, true),
        // Building within radius (must NOT be removed)
        PlacedNatureObject::new(103, 99, "BauGerHau00_Haupthaus", 105.0, 105.0, false),
        // Normal landscape tree outside radius (must NOT be removed)
        PlacedNatureObject::new(104, 1, "LanGerNad01_Tanne", 500.0, 500.0, false),
    ];

    let pending_additions = vec![
        // Pending addition within radius
        NatureAddition::new(1, "LanGerNad01_Tanne", 103.0, 0.0, 103.0, 0.0),
        // Pending addition outside radius
        NatureAddition::new(1, "LanGerNad01_Tanne", 600.0, 0.0, 600.0, 0.0),
    ];

    let mut already_removed = BTreeSet::new();
    already_removed.insert(999);

    let (removed_slots, removed_adds) = NatureBrushRemover::plan_removal(
        (100.0, 100.0),
        20.0,
        &level_objects,
        &pending_additions,
        &already_removed,
    );

    // Only slot 101 should be removed
    assert_eq!(removed_slots, vec![101]);
    // Only the first addition should be removed
    assert_eq!(removed_adds.len(), 1);
    assert_eq!(removed_adds[0].x, 103.0);
}

#[test]
fn test_nature_brush_painter_and_session_roundtrip() {
    let items = vec![
        NatureCatalogItem::new(1, "LanGerNad01_Tanne"),
        NatureCatalogItem::new(2, "LanGerEic01_Eiche"),
    ];
    let selected = &items[0];

    let mut rng = DotNetRandom::new(1337);
    let height_sampler = |x: f32, z: f32| -> f32 { (x + z) * 0.01 };

    // Single tile brush (brush_size = 1) plants exactly 1 addition
    let additions = NatureBrushPainter::plan_plant(
        10,
        10,
        1,
        NatureDensity::Normal,
        64,
        64.0,
        &[],
        &[],
        &BTreeSet::new(),
        selected,
        &items,
        false,
        height_sampler,
        &mut rng,
    );

    assert_eq!(additions.len(), 1);
    assert_eq!(additions[0].type_id, 1);
    assert_eq!(additions[0].name, "LanGerNad01_Tanne");
    assert!(additions[0].x >= 10.0 * 64.0 && additions[0].x <= 11.0 * 64.0);
    assert!(additions[0].z >= 10.0 * 64.0 && additions[0].z <= 11.0 * 64.0);
    assert_eq!(additions[0].y, (additions[0].x + additions[0].z) * 0.01);
    assert!(additions[0].rotation >= 0.0 && additions[0].rotation < std::f32::consts::PI * 2.0);

    // Apply to NatureEditSession
    let mut session = NatureEditSession::new();
    assert!(!session.is_dirty());
    assert!(session.plant_many(&additions).unwrap());
    assert!(session.is_dirty());
    assert_eq!(session.additions().len(), 1);

    // Undo should clear it
    assert!(session.undo());
    assert_eq!(session.additions().len(), 0);
    assert!(!session.is_dirty());

    // Redo should restore it
    assert!(session.redo());
    assert_eq!(session.additions().len(), 1);
    assert!(session.is_dirty());
}

#[test]
fn test_csharp_oracle_single_and_large_brush_placement_parity() {
    let items = vec![
        NatureCatalogItem::new(1, "langerNad01"),
        NatureCatalogItem::new(2, "LanGerNad02"),
        NatureCatalogItem::new(3, "LANITA01_Zyp"),
        NatureCatalogItem::new(4, "LanKelBir01"),
    ];
    let selected = &items[0]; // langerNad01

    let height_sampler = |x: f32, z: f32| -> f32 { (x + z) * 0.01 };

    // 1. Single tile brush (brush_size = 1), mix = false, seed = 1337
    // C# Oracle output from NatureOracle (net8.0 System.Random XoshiroImpl):
    // name=langerNad01 x=2715.0249 y=54.5133 z=2736.3032 rot=5.848829
    {
        let mut rng = DotNetRandom::new(1337);
        let additions = NatureBrushPainter::plan_plant(
            10,
            10,
            1,
            NatureDensity::Normal,
            64,
            256.0,
            &[],
            &[],
            &BTreeSet::new(),
            selected,
            &items,
            false,
            height_sampler,
            &mut rng,
        );

        assert_eq!(additions.len(), 1);
        let p = &additions[0];
        assert_eq!(p.name, "langerNad01");
        assert!((p.x - 2715.0249).abs() < 1e-3, "x mismatch: {}", p.x);
        assert!((p.y - 54.5133).abs() < 1e-3, "y mismatch: {}", p.y);
        assert!((p.z - 2736.3032).abs() < 1e-3, "z mismatch: {}", p.z);
        assert!((p.rotation - 5.848829).abs() < 1e-3, "rot mismatch: {}", p.rotation);
    }

    // 2. Single tile brush (brush_size = 1), mix = true, seed = 1337
    // When only one candidate exists (selected is the only Ger tree),
    // C# Oracle output matches Single_NoMix exactly:
    {
        let single_candidate_items = vec![items[0].clone(), items[2].clone(), items[3].clone()];
        let mut rng = DotNetRandom::new(1337);
        let additions = NatureBrushPainter::plan_plant(
            10,
            10,
            1,
            NatureDensity::Normal,
            64,
            256.0,
            &[],
            &[],
            &BTreeSet::new(),
            selected,
            &single_candidate_items,
            true,
            height_sampler,
            &mut rng,
        );

        assert_eq!(additions.len(), 1);
        let p = &additions[0];
        assert_eq!(p.name, "langerNad01");
        assert!((p.x - 2715.0249).abs() < 1e-3, "x mismatch: {}", p.x);
        assert!((p.y - 54.5133).abs() < 1e-3, "y mismatch: {}", p.y);
        assert!((p.z - 2736.3032).abs() < 1e-3, "z mismatch: {}", p.z);
        assert!((p.rotation - 5.848829).abs() < 1e-3, "rot mismatch: {}", p.rotation);
    }

    // 3. Large brush (brush_size = 5), mix = false, seed = 2026
    // C# Oracle output from NatureOracle (6 additions):
    let expected_oracle_large = [
        ("langerNad01", 5328.0024f32, 101.6478f32, 4836.7808f32, 3.832184f32),
        ("langerNad01", 5879.2866, 108.2512, 4945.8335, 2.784844),
        ("langerNad01", 5679.6494, 109.8929, 5309.6387, 2.035666),
        ("langerNad01", 4930.4146, 104.7812, 5547.7104, 3.218655),
        ("langerNad01", 5350.2695, 107.5977, 5409.4976, 4.734788),
        ("langerNad01", 5289.4985, 110.5660, 5767.1006, 2.275949),
    ];

    {
        let mut rng = DotNetRandom::new(2026);
        let additions = NatureBrushPainter::plan_plant(
            20,
            20,
            5,
            NatureDensity::Normal,
            64,
            256.0,
            &[],
            &[],
            &BTreeSet::new(),
            selected,
            &items,
            false,
            height_sampler,
            &mut rng,
        );

        assert_eq!(additions.len(), expected_oracle_large.len());
        for (i, p) in additions.iter().enumerate() {
            let (exp_name, exp_x, exp_y, exp_z, exp_rot) = expected_oracle_large[i];
            assert_eq!(p.name, exp_name, "Tree {} name mismatch", i);
            assert!((p.x - exp_x).abs() < 1e-3, "Tree {} x mismatch: got {}, exp {}", i, p.x, exp_x);
            assert!((p.y - exp_y).abs() < 1e-3, "Tree {} y mismatch: got {}, exp {}", i, p.y, exp_y);
            assert!((p.z - exp_z).abs() < 1e-3, "Tree {} z mismatch: got {}, exp {}", i, p.z, exp_z);
            assert!((p.rotation - exp_rot).abs() < 1e-3, "Tree {} rot mismatch: got {}, exp {}", i, p.rotation, exp_rot);
        }
    }
}

#[test]
fn test_dominant_regions_case_insensitivity_and_multiregion() {
    let items = vec![
        NatureCatalogItem::new(1, "langerNad01"),
        NatureCatalogItem::new(2, "LanGerNad02"),
        NatureCatalogItem::new(3, "LANITA01_Zyp"),
        NatureCatalogItem::new(4, "LanKelBir01"),
    ];

    // Filter with lowercase "ger"
    let mut lower_ger = BTreeSet::new();
    lower_ger.insert("ger".to_string());
    let filter = NaturePaletteFilter::new()
        .with_dominant_regions(lower_ger)
        .with_show_other_regions(false);

    let res = filter.filter_and_sort(&items);
    let names: Vec<&str> = res.iter().map(|i| i.name.as_str()).collect();
    // Must find both German trees despite case variations
    assert_eq!(names, vec!["langerNad01", "LanGerNad02"]);

    // Filter with mixed case "Ger" and "Ita"
    let mut mixed_regions = BTreeSet::new();
    mixed_regions.insert("Ger".to_string());
    mixed_regions.insert("iTa".to_string());
    let filter_mixed = NaturePaletteFilter::new()
        .with_dominant_regions(mixed_regions)
        .with_show_other_regions(false);

    let res_mixed = filter_mixed.filter_and_sort(&items);
    let names_mixed: Vec<&str> = res_mixed.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names_mixed, vec!["langerNad01", "LanGerNad02", "LANITA01_Zyp"]);
}

#[test]
fn test_nature_region_and_category_utf8_boundary_safety() {
    // Non-ASCII and multi-byte UTF-8 inputs must not panic
    let crossing_emoji = "Lan\u{1F600}Nad01";
    assert_eq!(nature_region(crossing_emoji), "");
    assert_eq!(nature_category(crossing_emoji), NatureCategory::Other);
    assert_eq!(nature_region("地景_樹木_01"), "景");
    assert_eq!(nature_category("地景_樹木_01"), NatureCategory::Other);
    assert_eq!(nature_region("short"), "");
    assert_eq!(nature_category("short"), NatureCategory::Other);
    assert_eq!(nature_region(""), "");
    assert_eq!(nature_category(""), NatureCategory::Other);
}

#[test]
fn test_nature_brush_remover_plan_removal_tile() {
    // Tile (10, 10), brushSize = 3, tileWorld = 256.0
    // Center = (10.5 * 256.0, 10.5 * 256.0) = (2688.0, 2688.0)
    // Radius = (3/2 + 0.26) * 256.0 = 1.76 * 256.0 = 450.56
    let center_x = 2688.0f32;
    let center_z = 2688.0f32;

    let level_objects = vec![
        // Inside radius (dist ~ 100)
        PlacedNatureObject::new(1, 10, "LanGerNad01", center_x + 70.0, center_z + 70.0, false),
        // Inside radius but linked (protected)
        PlacedNatureObject::new(2, 10, "LanGerNad01", center_x + 50.0, center_z + 50.0, true),
        // Outside radius (dist ~ 500 > 450.56)
        PlacedNatureObject::new(3, 10, "LanGerNad01", center_x + 350.0, center_z + 360.0, false),
    ];

    let pending_additions = vec![
        // Inside radius
        NatureAddition::new(10, "LanGerNad01".to_string(), center_x + 30.0, 0.0, center_z + 30.0, 0.0),
        // Outside radius
        NatureAddition::new(10, "LanGerNad01".to_string(), center_x + 400.0, 0.0, center_z + 400.0, 0.0),
    ];

    let (removed_slots, removed_adds) = NatureBrushRemover::plan_removal_tile(
        10,
        10,
        3,
        256.0,
        &level_objects,
        &pending_additions,
        &BTreeSet::new(),
    );

    assert_eq!(removed_slots, vec![1]);
    assert_eq!(removed_adds.len(), 1);
    assert_eq!(removed_adds[0].x, center_x + 30.0);
}

