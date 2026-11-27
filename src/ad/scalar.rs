//! Scalar and `InnerScalar` trait definitions.

use core::fmt;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

/// Numeric operations shared by plain values and compiler-recorded expressions.
/// Arithmetic preserves the concrete scalar type.
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

    /// Selects `yes` when this value strictly exceeds `rhs`, otherwise `no`.
    /// Comparisons carry no derivatives. Recorded scalars replay the decision.
    #[must_use]
    fn select_greater(self, rhs: Self, yes: Self, no: Self) -> Self {
        if self > rhs {
            yes
        } else {
            no
        }
    }
}

/// Backward-compatible alias for [`Scalar`].
///
/// Prefer using [`Scalar`] directly in new code.
pub trait IsReal: Scalar {}
impl<T: Scalar> IsReal for T {}

impl Scalar for f64 {
    /// Constructs a constant from a plain numerical value.
    #[inline]
    fn scalar(v: f64) -> Self {
        v
    }
    /// Returns the current numerical value under the scalar observation rules.
    #[inline]
    fn value(&self) -> f64 {
        *self
    }
    /// Returns the additive identity as a constant scalar.
    #[inline]
    fn zero() -> Self {
        0.0
    }
    /// Returns the multiplicative identity as a constant scalar.
    #[inline]
    fn one() -> Self {
        1.0
    }
    /// Calculates the exponential with the scalar implementation.
    #[inline]
    fn exp(self) -> Self {
        Self::exp(self)
    }
    /// Calculates the natural logarithm with the scalar implementation.
    #[inline]
    fn ln(self) -> Self {
        Self::ln(self)
    }
    /// Calculates the square root with the scalar implementation.
    #[inline]
    fn sqrt(self) -> Self {
        Self::sqrt(self)
    }
    /// Calculates the sine of an angle in radians.
    #[inline]
    fn sin(self) -> Self {
        Self::sin(self)
    }
    /// Calculates the cosine of an angle in radians.
    #[inline]
    fn cos(self) -> Self {
        Self::cos(self)
    }
    /// Calculates the absolute value under the scalar derivative convention.
    #[inline]
    fn abs(self) -> Self {
        Self::abs(self)
    }
    /// Raises this scalar to a constant floating-point exponent.
    #[inline]
    fn powf(self, p: f64) -> Self {
        Self::powf(self, p)
    }
    /// Raises this scalar to a scalar exponent, retaining both input dependencies.
    #[inline]
    fn pows(self, p: Self) -> Self {
        Self::powf(self, p)
    }
    /// Selects the larger value under the library's NaN and derivative conventions.
    #[inline]
    fn max_val(self, o: Self) -> Self {
        Self::max(self, o)
    }
    /// Selects the smaller value under the library's NaN and derivative conventions.
    #[inline]
    fn min_val(self, o: Self) -> Self {
        Self::min(self, o)
    }
    /// Adds two scalars while retaining their numerical dependencies.
    #[inline]
    fn add_val(self, other: Self) -> Self {
        self + other
    }
    /// Subtracts two scalars while retaining their numerical dependencies.
    #[inline]
    fn sub_val(self, other: Self) -> Self {
        self - other
    }
    /// Multiplies two scalars while retaining their numerical dependencies.
    #[inline]
    fn mul_val(self, other: Self) -> Self {
        self * other
    }
    /// Divides two scalars while retaining their numerical dependencies.
    #[inline]
    fn div_val(self, other: Self) -> Self {
        self / other
    }
    /// Negates a scalar while retaining its numerical dependencies.
    #[inline]
    fn neg_val(self) -> Self {
        -self
    }
}

/// A numeric scalar with closed arithmetic operator result types.
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
