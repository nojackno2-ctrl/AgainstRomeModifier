use against_rome_modifier::{apply_file, Patch, FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

const PATCHES: [Patch; 3] = [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20];
const USAGE: &str = "against-rome-modifier [inspect] EXE\nagainst-rome-modifier apply EXE --backup PATH (--enable FEATURE | --disable FEATURE)...\nFeatures: focus, roman-endless, unit-recruit-20\ninspect is read-only. apply requires a new backup path; installed Program Files targets are refused.";

fn next(args: &mut impl Iterator<Item = OsString>, label: &str) -> Result<OsString, String> {
    args.next().ok_or_else(|| format!("missing {label}"))
}

fn allowed_target(path: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = env::var_os(variable) {
            if let Ok(root) = fs::canonicalize(root) {
                if is_within(&canonical, &root) {
                    return Err(
                        "apply refuses installed Program Files targets; work on an explicit copy"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn is_within(path: &Path, root: &Path) -> bool {
    // Inputs are canonical paths. Windows path spelling is case-insensitive.
    let target = path.to_string_lossy().to_lowercase();
    let prefix = root.to_string_lossy().to_lowercase();
    target == prefix || target.starts_with(&(prefix + "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn program_directory_boundary_is_case_insensitive() {
        assert!(is_within(
            Path::new(r"C:\Program Files\game\game.exe"),
            Path::new(r"c:\PROGRAM FILES")
        ));
        assert!(!is_within(
            Path::new(r"C:\Program Files Copy\game.exe"),
            Path::new(r"C:\Program Files")
        ));
    }
}

fn run() -> Result<(), String> {
    let raw: Vec<String> = env::args().skip(1).collect();
    if against_rome_modifier::cli::is_cli_invocation(&raw)
        && !matches!(raw.first().map(String::as_str), Some("features" | "troops" | "inspect" | "apply") if !raw.iter().any(|a| a == "--cli" || a == "--json" || a == "--game"))
    {
        let code = against_rome_modifier::cli::run_cli(
            &raw,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        );
        std::process::exit(code);
    }
    let mut args = env::args_os().skip(1);
    let first = next(&mut args, "command or EXE path")?;
    if first == "--help" || first == "-h" {
        if args.next().is_some() {
            return Err("unexpected argument after help".into());
        }
        println!("{USAGE}");
        return Ok(());
    }
    let apply = first == "apply";
    let first_str = first.to_string_lossy();
    if first_str == "features" {
        for f in against_rome_modifier::FeatureRegistry::ALL {
            println!("{}: {:?} ({:?})", f.id, f.category, f.control_kind);
        }
        return Ok(());
    }
    if first_str == "troops" {
        for key in against_rome_modifier::TroopConfig::sorted_units() {
            let meta = against_rome_modifier::TroopConfig::get_metadata(key).unwrap();
            let name_zh = against_rome_modifier::TroopConfig::get_name_zh(key).unwrap_or("");
            println!(
                "{} ({}) - {} [{}/{}]",
                key, name_zh, meta.faction, meta.tier, meta.unit_type
            );
        }
        return Ok(());
    }
    let path = if apply || first == "inspect" {
        PathBuf::from(next(&mut args, "EXE path")?)
    } else {
        if first.to_string_lossy().starts_with('-') {
            return Err("unknown command option".into());
        }
        PathBuf::from(first)
    };
    if !apply {
        if args.next().is_some() {
            return Err("inspect accepts exactly one EXE path".into());
        }
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        for p in PATCHES {
            println!("{}: {:?}", p.name, p.detect(&bytes));
        }
        return Ok(());
    }
    let mut backup = None;
    let mut selections = Vec::new();
    while let Some(option) = args.next() {
        if option == "--backup" {
            if backup.is_some() {
                return Err("duplicate --backup".into());
            }
            backup = Some(PathBuf::from(next(&mut args, "backup path")?));
        } else if option == "--enable" || option == "--disable" {
            let name = next(&mut args, "feature name")?;
            let patch = PATCHES
                .iter()
                .find(|p| name == p.name)
                .copied()
                .ok_or_else(|| format!("unknown feature: {}", name.to_string_lossy()))?;
            if selections
                .iter()
                .any(|(p, _): &(Patch, bool)| p.name == patch.name)
            {
                return Err(format!("duplicate feature: {}", patch.name));
            }
            selections.push((patch, option == "--enable"));
        } else {
            return Err(format!("unknown argument: {}", option.to_string_lossy()));
        }
    }
    let backup = backup.ok_or("apply requires --backup PATH")?;
    if selections.is_empty() {
        return Err("apply requires at least one --enable/--disable feature".into());
    }
    allowed_target(&path)?;
    let changed = apply_file(&path, &backup, &selections).map_err(|e| e.to_string())?;
    println!(
        "{}",
        if changed {
            "Applied; original bytes saved to backup."
        } else {
            "Already in requested state; no files changed."
        }
    );
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}\n\n{USAGE}");
        std::process::exit(1);
    }
}
