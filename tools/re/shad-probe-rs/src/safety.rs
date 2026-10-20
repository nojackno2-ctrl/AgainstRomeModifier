//! Paths are validated even though archive entries are never extracted.
use std::{
    collections::HashSet,
    fs, io,
    path::{Component, Path, PathBuf},
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

pub fn component(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(invalid("Malformed, traversal or alias path component."));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap()
        .trim_end()
        .to_ascii_uppercase();
    // Windows 8.3 names can refer to a different long spelling of the same
    // directory/file. Avoid accepting short-name aliases as explicit paths.
    if name.contains('~') {
        return Err(invalid("Short-name alias paths are forbidden."));
    }
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    }) {
        return Err(invalid("Windows device alias path is forbidden."));
    }
    Ok(())
}

pub fn archive_path(name: &str) -> io::Result<String> {
    let name = name.strip_suffix('/').unwrap_or(name);
    for part in name.split('/') {
        component(part)?;
    }
    Ok(case_key(name))
}

/// Simple Unicode case mapping (no multi-character expansions), matching the
/// ordinal, case-insensitive filename lookup used by the C# probe.
pub fn case_key(name: &str) -> String {
    name.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            let first = upper.next().unwrap();
            if upper.next().is_none() {
                first
            } else {
                c
            }
        })
        .collect()
}

fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn absolute(path: &Path) -> io::Result<PathBuf> {
    // Do not normalize dot segments away before checking them.
    let raw = path
        .to_str()
        .ok_or_else(|| invalid("Path must be Unicode."))?;
    if raw.split(['/', '\\']).any(|v| v == "." || v == "..") {
        return Err(invalid("Traversal and dot aliases are forbidden."));
    }
    #[cfg(windows)]
    if raw.starts_with(r"\\") || raw.starts_with("//") {
        return Err(invalid("UNC/device paths are forbidden."));
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if !path.is_absolute() {
        return Err(invalid("Drive-relative paths are forbidden."));
    }
    for part in path.components() {
        match part {
            Component::Normal(v) => {
                component(v.to_str().ok_or_else(|| invalid("Path must be Unicode."))?)?
            }
            Component::RootDir | Component::Prefix(_) => (),
            _ => return Err(invalid("Path must not contain traversal or aliases.")),
        }
    }
    Ok(path)
}

fn check_chain(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if is_link(&m) => {
                return Err(invalid("Symlink/reparse point paths are forbidden."))
            }
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn input(path: &Path) -> io::Result<PathBuf> {
    let path = absolute(path)?;
    check_chain(&path)?;
    if !fs::metadata(&path)?.is_file() {
        return Err(invalid("Input must be an explicit regular file."));
    }
    Ok(path)
}

fn same_component(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

pub fn validate_output(path: &Path, temp: &Path) -> io::Result<PathBuf> {
    let path = absolute(path)?;
    let temp = absolute(temp)?;
    check_chain(&temp)?;
    check_chain(&path)?;
    let p: Vec<_> = path.components().collect();
    let t: Vec<_> = temp.components().collect();
    if p.len() <= t.len()
        || !p
            .iter()
            .zip(&t)
            .all(|(a, b)| same_component(a.as_os_str(), b.as_os_str()))
    {
        return Err(invalid(
            "Output must be a NEW child directory of TEMP; existing files are never overwritten.",
        ));
    }
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err(invalid(
            "Output must be a NEW child directory of TEMP; existing files are never overwritten.",
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    Ok(path)
}

pub fn create_output(path: &Path) -> io::Result<PathBuf> {
    let temp = std::env::temp_dir();
    let path = validate_output(path, &temp)?;
    // Reserve each missing directory separately. create_dir (not create_dir_all)
    // fails if another process wins the final name, including an empty directory.
    let mut missing = Vec::new();
    let mut parent = path
        .parent()
        .ok_or_else(|| invalid("Missing TEMP ancestor."))?;
    while !parent.exists() {
        missing.push(parent.to_path_buf());
        parent = parent
            .parent()
            .ok_or_else(|| invalid("Missing TEMP ancestor."))?;
    }
    for next in missing.iter().rev() {
        check_chain(next.parent().unwrap())?;
        fs::create_dir(next)?;
    }
    check_chain(path.parent().unwrap())?;
    fs::create_dir(&path)?;
    check_chain(&path)?;
    Ok(path)
}

pub fn register_path(name: &str, seen: &mut HashSet<String>) -> io::Result<()> {
    let key = archive_path(name)?;
    if !seen.insert(key) {
        return Err(invalid("Duplicate/alias ZIP entry path."));
    }
    Ok(())
}
