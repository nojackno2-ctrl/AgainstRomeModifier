use particle_render_contract::{hash, Pe, Result};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    path::Path,
};

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--definitions") {
        return audit_definitions(&args);
    }
    if args.len() < 3 {
        return Err("usage: EXE NEW_OUTPUT.json VA:HEX_LENGTH ...".into());
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .ok_or("repo")?
        .canonicalize()?;
    let output = Path::new(&args[1]);
    if output.exists()
        || !output
            .parent()
            .ok_or("parent")?
            .canonicalize()?
            .starts_with(repo.join("re_workspace"))
        || output.extension().is_none_or(|s| s != "json")
    {
        return Err("output must be a new JSON in re_workspace".into());
    }
    let before = fs::read(&args[0])?;
    let digest = hash(&before);
    if digest != "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf" {
        return Err("source EXE identity mismatch".into());
    }
    let pe = Pe::parse(&before)?;
    let mut windows = Vec::new();
    for window in &args[2..] {
        let raw = window.starts_with("raw:");
        let window = window.strip_prefix("raw:").unwrap_or(window);
        let (start, length) = window.split_once(':').ok_or("VA:HEX_LENGTH")?;
        let start = u64::from_str_radix(start.trim_start_matches("0x"), 16)?;
        let length = usize::from_str_radix(length.trim_start_matches("0x"), 16)?;
        if length == 0 || length > 16384 {
            return Err("window length outside 1..16384".into());
        }
        if raw {
            windows.push(json!({"start":format!("{start:#x}"),"length":length,
                "sha256":hash(pe.read(start,length)?),"bytes_hex":particle_render_contract::hex(pe.read(start,length)?)}));
            continue;
        }
        let instructions = pe
            .decode(start, length)?
            .iter()
            .map(|i| pe.item(i))
            .collect::<Result<Vec<_>>>()?;
        windows.push(json!({"start":format!("{start:#x}"),"length":length,
            "sha256":hash(pe.read(start,length)?),"instructions":instructions}));
    }
    if before != fs::read(&args[0])? {
        return Err("source EXE changed".into());
    }
    let report = json!({"scope":"Explicit byte windows; linear decode does not establish function ownership or reachability",
        "subject_sha256":digest,"source_unchanged":true,"windows":windows});
    serde_json::to_writer_pretty(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
        &report,
    )?;
    println!(
        "windows={} source_unchanged=true output={}",
        windows.len(),
        output.display()
    );
    Ok(())
}
fn audit_definitions(args: &[String]) -> Result<()> {
    use against_rome_data::particle_definitions::{
        definition_chain, geometry_definitions, grafik_definitions, object_particle_definitions,
        parameter_definitions,
    };
    if args.len() != 4 {
        return Err("usage: --definitions GAME_ROOT INVENTORY.json NEW_OUTPUT.json".into());
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .ok_or("repo")?
        .canonicalize()?;
    let output = Path::new(&args[3]);
    if output.exists()
        || !output
            .parent()
            .ok_or("parent")?
            .canonicalize()?
            .starts_with(repo.join("re_workspace"))
        || output.extension().is_none_or(|s| s != "json")
    {
        return Err("output must be new JSON in re_workspace".into());
    }
    let inventory: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut reports = Vec::new();
    let mut assertions = 0;
    let mut parameters = std::collections::BTreeMap::new();
    let mut geometries = std::collections::BTreeMap::new();
    let mut grafiks = std::collections::BTreeMap::new();
    let mut objects = std::collections::BTreeMap::new();
    for (key, filename) in [
        ("partpar", "partpar.dau"),
        ("partgfx", "partgfx.dau"),
        ("partgeo", "partgeo.dau"),
        ("objdef", "objdef.dau"),
    ] {
        let path = Path::new(&args[1])
            .join("SYSTEM/DATA_MP/DEFAULTS")
            .join(filename);
        let before = fs::read(&path)?;
        let metadata = fs::metadata(&path)?;
        let mut catalog = serde_json::Map::new();
        if key == "partpar" {
            parameters = parameter_definitions(&before).map_err(|e| format!("{filename}: {e}"))?;
            for (id, row) in &parameters {
                catalog.insert(
                    id.to_string(),
                    json!({"name":row.name,"numeric_raw":row.numeric_raw}),
                );
            }
        } else if key == "partgfx" {
            grafiks = grafik_definitions(&before).map_err(|e| format!("{filename}: {e}"))?;
            for (id, row) in &grafiks {
                catalog.insert(
                    id.to_string(),
                    json!({"name":row.name,"numeric_raw":row.numeric_raw}),
                );
            }
        } else if key == "partgeo" {
            geometries = geometry_definitions(&before).map_err(|e| format!("{filename}: {e}"))?;
            for (id, row) in &geometries {
                catalog.insert(
                    id.to_string(),
                    json!({"name":row.name,"numeric_raw":row.numeric_raw}),
                );
            }
        } else {
            objects =
                object_particle_definitions(&before).map_err(|e| format!("{filename}: {e}"))?;
            for (id, row) in &objects {
                catalog.insert(
                    id.to_string(),
                    json!({"name":row.name,"numeric_raw":row.numeric_raw}),
                );
            }
        }
        let oracle = inventory["tables"][key]["rows"]
            .as_array()
            .ok_or("inventory rows")?;
        if catalog.len() as u64
            != inventory["tables"][key]["row_count"]
                .as_u64()
                .ok_or("inventory row_count")?
        {
            return Err("catalog row count mismatch".into());
        }
        for row in oracle {
            let id = row["id"].as_i64().ok_or("oracle id")?.to_string();
            let expected = row["raw_values"].as_array().ok_or("oracle values")?;
            let actual = catalog.get(&id).ok_or("missing catalog id")?;
            let name_column = if key == "objdef" { 52 } else { 2 };
            if actual["name"].as_str() != expected[name_column].as_str().map(str::trim) {
                return Err("name mismatch".into());
            }
            let numeric = actual["numeric_raw"].as_array().ok_or("catalog numeric")?;
            let indices: Vec<_> = if key == "objdef" {
                vec![1, 2, 5, 63, 64, 65]
            } else {
                std::iter::once(1).chain(3..expected.len()).collect()
            };
            for (position, index) in indices.into_iter().enumerate() {
                let word = expected[index]
                    .as_str()
                    .ok_or("oracle numeric")?
                    .trim()
                    .parse::<i32>()?;
                if numeric[position].as_i64() != Some(word as i64) {
                    return Err("raw numeric mismatch".into());
                }
                assertions += 1;
            }
        }
        let after = fs::metadata(&path)?;
        if before != fs::read(&path)?
            || metadata.len() != after.len()
            || metadata.modified()? != after.modified()?
        {
            return Err("source changed".into());
        }
        reports.push(
            json!({"table":key,"sha256":hash(&before),"rows":catalog.len(),"oracle_rows_compared":oracle.len(),"catalog":catalog}),
        );
    }
    let mut chains = Vec::new();
    let mut resolved = 0;
    for (id, owner) in &objects {
        if [owner.parameter_id(), owner.geometry_id(), owner.grafik_id()]
            .iter()
            .all(|v| *v == -1)
        {
            continue;
        }
        match definition_chain(*id, &objects, &parameters, &geometries, &grafiks) {
            Ok(chain) => {
                resolved += 1;
                chains.push(json!({"owner_type":id,"status":"resolved_base_definitions_only",
                "parameter":chain.parameter.id,"geometry":chain.geometry.id,"grafik":chain.grafik.id,
                "grafik_object":chain.grafik_object.id,"alr_id":chain.grafik_object.alr_id(),
                "definition_lifetime":chain.parameter.lifetime(),"type_divisor":chain.grafik_object.animation_length()}));
            }
            Err(error) => chains
                .push(json!({"owner_type":id,"status":
                if error==against_rome_data::particle_definitions::DefinitionChainError::NoPrimaryGrafikObject {"no_primary_object"}else{"unresolved"},
                "reason":format!("{error:?}")})),
        }
    }
    serde_json::to_writer_pretty(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
        &json!({"scope":"Read-only base parameter/gfx/geometry/object fields; raw-word comparison and ID chains, not native draw execution",
        "assertions":assertions,"source_unchanged":true,"tables":reports,"definition_chains":chains,"resolved_chains":resolved}),
    )?;
    println!(
        "tables={} assertions={assertions} chains={} resolved={resolved} source_unchanged=true output={}",
        reports.len(),
        chains.len(),
        output.display()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
