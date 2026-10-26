use against_rome_data::sha256::sha256_hex;
use against_rome_textures::{
    BodenTexturesDocument, FloortexLibrary, NativeTerrainMaterialResolver, StandardFloorMaterialCatalog,
    TerrainBlendAuthoringMap, TerrainBlendEditSession,
};
use against_rome_textures::palette::build_full_catalog;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

type Rgb = [f64; 3];

fn audit_mean_color(layer: &against_rome_textures::terrain_layer::TerrainLayer, x: usize, y: usize, width: usize, height: usize) -> Rgb {
    let (mut sums, mut count) = ([0u64; 3], 0u64);
    let step_x = (width / 8).max(1);
    let step_y = (height / 8).max(1);
    let mut sy = y + step_y / 2;
    while sy < y + height {
        let mut sx = x + step_x / 2;
        while sx < x + width {
            let pixel = layer.argb[sy.min(layer.height - 1) * layer.width + sx.min(layer.width - 1)];
            sums[0] += ((pixel >> 16) & 0xff) as u64;
            sums[1] += ((pixel >> 8) & 0xff) as u64;
            sums[2] += (pixel & 0xff) as u64;
            count += 1;
            sx += step_x;
        }
        sy += step_y;
    }
    let n = count.max(1) as f64;
    [sums[0] as f64 / n, sums[1] as f64 / n, sums[2] as f64 / n]
}

fn audit_corner_colors(layer: &against_rome_textures::terrain_layer::TerrainLayer, divisor: usize) -> [Rgb; 4] {
    let (w, h) = ((layer.width / divisor).max(1), (layer.height / divisor).max(1));
    [
        audit_mean_color(layer, 0, 0, w, h),
        audit_mean_color(layer, layer.width - w, 0, w, h),
        audit_mean_color(layer, layer.width - w, layer.height - h, w, h),
        audit_mean_color(layer, 0, layer.height - h, w, h),
    ]
}

fn audit_dist(a: &Rgb, b: &Rgb) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

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
#[ignore = "requires installed retail floortex.dat; read-only 4T topology audit"]
fn test_installed_4t_tiles_use_pixel_inferred_three_material_corners() {
    let game_dir = get_game_dir();
    let mut library = FloortexLibrary::open(game_dir.join("floortex.dat")).unwrap();
    let names = library.names().to_vec();
    assert_eq!(names.len(), 3005, "retail floor archive inventory changed");

    let name_catalog = StandardFloorMaterialCatalog::from_texture_names(&names);
    let mut probe = |name: &str| library.get_layer(name);
    let pixel_catalog = build_full_catalog(&names, Some(&mut probe));
    drop(probe);
    let three_material_tiles: Vec<&String> = names
        .iter()
        .filter(|name| {
            name.len() == 8
                && name.as_bytes()[0] == b'4'
                && name.as_bytes()[1].eq_ignore_ascii_case(&b'T')
        })
        .collect();
    assert_eq!(three_material_tiles.len(), 333, "retail 4T inventory changed");

    let mut corrected = 0usize;
    let mut unresolved = 0usize;
    let mut roundtripped = 0usize;
    let mut unresolved_names = Vec::new();
    let mut invalid_name_chars = 0usize;
    let mut missing_materials = 0usize;
    let mut missing_base_variants = 0usize;
    let mut missing_pixels = 0usize;
    let mut rejected_non_three = 0usize;
    let mut rejected_distance = 0usize;
    let mut rejected_both = 0usize;
    let mut adaptive_corner_hits = [0usize; 3];
    let mut stable_adaptive_corner_hits = 0usize;
    let mut stable_16_32_tiles = Vec::new();
    let mut all_named_materials_present = 0usize;
    let mut shapes = std::collections::BTreeMap::<char, usize>::new();
    for name in &three_material_tiles {
        let Some(pixel_corners) = pixel_catalog.try_resolve_native_corners(name) else {
            unresolved += 1;
            unresolved_names.push(name.as_str());
            let bytes = name.as_bytes();
            if !bytes[6].is_ascii_alphanumeric() {
                invalid_name_chars += 1;
            } else {
                let ids: Vec<String> = bytes[2..5]
                    .iter()
                    .map(|code| format!("B{}", (*code as char).to_ascii_uppercase()))
                    .collect();
                if ids.iter().any(|id| !name_catalog.materials.iter().any(|m| m.eq_ignore_ascii_case(id))) {
                    missing_materials += 1;
                } else if ids.iter().any(|id| name_catalog.variants_by_material.get(id).is_none_or(Vec::is_empty)) {
                    missing_base_variants += 1;
                } else if library.get_layer(name).is_none() {
                    missing_pixels += 1;
                } else {
                    let refs: Option<Vec<(String, Rgb)>> = ids
                        .iter()
                        .map(|id| {
                            let variants = name_catalog.variants_by_material.get(&id.to_ascii_uppercase())?;
                            let representative = variants
                                .iter()
                                .find(|variant| variant.to_ascii_uppercase().ends_with("50"))
                                .or_else(|| variants.first())?;
                            let layer = library.get_layer(representative)?;
                            Some((id.clone(), audit_mean_color(&layer, 0, 0, layer.width, layer.height)))
                        })
                        .collect();
                    let Some(refs) = refs else {
                        missing_base_variants += 1;
                        continue;
                    };
                    let Some(tile) = library.get_layer(name) else {
                        missing_pixels += 1;
                        continue;
                    };
                    let samples = audit_corner_colors(&tile, 4);
                    let inferred: Vec<String> = samples
                        .iter()
                        .map(|sample| {
                            refs.iter()
                                .min_by(|a, b| audit_dist(sample, &a.1).total_cmp(&audit_dist(sample, &b.1)))
                                .unwrap()
                                .0
                                .clone()
                        })
                        .collect();
                    let distinct = inferred
                        .iter()
                        .map(|id| id.to_ascii_uppercase())
                        .collect::<std::collections::HashSet<_>>()
                        .len();
                    let max_distance = samples
                        .iter()
                        .map(|sample| refs.iter().map(|reference| audit_dist(sample, &reference.1)).fold(f64::INFINITY, f64::min))
                        .fold(0.0, f64::max);
                    let mut adaptive_assignments = Vec::<Vec<String>>::new();
                    let mut adaptive_confidence = Vec::<bool>::new();
                    let mut adaptive_max_distances = Vec::<f64>::new();
                    for (index, divisor) in [8usize, 16, 32].iter().enumerate() {
                        let smaller_samples = audit_corner_colors(&tile, *divisor);
                        let smaller_ids: Vec<String> = smaller_samples
                            .iter()
                            .map(|sample| {
                                refs.iter()
                                    .min_by(|a, b| audit_dist(sample, &a.1).total_cmp(&audit_dist(sample, &b.1)))
                                    .unwrap()
                                    .0
                                    .clone()
                            })
                            .collect();
                        let smaller_distinct = smaller_ids
                            .iter()
                            .map(|id| id.to_ascii_uppercase())
                            .collect::<std::collections::HashSet<_>>()
                            .len();
                        let smaller_max_distance = smaller_samples
                            .iter()
                            .map(|sample| refs.iter().map(|reference| audit_dist(sample, &reference.1)).fold(f64::INFINITY, f64::min))
                            .fold(0.0, f64::max);
                        if smaller_distinct == 3 && smaller_max_distance <= 32.0 {
                            adaptive_corner_hits[index] += 1;
                        }
                        adaptive_assignments.push(smaller_ids);
                        adaptive_confidence.push(smaller_distinct == 3 && smaller_max_distance <= 32.0);
                        adaptive_max_distances.push(smaller_max_distance);
                    }
                    if adaptive_confidence[0]
                        && adaptive_confidence[1]
                        && adaptive_assignments[0] == adaptive_assignments[1]
                        && adaptive_assignments[0]
                            .iter()
                            .map(|id| id.to_ascii_uppercase())
                            .collect::<std::collections::HashSet<_>>()
                            .len()
                            == 3
                    {
                        stable_adaptive_corner_hits += 1;
                    }
                    if adaptive_confidence[1]
                        && adaptive_confidence[2]
                        && adaptive_assignments[1] == adaptive_assignments[2]
                        && adaptive_assignments[1]
                            .iter()
                            .map(|id| id.to_ascii_uppercase())
                            .collect::<std::collections::HashSet<_>>()
                            .len()
                            == 3
                    {
                        stable_16_32_tiles.push(name.as_str());
                    }
                    println!(
                        "unresolved {name}: 1/4={inferred:?} d={max_distance:.2}; 1/8={:?} d={:.2}; 1/16={:?} d={:.2}; 1/32={:?} d={:.2}",
                        adaptive_assignments[0],
                        adaptive_max_distances[0],
                        adaptive_assignments[1],
                        adaptive_max_distances[1],
                        adaptive_assignments[2],
                        adaptive_max_distances[2],
                    );
                    println!("  nearest-color map (A/B/C = {ids:?}; one sample per 8x8 pixels):");
                    for y in (0..tile.height).step_by(8) {
                        let row: String = (0..tile.width)
                            .step_by(8)
                            .map(|x| {
                                let pixel = tile.argb[y * tile.width + x];
                                let sample = [
                                    ((pixel >> 16) & 0xff) as f64,
                                    ((pixel >> 8) & 0xff) as f64,
                                    (pixel & 0xff) as f64,
                                ];
                                let nearest = refs
                                    .iter()
                                    .enumerate()
                                    .min_by(|(_, a), (_, b)| {
                                        audit_dist(&sample, &a.1).total_cmp(&audit_dist(&sample, &b.1))
                                    })
                                    .map(|(index, _)| index)
                                    .unwrap_or(3);
                                char::from(b'A' + nearest as u8)
                            })
                            .collect();
                        println!("  {row}");
                    }
                    let mut full_tile_counts = std::collections::BTreeMap::<String, usize>::new();
                    for pixel in &tile.argb {
                        let sample = [((pixel >> 16) & 0xff) as f64, ((pixel >> 8) & 0xff) as f64, (pixel & 0xff) as f64];
                        let nearest = refs
                            .iter()
                            .min_by(|a, b| audit_dist(&sample, &a.1).total_cmp(&audit_dist(&sample, &b.1)))
                            .unwrap();
                        *full_tile_counts.entry(nearest.0.clone()).or_default() += 1;
                    }
                    if ids.iter().all(|id| full_tile_counts.get(id).is_some_and(|count| *count > 0)) {
                        all_named_materials_present += 1;
                    }
                    let non_three = distinct != 3;
                    let over_distance = max_distance > 32.0;
                    match (non_three, over_distance) {
                        (true, true) => rejected_both += 1,
                        (true, false) => rejected_non_three += 1,
                        (false, true) => rejected_distance += 1,
                        (false, false) => panic!("{name} audit rejected despite meeting both pixel gates"),
                    }
                }
            }
            continue;
        };
        let distinct: std::collections::HashSet<String> = pixel_corners
            .iter()
            .map(|material| material.to_ascii_uppercase())
            .collect();
        assert_eq!(distinct.len(), 3, "{name} did not resolve to three materials");
        let corner_refs = pixel_corners.each_ref().map(String::as_str);
        let baked = pixel_catalog
            .resolve_native_tile(&corner_refs, 0, 0)
            .unwrap_or_else(|| panic!("{name} corner topology did not bake"));
        assert_eq!(
            pixel_catalog.try_resolve_native_corners(&baked).as_ref(),
            Some(&pixel_corners),
            "{name} did not round-trip through native tile {baked}"
        );
        roundtripped += 1;
        *shapes.entry(name.as_bytes()[6] as char).or_default() += 1;
        if name_catalog.try_resolve_native_corners(name).as_ref() != Some(&pixel_corners) {
            corrected += 1;
        }
    }

    println!(
        "retail 4T tiles: {}, pixel-resolved: {}, round-tripped: {}, name mapping corrected: {}, unresolved: {}, by shape character: {:?}",
        three_material_tiles.len(),
        three_material_tiles.len() - unresolved,
        roundtripped,
        corrected,
        unresolved,
        shapes,
    );
    println!("unresolved examples: {:?}", unresolved_names.iter().take(24).collect::<Vec<_>>());
    println!("unresolved causes (invalid name / missing material / missing base variants / missing BMP): {invalid_name_chars}/{missing_materials}/{missing_base_variants}/{missing_pixels}; pixel evidence rejected: {}", unresolved - invalid_name_chars - missing_materials - missing_base_variants - missing_pixels);
    println!("pixel gate rejection (not exactly 3 / distance > 32 / both): {rejected_non_three}/{rejected_distance}/{rejected_both}");
    println!("rejected tiles recovered by smaller corner windows 1/8, 1/16, 1/32: {:?}", adaptive_corner_hits);
    println!("tiles whose 1/8 and 1/16 corner assignments agree on three materials: {stable_adaptive_corner_hits}");
    println!("tiles whose 1/16 and 1/32 assignments agree with the 3-material distance gate: {stable_16_32_tiles:?}");
    println!("unresolved tiles with all three named material colors present across the full tile: {all_named_materials_present}");
    assert!(corrected > 0, "pixel probing did not correct any name-derived 4T topology");
    assert!(three_material_tiles.len() - unresolved >= 308, "pixel inference coverage regressed");
}

#[test]
#[ignore = "requires installed retail maps; run the explicit -Retail verification gate"]
fn test_installed_maps_read_only_and_temp_hash_preservation() {
    let game_dir = get_game_dir();
    let sample_maps = ["TUTOR_01", "ENDL_000", "KAMP_002", "MP_000"];
    let temp_root = crate::temp_fixture::dir("ar_textures_installed");
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
    let temp_root = crate::temp_fixture::dir("ar_textures_tutor");
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

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
