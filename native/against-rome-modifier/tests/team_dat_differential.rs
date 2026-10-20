//! team.dat regression tests; expected values are derived from the C# semantics in
//! src.Core/Core/Patches/TeamDatPatcher.cs, PatchText.cs and src.Shared/Core/GameLZSS.cs.
use against_rome_modifier::{
    compress_pfil, decompress_pfil, encode_cp1251, is_pfil, TeamDatOptions, TeamDatPatcher,
};

fn header() -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"PFIL");
    h[4] = 7;
    h[40] = 9;
    h
}

fn pfil(text: &str) -> Vec<u8> {
    compress_pfil(&encode_cp1251(text), Some(&header())).unwrap()
}

fn opts(max: bool, roman: bool) -> TeamDatOptions {
    TeamDatOptions {
        max_population: max,
        roman_player: roman,
        ..Default::default()
    }
}

fn patch(text: &str, o: TeamDatOptions) -> String {
    let out = TeamDatPatcher::get_patched_bytes(&pfil(text), &o).unwrap();
    assert!(is_pfil(&out));
    String::from_utf8(decompress_pfil(&out).unwrap()).unwrap()
}

#[test]
fn nonstandard_rows_and_columns() {
    let src = "// c,omment 80\n[teamdata]\n0\n0,GER\n x ,a,b,c,5\n0,GER,b,c,abc,e\n1,A,b,c, 80 ,e\n1,A,b,c,+7,e\n1,A,b,c,-5,e\n1,A,b,c,99999999999,e\n1,A,b,c,0\n 0 ,GER,b,c,12\n00,GER,b,c,12\n,\n[Other]\n0,GER,b,c,80\n[TEAMDATA]\n0,GER,b,c,80\n";
    let both = patch(src, opts(true, true));
    assert_eq!(
        both,
        "// c,omment 80\n[teamdata]\n0\n0,ROM\n x ,a,b,c,1600\n0,ROM,b,c,abc,e\n1,A,b,c,1600,e\n1,A,b,c,1600,e\n1,A,b,c,-5,e\n1,A,b,c,99999999999,e\n1,A,b,c,0\nROM,ROM,b,c,1600\n00,GER,b,c,1600\n,\n[Other]\n0,GER,b,c,80\n[TEAMDATA]\n0,GER,b,c,80\n"
            .replace("\nROM,ROM,b,c,1600", "\n 0 ,ROM,b,c,1600")
    );
    let roman = patch(src, opts(false, true));
    assert!(roman.contains("\n 0 ,ROM,b,c,12\n") && roman.contains("\n00,GER,b,c,12\n"));
    assert!(roman.contains("\n1,A,b,c, 80 ,e\n"));
}

#[test]
fn comment_lines_before_section_and_line_endings() {
    let crlf = patch("[teamdata]\r\n0,GER,b,c,80\r\n", opts(true, true));
    assert_eq!(crlf, "[teamdata]\r\n0,ROM,b,c,1600\r\n");
    let no_trailing = patch("[teamdata]\n0,GER,b,c,80", opts(true, false));
    assert_eq!(no_trailing, "[teamdata]\n0,GER,b,c,1600");
    // Mixed endings: any CRLF selects CRLF as the separator; a bare LF stays inside a line, so
    // column 4 is `80<LF>1` and int.TryParse fails (row untouched).
    let mixed = patch(
        "[teamdata]\r\n1,A,b,c,80\n1,A,b,c,80\r\n",
        opts(true, false),
    );
    assert_eq!(mixed, "[teamdata]\r\n1,A,b,c,80\n1,A,b,c,80\r\n");
}

#[test]
fn custom_population_limit_is_written_verbatim() {
    let mut o = opts(true, false);
    o.population_limit = -3;
    assert_eq!(
        patch("[teamdata]\n1,A,b,c,80\n", o),
        "[teamdata]\n1,A,b,c,-3\n"
    );
    o.population_limit = 0;
    assert_eq!(
        patch("[teamdata]\n1,A,b,c,80\n", o),
        "[teamdata]\n1,A,b,c,0\n"
    );
}

#[test]
fn no_options_returns_input_without_parsing() {
    for bytes in [
        vec![],
        b"PFIL".to_vec(),
        vec![0xFF; 10],
        pfil("[teamdata]\n0,GER,b,c,80\n"),
    ] {
        assert_eq!(
            TeamDatPatcher::get_patched_bytes(&bytes, &opts(false, false)).unwrap(),
            bytes
        );
    }
}

#[test]
fn header_fields_preserved_and_size_updated() {
    let out =
        TeamDatPatcher::get_patched_bytes(&pfil("[teamdata]\n1,A,b,c,80\n"), &opts(true, false))
            .unwrap();
    assert_eq!((out[4], out[40]), (7, 9));
    assert_eq!(
        u32::from_le_bytes(out[16..20].try_into().unwrap()) as usize,
        "[teamdata]\n1,A,b,c,1600\n".len()
    );
}

#[test]
fn headerless_input_follows_csharp_compress_pfil_quirks() {
    // < 64 bytes: C# CompressPfil throws ArgumentException.
    assert!(
        TeamDatPatcher::get_patched_bytes(b"[teamdata]\n1,A,b,c,80\n", &opts(true, false)).is_err()
    );
    // "PFIL" magic but truncated: C# treats as raw text and also throws; Rust must not succeed.
    assert!(TeamDatPatcher::get_patched_bytes(b"PFIL[teamdata]\n", &opts(true, false)).is_err());
    // >= 64 bytes without magic: output header is the first 64 input bytes verbatim.
    let mut raw = b"[teamdata]\n1,A,b,c,80\n".to_vec();
    raw.resize(64, b' ');
    raw.extend_from_slice(b"\n");
    let out = TeamDatPatcher::get_patched_bytes(&raw, &opts(true, false)).unwrap();
    assert_eq!(&out[..16], &raw[..16]);
    assert_eq!(&out[20..64], &raw[20..64]);
    assert!(!is_pfil(&out));
}
