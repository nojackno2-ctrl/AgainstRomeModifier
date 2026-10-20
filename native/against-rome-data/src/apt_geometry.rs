//! Static default attachment projection and building world placement geometry.
//!
//! Follows native routines `0x4E5420` (projection), `0x419C80` + `0x49C2A0` (world grid),
//! and `0x4C82A0` (building light placement).

use crate::apt::{AptDocument, AptError, AptPoint};

/// 3D vector with float components used for world placements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AptVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AptVector3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Extracted light point with screen coordinates and delta relative to the anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct AptLightPoint {
    pub screen_x: i32,
    pub screen_y: i32,
    pub delta_anchor_x: i32,
    pub delta_anchor_y: i32,
}

impl AptLightPoint {
    pub const fn new(screen_x: i32, screen_y: i32, delta_anchor_x: i32, delta_anchor_y: i32) -> Self {
        Self {
            screen_x,
            screen_y,
            delta_anchor_x,
            delta_anchor_y,
        }
    }
}

/// Projects a 2D integer point relative to the document's projection origin.
///
/// Follows native `0x4E5420`: subtract the projection origin, then apply integer floor
/// division on each absolute point via arithmetic shift right 10.
#[inline]
pub fn project(document: &AptDocument, point: AptPoint) -> AptPoint {
    let x = point.x.wrapping_sub(document.projection_origin.x);
    let y = point.y.wrapping_sub(document.projection_origin.y);
    let px = ((y << 5).wrapping_add(x << 4)) >> 10;
    let py = ((y << 5).wrapping_sub(x << 4)) >> 10;
    AptPoint::new(px, py)
}

/// Rounds a double-precision floating-point number to the nearest integer,
/// with ties rounding to the nearest even number (IEEE 754 roundTiesToEven).
#[inline]
pub fn round_half_to_even(x: f64) -> f64 {
    let floor = x.floor();
    let diff = x - floor;
    if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else {
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    }
}

/// Biased FISTP under default nearest-even x87 mode, followed by arithmetic shift right 6 (SAR 6).
///
/// Follows native `0x419C80` + `0x49C2A0`.
pub fn world_grid(value: f32) -> Result<i32, AptError> {
    if !value.is_finite() {
        return Err(AptError::ParentCoordinateNotFinite);
    }
    let biased = (value as f64) - 0.49999991059303284_f64.copysign(value as f64);
    let rounded = round_half_to_even(biased);
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(AptError::ParentCoordinateOutOfBounds);
    }
    Ok((rounded as i32) >> 6)
}

/// Returns the world grid origin coordinate: `(WorldGrid(value) << 6) as f32`.
pub fn world_grid_origin(value: f32) -> Result<f32, AptError> {
    let grid = world_grid(value)?;
    Ok((grid.wrapping_shl(6)) as f32)
}

/// Computes the 3D world placement of an attachment point relative to the parent position and anchor.
pub fn place(
    document: &AptDocument,
    point: AptPoint,
    parent_x: f32,
    parent_z: f32,
    height: f32,
) -> Result<AptVector3, AptError> {
    let anchor = project(document, AptPoint::new(document.anchor_x, document.anchor_y));
    let projected = project(document, point);
    let wg_x = world_grid(parent_x)?;
    let wg_z = world_grid(parent_z)?;

    let x = ((wg_x.wrapping_sub(anchor.x).wrapping_add(projected.x)) << 6).wrapping_add(32);
    let z = ((wg_z.wrapping_sub(anchor.y).wrapping_add(projected.y)) << 6).wrapping_add(32);

    Ok(AptVector3::new(x as f32, height, z as f32))
}

/// Extracts light points for the completed static APT state (same as editor sprite).
pub fn extract_light_points(apt_bytes: &[u8]) -> Result<Vec<AptLightPoint>, AptError> {
    let document = AptDocument::parse(apt_bytes)?;
    let axis0 = if document.layout[0] == 0 {
        0
    } else {
        document.layout[0] - 1
    };
    let attachments = document.get_attachments(axis0, 0, 0)?;
    let result = attachments
        .lights
        .iter()
        .map(|point| {
            AptLightPoint::new(
                point.x,
                point.y,
                point.x.wrapping_sub(document.anchor_x),
                point.y.wrapping_sub(document.anchor_y),
            )
        })
        .collect();
    Ok(result)
}

/// Computes 3D world positions of building lights (up to native capacity of 20).
pub fn get_building_world_light_positions(
    apt_bytes: &[u8],
    building_world_x: f32,
    building_world_z: f32,
    ground_y: f32,
    apt_height_offset: f32,
) -> Result<Vec<AptVector3>, AptError> {
    let document = AptDocument::parse(apt_bytes)?;
    let axis0 = if document.layout[0] == 0 {
        0
    } else {
        document.layout[0] - 1
    };
    let attachments = document.get_attachments(axis0, 0, 0)?;
    let height = ground_y + apt_height_offset;

    let mut out = Vec::with_capacity(attachments.lights.len().min(20));
    for point in attachments.lights.iter().take(20) {
        let pos = place(&document, *point, building_world_x, building_world_z, height)?;
        out.push(pos);
    }
    Ok(out)
}
