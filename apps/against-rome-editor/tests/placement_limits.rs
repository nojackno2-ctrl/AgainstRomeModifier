use against_rome_editor_app::placement_ui::{
    placement_category, validate_squad_values, validate_team_for_name, PlacementCategory,
};

#[test]
fn categories_follow_name_prefixes() {
    assert_eq!(placement_category("FigRomLeg01"), PlacementCategory::Squad);
    assert_eq!(placement_category("FigTieWolf"), PlacementCategory::Animal);
    assert_eq!(placement_category("VerRomIco01"), PlacementCategory::UnitGroup);
    assert_eq!(placement_category("BauRomHaus"), PlacementCategory::Other);
}

#[test]
fn squad_team_is_0_to_7_and_others_minus1_to_15() {
    assert!(validate_team_for_name("FigRomLeg01", 7).is_ok());
    assert!(validate_team_for_name("FigRomLeg01", 8).is_err());
    assert!(validate_team_for_name("FigRomLeg01", -1).is_err());
    assert!(validate_team_for_name("BauRomHaus", 15).is_ok());
    assert!(validate_team_for_name("BauRomHaus", -1).is_ok());
    assert!(validate_team_for_name("BauRomHaus", 16).is_err());
}

#[test]
fn squad_unit_count_is_1_to_20_and_non_figures_force_zero() {
    assert_eq!(validate_squad_values("FigRomLeg01", 2, 20).unwrap(), (2, 20));
    assert!(validate_squad_values("FigRomLeg01", 2, 0).is_err());
    assert!(validate_squad_values("FigRomLeg01", 2, 21).is_err());
    assert!(validate_squad_values("FigRomLeg01", 9, 5).is_err());
    assert_eq!(validate_squad_values("BauRomHaus", 3, 9).unwrap(), (3, 0));
    assert_eq!(validate_squad_values("FigTieWolf", 3, 0).unwrap(), (3, 0));
}
