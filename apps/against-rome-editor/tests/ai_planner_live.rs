//! Live gate: real Ollama generation → preview → Apply → Undo/Redo → Safe Save → reload.
//! Ignored by default (needs a local Ollama). Run with:
//! `cargo test -p against-rome-editor-app --test ai_planner_live -- --ignored --nocapture`
//! Optional env: `ARM_AI_URL` (default http://localhost:11434), `ARM_AI_MODEL`.

use against_rome_editor_app::ai_planner_ui::AiPlannerUi;
use against_rome_editor_app::bmp::BmpImage;
use against_rome_editor_app::storage::{load_map, safe_save_map};
use std::fs;
use std::time::Duration;

const SIZE: usize = 257;
/// Edit area in material tiles (4 height cells each); vertices outside it must not change.
const BOUNDS: (usize, usize, usize, usize) = (8, 8, 48, 48);

#[test]
#[ignore = "requires a local Ollama server"]
fn live_ollama_terrain_plan_preview_apply_undo_save_reload() {
    let dir = std::env::temp_dir().join(format!("arm_ai_live_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("boden.bmp"), BmpImage::new_solid(SIZE, SIZE, 0, 60, 0).encode_bmp()).unwrap();
    fs::write(dir.join("collision.bmp"), BmpImage::new_solid(SIZE - 1, SIZE - 1, 0, 0, 0).encode_bmp()).unwrap();
    let mut session = load_map(&dir).unwrap();
    let original = session.heights().to_vec();

    let mut ui = AiPlannerUi::default();
    if let Ok(url) = std::env::var("ARM_AI_URL") {
        ui.ollama_url = url;
    }
    if let Ok(model) = std::env::var("ARM_AI_MODEL") {
        ui.role_models = std::array::from_fn(|_| model.clone());
    }
    ui.role_checks = [true, false, false];
    let (bx, by, bw, bh) = BOUNDS;
    ui.bounds_text = format!("{bx},{by},{bw},{bh}");
    ui.prompt_description = "Raise one rounded hill in the middle of the area and carve a shallow valley beside it.".into();

    ui.start_generation(0.0).unwrap();
    assert!(ui.wait_generation(Duration::from_secs(20 * 60)), "generation timed out");
    eprintln!("status: {}\n{}", ui.status_message, ui.plan_summary);
    assert!(ui.current_plan.is_some(), "no plan: {}", ui.status_message);
    eprintln!("plan: {}", serde_json::to_string(ui.current_plan.as_ref().unwrap()).unwrap());
    // Preview only: generation never touches the map or its history.
    assert_eq!(session.heights(), &original[..]);
    assert!(!session.can_undo());

    let result = ui.apply_plan_to_session(&mut session, 0.0).unwrap();
    eprintln!("applied: {} samples, {} operations", result.height_samples_changed, result.operation_count());
    assert!(result.height_samples_changed > 0, "plan changed nothing");
    let applied = session.heights().to_vec();
    for y in 0..SIZE {
        for x in 0..SIZE {
            if x > (bx + bw) * 4 || y > (by + bh) * 4 || x < bx * 4 || y < by * 4 {
                assert_eq!(applied[y * SIZE + x], original[y * SIZE + x], "vertex ({x},{y}) outside the edit area changed");
            }
        }
    }

    assert!(session.undo());
    assert_eq!(session.heights(), &original[..], "one undo reverts the AI plan");
    assert!(session.redo());
    assert_eq!(session.heights(), &applied[..]);

    safe_save_map(&dir, &mut session).unwrap();
    assert!(!session.is_dirty());
    let reloaded = load_map(&dir).unwrap();
    assert_eq!(reloaded.heights(), &applied[..], "saved heights reload identically");
    fs::remove_dir_all(&dir).unwrap();
}
