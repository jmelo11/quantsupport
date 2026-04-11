//! Expression-template system for automatic differentiation.
//!
//! Provides [`Expr`], [`BinOp`], [`UnOp`], operators,
//! [`BinExpr`], [`UnExpr`], [`FloatExt`], and free-standing transcendentals.

use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::ad::node::TapeNode;
use crate::ad::scalar::{InnerScalar, Scalar};
use crate::ad::tape::TapeHolder;

// Re-import sibling types so that the sealed impls and operator macros can
// refer to them without consumers having to spell a longer path.
use super::constant::Const;
use super::dual::Dual;

// ═══════════════════════════════════════════════════════════════════════════
//  Sealed trait — prevents external crates from implementing Expr
// ═══════════════════════════════════════════════════════════════════════════

mod sealed {
    pub trait Sealed {}
    impl<T> Sealed for super::Dual<T> {}
    impl<T> Sealed for super::Const<T> {}
    impl<T, L, R, O> Sealed for super::BinExpr<T, L, R, O> {}
    impl<T, A, O> Sealed for super::UnExpr<T, A, O> {}
}

// ═══════════════════════════════════════════════════════════════════════════
//  Expr, BinOp, UnOp traits
// ═══════════════════════════════════════════════════════════════════════════

/// A differentiable expression node in the expression-template tree.
///
/// Every arithmetic operation on [`Dual<T>`] values returns a lazy
/// expression type ([`BinExpr`] or [`UnExpr`]) instead of immediately
/// recording on the tape. Call [`.into()`](Into::into) on the final
/// expression to flatten the entire tree into a single tape node,
/// producing a `Dual<T>` again.
///
/// This trait is **sealed** — only types defined in this module
/// ([`Dual`], [`Const`], [`BinExpr`], [`UnExpr`]) implement it.
pub trait Expr<T>: sealed::Sealed + Clone {
    /// Returns the scalar value of the expression.
    fn inner_value(&self) -> T;
    /// Recursively pushes adjoint (derivative) contributions into a tape node.
    fn push_adj(&self, parent: &mut TapeNode<T>, adj: T);
}

/// Defines a binary operation for the expression-template system.
pub trait BinOp<T> {
    /// Evaluates the operator on the two operand values.
    fn eval(l: T, r: T) -> T;
    /// Partial derivative with respect to the left operand.
    fn d_left(l: T, r: T) -> T;
    /// Partial derivative with respect to the right operand.
    fn d_right(l: T, r: T) -> T;
}

/// Defines a unary operation for the expression-template system.
pub trait UnOp<T> {
    /// Evaluates the operator on the input value.
    fn eval(x: T) -> T;
    /// Derivative of the operation. `x` is the original input, `v` is `eval(x)`.
    fn deriv(x: T, v: T) -> T;
}

// ═══════════════════════════════════════════════════════════════════════════
//  Binary op structs
// ═══════════════════════════════════════════════════════════════════════════

/// Binary addition operator.
#[derive(Clone, Copy, Debug)]
pub struct AddOp;
impl<T: InnerScalar> BinOp<T> for AddOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l + r
    }
    #[inline]
    fn d_left(_l: T, _r: T) -> T {
        T::one()
    }
    #[inline]
    fn d_right(_l: T, _r: T) -> T {
        T::one()
    }
}

/// Binary subtraction operator.
#[derive(Clone, Copy, Debug)]
pub struct SubOp;
impl<T: InnerScalar> BinOp<T> for SubOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l - r
    }
    #[inline]
    fn d_left(_l: T, _r: T) -> T {
        T::one()
    }
    #[inline]
    fn d_right(_l: T, _r: T) -> T {
        T::zero() - T::one()
    }
}

/// Binary multiplication operator.
#[derive(Clone, Copy, Debug)]
pub struct MulOp;
impl<T: InnerScalar> BinOp<T> for MulOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l * r
    }
    #[inline]
    fn d_left(_l: T, r: T) -> T {
        r
    }
    #[inline]
    fn d_right(l: T, _r: T) -> T {
        l
    }
}

/// Binary division operator.
#[derive(Clone, Copy, Debug)]
pub struct DivOp;
impl<T: InnerScalar> BinOp<T> for DivOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l / r
    }
    #[inline]
    fn d_left(_l: T, r: T) -> T {
        T::one() / r
    }
    #[inline]
    fn d_right(l: T, r: T) -> T {
        T::zero() - l / (r * r)
    }
}

/// Binary power operator.
#[derive(Clone, Copy, Debug)]
pub struct PowOp;
impl<T: InnerScalar> BinOp<T> for PowOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l.pows(r)
    }
    #[inline]
    fn d_left(l: T, r: T) -> T {
        r * l.pows(r - T::one())
    }
    #[inline]
    fn d_right(l: T, r: T) -> T {
        l.pows(r) * l.ln()
    }
}

/// Binary maximum operator.
#[derive(Clone, Copy, Debug)]
pub struct MaxOp;
impl<T: InnerScalar> BinOp<T> for MaxOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l.max_val(r)
    }
    #[inline]
    fn d_left(l: T, r: T) -> T {
        if l.value() > r.value() {
            T::one()
        } else {
            T::zero()
        }
    }
    #[inline]
    fn d_right(l: T, r: T) -> T {
        if r.value() > l.value() {
            T::one()
        } else {
            T::zero()
        }
    }
}

/// Binary minimum operator.
#[derive(Clone, Copy, Debug)]
pub struct MinOp;
impl<T: InnerScalar> BinOp<T> for MinOp {
    #[inline]
    fn eval(l: T, r: T) -> T {
        l.min_val(r)
    }
    #[inline]
    fn d_left(l: T, r: T) -> T {
        if l.value() < r.value() {
            T::one()
        } else {
            T::zero()
        }
    }
    #[inline]
    fn d_right(l: T, r: T) -> T {
        if r.value() < l.value() {
            T::one()
        } else {
            T::zero()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Unary op structs (macro-generated)
// ═══════════════════════════════════════════════════════════════════════════

macro_rules! un_op {
    ($name:ident, $doc:expr, $eval:expr, $d:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        impl<T: InnerScalar> UnOp<T> for $name {
            #[inline]
            fn eval(x: T) -> T {
                $eval(x)
            }
            #[inline]
            fn deriv(x: T, v: T) -> T {
                $d(x, v)
            }
        }
    };
}

un_op!(
    ExpOp,
    "Unary exponential operator.",
    Scalar::exp,
    |_x: T, v: T| v
);
un_op!(
    LogOp,
    "Unary natural logarithm operator.",
    Scalar::ln,
    |x: T, _v: T| T::one() / x
);
un_op!(
    SqrtOp,
    "Unary square root operator.",
    Scalar::sqrt,
    |_x: T, v: T| T::scalar(0.5) / v
);
un_op!(
    FabsOp,
    "Unary absolute value operator (alias).",
    Scalar::abs,
    |x: T, _v: T| if x.value() >= 0.0 {
        T::one()
    } else {
        T::zero() - T::one()
    }
);
un_op!(SinOp, "Unary sine operator.", Scalar::sin, |x: T, _v: T| x
    .cos());
un_op!(
    CosOp,
    "Unary cosine operator.",
    Scalar::cos,
    |x: T, _v: T| T::zero() - x.sin()
);
un_op!(
    AbsOp,
    "Unary absolute value operator.",
    Scalar::abs,
    |x: T, _v: T| if x.value() >= 0.0 {
        T::one()
    } else {
        T::zero() - T::one()
    }
);

// ═══════════════════════════════════════════════════════════════════════════
//  BinExpr
// ═══════════════════════════════════════════════════════════════════════════

/// A lazy binary expression over two child expressions.
#[derive(Clone, Copy)]
pub struct BinExpr<T, L, R, O> {
    l: L,
    r: R,
    val: T,
    _ph: std::marker::PhantomData<O>,
}

impl<T: InnerScalar, L: Expr<T>, R: Expr<T>, O: BinOp<T>> BinExpr<T, L, R, O> {
    #[inline]
    pub(crate) fn new(l: L, r: R) -> Self {
        let val = O::eval(l.inner_value(), r.inner_value());
        Self {
            l,
            r,
            val,
            _ph: std::marker::PhantomData,
        }
    }
}

impl<T: InnerScalar, L: Expr<T>, R: Expr<T>, O: BinOp<T> + Clone> Expr<T> for BinExpr<T, L, R, O> {
    #[inline]
    fn inner_value(&self) -> T {
        self.val
    }
    fn push_adj(&self, parent: &mut TapeNode<T>, adj: T) {
        let lv = self.l.inner_value();
        let rv = self.r.inner_value();
        self.l.push_adj(parent, adj * O::d_left(lv, rv));
        self.r.push_adj(parent, adj * O::d_right(lv, rv));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  UnExpr
// ═══════════════════════════════════════════════════════════════════════════

/// A lazy unary expression over a child expression.
#[derive(Clone, Copy)]
pub struct UnExpr<T, A, O> {
    a: A,
    val: T,
    _ph: std::marker::PhantomData<O>,
}

impl<T: InnerScalar, A: Expr<T>, O: UnOp<T>> UnExpr<T, A, O> {
    #[inline]
    pub(crate) fn new(a: A) -> Self {
        let val = O::eval(a.inner_value());
        Self {
            a,
            val,
            _ph: std::marker::PhantomData,
        }
    }
}

impl<T: InnerScalar, A: Expr<T>, O: UnOp<T> + Clone> Expr<T> for UnExpr<T, A, O> {
    #[inline]
    fn inner_value(&self) -> T {
        self.val
    }
    fn push_adj(&self, parent: &mut TapeNode<T>, adj: T) {
        self.a
            .push_adj(parent, adj * O::deriv(self.a.inner_value(), self.val));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  flatten — records an expression tree into one tape node
// ═══════════════════════════════════════════════════════════════════════════

/// Records an expression tree into one tape node, returning a [`Dual<T>`].
pub(crate) fn flatten<T: TapeHolder + InnerScalar, E: Expr<T>>(e: &E) -> Dual<T> {
    let mut node = TapeNode::default();
    e.push_adj(&mut node, T::one());
    let ptr_opt = T::with_tape(|tape| tape.record(node));
    Dual::from_raw(e.inner_value(), ptr_opt)
}

// ═══════════════════════════════════════════════════════════════════════════
//  Expr<T> for Dual<T>
// ═══════════════════════════════════════════════════════════════════════════

impl<T: TapeHolder + InnerScalar> Expr<T> for Dual<T> {
    #[inline]
    fn inner_value(&self) -> T {
        self.val()
    }
    fn push_adj(&self, parent: &mut TapeNode<T>, deriv: T) {
        if let Some(p) = self.node_ptr() {
            parent.childs.push(p);
            parent.derivs.push(deriv);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Expr<T> for Const<T>
// ═══════════════════════════════════════════════════════════════════════════

impl<T: InnerScalar> Expr<T> for Const<T> {
    #[inline]
    fn inner_value(&self) -> T {
        self.0
    }
    #[inline]
    fn push_adj(&self, _: &mut TapeNode<T>, _: T) {}
}

// ═══════════════════════════════════════════════════════════════════════════
//  Operator impls — expression-template style  (Dual, Const, BinExpr, UnExpr)
// ═══════════════════════════════════════════════════════════════════════════

macro_rules! impl_bin_ops_for {
    (simple $Self:ty, $T:ident) => {
        impl<$T: TapeHolder + InnerScalar, Rhs: Expr<$T> + Clone> Add<Rhs> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Rhs, AddOp>;
            fn add(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<$T: TapeHolder + InnerScalar> Add<f64> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Const<$T>, AddOp>;
            fn add(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const($T::scalar(rhs)))
            }
        }
        impl<$T: TapeHolder + InnerScalar, Rhs: Expr<$T> + Clone> Sub<Rhs> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Rhs, SubOp>;
            fn sub(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<$T: TapeHolder + InnerScalar> Sub<f64> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Const<$T>, SubOp>;
            fn sub(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const($T::scalar(rhs)))
            }
        }
        impl<$T: TapeHolder + InnerScalar, Rhs: Expr<$T> + Clone> Mul<Rhs> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Rhs, MulOp>;
            fn mul(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<$T: TapeHolder + InnerScalar> Mul<f64> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Const<$T>, MulOp>;
            fn mul(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const($T::scalar(rhs)))
            }
        }
        impl<$T: TapeHolder + InnerScalar, Rhs: Expr<$T> + Clone> Div<Rhs> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Rhs, DivOp>;
            fn div(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<$T: TapeHolder + InnerScalar> Div<f64> for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Self, Const<$T>, DivOp>;
            fn div(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const($T::scalar(rhs)))
            }
        }
        impl<$T: TapeHolder + InnerScalar> Neg for $Self
        where
            Self: Expr<$T> + Clone,
        {
            type Output = BinExpr<$T, Const<$T>, Self, SubOp>;
            fn neg(self) -> Self::Output {
                BinExpr::new(Const($T::zero()), self)
            }
        }
    };
}

impl_bin_ops_for!(simple Dual<T>, T);
impl_bin_ops_for!(simple Const<T>, T);

// -- BinExpr operators -------------------------------------------------------

impl<T: TapeHolder + InnerScalar, L, R, O, Rhs> Add<Rhs> for BinExpr<T, L, R, O>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, AddOp>;
    fn add(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O> Add<f64> for BinExpr<T, L, R, O>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, AddOp>;
    fn add(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O, Rhs> Sub<Rhs> for BinExpr<T, L, R, O>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, SubOp>;
    fn sub(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O> Sub<f64> for BinExpr<T, L, R, O>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, SubOp>;
    fn sub(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O, Rhs> Mul<Rhs> for BinExpr<T, L, R, O>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, MulOp>;
    fn mul(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O> Mul<f64> for BinExpr<T, L, R, O>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, MulOp>;
    fn mul(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O, Rhs> Div<Rhs> for BinExpr<T, L, R, O>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, DivOp>;
    fn div(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O> Div<f64> for BinExpr<T, L, R, O>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, DivOp>;
    fn div(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, L, R, O> Neg for BinExpr<T, L, R, O>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Const<T>, Self, SubOp>;
    fn neg(self) -> Self::Output {
        BinExpr::new(Const(T::zero()), self)
    }
}

// -- UnExpr operators --------------------------------------------------------

impl<T: TapeHolder + InnerScalar, A, O2, Rhs> Add<Rhs> for UnExpr<T, A, O2>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, AddOp>;
    fn add(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, A, O2> Add<f64> for UnExpr<T, A, O2>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, AddOp>;
    fn add(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, A, O2, Rhs> Sub<Rhs> for UnExpr<T, A, O2>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, SubOp>;
    fn sub(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, A, O2> Sub<f64> for UnExpr<T, A, O2>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, SubOp>;
    fn sub(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, A, O2, Rhs> Mul<Rhs> for UnExpr<T, A, O2>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, MulOp>;
    fn mul(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, A, O2> Mul<f64> for UnExpr<T, A, O2>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, MulOp>;
    fn mul(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, A, O2, Rhs> Div<Rhs> for UnExpr<T, A, O2>
where
    Rhs: Expr<T> + Clone,
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Rhs, DivOp>;
    fn div(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<T: TapeHolder + InnerScalar, A, O2> Div<f64> for UnExpr<T, A, O2>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Self, Const<T>, DivOp>;
    fn div(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(T::scalar(rhs)))
    }
}
impl<T: TapeHolder + InnerScalar, A, O2> Neg for UnExpr<T, A, O2>
where
    Self: Expr<T> + Clone,
{
    type Output = BinExpr<T, Const<T>, Self, SubOp>;
    fn neg(self) -> Self::Output {
        BinExpr::new(Const(T::zero()), self)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Comparison for BinExpr / UnExpr
// ═══════════════════════════════════════════════════════════════════════════

impl<T: InnerScalar, L, R, O> PartialEq for BinExpr<T, L, R, O>
where
    L: Expr<T>,
    R: Expr<T>,
    O: BinOp<T> + Clone,
{
    fn eq(&self, rhs: &Self) -> bool {
        self.inner_value().value() == rhs.inner_value().value()
    }
}
impl<T: InnerScalar, L, R, O> PartialOrd for BinExpr<T, L, R, O>
where
    L: Expr<T>,
    R: Expr<T>,
    O: BinOp<T> + Clone,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.inner_value()
            .value()
            .partial_cmp(&rhs.inner_value().value())
    }
}
impl<T: InnerScalar, L, R, O> PartialEq<f64> for BinExpr<T, L, R, O>
where
    L: Expr<T>,
    R: Expr<T>,
    O: BinOp<T> + Clone,
{
    fn eq(&self, rhs: &f64) -> bool {
        self.inner_value().value() == *rhs
    }
}
impl<T: InnerScalar, L, R, O> PartialOrd<f64> for BinExpr<T, L, R, O>
where
    L: Expr<T>,
    R: Expr<T>,
    O: BinOp<T> + Clone,
{
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.inner_value().value().partial_cmp(rhs)
    }
}
impl<T: InnerScalar, A, O> PartialEq for UnExpr<T, A, O>
where
    A: Expr<T>,
    O: UnOp<T> + Clone,
{
    fn eq(&self, rhs: &Self) -> bool {
        self.inner_value().value() == rhs.inner_value().value()
    }
}
impl<T: InnerScalar, A, O> PartialOrd for UnExpr<T, A, O>
where
    A: Expr<T>,
    O: UnOp<T> + Clone,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.inner_value()
            .value()
            .partial_cmp(&rhs.inner_value().value())
    }
}
impl<T: InnerScalar, A, O> PartialEq<f64> for UnExpr<T, A, O>
where
    A: Expr<T>,
    O: UnOp<T> + Clone,
{
    fn eq(&self, rhs: &f64) -> bool {
        self.inner_value().value() == *rhs
    }
}
impl<T: InnerScalar, A, O> PartialOrd<f64> for UnExpr<T, A, O>
where
    A: Expr<T>,
    O: UnOp<T> + Clone,
{
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.inner_value().value().partial_cmp(rhs)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  FloatExt — lazy transcendentals for any expression
// ═══════════════════════════════════════════════════════════════════════════

/// Lazy transcendental methods available on any expression-template node.
pub trait FloatExt<T: InnerScalar>: Expr<T> + Clone + Sized {
    /// Returns `e^x` lazily.
    #[inline]
    fn exp(self) -> UnExpr<T, Self, ExpOp> {
        UnExpr::new(self)
    }
    /// Returns the natural logarithm lazily.
    #[inline]
    fn ln(self) -> UnExpr<T, Self, LogOp> {
        UnExpr::new(self)
    }
    /// Returns the square root lazily.
    #[inline]
    fn sqrt(self) -> UnExpr<T, Self, SqrtOp> {
        UnExpr::new(self)
    }
    /// Returns the sine lazily.
    #[inline]
    fn sin(self) -> UnExpr<T, Self, SinOp> {
        UnExpr::new(self)
    }
    /// Returns the cosine lazily.
    #[inline]
    fn cos(self) -> UnExpr<T, Self, CosOp> {
        UnExpr::new(self)
    }
    /// Returns the absolute value lazily.
    #[inline]
    fn abs(self) -> UnExpr<T, Self, AbsOp> {
        UnExpr::new(self)
    }
    /// Raises the expression to a constant power lazily.
    #[inline]
    fn powf(self, p: f64) -> BinExpr<T, Self, Const<T>, PowOp> {
        BinExpr::new(self, Const(T::scalar(p)))
    }
    /// Raises the expression to the power of another expression lazily.
    #[inline]
    fn pow_expr<R: Expr<T> + Clone>(self, p: R) -> BinExpr<T, Self, R, PowOp> {
        BinExpr::new(self, p)
    }
    /// Returns the minimum of two expressions lazily.
    #[inline]
    fn min<R: Expr<T> + Clone>(self, r: R) -> BinExpr<T, Self, R, MinOp> {
        BinExpr::new(self, r)
    }
    /// Returns the maximum of two expressions lazily.
    #[inline]
    fn max<R: Expr<T> + Clone>(self, r: R) -> BinExpr<T, Self, R, MaxOp> {
        BinExpr::new(self, r)
    }
}

impl<T: InnerScalar, E: Expr<T> + Clone> FloatExt<T> for E {}

// ═══════════════════════════════════════════════════════════════════════════
//  Assign ops  (flatten + assign)
// ═══════════════════════════════════════════════════════════════════════════

macro_rules! impl_assign {
    ($Trait:ident, $func:ident, $Op:ident, $sym:tt) => {
        impl<T: TapeHolder + InnerScalar, E: Expr<T> + Clone> $Trait<E> for Dual<T> {
            fn $func(&mut self, rhs: E) {
                *self = flatten(&(self.clone() $sym rhs));
            }
        }
        impl<T: TapeHolder + InnerScalar> $Trait<f64> for Dual<T> {
            fn $func(&mut self, rhs: f64) {
                *self = flatten(&(self.clone() $sym Const(T::scalar(rhs))));
            }
        }
    };
}

impl_assign!(AddAssign, add_assign, AddOp, +);
impl_assign!(SubAssign, sub_assign, SubOp, -);
impl_assign!(MulAssign, mul_assign, MulOp, *);
impl_assign!(DivAssign, div_assign, DivOp, /);

// ═══════════════════════════════════════════════════════════════════════════
//  From conversions — flattening expressions into Dual<T>
// ═══════════════════════════════════════════════════════════════════════════

impl<T, L, R, O> From<BinExpr<T, L, R, O>> for Dual<T>
where
    T: TapeHolder + InnerScalar,
    L: Expr<T> + Clone,
    R: Expr<T> + Clone,
    O: BinOp<T> + Clone,
{
    fn from(e: BinExpr<T, L, R, O>) -> Self {
        flatten(&e)
    }
}

impl<T, A, O> From<UnExpr<T, A, O>> for Dual<T>
where
    T: TapeHolder + InnerScalar,
    A: Expr<T> + Clone,
    O: UnOp<T> + Clone,
{
    fn from(e: UnExpr<T, A, O>) -> Self {
        flatten(&e)
    }
}

impl<T: TapeHolder + InnerScalar> From<f64> for Dual<T> {
    fn from(v: f64) -> Self {
        Self::new(v)
    }
}

impl<T: TapeHolder + InnerScalar> From<Const<T>> for Dual<T> {
    fn from(c: Const<T>) -> Self {
        Self::constant(c.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Rem (expression-template compatible)
// ═══════════════════════════════════════════════════════════════════════════

impl<T: TapeHolder + InnerScalar> std::ops::Rem for Dual<T> {
    type Output = Self;
    fn rem(self, r: Self) -> Self {
        Self::constant(T::scalar(self.value() % r.value()))
    }
}
impl<T: TapeHolder + InnerScalar> std::ops::Rem<f64> for Dual<T> {
    type Output = Self;
    fn rem(self, r: f64) -> Self {
        Self::constant(T::scalar(self.value() % r))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Free-standing transcendental functions
// ═══════════════════════════════════════════════════════════════════════════

/// Returns the exponential of an expression.
#[inline]
pub fn exp<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, ExpOp> {
    UnExpr::new(a)
}
/// Returns the natural logarithm of an expression.
#[inline]
pub fn log<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, LogOp> {
    UnExpr::new(a)
}
/// Returns the square root of an expression.
#[inline]
pub fn sqrt<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, SqrtOp> {
    UnExpr::new(a)
}
/// Returns the absolute value of an expression.
#[inline]
pub fn abs<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, AbsOp> {
    UnExpr::new(a)
}
/// Returns the absolute value of an expression (alias).
#[inline]
pub fn fabs<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, FabsOp> {
    UnExpr::new(a)
}
/// Returns the sine of an expression.
#[inline]
pub fn sin<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, SinOp> {
    UnExpr::new(a)
}
/// Returns the cosine of an expression.
#[inline]
pub fn cos<T: InnerScalar, A: Expr<T> + Clone>(a: A) -> UnExpr<T, A, CosOp> {
    UnExpr::new(a)
}
/// Raises one expression to the power of another.
#[inline]
pub fn pow<T: InnerScalar, L: Expr<T> + Clone, R: Expr<T> + Clone>(
    l: L,
    r: R,
) -> BinExpr<T, L, R, PowOp> {
    BinExpr::new(l, r)
}
/// Returns the maximum of two expressions.
#[inline]
pub fn max<T: InnerScalar, L: Expr<T> + Clone, R: Expr<T> + Clone>(
    l: L,
    r: R,
) -> BinExpr<T, L, R, MaxOp> {
    BinExpr::new(l, r)
}
/// Returns the minimum of two expressions.
#[inline]
pub fn min<T: InnerScalar, L: Expr<T> + Clone, R: Expr<T> + Clone>(
    l: L,
    r: R,
) -> BinExpr<T, L, R, MinOp> {
    BinExpr::new(l, r)
}
