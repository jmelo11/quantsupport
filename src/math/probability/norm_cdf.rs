/// Cumulative distribution function for the standard normal distribution.
pub fn norm_cdf(x: f64) -> f64 {
    // Abramowitz and Stegun approximation for the error function
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let poly = (((a5 * t + a4) * t + a3) * t + a2) * t + a1;
    let y = 1.0 - poly * t * (-x * x).exp();
    let erf = sign * y;
    0.5 * (1.0 + erf)
}
