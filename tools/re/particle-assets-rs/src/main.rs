mod alr;
mod archive;
mod table;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use table::Table;
pub fn digest(b: &[u8]) -> String {
    table::hex(&Sha256::digest(b))
}
fn fingerprint(p: &Path, b: &[u8]) -> Result<Value, String> {
    let m = fs::metadata(p).map_err(|e| e.to_string())?;
    if m.len() != b.len() as u64 {
        return Err("file length changed while reading".into());
    }
    Ok(
        json!({"path":p.to_string_lossy(),"length":m.len(),"sha256":digest(b),"mtime_unix_ns":m.modified().map_err(|e|e.to_string())?.duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos().to_string()}),
    )
}
fn decoded(b: &[u8]) -> Result<Vec<u8>, String> {
    if against_rome_data::lzss::has_pfil_magic(b) {
        against_rome_data::lzss::decompress_pfil(b, None).map_err(|e| e.to_string())
    } else {
        Ok(b.to_vec())
    }
}
fn audit(root: &Path) -> Result<Value, String> {
    let specs = [
        (
            "partpar",
            "SYSTEM/DATA_MP/DEFAULTS/partpar.dau",
            "ParticleParameterDefault",
        ),
        (
            "partgfx",
            "SYSTEM/DATA_MP/DEFAULTS/partgfx.dau",
            "ParticleGrafikDefault",
        ),
        (
            "partgeo",
            "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
            "ParticleGeometrieDefault",
        ),
        (
            "objdef",
            "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
            "ObjectDefaults",
        ),
        ("cl_alr", "SYSTEM/cl_alr.ini", "AlrNames"),
        ("alr_zip", "alr.dat", ""),
    ];
    let mut sources = Vec::new();
    let mut before = BTreeMap::new();
    let mut tables = BTreeMap::new();
    let mut descriptions = BTreeMap::new();
    let mut zip_bytes = Vec::new();
    let mut members = Vec::new();
    for (key, rel, section) in specs {
        let p = root.join(rel);
        match fs::read(&p) {
            Err(e) => {
                sources.push(json!({"path":p.to_string_lossy(),"status":"missing_or_unreadable","error":e.to_string()}));
                continue;
            }
            Ok(b) => {
                let f = fingerprint(&p, &b)?;
                before.insert(p.clone(), f.clone());
                let mut src = json!({"before":f,"status":"read","header_bytes_hex":table::hex(&b[..b.len().min(64)])});
                if key == "alr_zip" {
                    match archive::entries(&b) {
                        Ok(entries) => {
                            members = entries;
                            src["entry_count"] = json!(members.len());
                        }
                        Err(e) => src["parse_error"] = json!(e),
                    }
                    zip_bytes = b;
                } else {
                    match decoded(&b) {
                        Err(e) => src["decode_error"] = json!(e),
                        Ok(d) => {
                            src["decoded_length"] = json!(d.len());
                            src["decoded_sha256"] = json!(digest(&d));
                            src["encoding"] = json!(if d.is_ascii() {
                                "ASCII (all bytes <= 0x7f); compatible with legacy ASCII subset"
                            } else {
                                "unresolved non-ASCII"
                            });
                            match Table::parse(&d, section, key == "cl_alr") {
                                Err(e) => src["parse_error"] = json!(e),
                                Ok(t) => {
                                    let header: Vec<_> = t
                                        .columns
                                        .iter()
                                        .enumerate()
                                        .map(|(i, n)| json!({"index_zero_based":i,"name":n}))
                                        .collect();
                                    descriptions.insert(key,json!({"section":t.section,"header_raw_hex":table::hex(t.header.as_bytes()),"columns":header,"row_count":t.rows.values().map(Vec::len).sum::<usize>(),"unique_ids":t.rows.len(),"issues":t.issues,"rows":if key.starts_with("part"){t.rows.values().flatten().map(|r|r.json()).collect::<Vec<_>>()}else{Vec::new()}}));
                                    tables.insert(key, t);
                                }
                            }
                        }
                    }
                }
                sources.push(src);
            }
        }
    }
    let mut chains = Vec::new();
    let mut objects = BTreeSet::new();
    let mut name_ids = BTreeSet::new();
    let mut assets = BTreeMap::<String, Value>::new();
    if let (Some(gfx), Some(obj), Some(names)) = (
        tables.get("partgfx"),
        tables.get("objdef"),
        tables.get("cl_alr"),
    ) {
        for rows in gfx.rows.values() {
            for row in rows {
                let mut chain = json!({"gfx_id":row.id,"gfx_name":gfx.value(row,"name").ok(),"gfx_activ_raw":gfx.value(row,"activ").ok(),"relations":[]});
                let mut relations = Vec::new();
                // Both are header-identified object references to inventory; their runtime roles remain unresolved here.
                for field in ["obdef", "objpa"] {
                    let mut r = json!({"gfx_column":field,"status":"unresolved"});
                    let outcome = (|| -> Result<(), String> {
                        if rows.len() != 1 {
                            return Err("ambiguous gfx id".into());
                        }
                        let id = gfx.int(row, field)?;
                        r["objdef_id"] = json!(id);
                        if id == -1 {
                            r["status"] = json!("negative_sentinel_no_reference");
                            return Ok(());
                        }
                        if !(0..2500).contains(&id) {
                            return Err(format!("objdef reference out of range: {id}"));
                        }
                        let o = obj.one(id)?;
                        objects.insert(id);
                        r["objdef_name"] = json!(obj.value(o, "name")?);
                        r["objdef_activ_raw"] = json!(obj.value(o, "activ")?);
                        let mut layers = Vec::new();
                        for layer in ["alrid", "alrml"] {
                            let mut l = json!({"column":layer});
                            let resolve = (|| -> Result<(), String> {
                                let aid = obj.int(o, layer)?;
                                l["alr_id"] = json!(aid);
                                if aid == -1 {
                                    l["status"] = json!("negative_sentinel_no_reference");
                                    return Ok(());
                                }
                                if aid < 0 {
                                    return Err(format!("negative ALR id {aid}"));
                                }
                                let n = names.one(aid)?;
                                name_ids.insert(aid);
                                let name = n.values[1].trim();
                                l["name"] = json!(name);
                                let e = archive::resolve(&members, name)?;
                                l["member"] = json!(e.name);
                                if !assets.contains_key(&e.name) {
                                    let a = match archive::read(&zip_bytes, e) {
                                        Err(err) => json!({"status":"unresolved","error":err}),
                                        Ok(b) => {
                                            let mut a = json!({"sha256":digest(&b),"length":b.len(),"zip_crc32":format!("{:08x}",e.crc),"zip_method":e.method,"status":"parsed"});
                                            match alr::parse(&b) {
                                                Ok(v) => a["document"] = v,
                                                Err(err) => {
                                                    a["status"] = json!("unresolved");
                                                    a["error"] = json!(err);
                                                }
                                            }
                                            a
                                        }
                                    };
                                    assets.insert(e.name.clone(), a);
                                }
                                l["status"] = assets[&e.name]["status"].clone();
                                Ok(())
                            })();
                            if let Err(e) = resolve {
                                l["status"] = json!("unresolved");
                                l["error"] = json!(e);
                            }
                            layers.push(l);
                        }
                        r["status"] = json!("objdef_resolved");
                        r["alr_layers"] = json!(layers);
                        Ok(())
                    })();
                    if let Err(e) = outcome {
                        r["error"] = json!(e);
                    }
                    relations.push(r);
                }
                chain["relations"] = json!(relations);
                chains.push(chain);
            }
        }
    }
    // Source objdef rows referencing any particle table: preserve real references independently, never join by name.
    let mut emitters = Vec::new();
    if let Some(obj) = tables.get("objdef") {
        for rows in obj.rows.values() {
            for o in rows {
                let mut links = Vec::new();
                for (col, key) in [
                    ("papar", "partpar"),
                    ("pageo", "partgeo"),
                    ("pagfx", "partgfx"),
                ] {
                    let v = obj.int(o, col);
                    if matches!(v, Ok(-1)) {
                        continue;
                    }
                    let mut l = json!({"column":col,"table":key});
                    match v {
                        Err(e) => l["error"] = json!(e),
                        Ok(id) => {
                            l["id"] = json!(id);
                            match tables
                                .get(key)
                                .ok_or("missing table".to_string())
                                .and_then(|t| t.one(id).map(|r| (t, r)))
                            {
                                Ok((t, r)) => {
                                    l["status"] = json!("resolved");
                                    l["name"] = json!(t.value(r, "name").ok());
                                }
                                Err(e) => {
                                    l["status"] = json!("unresolved");
                                    l["error"] = json!(e);
                                }
                            }
                        }
                    }
                    links.push(l);
                }
                if !links.is_empty() {
                    objects.insert(o.id);
                    emitters.push(json!({"objdef_id":o.id,"activ_raw":obj.value(o,"activ").ok(),"name":obj.value(o,"name").ok(),"links":links}));
                }
            }
        }
    }
    for (key, ids) in [("objdef", objects), ("cl_alr", name_ids)] {
        if let Some(t) = tables.get(key) {
            descriptions.get_mut(key).unwrap()["rows"] = json!(ids
                .iter()
                .flat_map(|i| t.rows.get(i).into_iter().flatten())
                .map(|r| r.json())
                .collect::<Vec<_>>());
        }
    }
    let mut checks = Vec::new();
    let mut unchanged = true;
    for (p, f) in before {
        let after = fs::read(&p)
            .map_err(|e| e.to_string())
            .and_then(|b| fingerprint(&p, &b));
        match after {
            Ok(a) => {
                let same = a == f;
                unchanged &= same;
                checks.push(json!({"before":f,"after":a,"unchanged":same}));
            }
            Err(e) => {
                unchanged = false;
                checks.push(json!({"before":f,"error":e,"unchanged":false}));
            }
        }
    }
    let parsed = assets.values().filter(|a| a["status"] == "parsed").count();
    let frames: usize = assets
        .values()
        .filter_map(|a| a["document"]["frame_count"].as_u64())
        .map(|n| n as usize)
        .sum();
    Ok(
        json!({"schema":1,"scope":"read-only installed particle default inventory; static metadata only","source_unchanged":unchanged,"sources":sources,"source_checks":checks,"tables":descriptions,"gfx_object_alr_chains":chains,"particle_referencing_objdefs":emitters,"target_alr_assets":assets,"totals":{"gfx_rows":chains.len(),"particle_referencing_objdefs":emitters.len(),"target_alr_members":assets.len(),"parsed_alr_members":parsed,"unresolved_alr_members":assets.len()-parsed,"frames":frames,"source_files":checks.len()},"limitations":["No runtime type, tick, render-mode or table-value conversion inferred from column labels.","cl_alr.ini has no column header; only positional id/name used, remaining values retained raw.","Only obdef and objpa target chains; lolig/expde/objpv/objpp remain raw, their other tables are outside scope.","Installed base .dau only; overlays/clengine DefaultPath not applied.","Negative -1 references explicitly reported, never frame0 fallback.","ALR versions 4..6, 8-bit only; trailing metadata hashed, not interpreted.","No game launch, installed writes, ISO access, REA calls, host changes or pixel equivalence."]}),
    )
}
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: particle-assets <installed root> <NEW re_workspace/*.json>".into());
    }
    let root = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let parent = output
        .parent()
        .ok_or("output parent missing")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../re_workspace")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !parent.starts_with(workspace) || output.extension().is_none_or(|s| s != "json") {
        return Err("output must be a NEW JSON in re_workspace".into());
    }
    if output.exists() {
        return Err("output already exists; refusing overwrite".into());
    }
    let mut report = audit(&root)?;
    let source_issues: Vec<_> = report["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            s["status"] != "read"
                || s.get("parse_error").is_some()
                || s.get("decode_error").is_some()
        })
        .cloned()
        .collect();
    let table_issues: usize = report["tables"]
        .as_object()
        .unwrap()
        .values()
        .map(|t| t["issues"].as_array().map_or(0, Vec::len))
        .sum();
    let relations: Vec<_> = report["gfx_object_alr_chains"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["relations"].as_array().into_iter().flatten())
        .collect();
    let unresolved_relations = relations
        .iter()
        .filter(|r| r["status"] == "unresolved")
        .count();
    let unresolved_layers = relations
        .iter()
        .flat_map(|r| r["alr_layers"].as_array().into_iter().flatten())
        .filter(|l| l["status"] == "unresolved")
        .count();
    let unresolved_links = report["particle_referencing_objdefs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["links"].as_array().into_iter().flatten())
        .filter(|l| l["status"] != "resolved")
        .count();
    report["inventory_complete_within_scope"] = json!(
        source_issues.is_empty()
            && report["tables"].as_object().unwrap().len() == 5
            && table_issues == 0
            && unresolved_relations == 0
            && unresolved_layers == 0
            && unresolved_links == 0
            && report["totals"]["unresolved_alr_members"] == 0
    );
    report["inventory_issues"] = json!(source_issues);
    report["totals"]["table_issues"] = json!(table_issues);
    report["totals"]["unresolved_gfx_object_relations"] = json!(unresolved_relations);
    report["totals"]["unresolved_alr_layers"] = json!(unresolved_layers);
    report["totals"]["unresolved_emitter_links"] = json!(unresolved_links);
    let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|e| e.to_string())?;
    out.write_all(&bytes).map_err(|e| e.to_string())?;
    out.sync_all().map_err(|e| e.to_string())?;
    println!(
        "{}",
        json!({"output":output,"source_unchanged":report["source_unchanged"],"totals":report["totals"]})
    );
    if report["source_unchanged"] != true {
        return Err("source changed during inventory".into());
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_pfil_boundaries() {
        for n in 4..64 {
            let mut b = vec![0; n];
            b[..4].copy_from_slice(b"PFIL");
            assert!(decoded(&b).is_err());
        }
        let mut b = vec![0; 64];
        b[..4].copy_from_slice(b"PFIL");
        b[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(decoded(&b).is_err());
        b.extend([1, b'A']);
        assert_eq!(decoded(&b).unwrap(), b"A");
        b.push(0);
        assert!(decoded(&b).is_err());
    }
}
