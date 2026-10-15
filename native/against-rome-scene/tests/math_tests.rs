use against_rome_scene::{Mat4, Vec2, Vec3};

#[test]
fn vec2_operations() {
    let a = Vec2::new(3.0, 4.0);
    let b = Vec2::new(1.0, 2.0);

    assert_eq!(Vec2::new(4.0, 6.0), a + b);
    assert_eq!(Vec2::new(2.0, 2.0), a - b);
    assert_eq!(Vec2::new(6.0, 8.0), a * 2.0);

    assert_eq!(25.0, a.length_squared());
    assert_eq!(5.0, a.length());
    assert_eq!(11.0, a.dot(b)); // 3*1 + 4*2 = 11

    let c = Vec2::new(6.0, 8.0);
    assert_eq!(5.0, a.distance(c));
}

#[test]
fn vec3_operations() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);

    assert_eq!(Vec3::new(5.0, 7.0, 9.0), a + b);
    assert_eq!(Vec3::new(-3.0, -3.0, -3.0), a - b);
    assert_eq!(Vec3::new(2.0, 4.0, 6.0), a * 2.0);

    // Dot product: 1*4 + 2*5 + 3*6 = 32
    assert_eq!(32.0, a.dot(b));

    // Cross product: (2*6 - 3*5, 3*4 - 1*6, 1*5 - 2*4) = (-3, 6, -3)
    assert_eq!(Vec3::new(-3.0, 6.0, -3.0), a.cross(b));

    let norm = Vec3::new(0.0, 3.0, 4.0).normalized();
    assert!((norm.x - 0.0).abs() < 1e-5);
    assert!((norm.y - 0.6).abs() < 1e-5);
    assert!((norm.z - 0.8).abs() < 1e-5);
    assert!((norm.length() - 1.0).abs() < 1e-5);

    // Zero vector normalized returns ZERO
    assert_eq!(Vec3::ZERO, Vec3::ZERO.normalized());
}

#[test]
fn mat4_identity_mul_transform() {
    let id = Mat4::IDENTITY;
    let v = Vec3::new(2.0, 3.0, 4.0);
    assert_eq!(v, id.transform_point(v));

    // Identity * Identity == Identity
    assert_eq!(id, id * id);
}

#[test]
fn mat4_look_at_and_inverse() {
    let eye = Vec3::new(10.0, 20.0, 30.0);
    let target = Vec3::new(0.0, 0.0, 0.0);
    let view = Mat4::look_at(eye, target);

    let inv = view.inverse().expect("look_at view matrix must be invertible");
    let product = view * inv;

    // Product should be approximately identity
    for (i, row) in product.0.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!(
                (val - expected).abs() < 1e-4,
                "product[{i}][{j}] = {val}, expected {expected}"
            );
        }
    }
}

#[test]
fn mat4_orthographic_and_inverse() {
    let ortho = Mat4::orthographic(800.0, 600.0, 1.0, 1000.0);
    let inv = ortho.inverse().expect("orthographic matrix must be invertible");
    let product = ortho * inv;

    for (i, row) in product.0.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!(
                (val - expected).abs() < 1e-4,
                "product[{i}][{j}] = {val}, expected {expected}"
            );
        }
    }
}

#[test]
fn mat4_singular_matrix_inversion_fails_gracefully() {
    let zero = Mat4([[0.0; 4]; 4]);
    assert!(zero.inverse().is_none());
}
