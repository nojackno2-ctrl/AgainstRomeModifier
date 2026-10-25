use alr_scanline_differential::{cli, compare_member, run, validate_output};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Command,

};

struct Fixture {
    root: PathBuf,
    game: PathBuf,
    inventory: PathBuf,
    output: PathBuf,
}
impl Fixture {
    fn new() -> Self {

        let root = crate::temp_fixture::dir("arm-scanline-test");
        fs::create_dir_all(&root).unwrap();
        let game = root.join("game");
        fs::create_dir(&game).unwrap();
        Self {
            inventory: root.join("inventory.json"),
            output: root.join("result.json"),
            game,
            root,
        }
    }
    fn populate(
        &self,
        members: usize,
        frames: usize,
        bad_hash: bool,
        duplicate: bool,
        missing: bool,
    ) {
        let mut zip = zip::ZipWriter::new(File::create(self.game.join("alr.dat")).unwrap());
        let mut assets = serde_json::Map::new();
        for index in 0..members {
            let data = alr(if index == 67 && frames == 167 {
                156
            } else {
                frames
            });
            let name = format!("particle{index:02}.alr");
            if !(missing && index == 0) {
                zip.start_file(name.to_uppercase(), zip::write::FileOptions::default())
                    .unwrap();
                zip.write_all(&data).unwrap();
            }
            if duplicate && index == 0 {
                zip.start_file(&name, zip::write::FileOptions::default())
                    .unwrap();
                zip.write_all(&data).unwrap();
            }
            let sha = if bad_hash && index == 0 {
                "0".repeat(64)
            } else {
                format!("{:x}", Sha256::digest(&data))
            };
            assets.insert(name, json!({"sha256":sha}));
        }
        zip.finish().unwrap();
        fs::write(
            &self.inventory,
            serde_json::to_vec(&json!({"target_alr_assets":assets})).unwrap(),
        )
        .unwrap();
    }
    fn execute(&self) -> Result<alr_scanline_differential::Summary, String> {
        run(
            &self.game,
            &self.inventory,
            &self.output,
            None,
            std::slice::from_ref(&self.root),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Independently assembled on-disk bytes: no current/frozen encode helper used.
// Frame one supplies two palettes; second row uses two runs and opaque zero.
fn alr(frames: usize) -> Vec<u8> {
    let mut words = [0u32; 23];
    words[0] = 0x41524c41;
    words[1] = 5;
    words[3] = frames as u32;
    words[4] = 8;
    words[6] = 2;
    words[7] = 3;
    words[8] = 2;
    words[15] = 40;
    words[16] = 50;
    let mut bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let data = [0, 1, 2, 2, 0x81, 0, 1, 0];
    for word in [
        u32::MAX,
        32,
        65535 | (32768 << 16),
        6 | (2 << 11) | (3 << 22),
        data.len() as u32,
        0x00112233,
        0x00445566,
        0x00778899,
        0x00aabbcc,
        0x00ddeeff,
        0x00123456,
    ] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(data);
    for row in [0x80000000 | (1 << 21), 3 | 0x100000, 8] {
        bytes.extend(u32::to_le_bytes(row));
    }
    for _ in 1..frames {
        bytes.extend(0u32.to_le_bytes());
    }
    bytes.extend(b"ignored trailing metadata");
    bytes
}

#[test]
fn synthetic_full_scope_cli_and_create_new() {
    let f = Fixture::new();
    f.populate(68, 167, false, false, false);
    let source_before = fs::read(f.game.join("alr.dat")).unwrap();
    let mtime_before = fs::metadata(f.game.join("alr.dat"))
        .unwrap()
        .modified()
        .unwrap();
    let args: Vec<OsString> = [&f.game, &f.inventory, &f.output]
        .iter()
        .map(|p| p.as_os_str().to_owned())
        .collect();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        cli(&args, &mut stdout, &mut stderr),
        0,
        "{}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(stderr.is_empty());
    assert!(String::from_utf8(stdout).unwrap().starts_with("members=68 frames=11345 decodes=22690 pixels=272280 failures=0 source_unchanged=true output="));
    let bytes = fs::read(&f.output).unwrap();
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["members"], 68);
    assert_eq!(report["frames"], 11345);
    assert_eq!(report["decodes"], 22690);
    assert_eq!(report["pixels"], 272280);
    let expected_dll = std::env::var_os("ARM_NATIVE_DATA_DLL")
        .map(|p| Value::String(format!("{:x}", Sha256::digest(fs::read(p).unwrap()))))
        .unwrap_or(Value::Null);
    assert_eq!(report["native_dll_sha256"], expected_dll);
    assert_eq!(report["records"].as_array().unwrap().len(), 68);
    assert_eq!(report["records"][0]["member"], "particle00.alr");
    assert_eq!(report["source_unchanged"], true);
    assert_eq!(fs::read(f.game.join("alr.dat")).unwrap(), source_before);
    assert_eq!(
        fs::metadata(f.game.join("alr.dat"))
            .unwrap()
            .modified()
            .unwrap(),
        mtime_before
    );
    assert_eq!(cli(&args, &mut Vec::new(), &mut Vec::new()), 1);
    assert_eq!(fs::read(&f.output).unwrap(), bytes);
}

#[test]
fn independent_parsers_and_known_pixels() {
    let data = alr(2);
    let current = against_rome_data::AlrDocument::parse(&data).unwrap();
    let frozen = alr_scanline_differential::frozen::Document::parse(&data).unwrap();
    assert_eq!(frozen.frames[0].info.offset_x, 65535);
    assert_eq!(frozen.frames[0].info.offset_y, 32768);
    assert_eq!(compare_member(&data, "synthetic").unwrap().decodes, 4);
    let expected = vec![
        0, 0xff332211, 0xff665544, 0xff998877, 0, 0, 0, 0xff665544, 0, 0, 0, 0,
    ];
    // row 2 first zero transparent; second run's zero is opaque.
    let mut expected = expected;
    expected[6 + 3] = 0xff332211;
    for index in 0..2 {
        assert_eq!(current.decode_frame_argb(index, 0).unwrap(), expected);
        assert_eq!(frozen.decode_frame_argb(index, 0).unwrap(), expected);
        assert_eq!(
            current.decode_frame_argb(index, 1).unwrap(),
            frozen.decode_frame_argb(index, 1).unwrap()
        );
    }
    assert_ne!(
        frozen.decode_frame_argb(0, 0).unwrap(),
        frozen.decode_frame_argb(0, 1).unwrap()
    );
    assert!(frozen.decode_frame_argb(2, 0).is_err());
    assert!(frozen.decode_frame_argb(0, 2).is_err());
}

#[test]
fn malformed_container_rejection_is_independent() {
    let valid = alr(2);
    for (offset, value) in [
        (0, 0),
        (4, 7),
        (12, 0),
        (16, 16),
        (32, 0),
        (92, 0),
        (96, 9999),
    ] {
        let mut data = valid.clone();
        data[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(
            against_rome_data::AlrDocument::parse(&data).is_err(),
            "current offset {offset}"
        );
        assert!(
            alr_scanline_differential::frozen::Document::parse(&data).is_err(),
            "frozen offset {offset}"
        );
    }
    for end in [0, 20, 91, 95, 111, 155] {
        assert!(against_rome_data::AlrDocument::parse(&valid[..end]).is_err());
        assert!(alr_scanline_differential::frozen::Document::parse(&valid[..end]).is_err());
    }
    // Valid container, invalid first row palette index.
    let mut data = valid;
    data[137] = 3;
    let current = against_rome_data::AlrDocument::parse(&data).unwrap();
    let frozen = alr_scanline_differential::frozen::Document::parse(&data).unwrap();
    assert!(current.decode_frame_argb(0, 0).is_err());
    assert!(frozen.decode_frame_argb(0, 0).is_err());
}

#[test]
fn inventory_hash_and_full_scope_fail_closed() {
    for (members, frames, bad_hash, expected) in [
        (67, 167, false, "expected 68"),
        (68, 1, false, "expected 11345"),
        (68, 167, true, "Inventory identity changed"),
    ] {
        let f = Fixture::new();
        f.populate(members, frames, bad_hash, false, false);
        assert!(f.execute().unwrap_err().contains(expected));
        assert!(!f.output.exists());
    }
}

#[test]
fn duplicate_and_missing_zip_members_fail_closed() {
    for (duplicate, missing) in [(true, false), (false, true)] {
        let f = Fixture::new();
        f.populate(68, 167, false, duplicate, missing);
        assert!(f
            .execute()
            .unwrap_err()
            .contains("Missing/ambiguous archive member"));
        assert!(!f.output.exists());
    }
}

#[test]
fn optional_dll_is_hashed_only_when_requested() {
    let f = Fixture::new();
    f.populate(68, 167, false, false, false);
    let dll = f.root.join("synthetic.dll");
    fs::write(&dll, b"identity only; never loaded").unwrap();
    let report = run(
        &f.game,
        &f.inventory,
        &f.output,
        Some(&dll),
        std::slice::from_ref(&f.root),
    )
    .unwrap();
    assert_eq!(
        report.native_dll_sha256.unwrap(),
        format!("{:x}", Sha256::digest(b"identity only; never loaded"))
    );
    let output = f.root.join("missing-dll.json");
    assert!(run(
        &f.game,
        &f.inventory,
        &output,
        Some(&f.root.join("missing.dll")),
        std::slice::from_ref(&f.root)
    )
    .is_err());
    assert!(!output.exists());

    let output = f.root.join("dll-from-environment.json");
    let result = Command::new(env!("CARGO_BIN_EXE_alr-scanline-differential-rs"))
        .env("ARM_NATIVE_DATA_DLL", &dll)
        .args([&f.game, &f.inventory, &output])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        report["native_dll_sha256"],
        format!("{:x}", Sha256::digest(b"identity only; never loaded"))
    );
}

#[test]
fn output_path_safety_and_existing_input_untouched() {
    let f = Fixture::new();
    fs::write(&f.inventory, b"sentinel").unwrap();
    let roots = std::slice::from_ref(&f.root);
    for output in [
        f.inventory.clone(),
        f.game.join("result.json"),
        f.root.join("result.JSON"),
        f.root.join("sub/../escape.json"),
        f.root.join("missing/result.json"),
    ] {
        assert!(
            validate_output(&f.game, &output, roots).is_err(),
            "{}",
            output.display()
        );
    }
    assert_eq!(fs::read(&f.inventory).unwrap(), b"sentinel");
    let outside = Fixture::new();
    assert!(validate_output(&f.game, &outside.output, roots).is_err());
    assert!(validate_output(&f.game, &f.output, roots).is_ok());
    // Directory with a .json suffix is also an existing output.
    fs::create_dir(&f.output).unwrap();
    assert!(validate_output(&f.game, &f.output, roots).is_err());
}

#[test]
fn linked_parent_cannot_escape_allowed_root() {
    let f = Fixture::new();
    let outside = Fixture::new();
    let link = f.root.join("redirect");
    #[cfg(windows)]
    {
        assert!(Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.root)
            .output()
            .unwrap()
            .status
            .success());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside.root, &link).unwrap();
    }
    assert!(validate_output(
        &f.game,
        &link.join("escaped.json"),
        std::slice::from_ref(&f.root)
    )
    .is_err());
    // Remove the link before recursive fixture cleanup.
    #[cfg(windows)]
    fs::remove_dir(&link).unwrap();
    #[cfg(unix)]
    fs::remove_file(&link).unwrap();
    assert!(!outside.root.join("escaped.json").exists());
}

#[test]
fn executable_usage_and_io_exit_codes() {
    let binary = env!("CARGO_BIN_EXE_alr-scanline-differential-rs");
    let usage = Command::new(binary).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert!(String::from_utf8(usage.stderr).unwrap().contains("Usage:"));
    let f = Fixture::new();
    let failure = Command::new(binary)
        .args([&f.game, &f.inventory, &f.output])
        .output()
        .unwrap();
    assert_eq!(failure.status.code(), Some(1));
    assert!(failure.stdout.is_empty());
    assert!(!failure.stderr.is_empty());
    assert!(!f.output.exists());
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
