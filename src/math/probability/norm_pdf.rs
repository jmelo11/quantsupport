/// 1 / √(2π).
pub const FRAC_1_SQRT_2PI: f64 =
    std::f64::consts::FRAC_2_SQRT_PI * 0.5 * std::f64::consts::FRAC_1_SQRT_2;

/// Standard normal PDF: φ(x) = exp(−x²/2) / √(2π).
#[must_use]
pub fn norm_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() * FRAC_1_SQRT_2PI
}
