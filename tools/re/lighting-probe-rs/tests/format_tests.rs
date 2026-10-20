use lighting_probe::{format_scalar, format_vector3, NEWLINE};

#[test]
fn test_float_and_vector3_formatting() {
    assert_eq!(format_scalar(50.0), "50");
    assert_eq!(format_scalar(0.0), "0");
    assert_eq!(format_scalar(-0.0), "-0");
    assert_eq!(format_scalar(0.4609375), "0.4609375");
    assert_eq!(format_scalar(0.65625), "0.65625");
    assert_eq!(format_scalar(0.8515625), "0.8515625");

    assert_eq!(format_vector3([30.0, 20.0, 10.0]), "<30, 20, 10>");
    assert_eq!(format_vector3([50.0, 60.0, 70.0]), "<50, 60, 70>");
    assert_eq!(
        format_vector3([0.4609375, 0.65625, 0.8515625]),
        "<0.4609375, 0.65625, 0.8515625>"
    );
    assert_eq!(format_vector3([-0.0, 0.0, 1.0]), "<-0, 0, 1>");
}

#[test]
fn test_platform_newline_constant() {
    #[cfg(windows)]
    assert_eq!(NEWLINE, "\r\n");

    #[cfg(not(windows))]
    assert_eq!(NEWLINE, "\n");
}
