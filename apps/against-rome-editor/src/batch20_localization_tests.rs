//! 批次 20 的無視窗顯示驗證。
use eframe::egui;

pub(crate) fn texts(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn collect(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => out.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in shapes {
        collect(&shape.shape, &mut out);
    }
    out.join("\n")
}

pub(crate) fn render(mut draw: impl FnMut(&egui::Context)) -> String {
    let ctx = egui::Context::default();
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(3000.0, 5000.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(input(), |ctx| draw(ctx));
    texts(&ctx.run(input(), |ctx| draw(ctx)).shapes)
}

/// 透過實際指標事件點擊，並在視窗內容超高時捲動尋找按鈕。
pub(crate) fn click(mut draw: impl FnMut(&egui::Context), label: &str) {
    let ctx = egui::Context::default();
    let mut frame = |events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(3000.0, 5000.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| draw(ctx),
        )
    };
    frame(vec![]);
    let mut output = frame(vec![]);
    let mut pos = None;
    for _ in 0..8 {
        pos = output.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == label => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        });
        if pos.is_some() {
            break;
        }
        output = frame(vec![
            egui::Event::PointerMoved(egui::pos2(300.0, 300.0)),
            egui::Event::Scroll(egui::vec2(0.0, -1000.0)),
        ]);
    }
    let pos = pos.unwrap_or_else(|| panic!("找不到按鈕：{label}\n{}", texts(&output.shapes)));
    for pressed in [true, false] {
        frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
}
