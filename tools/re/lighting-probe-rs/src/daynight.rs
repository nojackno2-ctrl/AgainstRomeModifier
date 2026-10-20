use crate::bitmap::MapLightingBitmap;
use crate::error::ProbeError;

/// Raw daynight.bmp color table and proven formula calculations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapLightingDayNight {
    bitmap: MapLightingBitmap,
}

impl MapLightingDayNight {
    pub fn parse(bytes: &[u8]) -> Result<Self, ProbeError> {
        let bitmap = MapLightingBitmap::parse(bytes)?;
        if bitmap.width() != 24 || bitmap.height() != 6 {
            return Err(ProbeError::NotSupported(
                "Expected a 24x6 daynight table.".into(),
            ));
        }
        Ok(Self { bitmap })
    }

    #[inline]
    pub fn raw_color(&self, hour: i32, row: i32) -> Result<[f32; 3], ProbeError> {
        self.bitmap.color_at(hour, row)
    }

    /// 0x49A350: 16.16 weights floored individually, color channels >> 16 then divided by 256; reads row 0 only.
    pub fn ambient_at(&self, hour: i32, minute: i32) -> Result<[f32; 3], ProbeError> {
        if hour < 0 || hour >= 24 {
            return Err(ProbeError::OutOfRange("hour".into()));
        }
        if minute < 0 || minute >= 60 {
            return Err(ProbeError::OutOfRange("minute".into()));
        }

        let next_weight = minute * 65536 / 60;
        let current_weight = (60 - minute) * 65536 / 60;

        let current = self.raw_color(hour, 0)?;
        let next = self.raw_color((hour + 1) % 24, 0)?;

        let r = (((current[0] as i32 * current_weight) + (next[0] as i32 * next_weight)) >> 16)
            as f32
            / 256.0;
        let g = (((current[1] as i32 * current_weight) + (next[1] as i32 * next_weight)) >> 16)
            as f32
            / 256.0;
        let b = (((current[2] as i32 * current_weight) + (next[2] as i32 * next_weight)) >> 16)
            as f32
            / 256.0;

        Ok([r, g, b])
    }

    #[inline]
    pub fn bitmap(&self) -> &MapLightingBitmap {
        &self.bitmap
    }
}
