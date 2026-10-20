//! Small row-vector math vocabulary matching System.Numerics.
//! Row-vector convention: transformations are performed as `vector * matrix`.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance(self, b: Self) -> f32 {
        (self - b).length()
    }

    pub fn dot(self, b: Self) -> f32 {
        self.x * b.x + self.y * b.y
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self::new(self.x * b, self.y * b)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const UNIT_X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const UNIT_Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const UNIT_Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, b: Self) -> f32 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }

    pub fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance(self, b: Self) -> f32 {
        (self - b).length()
    }

    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 1e-12 {
            self * (1.0 / len)
        } else {
            Self::ZERO
        }
    }

    pub fn min(self, b: Self) -> Self {
        Self::new(self.x.min(b.x), self.y.min(b.y), self.z.min(b.z))
    }

    pub fn max(self, b: Self) -> Self {
        Self::new(self.x.max(b.x), self.y.max(b.y), self.z.max(b.z))
    }

    pub fn clamp(self, min: Self, max: Self) -> Self {
        self.max(min).min(max)
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self::new(self.x * b, self.y * b, self.z * b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    pub fn transform4(self, v: [f32; 4]) -> [f32; 4] {
        std::array::from_fn(|j| {
            v[0] * self.0[0][j]
                + v[1] * self.0[1][j]
                + v[2] * self.0[2][j]
                + v[3] * self.0[3][j]
        })
    }

    pub fn transform_point(self, v: Vec3) -> Vec3 {
        let p = self.transform4([v.x, v.y, v.z, 1.0]);
        Vec3::new(p[0], p[1], p[2])
    }

    /// System.Numerics CreateLookAt (right-handed, row-vector convention).
    pub fn look_at(eye: Vec3, target: Vec3) -> Self {
        let z = (eye - target).normalized();
        let x = Vec3::new(0.0, 1.0, 0.0).cross(z).normalized();
        let y = z.cross(x);
        Self([
            [x.x, y.x, z.x, 0.0],
            [x.y, y.y, z.y, 0.0],
            [x.z, y.z, z.z, 0.0],
            [-x.dot(eye), -y.dot(eye), -z.dot(eye), 1.0],
        ])
    }

    /// System.Numerics CreateOrthographic (right-handed, row-vector convention, depth 0..1).
    pub fn orthographic(width: f32, height: f32, near: f32, far: f32) -> Self {
        let dz = near - far;
        Self([
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, 2.0 / height, 0.0, 0.0],
            [0.0, 0.0, 1.0 / dz, 0.0],
            [0.0, 0.0, near / dz, 1.0],
        ])
    }

    /// System.Numerics CreatePerspectiveFieldOfView (right-handed, row-vector convention, depth 0..1).
    pub fn perspective_fov(fov_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let y_scale = 1.0 / (fov_radians * 0.5).tan();
        let x_scale = y_scale / aspect;
        let z_range = near - far;
        Self([
            [x_scale, 0.0, 0.0, 0.0],
            [0.0, y_scale, 0.0, 0.0],
            [0.0, 0.0, far / z_range, -1.0],
            [0.0, 0.0, (near * far) / z_range, 0.0],
        ])
    }

    pub fn inverse(self) -> Option<Self> {
        let mut a = [[0f64; 8]; 4];
        for (i, row) in a.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().take(4).enumerate() {
                *v = self.0[i][j] as f64;
            }
            row[i + 4] = 1.0;
        }
        for col in 0..4 {
            let pivot = (col..4).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
            if !a[pivot][col].is_finite() || a[pivot][col].abs() < 1e-30 {
                return None;
            }
            a.swap(col, pivot);
            let d = a[col][col];
            for v in &mut a[col] {
                *v /= d;
            }
            let p = a[col];
            for (i, row) in a.iter_mut().enumerate() {
                if i != col {
                    let f = row[col];
                    for j in 0..8 {
                        row[j] -= f * p[j];
                    }
                }
            }
        }
        Some(Self(std::array::from_fn(|i| {
            std::array::from_fn(|j| a[i][j + 4] as f32)
        })))
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self(std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..4).map(|k| self.0[i][k] * b.0[k][j]).sum())
        }))
    }
}
