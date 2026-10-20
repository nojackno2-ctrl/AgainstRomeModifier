use against_rome_textures::{
    BodenTexturesDocument, FloortexLibrary, StandardFloorMaterialCatalog, TerrainBlendAuthoringMap,
    TerrainBlendEditSession, TextureResourceKind,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "audit" => {
            if args.len() < 3 {
                eprintln!("Usage: ar-textures audit <map_dir_or_boden_txt>");
                std::process::exit(1);
            }
            run_audit(&args[2]);
        }
        "roundtrip-test" => {
            if args.len() < 4 {
                eprintln!("Usage: ar-textures roundtrip-test <map_dir_or_boden_txt> <temp_dir>");
                std::process::exit(1);
            }
            run_roundtrip_test(&args[2], &args[3]);
        }
        "paint-test" => {
            if args.len() < 7 {
                eprintln!("Usage: ar-textures paint-test <map_path> <temp_dir> <cx> <cy> <radius> <material>");
                std::process::exit(1);
            }
            let cx: f32 = args[4].parse().expect("Invalid cx");
            let cy: f32 = args[5].parse().expect("Invalid cy");
            let radius: f32 = args[6].parse().expect("Invalid radius");
            let material = if args.len() > 7 { &args[7] } else { "BB" };
            run_paint_test(&args[2], &args[3], cx, cy, radius, material);
        }
        "verify-all-installed" => {
            let game_dir = if args.len() > 2 {
                &args[2]
            } else {
                r"C:\Program Files (x86)\Against Rome"
            };
            run_verify_all_installed(game_dir);
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("Against Rome Terrain Textures Tool v{}", against_rome_textures::version());
    println!();
    println!("Commands:");
    println!("  audit <path>                         Audit boden.txt dimensions, entries, and format");
    println!("  roundtrip-test <map_path> <temp_dir> Verify bit-for-bit SHA256 preservation on save");
    println!("  paint-test <map> <temp> <x> <y> <r> <mat> Perform transactional paint in TEMP and verify");
    println!("  verify-all-installed [game_dir]      Audit all installed MAPS/*/boden.txt read-only");
}

fn resolve_boden_path(input: &str) -> PathBuf {
    match TextureResourceKind::resolve(input) {
        TextureResourceKind::BodenText(p) => p,
        TextureResourceKind::AliasRedirected { target, .. } => target,
        _ => PathBuf::from(input),
    }
}

fn run_audit(path_str: &str) {
    let path = resolve_boden_path(path_str);
    println!("Auditing texture document: {}", path.display());
    let doc = match BodenTexturesDocument::load(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading document: {}", e);
            std::process::exit(1);
        }
    };

    let textures = doc.textures();
    let mut unique = textures.clone();
    unique.sort();
    unique.dedup();

    println!("Dimension:       {}x{}", doc.dimension(), doc.dimension());
    println!("Total tiles:     {}", doc.tile_count());
    println!("Unique textures: {}", unique.len());

    // Build catalog to see resolution status
    let catalog = StandardFloorMaterialCatalog::from_texture_names(&unique);
    let import = TerrainBlendAuthoringMap::import(doc.dimension(), &textures, &catalog, "BB")
        .expect("Import authoring map");

    println!("Unresolved:      {}", import.unresolved_tile_indices.len());
    println!("Conflicts:       {}", import.corner_conflicts.len());
    println!("Audit completed successfully.");
}

fn run_roundtrip_test(src_str: &str, temp_dir: &str) {
    let src = resolve_boden_path(src_str);
    let orig_bytes = fs::read(&src).expect("Failed to read source boden.txt");
    let orig_sha = compute_sha256(&orig_bytes);

    let temp_path = Path::new(temp_dir).join("test_boden.txt");
    if let Some(parent) = temp_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&temp_path, &orig_bytes).expect("Failed to write to temp");

    let mut doc = BodenTexturesDocument::load(&temp_path).expect("Load doc from temp");
    // Batch set same textures
    let current = doc.textures();
    doc.set_textures(&current).expect("Set textures same");
    doc.save(&temp_path).expect("Save doc");

    let saved_bytes = fs::read(&temp_path).expect("Read saved bytes");
    let saved_sha = compute_sha256(&saved_bytes);

    println!("Original SHA256: {}", orig_sha);
    println!("Saved SHA256:    {}", saved_sha);

    if orig_sha == saved_sha {
        println!("PASSED: SHA256 bit-for-bit identical invariant confirmed.");
    } else {
        eprintln!("FAILED: Hash mismatch!");
        std::process::exit(1);
    }
}

fn run_paint_test(src_str: &str, temp_dir: &str, cx: f32, cy: f32, radius: f32, material: &str) {
    let src = resolve_boden_path(src_str);
    let orig_bytes = fs::read(&src).expect("Failed to read source");
    let temp_path = Path::new(temp_dir).join("paint_boden.txt");
    if let Some(parent) = temp_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&temp_path, &orig_bytes).expect("Write to temp");

    let mut doc = BodenTexturesDocument::load(&temp_path).expect("Load doc");
    let textures = doc.textures();
    let game_floortex = Path::new(r"C:\Program Files (x86)\Against Rome\floortex.dat");
    let catalog = if game_floortex.is_file() {
        FloortexLibrary::open(game_floortex)
            .map(|lib| std::sync::Arc::new(StandardFloorMaterialCatalog::from_texture_names(lib.names())))
            .unwrap_or_else(|_| std::sync::Arc::new(StandardFloorMaterialCatalog::from_texture_names(&textures)))
    } else {
        std::sync::Arc::new(StandardFloorMaterialCatalog::from_texture_names(&textures))
    };

    let import = TerrainBlendAuthoringMap::import(doc.dimension(), &textures, catalog.as_ref(), "BB")
        .expect("Import");

    let mut session =
        TerrainBlendEditSession::new(import, &textures, catalog).expect("Create session");

    println!("Painting circle at ({}, {}) r={} mat={}...", cx, cy, radius, material);
    let result = session
        .paint_circle(cx, cy, radius, material, true, true, None)
        .expect("Paint circle");

    if !result.succeeded {
        eprintln!("Paint rejected (issues: {})", result.issues.len());
        for issue in result.issues.iter().take(5) {
            eprintln!("  at ({}, {}): {:?}", issue.tile_x, issue.tile_y, issue.corner_material_ids);
        }
        std::process::exit(1);
    }

    println!("Paint succeeded: {} tile changes.", result.texture_changes.len());
    assert!(session.is_dirty());
    assert!(session.can_undo());

    // Test Undo
    let undone = session.undo().expect("Undo");
    println!("Undone {} changes. Dirty={}", undone.len(), session.is_dirty());

    // Test Redo
    let redone = session.redo().expect("Redo");
    println!("Redone {} changes. Dirty={}", redone.len(), session.is_dirty());

    // Commit baseline and write to doc
    session.commit_baseline();
    doc.set_textures(session.current_textures()).expect("Update doc");
    doc.save(&temp_path).expect("Save doc");

    // Verify reloaded document
    let reloaded = BodenTexturesDocument::load(&temp_path).expect("Reload doc");
    assert_eq!(reloaded.textures(), session.current_textures());
    println!("PASSED: Transactional paint and reload verified successfully.");
}

fn run_verify_all_installed(game_dir: &str) {
    let maps_dir = Path::new(game_dir).join("MAPS");
    if !maps_dir.is_dir() {
        eprintln!("MAPS directory not found: {}", maps_dir.display());
        std::process::exit(1);
    }

    println!("Auditing all installed maps under: {}", maps_dir.display());
    let mut total_maps = 0;
    let mut total_tiles = 0;

    for entry in fs::read_dir(maps_dir).expect("Read MAPS dir") {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() {
                let boden = path.join("boden.txt");
                if boden.is_file() {
                    match BodenTexturesDocument::load(&boden) {
                        Ok(doc) => {
                            total_maps += 1;
                            total_tiles += doc.tile_count();
                            println!(
                                "  Map {:<15} dim {:<2} tiles: {}",
                                path.file_name().unwrap().to_string_lossy(),
                                doc.dimension(),
                                doc.tile_count()
                            );
                        }
                        Err(e) => {
                            eprintln!("  Error in {}: {}", boden.display(), e);
                        }
                    }
                }
            }
        }
    }

    println!();
    println!("Total verified maps:  {}", total_maps);
    println!("Total verified tiles: {}", total_tiles);
}

fn compute_sha256(bytes: &[u8]) -> String {
    against_rome_data::sha256::sha256_hex(bytes)
}
