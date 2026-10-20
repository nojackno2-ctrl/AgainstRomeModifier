use lighting_probe::base64::decode_base64;

#[test]
fn base64_decoding_contract() {
    // Empty
    assert_eq!(decode_base64("").unwrap(), Vec::<u8>::new());

    // Basic standard cases
    assert_eq!(decode_base64("Zg==").unwrap(), b"f");
    assert_eq!(decode_base64("Zm8=").unwrap(), b"fo");
    assert_eq!(decode_base64("Zm9v").unwrap(), b"foo");
    assert_eq!(decode_base64("Zm9vYg==").unwrap(), b"foob");
    assert_eq!(decode_base64("Zm9vYmE=").unwrap(), b"fooba");
    assert_eq!(decode_base64("Zm9vYmFy").unwrap(), b"foobar");

    // Whitespace handling (newlines, carriage return, space, tab)
    assert_eq!(decode_base64(" \t\r\nZm9v\r\n").unwrap(), b"foo");
    assert_eq!(decode_base64("Z m 9 v").unwrap(), b"foo");

    // Errors
    assert!(decode_base64("Zm9").is_err()); // length not multiple of 4
    assert!(decode_base64("Zm9*").is_err()); // invalid character
    assert!(decode_base64("=m9v").is_err()); // invalid padding position
    assert!(decode_base64("Zm=v").is_err()); // invalid padding sequence

    // Data after padding must be rejected (matching C# Convert.FromBase64String)
    assert!(decode_base64("Zg==Zm9v").is_err());
    assert!(decode_base64("Zm8=Zm9v").is_err());
    assert!(decode_base64("Zg==AAAA").is_err());
    assert!(decode_base64("Zm8=A===").is_err());
    assert!(decode_base64("Zg========").is_err());
}
