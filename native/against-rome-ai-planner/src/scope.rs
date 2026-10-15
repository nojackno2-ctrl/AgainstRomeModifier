//! Immutable tile protection contract shared by preview and Apply.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rectangle {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub const fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    pub const fn is_valid(&self) -> bool {
        self.x >= 0
            && self.y >= 0
            && self.x < 64
            && self.y < 64
            && self.width > 0
            && self.height > 0
            && self.width <= 64
            && self.height <= 64
            && self.right() <= 64
            && self.bottom() <= 64
    }
}

/// Immutable tile protection contract shared by preview and Apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiMapEditScope {
    bounds: Rectangle,
    locked: Vec<Rectangle>,
    edit_passability: bool,
}

impl AiMapEditScope {
    pub fn new(
        bounds: Rectangle,
        locked: Vec<Rectangle>,
        edit_passability: bool,
    ) -> Result<Self, String> {
        if !bounds.is_valid() || locked.iter().any(|r| !r.is_valid()) {
            return Err("範圍須為 X,Y,寬,高，位於 64×64 地圖內。".to_string());
        }
        Ok(Self {
            bounds,
            locked,
            edit_passability,
        })
    }

    pub fn bounds(&self) -> &Rectangle {
        &self.bounds
    }

    pub fn locked(&self) -> &[Rectangle] {
        &self.locked
    }

    pub fn edit_passability(&self) -> bool {
        self.edit_passability
    }

    pub fn allows_tile(&self, x: i32, y: i32) -> bool {
        self.bounds.contains(x, y) && !self.locked.iter().any(|r| r.contains(x, y))
    }

    /// A vertex/corner belongs to up to four tiles; protect all neighbouring tile surfaces.
    pub fn allows_vertex(&self, x: f32, y: f32, dimension: i32) -> bool {
        let left = (x.ceil() as i32) - 1;
        let right = x.floor() as i32;
        let top = (y.ceil() as i32) - 1;
        let bottom = y.floor() as i32;

        for ty in top..=bottom {
            for tx in left..=right {
                if tx >= 0 && ty >= 0 && tx < dimension && ty < dimension && !self.allows_tile(tx, ty) {
                    return false;
                }
            }
        }
        true
    }

    pub fn parse(bounds_str: &str, locked_str: &str, edit_passability: bool) -> Result<Self, String> {
        let read = |text: &str| -> Result<Rectangle, String> {
            let parts: Vec<&str> = text.split(',').collect();
            if parts.len() != 4 {
                return Err("請輸入 X,Y,寬,高，例如 8,8,24,24。".to_string());
            }
            let mut values = [0i32; 4];
            for (i, part) in parts.iter().enumerate() {
                values[i] = part
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| "請輸入 X,Y,寬,高，例如 8,8,24,24。".to_string())?;
            }
            Ok(Rectangle::new(values[0], values[1], values[2], values[3]))
        };

        let bounds = read(bounds_str)?;
        let mut locked = Vec::new();
        for line in locked_str.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                locked.push(read(trimmed)?);
            }
        }

        Self::new(bounds, locked, edit_passability)
    }
}
