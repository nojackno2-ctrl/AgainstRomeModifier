#[path = "../tools/app_tables.rs"]
mod generator;

fn main() -> std::process::ExitCode {
    match generator::run(std::env::args_os().skip(1)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
