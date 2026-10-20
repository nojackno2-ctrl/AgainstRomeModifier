//! `probe_inspect_formatio_lager.py`: active-record samples of formatio / lager / biglager.
//! Read-only; prints JSON (the Python script printed `dict` reprs).

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::util::*;
use std::path::{Path, PathBuf};

fn decode_file(path: &Path) -> R<Vec<u8>> {
    Ok(decode_unbounded(&read(path)?)?.1)
}

pub fn inspect_formatio(decoded: &[u8]) -> R<J> {
    let (version, count) = (u32_at(decoded, 0)?, u32_at(decoded, 4)? as usize);
    let stride = 25;
    let mut active = 0u64;
    let mut samples = J::arr();
    for i in 0..count {
        let rec = decoded.get(8 + i * stride..8 + (i + 1) * stride).ok_or("formatio record outside decoded data")?;
        if rec[0] != 0 {
            active += 1;
            if samples.len() < 5 {
                samples.push(jobj! {
                    "slot" => i,
                    "act" => rec[0],
                    "u16_2" => u16_at(rec, 2)?,
                    "u32_4" => u32_at(rec, 4)?,
                    "u32_8" => u32_at(rec, 8)?,
                    "u32_12" => u32_at(rec, 12)?,
                    "tail_hex" => hex(&rec[16..]),
                });
            }
        }
    }
    Ok(jobj! { "version" => version, "count" => count, "active" => active, "samples" => samples })
}

pub fn inspect_lager(decoded: &[u8]) -> R<J> {
    let (version, count, e1, e2) = (u32_at(decoded, 0)?, u32_at(decoded, 4)? as usize, u32_at(decoded, 8)?, u32_at(decoded, 12)?);
    let stride = 45;
    let mut active = 0u64;
    let mut samples = J::arr();
    for i in 0..count {
        let rec = decoded.get(16 + i * stride..16 + (i + 1) * stride).ok_or("lager record outside decoded data")?;
        if rec[0] != 0 {
            active += 1;
            if samples.len() < 5 {
                samples.push(jobj! { "slot" => i, "act" => rec[0], "hex" => hex(rec) });
            }
        }
    }
    Ok(jobj! { "version" => version, "count" => count, "extra" => vec![e1, e2], "active" => active, "samples" => samples })
}

pub fn inspect_biglager(decoded: &[u8]) -> R<J> {
    Ok(jobj! {
        "version" => u32_at(decoded, 0)?,
        "count" => u32_at(decoded, 4)?,
        "subcount" => u32_at(decoded, 8)?,
        "extra" => u32_at(decoded, 12)?,
        "total_len" => decoded.len(),
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    if p.pos.len() > 1 {
        return Err(usage("probe-inspect-formatio-lager [GAME_ROOT]"));
    }
    let maps: PathBuf = p.pos.first().map(PathBuf::from).unwrap_or_else(installed_game).join("MAPS");
    for (map, with_big) in [("ENDL_000", true), ("MP_001", false), ("C1_M01", false)] {
        println!("=== {map} ===");
        let data = maps.join(map).join("DATA");
        println!("formatio: {}", inspect_formatio(&decode_file(&data.join("formatio.dat"))?)?.line(true));
        println!("lager: {}", inspect_lager(&decode_file(&data.join("lager.dat"))?)?.line(true));
        if with_big {
            println!("biglager: {}", inspect_biglager(&decode_file(&data.join("biglager.dat"))?)?.line(true));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn samples_active_records_only() {
        let mut formatio = u32s(&[1, 3]);
        formatio.extend(vec![0u8; 75]);
        formatio[8 + 25] = 7;
        formatio[8 + 25 + 4] = 9;
        let report = inspect_formatio(&formatio).unwrap();
        assert_eq!(report.get("active"), Some(&J::Int(1)));
        match report.get("samples").unwrap() {
            J::Arr(a) => {
                assert_eq!(a[0].get("slot"), Some(&J::Int(1)));
                assert_eq!(a[0].get("u32_4"), Some(&J::Int(9)));
            }
            _ => panic!(),
        }
        let mut lager = u32s(&[1, 2, 6, 10]);
        lager.extend(vec![0u8; 90]);
        lager[16] = 1;
        assert_eq!(inspect_lager(&lager).unwrap().get("active"), Some(&J::Int(1)));
        assert!(inspect_lager(&lager[..40]).is_err());
        let big = inspect_biglager(&u32s(&[1, 32, 800, 5])).unwrap();
        assert_eq!(big.get("subcount"), Some(&J::Int(800)));
    }
}
