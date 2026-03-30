//! Scalar and InnerScalar trait definitions.

use core::fmt;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

// ═══════════════════════════════════════════════════════════════════════════
//  Scalar trait  (replaces the old IsReal)
// ═══════════════════════════════════════════════════════════════════════════

/// Trait unifying plain `f64`, [`ADForward`](super::forward::ADForward), and
/// [`Dual<T>`](super::dual::Dual) so that pricing / math code can be written
/// generically over the scalar type.
///
/// Provides only construction, value access, and transcendentals.
/// Arithmetic operators are provided by the expression template system.
pub trait Scalar:
    Sized
    + Copy
    + Default
    + PartialEq
    + PartialOrd
    + PartialEq<f64>
    + PartialOrd<f64>
    + From<f64>
    + Send
    + Sync
    + fmt::Debug
    + fmt::Display
{
    /// Creates a new scalar from `f64`.
    fn scalar(v: f64) -> Self;
    /// Returns the underlying `f64` value.
    fn value(&self) -> f64;
    /// The additive identity.
    fn zero() -> Self;
    /// The multiplicative identity.
    fn one() -> Self;
    /// Exponential.
    fn exp(self) -> Self;
    /// Natural logarithm.
    fn ln(self) -> Self;
    /// Square root.
    fn sqrt(self) -> Self;
    /// Sine.
    fn sin(self) -> Self;
    /// Cosine.
    fn cos(self) -> Self;
    /// Absolute value.
    fn abs(self) -> Self;
    /// Raise to a constant `f64` power.
    fn powf(self, p: f64) -> Self;
    /// Raise to a `Self`-typed power (AD-through-AD).
    fn pows(self, p: Self) -> Self;
    /// Component-wise maximum.
    fn max_val(self, other: Self) -> Self;
    /// Component-wise minimum.
    fn min_val(self, other: Self) -> Self;

    // -- Eager arithmetic (always returns `Self`) ----------------------------
    // The expression-template operators on `Dual<T>` return `BinExpr`, not
    // `Self`.  These methods provide guaranteed-`Self`-returning arithmetic
    // for generic code (e.g. `Complex<T>`) that cannot work with lazy trees.

    /// Eager addition.
    fn add_val(self, other: Self) -> Self;
    /// Eager subtraction.
    fn sub_val(self, other: Self) -> Self;
    /// Eager multiplication.
    fn mul_val(self, other: Self) -> Self;
    /// Eager division.
    fn div_val(self, other: Self) -> Self;
    /// Eager negation.
    fn neg_val(self) -> Self;
}

/// Backward-compatible alias for [`Scalar`].
///
/// Prefer using [`Scalar`] directly in new code.
pub trait IsReal: Scalar {}
impl<T: Scalar> IsReal for T {}

// -- Scalar impl for f64 ----------------------------------------------------

impl Scalar for f64 {
    #[inline]
    fn scalar(v: f64) -> Self {
        v
    }
    #[inline]
    fn value(&self) -> f64 {
        *self
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn exp(self) -> Self {
        f64::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        f64::ln(self)
    }
    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline]
    fn sin(self) -> Self {
        f64::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        f64::cos(self)
    }
    #[inline]
    fn abs(self) -> Self {
        f64::abs(self)
    }
    #[inline]
    fn powf(self, p: f64) -> Self {
        f64::powf(self, p)
    }
    #[inline]
    fn pows(self, p: Self) -> Self {
        f64::powf(self, p)
    }
    #[inline]
    fn max_val(self, o: Self) -> Self {
        f64::max(self, o)
    }
    #[inline]
    fn min_val(self, o: Self) -> Self {
        f64::min(self, o)
    }
    #[inline]
    fn add_val(self, other: Self) -> Self {
        self + other
    }
    #[inline]
    fn sub_val(self, other: Self) -> Self {
        self - other
    }
    #[inline]
    fn mul_val(self, other: Self) -> Self {
        self * other
    }
    #[inline]
    fn div_val(self, other: Self) -> Self {
        self / other
    }
    #[inline]
    fn neg_val(self) -> Self {
        -self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  InnerScalar — Scalar + arithmetic ops (only f64 and ADForward)
// ═══════════════════════════════════════════════════════════════════════════

/// A [`Scalar`] that also supports the standard arithmetic operators
/// with `Output = Self`. This is satisfied by the *inner* scalar types
/// (`f64`, [`ADForward`](super::forward::ADForward)) but **not** by
/// [`Dual<T>`](super::dual::Dual) (whose operators return expression-template
/// types).
pub trait InnerScalar:
    Scalar
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + Add<f64, Output = Self>
    + Sub<f64, Output = Self>
    + Mul<f64, Output = Self>
    + Div<f64, Output = Self>
{
}

impl InnerScalar for f64 {}
