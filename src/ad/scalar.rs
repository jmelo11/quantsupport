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
    #[must_use]
    fn exp(self) -> Self;
    /// Natural logarithm.
    #[must_use]
    fn ln(self) -> Self;
    /// Square root.
    #[must_use]
    fn sqrt(self) -> Self;
    /// Sine.
    #[must_use]
    fn sin(self) -> Self;
    /// Cosine.
    #[must_use]
    fn cos(self) -> Self;
    /// Absolute value.
    #[must_use]
    fn abs(self) -> Self;
    /// Raise to a constant `f64` power.
    #[must_use]
    fn powf(self, p: f64) -> Self;
    /// Raise to a `Self`-typed power (AD-through-AD).
    #[must_use]
    fn pows(self, p: Self) -> Self;
    /// Component-wise maximum.
    #[must_use]
    fn max_val(self, other: Self) -> Self;
    /// Component-wise minimum.
    #[must_use]
    fn min_val(self, other: Self) -> Self;

    // -- Eager arithmetic (always returns `Self`) ----------------------------
    // The expression-template operators on `Dual<T>` return `BinExpr`, not
    // `Self`.  These methods provide guaranteed-`Self`-returning arithmetic
    // for generic code (e.g. `Complex<T>`) that cannot work with lazy trees.

    /// Eager addition.
    #[must_use]
    fn add_val(self, other: Self) -> Self;
    /// Eager subtraction.
    #[must_use]
    fn sub_val(self, other: Self) -> Self;
    /// Eager multiplication.
    #[must_use]
    fn mul_val(self, other: Self) -> Self;
    /// Eager division.
    #[must_use]
    fn div_val(self, other: Self) -> Self;
    /// Eager negation.
    #[must_use]
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
        Self::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        Self::ln(self)
    }
    #[inline]
    fn sqrt(self) -> Self {
        Self::sqrt(self)
    }
    #[inline]
    fn sin(self) -> Self {
        Self::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        Self::cos(self)
    }
    #[inline]
    fn abs(self) -> Self {
        Self::abs(self)
    }
    #[inline]
    fn powf(self, p: f64) -> Self {
        Self::powf(self, p)
    }
    #[inline]
    fn pows(self, p: Self) -> Self {
        Self::powf(self, p)
    }
    #[inline]
    fn max_val(self, o: Self) -> Self {
        Self::max(self, o)
    }
    #[inline]
    fn min_val(self, o: Self) -> Self {
        Self::min(self, o)
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

/// A [`Scalar`] that also supports the standard arithmetic operators.
///
/// Requires `Output = Self`. This is satisfied by the *inner* scalar types
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
