//! Inverse Fast Fourier Transform (with 1/N normalisation).
//!
//! Operates **in-place** on slices of [`Complex<T>`] whose length
//! is a power of two.
//!
//! # Example
//! ```
//! use quantsupport::math::fft::{fft, Complex};
//! use quantsupport::math::ifft::ifft;
//!
//! let mut data: Vec<Complex<f64>> = (0..4)
//!     .map(|i| Complex::new(i as f64, 0.0))
//!     .collect();
//! let original = data.clone();
//! fft(&mut data).unwrap();
//! ifft(&mut data).unwrap();
//! for (a, b) in data.iter().zip(&original) {
//!     assert!((a.re - b.re).abs() < 1e-12);
//! }
//! ```

use crate::ad::scalar::Scalar;
use crate::math::fft::{fft_core, Complex};
use crate::utils::errors::{QSError, Result};

/// Computes the inverse DFT in-place (with 1/N normalisation).
///
/// `data.len()` **must** be a power of two.
///
/// # Errors
/// Returns [`QSError::InvalidValueErr`] when the length is not a power of two.
pub fn ifft<T: Scalar>(data: &mut [Complex<T>]) -> Result<()> {
    let n = data.len();
    if n <= 1 {
        return Ok(());
    }
    if !n.is_power_of_two() {
        return Err(QSError::InvalidValueErr(
            "IFFT length must be a power of two".into(),
        ));
    }

    // Run the core with inverse twiddle direction.
    fft_core(data, false)?;

    // Normalise by 1/N.
    let inv_n = T::scalar(1.0 / n as f64);
    for x in data.iter_mut() {
        *x = *x * inv_n;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::fft::fft;

    const EPS: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // FFT then IFFT recovers the original.
    #[test]
    fn roundtrip() {
        let original: Vec<Complex<f64>> = (0..8).map(|i| Complex::new(i as f64, 0.0)).collect();
        let mut data = original.clone();
        fft(&mut data).expect("fft");
        ifft(&mut data).expect("ifft");
        for (a, b) in data.iter().zip(&original) {
            assert!(approx(a.re, b.re));
            assert!(approx(a.im, b.im));
        }
    }

    // Round-trip with complex input.
    #[test]
    fn roundtrip_complex() {
        let original: Vec<Complex<f64>> = (0..4)
            .map(|i| Complex::new(i as f64, (i as f64) * 0.5))
            .collect();
        let mut data = original.clone();
        fft(&mut data).expect("fft");
        ifft(&mut data).expect("ifft");
        for (a, b) in data.iter().zip(&original) {
            assert!(approx(a.re, b.re));
            assert!(approx(a.im, b.im));
        }
    }

    // Non-power-of-two should error.
    #[test]
    fn non_power_of_two() {
        let mut data: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); 6];
        assert!(ifft(&mut data).is_err());
    }
}
