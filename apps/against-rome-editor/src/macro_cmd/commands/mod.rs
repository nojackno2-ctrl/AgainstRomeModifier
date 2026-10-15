//! 編輯器巨集指令集 (Macro Commands)

pub mod nav_cmds;
pub mod placement_cmds;
pub mod system_cmds;
pub mod terrain_cmds;

use crate::macro_cmd::execution::CommandRegistry;

/// 註冊所有內建預設指令至指令登錄表。
pub fn register_default_commands(registry: &mut CommandRegistry) {
    // Terrain
    registry.register(Box::new(terrain_cmds::ElevateCommand));
    registry.register(Box::new(terrain_cmds::FlattenCommand));
    registry.register(Box::new(terrain_cmds::BrushCommand));
    registry.register(Box::new(texture_cmds::ReplaceTextureCommand));

    // Placement
    registry.register(Box::new(placement_cmds::SpawnRingCommand));
    registry.register(Box::new(placement_cmds::AlignGridCommand));
    registry.register(Box::new(placement_cmds::SelectTeamCommand));
    registry.register(Box::new(placement_cmds::SetTeamCommand));
    registry.register(Box::new(placement_cmds::ScatterCommand));

    // Nav & Diagnostics
    registry.register(Box::new(nav_cmds::HealNavMeshCommand));
    registry.register(Box::new(nav_cmds::DiagnoseCommand));

    // System
    registry.register(Box::new(system_cmds::HelpCommand));
    registry.register(Box::new(system_cmds::EchoCommand));
    registry.register(Box::new(system_cmds::RunMacroCommand));
    registry.register(Box::new(system_cmds::UndoCommand));
    registry.register(Box::new(system_cmds::RedoCommand));
    registry.register(Box::new(system_cmds::ClearCommand));
}

pub mod texture_cmds;
