//! Forward-mode automatic differentiation: [`ADForward`].

use core::fmt;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, Sub, SubAssign};

use bumpalo::Bump;

use crate::ad::scalar::{InnerScalar, Scalar};
use crate::ad::tape::{Tape, TapeHolder};

// ═══════════════════════════════════════════════════════════════════════════
//  ADForward — pure forward-mode type  (val, dot, dot2)
// ═══════════════════════════════════════════════════════════════════════════

/// A forward-mode automatic differentiation number carrying a value and up to
/// second-order derivative seeds.
#[derive(Clone, Copy)]
pub struct ADForward {
    /// Function value.
    pub val: f64,
    /// First-order forward derivative.
    pub dot: f64,
    /// Second-order forward derivative.
    pub dot2: f64,
}

impl ADForward {
    /// Constant (no derivative seeds).
    #[inline]
    #[must_use]
    pub const fn constant(v: f64) -> Self {
        Self {
            val: v,
            dot: 0.0,
            dot2: 0.0,
        }
    }

    /// Independent variable seeded for first-derivative computation.
    #[inline]
    #[must_use]
    pub const fn var(v: f64) -> Self {
        Self {
            val: v,
            dot: 1.0,
            dot2: 0.0,
        }
    }

    /// Returns the function value.
    #[inline]
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.val
    }

    /// Returns the first forward derivative.
    #[inline]
    #[must_use]
    pub const fn first_derivative(&self) -> f64 {
        self.dot
    }

    /// Returns the second forward derivative.
    #[inline]
    #[must_use]
    pub const fn second_derivative(&self) -> f64 {
        self.dot2
    }
}

impl Default for ADForward {
    fn default() -> Self {
        Self {
            val: 0.0,
            dot: 0.0,
            dot2: 0.0,
        }
    }
}
impl fmt::Debug for ADForward {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fwd({}, d:{}, d2:{})", self.val, self.dot, self.dot2)
    }
}
impl fmt::Display for ADForward {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.val)
    }
}
impl PartialEq for ADForward {
    fn eq(&self, o: &Self) -> bool {
        self.val == o.val
    }
}
impl PartialOrd for ADForward {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        self.val.partial_cmp(&o.val)
    }
}
impl PartialEq<f64> for ADForward {
    fn eq(&self, rhs: &f64) -> bool {
        self.val == *rhs
    }
}
impl PartialOrd<f64> for ADForward {
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.val.partial_cmp(rhs)
    }
}
impl From<f64> for ADForward {
    fn from(v: f64) -> Self {
        Self::constant(v)
    }
}

// -- ADForward arithmetic ---------------------------------------------------

impl Add for ADForward {
    type Output = Self;
    #[inline]
    fn add(self, r: Self) -> Self {
        Self {
            val: self.val + r.val,
            dot: self.dot + r.dot,
            dot2: self.dot2 + r.dot2,
        }
    }
}
impl Sub for ADForward {
    type Output = Self;
    #[inline]
    fn sub(self, r: Self) -> Self {
        Self {
            val: self.val - r.val,
            dot: self.dot - r.dot,
            dot2: self.dot2 - r.dot2,
        }
    }
}
impl Mul for ADForward {
    type Output = Self;
    #[inline]
    fn mul(self, r: Self) -> Self {
        Self {
            val: self.val * r.val,
            dot: self.dot.mul_add(r.val, self.val * r.dot),
            dot2: self.val.mul_add(r.dot2, self.dot2.mul_add(r.val, 2.0 * self.dot * r.dot)),
        }
    }
}
impl Div for ADForward {
    type Output = Self;
    #[inline]
    fn div(self, r: Self) -> Self {
        let inv = 1.0 / r.val;
        let inv2 = inv * inv;
        Self {
            val: self.val * inv,
            dot: self.dot.mul_add(r.val, -(self.val * r.dot)) * inv2,
            dot2: self.dot2.mul_add(r.val, -(self.val * r.dot2)).mul_add(r.val, -(2.0 * self.dot.mul_add(r.val, -(self.val * r.dot)) * r.dot))
                / (r.val * r.val * r.val),
        }
    }
}
impl Neg for ADForward {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            val: -self.val,
            dot: -self.dot,
            dot2: -self.dot2,
        }
    }
}

impl Add<f64> for ADForward {
    type Output = Self;
    #[inline]
    fn add(self, c: f64) -> Self {
        Self {
            val: self.val + c,
            dot: self.dot,
            dot2: self.dot2,
        }
    }
}
impl Add<ADForward> for f64 {
    type Output = ADForward;
    #[inline]
    fn add(self, r: ADForward) -> ADForward {
        r + self
    }
}
impl Sub<f64> for ADForward {
    type Output = Self;
    #[inline]
    fn sub(self, c: f64) -> Self {
        Self {
            val: self.val - c,
            dot: self.dot,
            dot2: self.dot2,
        }
    }
}
impl Sub<ADForward> for f64 {
    type Output = ADForward;
    #[inline]
    fn sub(self, r: ADForward) -> ADForward {
        ADForward {
            val: self - r.val,
            dot: -r.dot,
            dot2: -r.dot2,
        }
    }
}
impl Mul<f64> for ADForward {
    type Output = Self;
    #[inline]
    fn mul(self, c: f64) -> Self {
        Self {
            val: self.val * c,
            dot: self.dot * c,
            dot2: self.dot2 * c,
        }
    }
}
impl Mul<ADForward> for f64 {
    type Output = ADForward;
    #[inline]
    fn mul(self, r: ADForward) -> ADForward {
        r * self
    }
}
impl Div<f64> for ADForward {
    type Output = Self;
    #[inline]
    fn div(self, c: f64) -> Self {
        let inv = 1.0 / c;
        Self {
            val: self.val * inv,
            dot: self.dot * inv,
            dot2: self.dot2 * inv,
        }
    }
}
impl Div<ADForward> for f64 {
    type Output = ADForward;
    #[inline]
    fn div(self, r: ADForward) -> ADForward {
        ADForward::constant(self) / r
    }
}

impl AddAssign for ADForward {
    fn add_assign(&mut self, r: Self) {
        *self = *self + r;
    }
}
impl AddAssign<f64> for ADForward {
    fn add_assign(&mut self, r: f64) {
        *self = *self + r;
    }
}
impl SubAssign for ADForward {
    fn sub_assign(&mut self, r: Self) {
        *self = *self - r;
    }
}
impl SubAssign<f64> for ADForward {
    fn sub_assign(&mut self, r: f64) {
        *self = *self - r;
    }
}
impl MulAssign for ADForward {
    fn mul_assign(&mut self, r: Self) {
        *self = *self * r;
    }
}
impl MulAssign<f64> for ADForward {
    fn mul_assign(&mut self, r: f64) {
        *self = *self * r;
    }
}
impl DivAssign for ADForward {
    fn div_assign(&mut self, r: Self) {
        *self = *self / r;
    }
}
impl DivAssign<f64> for ADForward {
    fn div_assign(&mut self, r: f64) {
        *self = *self / r;
    }
}
impl Rem for ADForward {
    type Output = Self;
    fn rem(self, r: Self) -> Self {
        Self::constant(self.val % r.val)
    }
}
impl Rem<f64> for ADForward {
    type Output = Self;
    fn rem(self, r: f64) -> Self {
        Self::constant(self.val % r)
    }
}

// -- Scalar impl for ADForward -----------------------------------------------

impl Scalar for ADForward {
    #[inline]
    fn scalar(v: f64) -> Self {
        Self::constant(v)
    }
    #[inline]
    fn value(&self) -> f64 {
        self.val
    }
    #[inline]
    fn zero() -> Self {
        Self::constant(0.0)
    }
    #[inline]
    fn one() -> Self {
        Self::constant(1.0)
    }
    fn exp(self) -> Self {
        let ev = self.val.exp();
        Self {
            val: ev,
            dot: ev * self.dot,
            dot2: ev * self.dot.mul_add(self.dot, self.dot2),
        }
    }
    fn ln(self) -> Self {
        let inv = 1.0 / self.val;
        Self {
            val: self.val.ln(),
            dot: self.dot * inv,
            dot2: self.dot2.mul_add(inv, -(self.dot * self.dot * inv * inv)),
        }
    }
    fn sqrt(self) -> Self {
        let sv = self.val.sqrt();
        let inv2s = 0.5 / sv;
        Self {
            val: sv,
            dot: self.dot * inv2s,
            dot2: self.dot2.mul_add(inv2s, -(self.dot * self.dot / (4.0 * self.val * sv))),
        }
    }
    fn sin(self) -> Self {
        let (s, c) = self.val.sin_cos();
        Self {
            val: s,
            dot: c * self.dot,
            dot2: c.mul_add(self.dot2, -(s * self.dot * self.dot)),
        }
    }
    fn cos(self) -> Self {
        let (s, c) = self.val.sin_cos();
        Self {
            val: c,
            dot: -s * self.dot,
            dot2: (-s).mul_add(self.dot2, -(c * self.dot * self.dot)),
        }
    }
    fn abs(self) -> Self {
        let sgn = if self.val >= 0.0 { 1.0 } else { -1.0 };
        Self {
            val: self.val.abs(),
            dot: sgn * self.dot,
            dot2: sgn * self.dot2,
        }
    }
    fn powf(self, p: f64) -> Self {
        let vp1 = self.val.powf(p - 1.0);
        let vp = vp1 * self.val;
        Self {
            val: vp,
            dot: p * vp1 * self.dot,
            dot2: (p * (p - 1.0) * self.val.powf(p - 2.0) * self.dot).mul_add(self.dot, p * vp1 * self.dot2),
        }
    }
    #[allow(clippy::suspicious_operation_groupings)]
    fn pows(self, b: Self) -> Self {
        let lna = self.val.ln();
        let y = self.val.powf(b.val);
        let u = b.dot.mul_add(lna, b.val * self.dot / self.val);
        let dot = y * u;
        let up = b.val.mul_add(self.dot2 / self.val - self.dot * self.dot / (self.val * self.val), b.dot2.mul_add(lna, 2.0 * b.dot * self.dot / self.val));
        Self {
            val: y,
            dot,
            dot2: dot.mul_add(u, y * up),
        }
    }
    fn max_val(self, o: Self) -> Self {
        if self.val >= o.val {
            self
        } else {
            o
        }
    }
    fn min_val(self, o: Self) -> Self {
        if self.val <= o.val {
            self
        } else {
            o
        }
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

impl InnerScalar for ADForward {}

impl num_traits::Zero for ADForward {
    fn zero() -> Self {
        Self::constant(0.0)
    }
    fn is_zero(&self) -> bool {
        self.val == 0.0
    }
}
impl num_traits::One for ADForward {
    fn one() -> Self {
        Self::constant(1.0)
    }
}
impl num_traits::Num for ADForward {
    type FromStrRadixErr = String;
    fn from_str_radix(s: &str, _: u32) -> std::result::Result<Self, String> {
        s.parse::<f64>()
            .map(Self::constant)
            .map_err(|e| e.to_string())
    }
}

// -- TapeHolder for ADForward ------------------------------------------------

thread_local! {
    /// Thread-local tape for [`Dual<ADForward>`](super::dual::Dual).
    pub static TAPE_FWD: RefCell<Tape<ADForward>> = RefCell::new(Tape {
        bump: Bump::new(), book: Vec::new(), mark: 0, active: false,
    });
}

impl TapeHolder for ADForward {
    fn with_tape<R>(f: impl FnOnce(&mut Tape<Self>) -> R) -> R {
        TAPE_FWD.with(|tc| {
            let mut t = tc.borrow_mut();
            f(&mut t)
        })
    }
}

/// Static convenience methods for the [`ADForward`] tape.
impl Tape<ADForward> {
    /// Clears the tape and begins recording.
    pub fn start_recording_fwd() {
        TAPE_FWD.with(|tc| tc.borrow_mut().start_inner());
    }
    /// Stops recording.
    pub fn stop_recording_fwd() {
        TAPE_FWD.with(|tc| tc.borrow_mut().active = false);
    }
    /// Resets all adjoints on the [`ADForward`] tape to zero.
    pub fn reset_adjoints_fwd() {
        TAPE_FWD.with(|tc| tc.borrow().reset_adjoints_inner());
    }
    /// Clears the tape and resets the mark.
    pub fn rewind_to_init_fwd() {
        TAPE_FWD.with(|tc| {
            let mut t = tc.borrow_mut();
            t.bump.reset();
            t.book.clear();
            t.mark = 0;
        });
    }
    /// Sets the current mark to the end of the tape.
    pub fn set_mark_fwd() {
        TAPE_FWD.with(|tc| {
            let len = tc.borrow().book.len();
            tc.borrow_mut().mark = len;
        });
    }
    /// Truncates the tape back to the current mark.
    pub fn rewind_to_mark_fwd() {
        TAPE_FWD.with(|tc| {
            let mark = tc.borrow().mark;
            tc.borrow_mut().book.truncate(mark);
        });
    }
    /// Resets the mark to the beginning of the tape.
    pub fn reset_mark_fwd() {
        TAPE_FWD.with(|tc| {
            tc.borrow_mut().mark = 0;
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-10;
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn forward_x_squared() {
        let x = ADForward::var(3.0);
        let y = x * x;
        assert!(approx(y.val, 9.0));
        assert!(approx(y.dot, 6.0));
        assert!(approx(y.dot2, 2.0));
    }

    #[test]
    fn forward_x_cubed() {
        let x = ADForward::var(2.0);
        let y = x * x * x;
        assert!(approx(y.val, 8.0));
        assert!(approx(y.dot, 12.0));
        assert!(approx(y.dot2, 12.0));
    }

    #[test]
    fn forward_exp() {
        let x = ADForward::var(1.0);
        let y = x.exp();
        let e = 1.0_f64.exp();
        assert!(approx(y.val, e));
        assert!(approx(y.dot, e));
        assert!(approx(y.dot2, e));
    }

    #[test]
    fn forward_ln() {
        let x = ADForward::var(2.0);
        let y = x.ln();
        assert!(approx(y.val, 2.0_f64.ln()));
        assert!(approx(y.dot, 0.5));
        assert!(approx(y.dot2, -0.25));
    }

    #[test]
    fn forward_sin() {
        let x = ADForward::var(1.0);
        let y = x.sin();
        assert!(approx(y.val, 1.0_f64.sin()));
        assert!(approx(y.dot, 1.0_f64.cos()));
        assert!(approx(y.dot2, -1.0_f64.sin()));
    }

    #[test]
    fn complex_ad_forward_basic() {
        use num_complex::Complex;
        let a = Complex::new(ADForward::constant(1.0), ADForward::constant(2.0));
        let b = Complex::new(ADForward::constant(3.0), ADForward::constant(-1.0));
        let c = a * b;
        assert!(approx(c.re.val, 5.0));
        assert!(approx(c.im.val, 5.0));
    }
}
