//! Deterministic pseudo-random number generator matching modern .NET System.Random (xoshiro256**).

#[derive(Clone, Debug, PartialEq)]
pub struct DotNetRandom {
    s0: u64,
    s1: u64,
    s2: u64,
    s3: u64,
}

impl DotNetRandom {
    pub fn new(seed: i32) -> Self {
        let mut sm = seed as u32 as u64;
        let s0 = splitmix64(&mut sm);
        let s1 = splitmix64(&mut sm);
        let s2 = splitmix64(&mut sm);
        let s3 = splitmix64(&mut sm);
        Self { s0, s1, s2, s3 }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = (self.s1.wrapping_mul(5)).rotate_left(7).wrapping_mul(9);
        let t = self.s1 << 17;
        self.s2 ^= self.s0;
        self.s3 ^= self.s1;
        self.s1 ^= self.s2;
        self.s0 ^= self.s3;
        self.s2 ^= t;
        self.s3 = self.s3.rotate_left(45);
        result
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Returns a random floating-point number that is greater than or equal to 0.0, and less than 1.0.
    #[inline]
    pub fn next_double(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Returns a random single-precision float in [0.0, 1.0).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.next_double() as f32
    }

    /// Returns a non-negative random integer that is less than the specified maximum.
    pub fn next_i32(&mut self, max_value: i32) -> i32 {
        if max_value <= 0 {
            return 0;
        }
        let max = max_value as u32;
        let mut result = (self.next_u32() as u64).wrapping_mul(max as u64);
        let mut low = result as u32;
        if low < max {
            let remainder = (0u32.wrapping_sub(max)) % max;
            while low < remainder {
                result = (self.next_u32() as u64).wrapping_mul(max as u64);
                low = result as u32;
            }
        }
        (result >> 32) as i32
    }

    /// Returns a random integer that is within a specified range [min, max).
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        min + self.next_i32(max - min)
    }
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_reproducible() {
        let mut r1 = DotNetRandom::new(12345);
        let mut r2 = DotNetRandom::new(12345);
        for _ in 0..100 {
            assert_eq!(r1.next_double(), r2.next_double());
            assert_eq!(r1.next_i32(50), r2.next_i32(50));
        }
    }

    #[test]
    fn values_in_bounds() {
        let mut r = DotNetRandom::new(42);
        for _ in 0..1000 {
            let d = r.next_double();
            assert!(d >= 0.0 && d < 1.0);
            let n = r.next_i32(10);
            assert!(n >= 0 && n < 10);
            let rng = r.next_range(5, 15);
            assert!(rng >= 5 && rng < 15);
        }
    }
}
