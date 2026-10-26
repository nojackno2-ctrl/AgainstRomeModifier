use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan, FEATURE_TYPES, MAX_FEATURES};

const MATERIAL_IDS: &[&str] = &["Grass", "Sand"];

#[test]
fn test_parse_accepts_json_fences_and_leading_prose() {
    let json = r#"{"baseHeight":70,"features":[]}"#;
    for response in &[
        format!("```json\n{}\n```", json),
        format!("Here is your plan:\n{}", json),
        format!("Here is your plan:\n```json\n{}\n```", json),
    ] {
        let plan = AiMapPlan::parse(response, MATERIAL_IDS).unwrap();
        assert_eq!(Some(70), plan.base_height);
    }
}

#[test]
fn test_parse_normalizes_coordinates_radii_amounts_and_base_height() {
    let source = AiMapPlan {
        base_height: Some(999),
        features: vec![
            AiMapFeature {
                feature_type: Some(" HILL ".to_string()),
                x: -9,
                y: 99,
                x2: Some(99),
                y2: Some(-9),
                radius: 99,
                amount: Some(999),
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("lake".to_string()),
                x: 99,
                y: -9,
                x2: Some(-9),
                y2: Some(99),
                radius: 0,
                amount: Some(-9),
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("plateau".to_string()),
                radius: -9,
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("hill".to_string()),
                radius: 1,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let serialized = serde_json::to_string(&source).unwrap();
    let plan = AiMapPlan::parse(&serialized, MATERIAL_IDS).unwrap();
    assert_eq!(Some(255), plan.base_height);

    let first = &plan.features[0];
    let second = &plan.features[1];
    assert_eq!("hill", first.feature_type.as_deref().unwrap());
    assert_eq!((0, 63, Some(63), Some(0)), (first.x, first.y, first.x2, first.y2));
    assert_eq!((63, 0, Some(0), Some(63)), (second.x, second.y, second.x2, second.y2));
    assert_eq!(Some(255), first.amount);
    assert_eq!(Some(5), second.amount);

    let radii: Vec<i32> = plan.features.iter().map(|f| f.radius).collect();
    assert_eq!(vec![32, 4, 4, 1], radii);

    assert_eq!(
        Some(0),
        AiMapPlan::parse(r#"{"baseHeight":-9}"#, MATERIAL_IDS).unwrap().base_height
    );
}

#[test]
fn lake_and_river_depths_default_and_clamp_to_documented_height_units() {
    let plan = AiMapPlan::parse(
        r#"{"features":[{"type":"lake","x":10,"y":10},{"type":"river","x":1,"y":1,"x2":20,"y2":20,"amount":99}]}"#,
        MATERIAL_IDS,
    )
    .unwrap();
    assert_eq!(Some(16), plan.features[0].amount);
    assert_eq!(Some(30), plan.features[1].amount);
}

#[test]
fn test_parse_drops_unknown_features_and_materials_and_preserves_canonical_ids() {
    let raw = r#"
    {"baseMaterial":" sand ","features":[
      {"type":"volcano"},
      {"type":"material","material":"Unknown"},
      {"type":"material"},
      {"type":" MATERIAL ","material":" grass "},
      {"type":"hill","material":"SAND"}
    ]}
    "#;
    let plan = AiMapPlan::parse(raw, MATERIAL_IDS).unwrap();
    assert_eq!(Some("Sand".to_string()), plan.base_material);
    assert_eq!(2, plan.features.len());
    assert_eq!("material", plan.features[0].feature_type.as_deref().unwrap());
    assert_eq!(Some("Grass".to_string()), plan.features[0].material);
    assert_eq!(Some("Sand".to_string()), plan.features[1].material);

    assert_eq!(
        None,
        AiMapPlan::parse(r#"{"baseHeight":70,"baseMaterial":"Unknown"}"#, MATERIAL_IDS)
            .unwrap()
            .base_material
    );
}

#[test]
fn test_parse_truncates_features_to_max_features() {
    let source = AiMapPlan {
        features: (0..MAX_FEATURES + 10)
            .map(|i| AiMapFeature {
                feature_type: Some("hill".to_string()),
                x: i as i32,
                radius: 2,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };

    let serialized = serde_json::to_string(&source).unwrap();
    let plan = AiMapPlan::parse(&serialized, MATERIAL_IDS).unwrap();
    assert_eq!(MAX_FEATURES, plan.features.len());
    let xs: Vec<i32> = plan.features.iter().map(|f| f.x).collect();
    let expected: Vec<i32> = (0..MAX_FEATURES as i32).collect();
    assert_eq!(expected, xs);
}

#[test]
fn test_parse_rejects_empty_non_json_malformed_and_unusable_plans() {
    for response in &[
        "",
        " \r\n",
        "not JSON",
        "{invalid}",
        r#"{"features":[}"#,
        "{}",
        r#"{"features":[]}"#,
        r#"{"baseMaterial":"Unknown","features":[{"type":"unknown"},{"type":"material","material":"Unknown"}]}"#,
    ] {
        assert!(AiMapPlan::parse(response, MATERIAL_IDS).is_err());
    }
}

#[test]
fn test_compass_locations_translate_accurately() {
    let raw = r#"
    {"features":[
      {"type":"hill","location":"north","radius":4},
      {"type":"lake","location":"south","radius":4},
      {"type":"ridge","location":"west","toLocation":"east","radius":2}
    ]}
    "#;
    let plan = AiMapPlan::parse(raw, MATERIAL_IDS).unwrap();
    assert_eq!(3, plan.features.len());
    assert_eq!((32, 9), (plan.features[0].x, plan.features[0].y));
    assert_eq!((32, 55), (plan.features[1].x, plan.features[1].y));
    assert_eq!((9, 32), (plan.features[2].x, plan.features[2].y));
    assert_eq!((Some(55), Some(32)), (plan.features[2].x2, plan.features[2].y2));
}

#[test]
fn test_json_schema_serializes_feature_types_and_material_ids() {
    let schema = AiMapPlan::json_schema(MATERIAL_IDS);
    assert!(schema.is_object());
    let features_prop = &schema["properties"]["features"]["items"]["properties"];
    let types = features_prop["type"]["enum"].as_array().unwrap();
    let types_str: Vec<&str> = types.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(FEATURE_TYPES, types_str.as_slice());

    let mats = features_prop["material"]["enum"].as_array().unwrap();
    let mats_str: Vec<&str> = mats.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(MATERIAL_IDS, mats_str.as_slice());
}

#[test]
fn json_schema_omits_material_fields_when_map_has_no_material_catalog() {
    let schema = AiMapPlan::json_schema(&[]);
    let properties = schema["properties"].as_object().unwrap();
    assert!(!properties.contains_key("baseMaterial"));
    let feature_properties = properties["features"]["items"]["properties"]
        .as_object()
        .unwrap();
    assert!(!feature_properties.contains_key("material"));
    let serialized = serde_json::to_string(&schema).unwrap();
    assert!(!serialized.contains("\"enum\":[]"));
}
