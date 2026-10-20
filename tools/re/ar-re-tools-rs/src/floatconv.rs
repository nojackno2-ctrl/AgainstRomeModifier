//! Exact conditional x87 model of the native float -> signed32 conversion at `0x419c80`
//! (`FSUB` signed near-half bias, then `FISTP`), port of
//! `probe_native_float_conversion.conditional_conversion`.
//!
//! The Python model uses `Fraction`; here values are integers in units of 2^-80, which is exact for
//! every finite float32 below 2^40 in magnitude. Larger values are out of the signed32 range in
//! every rounding mode, and values smaller than one unit are clamped to a single unit, which
//! cannot change any directed or nearest rounding decision (the bias is itself a 24-bit grid
//! point of every supported precision, so the tiny addend only decides the side).

use crate::util::{req, R};

const UNIT_BITS: u32 = 80;
/// float32 `0x3efffffd` = (2^24 - 3) * 2^-25 expressed in units of 2^-80.
const BIAS_UNITS: i128 = ((1i128 << 24) - 3) << (UNIT_BITS - 25);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// `None` when the result is outside signed32.
    pub integer: Option<i64>,
    pub out_of_signed32_range: bool,
    pub precision_bits: u32,
    pub rounding_bits: u32,
}

/// `round(x / 2^k)` for the four x87 rounding modes (0 nearest-even, 1 down, 2 up, 3 truncate).
fn round_div(x: i128, k: u32, mode: u32) -> i128 {
    if k == 0 {
        return x;
    }
    let lower = x >> k;
    let remainder = x - (lower << k);
    match mode {
        1 => lower,
        2 => lower + (remainder != 0) as i128,
        3 => lower + (x < 0 && remainder != 0) as i128,
        _ => {
            let half = 1i128 << (k - 1);
            lower + (remainder > half || (remainder == half && lower & 1 != 0)) as i128
        }
    }
}

fn operation_round(x: i128, precision: u32, mode: u32) -> i128 {
    if x == 0 {
        return 0;
    }
    let exponent = 127 - x.unsigned_abs().leading_zeros() as i32; // floor(log2 |x|) in units
    let k = exponent - precision as i32 + 1;
    if k <= 0 {
        x
    } else {
        round_div(x, k as u32, mode) << k
    }
}

pub fn conditional_conversion(bits: u32, control: u16) -> R<Conversion> {
    req(bits & 0x7f80_0000 != 0x7f80_0000, "model requires finite float32 input")?;
    let precision = match (control >> 8) & 3 {
        0 => 24,
        2 => 53,
        3 => 64,
        _ => return Err("reserved x87 precision control".into()),
    };
    let mode = ((control >> 10) & 3) as u32;
    let negative = bits & 0x8000_0000 != 0;
    let (exp_field, fraction) = ((bits >> 23) & 0xff, (bits & 0x7f_ffff) as i128);
    let (mantissa, exp2) = if exp_field == 0 { (fraction, -149) } else { (fraction | 1 << 23, exp_field as i32 - 150) };
    let rounding_bits = mode;
    if mantissa != 0 && exp2 + 24 > 41 {
        return Ok(Conversion { integer: None, out_of_signed32_range: true, precision_bits: precision, rounding_bits });
    }
    let shift = exp2 + UNIT_BITS as i32;
    let magnitude = if mantissa == 0 {
        0
    } else if shift >= 0 {
        mantissa << shift
    } else {
        // below one unit: clamp to a single unit (or the exact value when it divides evenly)
        let exact = mantissa >> -shift;
        if exact << -shift == mantissa {
            exact
        } else {
            1
        }
    };
    let value = if negative { -magnitude } else { magnitude };
    let bias = if negative { -BIAS_UNITS } else { BIAS_UNITS };
    let adjusted = operation_round(value - bias, precision, mode);
    let result = round_div(adjusted, UNIT_BITS, mode);
    let in_range = (-(1i128 << 31)..(1i128 << 31)).contains(&result);
    Ok(Conversion {
        integer: in_range.then_some(result as i64),
        out_of_signed32_range: !in_range,
        precision_bits: precision,
        rounding_bits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(bits: u32, control: u16) -> Option<i64> {
        conditional_conversion(bits, control).unwrap().integer
    }

    #[test]
    fn matches_the_documented_boundaries() {
        assert_eq!(int(0x3f7f_ffff, 0x037f), Some(1), "extended-nearest boundary");
        assert_eq!(int(0x3f7f_ffff, 0x007f), Some(0), "single-nearest boundary");
        assert_eq!(int(0xbf7f_ffff, 0x037f), Some(-1));
        assert_eq!(int(0, 0x077f), Some(-1), "round-down of +0 minus bias");
        assert_eq!(int(0x8000_0000, 0x0b7f), Some(1), "round-up of -0 plus bias");
        assert!(conditional_conversion(0x4f00_0000, 0x037f).unwrap().out_of_signed32_range);
        assert_eq!(int(0x4eff_ffff, 0x037f), Some(2147483520));
        assert_eq!(int(0xcf00_0000, 0x037f), Some(-2147483648));
        assert!(conditional_conversion(0xcf00_0001, 0x037f).unwrap().out_of_signed32_range);
    }

    #[test]
    fn rejects_nonfinite_and_reserved_precision() {
        assert!(conditional_conversion(0x7f80_0000, 0x037f).is_err());
        assert!(conditional_conversion(0x7fc0_0000, 0x037f).is_err());
        assert!(conditional_conversion(0, 0x017f).is_err());
    }

    #[test]
    fn small_and_denormal_inputs_stay_exact() {
        // Denormals are far below the 24-bit bias grid; only the rounding side matters.
        assert_eq!(int(1, 0x037f), Some(0));
        assert_eq!(int(1, 0x077f), Some(-1));
        assert_eq!(int(1, 0x0b7f), Some(0));
        assert_eq!(int(0x8000_0001, 0x0f7f), Some(0));
        let c = conditional_conversion(0x3f80_0000, 0x0b7f).unwrap();
        assert_eq!((c.precision_bits, c.rounding_bits), (64, 2));
    }
}
