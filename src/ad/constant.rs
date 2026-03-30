//! [`Const<T>`] — constant expression wrapper for interoperability with AD
//! expressions.

use core::fmt;
use std::cmp::Ordering;

use crate::ad::scalar::{InnerScalar, Scalar};

/// A constant expression wrapper for interoperability with AD expressions.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Const<T = f64>(pub T);

// -- Scalar impl  -----------------------------------------------------------

impl<T: InnerScalar> Scalar for Const<T> {
    #[inline]
    fn scalar(v: f64) -> Self {
        Self(T::scalar(v))
    }
    #[inline]
    fn value(&self) -> f64 {
        self.0.value()
    }
    #[inline]
    fn zero() -> Self {
        Self(T::zero())
    }
    #[inline]
    fn one() -> Self {
        Self(T::one())
    }
    #[inline]
    fn exp(self) -> Self {
        Self(self.0.exp())
    }
    #[inline]
    fn ln(self) -> Self {
        Self(self.0.ln())
    }
    #[inline]
    fn sqrt(self) -> Self {
        Self(self.0.sqrt())
    }
    #[inline]
    fn sin(self) -> Self {
        Self(self.0.sin())
    }
    #[inline]
    fn cos(self) -> Self {
        Self(self.0.cos())
    }
    #[inline]
    fn abs(self) -> Self {
        Self(self.0.abs())
    }
    #[inline]
    fn powf(self, p: f64) -> Self {
        Self(self.0.powf(p))
    }
    #[inline]
    fn pows(self, p: Self) -> Self {
        Self(self.0.pows(p.0))
    }
    #[inline]
    fn max_val(self, o: Self) -> Self {
        Self(self.0.max_val(o.0))
    }
    #[inline]
    fn min_val(self, o: Self) -> Self {
        Self(self.0.min_val(o.0))
    }
    #[inline]
    fn add_val(self, other: Self) -> Self {
        Self(self.0.add_val(other.0))
    }
    #[inline]
    fn sub_val(self, other: Self) -> Self {
        Self(self.0.sub_val(other.0))
    }
    #[inline]
    fn mul_val(self, other: Self) -> Self {
        Self(self.0.mul_val(other.0))
    }
    #[inline]
    fn div_val(self, other: Self) -> Self {
        Self(self.0.div_val(other.0))
    }
    #[inline]
    fn neg_val(self) -> Self {
        Self(self.0.neg_val())
    }
}

// -- From conversions --------------------------------------------------------

impl<T: InnerScalar> From<f64> for Const<T> {
    fn from(v: f64) -> Self {
        Self(T::scalar(v))
    }
}
impl From<f32> for Const<f64> {
    fn from(v: f32) -> Self {
        Self(f64::from(v))
    }
}
impl From<i32> for Const<f64> {
    fn from(v: i32) -> Self {
        Self(f64::from(v))
    }
}
impl From<u32> for Const<f64> {
    fn from(v: u32) -> Self {
        Self(f64::from(v))
    }
}
impl From<Const<f64>> for f64 {
    fn from(c: Const<f64>) -> Self {
        c.0
    }
}

// -- Default / Display / comparison with f64 ---------------------------------

impl<T: InnerScalar> Default for Const<T> {
    fn default() -> Self {
        Self(T::default())
    }
}
impl<T: InnerScalar> PartialEq<f64> for Const<T> {
    fn eq(&self, rhs: &f64) -> bool {
        self.0.value() == *rhs
    }
}
impl<T: InnerScalar> PartialOrd<f64> for Const<T> {
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.0.value().partial_cmp(rhs)
    }
}
impl<T: InnerScalar> fmt::Display for Const<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
