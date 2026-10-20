use super::error::ShadowError;

pub const DIMENSION: usize = 256;
pub const SLICE_COUNT: usize = 128;
/// 32-byte header + 128 slices of 256x256 bits.
pub const DECODED_LENGTH: usize = 32 + SLICE_COUNT * DIMENSION * DIMENSION / 8;

const SLICE_BYTES: usize = DIMENSION * DIMENSION / 8;
const ROW_BYTES: usize = DIMENSION / 8;

/// Decoded `shadows.dat`: 128 time slices of 256x256 LSB-first terrain occlusion bitplanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightingShadowMap {
    header: [u32; 8],
    bits: Vec<u8>,
}

impl LightingShadowMap {
    /// Parses already-decompressed bytes. Deliberately strict: any extent mismatch is rejected.
    pub fn parse_decoded(source: &[u8]) -> Result<Self, ShadowError> {
        if source.len() != DECODED_LENGTH {
            return Err(ShadowError::invalid("Invalid decoded shadows.dat extent."));
        }
        let mut header = [0u32; 8];
        for (i, slot) in header.iter_mut().enumerate() {
            *slot = u32::from_le_bytes([
                source[i * 4],
                source[i * 4 + 1],
                source[i * 4 + 2],
                source[i * 4 + 3],
            ]);
        }
        if header[0] != DIMENSION as u32
            || header[1] != DIMENSION as u32
            || header[2] != SLICE_COUNT as u32
        {
            return Err(ShadowError::unsupported(
                "Only 256x256, 128-slice shadow meshes are supported.",
            ));
        }
        // The last five fields are cache settings / height sum; preserved verbatim.
        Ok(Self { header, bits: source[32..].to_vec() })
    }

    pub fn header(&self) -> &[u32; 8] {
        &self.header
    }

    pub fn is_occluded(&self, slice: i32, x: i32, z: i32) -> Result<bool, ShadowError> {
        if !(0..SLICE_COUNT as i32).contains(&slice) {
            return Err(ShadowError::OutOfRange("slice"));
        }
        check_cell(x, z)?;
        Ok(self.bit(slice as usize, x as usize, z as usize))
    }

    fn bit(&self, slice: usize, x: usize, z: usize) -> bool {
        self.bits[slice * SLICE_BYTES + z * ROW_BYTES + (x >> 3)] & (1 << (x & 7)) != 0
    }

    /// 0x49A780 integer time interpolation: 86400 s/day, 675 s per slice. Returns 0..=255.
    pub fn occlusion_at(&self, seconds_of_day: i32, x: i32, z: i32) -> Result<i32, ShadowError> {
        check_seconds(seconds_of_day)?;
        check_cell(x, z)?;
        let (x, z) = (x as usize, z as usize);
        let slice = (seconds_of_day * SLICE_COUNT as i32 / 86400) as usize;
        let weight = (seconds_of_day % 675) * 256 / 675;
        let a = if self.bit(slice, x, z) { 255 } else { 0 };
        let b = if self.bit((slice + 1) % SLICE_COUNT, x, z) { 255 } else { 0 };
        Ok((a * (256 - weight) + b * weight) >> 8)
    }

    /// 0x49A620 fixed-point spatial sampling; world X/Z are 64 units per cell.
    ///
    /// Native 32-bit shifts overflow, so only world coordinates in `-32768..=32767` are accepted.
    pub fn sample_occlusion(
        &self,
        seconds_of_day: i32,
        world_x: i32,
        world_z: i32,
    ) -> Result<i32, ShadowError> {
        if !(-32768..=32767).contains(&world_x) {
            return Err(ShadowError::OutOfRange("world_x"));
        }
        if !(-32768..=32767).contains(&world_z) {
            return Err(ShadowError::OutOfRange("world_z"));
        }
        check_seconds(seconds_of_day)?;
        let max = DIMENSION as i32 - 1;
        let fx = world_x << 10;
        let fz = world_z << 10;
        let x0 = (fx >> 16).clamp(0, max);
        let x1 = (x0 + 1).min(max);
        let z0 = (fz >> 16).clamp(0, max);
        let z1 = (z0 + 1).min(max);
        let wx = fx & 65535;
        let wz = fz & 65535;
        let at = |x, z| self.occlusion_at(seconds_of_day, x, z);
        // Max intermediate: 32639 * 65535 < i32::MAX, so no overflow like the native path.
        let left = (at(x0, z0)? * (65535 - wz) + at(x0, z1)? * wz) >> 9;
        let right = (at(x1, z0)? * (65535 - wz) + at(x1, z1)? * wz) >> 9;
        Ok((left * (65535 - wx) + right * wx) >> 23)
    }

    /// Ground brightness multiplier: `1 - occlusion/256 * strength`, `strength` in `[0, 1]`.
    pub fn shadow_factor(
        &self,
        seconds_of_day: i32,
        world_x: i32,
        world_z: i32,
        strength: f32,
    ) -> Result<f32, ShadowError> {
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return Err(ShadowError::OutOfRange("strength"));
        }
        let occlusion = self.sample_occlusion(seconds_of_day, world_x, world_z)?;
        Ok(1.0 - occlusion as f32 * (1.0 / 256.0) * strength)
    }
}

fn check_seconds(seconds_of_day: i32) -> Result<(), ShadowError> {
    if (0..86400).contains(&seconds_of_day) {
        Ok(())
    } else {
        Err(ShadowError::OutOfRange("seconds_of_day"))
    }
}

fn check_cell(x: i32, z: i32) -> Result<(), ShadowError> {
    if !(0..DIMENSION as i32).contains(&x) {
        return Err(ShadowError::OutOfRange("x"));
    }
    if !(0..DIMENSION as i32).contains(&z) {
        return Err(ShadowError::OutOfRange("z"));
    }
    Ok(())
}
