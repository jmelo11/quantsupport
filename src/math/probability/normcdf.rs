//! The standard normal cumulative distribution for numerical and recorded scalars.
//!
//! Plain values use Hart's rational approximation and a continued fraction in
//! the tails. Recorded values capture a normal-CDF instruction, whose compiled
//! derivatives use the analytic Gaussian density and its derivative.

use crate::ad::scalar::Scalar;

/// `sqrt(2 * pi)`.
const SQRT_2PI: f64 = 2.506_628_274_631_000_5;

/// Threshold between the rational-Hart branch and the continued-fraction
/// tail branch: `5 * sqrt(2)`.
const HART_THRESHOLD: f64 = 7.071_067_811_865_475;

/// Evaluates the standard normal cumulative probability using the scalar's implementation.
#[must_use]
pub fn norm_cdf<T: NormCDF>(x: T) -> T {
    x.norm_cdf()
}

/// Supplies the standard normal cumulative distribution for a scalar type.
///
/// The `f64` implementation follows G. West, "Better approximations to cumulative normal
/// functions", Wilmott 2005. Absolute error ~1e-17 everywhere; relative
/// error in the deep tails (|x| > 5) is ~1e-9.
pub trait NormCDF: Scalar + Clone {
    /// Computes the standard normal CDF.
    #[must_use]
    fn norm_cdf(self) -> Self;
}

/// Evaluates the Hart approximation using double-precision arithmetic.
impl NormCDF for f64 {
    /// Calculates the cumulative probability with a separate approximation for deep tails.
    fn norm_cdf(self) -> Self {
        let z = self.abs();
        let cum = if z > 37.0 {
            0.0
        } else {
            let e = (-z * z * 0.5).exp();
            if z < HART_THRESHOLD {
                let num = 3.526_249_659_989_11e-2_f64
                    .mul_add(z, 0.700_383_064_443_688)
                    .mul_add(z, 6.373_962_203_531_65)
                    .mul_add(z, 33.912_866_078_383)
                    .mul_add(z, 112.079_291_497_871)
                    .mul_add(z, 221.213_596_169_931)
                    .mul_add(z, 220.206_867_912_376);
                let den = 8.838_834_764_831_84e-2_f64
                    .mul_add(z, 1.755_667_163_182_64)
                    .mul_add(z, 16.064_177_579_207)
                    .mul_add(z, 86.780_732_202_946_1)
                    .mul_add(z, 296.564_248_779_674)
                    .mul_add(z, 637.333_633_378_831)
                    .mul_add(z, 793.826_512_519_948)
                    .mul_add(z, 440.413_735_824_752);
                e * num / den
            } else {
                let b = z + 0.65;
                let b = z + 4.0 / b;
                let b = z + 3.0 / b;
                let b = z + 2.0 / b;
                let b = z + 1.0 / b;
                e / (b * SQRT_2PI)
            }
        };
        if self > 0.0 {
            1.0 - cum
        } else {
            cum
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::Recorded;
    /// Reference values computed via `erfc(-x/sqrt(2))/2` (correctly rounded).
    const REFERENCES: [(f64, f64); 7] = [
        (0.0, 0.5),
        (0.5, 0.691_462_461_274_013),
        (1.0, 0.841_344_746_068_542_9),
        (2.0, 0.977_249_868_051_820_8),
        (-1.0, 0.158_655_253_931_457_05),
        (-5.0, 2.866_515_718_791_945e-7),
        (-10.0, 7.619_853_024_160_593e-24),
    ];

    #[test]
    fn matches_reference_values_including_deep_tails() {
        for (x, expected) in REFERENCES {
            let computed = x.norm_cdf();
            // West's algorithm: ~1e-17 absolute error everywhere, ~1e-9
            // relative error in the deep tails.
            let ok = (computed - expected).abs() < 1e-15
                || (computed - expected).abs() / expected < 1e-8;
            assert!(ok, "norm_cdf({x}) = {computed}, expected {expected}");
        }
    }

    #[test]
    fn is_symmetric_and_monotone() {
        let mut prev = 0.0;
        for i in -400..=400 {
            let x = f64::from(i) * 0.05;
            let cdf = x.norm_cdf();
            assert!(
                (cdf + (-x).norm_cdf() - 1.0).abs() < 1e-15,
                "symmetry at {x}"
            );
            assert!(cdf >= prev, "monotonicity at {x}");
            prev = cdf;
        }
        assert!((37.5_f64).norm_cdf() == 1.0);
        assert!((-37.5_f64).norm_cdf() == 0.0);
    }

    #[test]
    fn recorded_value_matches_f64() {
        for x in [-10.0, -5.0, -1.0, 0.0, 0.5, 2.0, 8.0, 20.0] {
            let recorded: Recorded = Recorded::from(x).norm_cdf();
            let plain = x.norm_cdf();
            assert!(
                (recorded.value() - plain).abs() <= plain.abs().mul_add(1e-14, 1e-300),
                "Recorded norm_cdf({x}) = {} but f64 gives {plain}",
                recorded.value()
            );
        }
    }
}
