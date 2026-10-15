//! Ecology noise generator matching C# EcologyNoise.
//! Allocation-free, deterministic 2D gradient and multi-octave fractal noise.

/// 2D Gradient Noise normalized to [0.0, 1.0].
pub fn perlin_2d(x: f32, y: f32, seed: i32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let dx = x - x0 as f32;
    let dy = y - y0 as f32;

    // Quintic Fade Curve: 6t^5 - 15t^4 + 10t^3
    let u = dx * dx * dx * (dx * (dx * 6.0 - 15.0) + 10.0);
    let v = dy * dy * dy * (dy * (dy * 6.0 - 15.0) + 10.0);

    let g00 = dot_grad(hash(x0, y0, seed), dx, dy);
    let g10 = dot_grad(hash(x1, y0, seed), dx - 1.0, dy);
    let g01 = dot_grad(hash(x0, y1, seed), dx, dy - 1.0);
    let g11 = dot_grad(hash(x1, y1, seed), dx - 1.0, dy - 1.0);

    let nx0 = lerp(g00, g10, u);
    let nx1 = lerp(g01, g11, u);
    let value = lerp(nx0, nx1, v);

    ((value + 1.0) * 0.5).clamp(0.0, 1.0)
}

/// Multi-octave fractal brownian motion noise normalized to [0.0, 1.0].
pub fn octave_noise_2d(
    x: f32,
    y: f32,
    octaves: i32,
    persistence: f32,
    lacunarity: f32,
    seed: i32,
) -> f32 {
    if octaves <= 1 {
        return perlin_2d(x, y, seed);
    }

    let mut total = 0.0f32;
    let mut frequency = 1.0f32;
    let mut amplitude = 1.0f32;
    let mut max_amplitude = 0.0f32;

    for i in 0..octaves {
        total += perlin_2d(x * frequency, y * frequency, seed + i * 1013) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amplitude > 0.0 {
        (total / max_amplitude).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[inline]
fn hash(x: i32, y: i32, seed: i32) -> i32 {
    let mut h = (seed as u32)
        ^ (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h ^ (h >> 16)) as i32
}

#[inline]
fn dot_grad(hash: i32, x: f32, y: f32) -> f32 {
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x * 1.2,
        5 => -x * 1.2,
        6 => y * 1.2,
        _ => -y * 1.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_2d_deterministic_and_bounded() {
        for seed in [0, 42, 1337, -999] {
            for x in [0.0, 0.5, 1.25, 10.7, -4.3] {
                for y in [0.0, 0.5, 2.75, 15.1, -8.6] {
                    let val = perlin_2d(x, y, seed);
                    assert!(val >= 0.0 && val <= 1.0, "value {val} out of bounds");
                    assert_eq!(val, perlin_2d(x, y, seed));
                }
            }
        }
    }

    #[test]
    fn octave_noise_bounded() {
        for octaves in [1, 2, 3, 4] {
            let val = octave_noise_2d(15.5, 22.3, octaves, 0.5, 2.0, 1234);
            assert!(val >= 0.0 && val <= 1.0);
        }
    }
}
