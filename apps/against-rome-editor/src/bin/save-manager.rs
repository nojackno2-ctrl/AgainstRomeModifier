//! Standalone Rust Save Manager Desktop Entry (`save-manager`).
//! Direct port of the existing C# `AgainstRomeSaveManager.exe` / `SaveManagerForm.cs` workflow.

fn main() {
    if let Err(e) = against_rome_editor_app::save_manager_ui::run_standalone_entry() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
