//! SettlementGeneratorEngine: High-level pipeline converting distributions into layout presets and JSON.

use super::layout_preset::{MapLayoutEntry, MapLayoutKind, MapLayoutPreset, MapLayoutPresets};
use super::models::MultiplayerDistributionResult;

pub struct SettlementGeneratorEngine;

impl SettlementGeneratorEngine {
    /// 將多勢力生成結果中的建築與放置單位轉為標準 Placement 類型的 MapLayoutPreset。
    pub fn to_placement_layout_preset(distribution: &MultiplayerDistributionResult) -> MapLayoutPreset {
        let mut entries = Vec::new();

        for player in &distribution.players {
            // 建築物
            for b in &player.buildings {
                entries.push(MapLayoutEntry::with_team_and_count(
                    b.type_name.clone(),
                    b.world_x,
                    b.world_z,
                    b.height_offset,
                    b.angle,
                    b.team,
                    0,
                ));
            }

            // 野生動物或放置資源
            for w in &player.wildlife_and_food {
                entries.push(MapLayoutEntry::with_team_and_count(
                    w.type_name.clone(),
                    w.world_x,
                    w.world_z,
                    w.height_offset,
                    w.angle_deg,
                    w.team,
                    w.count,
                ));
            }
        }

        MapLayoutPreset::new(MapLayoutKind::Placement, entries)
    }

    /// 將多勢力生成結果中的樹林與石礦自然地景物件轉為標準 Nature 類型的 MapLayoutPreset。
    pub fn to_nature_layout_preset(distribution: &MultiplayerDistributionResult) -> MapLayoutPreset {
        let mut entries = Vec::new();

        for player in &distribution.players {
            // 森林樹木
            for tree in &player.forest_trees {
                entries.push(MapLayoutEntry::with_team_and_count(
                    tree.type_name.clone(),
                    tree.world_x,
                    tree.world_z,
                    tree.height_offset,
                    tree.angle_deg,
                    0,
                    0,
                ));
            }

            // 採石場與石礦露頭
            for rock in &player.stone_quarries {
                entries.push(MapLayoutEntry::with_team_and_count(
                    rock.type_name.clone(),
                    rock.world_x,
                    rock.world_z,
                    rock.height_offset,
                    rock.angle_deg,
                    0,
                    0,
                ));
            }
        }

        MapLayoutPreset::new(MapLayoutKind::Nature, entries)
    }

    /// 匯出為可攜式 JSON 字串（與 .arm-layout.json 相容）。
    pub fn export_layout_jsons(
        distribution: &MultiplayerDistributionResult,
    ) -> Result<(String, String), String> {
        let placement_preset = Self::to_placement_layout_preset(distribution);
        let nature_preset = Self::to_nature_layout_preset(distribution);

        let placement_json = MapLayoutPresets::serialize(&placement_preset)?;
        let nature_json = MapLayoutPresets::serialize(&nature_preset)?;

        Ok((placement_json, nature_json))
    }
}
