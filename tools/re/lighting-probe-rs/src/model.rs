use crate::error::ProbeError;

/// Lighting model formulas reverse-engineered from native game binaries.
pub struct MapLightingModel;

impl MapLightingModel {
    /// 0x49FFE0: per-channel max(ambient, lightColor * (1 - distance²/radius²)), clamped at upper bound 1.0.
    pub fn apply_local_light(
        ambient: [f32; 3],
        light_color: [f32; 3],
        distance_squared: f32,
        radius_squared: f32,
    ) -> Result<[f32; 3], ProbeError> {
        if !distance_squared.is_finite() || distance_squared < 0.0 {
            return Err(ProbeError::OutOfRange("distance_squared".into()));
        }
        if !radius_squared.is_finite() || radius_squared <= 0.0 {
            return Err(ProbeError::OutOfRange("radius_squared".into()));
        }

        if distance_squared >= radius_squared {
            return Ok([
                dotnet_vector_min(ambient[0], 1.0),
                dotnet_vector_min(ambient[1], 1.0),
                dotnet_vector_min(ambient[2], 1.0),
            ]);
        }

        let factor = 1.0 - (distance_squared / radius_squared);
        Ok([
            dotnet_vector_min(dotnet_vector_max(ambient[0], light_color[0] * factor), 1.0),
            dotnet_vector_min(dotnet_vector_max(ambient[1], light_color[1] * factor), 1.0),
            dotnet_vector_min(dotnet_vector_max(ambient[2], light_color[2] * factor), 1.0),
        ])
    }

    /// 0x49A490 end section: caller supplies cloud * terrainShadow and extra byte sample value.
    /// Returns normalized gain; native return value is this value * 256.
    pub fn sprite_gain(
        locally_lit: [f32; 3],
        cloud_and_terrain_shadow: f32,
        visibility_sample: u8,
    ) -> Result<[f32; 3], ProbeError> {
        if !cloud_and_terrain_shadow.is_finite() || !(0.0..=1.0).contains(&cloud_and_terrain_shadow)
        {
            return Err(ProbeError::OutOfRange("cloud_and_terrain_shadow".into()));
        }

        // 0x3B7FFF34: native is slightly lower than 1/256.
        let native_scale = f32::from_bits(0x3B7FFF34);
        let visibility = visibility_sample as f32 * native_scale;
        let factor = (cloud_and_terrain_shadow * 0.707 + 0.293) * visibility * visibility;

        Ok([
            locally_lit[0] * factor,
            locally_lit[1] * factor,
            locally_lit[2] * factor,
        ])
    }
}

#[inline]
fn dotnet_vector_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.max(b)
    }
}

#[inline]
fn dotnet_vector_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.min(b)
    }
}
