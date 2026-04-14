//! [`Dual<T>`] — backward-mode AD wrapper generic over inner scalar `T`.

use core::fmt;
use std::cmp::Ordering;
use std::ptr::NonNull;

use crate::ad::constant::Const;
use crate::ad::expr::{
    flatten, AbsOp, BinExpr, CosOp, ExpOp, Expr, LogOp, MaxOp, MinOp, PowOp, SinOp, SqrtOp, UnExpr,
};
use crate::ad::forward::ADForward;
use crate::ad::node::TapeNode;
use crate::ad::scalar::{InnerScalar, Scalar};
use crate::ad::tape::{Tape, TapeHolder};
use crate::utils::errors::{QSError, Result};

// ═══════════════════════════════════════════════════════════════════════════
//  Dual<T>
// ═══════════════════════════════════════════════════════════════════════════

/// A number that participates in reverse-mode automatic differentiation.
///
/// * `Dual<f64>` — first-order backward-mode AD.
/// * `Dual<ADForward>` (aka [`DualFwd`]) — mixed backward (1st) + forward (2nd) order AD.
///
/// Operations return lazy expression types (`BinExpr`, `UnExpr`). Call
/// `.into()` to flatten them into a single tape node.
#[derive(Clone, Copy)]
pub struct Dual<T> {
    pub(crate) val: T,
    pub(crate) node: Option<NonNull<TapeNode<T>>>,
}

unsafe impl<T: Send> Send for Dual<T> {}
unsafe impl<T: Sync> Sync for Dual<T> {}

impl<T> Dual<T> {
    /// Low-level constructor used by [`flatten`](crate::ad::expr::flatten).
    #[inline]
    pub(crate) const fn from_raw(val: T, node: Option<NonNull<TapeNode<T>>>) -> Self {
        Self { val, node }
    }

    /// Returns the inner `T` value (for the expression-template system).
    #[inline]
    pub(crate) const fn val(&self) -> T
    where
        T: Copy,
    {
        self.val
    }

    /// Returns the raw tape-node pointer (for the expression-template system).
    #[inline]
    pub(crate) const fn node_ptr(&self) -> Option<NonNull<TapeNode<T>>> {
        self.node
    }
}

impl<T: Default> Default for Dual<T> {
    fn default() -> Self {
        Self {
            val: T::default(),
            node: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Constructors & accessors
// ═══════════════════════════════════════════════════════════════════════════

impl<T: TapeHolder + InnerScalar> Dual<T> {
    /// Creates a new value, registering a leaf on the active tape.
    #[inline]
    #[must_use] 
    pub fn new(val: f64) -> Self {
        let v = T::scalar(val);
        let node = T::with_tape(super::tape::Tape::new_leaf);
        Self { val: v, node }
    }

    /// Creates a `Dual` from an already-constructed inner `T` value.
    #[inline]
    pub fn new_from_inner(val: T) -> Self {
        let node = T::with_tape(super::tape::Tape::new_leaf);
        Self { val, node }
    }

    /// Creates a constant — no tape node.
    #[inline]
    #[must_use]
    pub const fn constant(val: T) -> Self {
        Self { val, node: None }
    }

    /// Returns the underlying `f64` value.
    #[inline]
    #[must_use]
    pub fn value(&self) -> f64 {
        self.val.value()
    }

    /// Returns the inner `T` value.
    #[inline]
    #[must_use]
    pub const fn inner(&self) -> T {
        self.val
    }

    /// Zero constant (no tape).
    #[inline]
    #[must_use] 
    pub fn zero() -> Self {
        Self::constant(T::zero())
    }

    /// One constant (no tape).
    #[inline]
    #[must_use] 
    pub fn one() -> Self {
        Self::constant(T::one())
    }

    /// Returns the adjoint after a backward pass.
    ///
    /// # Errors
    /// Returns [`QSError::NodeNotIndexedInTapeErr`] if this node is not on the tape.
    #[inline]
    pub fn adjoint(&self) -> Result<T> {
        self.node
            .map(|p| unsafe { p.as_ref().adj })
            .ok_or(QSError::NodeNotIndexedInTapeErr)
    }

    /// Sets the thread-local tape for type `T`.
    pub fn set_tape(t: Tape<T>) {
        T::with_tape(|tape| *tape = t);
    }

    /// Full backward pass from this node to tape start.
    ///
    /// # Errors
    /// Returns [`QSError::NodeNotIndexedInTapeErr`] if this node is not on the tape.
    pub fn backward(&self) -> Result<()> {
        let root = self.node.ok_or(QSError::NodeNotIndexedInTapeErr)?;
        T::with_tape(|tape| {
            tape.mut_node(root)
                .ok_or(QSError::NodeNotIndexedInTapeErr)?
                .adj = T::one();
            tape.propagate_from(root)
        })
    }

    /// Backward pass from current mark to start.
    ///
    /// # Errors
    /// Returns [`QSError::NodeNotIndexedInTapeErr`] if this node is not on the tape.
    pub fn backward_mark_to_start(&self) -> Result<()> {
        let root = self.node.ok_or(QSError::NodeNotIndexedInTapeErr)?;
        T::with_tape(|tape| {
            tape.mut_node(root)
                .ok_or(QSError::NodeNotIndexedInTapeErr)?
                .adj = T::one();
            tape.propagate_mark_to_start()
        })
    }

    /// Backward pass from tape end to current mark.
    ///
    /// # Errors
    /// Returns [`QSError::NodeNotIndexedInTapeErr`] if this node is not on the tape.
    pub fn backward_to_mark(&self) -> Result<()> {
        let root = self.node.ok_or(QSError::NodeNotIndexedInTapeErr)?;
        T::with_tape(|tape| {
            tape.mut_node(root)
                .ok_or(QSError::NodeNotIndexedInTapeErr)?
                .adj = T::one();
            tape.propagate_to_mark()
        })
    }

    /// Attaches this value to the current tape.
    pub fn put_on_tape(&mut self) {
        T::with_tape(|tape| {
            self.node = tape.new_leaf();
        });
    }

    /// Returns a copy ensured to be on the tape.
    #[must_use]
    pub fn ensure_on_tape(&self) -> Self {
        if self.node.is_some() {
            return *self;
        }
        let mut r = *self;
        r.put_on_tape();
        r
    }

    /// Check if recorded on a tape.
    #[must_use]
    pub const fn is_on_tape(&self) -> bool {
        self.node.is_some()
    }

    // -- Inherent transcendentals (eagerly flattened, for method syntax) -----

    /// Exponential.
    #[inline]
    #[must_use]
    pub fn exp(self) -> Self {
        flatten(&UnExpr::<T, Self, ExpOp>::new(self))
    }
    /// Natural logarithm.
    #[inline]
    #[must_use]
    pub fn ln(self) -> Self {
        flatten(&UnExpr::<T, Self, LogOp>::new(self))
    }
    /// Square root.
    #[inline]
    #[must_use]
    pub fn sqrt(self) -> Self {
        flatten(&UnExpr::<T, Self, SqrtOp>::new(self))
    }
    /// Sine.
    #[inline]
    #[must_use]
    pub fn sin(self) -> Self {
        flatten(&UnExpr::<T, Self, SinOp>::new(self))
    }
    /// Cosine.
    #[inline]
    #[must_use]
    pub fn cos(self) -> Self {
        flatten(&UnExpr::<T, Self, CosOp>::new(self))
    }
    /// Absolute value.
    #[inline]
    #[must_use]
    pub fn abs(self) -> Self {
        flatten(&UnExpr::<T, Self, AbsOp>::new(self))
    }
    /// Raise to a constant `f64` power.
    #[inline]
    #[must_use]
    pub fn powf(self, p: f64) -> Self {
        flatten(&BinExpr::<T, Self, Const<T>, PowOp>::new(
            self,
            Const(T::scalar(p)),
        ))
    }
    /// Component-wise maximum.
    #[inline]
    #[must_use]
    pub fn max<R: Expr<T>>(self, r: R) -> Self {
        flatten(&BinExpr::<T, Self, R, MaxOp>::new(self, r))
    }
    /// Component-wise minimum.
    #[inline]
    #[must_use]
    pub fn min<R: Expr<T>>(self, r: R) -> Self {
        flatten(&BinExpr::<T, Self, R, MinOp>::new(self, r))
    }
    /// Raise to a `Self`-typed exponent.
    #[inline]
    #[must_use]
    pub fn pow_expr<R: Expr<T>>(self, p: R) -> Self {
        flatten(&BinExpr::<T, Self, R, PowOp>::new(self, p))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Scalar impl for Dual<T>
// ═══════════════════════════════════════════════════════════════════════════

impl<T: TapeHolder + InnerScalar> Scalar for Dual<T> {
    #[inline]
    fn scalar(v: f64) -> Self {
        Self::new(v)
    }
    #[inline]
    fn value(&self) -> f64 {
        self.val.value()
    }
    #[inline]
    fn zero() -> Self {
        Self::constant(T::zero())
    }
    #[inline]
    fn one() -> Self {
        Self::constant(T::one())
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
        Self::pow_expr(self, p)
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
        let mut r = self;
        r += other;
        r
    }
    #[inline]
    fn sub_val(self, other: Self) -> Self {
        let mut r = self;
        r -= other;
        r
    }
    #[inline]
    fn mul_val(self, other: Self) -> Self {
        let mut r = self;
        r *= other;
        r
    }
    #[inline]
    fn div_val(self, other: Self) -> Self {
        let mut r = self;
        r /= other;
        r
    }
    #[inline]
    fn neg_val(self) -> Self {
        let mut r = Self::zero();
        r -= self;
        r
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Display / Debug
// ═══════════════════════════════════════════════════════════════════════════

impl<T: fmt::Debug> fmt::Debug for Dual<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dual({:?}, Node: {:?})", self.val, self.node)
    }
}
impl<T: fmt::Display> fmt::Display for Dual<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dual({})", self.val)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Comparison (value-based)
// ═══════════════════════════════════════════════════════════════════════════

impl<T: TapeHolder + InnerScalar> PartialEq for Dual<T> {
    fn eq(&self, o: &Self) -> bool {
        self.val.value() == o.val.value()
    }
}
impl<T: TapeHolder + InnerScalar> PartialOrd for Dual<T> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        self.val.value().partial_cmp(&o.val.value())
    }
}
impl<T: TapeHolder + InnerScalar> PartialEq<f64> for Dual<T> {
    fn eq(&self, rhs: &f64) -> bool {
        self.val.value() == *rhs
    }
}
impl<T: TapeHolder + InnerScalar> PartialOrd<f64> for Dual<T> {
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.val.value().partial_cmp(rhs)
    }
}

impl<T: TapeHolder + InnerScalar> From<Dual<T>> for f64 {
    #[inline]
    fn from(d: Dual<T>) -> Self {
        d.val.value()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Type alias
// ═══════════════════════════════════════════════════════════════════════════

/// Mixed-mode AD number: backward (1st order) + forward (2nd order).
pub type DualFwd = Dual<ADForward>;

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::constant::Const;
    use crate::ad::expr::*;
    use crate::ad::tape::Tape;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_tape_test<F: FnOnce()>(f: F) {
        let _g = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        f();
        Tape::stop_recording_fwd();
    }

    const EPS: f64 = 1e-10;
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn compare_and_flatten() {
        with_tape_test(|| {
            let x = DualFwd::new(5.0);
            let y = abs(x - 2.0);
            assert!(y > 2.0);
            let z: DualFwd = (y + 1.0).into();
            assert_eq!(z.value(), 4.0);
        });
    }

    #[test]
    fn backprop_basic() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let b = DualFwd::new(4.0);
            let out: DualFwd = (a * b).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn test_late_tape_recording() {
        with_tape_test(|| {
            let mut a = DualFwd::new(3.0);
            Tape::start_recording_fwd();
            a.put_on_tape();
            let expr = a * a;
            let out: DualFwd = expr.into();
            out.backward().unwrap();
            assert_eq!(a.adjoint().unwrap().value(), 6.0);
        });
    }

    #[test]
    fn backprop_with_const() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let out: DualFwd = (a * 4.0).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn tape_reset() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let b = DualFwd::new(4.0);
            let out: DualFwd = (a * b).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
            Tape::reset_adjoints_fwd();
            assert_eq!(out.adjoint().unwrap().value(), 0.0);
        });
    }

    #[test]
    fn check_exp_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = exp(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), f64::exp(2.0)));
        });
    }

    #[test]
    fn check_log_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = log(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.5));
        });
    }

    #[test]
    fn check_sqrt_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(4.0);
            let out: DualFwd = sqrt(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.25));
        });
    }

    #[test]
    fn check_sin_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(0.0);
            let out: DualFwd = sin(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 1.0));
        });
    }

    #[test]
    fn check_cos_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(0.0);
            let out: DualFwd = cos(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.0));
        });
    }

    #[test]
    fn check_pow_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = x.pow_expr(Const::<ADForward>::scalar(3.0)).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 12.0)); // 3x^2 at x=2
        });
    }

    #[test]
    fn check_add_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = (x + y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 1.0);
            assert_eq!(y.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn check_mul_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(4.0);
            let y = DualFwd::new(2.0);
            let out: DualFwd = (x * y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 2.0);
            assert_eq!(y.adjoint().unwrap().value(), 4.0);
        });
    }

    #[test]
    fn check_div_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(6.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = (x / y).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 1.0 / 3.0));
            assert!(approx(y.adjoint().unwrap().value(), -6.0 / 9.0));
        });
    }

    #[test]
    fn check_max_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = max(x, y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 0.0);
            assert_eq!(y.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn test_reassigning() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a0 = DualFwd::new(5.0);
            let b = DualFwd::new(3.0);
            let mut a = a0;
            a *= b;
            let c = a;
            assert_eq!(c.value(), 15.0);
            c.backward().unwrap();
            assert_eq!(a0.adjoint().unwrap().value(), 3.0);
            assert_eq!(b.adjoint().unwrap().value(), 5.0);
        });
    }

    #[test]
    fn multithread_recording() {
        with_tape_test(|| {
            let handle = std::thread::spawn(|| {
                Tape::start_recording_fwd();
                let x = DualFwd::new(2.0);
                let y = DualFwd::new(3.0);
                let out: DualFwd = (x * y + x).into();
                out.backward().unwrap();
                (
                    x.adjoint().unwrap().value(),
                    y.adjoint().unwrap().value(),
                    out.adjoint().unwrap().value(),
                )
            });
            let (dx, dy, dout) = handle.join().unwrap();
            assert_eq!(dx, 4.0);
            assert_eq!(dy, 2.0);
            assert_eq!(dout, 1.0);
        });
    }

    #[test]
    fn mixed_x_squared() {
        Tape::start_recording_fwd();
        let x_inner = ADForward::var(3.0);
        let x = Dual::<ADForward>::new_from_inner(x_inner);
        let y = x * x;
        let out: Dual<ADForward> = y.into();
        out.backward().unwrap();
        let adj = x.adjoint().unwrap();
        assert!(approx(adj.val, 6.0));
        assert!(approx(adj.dot, 2.0));
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
    }

    #[test]
    fn mixed_exp() {
        Tape::start_recording_fwd();
        let x_inner = ADForward::var(1.0);
        let x = Dual::<ADForward>::new_from_inner(x_inner);
        let y: Dual<ADForward> = x.exp().into();
        y.backward().unwrap();
        let adj = x.adjoint().unwrap();
        let e = 1.0_f64.exp();
        assert!(approx(adj.val, e));
        assert!(approx(adj.dot, e));
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
    }
}
