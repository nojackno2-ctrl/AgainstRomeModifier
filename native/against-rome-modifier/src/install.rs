//! Managed compatibility and language deployments. Planning is read-only;
//! callers commit every write/delete through their rollback transaction.
use crate::PatchProfile;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path, PathBuf},
};

pub const DG_MARKER: &str = ".against-rome-modifier-dgvoodoo.json";
pub const TRACE_MARKER: &str = ".against-rome-modifier-argmtrace.json";
pub const LANGUAGE_BACKUP: &str = ".against-rome-modifier-language-backup";
const DG_FILES: [&str; 4] = ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe", "dgVoodoo.conf"];
const SETTINGS: [(&str, &str); 5] = [
    ("FullScreenMode", "true"),
    ("ScalingMode", "stretched_ar"),
    ("CenterAppWindow", "true"),
    ("WindowedAttributes", ""),
    ("FullscreenAttributes", "fake"),
];

#[derive(Debug, Clone, Default)]
pub struct InstallResources {
    pub dgvoodoo: BTreeMap<String, Vec<u8>>,
    pub argm_winmm: Vec<u8>,
}
impl InstallResources {
    /// Loads the bundled third-party payloads (C# embedded resources) from a `ThirdParty` directory.
    pub fn from_third_party(root: &Path) -> io::Result<Self> {
        let mut r = Self::default();
        for name in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe", "dgVoodoo.conf"] {
            r.dgvoodoo
                .insert(name.into(), fs::read(root.join("dgVoodoo2").join(name))?);
        }
        r.argm_winmm = fs::read(root.join("argm-trace").join("winmm.dll"))?;
        Ok(r)
    }
    /// Looks for `ThirdParty` via `ARM_THIRDPARTY_DIR`, next to the executable, then up to four parents (repo layout); empty if absent.
    pub fn discover() -> Self {
        let mut roots: Vec<PathBuf> = std::env::var_os("ARM_THIRDPARTY_DIR")
            .map(PathBuf::from)
            .into_iter()
            .collect();
        if let Some(mut dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            for _ in 0..5 {
                roots.push(dir.join("ThirdParty"));
                if !dir.pop() {
                    break;
                }
            }
        }
        roots
            .iter()
            .find_map(|r| Self::from_third_party(r).ok())
            .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Default)]
pub struct InstallPlan {
    pub writes: BTreeMap<PathBuf, Vec<u8>>,
    pub deletes: Vec<PathBuf>,
}
fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
fn marker(plan: &mut InstallPlan, path: PathBuf, v: Value) -> io::Result<()> {
    plan.writes.insert(path, serde_json::to_vec_pretty(&v)?);
    Ok(())
}
fn hash_matches(path: &Path, expected: Option<&str>) -> io::Result<bool> {
    Ok(expected.is_some_and(|s| fs::read(path).is_ok_and(|b| sha256(&b).eq_ignore_ascii_case(s))))
}

/// Reject rooted, traversal and Windows alternate-stream paths on every host.
pub fn safe_language_path(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let normalized = relative.replace('\\', "/");
    if normalized.trim().is_empty()
        || normalized.contains(':')
        || normalized.starts_with('/')
        || normalized.split('/').any(|p| p == "..")
    {
        return Err(invalid("unsafe language restore path"));
    }
    let p = Path::new(&normalized);
    if p.components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(invalid("unsafe language restore path"));
    }
    Ok(root.join(p))
}
fn files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for item in fs::read_dir(root)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_symlink() {
            return Err(invalid("symbolic links are not language assets"));
        }
        if kind.is_dir() {
            result.extend(files(&item.path())?);
        } else if kind.is_file() {
            result.push(item.path());
        }
    }
    result.sort();
    Ok(result)
}
pub fn language_overlay_state(game: &Path) -> io::Result<Option<bool>> {
    let source = game.join("ToEng");
    if !source.is_dir() {
        return Ok(None);
    }
    let assets = files(&source)?;
    if assets.is_empty() {
        return Ok(None);
    }
    for asset in assets {
        let relative = asset
            .strip_prefix(&source)
            .map_err(|_| invalid("invalid asset path"))?;
        let dst = game.join(relative);
        if !dst.is_file() || fs::read(asset)? != fs::read(dst)? {
            return Ok(Some(false));
        }
    }
    Ok(Some(true))
}
pub struct LanguagePackFeature;
impl LanguagePackFeature {
    pub fn plan(game: &Path, enabled: bool) -> io::Result<InstallPlan> {
        let mut plan = InstallPlan::default();
        let backup = game.join(LANGUAGE_BACKUP);
        let mp = backup.join("manifest.json");
        if !enabled && !mp.is_file() {
            if language_overlay_state(game)? == Some(true) {
                return Err(invalid("missing language restore manifest.json"));
            }
            return Ok(plan);
        }
        let existing = if mp.is_file() {
            Some(read_json(&mp).ok_or_else(|| invalid("corrupt language manifest"))?)
        } else {
            None
        };
        let parse_paths = |m: &Value, key: &str| -> io::Result<Vec<String>> {
            m.get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("corrupt language manifest"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("corrupt language manifest"))
                })
                .collect()
        };
        if !enabled {
            let m = existing.unwrap();
            for rel in parse_paths(&m, "ExistingFiles")? {
                let src = safe_language_path(&backup.join("files"), &rel)?;
                plan.writes
                    .insert(safe_language_path(game, &rel)?, fs::read(src)?);
            }
            for rel in parse_paths(&m, "MissingFiles")? {
                let dst = safe_language_path(game, &rel)?;
                if dst.is_file() {
                    plan.deletes.push(dst);
                }
            }
            return Ok(plan);
        }
        let source = game.join("ToEng");
        let assets = files(&source)?;
        if assets.is_empty() {
            return Err(invalid("empty ToEng package"));
        }
        let mut present = Vec::new();
        let mut missing = Vec::new();
        if let Some(m) = existing {
            present = parse_paths(&m, "ExistingFiles")?;
            missing = parse_paths(&m, "MissingFiles")?;
            for rel in &present {
                if !safe_language_path(&backup.join("files"), rel)?.is_file() {
                    return Err(invalid("corrupt language backup"));
                }
            }
            for asset in &assets {
                let rel = asset
                    .strip_prefix(&source)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if !present
                    .iter()
                    .chain(&missing)
                    .any(|s| s.eq_ignore_ascii_case(&rel))
                {
                    return Err(invalid("language backup does not cover package"));
                }
            }
        } else {
            if backup.exists() {
                return Err(invalid("corrupt language backup directory"));
            }
            if language_overlay_state(game)? == Some(true) {
                return Err(invalid(
                    "language overlay already enabled without restore backup",
                ));
            }
            for asset in &assets {
                let rel = asset
                    .strip_prefix(&source)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let dst = safe_language_path(game, &rel)?;
                if dst.is_file() {
                    plan.writes.insert(
                        safe_language_path(&backup.join("files"), &rel)?,
                        fs::read(dst)?,
                    );
                    present.push(rel);
                } else {
                    missing.push(rel);
                }
            }
            marker(
                &mut plan,
                mp,
                json!({"ExistingFiles":present,"MissingFiles":missing}),
            )?;
        }
        for asset in assets {
            let rel = asset.strip_prefix(&source).unwrap().to_string_lossy();
            plan.writes
                .insert(safe_language_path(game, &rel)?, fs::read(&asset)?);
        }
        Ok(plan)
    }
}
pub fn apply_native_widescreen_window(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let bom = bytes.starts_with(&[239, 187, 191]);
    let text = String::from_utf8_lossy(if bom { &bytes[3..] } else { bytes });
    let mut text = text.into_owned();
    for (key, value) in SETTINGS {
        let mut count = 0;
        let mut out = String::new();
        for line in text.split_inclusive('\n') {
            let body = line.trim_end_matches(['\r', '\n']);
            let newline = &line[body.len()..];
            let leading = body.trim_start_matches([' ', '\t']);
            let replacement = leading.strip_prefix(key).and_then(|tail| {
                let tail = tail.trim_start_matches([' ', '\t']);
                tail.strip_prefix('=')
            });
            if let Some(after) = replacement {
                count += 1;
                let prefix_len = body.len() - after.len() + after.len()
                    - after.trim_start_matches([' ', '\t']).len();
                out.push_str(&body[..prefix_len]);
                out.push_str(value);
            } else {
                out.push_str(body)
            }
            out.push_str(newline);
        }
        if count != 1 {
            return Err(invalid(format!(
                "Expected exactly one dgVoodoo setting named '{key}', found {count}."
            )));
        }
        text = out;
    }
    let mut result = if bom { vec![239, 187, 191] } else { Vec::new() };
    result.extend(text.as_bytes());
    Ok(result)
}
pub struct DgVoodooFeature;
impl DgVoodooFeature {
    pub fn is_installed(game: &Path) -> bool {
        read_json(&game.join(DG_MARKER)).is_some_and(|m| {
            m.get("Files").and_then(Value::as_object).is_some_and(|f| {
                ["D3D8.dll", "DDraw.dll"]
                    .iter()
                    .all(|n| f.keys().any(|k| k.eq_ignore_ascii_case(n)) && game.join(n).is_file())
            })
        })
    }
    pub fn is_centered_presentation_configured(game: &Path) -> bool {
        fs::read_to_string(game.join("dgVoodoo.conf")).is_ok_and(|s| {
            SETTINGS.iter().all(|(k, v)| {
                s.lines().any(|l| {
                    l.trim_start_matches('\u{feff}')
                        .split_once('=')
                        .is_some_and(|(a, b)| {
                            a.trim().eq_ignore_ascii_case(k) && b.trim().eq_ignore_ascii_case(v)
                        })
                })
            })
        })
    }
    pub fn plan(
        game: &Path,
        enabled: bool,
        native: bool,
        resources: &InstallResources,
    ) -> io::Result<InstallPlan> {
        let mut plan = InstallPlan::default();
        let mp = game.join(DG_MARKER);
        let old = read_json(&mp);
        let oldfiles = old
            .as_ref()
            .and_then(|v| v.get("Files"))
            .and_then(Value::as_object);
        let expected = |name: &str| {
            oldfiles
                .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)))
                .and_then(|(_, v)| v.as_str())
        };
        if !enabled {
            if oldfiles.is_none() {
                return Ok(plan);
            }
            let mut preserved = serde_json::Map::new();
            for name in DG_FILES {
                let dst = game.join(name);
                if let Some(hash) = expected(name) {
                    if dst.is_file() {
                        if hash_matches(&dst, Some(hash))? {
                            plan.deletes.push(dst)
                        } else {
                            preserved.insert(name.to_owned(), json!(hash));
                        }
                    }
                }
            }
            if preserved.is_empty() {
                plan.deletes.push(mp)
            } else {
                let mut m = old.unwrap();
                m["Files"] = Value::Object(preserved);
                marker(&mut plan, mp, m)?;
            }
            return Ok(plan);
        }
        let mut hashes = serde_json::Map::new();
        for name in DG_FILES {
            let dst = game.join(name);
            let mut bytes = resources
                .dgvoodoo
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| invalid(format!("missing dgVoodoo resource {name}")))?;
            if dst.is_file() {
                if expected(name).is_none() {
                    return Err(invalid(format!(
                        "unmanaged dgVoodoo conflict: {}",
                        dst.display()
                    )));
                }
                if !hash_matches(&dst, expected(name))? {
                    if name != "dgVoodoo.conf" {
                        return Err(invalid(format!(
                            "modified dgVoodoo conflict: {}",
                            dst.display()
                        )));
                    }
                    bytes = fs::read(&dst)?;
                }
            }
            if name == "dgVoodoo.conf" && native {
                bytes = apply_native_widescreen_window(&bytes)?;
            }
            hashes.insert(name.to_owned(), json!(sha256(&bytes)));
            plan.writes.insert(dst, bytes);
        }
        marker(&mut plan, mp, json!({"Version":"v2.87.3","Files":hashes}))?;
        Ok(plan)
    }
}
pub struct ArgmTraceFeature;
impl ArgmTraceFeature {
    pub const KNOWN_TIMESTAMP: u32 = 0x404d1710;
    pub fn is_installed(game: &Path) -> bool {
        read_json(&game.join(TRACE_MARKER)).is_some_and(|m| {
            m["DllName"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("winmm.dll"))
                && game.join("winmm.dll").is_file()
        })
    }
    pub fn plan(
        game: &Path,
        enabled: bool,
        resources: &InstallResources,
    ) -> io::Result<InstallPlan> {
        let mut plan = InstallPlan::default();
        let mp = game.join(TRACE_MARKER);
        let old = read_json(&mp);
        let name = old
            .as_ref()
            .and_then(|m| m["DllName"].as_str())
            .unwrap_or("");
        let dh = old.as_ref().and_then(|m| m["DllSha256"].as_str());
        let ih = old.as_ref().and_then(|m| m["IniSha256"].as_str());
        let legacy = game.join("version.dll");
        if old.is_some()
            && (name.is_empty() || name.eq_ignore_ascii_case("version.dll"))
            && legacy.is_file()
            && hash_matches(&legacy, dh)?
        {
            plan.deletes.push(legacy);
        }
        let dll = game.join("winmm.dll");
        let ini = game.join("argm_trace.ini");
        if !enabled {
            if old.is_none() {
                return Ok(plan);
            }
            if name.eq_ignore_ascii_case("winmm.dll") && dll.is_file() && hash_matches(&dll, dh)? {
                plan.deletes.push(dll);
            }
            if ini.is_file() && hash_matches(&ini, ih)? {
                plan.deletes.push(ini);
            }
            plan.deletes.push(mp);
            return Ok(plan);
        }
        if resources.argm_winmm.is_empty() {
            return Err(invalid("missing argm-trace winmm.dll resource"));
        }
        let hash = sha256(&resources.argm_winmm);
        if dll.is_file()
            && !(name.eq_ignore_ascii_case("winmm.dll")
                && (hash_matches(&dll, dh)? || hash_matches(&dll, Some(&hash))?))
        {
            return Err(invalid("unmanaged winmm.dll conflict"));
        }
        let timestamp = fs::read(game.join("Against_Rome.exe"))
            .ok()
            .and_then(|b| exe_timestamp(&b))
            .filter(|v| *v == Self::KNOWN_TIMESTAMP)
            .unwrap_or(0);
        let ini_bytes = if ini.is_file() && !hash_matches(&ini, ih)? {
            fs::read(&ini)?
        } else {
            build_trace_ini(timestamp).into_bytes()
        };
        let ini_hash = sha256(&ini_bytes);
        plan.writes.insert(dll, resources.argm_winmm.clone());
        if !ini.is_file() || hash_matches(&ini, ih)? {
            plan.writes.insert(ini, ini_bytes);
        }
        marker(
            &mut plan,
            mp,
            json!({"DllName":"winmm.dll","DllSha256":hash,"IniSha256":ini_hash,"ExpectedTimeDateStamp":timestamp}),
        )?;
        Ok(plan)
    }
}
pub fn exe_timestamp(b: &[u8]) -> Option<u32> {
    let offset = u32::from_le_bytes(b.get(0x3c..0x40)?.try_into().ok()?) as usize;
    if b.get(offset..offset.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    Some(u32::from_le_bytes(
        b.get(offset.checked_add(8)?..offset.checked_add(12)?)?
            .try_into()
            .ok()?,
    ))
}
pub fn build_trace_ini(expected: u32) -> String {
    let build = if expected != 0 {
        format!("; 目標組建已辨識,開啟 hook 後位址型 AI hook 會一併解鎖。\r\nexpectedTimeDateStamp={expected:08X}\r\n\r\n")
    } else {
        "; 目標組建未辨識;開啟 hook 後僅安裝有簽章驗證的 hook。\r\nexpectedTimeDateStamp=0\r\n\r\n"
            .to_owned()
    };
    format!("; 由 Against Rome Modifier 自動產生（argm-trace 執行期飛行紀錄器,winmm 代理）。\r\n; 逐鍵說明見 native/argm-trace/argm_trace.ini.sample。\r\n\r\n[general]\r\nenabled=1\r\nverifySignatures=1\r\n; 首次部署為 log-only（不裝任何 hook）。確認遊戲能正常啟動且產生\r\n; argm_trace.log 後,把下一行改成 enableHooks=1 即可開啟 AI 事件記錄。\r\nenableHooks=0\r\nargDumpCount=9\r\nmaxLogMegabytes=256\r\n\r\n[build]\r\n{build}[hooks]\r\ntraceNpcJobs=1\r\ntraceNpcActive=1\r\ntraceVillage=1\r\ntraceCreateUnit=1\r\n; 無盡 AI 決策脈絡：復活資格快照、新局邊界、單位數量上限。\r\ntraceNpcQuery=1\r\ntraceLevelInit=1\r\ntraceUnitMax=1\r\ntraceOpcodes=0\r\n")
}
pub fn plan_install(
    game: &Path,
    profile: &PatchProfile,
    resources: &InstallResources,
) -> io::Result<InstallPlan> {
    let mut result = InstallPlan::default();
    for plan in [
        LanguagePackFeature::plan(game, profile.get_bool("ToEnglish"))?,
        DgVoodooFeature::plan(
            game,
            profile.get_bool("DgVoodoo"),
            profile.get_bool("NativeWidescreen1920x1080"),
            resources,
        )?,
        ArgmTraceFeature::plan(game, profile.get_bool("ArgmTrace"), resources)?,
    ] {
        result.writes.extend(plan.writes);
        result.deletes.extend(plan.deletes);
    }
    Ok(result)
}

pub struct InstallFeatureDetector;
impl InstallFeatureDetector {
    pub fn detect(game_path: &Path, profile: &mut PatchProfile) {
        if let Ok(overlay) = language_overlay_state(game_path) {
            profile.set_bool("ToEnglish", overlay.unwrap_or(false));
        }
        let dg_installed = DgVoodooFeature::is_installed(game_path);
        profile.set_bool("DgVoodoo", dg_installed);
        if dg_installed && DgVoodooFeature::is_centered_presentation_configured(game_path) {
            profile.set_bool("NativeWidescreen1920x1080", true);
        }
        profile.set_bool("ArgmTrace", ArgmTraceFeature::is_installed(game_path));
    }
}
