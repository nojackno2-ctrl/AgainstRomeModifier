//! 批次 20 的無視窗顯示驗證。
use eframe::egui;

pub(crate) fn texts(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn collect(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => out.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes { collect(shape, out); }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in shapes { collect(&shape.shape, &mut out); }
    out.join("\n")
}

pub(crate) fn render(mut draw: impl FnMut(&egui::Context)) -> String {
    let ctx = egui::Context::default();
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(3000.0, 5000.0))),
        ..Default::default()
    };
    let _ = ctx.run(input(), |ctx| draw(ctx));
    texts(&ctx.run(input(), |ctx| draw(ctx)).shapes)
}
