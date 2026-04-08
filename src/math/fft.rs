//! Radix-2 Cooley–Tukey Fast Fourier Transform (forward DFT).
//!
//! Operates **in-place** on slices of [`Complex<T>`] whose length
//! is a power of two.  The scalar `T` must implement [`Scalar`].
//!
//! # Example
//! ```
//! use quantsupport::math::fft::{fft, Complex};
//!
//! let mut data: Vec<Complex<f64>> = vec![
//!     Complex::new(1.0, 0.0),
//!     Complex::new(1.0, 0.0),
//!     Complex::new(1.0, 0.0),
//!     Complex::new(1.0, 0.0),
//! ];
//! fft(&mut data).unwrap();
//! assert!((data[0].re - 4.0).abs() < 1e-12);
//! ```

use crate::ad::scalar::Scalar;
use crate::utils::errors::{QSError, Result};

/// Minimal complex number that only requires [`Scalar`].
///
/// Unlike `num_complex::Complex<T>`, this does not require `T: Num`, which
/// makes it compatible with expression-template AD types like `DualFwd`.
#[derive(Clone, Copy, Debug)]
pub struct Complex<T> {
    /// Real part.
    pub re: T,
    /// Imaginary part.
    pub im: T,
}

impl<T: Scalar> Complex<T> {
    /// Creates a new complex number.
    #[inline]
    pub fn new(re: T, im: T) -> Self {
        Self { re, im }
    }

    /// The complex zero.
    #[inline]
    pub fn zero() -> Self {
        Self {
            re: T::zero(),
            im: T::zero(),
        }
    }
}

/// Complex addition.
impl<T: Scalar> std::ops::Add for Complex<T> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Complex {
            re: self.re.add_val(rhs.re),
            im: self.im.add_val(rhs.im),
        }
    }
}

/// Complex subtraction.
impl<T: Scalar> std::ops::Sub for Complex<T> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Complex {
            re: self.re.sub_val(rhs.re),
            im: self.im.sub_val(rhs.im),
        }
    }
}

/// Complex multiplication.
impl<T: Scalar> std::ops::Mul for Complex<T> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Complex {
            re: self.re.mul_val(rhs.re).sub_val(self.im.mul_val(rhs.im)),
            im: self.re.mul_val(rhs.im).add_val(self.im.mul_val(rhs.re)),
        }
    }
}

/// Scalar multiplication (real × complex).
impl<T: Scalar> std::ops::Mul<T> for Complex<T> {
    type Output = Self;
    #[inline]
    fn mul(self, s: T) -> Self {
        Complex {
            re: self.re.mul_val(s),
            im: self.im.mul_val(s),
        }
    }
}

/// Computes the forward DFT in-place using the Cooley–Tukey radix-2 algorithm.
///
/// `data.len()` **must** be a power of two.
///
/// # Errors
/// Returns [`QSError::InvalidValueErr`] when the length is not a power of two.
pub fn fft<T: Scalar>(data: &mut [Complex<T>]) -> Result<()> {
    fft_core(data, true)
}

/// Shared core for forward / inverse FFT.
pub(crate) fn fft_core<T: Scalar>(data: &mut [Complex<T>], forward: bool) -> Result<()> {
    let n = data.len();
    if n <= 1 {
        return Ok(());
    }
    if !n.is_power_of_two() {
        return Err(QSError::InvalidValueErr(
            "FFT length must be a power of two".into(),
        ));
    }

    let log_n = n.trailing_zeros();
    let angle_sign: f64 = if forward { -1.0 } else { 1.0 };

    // Bit-reversal permutation.
    bit_reverse_permute(data, log_n);

    // Cooley–Tukey butterfly stages.
    let mut step: usize = 2;
    while step <= n {
        let half = step / 2;
        let angle_step = angle_sign * 2.0 * std::f64::consts::PI / step as f64;

        let mut group = 0;
        while group < n {
            for j in 0..half {
                let theta = angle_step * j as f64;
                let twiddle = Complex::new(T::scalar(theta.cos()), T::scalar(theta.sin()));
                let even_idx = group + j;
                let odd_idx = even_idx + half;
                let t = twiddle * data[odd_idx];
                let u = data[even_idx];
                data[even_idx] = u + t;
                data[odd_idx] = u - t;
            }
            group += step;
        }
        step *= 2;
    }

    Ok(())
}

/// In-place bit-reversal permutation.
fn bit_reverse_permute<T: Copy>(data: &mut [Complex<T>], log_n: u32) {
    let n = data.len();
    for i in 0..n {
        let rev = i.reverse_bits() >> (usize::BITS - log_n);
        if i < rev {
            data.swap(i, rev);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // FFT of all ones (DC signal) → [N, 0, 0, …, 0].
    #[test]
    fn dc_signal() {
        let n = 4;
        let mut data: Vec<Complex<f64>> = vec![Complex::new(1.0, 0.0); n];
        fft(&mut data).expect("fft should succeed");
        assert!(approx(data[0].re, 4.0));
        for item in data.iter().skip(1) {
            assert!(approx(item.re, 0.0));
            assert!(approx(item.im, 0.0));
        }
    }

    // FFT of delta: [1, 0, 0, …] → all ones.
    #[test]
    fn delta_function() {
        let n = 8;
        let mut data: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); n];
        data[0] = Complex::new(1.0, 0.0);
        fft(&mut data).expect("fft");
        for item in &data {
            assert!(approx(item.re, 1.0));
            assert!(approx(item.im, 0.0));
        }
    }

    // Non-power-of-two should error.
    #[test]
    fn non_power_of_two() {
        let mut data: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); 6];
        assert!(fft(&mut data).is_err());
    }

    // Parseval's theorem: sum |X[k]|² = N * sum |x[n]|².
    #[test]
    fn parseval() {
        let original: Vec<Complex<f64>> = vec![
            Complex::new(1.0, 0.0),
            Complex::new(2.0, 0.0),
            Complex::new(3.0, 0.0),
            Complex::new(4.0, 0.0),
        ];
        let energy_time: f64 = original.iter().map(|z| z.re * z.re + z.im * z.im).sum();
        let mut data = original;
        fft(&mut data).expect("fft");
        let energy_freq: f64 = data.iter().map(|z| z.re * z.re + z.im * z.im).sum();
        assert!(approx(energy_freq, 4.0 * energy_time));
    }

    // FFT with DualFwd scalars.
    #[test]
    fn adreal_fft_derivative() {
        use crate::ad::dual::DualFwd;
        use crate::ad::tape::Tape;

        Tape::start_recording_fwd();
        let n = 4;
        let x0 = DualFwd::new(1.0);
        let c = DualFwd::scalar(1.0);
        let z = DualFwd::scalar(0.0);
        let mut data: Vec<Complex<DualFwd>> = vec![
            Complex::new(x0, z),
            Complex::new(c, z),
            Complex::new(c, z),
            Complex::new(c, z),
        ];
        fft(&mut data).expect("fft");
        // X[0] = sum of all inputs = x0 + 1 + 1 + 1 = x0 + 3
        assert!(approx(data[0].re.value(), n as f64));
        Tape::stop_recording_fwd();
    }
}
