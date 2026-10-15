use against_rome_ai_planner::formation::{
    FormationLayout, FormationLayoutDefinition, FormationLayoutSegment,
};

const NATIVE_GEOMETRY: &str = "0,1,9,.05,-.05,.20,.05,-.20,.15,-.05,.05,.20,-.20,.05,-.15,-.10,-.05,-.20,-.20,.30,-.15,.40,.10,-.25,.25,-.40,.05,.05,.30,.30,.20,.34,-.25,.10,-.34,-.20,-.34,-.40,-.15";

fn parsed_native() -> FormationLayoutDefinition {
    let padding = ",0".repeat(44);
    let text = format!(
        "[FormationDefault]\r\n;idx,activ,keys,...\r\n {}{},0,180,All_Haufen;geometry excerpt\r\n",
        NATIVE_GEOMETRY, padding
    );
    FormationLayoutDefinition::from_form_def_text(&text, 0).unwrap()
}

fn near(expected: (f32, f32), actual: (f32, f32)) {
    assert!(
        (expected.0 - actual.0).abs() <= 0.001,
        "X: expected {}, got {}",
        expected.0,
        actual.0
    );
    assert!(
        (expected.1 - actual.1).abs() <= 0.001,
        "Y: expected {}, got {}",
        expected.1,
        actual.1
    );
}

#[test]
fn test_real_default_structural_excerpt_matches_embedded_geometry() {
    assert_eq!(
        FormationLayoutDefinition::native_default().segments(),
        parsed_native().segments()
    );
}

#[test]
fn test_native_counts_sample_the_correct_segment_and_rotate_rigidly() {
    let definition = parsed_native();
    for count in 1..=20 {
        let zero = FormationLayout::create(count, 0.0, &definition, 320.0).unwrap();
        assert_eq!(count, zero.len());

        for (i, p) in zero.iter().enumerate() {
            let position = (i as f64 + 0.5) * 9.0 / count as f64;
            let j = position.floor() as usize;
            let t = (position - j as f64) as f32;
            let segment = definition.segments()[j];
            let pt_x = segment.start.0 * (1.0 - t) + segment.end.0 * t;
            let pt_y = segment.start.1 * (1.0 - t) + segment.end.1 * t;

            near((-pt_y * 320.0, -pt_x * 320.0), *p);
            assert!(p.0 >= -96.0 && p.0 <= 108.81);
            assert!(p.1 >= -128.0 && p.1 <= 128.0);
        }

        for &angle in &[22.5f32, 45.0, 90.0, 180.0, 270.0, -45.0, 405.0] {
            let rotated = FormationLayout::create(count, angle, &definition, 320.0).unwrap();
            let theta = -(angle as f64) * std::f64::consts::PI / 180.0;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            for i in 0..count {
                let exp_x = (zero[i].0 as f64 * cos_t - zero[i].1 as f64 * sin_t) as f32;
                let exp_y = (zero[i].0 as f64 * sin_t + zero[i].1 as f64 * cos_t) as f32;
                near((exp_x, exp_y), rotated[i]);

                let len_zero = (zero[i].0 * zero[i].0 + zero[i].1 * zero[i].1).sqrt();
                let len_rot = (rotated[i].0 * rotated[i].0 + rotated[i].1 * rotated[i].1).sqrt();
                assert!((len_zero - len_rot).abs() <= 0.001);
            }
        }
    }
}

#[test]
fn test_members_sample_interval_midpoints_and_banner_is_separate() {
    let line = FormationLayoutDefinition::new(vec![FormationLayoutSegment::new(
        (-1.0, 0.0),
        (1.0, 0.0),
    )])
    .unwrap();

    let offsets = FormationLayout::create(4, 0.0, &line, 320.0).unwrap();
    assert_eq!(4, offsets.len());
    for i in 0..4 {
        near((0.0, 240.0 - 160.0 * i as f32), offsets[i]);
    }
    assert_eq!((0.0, 0.0), FormationLayout::banner_offset());
    assert!(FormationLayout::create(0, 0.0, &line, 320.0).unwrap().is_empty());
    near((0.0, 0.0), FormationLayout::create(1, 0.0, &line, 320.0).unwrap()[0]);
}

#[test]
fn test_orientation_reflects_local_x_and_rotates_by_90_minus_angle() {
    let point = FormationLayoutDefinition::new(vec![FormationLayoutSegment::new(
        (0.25, 0.0),
        (0.25, 0.0),
    )])
    .unwrap();

    let cases = [
        (0.0f32, 0.0f32, -80.0f32),
        (90.0, -80.0, 0.0),
        (180.0, 0.0, 80.0),
        (270.0, 80.0, 0.0),
        (-90.0, 80.0, 0.0),
        (450.0, -80.0, 0.0),
    ];

    for (angle, x, z) in cases {
        let res = FormationLayout::create(1, angle, &point, 320.0).unwrap();
        near((x, z), res[0]);
    }
}

#[test]
fn test_two_local_axes_have_native_handedness_and_spacing_is_a_scale() {
    let point = FormationLayoutDefinition::new(vec![FormationLayoutSegment::new(
        (0.25, 0.5),
        (0.25, 0.5),
    )])
    .unwrap();

    near((-160.0, -80.0), FormationLayout::create(1, 0.0, &point, 320.0).unwrap()[0]);
    near((-80.0, 160.0), FormationLayout::create(1, 90.0, &point, 320.0).unwrap()[0]);
    near((-320.0, -160.0), FormationLayout::create(1, 0.0, &point, 640.0).unwrap()[0]);
}

#[test]
fn test_invalid_inputs_are_rejected() {
    let line = FormationLayoutDefinition::new(vec![FormationLayoutSegment::new(
        (-1.0, 0.0),
        (1.0, 0.0),
    )])
    .unwrap();

    assert!(FormationLayout::create(21, 0.0, &line, 320.0).is_err());
    assert!(FormationLayout::create(1, f32::NAN, &line, 320.0).is_err());
    assert!(FormationLayout::create(1, 0.0, &line, 0.0).is_err());
    assert!(FormationLayout::create(1, 0.0, &line, -1.0).is_err());
    assert!(FormationLayout::create(1, 0.0, &line, f32::NAN).is_err());

    assert!(FormationLayoutDefinition::new(vec![]).is_err());
    assert!(FormationLayoutDefinition::new(vec![FormationLayoutSegment::new(
        (f32::NAN, 0.0),
        (0.0, 0.0)
    )])
    .is_err());
}
