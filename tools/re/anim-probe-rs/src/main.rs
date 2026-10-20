use std::{env, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 3 {
        eprintln!("Usage: anim-probe <alr.dat> <apt.dat> <NEW TEMP output dir>");
        return ExitCode::from(2);
    }
    let output = match anim_probe::Output::new(Path::new(&args[2])) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Output must be a NEW child directory of TEMP; existing directories are rejected. {error}");
            return ExitCode::from(2);
        }
    };
    match anim_probe::run(Path::new(&args[0]), Path::new(&args[1]), &output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("anim-probe: {error}");
            ExitCode::FAILURE
        }
    }
}
