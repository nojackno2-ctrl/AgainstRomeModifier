use formation_probe::pfil::{
    create_synthetic_pfil, decompress_pfil, format_dump, parse_dump_range, python_bytes_repr,
    sha256_hex,
};

#[test]
fn test_uncompressed_pfil_returns_input() {
    let raw = b"This is not a PFIL file, just raw text.";
    let result = decompress_pfil(raw).unwrap();
    assert_eq!(result, raw);
}

#[test]
fn test_synthetic_pfil_roundtrip() {
    let payload = b"Hello, Against Rome formation research! Testing 1 2 3.";
    let pfil = create_synthetic_pfil(payload);
    assert_eq!(&pfil[0..4], b"PFIL");
    let decoded = decompress_pfil(&pfil).unwrap();
    assert_eq!(decoded, payload);
}

#[test]
fn test_python_bytes_repr() {
    assert_eq!(python_bytes_repr(b"abc"), "b'abc'");
    assert_eq!(python_bytes_repr(b"\""), "b'\"'");
    assert_eq!(python_bytes_repr(b"'"), "b\"'\"");
    assert_eq!(python_bytes_repr(b"\"'"), "b'\"\\''");
    assert_eq!(
        python_bytes_repr(b"hello\t\r\n\x00\xff"),
        "b'hello\\t\\r\\n\\x00\\xff'"
    );
}

#[test]
fn test_format_dump() {
    let payload = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let pfil = create_synthetic_pfil(payload);
    let dump = format_dump(&pfil, 0, 32).unwrap();
    let lines: Vec<&str> = dump.lines().collect();

    assert!(lines[0].starts_with("SHA256 "));
    assert_eq!(lines[1], format!("payload length {}", payload.len()));
    // Line 2: offset 000000
    assert!(lines[2].starts_with("000000 "));
    assert!(lines[2].contains("30 31 32 33 34 35 36 37 38 39 61 62 63 64 65 66"));
    assert!(lines[2].ends_with("b'0123456789abcdef'"));
    // Line 3: offset 000010
    assert!(lines[3].starts_with("000010 "));
}

#[test]
fn test_parse_dump_range() {
    assert_eq!(parse_dump_range("0:256").unwrap(), (0, 256));
    assert_eq!(parse_dump_range("0x0:0x100").unwrap(), (0, 256));
    assert_eq!(parse_dump_range("16:64").unwrap(), (16, 64));
    assert!(parse_dump_range("invalid").is_err());
    assert!(parse_dump_range("100:10").is_err());
}

#[test]
fn test_sha256_hex() {
    assert_eq!(
        sha256_hex(b"hello world"),
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}
