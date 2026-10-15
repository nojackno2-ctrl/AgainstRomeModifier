use serde::{Deserialize, Serialize};

/// 樣條插值過渡曲線類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraEasingType {
    /// 等速線性過渡。
    Linear,
    /// 平滑 S 型過渡 (3t^2 - 2t^3)。
    SmoothStep,
    /// 慢入二次方 (t^2)。
    EaseInQuad,
    /// 慢出二次方 (1 - (1-t)^2)。
    EaseOutQuad,
    /// 慢入慢出三次方 (前半 4t^3, 後半 1 - (-2t+2)^3 / 2)。
    EaseInOutCubic,
}

impl Default for CameraEasingType {
    fn default() -> Self {
        Self::SmoothStep
    }
}

/// 相機路徑點 (Camera Waypoint)，定義特定時刻相機在世界空間中的焦點位置與姿態。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraWaypoint {
    /// 世界空間焦點座標 (X, Y, Z)。地圖範圍為 0..16384。
    pub position: [f32; 3],
    /// 相機俯仰角 (Pitch)，以度為單位 (原遊戲預設約 30° 等角俯角，範圍 15°..85°)。
    pub pitch_degrees: f32,
    /// 相機偏航角 (Yaw)，以度為單位 (原遊戲預設 45° 等角方向，0°..360°)。
    pub yaw_degrees: f32,
    /// 相機焦距 / 距離 (Zoom / Distance，預設 82，僅用於預覽)。
    pub zoom: f32,
    /// 從前一個航點飛行到達此航點所需持續時間（秒）。第一個航點通常為 0。
    pub duration_from_previous: f32,
    /// 到達此航點之加減速曲線風格。
    pub easing: CameraEasingType,
    /// 航點備註標籤。
    pub label: Option<String>,
}

impl Default for CameraWaypoint {
    fn default() -> Self {
        Self {
            position: [8192.0, 0.0, 8192.0],
            pitch_degrees: 30.0,
            yaw_degrees: 45.0,
            zoom: 82.0,
            duration_from_previous: 3.0,
            easing: CameraEasingType::SmoothStep,
            label: None,
        }
    }
}

impl CameraWaypoint {
    pub fn new(
        position: [f32; 3],
        pitch_degrees: f32,
        yaw_degrees: f32,
        zoom: f32,
        duration: f32,
        easing: CameraEasingType,
        label: Option<String>,
    ) -> Self {
        Self {
            position,
            pitch_degrees,
            yaw_degrees,
            zoom,
            duration_from_previous: duration.max(0.0),
            easing,
            label,
        }
    }
}

/// 相機在特定時間戳的即時姿態與幾何數據。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraPose {
    pub position: [f32; 3],
    pub pitch_degrees: f32,
    pub yaw_degrees: f32,
    pub zoom: f32,
    pub time_seconds: f32,
}

impl Default for CameraPose {
    fn default() -> Self {
        Self {
            position: [8192.0, 0.0, 8192.0],
            pitch_degrees: 30.0,
            yaw_degrees: 45.0,
            zoom: 82.0,
            time_seconds: 0.0,
        }
    }
}

/// 樣條相機軌跡規劃器 (Camera Track Spline Planner)
#[derive(Debug, Clone, Default)]
pub struct CameraTrackSplinePlanner {
    waypoints: Vec<CameraWaypoint>,
    cumulative_times: Vec<f32>,
    unwrapped_yaws: Vec<f32>,
}

impl CameraTrackSplinePlanner {
    pub fn new(waypoints: &[CameraWaypoint]) -> Self {
        let mut planner = Self {
            waypoints: waypoints.to_vec(),
            cumulative_times: vec![],
            unwrapped_yaws: vec![],
        };
        planner.rebuild_timeline();
        planner
    }

    pub fn waypoints(&self) -> &[CameraWaypoint] {
        &self.waypoints
    }

    pub fn total_duration(&self) -> f32 {
        self.cumulative_times.last().copied().unwrap_or(0.0)
    }

    pub fn count(&self) -> usize {
        self.waypoints.len()
    }

    pub fn set_waypoints(&mut self, waypoints: &[CameraWaypoint]) {
        self.waypoints = waypoints.to_vec();
        self.rebuild_timeline();
    }

    pub fn add_waypoint(&mut self, waypoint: CameraWaypoint) {
        self.waypoints.push(waypoint);
        self.rebuild_timeline();
    }

    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.cumulative_times.clear();
        self.unwrapped_yaws.clear();
    }

    fn rebuild_timeline(&mut self) {
        self.cumulative_times.clear();
        self.unwrapped_yaws.clear();

        if self.waypoints.is_empty() {
            return;
        }

        // 1. 累積時間戳記
        let mut total = 0.0f32;
        for (i, wp) in self.waypoints.iter().enumerate() {
            if i > 0 {
                total += 0.001f32.max(wp.duration_from_previous);
            }
            self.cumulative_times.push(total);
        }

        // 2. 角度連續展開 (Angle Unwrapping)
        let mut last_yaw = self.waypoints[0].yaw_degrees;
        self.unwrapped_yaws.push(last_yaw);
        for i in 1..self.waypoints.len() {
            let current_yaw = self.waypoints[i].yaw_degrees;
            let mut diff = current_yaw - (last_yaw % 360.0);
            diff = ((diff + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
            last_yaw += diff;
            self.unwrapped_yaws.push(last_yaw);
        }
    }

    pub fn evaluate(&self, time_seconds: f32) -> CameraPose {
        if self.waypoints.is_empty() {
            return CameraPose {
                position: [8192.0, 0.0, 8192.0],
                pitch_degrees: 30.0,
                yaw_degrees: 45.0,
                zoom: 82.0,
                time_seconds,
            };
        }

        if self.waypoints.len() == 1 || time_seconds <= 0.0 {
            let first = &self.waypoints[0];
            return CameraPose {
                position: first.position,
                pitch_degrees: first.pitch_degrees,
                yaw_degrees: normalize_angle(first.yaw_degrees),
                zoom: first.zoom,
                time_seconds: 0.0,
            };
        }

        let total_duration = self.total_duration();
        if time_seconds >= total_duration {
            let last = self.waypoints.last().unwrap();
            return CameraPose {
                position: last.position,
                pitch_degrees: last.pitch_degrees,
                yaw_degrees: normalize_angle(last.yaw_degrees),
                zoom: last.zoom,
                time_seconds: total_duration,
            };
        }

        // 尋找當前時間所在的航點區間 [segment_index, segment_index + 1]
        let mut segment_index = 0;
        for i in 0..self.cumulative_times.len() - 1 {
            if time_seconds >= self.cumulative_times[i] && time_seconds <= self.cumulative_times[i + 1] {
                segment_index = i;
                break;
            }
        }

        let t_start = self.cumulative_times[segment_index];
        let t_end = self.cumulative_times[segment_index + 1];
        let segment_duration = t_end - t_start;
        let raw_t = if segment_duration > 0.0001 {
            ((time_seconds - t_start) / segment_duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let eased_t = Self::apply_easing(raw_t, self.waypoints[segment_index + 1].easing);
        let (p0, p1, p2, p3) = self.get_spline_segment_positions(segment_index);
        let position = Self::evaluate_centripetal_catmull_rom(p0, p1, p2, p3, eased_t);

        let wp1 = &self.waypoints[segment_index];
        let wp2 = &self.waypoints[segment_index + 1];

        let pitch = lerp(wp1.pitch_degrees, wp2.pitch_degrees, eased_t).clamp(10.0, 89.0);
        let yaw_unwrapped = lerp(
            self.unwrapped_yaws[segment_index],
            self.unwrapped_yaws[segment_index + 1],
            eased_t,
        );
        let zoom = lerp(wp1.zoom, wp2.zoom, eased_t).max(1.0);

        CameraPose {
            position,
            pitch_degrees: pitch,
            yaw_degrees: normalize_angle(yaw_unwrapped),
            zoom,
            time_seconds,
        }
    }

    pub fn sample_path(&self, sample_count: usize) -> Vec<CameraPose> {
        if sample_count < 2 || self.waypoints.len() < 2 {
            return self
                .waypoints
                .iter()
                .map(|w| CameraPose {
                    position: w.position,
                    pitch_degrees: w.pitch_degrees,
                    yaw_degrees: normalize_angle(w.yaw_degrees),
                    zoom: w.zoom,
                    time_seconds: 0.0,
                })
                .collect();
        }

        let mut results = Vec::with_capacity(sample_count);
        let step = self.total_duration() / (sample_count - 1) as f32;
        for i in 0..sample_count {
            let t = i as f32 * step;
            results.push(self.evaluate(t));
        }
        results
    }

    fn get_spline_segment_positions(
        &self,
        segment_index: usize,
    ) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
        let count = self.waypoints.len();
        let p1 = self.waypoints[segment_index].position;
        let p2 = self.waypoints[segment_index + 1].position;

        let p0 = if segment_index > 0 {
            self.waypoints[segment_index - 1].position
        } else {
            // p1 + (p1 - p2)
            [
                p1[0] + (p1[0] - p2[0]),
                p1[1] + (p1[1] - p2[1]),
                p1[2] + (p1[2] - p2[2]),
            ]
        };

        let p3 = if segment_index + 2 < count {
            self.waypoints[segment_index + 2].position
        } else {
            // p2 + (p2 - p1)
            [
                p2[0] + (p2[0] - p1[0]),
                p2[1] + (p2[1] - p1[1]),
                p2[2] + (p2[2] - p1[2]),
            ]
        };

        (p0, p1, p2, p3)
    }

    /// 向心 Catmull-Rom 樣條評估 (Centripetal Catmull-Rom Spline, α = 0.5)
    pub fn evaluate_centripetal_catmull_rom(
        p0: [f32; 3],
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
        t: f32,
    ) -> [f32; 3] {
        const ALPHA: f32 = 0.5;

        fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
            let dx = a[0] - b[0];
            let dy = a[1] - b[1];
            let dz = a[2] - b[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        }

        fn get_knot(t_prev: f32, a: [f32; 3], b: [f32; 3]) -> f32 {
            let d = dist(a, b);
            t_prev + (1e-4f32.max(d)).powf(ALPHA)
        }

        let t0 = 0.0f32;
        let t1 = get_knot(t0, p0, p1);
        let t2 = get_knot(t1, p1, p2);
        let t3 = get_knot(t2, p2, p3);

        let actual_t = t1 + t * (t2 - t1);

        fn vlerp(t_val: f32, t_start: f32, t_end: f32, a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            let factor = actual_t_factor(t_val, t_start, t_end);
            [
                (1.0 - factor) * a[0] + factor * b[0],
                (1.0 - factor) * a[1] + factor * b[1],
                (1.0 - factor) * a[2] + factor * b[2],
            ]
        }

        fn actual_t_factor(t_val: f32, t_start: f32, t_end: f32) -> f32 {
            if (t_end - t_start).abs() > 1e-6 {
                (t_val - t_start) / (t_end - t_start)
            } else {
                0.0
            }
        }

        let a1 = vlerp(actual_t, t0, t1, p0, p1);
        let a2 = vlerp(actual_t, t1, t2, p1, p2);
        let a3 = vlerp(actual_t, t2, t3, p2, p3);

        let b1 = vlerp(actual_t, t0, t2, a1, a2);
        let b2 = vlerp(actual_t, t1, t3, a2, a3);

        vlerp(actual_t, t1, t2, b1, b2)
    }

    /// 套用過渡曲線
    pub fn apply_easing(t: f32, easing: CameraEasingType) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match easing {
            CameraEasingType::Linear => t,
            CameraEasingType::SmoothStep => t * t * (3.0 - 2.0 * t),
            CameraEasingType::EaseInQuad => t * t,
            CameraEasingType::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            CameraEasingType::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn normalize_angle(degrees: f32) -> f32 {
    let mut angle = degrees % 360.0;
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}
