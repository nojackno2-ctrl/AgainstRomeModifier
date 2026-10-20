use against_rome_ambience::soundscape::{
    SoundscapeBinaryStorage, SoundscapeError, SoundscapeSnapshot, SoundscapeZone,
};

fn json_with_attribute(key: &str, value: u64) -> String {
    let snapshot = SoundscapeSnapshot {
        zones: vec![SoundscapeZone::default()],
        ..Default::default()
    };
    let mut json = serde_json::to_value(snapshot).unwrap();
    let zone = json["Zones"][0].as_object_mut().unwrap();
    zone.remove("Attributes");
    zone.insert(key.into(), value.into());
    json.to_string()
}

#[test]
fn json_attributes_enforce_csharp_uint32_boundary_without_losing_unknown_bits() {
    // C#: SoundscapeZoneAttributes : uint, including the writable Flags alias.
    // JsonStringEnumConverter accepts numeric values only within that width.
    for key in ["Attributes", "Flags"] {
        for value in [0, 0x8000_0000, u32::MAX as u64] {
            let snapshot =
                SoundscapeBinaryStorage::import_from_json(&json_with_attribute(key, value))
                    .unwrap();
            assert_eq!(snapshot.zones[0].attributes, value as u32);

            let binary = SoundscapeBinaryStorage::serialize_to_binary(&snapshot).unwrap();
            assert_eq!(&binary[148..152], &(value as u32).to_le_bytes());
            let restored = SoundscapeBinaryStorage::deserialize_from_binary(&binary).unwrap();
            assert_eq!(restored.zones[0].attributes, value as u32);

            let exported = SoundscapeBinaryStorage::export_to_json(&restored).unwrap();
            assert_eq!(
                SoundscapeBinaryStorage::import_from_json(&exported)
                    .unwrap()
                    .zones[0]
                    .attributes,
                value as u32
            );
        }
        for value in [u32::MAX as u64 + 1, (1u64 << 32) + 3, u64::MAX] {
            assert!(
                matches!(
                    SoundscapeBinaryStorage::import_from_json(&json_with_attribute(key, value)),
                    Err(SoundscapeError::Deserialization(_))
                ),
                "{key}={value} must fail closed instead of truncating"
            );
        }
    }
}
