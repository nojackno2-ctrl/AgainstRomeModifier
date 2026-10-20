//! Terrain emboss light fitting, preview, and post-height-edit relighting.
//!
//! Replicates C# `TerrainHeightEditSession` and `EmbossLightModel` from Against Rome Map Editor.
//!
//! Reverse-engineering basis:
//! - Against_Rome.exe pipeline (LDFLOOREMBMAP):
//!   - `emboss.bmp` is per-vertex illumination (green channel 0-255), defaults to 255 if missing.
//!   - The illumination is approximated via least-squares plane fitting:
//!     `light ≈ c0 + cx * ∂h/∂x + cy * ∂h/∂y`.
//!   - Light direction varies per map, so coefficients are calibrated from the initial baseline
//!     map data and never hardcoded.
//!   - Editing height only updates vertices whose gradient changed:
//!     `emboss' = clamp(round(emboss + cx * Δgx + cy * Δgy), 0, 255)`.
//!   - Applying blank terrain triggers a flat relight:
//!     `emboss' = clamp(round(flat_base + cx * gx + cy * gy), 0, 255)`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbossError {
    InvalidDimensions,
    InvalidParameters,
}

impl std::fmt::Display for EmbossError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions => write!(f, "Invalid dimensions for terrain or emboss buffer"),
            Self::InvalidParameters => write!(f, "Invalid or non-finite parameters"),
        }
    }
}

impl std::error::Error for EmbossError {}

/// Quantize a floating-point light value to 0-255 byte using banker's rounding (round ties to even).
#[inline]
pub fn quantize_f32(v: f32) -> u8 {
    let rounded = v.round_ties_even();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        rounded as u8
    }
}

/// Quantize a 64-bit floating-point value to 0-255 byte using banker's rounding (round ties to even).
#[inline]
pub fn quantize_f64(v: f64) -> u8 {
    let rounded = v.round_ties_even();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        rounded as u8
    }
}

/// Computes the central difference gradient (gx, gy) at vertex (x, y).
///
/// Uses one-sided difference at grid boundaries matching C# `TerrainHeightEditSession.Gradient`.
#[inline]
pub fn gradient(heights: &[u8], size: usize, x: usize, y: usize) -> (f32, f32) {
    let left = if x == 0 { 0 } else { x - 1 };
    let right = if x + 1 >= size { size - 1 } else { x + 1 };
    let top = if y == 0 { 0 } else { y - 1 };
    let bottom = if y + 1 >= size { size - 1 } else { y + 1 };
    let gx = (heights[y * size + right] as i32 - heights[y * size + left] as i32) as f32;
    let gy = (heights[bottom * size + x] as i32 - heights[top * size + x] as i32) as f32;
    (gx, gy)
}

/// Validates dimensions and computes central difference gradient (gx, gy) at vertex (x, y).
pub fn calculate_gradient(
    heights: &[u8],
    size: usize,
    x: usize,
    y: usize,
) -> Result<(f32, f32), EmbossError> {
    if size < 2 || size.checked_mul(size) != Some(heights.len()) {
        return Err(EmbossError::InvalidDimensions);
    }
    if x >= size || y >= size {
        return Err(EmbossError::InvalidParameters);
    }
    Ok(gradient(heights, size, x, y))
}

/// Determinant of a 3x3 matrix in row-major order.
#[inline]
fn det3x3(
    m00: f64, m01: f64, m02: f64,
    m10: f64, m11: f64, m12: f64,
    m20: f64, m21: f64, m22: f64,
) -> f64 {
    m00 * (m11 * m22 - m12 * m21)
        - m01 * (m10 * m22 - m12 * m20)
        + m02 * (m10 * m21 - m11 * m20)
}

/// Emboss light model parameters fitted via least-squares regression:
/// `light ≈ intercept + slope_x * ∂h/∂x + slope_y * ∂h/∂y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmbossLightModel {
    pub intercept: f32,
    pub slope_x: f32,
    pub slope_y: f32,
    pub r_squared: f64,
}

impl EmbossLightModel {
    /// Fallback model representing missing light or un-fittable data (intercept=255, slopes=0, R²=0).
    pub const fn none() -> Self {
        Self {
            intercept: 255.0,
            slope_x: 0.0,
            slope_y: 0.0,
            r_squared: 0.0,
        }
    }

    /// Whether this model has positive predictive power and finite parameters.
    pub fn is_usable(&self) -> bool {
        self.r_squared > 0.0
            && self.intercept.is_finite()
            && self.slope_x.is_finite()
            && self.slope_y.is_finite()
    }

    /// Convenience wrapper fitting a model from heights and emboss buffers.
    pub fn fit(size: usize, heights: &[u8], emboss: &[u8]) -> Result<Self, EmbossError> {
        let report = fit_emboss_light(size, heights, emboss)?;
        Ok(report.model)
    }
}

impl Default for EmbossLightModel {
    fn default() -> Self {
        Self::none()
    }
}

/// Detailed regression report for light fitting diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbossFitReport {
    pub model: EmbossLightModel,
    pub sample_count: usize,
    pub mean_emboss: f64,
    pub total_variance: f64,
    pub residual_variance: f64,
    pub determinant: f64,
    pub is_usable: bool,
}

/// Fits a 3x3 normal equation for `light ≈ c0 + cx * gx + cy * gy` over all interior vertices `1..size-1`.
///
/// Returns fallback model if count < 16, determinant < 1e-9, total variance <= 1e-6 (constant emboss),
/// or if solving yields non-finite values.
pub fn fit_emboss_light(
    size: usize,
    heights: &[u8],
    emboss: &[u8],
) -> Result<EmbossFitReport, EmbossError> {
    if size < 2 || size.checked_mul(size) != Some(heights.len()) || heights.len() != emboss.len() {
        return Err(EmbossError::InvalidDimensions);
    }

    let mut s00 = 0.0f64;
    let mut s01 = 0.0f64;
    let mut s02 = 0.0f64;
    let mut s11 = 0.0f64;
    let mut s12 = 0.0f64;
    let mut s22 = 0.0f64;
    let mut t0 = 0.0f64;
    let mut t1 = 0.0f64;
    let mut t2 = 0.0f64;
    let mut sum_e = 0.0f64;
    let mut sum_e2 = 0.0f64;
    let mut count = 0usize;

    if size >= 3 {
        for y in 1..size - 1 {
            for x in 1..size - 1 {
                let (gx, gy) = gradient(heights, size, x, y);
                let e = emboss[y * size + x] as f64;
                let gx_d = gx as f64;
                let gy_d = gy as f64;
                s00 += 1.0;
                s01 += gx_d;
                s02 += gy_d;
                s11 += gx_d * gx_d;
                s12 += gx_d * gy_d;
                s22 += gy_d * gy_d;
                t0 += e;
                t1 += gx_d * e;
                t2 += gy_d * e;
                sum_e += e;
                sum_e2 += e * e;
                count += 1;
            }
        }
    }

    if count < 16 {
        return Ok(EmbossFitReport {
            model: EmbossLightModel::none(),
            sample_count: count,
            mean_emboss: if count > 0 { sum_e / (count as f64) } else { 255.0 },
            total_variance: 0.0,
            residual_variance: 0.0,
            determinant: 0.0,
            is_usable: false,
        });
    }

    let det = det3x3(
        s00, s01, s02,
        s01, s11, s12,
        s02, s12, s22,
    );

    if det.abs() < 1e-9 {
        let mean = sum_e / (count as f64);
        let total = sum_e2 - (count as f64) * mean * mean;
        return Ok(EmbossFitReport {
            model: EmbossLightModel::none(),
            sample_count: count,
            mean_emboss: mean,
            total_variance: total.max(0.0),
            residual_variance: 0.0,
            determinant: det,
            is_usable: false,
        });
    }

    let det_c0 = det3x3(
        t0, s01, s02,
        t1, s11, s12,
        t2, s12, s22,
    );
    let det_c1 = det3x3(
        s00, t0, s02,
        s01, t1, s12,
        s02, t2, s22,
    );
    let det_c2 = det3x3(
        s00, s01, t0,
        s01, s11, t1,
        s02, s12, t2,
    );

    let c0 = det_c0 / det;
    let c1 = det_c1 / det;
    let c2 = det_c2 / det;

    if !c0.is_finite() || !c1.is_finite() || !c2.is_finite() {
        let mean = sum_e / (count as f64);
        let total = sum_e2 - (count as f64) * mean * mean;
        return Ok(EmbossFitReport {
            model: EmbossLightModel::none(),
            sample_count: count,
            mean_emboss: mean,
            total_variance: total.max(0.0),
            residual_variance: 0.0,
            determinant: det,
            is_usable: false,
        });
    }

    let mean = sum_e / (count as f64);
    let total = sum_e2 - (count as f64) * mean * mean;
    if total <= 1e-6 {
        return Ok(EmbossFitReport {
            model: EmbossLightModel::none(),
            sample_count: count,
            mean_emboss: mean,
            total_variance: total.max(0.0),
            residual_variance: 0.0,
            determinant: det,
            is_usable: false,
        });
    }

    let mut residual = 0.0f64;
    for y in 1..size - 1 {
        for x in 1..size - 1 {
            let (gx, gy) = gradient(heights, size, x, y);
            let predicted = c0 + c1 * (gx as f64) + c2 * (gy as f64);
            let error = (emboss[y * size + x] as f64) - predicted;
            residual += error * error;
        }
    }

    let r2 = if (1.0 - residual / total).is_finite() {
        (1.0 - residual / total).max(0.0)
    } else {
        0.0
    };

    let model = EmbossLightModel {
        intercept: c0 as f32,
        slope_x: c1 as f32,
        slope_y: c2 as f32,
        r_squared: r2,
    };

    Ok(EmbossFitReport {
        is_usable: model.is_usable(),
        model,
        sample_count: count,
        mean_emboss: mean,
        total_variance: total,
        residual_variance: residual,
        determinant: det,
    })
}

/// Computes the flat emboss base value when applying blank terrain.
///
/// Hierarchy:
/// 1. If light model is usable: `round(intercept).clamp(0, 255)`.
/// 2. Else if saved emboss exists: `round(mean(saved_emboss)).clamp(0, 255)`.
/// 3. Otherwise: 255 (retail default for missing emboss.bmp).
pub fn compute_flat_emboss_base(light: &EmbossLightModel, saved_emboss: Option<&[u8]>) -> u8 {
    if light.is_usable() {
        quantize_f32(light.intercept)
    } else if let Some(emboss) = saved_emboss {
        if !emboss.is_empty() {
            let sum: f64 = emboss.iter().map(|&b| b as f64).sum();
            let avg = sum / (emboss.len() as f64);
            quantize_f64(avg)
        } else {
            255
        }
    } else {
        255
    }
}

/// Pure preview of flat terrain relighting:
/// `lit(x, y) = clamp(round(flat_base + slope_x * gx + slope_y * gy), 0, 255)`.
/// If light model is not usable, all vertices receive `flat_base`.
pub fn preview_relight_flat(
    size: usize,
    flat_base: u8,
    current_heights: &[u8],
    light: &EmbossLightModel,
) -> Result<Vec<u8>, EmbossError> {
    if size < 2 || size.checked_mul(size) != Some(current_heights.len()) {
        return Err(EmbossError::InvalidDimensions);
    }
    if !light.intercept.is_finite()
        || !light.slope_x.is_finite()
        || !light.slope_y.is_finite()
        || !light.r_squared.is_finite()
    {
        return Err(EmbossError::InvalidParameters);
    }

    let mut lit = vec![0u8; current_heights.len()];
    let flat = flat_base as f32;
    let usable = light.is_usable();

    for y in 0..size {
        for x in 0..size {
            let (gx, gy) = gradient(current_heights, size, x, y);
            let value = if usable {
                flat + light.slope_x * gx + light.slope_y * gy
            } else {
                flat
            };
            lit[y * size + x] = quantize_f32(value);
        }
    }
    Ok(lit)
}

/// Pure preview of delta terrain relighting:
/// Only modifies vertices where the gradient changed:
/// `lit(x, y) = clamp(round(saved[i] + slope_x * (gx - bx) + slope_y * (gy - by)), 0, 255)`.
/// If heights are identical to baseline or light model is not usable, returns `saved_emboss` unchanged.
pub fn preview_relight_delta(
    size: usize,
    saved_emboss: &[u8],
    baseline_heights: &[u8],
    current_heights: &[u8],
    light: &EmbossLightModel,
) -> Result<Vec<u8>, EmbossError> {
    if size < 2
        || size.checked_mul(size) != Some(saved_emboss.len())
        || saved_emboss.len() != baseline_heights.len()
        || saved_emboss.len() != current_heights.len()
    {
        return Err(EmbossError::InvalidDimensions);
    }
    if !light.intercept.is_finite()
        || !light.slope_x.is_finite()
        || !light.slope_y.is_finite()
        || !light.r_squared.is_finite()
    {
        return Err(EmbossError::InvalidParameters);
    }

    let mut result = saved_emboss.to_vec();
    if current_heights == baseline_heights || !light.is_usable() {
        return Ok(result);
    }

    for y in 0..size {
        for x in 0..size {
            let (gx, gy) = gradient(current_heights, size, x, y);
            let (bx, by) = gradient(baseline_heights, size, x, y);
            if gx == bx && gy == by {
                continue;
            }
            let index = y * size + x;
            let value = saved_emboss[index] as f32
                + light.slope_x * (gx - bx)
                + light.slope_y * (gy - by);
            result[index] = quantize_f32(value);
        }
    }
    Ok(result)
}

/// Pure preview function multiplexing flat vs delta relight depending on `flat_emboss_base`.
/// Returns `Ok(None)` if `saved_emboss` is `None`.
pub fn preview_emboss(
    size: usize,
    saved_emboss: Option<&[u8]>,
    baseline_heights: &[u8],
    current_heights: &[u8],
    light: &EmbossLightModel,
    flat_emboss_base: Option<u8>,
) -> Result<Option<Vec<u8>>, EmbossError> {
    let Some(saved) = saved_emboss else {
        return Ok(None);
    };
    if let Some(flat) = flat_emboss_base {
        let lit = preview_relight_flat(size, flat, current_heights, light)?;
        Ok(Some(lit))
    } else {
        let lit = preview_relight_delta(size, saved, baseline_heights, current_heights, light)?;
        Ok(Some(lit))
    }
}

/// Manages the emboss baseline lifecycle and relight state matching C# `TerrainHeightEditSession`.
#[derive(Debug, Clone)]
pub struct EmbossSession {
    size: usize,
    baseline_heights: Vec<u8>,
    baseline_emboss: Option<Vec<u8>>,
    saved_emboss: Option<Vec<u8>>,
    light: EmbossLightModel,
    fit_report: EmbossFitReport,
    flat_emboss_base: Option<u8>,
}

impl EmbossSession {
    /// Creates a new emboss session and fits the light model from the baseline terrain.
    pub fn new(
        size: usize,
        baseline_heights: Vec<u8>,
        emboss: Option<Vec<u8>>,
    ) -> Result<Self, EmbossError> {
        if size < 2 || size.checked_mul(size) != Some(baseline_heights.len()) {
            return Err(EmbossError::InvalidDimensions);
        }
        if let Some(ref e) = emboss {
            if e.len() != baseline_heights.len() {
                return Err(EmbossError::InvalidDimensions);
            }
        }

        let fit_report = if let Some(ref e) = emboss {
            fit_emboss_light(size, &baseline_heights, e)?
        } else {
            EmbossFitReport {
                model: EmbossLightModel::none(),
                sample_count: 0,
                mean_emboss: 255.0,
                total_variance: 0.0,
                residual_variance: 0.0,
                determinant: 0.0,
                is_usable: false,
            }
        };

        let light = fit_report.model;
        let baseline_emboss = emboss.clone();
        let saved_emboss = emboss;

        Ok(Self {
            size,
            baseline_heights,
            baseline_emboss,
            saved_emboss,
            light,
            fit_report,
            flat_emboss_base: None,
        })
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn has_emboss(&self) -> bool {
        self.saved_emboss.is_some()
    }

    #[inline]
    pub fn baseline_heights(&self) -> &[u8] {
        &self.baseline_heights
    }

    #[inline]
    pub fn baseline_emboss(&self) -> Option<&[u8]> {
        self.baseline_emboss.as_deref()
    }

    #[inline]
    pub fn saved_emboss(&self) -> Option<&[u8]> {
        self.saved_emboss.as_deref()
    }

    #[inline]
    pub fn light(&self) -> &EmbossLightModel {
        &self.light
    }

    #[inline]
    pub fn fit_report(&self) -> &EmbossFitReport {
        &self.fit_report
    }

    /// Coefficient of determination (R²) of the light model. 0 if unusable or missing.
    #[inline]
    pub fn light_fit_quality(&self) -> f64 {
        self.light.r_squared
    }

    #[inline]
    pub fn flat_emboss_base(&self) -> Option<u8> {
        self.flat_emboss_base
    }

    /// Whether full-map flat relight is pending on the next build/commit (e.g. after blank terrain).
    #[inline]
    pub fn is_relight_pending(&self) -> bool {
        self.flat_emboss_base.is_some()
    }

    /// Whether current heights differ from the baseline heights.
    #[inline]
    pub fn heights_dirty(&self, current_heights: &[u8]) -> bool {
        current_heights != self.baseline_heights
    }

    /// Whether heights are dirty or flat relight is pending.
    #[inline]
    pub fn is_dirty(&self, current_heights: &[u8]) -> bool {
        self.heights_dirty(current_heights) || self.is_relight_pending()
    }

    /// Pure preview of the emboss map for the given `current_heights`.
    pub fn preview_emboss(&self, current_heights: &[u8]) -> Result<Option<Vec<u8>>, EmbossError> {
        preview_emboss(
            self.size,
            self.saved_emboss.as_deref(),
            &self.baseline_heights,
            current_heights,
            &self.light,
            self.flat_emboss_base,
        )
    }

    /// Alias for `preview_emboss` matching C#'s `BuildEmboss()`.
    #[inline]
    pub fn build_emboss(&self, current_heights: &[u8]) -> Result<Option<Vec<u8>>, EmbossError> {
        self.preview_emboss(current_heights)
    }

    /// Sets flat relight pending, computing flat base brightness matching C# `ApplyBlankTerrain`.
    pub fn apply_blank_terrain(&mut self) {
        self.flat_emboss_base = Some(compute_flat_emboss_base(
            &self.light,
            self.saved_emboss.as_deref(),
        ));
    }

    /// Manually sets or clears the pending flat relight base value.
    pub fn set_flat_relight_base(&mut self, flat_base: Option<u8>) {
        self.flat_emboss_base = flat_base;
    }

    /// Commits the current heights and optional new emboss map as the new baseline.
    ///
    /// Clears `flat_emboss_base`. Preserves the originally fitted light model calibration.
    pub fn commit_baseline(
        &mut self,
        current_heights: &[u8],
        saved_emboss: Option<&[u8]>,
    ) -> Result<(), EmbossError> {
        if current_heights.len() != self.baseline_heights.len() {
            return Err(EmbossError::InvalidDimensions);
        }
        if let Some(e) = saved_emboss {
            if e.len() != self.baseline_heights.len() {
                return Err(EmbossError::InvalidDimensions);
            }
            self.saved_emboss = Some(e.to_vec());
        }
        self.baseline_heights = current_heights.to_vec();
        self.flat_emboss_base = None;
        Ok(())
    }

    /// Resets uncommitted state: clears pending flat relight.
    ///
    /// Note: caller is responsible for resetting its current heights buffer to `baseline_heights()`.
    pub fn reset_to_baseline(&mut self) {
        self.flat_emboss_base = None;
    }
}
