use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidDimensions,
    InvalidParameters,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleChange {
    pub index: usize,
    pub before: u8,
    pub after: u8,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stroke {
    pub heights: Vec<SampleChange>,
    pub collision: Vec<SampleChange>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightOperation {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Roughen,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionOperation {
    Block,
    Clear,
}
#[derive(Debug, Clone, Copy, Default)]
pub enum Falloff {
    #[default]
    Smoothstep,
    Cosine,
    Spherical,
    Gaussian,
    Linear,
    Plateau,
}
impl Falloff {
    pub fn weight(self, t: f32) -> f32 {
        if t <= 0.0 {
            return 1.0;
        }
        if t >= 1.0 {
            return 0.0;
        }
        match self {
            Self::Smoothstep => 1.0 - t * t * (3.0 - 2.0 * t),
            Self::Cosine => 0.5 * (1.0 + (std::f32::consts::PI * t).cos()),
            Self::Spherical => (1.0 - t * t).max(0.0).sqrt(),
            Self::Gaussian => (-3.5 * t * t).exp(),
            Self::Linear => 1.0 - t,
            Self::Plateau => {
                if t < 0.4 {
                    1.0
                } else {
                    let v = (t - 0.4) / 0.6;
                    1.0 - v * v * (3.0 - 2.0 * v)
                }
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct RampParameters {
    pub width: f32,
    pub falloff: f32,
    pub start_height: Option<f32>,
    pub end_height: Option<f32>,
    pub falloff_type: Falloff,
    pub round_caps: bool,
}
impl Default for RampParameters {
    fn default() -> Self {
        Self {
            width: 4.0,
            falloff: 2.0,
            start_height: None,
            end_height: None,
            falloff_type: Falloff::Smoothstep,
            round_caps: true,
        }
    }
}
fn validate(size: usize, data: &[u8]) -> Result<(), Error> {
    if size < 2 || size.checked_mul(size) != Some(data.len()) {
        Err(Error::InvalidDimensions)
    } else {
        Ok(())
    }
}
fn quantize(v: f32) -> u8 {
    v.round_ties_even().clamp(0.0, 255.0) as u8
}
fn finite(values: &[f32]) -> Result<(), Error> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Error::InvalidParameters)
    }
}
fn sample(data: &[u8], size: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, (size - 1) as f32);
    let y = y.clamp(0.0, (size - 1) as f32);
    let left = (x as usize).min(size - 2);
    let top = (y as usize).min(size - 2);
    let tx = x - left as f32;
    let ty = y - top as f32;
    let a = data[top * size + left] as f32;
    let b = data[top * size + left + 1] as f32;
    let c = data[(top + 1) * size + left] as f32;
    let d = data[(top + 1) * size + left + 1] as f32;
    (a + (b - a) * tx) * (1.0 - ty) + (c + (d - c) * tx) * ty
}
pub fn sample_bilinear(data: &[u8], size: usize, x: f32, y: f32) -> Result<f32, Error> {
    validate(size, data)?;
    finite(&[x, y])?;
    Ok(sample(data, size, x, y))
}
pub fn tile_to_vertex(x: i32, y: i32, dimension: usize, size: usize) -> (f32, f32) {
    if dimension == 0 || size < 2 {
        return (0.0, 0.0);
    }
    let step = (size - 1) as f32 / dimension as f32;
    ((x as f32 + 0.5) * step, (y as f32 + 0.5) * step)
}
/// Pure ramp calculation: the source is never mutated, so callers can preview.
pub fn calculate_ramp(
    data: &[u8],
    size: usize,
    start: (f32, f32),
    end: (f32, f32),
    p: &RampParameters,
) -> Result<Vec<SampleChange>, Error> {
    validate(size, data)?;
    finite(&[start.0, start.1, end.0, end.1, p.width, p.falloff])?;
    for h in [p.start_height, p.end_height].into_iter().flatten() {
        finite(&[h])?;
    }
    if p.width < 0.0 || p.falloff < 0.0 {
        return Err(Error::InvalidParameters);
    }
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len_sq = dx * dx + dy * dy;
    finite(&[len_sq, p.width / 2.0 + p.falloff])?;
    let half = p.width / 2.0;
    let radius = half + p.falloff;
    if len_sq < 1e-6 || radius <= 0.0 {
        return Ok(vec![]);
    }
    let h0 = p
        .start_height
        .unwrap_or_else(|| sample(data, size, start.0, start.1))
        .clamp(0.0, 255.0);
    let h1 = p
        .end_height
        .unwrap_or_else(|| sample(data, size, end.0, end.1))
        .clamp(0.0, 255.0);
    let mut changes = vec![];
    for y in 0..size {
        for x in 0..size {
            let vx = x as f32 - start.0;
            let vy = y as f32 - start.1;
            let t = (vx * dx + vy * dy) / len_sq;
            let (t, dist) = if p.round_caps {
                let t = t.clamp(0.0, 1.0);
                let px = start.0 + t * dx;
                let py = start.1 + t * dy;
                (
                    t,
                    ((x as f32 - px).powi(2) + (y as f32 - py).powi(2)).sqrt(),
                )
            } else {
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                (t, (vx * (-dy) + vy * dx).abs() / len_sq.sqrt())
            };
            if dist > radius {
                continue;
            }
            let w = if dist <= half {
                1.0
            } else if p.falloff > 1e-4 {
                p.falloff_type.weight((dist - half) / p.falloff)
            } else {
                0.0
            };
            let index = y * size + x;
            let before = data[index];
            let after = quantize(w * (h0 + t * (h1 - h0)) + (1.0 - w) * before as f32);
            if after != before {
                changes.push(SampleChange {
                    index,
                    before,
                    after,
                });
            }
        }
    }
    Ok(changes)
}

/// Height/collision strokes share a transaction and a single undo item.
#[derive(Clone)]
pub struct TerrainSession {
    size: usize,
    collision_size: usize,
    heights: Vec<u8>,
    collision: Option<Vec<u8>>,
    baseline_heights: Vec<u8>,
    baseline_collision: Option<Vec<u8>>,
    pending_heights: BTreeMap<usize, SampleChange>,
    pending_collision: BTreeMap<usize, SampleChange>,
    undo: Vec<Stroke>,
    redo: Vec<Stroke>,
}
fn track(pending: &mut BTreeMap<usize, SampleChange>, index: usize, before: u8, after: u8) {
    let before = pending.get(&index).map_or(before, |c| c.before);
    if before == after {
        pending.remove(&index);
    } else {
        pending.insert(
            index,
            SampleChange {
                index,
                before,
                after,
            },
        );
    }
}
impl TerrainSession {
    pub fn new(
        size: usize,
        heights: Vec<u8>,
        collision_size: usize,
        collision: Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        validate(size, &heights)?;
        if let Some(c) = &collision {
            if collision_size == 0 || collision_size.checked_mul(collision_size) != Some(c.len()) {
                return Err(Error::InvalidDimensions);
            }
        }
        Ok(Self {
            size,
            collision_size,
            baseline_heights: heights.clone(),
            baseline_collision: collision.clone(),
            heights,
            collision,
            pending_heights: BTreeMap::new(),
            pending_collision: BTreeMap::new(),
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn vertex_size(&self) -> usize {
        self.size
    }
    pub fn collision_size(&self) -> usize {
        self.collision_size
    }
    pub fn heights(&self) -> &[u8] {
        &self.heights
    }
    pub fn collision(&self) -> Option<&[u8]> {
        self.collision.as_deref()
    }
    pub fn is_dirty(&self) -> bool {
        self.heights != self.baseline_heights || self.collision != self.baseline_collision
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
            || !self.pending_heights.is_empty()
            || !self.pending_collision.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn apply_height_changes(&mut self, changes: &[SampleChange]) -> Vec<SampleChange> {
        let mut applied = vec![];
        for c in changes {
            if let Some(v) = self.heights.get_mut(c.index) {
                let before = *v;
                if before != c.after {
                    *v = c.after;
                    track(&mut self.pending_heights, c.index, before, c.after);
                    applied.push(SampleChange {
                        index: c.index,
                        before,
                        after: c.after,
                    });
                }
            }
        }
        if !applied.is_empty() {
            self.redo.clear();
        }
        applied
    }
    /// Sets specific vertices to target heights (e.g. river bed carving); out-of-range indices are skipped.
    pub fn apply_height_adjustments(&mut self, targets: &[(usize, u8)]) -> Vec<SampleChange> {
        let changes: Vec<SampleChange> = targets
            .iter()
            .map(|&(index, after)| SampleChange {
                index,
                before: 0,
                after,
            })
            .collect();
        self.apply_height_changes(&changes)
    }
    /// Applies a per-vertex transform over the whole map (batch/AI edits). Non-finite results are
    /// skipped; values round ties-to-even and clamp to 0..=255. Caller commits the stroke.
    pub fn transform_heights(
        &mut self,
        mut f: impl FnMut(usize, usize, u8) -> f32,
    ) -> Vec<SampleChange> {
        let size = self.size;
        let mut changes = vec![];
        for y in 0..size {
            for x in 0..size {
                let index = y * size + x;
                let value = f(x, y, self.heights[index]);
                if !value.is_finite() {
                    continue;
                }
                changes.push(SampleChange {
                    index,
                    before: 0,
                    after: value.round_ties_even().clamp(0.0, 255.0) as u8,
                });
            }
        }
        self.apply_height_changes(&changes)
    }
    /// C# TerrainSculptPipeline.Elevate: float strength, selectable falloff and an
    /// upper height bound. Quantizes once per call; leaves the stroke uncommitted.
    /// Non-finite inputs follow the original .NET 8 x64 pipeline, including its
    /// unchecked float-to-int conversion; unlike PaintHeight, it does not reject them.
    pub fn elevate(
        &mut self,
        center: (f32, f32),
        radius: f32,
        strength: f32,
        falloff: Falloff,
        max_height: f32,
    ) -> Result<Vec<SampleChange>, Error> {
        self.sculpt_offset(center, radius, strength, falloff, max_height, true)
    }

    /// C# TerrainSculptPipeline.Depress; the lower bound is applied before byte
    /// quantization. Shares pending history with other height/collision edits.
    pub fn depress(
        &mut self,
        center: (f32, f32),
        radius: f32,
        strength: f32,
        falloff: Falloff,
        min_height: f32,
    ) -> Result<Vec<SampleChange>, Error> {
        self.sculpt_offset(center, radius, strength, falloff, min_height, false)
    }

    fn sculpt_offset(
        &mut self,
        center: (f32, f32),
        radius: f32,
        strength: f32,
        falloff: Falloff,
        limit: f32,
        elevate: bool,
    ) -> Result<Vec<SampleChange>, Error> {
        // .NET 8 x64 unchecked (int)float returns int.MinValue for NaN,
        // infinities and overflow. Rust's saturating cast has different semantics.
        fn csharp_int(value: f32) -> i32 {
            if (-2147483648.0..2147483648.0).contains(&value) {
                value as i32
            } else {
                i32::MIN
            }
        }
        if radius <= 0.0 || strength <= 0.0 {
            return Ok(vec![]);
        }
        // Match GetBrushBounds before distance checks, including overflow/NaN.
        let min_x = csharp_int((center.0 - radius).floor()).max(0);
        let max_x = csharp_int((center.0 + radius).ceil()).min(self.size as i32 - 1);
        let min_y = csharp_int((center.1 - radius).floor()).max(0);
        let max_y = csharp_int((center.1 + radius).ceil()).min(self.size as i32 - 1);
        let mut changes = vec![];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - center.0;
                let dy = y as f32 - center.1;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > radius {
                    continue;
                }
                let index = y as usize * self.size + x as usize;
                let before = self.heights[index];
                let delta = strength * falloff.weight(distance / radius);
                // Apply the bound even at zero-weight boundary vertices, as C# does.
                // Math.Min/Max propagate NaN; Rust's min/max would ignore it.
                let value = if limit.is_nan() || delta.is_nan() {
                    f32::NAN
                } else if elevate {
                    (before as f32 + delta).min(limit)
                } else {
                    (before as f32 - delta).max(limit)
                };
                // ExtractChanges rounds, casts to int, THEN clamps to byte range.
                let after = csharp_int(value.round_ties_even()).clamp(0, 255) as u8;
                if before != after {
                    changes.push(SampleChange {
                        index,
                        before,
                        after,
                    });
                }
            }
        }
        Ok(self.apply_height_changes(&changes))
    }

    pub fn apply_ramp(
        &mut self,
        start: (f32, f32),
        end: (f32, f32),
        p: &RampParameters,
    ) -> Result<Vec<SampleChange>, Error> {
        let changes = calculate_ramp(&self.heights, self.size, start, end, p)?;
        let changes = self.apply_height_changes(&changes);
        if !changes.is_empty() {
            self.commit_stroke();
        }
        Ok(changes)
    }
    pub fn paint_height(
        &mut self,
        center: (f32, f32),
        radius: f32,
        operation: HeightOperation,
        strength: i32,
        flatten_target: Option<u8>,
        seed: i32,
    ) -> Result<Vec<SampleChange>, Error> {
        finite(&[center.0, center.1, radius])?;
        if radius <= 0.0 {
            return Err(Error::InvalidParameters);
        }
        let strength = strength.clamp(1, 64) as f32;
        let source = self.heights.clone();
        let target = flatten_target.unwrap_or_else(|| {
            source[(center
                .1
                .round_ties_even()
                .clamp(0.0, (self.size - 1) as f32) as usize)
                * self.size
                + center
                    .0
                    .round_ties_even()
                    .clamp(0.0, (self.size - 1) as f32) as usize]
        });
        let mut changes = vec![];
        for y in 0..self.size {
            for x in 0..self.size {
                let distance =
                    ((x as f32 - center.0).powi(2) + (y as f32 - center.1).powi(2)).sqrt();
                if distance > radius {
                    continue;
                }
                let f = 0.5 + 0.5 * (std::f32::consts::PI * distance / radius).cos();
                let index = y * self.size + x;
                let before = source[index];
                let v = match operation {
                    HeightOperation::Raise => before as f32 + strength * f,
                    HeightOperation::Lower => before as f32 - strength * f,
                    HeightOperation::Flatten => {
                        before as f32
                            + (target as f32 - before as f32) * (strength / 16.0).min(1.0) * f
                    }
                    HeightOperation::Roughen => {
                        before as f32 + strength * f * roughness_noise(x as i32, y as i32, seed)
                    }
                    HeightOperation::Smooth => {
                        let mut sum = 0u32;
                        let mut count = 0;
                        for sy in y.saturating_sub(1)..=(y + 1).min(self.size - 1) {
                            for sx in x.saturating_sub(1)..=(x + 1).min(self.size - 1) {
                                sum += source[sy * self.size + sx] as u32;
                                count += 1;
                            }
                        }
                        before as f32
                            + (sum as f32 / count as f32 - before as f32)
                                * (strength / 8.0).min(1.0)
                                * f
                    }
                };
                let after = quantize(v);
                if before != after {
                    changes.push(SampleChange {
                        index,
                        before,
                        after,
                    });
                }
            }
        }
        Ok(self.apply_height_changes(&changes))
    }
    pub fn paint_collision(
        &mut self,
        center: (f32, f32),
        radius: f32,
        operation: CollisionOperation,
    ) -> Result<Vec<SampleChange>, Error> {
        self.paint_collision_where(center, radius, operation, |_, _| true)
    }
    /// Designer water tool, not an inferred native navigation algorithm.
    /// Existing nonzero navigation values and dry shore are preserved.
    pub fn paint_water(
        &mut self,
        center: (f32, f32),
        radius: f32,
        strength: i32,
        bed: u8,
        surface: f32,
    ) -> Result<Stroke, Error> {
        finite(&[surface])?;
        if surface <= 0.0 || bed as f32 >= surface {
            return Err(Error::InvalidParameters);
        }
        let heights = self.paint_height(
            center,
            radius,
            HeightOperation::Flatten,
            strength,
            Some(bed),
            0,
        )?;
        if self.collision.is_none() {
            return Ok(Stroke {
                heights,
                collision: vec![],
            });
        }
        let scale = self.collision_size as f32 / (self.size - 1) as f32;
        let source = self.heights.clone();
        let size = self.size;
        let collision_size = self.collision_size;
        let previous = self.collision.clone().unwrap();
        let collision = self.paint_collision_where(
            (center.0 * scale, center.1 * scale),
            radius * scale,
            CollisionOperation::Block,
            |x, y| {
                previous[y * collision_size + x] == 0
                    && sample(
                        &source,
                        size,
                        (x as f32 + 0.5) / scale,
                        (y as f32 + 0.5) / scale,
                    ) < surface
            },
        )?;
        Ok(Stroke { heights, collision })
    }
    pub fn paint_collision_where(
        &mut self,
        center: (f32, f32),
        radius: f32,
        operation: CollisionOperation,
        allows: impl Fn(usize, usize) -> bool,
    ) -> Result<Vec<SampleChange>, Error> {
        if self.collision.is_none() {
            return Ok(vec![]);
        }
        finite(&[center.0, center.1, radius])?;
        if radius <= 0.0 {
            return Err(Error::InvalidParameters);
        }
        let data = self.collision.as_mut().unwrap();
        let after = if operation == CollisionOperation::Block {
            255
        } else {
            0
        };
        let mut changes = vec![];
        for y in 0..self.collision_size {
            for x in 0..self.collision_size {
                let dx = x as f32 + 0.5 - center.0;
                let dy = y as f32 + 0.5 - center.1;
                if dx * dx + dy * dy > radius * radius || !allows(x, y) {
                    continue;
                }
                let index = y * self.collision_size + x;
                let before = data[index];
                if before != after {
                    data[index] = after;
                    track(&mut self.pending_collision, index, before, after);
                    changes.push(SampleChange {
                        index,
                        before,
                        after,
                    });
                }
            }
        }
        if !changes.is_empty() {
            self.redo.clear();
        }
        Ok(changes)
    }
    pub fn commit_stroke(&mut self) -> bool {
        if self.pending_heights.is_empty() && self.pending_collision.is_empty() {
            return false;
        }
        self.undo.push(Stroke {
            heights: std::mem::take(&mut self.pending_heights)
                .into_values()
                .collect(),
            collision: std::mem::take(&mut self.pending_collision)
                .into_values()
                .collect(),
        });
        true
    }
    fn restore(&mut self, stroke: &Stroke, after: bool) {
        for c in &stroke.heights {
            self.heights[c.index] = if after { c.after } else { c.before };
        }
        if let Some(data) = &mut self.collision {
            for c in &stroke.collision {
                data[c.index] = if after { c.after } else { c.before };
            }
        }
    }
    pub fn cancel_stroke(&mut self) -> Stroke {
        let stroke = Stroke {
            heights: std::mem::take(&mut self.pending_heights)
                .into_values()
                .collect(),
            collision: std::mem::take(&mut self.pending_collision)
                .into_values()
                .collect(),
        };
        self.restore(&stroke, false);
        stroke
    }
    pub fn undo(&mut self) -> Option<Stroke> {
        self.commit_stroke();
        let stroke = self.undo.pop()?;
        self.restore(&stroke, false);
        self.redo.push(stroke.clone());
        Some(Stroke {
            heights: stroke.heights.into_iter().map(invert).collect(),
            collision: stroke.collision.into_iter().map(invert).collect(),
        })
    }
    pub fn redo(&mut self) -> Option<Stroke> {
        let stroke = self.redo.pop()?;
        self.restore(&stroke, true);
        self.undo.push(stroke.clone());
        Some(stroke)
    }
    pub fn commit_baseline(&mut self) {
        self.baseline_heights = self.heights.clone();
        self.baseline_collision = self.collision.clone();
        self.clear_history();
    }
    pub fn reset_to_baseline(&mut self) {
        self.heights.clone_from(&self.baseline_heights);
        self.collision.clone_from(&self.baseline_collision);
        self.clear_history();
    }
    fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.pending_heights.clear();
        self.pending_collision.clear();
    }
}
fn invert(c: SampleChange) -> SampleChange {
    SampleChange {
        index: c.index,
        before: c.after,
        after: c.before,
    }
}
pub fn roughness_noise(x: i32, y: i32, seed: i32) -> f32 {
    fn lattice(x: i32, y: i32, s: i32) -> f32 {
        let mut h = x
            .wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(s.wrapping_mul(1442695041)) as u32;
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        h ^= h >> 16;
        (h & 65535) as f32 / 32767.5 - 1.0
    }
    fn smooth(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }
    let fx = x as f32 / 4.0;
    let fy = y as f32 / 4.0;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let tx = smooth(fx - x0 as f32);
    let ty = smooth(fy - y0 as f32);
    lerp(
        lerp(lattice(x0, y0, seed), lattice(x0 + 1, y0, seed), tx),
        lerp(lattice(x0, y0 + 1, seed), lattice(x0 + 1, y0 + 1, seed), tx),
        ty,
    )
}
