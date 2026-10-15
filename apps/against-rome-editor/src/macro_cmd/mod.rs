//! 編輯器巨集與指令模組 (Macro Command Module)
//!
//! 包含 DSL 語法解析 (parser)、模型定義 (models)、指令集 (commands) 與巨集交易執行器 (execution)。

pub mod commands;
pub mod execution;
pub mod models;
pub mod parser;

#[cfg(test)]
mod tests;

pub use execution::{CommandRegistry, CompoundMacroCommand, EditorCommand, MacroScriptRunner};
pub use models::{
    CommandExecutionContext, CommandParameter, CommandParameterType, CommandResult, LogLevel,
    MacroExecutionReport, MacroPlacedObject, MacroStepSnapshot, MapRegionCircle, MapRegionRect,
    ParsedCommand,
};
pub use parser::{get_completions, parse, substitute_variables, tokenize};
