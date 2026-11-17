//! Square root (and arc cosine) to float precision
//!
//! The core is `no_std`. On the device - and in the integration tests - `x.sqrt()` is the
//! approximation of micromath: exact at powers of 4, up to 6 % too large in between, never too
//! small. That is the method, not a rounding error. It made the speed to fly up to 17 km/h too
//! high (0.4.1.49, plan_wurzel_genau.md). The unit tests did not see it: with std present,
//! `x.sqrt()` is the exact square root of std there.
//!
//! Wherever a root goes into a number the pilot reads or into a decision, use [`sqrt`].
//! Drawing code that only places pixels may keep the fast one
//! (tests/exact_sqrt.rs lists where, and why).

#[allow(unused_imports)]
use micromath::F32Ext;

/// Square root to f32 precision: the estimate of micromath and three Newton steps.
///
/// The estimate is up to 6.07 % off; the steps bring that to 1.8e-3, 1.7e-6 and then to the
/// rounding of f32 (relative error below 2.5e-7). The estimate is taken explicitly from
/// micromath, so the unit tests compute exactly what the device computes.
///
/// 0 gives 0, negative numbers and NaN give NaN, +inf gives +inf (like `f32::sqrt`).
/// Subnormal numbers (below 1.2e-38) are not handled exactly; they do not occur here.
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x == f32::INFINITY {
        return x;
    }
    let mut y = micromath::F32Ext::sqrt(x);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

/// Arc cosine in [0, pi] with the exact square root. The acos of micromath takes the
/// approximate root inside (up to 0.03 rad off) and gives NaN for arguments a rounding step
/// beyond +-1; those are clamped here. NaN gives NaN. On the device the atan2 of micromath
/// remains (within 0.003 rad, 0.2 deg).
pub fn acos(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    sqrt(1.0 - x * x).atan2(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the worst points of the estimate (2 * 4^k) and ordinary numbers, against f64
    #[test]
    fn the_square_root_is_exact_to_float_precision() {
        let mut worst = 0.0f64;
        let mut x = 1.0e-30f32;
        while x < 1.0e30 {
            for v in [x, 2.0 * x, 3.0 * x] {
                let got = sqrt(v) as f64;
                let want = (v as f64).sqrt();
                worst = worst.max(((got - want) / want).abs());
            }
            x *= 1.37;
        }
        for k in -40..40 {
            let v = 2.0f32 * 4.0f32.powi(k);
            let want = (v as f64).sqrt();
            worst = worst.max(((sqrt(v) as f64 - want) / want).abs());
        }
        assert!(worst < 2.5e-7, "relative error {worst}");
        // the estimate alone is that far off - which is why this function exists
        let est = micromath::F32Ext::sqrt(2.0f32);
        assert!((est - 1.5).abs() < 1e-6, "{est}");
        assert_eq!(sqrt(0.0), 0.0);
        assert!(sqrt(-1.0).is_nan() && sqrt(f32::NAN).is_nan());
        assert_eq!(sqrt(f32::INFINITY), f32::INFINITY);
        assert_eq!(sqrt(4.0), 2.0);
        assert_eq!(sqrt(1.0e6), 1000.0);
    }

    #[test]
    fn acos_is_exact_and_clamps_rounding_beyond_one() {
        for i in 0..=200 {
            let x = -1.0 + i as f32 * 0.01;
            let want = (x as f64).acos();
            assert!(
                (acos(x) as f64 - want).abs() < 1e-5,
                "acos({x}) = {}",
                acos(x)
            );
        }
        assert!((acos(1.000_000_1) - 0.0).abs() < 1e-6);
        assert!((acos(-1.000_000_1) - core::f32::consts::PI).abs() < 1e-6);
        assert!(acos(f32::NAN).is_nan());
    }
}
