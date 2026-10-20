use crate::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainRay {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[derive(Clone, Debug)]
pub struct EditorCamera {
    pub target: Vec3,
    yaw_degrees: f32,
    pitch_degrees: f32,
    distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub orthographic: bool,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(32.0, 0.0, 32.0),
            yaw_degrees: 45.0,
            pitch_degrees: 30.0,
            distance: 82.0,
            min_distance: 2.0,
            max_distance: 180.0,
            orthographic: true,
        }
    }
}

impl EditorCamera {
    pub const FIELD_OF_VIEW_DEGREES: f32 = 52.0;
    pub const ORTHOGRAPHIC_EYE_DISTANCE: f32 = 400.0;
    pub const GAME_PIXELS_PER_TILE: f32 = 256.0 * 0.70710678;

    pub fn yaw_degrees(&self) -> f32 {
        self.yaw_degrees
    }

    pub fn pitch_degrees(&self) -> f32 {
        self.pitch_degrees
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn set_distance(&mut self, distance: f32) {
        self.distance = distance.clamp(self.min_distance, self.max_distance);
    }

    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        self.yaw_degrees += yaw;
        self.pitch_degrees = (self.pitch_degrees + pitch).clamp(20.0, 80.0);
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(self.min_distance, self.max_distance);
    }

    pub fn zoom_to_game_scale(&mut self, viewport_height: i32) {
        let half_fov_tan = (Self::FIELD_OF_VIEW_DEGREES * 0.5).to_radians().tan();
        let target_dist = viewport_height as f32 / Self::GAME_PIXELS_PER_TILE / (2.0 * half_fov_tan);
        self.distance = target_dist.clamp(self.min_distance, self.max_distance);
    }

    pub fn pan(&mut self, right: f32, forward: f32) {
        let yaw = self.yaw_degrees.to_radians();
        let f = Vec3::new(yaw.sin(), 0.0, yaw.cos()).normalized();
        let r = Vec3::new(f.z, 0.0, -f.x);
        self.target = self.target + r * right + f * forward;
    }

    pub fn position(&self) -> Vec3 {
        let yaw = self.yaw_degrees.to_radians();
        let pitch = self.pitch_degrees.to_radians();
        let direction = Vec3::new(
            pitch.cos() * yaw.sin(),
            pitch.sin(),
            pitch.cos() * yaw.cos(),
        );
        let dist = if self.orthographic {
            Self::ORTHOGRAPHIC_EYE_DISTANCE
        } else {
            self.distance
        };
        self.target + direction * dist
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at(self.position(), self.target)
    }

    /// Right-handed System.Numerics projection matching C# EditorCamera, depth range 0..1.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(0.1);
        if self.orthographic {
            let half_fov_tan = (Self::FIELD_OF_VIEW_DEGREES * 0.5).to_radians().tan();
            let height = 2.0 * self.distance * half_fov_tan;
            Mat4::orthographic(height * aspect, height, 1.0, Self::ORTHOGRAPHIC_EYE_DISTANCE * 2.0)
        } else {
            Mat4::perspective_fov(Self::FIELD_OF_VIEW_DEGREES.to_radians(), aspect, 0.1, 500.0)
        }
    }

    /// Retains C#'s clip-z conversion z*2-1, including its near-ray origin convention.
    pub fn create_ray(&self, x: f32, y: f32, width: f32, height: f32) -> TerrainRay {
        if width <= 0.0 || height <= 0.0 {
            return TerrainRay {
                origin: self.target,
                direction: Vec3::new(0.0, -1.0, 0.0),
            };
        }
        let aspect = width / height;
        let inverse = (self.view_matrix() * self.projection_matrix(aspect)).inverse();
        let unproject = |z: f32| {
            if let Some(m) = inverse {
                let w = m.transform4([
                    x / width * 2.0 - 1.0,
                    1.0 - y / height * 2.0,
                    z * 2.0 - 1.0,
                    1.0,
                ]);
                if w[3].abs() > 1e-12 {
                    Vec3::new(w[0] / w[3], w[1] / w[3], w[2] / w[3])
                } else {
                    self.target
                }
            } else {
                self.target
            }
        };
        let near = unproject(0.0);
        let far = unproject(1.0);
        let dir = (far - near).normalized();
        TerrainRay {
            origin: near,
            direction: dir,
        }
    }

    /// Focuses the camera target on a specific tile position, sampling terrain height.
    pub fn focus_tile(&mut self, tile_x: f32, tile_y: f32, heights: &crate::terrain::TerrainHeightField) {
        let y = heights.sample_height(tile_x, tile_y);
        self.target = Vec3::new(tile_x, y, tile_y);
    }

    /// Zooms in or out by the standard editor stepping factor (0.84 / 1.19).
    pub fn zoom_step(&mut self, zoom_in: bool) {
        self.zoom(if zoom_in { 0.84 } else { 1.19 });
    }
}
