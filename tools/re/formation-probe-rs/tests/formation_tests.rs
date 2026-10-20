use formation_probe::formation_layout::{FormationLayout, FormationLayoutDefinition};
use formation_probe::run_layout;
use std::fs;

const NATIVE_CSV_ROW0: &str = "\
[FormationDefault]\r\n\
;idx,activ,keys,x1 00,z1 00,x2 00,z2 00,...\r\n\
0, 1, 9, 0.05,-0.05, 0.20, 0.05,-0.20, 0.15,-0.05, 0.05, 0.20,-0.20, 0.05,-0.15,-0.10,-0.05,-0.20,-0.20, 0.30,-0.15, 0.40, 0.10,-0.25, 0.25,-0.40, 0.05, 0.05, 0.30, 0.30, 0.20, 0.34,-0.25, 0.10,-0.34,-0.20,-0.34,-0.40,-0.15, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 0.0, 180.0, All_Haufen\r\n";

#[test]
fn test_native_default_matches_embedded() {
    let def = FormationLayoutDefinition::from_form_def_text(NATIVE_CSV_ROW0, 0).unwrap();
    assert_eq!(def.segments().len(), 9);
    assert!(def.matches_embedded());

    let default = FormationLayoutDefinition::native_default();
    assert!(default.matches_embedded());
    assert_eq!(def.segments(), default.segments());
}

#[test]
fn test_matches_embedded_uses_exact_csharp_record_equality() {
    let mut segments = FormationLayoutDefinition::native_default()
        .segments()
        .to_vec();
    segments[0].start.0 += 1e-6;
    let near_match = FormationLayoutDefinition::new(segments).unwrap();
    assert!(!near_match.matches_embedded());
}

#[test]
fn test_create_counts_1_to_20() {
    let def = FormationLayoutDefinition::native_default();
    for count in 1..=20 {
        let offsets = FormationLayout::create(count, 0.0, &def, 320.0).unwrap();
        assert_eq!(offsets.len(), count);
        for pt in &offsets {
            assert!(pt.x.is_finite());
            assert!(pt.z.is_finite());
        }
    }
}

#[test]
fn test_create_boundary_conditions() {
    let def = FormationLayoutDefinition::native_default();
    assert_eq!(FormationLayout::banner_offset(), (0.0, 0.0));
    assert!(FormationLayout::create(0, 0.0, &def, 320.0)
        .unwrap()
        .is_empty());
    assert!(FormationLayout::create(21, 0.0, &def, 320.0).is_err());
    assert!(FormationLayout::create(5, f32::NAN, &def, 320.0).is_err());
    assert!(FormationLayout::create(5, 0.0, &def, 0.0).is_err());
    assert!(FormationLayout::create(5, 0.0, &def, -10.0).is_err());
}

#[test]
fn test_from_form_def_text_invalid() {
    // Missing [FormationDefault] section
    assert!(FormationLayoutDefinition::from_form_def_text("0,1,9,0.1,0.2", 0).is_err());
    // Out of range ID
    assert!(FormationLayoutDefinition::from_form_def_text(NATIVE_CSV_ROW0, -1).is_err());
    assert!(FormationLayoutDefinition::from_form_def_text(NATIVE_CSV_ROW0, 100).is_err());
    // ID not present
    assert!(FormationLayoutDefinition::from_form_def_text(NATIVE_CSV_ROW0, 5).is_err());
    // Insufficient fields (< 83)
    let truncated = "[FormationDefault]\n0, 1, 9, 0.1, 0.2, 0.3, 0.4\n";
    assert!(FormationLayoutDefinition::from_form_def_text(truncated, 0).is_err());
    // Duplicate ID
    let duplicate = format!("{NATIVE_CSV_ROW0}\n0, 1, 9, 0.1, 0.2, 0.3, 0.4\n");
    assert!(FormationLayoutDefinition::from_form_def_text(&duplicate, 0).is_err());
}

#[test]
fn test_run_layout_synthetic_file() {
    let temp_dir =
        std::env::temp_dir().join(format!("formation_layout_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let uncompressed_path = temp_dir.join("formdef_raw.txt");
    fs::write(&uncompressed_path, NATIVE_CSV_ROW0).unwrap();

    let report = run_layout(&uncompressed_path).unwrap();
    assert_eq!(report.segment_count, 9);
    assert!(report.matches_embedded);
    assert_eq!(report.layouts.len(), 20);
    for (i, l) in report.layouts.iter().enumerate() {
        assert_eq!(l.count, i + 1);
        assert_eq!(l.offsets.len(), i + 1);
    }

    // Now wrap in synthetic PFIL
    let pfil_data = formation_probe::pfil::create_synthetic_pfil(NATIVE_CSV_ROW0.as_bytes());
    let pfil_path = temp_dir.join("formdef.dau");
    fs::write(&pfil_path, pfil_data).unwrap();

    let pfil_report = run_layout(&pfil_path).unwrap();
    assert_eq!(pfil_report.segment_count, 9);
    assert!(pfil_report.matches_embedded);
    assert_eq!(pfil_report.layouts, report.layouts);

    let _ = fs::remove_dir_all(&temp_dir);
}
