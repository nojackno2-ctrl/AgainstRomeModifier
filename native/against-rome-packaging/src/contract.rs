use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;

const ROOT_FILES: &[&str] = &[
    "boden.bmp",
    "boden.ini",
    "boden.txt",
    "collision.bmp",
    "emboss.bmp",
    "vertex.bmp",
    "smooth.bmp",
    "minimap.bmp",
    "floortex.dat",
    "textures.txt",
    "briefpic.tga",
    "daynight.bmp",
    "preload.ini",
    "arm_scenario.json",
];

fn is_word_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_dos_device_name(part: &str) -> bool {
    let base = part.split('.').next().unwrap_or(part);
    if base.eq_ignore_ascii_case("CON")
        || base.eq_ignore_ascii_case("PRN")
        || base.eq_ignore_ascii_case("AUX")
        || base.eq_ignore_ascii_case("NUL")
        || base.eq_ignore_ascii_case("CLOCK$")
    {
        return true;
    }
    if base.len() == 4 {
        let prefix = &base[..3];
        let num = base.as_bytes()[3];
        if (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
            && (b'0'..=b'9').contains(&num)
        {
            return true;
        }
    }
    false
}

fn matches_sdl_pattern(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".sdl") {
        return false;
    }
    let stem = &lower[..lower.len() - 4];
    if !stem.starts_with("endlos_") {
        return false;
    }
    let rest = &stem["endlos_".len()..];
    // Must contain "_siedlung"
    if let Some(idx) = rest.find("_siedlung") {
        let middle = &rest[..idx];
        let tail = &rest[idx + "_siedlung".len()..];
        if is_word_ident(middle) && (tail.is_empty() || is_word_ident(tail)) {
            return true;
        }
    }
    false
}

/// Checks whether a relative zip/archive path conforms to Against Rome map payload rules.
pub fn is_payload_path(path: &str) -> bool {
    if path.trim().is_empty() || path.contains('\\') || path.contains(':') {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    for p in &parts {
        if *p == ""
            || *p == "."
            || *p == ".."
            || p.starts_with(' ')
            || p.ends_with('.')
            || p.ends_with(' ')
            || p.chars().any(|c| (c as u32) < 32 || "<>:\"/\\|?*".contains(c))
            || is_dos_device_name(p)
        {
            return false;
        }
    }

    if parts.len() == 1 {
        let single = parts[0];
        return ROOT_FILES.iter().any(|f| f.eq_ignore_ascii_case(single))
            || matches_sdl_pattern(single);
    }

    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("DATA") {
        let file = parts[1];
        if file.to_ascii_lowercase().ends_with(".dat") {
            let stem = &file[..file.len() - 4];
            return is_word_ident(stem);
        }
        return false;
    }

    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("SCRIPT") {
        let file = parts[1];
        if file.to_ascii_lowercase().ends_with(".bci") {
            let stem = &file[..file.len() - 4];
            return is_word_ident(stem);
        }
        return false;
    }

    if parts.len() == 3 && parts[0].eq_ignore_ascii_case("TEXT") {
        let lang = parts[1];
        let file = parts[2];
        let is_lang = lang.len() == 2 && lang.chars().all(|c| c.is_ascii_alphabetic());
        let is_put = file.to_ascii_lowercase().ends_with(".put")
            && is_word_ident(&file[..file.len() - 4]);
        return is_lang && is_put;
    }

    false
}

/// Rejects symbolic links and Windows reparse points/junctions for path, existing ancestors and descendants.
pub fn reject_links(path: &Path) -> Result<()> {
    fn check_entry(entry: &Path) -> Result<()> {
        match fs::symlink_metadata(entry) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("Linked payloads are not supported: {}", entry.display()),
                    ));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            format!("Linked payloads are not supported: {}", entry.display()),
                        ));
                    }
                }
                Ok(())
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    for ancestor in path.ancestors() {
        check_entry(ancestor)?;
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let child = entry.path();
            check_entry(&child)?;
            if child.is_dir() {
                reject_links(&child)?;
            }
        }
    }
    Ok(())
}
