use crate::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const EMPTY: Self = Self {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

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

    pub fn contains(self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectF {
    pub const EMPTY: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

pub fn zoomed_image_bounds(client: (i32, i32), image: (i32, i32)) -> Rect {
    if client.0 <= 0 || client.1 <= 0 || image.0 <= 0 || image.1 <= 0 {
        return Rect::EMPTY;
    }
    let s = (client.0 as f32 / image.0 as f32).min(client.1 as f32 / image.1 as f32);
    let w = ((image.0 as f32 * s).round_ties_even() as i32).max(1);
    let h = ((image.1 as f32 * s).round_ties_even() as i32).max(1);
    Rect::new((client.0 - w) / 2, (client.1 - h) / 2, w, h)
}

pub fn point_to_tile(point: (i32, i32), r: Rect, dimension: i32) -> Option<Vec2> {
    if dimension <= 0 || r.width <= 0 || r.height <= 0 || !r.contains(point.0, point.1) {
        return None;
    }
    let u = (point.0 - r.x) as f32 / r.width as f32;
    let v = (point.1 - r.y) as f32 / r.height as f32;
    let dim_f = dimension as f32;
    Some(Vec2::new((u * dim_f).clamp(0.0, dim_f), (v * dim_f).clamp(0.0, dim_f)))
}

pub fn tile_to_point(tile: Vec2, r: Rect, dimension: i32) -> Vec2 {
    if dimension <= 0 || r.width <= 0 || r.height <= 0 {
        return Vec2::new(r.x as f32, r.y as f32);
    }
    Vec2::new(
        r.x as f32 + (tile.x / dimension as f32) * r.width as f32,
        r.y as f32 + (tile.y / dimension as f32) * r.height as f32,
    )
}

pub fn canvas_visible_rect(scene: Rect, client: (i32, i32), minimap: Rect, dimension: i32) -> RectF {
    if dimension <= 0
        || scene.width <= 0
        || scene.height <= 0
        || minimap.width <= 0
        || minimap.height <= 0
    {
        return RectF::EMPTY;
    }
    let left = (0.max(-scene.x) as f32) / scene.width as f32;
    let top = (0.max(-scene.y) as f32) / scene.height as f32;
    let right = (scene.width.min(client.0 - scene.x) as f32) / scene.width as f32;
    let bottom = (scene.height.min(client.1 - scene.y) as f32) / scene.height as f32;
    if right <= left || bottom <= top {
        return RectF::EMPTY;
    }
    let (u0, v0, u1, v1) = (
        left.clamp(0.0, 1.0),
        top.clamp(0.0, 1.0),
        right.clamp(0.0, 1.0),
        bottom.clamp(0.0, 1.0),
    );
    RectF {
        x: minimap.x as f32 + u0 * minimap.width as f32,
        y: minimap.y as f32 + v0 * minimap.height as f32,
        width: ((u1 - u0) * minimap.width as f32).max(2.0),
        height: ((v1 - v0) * minimap.height as f32).max(2.0),
    }
}
