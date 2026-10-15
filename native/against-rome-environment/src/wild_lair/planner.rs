//! Resource regeneration planning and editor addition.

use super::models::{NativeResourceDefinition, NativeResourceKind, TimedResourceReplacement};
use crate::nature::engine::NatureAddition;

/// Plans explicit replacements of observed resources.
pub struct ResourceRegenerationPlanner;

impl ResourceRegenerationPlanner {
    pub fn known_resources() -> &'static [NativeResourceDefinition] {
        static RESOURCES: std::sync::OnceLock<Vec<NativeResourceDefinition>> = std::sync::OnceLock::new();
        RESOURCES.get_or_init(|| {
            vec![
                NativeResourceDefinition::new("LanGerSte00_1Stein", NativeResourceKind::Stone, 0),
                NativeResourceDefinition::new("LanGerSte01_1Stein", NativeResourceKind::Stone, 1),
                NativeResourceDefinition::new("LanGerSte02_1Stein", NativeResourceKind::Stone, 2),
                NativeResourceDefinition::with_alias("BauGerMin00_Mine", NativeResourceKind::MineBuilding, 43, "GER_MIN00"),
                NativeResourceDefinition::new("LanGerNad00_Tanne_gross", NativeResourceKind::Tree, 46),
                NativeResourceDefinition::new("LanGerNad01_Tanne_gross", NativeResourceKind::Tree, 80),
                NativeResourceDefinition::new("LanGerNad02_Tanne_gross", NativeResourceKind::Tree, 81),
                NativeResourceDefinition::new("LanGerNad03_Tanne_gross", NativeResourceKind::Tree, 82),
                NativeResourceDefinition::new("LanGerNad04_Tanne_gross", NativeResourceKind::Tree, 83),
                NativeResourceDefinition::new("LanGerNad05_Tanne_gross", NativeResourceKind::Tree, 84),
                NativeResourceDefinition::new("LanGerNad06_Tanne_klein", NativeResourceKind::Tree, 85),
                NativeResourceDefinition::new("LanGerNad07_Tanne_klein", NativeResourceKind::Tree, 86),
                NativeResourceDefinition::new("LanGerNad08_Tanne_klein", NativeResourceKind::Tree, 87),
                NativeResourceDefinition::new("LanGerNad09_Tanne_klein", NativeResourceKind::Tree, 88),
                NativeResourceDefinition::new("LanGerNad10_Tanne_klein", NativeResourceKind::Tree, 89),
                NativeResourceDefinition::new("LanGerNad11_Tanne_klein", NativeResourceKind::Tree, 90),
                NativeResourceDefinition::new("LanGerNad12_Tanne_gross", NativeResourceKind::Tree, 126),
                NativeResourceDefinition::new("LanGerNad13_Tanne_gross", NativeResourceKind::Tree, 127),
                NativeResourceDefinition::new("LanGerNad14_Tanne_gross", NativeResourceKind::Tree, 128),
                NativeResourceDefinition::new("LanGerNad15_Tanne_gross", NativeResourceKind::Tree, 129),
                NativeResourceDefinition::new("LanGerNad16_Tanne_gross", NativeResourceKind::Tree, 130),
                NativeResourceDefinition::new("LanGerNad17_Tanne_gross", NativeResourceKind::Tree, 131),
                NativeResourceDefinition::new("LanGerNad18_Tanne_klein", NativeResourceKind::Tree, 132),
                NativeResourceDefinition::new("LanGerNad19_Tanne_klein", NativeResourceKind::Tree, 133),
                NativeResourceDefinition::new("LanGerNad20_Tanne_klein", NativeResourceKind::Tree, 134),
                NativeResourceDefinition::new("LanGerNad21_Tanne_klein", NativeResourceKind::Tree, 135),
                NativeResourceDefinition::new("LanGerNad22_Tanne_klein", NativeResourceKind::Tree, 136),
                NativeResourceDefinition::new("LanGerNad23_Tanne_klein", NativeResourceKind::Tree, 137),
                NativeResourceDefinition::with_alias("BauGerGol00_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 234, "GER_GOL00"),
                NativeResourceDefinition::new("LanGerNad24_Tanne_mittel", NativeResourceKind::Tree, 320),
                NativeResourceDefinition::new("LanGerNad25_Tanne_mittel", NativeResourceKind::Tree, 321),
                NativeResourceDefinition::new("LanGerNad26_Tanne_mittel", NativeResourceKind::Tree, 322),
                NativeResourceDefinition::new("LanGerNad27_Tanne_mittel", NativeResourceKind::Tree, 323),
                NativeResourceDefinition::new("LanGerNad28_Tanne_mittel", NativeResourceKind::Tree, 324),
                NativeResourceDefinition::new("LanGerNad29_Tanne_mittel", NativeResourceKind::Tree, 325),
                NativeResourceDefinition::new("LanGerNad30_Tanne_mittel", NativeResourceKind::Tree, 326),
                NativeResourceDefinition::new("LanGerNad31_Tanne_mittel", NativeResourceKind::Tree, 327),
                NativeResourceDefinition::new("LanGerNad32_Tanne_mittel", NativeResourceKind::Tree, 328),
                NativeResourceDefinition::with_alias("BauKelMin00_Mine", NativeResourceKind::MineBuilding, 387, "KEL_MIN00"),
                NativeResourceDefinition::with_alias("BauKelGol01_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 391, "KEL_GOL01"),
                NativeResourceDefinition::with_alias("BauKelGol00_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 392, "KEL_GOL00"),
                NativeResourceDefinition::new("LanItaWei00_Weizenfeld", NativeResourceKind::Field, 529),
                NativeResourceDefinition::new("LanItaWei01_Weizenfeld", NativeResourceKind::Field, 530),
                NativeResourceDefinition::new("LanItaWei02_Weizenfeld", NativeResourceKind::Field, 531),
                NativeResourceDefinition::new("LanItaWei03_Weizenfeld", NativeResourceKind::Field, 532),
                NativeResourceDefinition::new("LanItaWei04_Weizenfeld", NativeResourceKind::Field, 533),
                NativeResourceDefinition::with_alias("BauRomGol00_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 705, "ROM_GOL00"),
                NativeResourceDefinition::with_alias("BauRomGol01_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 706, "ROM_GOL01"),
                NativeResourceDefinition::with_alias("BauRomMin00_Mine", NativeResourceKind::MineBuilding, 715, "ROM_MIN00"),
                NativeResourceDefinition::new("LanBriLau00_Laubbaum_gross", NativeResourceKind::Tree, 717),
                NativeResourceDefinition::new("LanBriLau01_Laubbaum_gross", NativeResourceKind::Tree, 718),
                NativeResourceDefinition::new("LanBriLau02_Laubbaum_gross", NativeResourceKind::Tree, 719),
                NativeResourceDefinition::new("LanBriLau03_Laubbaum_gross", NativeResourceKind::Tree, 720),
                NativeResourceDefinition::new("LanBriLau04_Laubbaum_gross", NativeResourceKind::Tree, 721),
                NativeResourceDefinition::new("LanBriLau05_Laubbaum_gross", NativeResourceKind::Tree, 722),
                NativeResourceDefinition::new("LanBriLau06_Laubbaum_gross", NativeResourceKind::Tree, 723),
                NativeResourceDefinition::new("LanBriLau07_Laubbaum_mittel", NativeResourceKind::Tree, 724),
                NativeResourceDefinition::new("LanBriLau08_Laubbaum_mittel", NativeResourceKind::Tree, 725),
                NativeResourceDefinition::with_alias("BauHunGol00_Goldschmiede", NativeResourceKind::GoldsmithBuilding, 998, "HUN_GOL00"),
                NativeResourceDefinition::with_alias("BauHunMin00_Mine", NativeResourceKind::MineBuilding, 1010, "HUN_MIN00"),
                NativeResourceDefinition::new("LanBriLau09_Laubbaum_mittel", NativeResourceKind::Tree, 1719),
                NativeResourceDefinition::new("LanBriLau10_Laubbaum_klein", NativeResourceKind::Tree, 1720),
                NativeResourceDefinition::new("LanBriLau11_Laubbaum_klein", NativeResourceKind::Tree, 1721),
                NativeResourceDefinition::new("LanBriLau12_Laubbaum_klein", NativeResourceKind::Tree, 1722),
            ]
        })
    }

    pub fn plan_replacement(
        &self,
        name_def: &str,
        world_x: f32,
        world_z: f32,
        delay_seconds: i32,
        team: i32,
    ) -> Result<TimedResourceReplacement, String> {
        let known = Self::known_resources();
        if !known.iter().any(|r| r.name_def.eq_ignore_ascii_case(name_def)) {
            return Err("Unknown resource definition; require observed objdef evidence.".to_string());
        }
        if !world_x.is_finite()
            || !world_z.is_finite()
            || !(0.0..=16383.0).contains(&world_x)
            || !(0.0..=16383.0).contains(&world_z)
        {
            return Err("Coordinates must be finite world units in 0-16383.".to_string());
        }
        if !(1..=86400).contains(&delay_seconds) {
            return Err("delay_seconds must be in 1..=86400.".to_string());
        }
        if !(0..=8).contains(&team) {
            return Err("team must be in 0..=8.".to_string());
        }
        Ok(TimedResourceReplacement {
            name_def: name_def.to_string(),
            world_x,
            world_z,
            delay_seconds,
            team,
        })
    }

    pub fn create_editor_addition(
        &self,
        request: &TimedResourceReplacement,
        template_resolver: impl Fn(&str) -> Option<i32>,
        world_y: f32,
    ) -> Result<NatureAddition, String> {
        self.plan_replacement(
            &request.name_def,
            request.world_x,
            request.world_z,
            request.delay_seconds,
            request.team,
        )?;

        let known = Self::known_resources();
        let resource = known
            .iter()
            .find(|r| r.name_def.eq_ignore_ascii_case(&request.name_def))
            .ok_or_else(|| "Unknown resource definition".to_string())?;

        if matches!(
            resource.kind,
            NativeResourceKind::MineBuilding | NativeResourceKind::GoldsmithBuilding
        ) || request.team != 8
        {
            return Err("Nature placement requires a neutral landscape template; buildings use the existing placement workflow.".to_string());
        }

        if !world_y.is_finite() {
            return Err("world_y must be finite".to_string());
        }

        let template_type_id = template_resolver(&resource.name_def)
            .ok_or_else(|| "No native template for this observed resource.".to_string())?;

        if template_type_id != resource.definition_id {
            return Err("The supplied template does not match the observed resource definition ID.".to_string());
        }

        Ok(NatureAddition::new(
            template_type_id,
            resource.name_def.clone(),
            request.world_x,
            world_y,
            request.world_z,
            0.0,
        ))
    }
}
