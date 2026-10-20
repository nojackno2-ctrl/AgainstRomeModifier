//! 與真 .NET 8 oracle（tests/csharp_oracle）對拍：sanitize_name、OrdinalIgnoreCase 事件名碰撞與別名比對。
//! 夾具由 oracle 呼叫真正的 `CampaignMissionCompiler.SanitizeName`（反射）與 BCL 比較器產生。
//! 夾具已凍結於 tests/fixtures（C# oracle 專案已移除，不再需要 dotnet）。

use against_rome_campaign::ai::compiler::sanitize_name;
use against_rome_campaign::ai::dotnet_unicode::{
    is_letter_or_digit_unit, ordinal_ignore_case_eq, ordinal_ignore_case_key,
};
use std::collections::HashSet;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// "0041.D83D" → String；"-" → 空字串。夾具只含合法 scalar（oracle 輸入不含孤立 surrogate）。
fn dec(s: &str) -> String {
    if s == "-" {
        return String::new();
    }
    let units: Vec<u16> = s.split('.').map(|h| u16::from_str_radix(h, 16).unwrap()).collect();
    String::from_utf16(&units).unwrap_or_else(|_| panic!("lone surrogate in fixture: {s}"))
}

fn dec_list(s: &str) -> Vec<String> {
    if s == "-" || s.is_empty() {
        return Vec::new();
    }
    s.split(',').map(dec).collect()
}

#[test]
fn sanitize_name_matches_real_csharp_oracle() {
    let data = fixture("dotnet_sanitize_cases.tsv");
    let mut n = 0;
    for line in data.lines() {
        let (i, o) = line.split_once('\t').unwrap();
        let (input, expected) = (dec(i), dec(o));
        let got = sanitize_name(&input);
        assert_eq!(
            got.encode_utf16().collect::<Vec<_>>(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "input units {i}"
        );
        n += 1;
    }
    assert!(n > 3000, "fixture unexpectedly small: {n}");
}

#[test]
fn letter_or_digit_table_matches_documented_net_semantics() {
    // 抽查已知邊界：Nl/No 不是 IsLetterOrDigit；Nd 與 L* 是；surrogate 恆否。
    for (u, want) in [
        (0x0030u16, true),
        (0x0661, true),  // Nd
        (0x2160, false), // Nl 羅馬數字
        (0x00B2, false), // No 上標 2
        (0x00BD, false), // No ½
        (0x0301, false), // Mn 組合附加符號
        (0x4E00, true),
        (0xD83D, false),
        (0xDE00, false),
        (0x005F, false),
    ] {
        assert_eq!(is_letter_or_digit_unit(u), want, "U+{u:04X}");
    }
}

#[test]
fn supplementary_letters_become_two_underscores_like_csharp() {
    // C# 逐 UTF-16 單元判斷：surrogate 非字母 → 補充平面字母成為 "__"。
    assert_eq!(sanitize_name("a𠀀b"), "a__b");
    assert_eq!(sanitize_name("𠀀"), "");
    assert_eq!(sanitize_name(&"a".repeat(29).chars().chain("😀".chars()).collect::<String>()), "a".repeat(29));
}

#[test]
fn event_name_collisions_match_ordinal_ignore_case() {
    let data = fixture("dotnet_event_name_collision_cases.tsv");
    let mut n = 0;
    for line in data.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 3, "{line}");
        let existing = dec_list(cols[0]);
        let incoming = dec_list(cols[1]);
        let expected = dec_list(cols[2]);
        // 與 pipeline.rs 第 5 步相同的演算法。
        let mut used: HashSet<String> = existing.iter().map(|e| ordinal_ignore_case_key(e)).collect();
        let mut got = Vec::new();
        for original in &incoming {
            let mut name = original.clone();
            let mut counter = 2;
            while !used.insert(ordinal_ignore_case_key(&name)) {
                name = format!("{original}_{counter}");
                counter += 1;
            }
            got.push(name);
        }
        assert_eq!(got, expected, "existing={existing:?} incoming={incoming:?}");
        n += 1;
    }
    assert!(n >= 10);
}

#[test]
fn alias_membership_matches_ordinal_ignore_case() {
    let data = fixture("dotnet_alias_cases.tsv");
    let mut lines = data.lines();
    let known = dec_list(lines.next().unwrap().strip_prefix("KNOWN\t").unwrap());
    for line in lines {
        let (p, r) = line.split_once('\t').unwrap();
        let probe = dec(p);
        let want = r == "1";
        let got = known.iter().any(|k| ordinal_ignore_case_eq(k, &probe));
        assert_eq!(got, want, "probe {probe:?}");
    }
}
