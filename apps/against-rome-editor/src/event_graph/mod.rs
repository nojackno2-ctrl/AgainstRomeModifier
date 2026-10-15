//! 劇情事件圖模組 (Event Graph Module)
//!
//! 包含節點圖模型 (model)、無損雙向轉換 (converter)、靜態語法與循環檢查器 (validator) 以及 egui 互動畫布 (ui)。

pub mod converter;
pub mod model;
pub mod ui;
pub mod validator;

#[cfg(test)]
mod tests;

pub use model::{
    EventGraph, GraphDiagnostic, GraphDiagnosticSeverity, GraphEdge, GraphNode, GraphPort,
    NodeKind, PortDirection, PortType,
};
pub use ui::EventGraphUi;
