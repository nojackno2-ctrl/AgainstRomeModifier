use against_rome_data::sha256::sha256_hex;
use against_rome_textures::{
    BodenTexturesDocument, FloortexLibrary, StandardFloorMaterialCatalog,
    TerrainBlendAuthoringMap, TerrainBlendEditSession,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn get_game_dir() -> PathBuf {
    let default_path = PathBuf::from(r"C:\Program Files (x86)\Against Rome");
    assert!(
        default_path.join("MAPS").is_dir(),
        "Installed Against Rome game directory not found at {}",
        default_path.display()
    );
    default_path
}

#[test]
#[ignore = "requires installed retail maps; run the explicit -Retail verification gate"]
fn test_installed_maps_read_only_and_temp_hash_preservation() {
    let game_dir = get_game_dir();
    let sample_maps = ["TUTOR_01", "ENDL_000", "KAMP_002", "MP_000"];
    let temp_root = env::temp_dir().join(format!("ar_textures_installed_{}", std::process::id()));
    fs::create_dir_all(&temp_root).unwrap();

    for map_name in sample_maps {
        let original_path = game_dir.join("MAPS").join(map_name).join("boden.txt");
        assert!(
            original_path.is_file(),
            "Required map boden.txt not found at {}",
            original_path.display()
        );

        // 1. Read-only load and SHA256 verification (never modifying original)
        let original_bytes = fs::read(&original_path).unwrap();
        let original_hash = sha256_hex(&original_bytes);

        let doc = BodenTexturesDocument::load(&original_path).unwrap();
        assert!(doc.dimension() > 0);
        assert_eq!(doc.textures().len(), doc.dimension() * doc.dimension());
        assert!(doc.is_pfil(), "Installed map {} must be PFIL compressed", map_name);

        // 2. Copy to TEMP, save unchanged, and verify exact bit-for-bit SHA256 identity
        let temp_file = temp_root.join(format!("{}_boden.txt", map_name));
        fs::write(&temp_file, &original_bytes).unwrap();

        let mut temp_doc = BodenTexturesDocument::load(&temp_file).unwrap();
        let textures = temp_doc.textures();
        temp_doc.set_textures(&textures).unwrap();
        temp_doc.save(&temp_file).unwrap();

        let saved_bytes = fs::read(&temp_file).unwrap();
        let saved_hash = sha256_hex(&saved_bytes);

        assert_eq!(
            original_hash, saved_hash,
            "Map {} failed bit-for-bit SHA256 preservation in TEMP",
            map_name
        );

        // Clean up TEMP files
        let _ = fs::remove_file(&temp_file);
        let _ = fs::remove_file(temp_root.join(format!("{}_boden.txt.bak", map_name)));
    }

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
#[ignore = "requires installed TUTOR_01 and floortex.dat; edits only TEMP copies"]
fn test_tutor_01_real_map_pfil_decode_edit_undo_and_safe_save() {
    let game_dir = get_game_dir();
    let original_path = game_dir.join("MAPS").join("TUTOR_01").join("boden.txt");
    assert!(
        original_path.is_file(),
        "TUTOR_01 boden.txt not found at {}",
        original_path.display()
    );

    let original_bytes = fs::read(&original_path).unwrap();
    let original_hash = sha256_hex(&original_bytes);

    // 1. Verify read-only PFIL decoding on real TUTOR_01
    let doc = BodenTexturesDocument::load(&original_path).unwrap();
    assert_eq!(doc.dimension(), 48, "TUTOR_01 tile dimension must be 48");
    assert_eq!(doc.textures().len(), 48 * 48, "TUTOR_01 tile count must be 2304");
    assert!(doc.is_pfil(), "TUTOR_01 must be recognized as PFIL");

    // 2. Copy TUTOR_01 to TEMP directory
    let temp_root = env::temp_dir().join(format!("ar_textures_tutor_{}", std::process::id()));
    fs::create_dir_all(&temp_root).unwrap();
    let temp_file = temp_root.join("boden.txt");
    fs::write(&temp_file, &original_bytes).unwrap();

    // 3. Test safe save: read-only preflight rejection
    let writable_perms = fs::metadata(&temp_file).unwrap().permissions();
    let mut ro_perms = writable_perms.clone();
    ro_perms.set_readonly(true);
    fs::set_permissions(&temp_file, ro_perms).unwrap();

    let mut temp_doc = BodenTexturesDocument::load(&temp_file).unwrap();
    temp_doc.set_texture(0, 0, "4B6___51").unwrap();
    let ro_save_result = temp_doc.save(&temp_file);
    assert!(
        ro_save_result.is_err(),
        "Saving to read-only file must be rejected"
    );

    // Verify TEMP file content on disk was not modified
    assert_eq!(
        sha256_hex(&fs::read(&temp_file).unwrap()),
        original_hash,
        "Read-only file must remain completely unmodified"
    );

    // Restore writable permissions
    fs::set_permissions(&temp_file, writable_perms).unwrap();

    // 4. Test safe save: backup creation
    temp_doc.save(&temp_file).unwrap();
    let backup_file = temp_root.join("boden.txt.bak");
    assert!(
        backup_file.is_file(),
        "Safe save must create boden.txt.bak backup"
    );
    assert_eq!(
        sha256_hex(&fs::read(&backup_file).unwrap()),
        original_hash,
        "Backup file must contain exact original bytes"
    );

    // 5. Test real map blend editing on TEMP copy using floortex.dat catalog
    let floortex_path = game_dir.join("floortex.dat");
    assert!(
        floortex_path.is_file(),
        "floortex.dat must exist at {}",
        floortex_path.display()
    );

    let floortex = FloortexLibrary::open(&floortex_path).unwrap();
    assert_eq!(
        floortex.names().len(),
        3005,
        "floortex.dat must contain exactly 3005 floor textures"
    );

    // Reset TEMP copy to original bytes for clean paint test
    fs::write(&temp_file, &original_bytes).unwrap();
    let mut clean_doc = BodenTexturesDocument::load(&temp_file).unwrap();
    let original_textures = clean_doc.textures();

    let catalog = Arc::new(StandardFloorMaterialCatalog::from_texture_names(floortex.names()));
    let import = TerrainBlendAuthoringMap::import(
        clean_doc.dimension(),
        &original_textures,
        catalog.as_ref(),
        "B6",
    )
    .unwrap();

    let mut session =
        TerrainBlendEditSession::new(import, &original_textures, catalog).unwrap();

    // Paint B7 circle at (12, 0) radius 1.0 (where B6 and B7 transitions exist)
    let paint_res = session
        .paint_circle(12.0, 0.0, 1.0, "B7", true, true, None)
        .unwrap();

    // Verify paint succeeded without fake-positives
    assert!(
        paint_res.succeeded,
        "Paint on real TUTOR_01 must succeed, but got issues: {:?}",
        paint_res.issues
    );
    assert_eq!(
        paint_res.texture_changes.len(),
        11,
        "Painting B7 at (12, 0) r=1 must produce exactly 11 changed tiles"
    );
    assert!(session.is_dirty(), "Session must be dirty after paint");
    assert!(session.can_undo(), "Session must support undo after paint");

    // Test Undo
    let undone = session.undo().unwrap();
    assert_eq!(undone.len(), 11, "Undo must revert all 11 tile changes");
    assert!(!session.is_dirty(), "Session must not be dirty after undo to baseline");
    assert_eq!(
        session.current_textures(),
        original_textures.as_slice(),
        "Session textures after undo must equal original TUTOR_01 textures"
    );

    // Test Redo
    let redone = session.redo().unwrap();
    assert_eq!(redone.len(), 11, "Redo must reapply all 11 tile changes");
    assert!(session.is_dirty(), "Session must be dirty after redo");

    // Commit baseline, apply to document, and save to TEMP file
    session.commit_baseline();
    assert!(!session.is_dirty(), "Session must be clean after baseline commit");
    clean_doc.set_textures(session.current_textures()).unwrap();
    clean_doc.save(&temp_file).unwrap();

    // Reload from TEMP file and verify PFIL roundtrip
    let reloaded = BodenTexturesDocument::load(&temp_file).unwrap();
    assert!(reloaded.is_pfil(), "Saved document must retain PFIL format");
    assert_eq!(
        reloaded.textures(),
        session.current_textures(),
        "Reloaded textures must match session state"
    );

    // Clean up TEMP directory
    let _ = fs::remove_dir_all(&temp_root);
}
