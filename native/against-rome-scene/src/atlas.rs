//! MaxRects packing; no rotation. Free-list order and score ties match C# DynamicAtlasPacker.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl AtlasRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub fn area(self) -> i64 {
        self.width as i64 * self.height as i64
    }

    pub fn contains(self, b: Self) -> bool {
        b.x >= self.x && b.y >= self.y && b.right() <= self.right() && b.bottom() <= self.bottom()
    }

    pub fn intersects(self, b: Self) -> bool {
        self.x < b.right() && self.right() > b.x && self.y < b.bottom() && self.bottom() > b.y
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AtlasUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaxRectsHeuristic {
    #[default]
    BestShortSideFit,
    BestAreaFit,
    BestLongSideFit,
}

#[derive(Clone, Debug)]
pub struct DynamicAtlasPacker {
    max_size: i32,
    gutter: i32,
    free: Vec<AtlasRect>,
    used: Vec<AtlasRect>,
    transactions: Vec<(Vec<AtlasRect>, usize)>,
}

impl Default for DynamicAtlasPacker {
    fn default() -> Self {
        Self::new(4096, 1).unwrap()
    }
}

impl DynamicAtlasPacker {
    pub fn new(max_size: i32, gutter: i32) -> Result<Self, &'static str> {
        if max_size < 16 || gutter < 0 {
            return Err("invalid atlas size or gutter");
        }
        let mut p = Self {
            max_size,
            gutter,
            free: Vec::new(),
            used: Vec::new(),
            transactions: Vec::new(),
        };
        p.reset();
        Ok(p)
    }

    pub fn max_size(&self) -> i32 {
        self.max_size
    }

    pub fn gutter(&self) -> i32 {
        self.gutter
    }

    pub fn placed_count(&self) -> usize {
        self.used.len()
    }

    pub fn free_rectangles_count(&self) -> usize {
        self.free.len()
    }

    pub fn used_width(&self) -> i32 {
        self.used.iter().map(|r| r.right()).max().unwrap_or(0)
    }

    pub fn used_height(&self) -> i32 {
        self.used.iter().map(|r| r.bottom()).max().unwrap_or(0)
    }

    pub fn reset(&mut self) {
        self.free.clear();
        self.used.clear();
        self.transactions.clear();
        self.free.push(AtlasRect {
            x: 0,
            y: 0,
            width: self.max_size,
            height: self.max_size,
        });
    }

    pub fn calculate_efficiency(&self, w: i32, h: i32) -> f64 {
        if w <= 0 || h <= 0 {
            return 0.0;
        }
        let area: i64 = self
            .used
            .iter()
            .map(|r| (r.width - self.gutter * 2) as i64 * (r.height - self.gutter * 2) as i64)
            .sum();
        area as f64 / (w as f64 * h as f64) * 100.0
    }

    /// Reject nonpositive/overflowing sizes instead of creating invalid free rectangles.
    pub fn try_insert(
        &mut self,
        width: i32,
        height: i32,
        heuristic: MaxRectsHeuristic,
    ) -> Option<(AtlasRect, AtlasUv)> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let g = self.gutter.checked_mul(2)?;
        let w = width.checked_add(g)?;
        let h = height.checked_add(g)?;
        if w > self.max_size || h > self.max_size {
            return None;
        }

        let mut best = None;
        let mut score = (i64::MAX, i64::MAX);

        for f in &self.free {
            if f.width < w || f.height < h {
                continue;
            }
            let dx = (f.width - w) as i64;
            let dy = (f.height - h) as i64;
            let s = match heuristic {
                MaxRectsHeuristic::BestShortSideFit => (dx.min(dy), dx.max(dy)),
                MaxRectsHeuristic::BestAreaFit => (f.area() - w as i64 * h as i64, dx.min(dy)),
                MaxRectsHeuristic::BestLongSideFit => (dx.max(dy), dx.min(dy)),
            };
            if s < score {
                score = s;
                best = Some(AtlasRect {
                    x: f.x,
                    y: f.y,
                    width: w,
                    height: h,
                });
            }
        }

        let p = best?;
        let mut i = 0;
        while i < self.free.len() {
            let f = self.free[i];
            if !f.intersects(p) {
                i += 1;
                continue;
            }
            self.free.remove(i);
            if p.y > f.y && p.y < f.bottom() {
                self.free.push(AtlasRect {
                    x: f.x,
                    y: f.y,
                    width: f.width,
                    height: p.y - f.y,
                });
            }
            if p.bottom() < f.bottom() {
                self.free.push(AtlasRect {
                    x: f.x,
                    y: p.bottom(),
                    width: f.width,
                    height: f.bottom() - p.bottom(),
                });
            }
            if p.x > f.x && p.x < f.right() {
                self.free.push(AtlasRect {
                    x: f.x,
                    y: f.y,
                    width: p.x - f.x,
                    height: f.height,
                });
            }
            if p.right() < f.right() {
                self.free.push(AtlasRect {
                    x: p.right(),
                    y: f.y,
                    width: f.right() - p.right(),
                    height: f.height,
                });
            }
        }

        let mut i = 0;
        while i < self.free.len() {
            let mut j = i + 1;
            let mut removed = false;
            while j < self.free.len() {
                if self.free[j].contains(self.free[i]) {
                    self.free.remove(i);
                    removed = true;
                    break;
                }
                if self.free[i].contains(self.free[j]) {
                    self.free.remove(j);
                } else {
                    j += 1;
                }
            }
            if !removed {
                i += 1;
            }
        }

        self.used.push(p);
        let r = AtlasRect {
            x: p.x + self.gutter,
            y: p.y + self.gutter,
            width,
            height,
        };
        let s = self.max_size as f32;
        Some((
            r,
            AtlasUv {
                u0: r.x as f32 / s,
                v0: r.y as f32 / s,
                u1: r.right() as f32 / s,
                v1: r.bottom() as f32 / s,
            },
        ))
    }

    pub fn begin_transaction(&mut self) {
        self.transactions.push((self.free.clone(), self.used.len()));
    }

    pub fn commit_transaction(&mut self) {
        self.transactions.pop();
    }

    pub fn rollback_transaction(&mut self) {
        if let Some((free, n)) = self.transactions.pop() {
            self.free = free;
            self.used.truncate(n);
        }
    }

    pub fn try_insert_batch(
        &mut self,
        frames: &[(i32, i32)],
    ) -> Option<Vec<(AtlasRect, AtlasUv)>> {
        self.try_insert_batch_with_heuristic(frames, MaxRectsHeuristic::default())
    }

    pub fn try_insert_batch_with_heuristic(
        &mut self,
        frames: &[(i32, i32)],
        heuristic: MaxRectsHeuristic,
    ) -> Option<Vec<(AtlasRect, AtlasUv)>> {
        self.begin_transaction();
        let mut out = Vec::with_capacity(frames.len());
        for &(w, h) in frames {
            if let Some(r) = self.try_insert(w, h, heuristic) {
                out.push(r);
            } else {
                self.rollback_transaction();
                return None;
            }
        }
        self.commit_transaction();
        Some(out)
    }

    pub fn next_power_of_two(value: i32) -> u32 {
        if value <= 1 {
            1
        } else {
            (value as u32).next_power_of_two()
        }
    }
}
