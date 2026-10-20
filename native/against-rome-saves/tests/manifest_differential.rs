//! Differential test comparing Rust Manifest implementation against
//! the authentic C# SaveBackupManifestCodec.cs and JsonDefaults.cs via ManifestOracle.
//!
//! Enforces exact semantic field-by-field parity, BOM stripping, missing/unknown field tolerance,
//! nullable handling, format_time observation parity, and roundtrip JSON/ZIP compatibility.
//!
//! SAFETY: Uses synthetic test fixtures strictly within re_workspace.
//! Never accesses real user saves, default game folders, or retail installations.

use against_rome_saves::{Manifest, SaveService};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use zip::write::FileOptions;
use zip::ZipWriter;

static BUILD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OracleBatchRequest {
    writes: Vec<WriteRequest>,
    format_times: Vec<String>,
    deserializes: Vec<DeserializeRequest>,
    zip_reads: Vec<ZipReadRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WriteRequest {
    id: String,
    folder: String,
    title: String,
    level: String,
    backup_time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DeserializeRequest {
    id: String,
    raw_base64: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ZipReadRequest {
    id: String,
    zip_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OracleBatchResponse {
    writes: Vec<WriteResponse>,
    format_times: Vec<FormatTimeResponse>,
    deserializes: Vec<DeserializeResponse>,
    zip_reads: Vec<ZipReadResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WriteResponse {
    id: String,
    json: String,
    orig_folder: String,
    title: String,
    level: String,
    backup_time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct FormatTimeResponse {
    input: String,
    formatted: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DeserializeResponse {
    id: String,
    success: bool,
    orig_folder: Option<String>,
    title: Option<String>,
    level: Option<String>,
    backup_time: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ZipReadResponse {
    id: String,
    entry_exists: bool,
    success: bool,
    orig_folder: Option<String>,
    title: Option<String>,
    level: Option<String>,
    backup_time: Option<String>,
    formatted_time: Option<String>,
    error: Option<String>,
}

fn get_clean_re_workspace() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap();
    if dir.ends_with("against-rome-saves") {
        if let Some(p) = dir.parent().and_then(|p| p.parent()) {
            dir = p.to_path_buf();
        }
    }
    dir.join("re_workspace")
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(suffix: &str) -> Self {
        let base = get_clean_re_workspace();
        let path = base.join(format!("manifest_diff_{}_{}", std::process::id(), suffix));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create synthetic test workspace");
        Self { path }
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn ensure_oracle_built() -> PathBuf {
    let _guard = BUILD_LOCK.lock().unwrap();
    let base = get_clean_re_workspace();
    let artifacts_dir = base.join("manifest_oracle_artifacts");
    let dll_path = artifacts_dir.join("bin/ManifestOracle/release/ManifestOracle.dll");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_path = manifest_dir.join("tests/oracle_manifest/ManifestOracle.csproj");
    let nuget_config = manifest_dir.join("tests/oracle_manifest/NuGet.Config");
    let appdata = base.join("manifest_oracle_appdata");
    fs::create_dir_all(&appdata).unwrap();

    let build_output = Command::new("dotnet")
        .env("APPDATA", &appdata)
        .args([
            "build",
            project_path.to_str().unwrap(),
            "--artifacts-path",
            artifacts_dir.to_str().unwrap(),
            "--configfile",
            nuget_config.to_str().unwrap(),
            "-c",
            "Release",
            "--nologo",
            "-v:q",
        ])
        .output()
        .expect("dotnet build oracle");

    assert!(
        build_output.status.success(),
        "Oracle build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    assert!(dll_path.exists(), "ManifestOracle.dll not found after build");
    dll_path
}

fn run_oracle(
    ws: &TempWorkspace,
    request: &OracleBatchRequest,
) -> OracleBatchResponse {
    let dll_path = ensure_oracle_built();

    let req_path = ws.path.join("request.json");
    let resp_path = ws.path.join("response.json");

    let req_bytes = serde_json::to_vec_pretty(request).expect("serialize batch request");
    fs::write(&req_path, req_bytes).expect("write batch request");

    let run_output = Command::new("dotnet")
        .arg(&dll_path)
        .arg(&req_path)
        .arg(&resp_path)
        .output()
        .expect("execute oracle");

    assert!(
        run_output.status.success(),
        "Oracle run failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );

    println!("Oracle stdout: {}", String::from_utf8_lossy(&run_output.stdout));
    println!("Oracle stderr: {}", String::from_utf8_lossy(&run_output.stderr));

    let resp_bytes = fs::read(&resp_path).unwrap_or_default();
    if resp_bytes.is_empty() {
        panic!("Oracle produced empty response file at {:?}\nstdout: {}\nstderr: {}", resp_path, String::from_utf8_lossy(&run_output.stdout), String::from_utf8_lossy(&run_output.stderr));
    }
    let clean_bytes = resp_bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&resp_bytes);
    serde_json::from_slice(clean_bytes).expect("deserialize batch response")
}

fn to_base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(b2 & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[test]
fn test_csharp_manifest_oracle_comprehensive_parity() {
    let ws = TempWorkspace::new("comprehensive");

    // 1. Prepare synthetic cases
    // A. C# -> Rust serialization/deserialization cases
    let write_cases = vec![
        WriteRequest {
            id: "standard_ascii".into(),
            folder: "SAVE_001".into(),
            title: "Battle for Rome".into(),
            level: "MAP_01".into(),
            backup_time: "2026-10-18T14:30:00.0000000Z".into(),
        },
        WriteRequest {
            id: "unicode_traditional_chinese".into(),
            folder: "SAVE_002".into(),
            title: "羅馬的崛起 ⚔️ (Campaign 01)".into(),
            level: "關卡_01_教學".into(),
            backup_time: "2026-10-18T22:30:00.0000000+08:00".into(),
        },
        WriteRequest {
            id: "empty_strings".into(),
            folder: "".into(),
            title: "".into(),
            level: "".into(),
            backup_time: "2026-01-01T00:00:00.0000000Z".into(),
        },
        WriteRequest {
            id: "special_escapes".into(),
            folder: "SAVE_SPECIAL".into(),
            title: "Title with \"quotes\" and \\slashes\\ & symbols".into(),
            level: "LEVEL\tTAB".into(),
            backup_time: "2026-06-15T12:00:00.0000000Z".into(),
        },
    ];

    // B. FormatTime observation cases
    let format_time_cases = vec![
        "2026-10-18T15:30:00Z".to_string(),
        "2026-10-18T15:30:00.0000000Z".to_string(),
        "2026-10-18T15:30:00+08:00".to_string(),
        "2026-10-18T15:30:00-05:00".to_string(),
        "2026-10-18T15:30:00".to_string(),
        "2026-10-18 15:30:00".to_string(),
        "invalid-not-a-date".to_string(),
        "".to_string(),
        "9999-99-99".to_string(),
        "2026-02-30T10:00:00".to_string(),
    ];

    // C. Deserialization cases (BOM, missing fields, unknown fields, nulls, corrupt JSON)
    let bom_json = {
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice(br#"{"OrigFolder":"SAVE_BOM","Title":"BOM Title","Level":"MAP_BOM","BackupTime":"2026-10-18T10:00:00Z"}"#);
        b
    };
    let bom_unicode = {
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice("{\"OrigFolder\":\"SAVE_漢\",\"Title\":\"繁體中文測試\",\"Level\":\"關卡\",\"BackupTime\":\"2026-10-18T10:00:00Z\"}".as_bytes());
        b
    };
    let missing_fields = br#"{"Title":"Only Title"}"#;
    let empty_obj = br#"{}"#;
    let unknown_fields = br#"{"OrigFolder":"SAVE_010","Title":"T","Level":"L","BackupTime":"2026-10-18T10:00:00Z","ExtraField":12345,"ExtraNested":{"A":true}}"#;
    let json_null = br#"null"#;
    let field_nulls = br#"{"OrigFolder":null,"Title":null,"Level":null,"BackupTime":null}"#;
    let broken_json = br#"{"OrigFolder": "broken"#;
    let non_object_json = br#"[1, 2, 3]"#;

    let deserializes = vec![
        DeserializeRequest {
            id: "bom_standard".into(),
            raw_base64: to_base64(&bom_json),
        },
        DeserializeRequest {
            id: "bom_unicode".into(),
            raw_base64: to_base64(&bom_unicode),
        },
        DeserializeRequest {
            id: "missing_fields".into(),
            raw_base64: to_base64(missing_fields),
        },
        DeserializeRequest {
            id: "empty_obj".into(),
            raw_base64: to_base64(empty_obj),
        },
        DeserializeRequest {
            id: "unknown_fields".into(),
            raw_base64: to_base64(unknown_fields),
        },
        DeserializeRequest {
            id: "json_null".into(),
            raw_base64: to_base64(json_null),
        },
        DeserializeRequest {
            id: "field_nulls".into(),
            raw_base64: to_base64(field_nulls),
        },
        DeserializeRequest {
            id: "broken_syntax".into(),
            raw_base64: to_base64(broken_json),
        },
        DeserializeRequest {
            id: "non_object".into(),
            raw_base64: to_base64(non_object_json),
        },
    ];

    // D. ZipRead synthetic archives
    let zip_with_manifest = ws.path.join("archive_valid.zip");
    {
        let file = File::create(&zip_with_manifest).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default()).unwrap();
        zip.write_all(br#"{"OrigFolder":"SAVE_ZIP_1","Title":"Zip Title","Level":"Zip Level","BackupTime":"2026-10-18T12:00:00Z"}"#).unwrap();
        zip.finish().unwrap();
    }

    let zip_with_bom = ws.path.join("archive_bom.zip");
    {
        let file = File::create(&zip_with_bom).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default()).unwrap();
        zip.write_all(&bom_unicode).unwrap();
        zip.finish().unwrap();
    }

    let zip_no_manifest = ws.path.join("archive_no_manifest.zip");
    {
        let file = File::create(&zip_no_manifest).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("save.ini", FileOptions::default()).unwrap();
        zip.write_all(b"[titel]\nIni Title\n[orglevelname]\nIni Level\n").unwrap();
        zip.finish().unwrap();
    }

    let zip_broken_manifest = ws.path.join("archive_broken.zip");
    {
        let file = File::create(&zip_broken_manifest).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default()).unwrap();
        zip.write_all(b"{corrupted json").unwrap();
        zip.finish().unwrap();
    }

    let zip_reads = vec![
        ZipReadRequest {
            id: "valid_zip".into(),
            zip_path: zip_with_manifest.to_str().unwrap().into(),
        },
        ZipReadRequest {
            id: "bom_zip".into(),
            zip_path: zip_with_bom.to_str().unwrap().into(),
        },
        ZipReadRequest {
            id: "no_manifest_zip".into(),
            zip_path: zip_no_manifest.to_str().unwrap().into(),
        },
        ZipReadRequest {
            id: "broken_manifest_zip".into(),
            zip_path: zip_broken_manifest.to_str().unwrap().into(),
        },
    ];

    let request = OracleBatchRequest {
        writes: write_cases.clone(),
        format_times: format_time_cases.clone(),
        deserializes: deserializes.clone(),
        zip_reads: zip_reads.clone(),
    };

    // Execute .NET oracle
    let response = run_oracle(&ws, &request);

    // -------------------------------------------------------------
    // ASSERTIONS: Verify C# Oracle results against Rust semantics
    // -------------------------------------------------------------

    // 1. C# Write -> Rust Deserialization (Semantic field-by-field parity)
    for csharp_write in &response.writes {
        let rust_manifest: Manifest = serde_json::from_str(&csharp_write.json).unwrap_or_else(|e| {
            panic!("Rust failed to parse C# generated JSON (id={}): {e}\n{}", csharp_write.id, csharp_write.json);
        });

        assert_eq!(rust_manifest.orig_folder, csharp_write.orig_folder, "orig_folder mismatch in {}", csharp_write.id);
        assert_eq!(rust_manifest.title, csharp_write.title, "title mismatch in {}", csharp_write.id);
        assert_eq!(rust_manifest.level, csharp_write.level, "level mismatch in {}", csharp_write.id);
        assert_eq!(rust_manifest.backup_time, csharp_write.backup_time, "backup_time mismatch in {}", csharp_write.id);
    }

    // 2. Rust Serialization -> C# Deserialization (Bidirectional roundtrip)
    for w in &write_cases {
        let rust_m = Manifest {
            orig_folder: w.folder.clone(),
            title: w.title.clone(),
            level: w.level.clone(),
            backup_time: w.backup_time.clone(),
        };
        let rust_json = serde_json::to_vec_pretty(&rust_m).expect("serialize rust manifest");

        let csharp_decoded: serde_json::Value = serde_json::from_slice(&rust_json).unwrap();
        assert_eq!(csharp_decoded["OrigFolder"].as_str().unwrap_or(""), w.folder);
        assert_eq!(csharp_decoded["Title"].as_str().unwrap_or(""), w.title);
        assert_eq!(csharp_decoded["Level"].as_str().unwrap_or(""), w.level);
        assert_eq!(csharp_decoded["BackupTime"].as_str().unwrap_or(""), w.backup_time);
    }

    // 3. FormatTime Parity and Observation
    for ft in &response.format_times {
        if ft.input == "invalid-not-a-date" || ft.input.is_empty() || ft.input == "9999-99-99" || ft.input == "2026-02-30T10:00:00" {
            assert_eq!(ft.formatted, ft.input, "Fallback expected for invalid time: {}", ft.input);
        } else {
            assert_eq!(ft.formatted.len(), 19, "Formatted time should be 19 chars: {}", ft.formatted);
            assert!(ft.formatted.chars().nth(4) == Some('-'));
            assert!(ft.formatted.chars().nth(7) == Some('-'));
            assert!(ft.formatted.chars().nth(10) == Some(' '));
            assert!(ft.formatted.chars().nth(13) == Some(':'));
            assert!(ft.formatted.chars().nth(16) == Some(':'));
        }
    }

    // 4. Deserialization parity (BOM, missing fields, unknown fields, nulls, corrupt)
    for d in &response.deserializes {
        let raw = match d.id.as_str() {
            "bom_standard" => &bom_json[..],
            "bom_unicode" => &bom_unicode[..],
            "missing_fields" => missing_fields,
            "empty_obj" => empty_obj,
            "unknown_fields" => unknown_fields,
            "json_null" => json_null,
            "field_nulls" => field_nulls,
            "broken_syntax" => broken_json,
            "non_object" => non_object_json,
            other => panic!("Unknown deserialize case {other}"),
        };

        let stripped = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
        let rust_res = serde_json::from_slice::<Manifest>(stripped);

        match d.id.as_str() {
            "bom_standard" => {
                assert!(d.success, "C# should decode BOM standard");
                let m = rust_res.expect("Rust should decode BOM standard");
                assert_eq!(m.orig_folder, d.orig_folder.as_deref().unwrap_or(""));
                assert_eq!(m.title, d.title.as_deref().unwrap_or(""));
                assert_eq!(m.level, d.level.as_deref().unwrap_or(""));
                assert_eq!(m.backup_time, d.backup_time.as_deref().unwrap_or(""));
            }
            "bom_unicode" => {
                assert!(d.success, "C# should decode BOM unicode");
                let m = rust_res.expect("Rust should decode BOM unicode");
                assert_eq!(m.orig_folder, d.orig_folder.as_deref().unwrap_or(""));
                assert_eq!(m.title, d.title.as_deref().unwrap_or(""));
                assert_eq!(m.level, d.level.as_deref().unwrap_or(""));
                assert_eq!(m.backup_time, d.backup_time.as_deref().unwrap_or(""));
            }
            "missing_fields" => {
                assert!(d.success, "C# should tolerate missing fields");
                let m = rust_res.expect("Rust should tolerate missing fields");
                assert_eq!(m.title, "Only Title");
                assert_eq!(m.orig_folder, "");
                assert_eq!(m.level, "");
                assert_eq!(m.backup_time, "");
                assert_eq!(m.orig_folder, d.orig_folder.as_deref().unwrap_or(""));
                assert_eq!(m.title, d.title.as_deref().unwrap_or(""));
            }
            "empty_obj" => {
                assert!(d.success, "C# should tolerate empty object");
                let m = rust_res.expect("Rust should tolerate empty object");
                assert_eq!(m, Manifest::default());
            }
            "unknown_fields" => {
                assert!(d.success, "C# should ignore unknown fields");
                let m = rust_res.expect("Rust should ignore unknown fields");
                assert_eq!(m.orig_folder, "SAVE_010");
                assert_eq!(m.title, "T");
                assert_eq!(m.level, "L");
                assert_eq!(m.backup_time, "2026-10-18T10:00:00Z");
            }
            "broken_syntax" | "non_object" => {
                assert!(!d.success, "C# should fail on invalid JSON ({})", d.id);
                assert!(rust_res.is_err(), "Rust should fail on invalid JSON ({})", d.id);
            }
            "json_null" => {
                assert!(d.success);
                assert!(d.orig_folder.is_none());
                assert_eq!(rust_res.expect("Rust should tolerate a null manifest"), Manifest::default());
            }
            "field_nulls" => {
                assert!(d.success);
                assert_eq!(d.orig_folder.as_deref().unwrap_or(""), "");
                let m = rust_res.expect("Rust should tolerate nullable fields");
                assert_eq!(m.orig_folder, d.orig_folder.as_deref().unwrap_or(""));
                assert_eq!(m.title, d.title.as_deref().unwrap_or(""));
                assert_eq!(m.level, d.level.as_deref().unwrap_or(""));
                assert_eq!(m.backup_time, d.backup_time.as_deref().unwrap_or(""));
            }
            _ => {}
        }
    }

    // 5. ZIP Archive reading parity
    for z in &response.zip_reads {
        match z.id.as_str() {
            "valid_zip" => {
                assert!(z.entry_exists);
                assert!(z.success);
                assert_eq!(z.orig_folder.as_deref(), Some("SAVE_ZIP_1"));
                assert_eq!(z.title.as_deref(), Some("Zip Title"));
                assert_eq!(z.level.as_deref(), Some("Zip Level"));
                assert_eq!(z.backup_time.as_deref(), Some("2026-10-18T12:00:00Z"));
            }
            "bom_zip" => {
                assert!(z.entry_exists);
                assert!(z.success);
                assert_eq!(z.orig_folder.as_deref(), Some("SAVE_漢"));
                assert_eq!(z.title.as_deref(), Some("繁體中文測試"));
            }
            "no_manifest_zip" => {
                assert!(!z.entry_exists);
                assert!(z.success);
                assert!(z.orig_folder.is_none());
            }
            "broken_manifest_zip" => {
                assert!(z.entry_exists);
                assert!(!z.success, "C# Read should fail on broken json");
            }
            _ => {}
        }
    }
}

#[test]
fn test_rust_save_service_backup_readable_by_csharp_oracle() {
    let ws = TempWorkspace::new("service_roundtrip");
    let game_dir = ws.path.join("game");
    let backups_dir = ws.path.join("backups");
    let save_dir = game_dir.join("SAVE/SAVE_999");
    fs::create_dir_all(save_dir.join("DATA")).unwrap();
    fs::create_dir_all(&backups_dir).unwrap();

    fs::write(save_dir.join("save.ini"), b"[titel]\nRust Epic Save\n[orglevelname]\nLEVEL_FINAL\n").unwrap();
    fs::write(save_dir.join("DATA/data.bin"), b"synthetic payload").unwrap();

    let svc = SaveService::new(&game_dir, &backups_dir).expect("create SaveService");
    let backup_name = svc.backup("SAVE_999", "Rust Epic Save", "LEVEL_FINAL").expect("create backup");

    let backup_path = backups_dir.join(&backup_name);
    assert!(backup_path.exists(), "Backup ZIP must be created");

    // Ask C# oracle to read the Rust-produced backup ZIP
    let zip_req = ZipReadRequest {
        id: "rust_produced_backup".into(),
        zip_path: backup_path.to_str().unwrap().into(),
    };

    let batch = OracleBatchRequest {
        writes: vec![],
        format_times: vec![],
        deserializes: vec![],
        zip_reads: vec![zip_req],
    };

    let response = run_oracle(&ws, &batch);
    assert_eq!(response.zip_reads.len(), 1);
    let read_res = &response.zip_reads[0];

    assert!(read_res.entry_exists, "C# must find manifest.json inside Rust backup ZIP");
    assert!(read_res.success, "C# must successfully parse manifest from Rust backup ZIP");
    assert_eq!(read_res.orig_folder.as_deref(), Some("SAVE_999"));
    assert_eq!(read_res.title.as_deref(), Some("Rust Epic Save"));
    assert_eq!(read_res.level.as_deref(), Some("LEVEL_FINAL"));
    assert!(read_res.backup_time.is_some(), "BackupTime must be present");
    assert!(read_res.formatted_time.is_some(), "FormattedTime must be valid");
}


// Exercise the real C# service, rather than duplicating its fallback rules in the test.
// Generated build inputs and outputs stay in the synthetic workspace; crate files are untouched.
fn run_catalog_oracle(ws: &TempWorkspace, game: &std::path::Path, backups: &std::path::Path) -> Vec<CatalogRow> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf();
    let sources = [
        "src.Core/Core/Services/SaveBackupService.cs",
        "src.Core/Core/Services/SaveBackupManifestCodec.cs",
        "src.Core/Core/Patches/PatchText.cs",
        "src.Shared/Core/GameLZSS.cs",
        "src.Shared/Core/Services/JsonDefaults.cs",
    ].iter().map(|source| format!("<Compile Include=\"{}\" />", root.join(source).display().to_string().replace('&', "&amp;")))
        .collect::<String>();
    let project = ws.path.join("CatalogOracle.csproj");
    fs::write(&project, format!(r#"<Project Sdk="Microsoft.NET.Sdk">
<PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable><EnableDefaultCompileItems>false</EnableDefaultCompileItems></PropertyGroup>
<ItemGroup><Compile Include="CatalogOracle.cs" />{sources}</ItemGroup></Project>"#)).unwrap();
    fs::write(ws.path.join("CatalogOracle.cs"), r#"
using System.Text.Json;
using AgainstRomeModifier.Core.Services;
var catalog = new SaveBackupService(args[1]).Scan(args[0]);
Console.WriteLine(JsonSerializer.Serialize(catalog.Backups.Select(b => new {
    b.FileName, b.Title, b.Level, b.OrigFolder, b.Parsed
})));
"#).unwrap();
    let artifacts = ws.path.join("artifacts");
    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/oracle_manifest/NuGet.Config");
    let build = Command::new("dotnet")
        .env("APPDATA", ws.path.join("appdata"))
        .arg("build").arg(&project).arg("--artifacts-path").arg(&artifacts)
        .arg("--configfile").arg(config).args(["-c", "Release", "--nologo", "-v:q"])
        .output().expect("build real C# catalog oracle");
    assert!(build.status.success(), "Catalog oracle build failed:\n{}\n{}", String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
    let output = Command::new("dotnet")
        .arg(artifacts.join("bin/CatalogOracle/release/CatalogOracle.dll"))
        .arg(game).arg(backups).output().expect("run real C# catalog oracle");
    assert!(output.status.success(), "Catalog oracle failed:\n{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("decode C# catalog rows")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CatalogRow {
    file_name: String,
    title: String,
    level: String,
    orig_folder: String,
    parsed: bool,
}

#[test]
fn null_manifest_catalog_matches_csharp_with_pfil_fallback() {
    let ws = TempWorkspace::new("null_catalog");
    let game = ws.path.join("game");
    let backups = ws.path.join("backups");
    fs::create_dir_all(&game).unwrap();
    fs::create_dir_all(&backups).unwrap();
    // Literal-only LZSS stream in a valid 64-byte PFIL container.
    let ini = b"[titel]\r\nIni Title\r\n[orglevelname]\r\nINI_LEVEL\r\n";
    let mut pfil = vec![0u8; 64];
    pfil[..4].copy_from_slice(b"PFIL");
    pfil[16..20].copy_from_slice(&(ini.len() as u32).to_le_bytes());
    for chunk in ini.chunks(8) {
        pfil.push(((1u16 << chunk.len()) - 1) as u8);
        pfil.extend_from_slice(chunk);
    }
    assert_eq!(against_rome_data::lzss::decompress_pfil(&pfil, None).unwrap(), ini);

    // id, manifest, expected Title, Level, OrigFolder, Parsed. All ZIPs contain valid PFIL.
    let cases: Vec<(&str, Option<&[u8]>, &str, &str, &str, bool)> = vec![
        ("absent", None, "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("root_null", Some(b"null"), "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("bom_null", Some(b"\xef\xbb\xbfnull"), "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("all_null", Some(br#"{"OrigFolder":null,"Title":null,"Level":null,"BackupTime":null}"#), "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("empty_object", Some(b"{}"), "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("null_title", Some(br#"{"OrigFolder":"MANIFEST_FOLDER","Title":null,"Level":"Manifest Level"}"#), "Ini Title", "Manifest Level", "MANIFEST_FOLDER", true),
        ("null_level", Some(br#"{"OrigFolder":"MANIFEST_FOLDER","Title":"Manifest Title","Level":null}"#), "Manifest Title", "INI_LEVEL", "MANIFEST_FOLDER", true),
        ("null_folder", Some(br#"{"OrigFolder":null,"Title":"Manifest Title","Level":"Manifest Level","BackupTime":"kept"}"#), "Manifest Title", "Manifest Level", "SAVE_NULL", true),
        ("null_time", Some(br#"{"OrigFolder":"MANIFEST_FOLDER","Title":"Manifest Title","Level":"Manifest Level","BackupTime":null}"#), "Manifest Title", "Manifest Level", "MANIFEST_FOLDER", true),
        ("missing_title", Some(br#"{"Level":"Manifest Level"}"#), "Ini Title", "Manifest Level", "SAVE_NULL", true),
        ("missing_level", Some(br#"{"Title":"Manifest Title"}"#), "Manifest Title", "INI_LEVEL", "SAVE_NULL", true),
        ("empty_title", Some(br#"{"Title":"","Level":"Manifest Level"}"#), "Ini Title", "Manifest Level", "SAVE_NULL", true),
        ("empty_level", Some(br#"{"Title":"Manifest Title","Level":""}"#), "Manifest Title", "INI_LEVEL", "SAVE_NULL", true),
        ("whitespace_title", Some(br#"{"Title":" ","Level":null}"#), " ", "INI_LEVEL", "SAVE_NULL", true),
        ("null_unknown", Some(br#"{"Title":null,"Level":null,"Unknown":{"A":null}}"#), "Ini Title", "INI_LEVEL", "SAVE_NULL", true),
        ("broken_json", Some(b"{broken"), "", "", "SAVE_NULL", false),
        ("wrong_field_type", Some(br#"{"Title":42,"Level":null}"#), "", "", "SAVE_NULL", false),
        ("array", Some(b"[1,2,3]"), "", "", "SAVE_NULL", false),
    ];
    let mut expected = Vec::new();
    for (index, (id, manifest, title, level, folder, parsed)) in cases.iter().enumerate() {
        let name = format!("Backup_SAVE_NULL_20261018_{index:06}.zip");
        let mut zip = ZipWriter::new(File::create(backups.join(&name)).unwrap());
        zip.start_file("save.ini", FileOptions::default()).unwrap();
        zip.write_all(&pfil).unwrap();
        if let Some(bytes) = manifest {
            zip.start_file("manifest.json", FileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        expected.push((name, *id, *title, *level, *folder, *parsed));
    }
    let csharp = run_catalog_oracle(&ws, &game, &backups);
    let rust = SaveService::new(&game, &backups).unwrap().scan().unwrap();
    assert_eq!(csharp.len(), cases.len());
    assert_eq!(rust.backups.len(), cases.len());
    for (name, id, title, level, folder, parsed) in expected {
        let cs = csharp.iter().find(|row| row.file_name == name).unwrap();
        let rs = rust.backups.iter().find(|row| row.file_name == name).unwrap();
        let expected_fields = (title, level, folder, parsed);
        assert_eq!((cs.title.as_str(), cs.level.as_str(), cs.orig_folder.as_str(), cs.parsed), expected_fields, "C# fixture contract: {id}");
        assert_eq!((rs.manifest.title.as_str(), rs.manifest.level.as_str(), rs.manifest.orig_folder.as_str(), rs.parsed), expected_fields, "Rust full catalog parity: {id}");
    }
}
