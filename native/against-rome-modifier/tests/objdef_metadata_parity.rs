//! ObjdefPatcher.cs reads the default-comparer (case-sensitive) UnitMeta dictionary.
use against_rome_modifier::{parse_csv_line, ObjdefIndex, ObjdefOptions, ObjdefPatcher};

const UNIT: &str = "FigRomSch01_Bogen";
const CIVILIANS: [&str; 3] = [
    "FigZivMan00_Zivilist",
    "FigZivWei00_Zivilistin",
    "FigTiePac00_Packpferd",
];

fn patch(name: &str, options: &ObjdefOptions) -> Vec<String> {
    let mut cols = vec![format!("{:>8}", "0"); 256];
    cols[52] = format!("{name:>40}");
    for (index, value) in [
        (ObjdefIndex::Hp, "100"),
        (ObjdefIndex::Sirad, "200"),
        (ObjdefIndex::Lpsub, "1.00"),
        (ObjdefIndex::Moves, "2.00"),
        (ObjdefIndex::Movsf, "3.00"),
        (ObjdefIndex::Bmovs, "4.00"),
    ] {
        cols[index as usize] = format!("{value:>8}");
    }
    let raw = format!("header1\r\nheader2\r\n{}\r\n", cols.join(","));
    let bytes = ObjdefPatcher::get_patched_bytes(raw.as_bytes(), options).unwrap();
    assert_eq!(bytes.len(), raw.len());
    let text = String::from_utf8(bytes).unwrap();
    parse_csv_line(text.lines().nth(2).unwrap())
}

#[test]
fn unit_stats_require_exact_metadata_even_when_stats_match_row_case() {
    for name in [
        UNIT.to_string(),
        UNIT.to_ascii_lowercase(),
        UNIT.to_ascii_uppercase(),
        "UnknownUnit".into(),
    ] {
        let options = ObjdefOptions {
            unit_stats: [(name.clone(), [321., 0., 0., 0., 0., 200., 0., 0., 0.])].into(),
            unit_movement_speed_2x: true,
            ..Default::default()
        };
        let cols = patch(&name, &options);
        let exact = name == UNIT;
        assert_eq!(
            cols[ObjdefIndex::Hp as usize].trim(),
            if exact { "321" } else { "100" },
            "{name}"
        );
        assert_eq!(
            cols[ObjdefIndex::Moves as usize].trim(),
            if exact { "4.00" } else { "2.00" },
            "{name}"
        );
    }
}

#[test]
fn entire_map_vision_requires_exact_metadata_or_civilian_name() {
    let options = ObjdefOptions {
        all_units_entire_map_vision: true,
        ..Default::default()
    };
    for name in std::iter::once(UNIT)
        .chain(CIVILIANS)
        .chain(["UnknownUnit"])
    {
        for candidate in [
            name.to_string(),
            name.to_ascii_lowercase(),
            name.to_ascii_uppercase(),
        ] {
            let expected = candidate == name && name != "UnknownUnit";
            assert_eq!(
                ObjdefPatcher::supports_entire_map_vision(&candidate),
                expected,
                "{candidate}"
            );
            let cols = patch(&candidate, &options);
            assert_eq!(
                cols[ObjdefIndex::Sirad as usize].trim(),
                if expected { "30000" } else { "200" },
                "{candidate}"
            );
        }
    }
}

#[test]
fn no_run_hp_loss_requires_exact_metadata_or_civilian_name_without_stats() {
    let options = ObjdefOptions {
        no_run_hp_loss: true,
        ..Default::default()
    };
    for name in std::iter::once(UNIT)
        .chain(CIVILIANS)
        .chain(["UnknownUnit"])
    {
        for candidate in [
            name.to_string(),
            name.to_ascii_lowercase(),
            name.to_ascii_uppercase(),
        ] {
            let cols = patch(&candidate, &options);
            let expected = candidate == name && name != "UnknownUnit";
            assert_eq!(
                cols[ObjdefIndex::Lpsub as usize].trim(),
                if expected { "0.00" } else { "1.00" },
                "{candidate}"
            );
        }
    }
}

#[test]
fn civilian_speed_keeps_exact_name_exceptions() {
    let options = ObjdefOptions {
        villager_movement_speed_5x: true,
        ..Default::default()
    };
    for name in CIVILIANS {
        for candidate in [name.to_string(), name.to_ascii_lowercase()] {
            let cols = patch(&candidate, &options);
            for (index, original, scaled) in [
                (ObjdefIndex::Moves, "2.00", "10.00"),
                (ObjdefIndex::Movsf, "3.00", "15.00"),
                (ObjdefIndex::Bmovs, "4.00", "20.00"),
            ] {
                assert_eq!(
                    cols[index as usize].trim(),
                    if candidate == name { scaled } else { original },
                    "{candidate}"
                );
            }
        }
    }
}
