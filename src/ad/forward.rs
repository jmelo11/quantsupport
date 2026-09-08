//! Forward-mode automatic differentiation: the generic, nestable [`Fwd`].
//!
//! `Fwd<T>` carries a value and a first-order derivative seed of type `T`.
//! Nesting multiplies the derivative order:
//!
//! * [`Fwd1`] = `Fwd<f64>` — 1st order
//! * [`Fwd2`] = `Fwd<Fwd<f64>>` — 2nd order (alias [`ADForward`])
//! * [`Fwd3`] = `Fwd<Fwd<Fwd<f64>>>` — 3rd order
//! * [`Fwd4`] — 4th order, and so on.
//!
//! Pure forward-mode works at any depth with no tape. To use a forward type
//! as the inner scalar of a [`Dual`](crate::ad::dual::Dual) (mixed
//! backward-over-forward), it needs a thread-local tape: `Fwd1`…`Fwd4` are
//! provided out of the box.

use core::fmt;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, Sub, SubAssign};

use crate::ad::blocklist::BlockList;
use crate::ad::scalar::{InnerScalar, Scalar};
use crate::ad::tape::{Tape, TapeHolder};

/// A forward-mode AD number carrying a value and a derivative seed.
///
/// Generic over the inner scalar `T`, so it nests: `Fwd<Fwd<f64>>` computes
/// second-order derivatives, `Fwd<Fwd<Fwd<f64>>>` third-order, and so on.
#[derive(Clone, Copy, Default)]
pub struct Fwd<T> {
    /// Function value.
    pub val: T,
    /// First-order forward derivative.
    pub dot: T,
}

/// First-order forward AD.
pub type Fwd1 = Fwd<f64>;
/// Second-order forward AD.
pub type Fwd2 = Fwd<Fwd1>;
/// Third-order forward AD.
pub type Fwd3 = Fwd<Fwd2>;
/// Fourth-order forward AD.
pub type Fwd4 = Fwd<Fwd3>;

/// Backward-compatible alias: the 2nd-order forward type used by
/// [`DualFwd`](crate::ad::dual::DualFwd).
pub type ADForward = Fwd2;

/// Scalars that can be seeded as an independent variable at every nesting
/// level (used by [`Fwd::var`]).
pub trait FwdSeed: InnerScalar {
    /// Seeds an independent variable with value `v`.
    fn seed_var(v: f64) -> Self;
}

impl FwdSeed for f64 {
    #[inline]
    fn seed_var(v: f64) -> Self {
        v
    }
}

impl<T: FwdSeed> FwdSeed for Fwd<T> {
    #[inline]
    fn seed_var(v: f64) -> Self {
        Self {
            val: T::seed_var(v),
            dot: T::one(),
        }
    }
}

// ---------------------------------------------------------------------------
// Constructors & accessors
// ---------------------------------------------------------------------------

impl<T: InnerScalar> Fwd<T> {
    /// Constant (no derivative seeds).
    #[inline]
    #[must_use]
    pub fn constant(v: f64) -> Self {
        Self {
            val: T::scalar(v),
            dot: T::zero(),
        }
    }

    /// Returns the function value.
    #[inline]
    #[must_use]
    pub fn value(&self) -> f64 {
        self.val.value()
    }

    /// Returns the first forward derivative.
    #[inline]
    #[must_use]
    pub fn first_derivative(&self) -> f64 {
        self.dot.value()
    }
}

impl<T: FwdSeed> Fwd<T> {
    /// Independent variable seeded for derivative computation at all levels.
    #[inline]
    #[must_use]
    pub fn var(v: f64) -> Self {
        <Self as FwdSeed>::seed_var(v)
    }
}

impl<U: InnerScalar> Fwd<Fwd<U>> {
    /// Returns the second forward derivative (nesting depth ≥ 2).
    #[inline]
    #[must_use]
    pub fn second_derivative(&self) -> f64 {
        self.dot.dot.value()
    }
}

impl<U: InnerScalar> Fwd<Fwd<Fwd<U>>> {
    /// Returns the third forward derivative (nesting depth ≥ 3).
    #[inline]
    #[must_use]
    pub fn third_derivative(&self) -> f64 {
        self.dot.dot.dot.value()
    }
}

// ---------------------------------------------------------------------------
// Comparisons, formatting, conversions
// ---------------------------------------------------------------------------

impl<T: fmt::Debug> fmt::Debug for Fwd<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fwd({:?}, d:{:?})", self.val, self.dot)
    }
}
impl<T: InnerScalar> fmt::Display for Fwd<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}
impl<T: InnerScalar> PartialEq for Fwd<T> {
    fn eq(&self, o: &Self) -> bool {
        self.value() == o.value()
    }
}
impl<T: InnerScalar> PartialOrd for Fwd<T> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        self.value().partial_cmp(&o.value())
    }
}
impl<T: InnerScalar> PartialEq<f64> for Fwd<T> {
    fn eq(&self, rhs: &f64) -> bool {
        self.value() == *rhs
    }
}
impl<T: InnerScalar> PartialOrd<f64> for Fwd<T> {
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.value().partial_cmp(rhs)
    }
}
impl<T: InnerScalar> From<f64> for Fwd<T> {
    fn from(v: f64) -> Self {
        Self::constant(v)
    }
}

// -- Fwd<T> arithmetic --------------------------------------------------------

impl<T: InnerScalar> Add for Fwd<T> {
    type Output = Self;
    #[inline]
    fn add(self, r: Self) -> Self {
        Self {
            val: self.val + r.val,
            dot: self.dot + r.dot,
        }
    }
}
impl<T: InnerScalar> Sub for Fwd<T> {
    type Output = Self;
    #[inline]
    fn sub(self, r: Self) -> Self {
        Self {
            val: self.val - r.val,
            dot: self.dot - r.dot,
        }
    }
}
impl<T: InnerScalar> Mul for Fwd<T> {
    type Output = Self;
    #[inline]
    fn mul(self, r: Self) -> Self {
        Self {
            val: self.val * r.val,
            dot: self.val * r.dot + self.dot * r.val,
        }
    }
}
impl<T: InnerScalar> Div for Fwd<T> {
    type Output = Self;
    #[inline]
    fn div(self, r: Self) -> Self {
        let val = self.val / r.val;
        Self {
            val,
            dot: (self.dot - val * r.dot) / r.val,
        }
    }
}
impl<T: InnerScalar> Neg for Fwd<T> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            val: -self.val,
            dot: -self.dot,
        }
    }
}

impl<T: InnerScalar> Add<f64> for Fwd<T> {
    type Output = Self;
    #[inline]
    fn add(self, c: f64) -> Self {
        Self {
            val: self.val + c,
            dot: self.dot,
        }
    }
}
impl<T: InnerScalar> Add<Fwd<T>> for f64 {
    type Output = Fwd<T>;
    #[inline]
    fn add(self, r: Fwd<T>) -> Fwd<T> {
        r + self
    }
}
impl<T: InnerScalar> Sub<f64> for Fwd<T> {
    type Output = Self;
    #[inline]
    fn sub(self, c: f64) -> Self {
        Self {
            val: self.val - c,
            dot: self.dot,
        }
    }
}
impl<T: InnerScalar> Sub<Fwd<T>> for f64 {
    type Output = Fwd<T>;
    #[inline]
    fn sub(self, r: Fwd<T>) -> Fwd<T> {
        Fwd {
            val: T::scalar(self) - r.val,
            dot: -r.dot,
        }
    }
}
impl<T: InnerScalar> Mul<f64> for Fwd<T> {
    type Output = Self;
    #[inline]
    fn mul(self, c: f64) -> Self {
        Self {
            val: self.val * c,
            dot: self.dot * c,
        }
    }
}
impl<T: InnerScalar> Mul<Fwd<T>> for f64 {
    type Output = Fwd<T>;
    #[inline]
    fn mul(self, r: Fwd<T>) -> Fwd<T> {
        r * self
    }
}
impl<T: InnerScalar> Div<f64> for Fwd<T> {
    type Output = Self;
    #[inline]
    fn div(self, c: f64) -> Self {
        let inv = 1.0 / c;
        Self {
            val: self.val * inv,
            dot: self.dot * inv,
        }
    }
}
impl<T: InnerScalar> Div<Fwd<T>> for f64 {
    type Output = Fwd<T>;
    #[inline]
    fn div(self, r: Fwd<T>) -> Fwd<T> {
        Fwd::<T>::constant(self) / r
    }
}

impl<T: InnerScalar> AddAssign for Fwd<T> {
    fn add_assign(&mut self, r: Self) {
        *self = *self + r;
    }
}
impl<T: InnerScalar> AddAssign<f64> for Fwd<T> {
    fn add_assign(&mut self, r: f64) {
        *self = *self + r;
    }
}
impl<T: InnerScalar> SubAssign for Fwd<T> {
    fn sub_assign(&mut self, r: Self) {
        *self = *self - r;
    }
}
impl<T: InnerScalar> SubAssign<f64> for Fwd<T> {
    fn sub_assign(&mut self, r: f64) {
        *self = *self - r;
    }
}
impl<T: InnerScalar> MulAssign for Fwd<T> {
    fn mul_assign(&mut self, r: Self) {
        *self = *self * r;
    }
}
impl<T: InnerScalar> MulAssign<f64> for Fwd<T> {
    fn mul_assign(&mut self, r: f64) {
        *self = *self * r;
    }
}
impl<T: InnerScalar> DivAssign for Fwd<T> {
    fn div_assign(&mut self, r: Self) {
        *self = *self / r;
    }
}
impl<T: InnerScalar> DivAssign<f64> for Fwd<T> {
    fn div_assign(&mut self, r: f64) {
        *self = *self / r;
    }
}
impl<T: InnerScalar> Rem for Fwd<T> {
    type Output = Self;
    fn rem(self, r: Self) -> Self {
        Self::constant(self.value() % r.value())
    }
}
impl<T: InnerScalar> Rem<f64> for Fwd<T> {
    type Output = Self;
    fn rem(self, r: f64) -> Self {
        Self::constant(self.value() % r)
    }
}

// -- Scalar impl for Fwd<T> ---------------------------------------------------
//
// Chain rules are written first-order in the inner scalar `T`; nesting the
// type recursively yields exact higher-order derivatives.

impl<T: InnerScalar> Scalar for Fwd<T> {
    #[inline]
    fn scalar(v: f64) -> Self {
        Self::constant(v)
    }
    #[inline]
    fn value(&self) -> f64 {
        self.val.value()
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
        let e = self.val.exp();
        Self {
            val: e,
            dot: self.dot * e,
        }
    }
    fn ln(self) -> Self {
        Self {
            val: self.val.ln(),
            dot: self.dot / self.val,
        }
    }
    fn sqrt(self) -> Self {
        let s = self.val.sqrt();
        Self {
            val: s,
            dot: self.dot / (s * 2.0),
        }
    }
    fn sin(self) -> Self {
        Self {
            val: self.val.sin(),
            dot: self.dot * self.val.cos(),
        }
    }
    fn cos(self) -> Self {
        Self {
            val: self.val.cos(),
            dot: -(self.dot * self.val.sin()),
        }
    }
    fn abs(self) -> Self {
        if self.value() >= 0.0 {
            self
        } else {
            -self
        }
    }
    fn powf(self, p: f64) -> Self {
        Self {
            val: self.val.powf(p),
            dot: self.dot * self.val.powf(p - 1.0) * p,
        }
    }
    fn pows(self, b: Self) -> Self {
        // a^b = exp(b · ln a), valid for a > 0 (same domain as before).
        Scalar::exp(b * Scalar::ln(self))
    }
    fn max_val(self, o: Self) -> Self {
        if self.value() >= o.value() {
            self
        } else {
            o
        }
    }
    fn min_val(self, o: Self) -> Self {
        if self.value() <= o.value() {
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

impl<T: InnerScalar> InnerScalar for Fwd<T> {}

impl<T: InnerScalar> num_traits::Zero for Fwd<T> {
    fn zero() -> Self {
        Self::constant(0.0)
    }
    fn is_zero(&self) -> bool {
        self.value() == 0.0
    }
}
impl<T: InnerScalar> num_traits::One for Fwd<T> {
    fn one() -> Self {
        Self::constant(1.0)
    }
}
impl<T: InnerScalar> num_traits::Num for Fwd<T> {
    type FromStrRadixErr = String;
    fn from_str_radix(s: &str, _: u32) -> std::result::Result<Self, String> {
        s.parse::<f64>()
            .map(Self::constant)
            .map_err(|e| e.to_string())
    }
}

// -- TapeHolder for forward types ----------------------------------------------

/// Implements [`TapeHolder`] for a concrete forward type with its own
/// thread-local tape, enabling `Dual<$ty>` (backward-over-forward AD).
macro_rules! impl_fwd_tape_holder {
    ($ty:ty, $tls:ident, $doc:literal) => {
        thread_local! {
            #[doc = $doc]
            pub static $tls: RefCell<Tape<$ty>> = RefCell::new(Tape {
                storage: BlockList::with_default_cap(), book: Vec::new(), mark: 0, active: false,
            });
        }

        impl TapeHolder for $ty {
            fn with_tape<R>(f: impl FnOnce(&mut Tape<Self>) -> R) -> R {
                $tls.with(|tc| {
                    let mut t = tc.borrow_mut();
                    f(&mut t)
                })
            }
        }
    };
}

impl_fwd_tape_holder!(
    Fwd1,
    TAPE_FWD1,
    "Thread-local tape for `Dual<Fwd1>` (1st-order forward inner scalar)."
);
impl_fwd_tape_holder!(
    Fwd2,
    TAPE_FWD,
    "Thread-local tape for `Dual<ADForward>`/`DualFwd` (2nd-order forward inner scalar)."
);
impl_fwd_tape_holder!(
    Fwd3,
    TAPE_FWD3,
    "Thread-local tape for `Dual<Fwd3>` (3rd-order forward inner scalar)."
);
impl_fwd_tape_holder!(
    Fwd4,
    TAPE_FWD4,
    "Thread-local tape for `Dual<Fwd4>` (4th-order forward inner scalar)."
);

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
            t.storage.reset();
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
    /// Truncates the tape back to the current mark, dropping post-mark nodes.
    pub fn rewind_to_mark_fwd() {
        TAPE_FWD.with(|tc| {
            let mut t = tc.borrow_mut();
            let mark = t.mark;
            t.book.truncate(mark);
            t.storage.rewind_to(mark);
        });
    }
    /// Resets the mark to the beginning of the tape.
    pub fn reset_mark_fwd() {
        TAPE_FWD.with(|tc| {
            tc.borrow_mut().mark = 0;
        });
    }
    /// Propagates accumulated adjoints from the mark backward to the start
    /// of the [`ADForward`] tape.
    ///
    /// # Errors
    /// Returns an error if adjoint propagation fails.
    pub fn propagate_mark_to_start_fwd() -> crate::utils::errors::Result<()> {
        TAPE_FWD.with(|tc| tc.borrow_mut().propagate_mark_to_start())
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
        assert!(approx(y.value(), 9.0));
        assert!(approx(y.first_derivative(), 6.0));
        assert!(approx(y.second_derivative(), 2.0));
    }

    #[test]
    fn forward_x_cubed() {
        let x = ADForward::var(2.0);
        let y = x * x * x;
        assert!(approx(y.value(), 8.0));
        assert!(approx(y.first_derivative(), 12.0));
        assert!(approx(y.second_derivative(), 12.0));
    }

    #[test]
    fn forward_exp() {
        let x = ADForward::var(1.0);
        let y = x.exp();
        let e = 1.0_f64.exp();
        assert!(approx(y.value(), e));
        assert!(approx(y.first_derivative(), e));
        assert!(approx(y.second_derivative(), e));
    }

    #[test]
    fn forward_ln() {
        let x = ADForward::var(2.0);
        let y = x.ln();
        assert!(approx(y.value(), 2.0_f64.ln()));
        assert!(approx(y.first_derivative(), 0.5));
        assert!(approx(y.second_derivative(), -0.25));
    }

    #[test]
    fn forward_sin() {
        let x = ADForward::var(1.0);
        let y = x.sin();
        assert!(approx(y.value(), 1.0_f64.sin()));
        assert!(approx(y.first_derivative(), 1.0_f64.cos()));
        assert!(approx(y.second_derivative(), -1.0_f64.sin()));
    }

    #[test]
    fn forward_div_second_order() {
        // f(x) = 1/x: f'(x) = -1/x^2, f''(x) = 2/x^3
        let x = ADForward::var(2.0);
        let y = ADForward::constant(1.0) / x;
        assert!(approx(y.value(), 0.5));
        assert!(approx(y.first_derivative(), -0.25));
        assert!(approx(y.second_derivative(), 0.25));
    }

    #[test]
    fn forward_third_order() {
        // f(x) = x^4: f'''(x) = 24x; at x=2 -> 48
        let x = Fwd3::var(2.0);
        let y = x * x * x * x;
        assert!(approx(y.value(), 16.0));
        assert!(approx(y.first_derivative(), 32.0));
        assert!(approx(y.second_derivative(), 48.0));
        assert!(approx(y.third_derivative(), 48.0));
    }

    #[test]
    fn forward_third_order_exp() {
        // All derivatives of e^x equal e^x.
        let x = Fwd3::var(1.5);
        let y = x.exp();
        let e = 1.5_f64.exp();
        assert!(approx(y.value(), e));
        assert!(approx(y.first_derivative(), e));
        assert!(approx(y.second_derivative(), e));
        assert!(approx(y.third_derivative(), e));
    }

    #[test]
    fn forward_fourth_order() {
        // f(x) = x^4: f''''(x) = 24 everywhere.
        let x = Fwd4::var(2.0);
        let y = x * x * x * x;
        assert!(approx(y.dot.dot.dot.dot.value(), 24.0));
    }

    #[test]
    fn complex_ad_forward_basic() {
        use num_complex::Complex;
        let a = Complex::new(ADForward::constant(1.0), ADForward::constant(2.0));
        let b = Complex::new(ADForward::constant(3.0), ADForward::constant(-1.0));
        let c = a * b;
        assert!(approx(c.re.value(), 5.0));
        assert!(approx(c.im.value(), 5.0));
    }
}
