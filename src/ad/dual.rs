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
    pub(crate) fn from_raw(val: T, node: Option<NonNull<TapeNode<T>>>) -> Self {
        Self { val, node }
    }

    /// Returns the inner `T` value (for the expression-template system).
    #[inline]
    pub(crate) fn val(&self) -> T
    where
        T: Copy,
    {
        self.val
    }

    /// Returns the raw tape-node pointer (for the expression-template system).
    #[inline]
    pub(crate) fn node_ptr(&self) -> Option<NonNull<TapeNode<T>>> {
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
    pub fn new(val: f64) -> Self {
        let v = T::scalar(val);
        let node = T::with_tape(|tape| tape.new_leaf());
        Self { val: v, node }
    }

    /// Creates a `Dual` from an already-constructed inner `T` value.
    #[inline]
    pub fn new_from_inner(val: T) -> Self {
        let node = T::with_tape(|tape| tape.new_leaf());
        Self { val, node }
    }

    /// Creates a constant — no tape node.
    #[inline]
    #[must_use]
    pub fn constant(val: T) -> Self {
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
    pub fn inner(&self) -> T {
        self.val
    }

    /// Zero constant (no tape).
    #[inline]
    pub fn zero() -> Self {
        Self::constant(T::zero())
    }

    /// One constant (no tape).
    #[inline]
    pub fn one() -> Self {
        Self::constant(T::one())
    }

    /// Returns the adjoint after a backward pass.
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
    pub fn exp(self) -> Self {
        flatten(&UnExpr::<T, Self, ExpOp>::new(self))
    }
    /// Natural logarithm.
    #[inline]
    pub fn ln(self) -> Self {
        flatten(&UnExpr::<T, Self, LogOp>::new(self))
    }
    /// Square root.
    #[inline]
    pub fn sqrt(self) -> Self {
        flatten(&UnExpr::<T, Self, SqrtOp>::new(self))
    }
    /// Sine.
    #[inline]
    pub fn sin(self) -> Self {
        flatten(&UnExpr::<T, Self, SinOp>::new(self))
    }
    /// Cosine.
    #[inline]
    pub fn cos(self) -> Self {
        flatten(&UnExpr::<T, Self, CosOp>::new(self))
    }
    /// Absolute value.
    #[inline]
    pub fn abs(self) -> Self {
        flatten(&UnExpr::<T, Self, AbsOp>::new(self))
    }
    /// Raise to a constant `f64` power.
    #[inline]
    pub fn powf(self, p: f64) -> Self {
        flatten(&BinExpr::<T, Self, Const<T>, PowOp>::new(
            self,
            Const(T::scalar(p)),
        ))
    }
    /// Component-wise maximum.
    #[inline]
    pub fn max<R: Expr<T>>(self, r: R) -> Self {
        flatten(&BinExpr::<T, Self, R, MaxOp>::new(self, r))
    }
    /// Component-wise minimum.
    #[inline]
    pub fn min<R: Expr<T>>(self, r: R) -> Self {
        flatten(&BinExpr::<T, Self, R, MinOp>::new(self, r))
    }
    /// Raise to a `Self`-typed exponent.
    #[inline]
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
        Dual::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        Dual::ln(self)
    }
    #[inline]
    fn sqrt(self) -> Self {
        Dual::sqrt(self)
    }
    #[inline]
    fn sin(self) -> Self {
        Dual::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        Dual::cos(self)
    }
    #[inline]
    fn abs(self) -> Self {
        Dual::abs(self)
    }
    #[inline]
    fn powf(self, p: f64) -> Self {
        Dual::powf(self, p)
    }
    #[inline]
    fn pows(self, p: Self) -> Self {
        Dual::pow_expr(self, p)
    }
    #[inline]
    fn max_val(self, o: Self) -> Self {
        Dual::max(self, o)
    }
    #[inline]
    fn min_val(self, o: Self) -> Self {
        Dual::min(self, o)
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

// ═══════════════════════════════════════════════════════════════════════════
//  Type alias
// ═══════════════════════════════════════════════════════════════════════════

/// Mixed-mode AD number: backward (1st order) + forward (2nd order).
pub type DualFwd = Dual<ADForward>;
