//! Automatic differentiation number types and expression building blocks.

use core::fmt;

use crate::ad::node::TapeNode;
use crate::ad::tape::{Tape, TAPE};
use crate::utils::errors::{AtlasError, Result};
use std::cell::Cell;
use std::ptr::NonNull;
use std::{cmp::Ordering, ops::*};

/// A scalar value tracked on the automatic differentiation tape.
#[derive(Clone, Copy, Default)]
pub struct ADReal {
    val: f64,
    node: Option<NonNull<TapeNode>>,
}

/// Conversion helpers for numeric types used by this crate.
pub trait IsReal
where
    Self: Sized,
{
    /// Creates a new numeric value from the given scalar in f64.
    fn new(v: f64) -> Self;
    /// Returns the underlying scalar value.
    fn value(&self) -> f64;
    /// Returns one as base-type.
    fn one() -> Self;
    /// Returns zero as base-type.
    fn zero() -> Self;
}

impl IsReal for f64 {
    #[inline]
    fn new(v: f64) -> Self {
        v
    }

    #[inline]
    fn value(&self) -> f64 {
        *self
    }

    #[inline]
    fn one() -> Self {
        1.0
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
}

impl IsReal for ADReal {
    #[inline]
    fn new(val: f64) -> Self {
        let node = Self::TAPE_PTR.with(|t| unsafe { t.get().as_mut().new_leaf() });
        Self { val, node }
    }
    #[inline]
    fn value(&self) -> f64 {
        self.val
    }
    #[inline]
    fn one() -> Self {
        Self::new(1.0)
    }
    #[inline]
    fn zero() -> Self {
        Self::new(0.0)
    }
}

/// A differentiable expression that can record its contribution to the tape.
pub trait Expr: Clone {
    /// Returns the scalar value of the expression.
    fn inner_value(&self) -> f64;
    /// Pushes this expression's adjoint contribution into the tape node.
    fn push_adj(&self, parent: &mut TapeNode, adj: f64);
}

impl fmt::Debug for ADReal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ADReal({}, Node: {:?})", self.val, self.node)
    }
}

impl fmt::Display for ADReal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ADReal({})", self.val)
    }
}

unsafe impl Sync for ADReal {}
unsafe impl Send for ADReal {}

impl ADReal {
    thread_local! {
        static TAPE_PTR: Cell<NonNull<Tape>> = Cell::new(TAPE.with(|t| NonNull::new(t.as_ptr()).unwrap()));
    }

    /// Sets the active tape for the current thread.
    pub fn set_tape(t: NonNull<Tape>) {
        Self::TAPE_PTR.with(|c| c.set(t));
    }

    /// Returns the raw pointer address of the current tape.
    #[must_use]
    pub fn tape_addr() -> NonNull<Tape> {
        Self::TAPE_PTR.with(Cell::get)
    }

    #[inline]
    /// Returns the adjoint for this value if it is on the tape.
    pub fn adjoint(&self) -> Result<f64> {
        self.node
            .map(|p| unsafe { p.as_ref().adj })
            .ok_or(AtlasError::NodeNotIndexedInTapeErr)
    }

    /// Runs a full backward pass from this node to the start of the tape.
    pub fn backward(&self) -> Result<()> {
        let root = self.node.ok_or(AtlasError::NodeNotIndexedInTapeErr)?;

        Self::TAPE_PTR.with(|t| {
            let tape = unsafe { &mut *t.get().as_ptr() };
            tape.mut_node(root).unwrap().adj = 1.0;
            tape.propagate_from(root)
        })?;
        Ok(())
    }

    /// Runs a backward pass from the current mark down to the start.
    pub fn backward_mark_to_start(&self) -> Result<()> {
        let root: NonNull<TapeNode> = self.node.ok_or(AtlasError::NodeNotIndexedInTapeErr)?;

        Self::TAPE_PTR.with(|t| {
            let tape = unsafe { &mut *t.get().as_ptr() };
            tape.mut_node(root).unwrap().adj = 1.0;
            tape.propagate_mark_to_start();
        });
        Ok(())
    }

    /// Runs a backward pass from the end of the tape down to the current mark.
    pub fn backward_to_mark(&self) -> Result<()> {
        let root: NonNull<TapeNode> = self.node.ok_or(AtlasError::NodeNotIndexedInTapeErr)?;
        Self::TAPE_PTR.with(|t| {
            let tape = unsafe { &mut *t.get().as_ptr() };
            tape.mut_node(root).unwrap().adj = 1.0;
            tape.propagate_to_mark();
        });
        Ok(())
    }

    /// Attaches this value to the current tape if it is not already recorded.
    pub fn put_on_tape(&mut self) {
        if self.node.is_some() {
            return; // already on a tape
        }

        Self::TAPE_PTR.with(|t| {
            let ptr = t.get().as_ptr();
            let node = unsafe { (*ptr).new_leaf() };
            self.node = node;
        });
    }
}

impl Expr for ADReal {
    #[inline]
    /// Returns the scalar value for use in tape recording.
    fn inner_value(&self) -> f64 {
        self.val
    }

    /// Pushes this value into the parent tape node with the given derivative.
    fn push_adj(&self, parent: &mut TapeNode, deriv: f64) {
        if let Some(p) = self.node {
            parent.childs.push(p);
            parent.derivs.push(deriv);
        }
    }
}

/// Records an expression into the tape, returning the resulting `ADReal`.
fn flatten<E: Expr + Clone>(e: &E) -> ADReal {
    let mut node = TapeNode::default();
    e.push_adj(&mut node, 1.0);

    let ptr_opt = ADReal::TAPE_PTR.with(|t| {
        let tape_ptr = unsafe { &mut *t.get().as_ptr() };
        tape_ptr.record(node)
    });

    ADReal {
        val: e.inner_value(),
        node: ptr_opt,
    }
}

impl PartialEq for ADReal {
    fn eq(&self, o: &Self) -> bool {
        self.val == o.val
    }
}
impl PartialOrd for ADReal {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        self.val.partial_cmp(&o.val)
    }
}

#[derive(Clone, Copy)]
/// A constant expression wrapper for interoperability.
pub struct Const(pub f64);

impl From<f64> for Const {
    #[inline]
    /// Converts a `f64` into a constant expression.
    fn from(v: f64) -> Self {
        Const(v)
    }
}
impl From<f32> for Const {
    #[inline]
    /// Converts a `f32` into a constant expression.
    fn from(v: f32) -> Self {
        Const(f64::from(v))
    }
}
impl From<i32> for Const {
    #[inline]
    /// Converts an `i32` into a constant expression.
    fn from(v: i32) -> Self {
        Const(f64::from(v))
    }
}
impl From<u32> for Const {
    #[inline]
    /// Converts a `u32` into a constant expression.
    fn from(v: u32) -> Self {
        Const(f64::from(v))
    }
}
impl From<i64> for Const {
    #[inline]
    /// Converts an `i64` into a constant expression.
    fn from(v: i64) -> Self {
        Const(v as f64)
    }
}
impl From<u64> for Const {
    #[inline]
    /// Converts a `u64` into a constant expression.
    fn from(v: u64) -> Self {
        Const(v as f64)
    }
}
impl From<Const> for f64 {
    #[inline]
    /// Extracts the underlying `f64` from a constant expression.
    fn from(c: Const) -> Self {
        c.0
    }
}

impl Expr for Const {
    #[inline]
    /// Returns the scalar value of the constant expression.
    fn inner_value(&self) -> f64 {
        self.0
    }
    #[inline]
    /// Constants do not contribute adjoints to the tape.
    fn push_adj(&self, _: &mut TapeNode, _: f64) {}
}

/// A binary operation definition for the expression system.
pub trait BinOp {
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64;
    /// Computes the derivative with respect to the left operand.
    fn d_left(l: f64, r: f64) -> f64;
    /// Computes the derivative with respect to the right operand.
    fn d_right(l: f64, r: f64) -> f64;
}

#[derive(Clone, Copy, Debug)]
/// Binary addition operator.
pub struct AddOp;
impl BinOp for AddOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l + r
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(_: f64, _: f64) -> f64 {
        1.0
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(_: f64, _: f64) -> f64 {
        1.0
    }
}
#[derive(Clone, Copy, Debug)]
/// Binary subtraction operator.
pub struct SubOp;
impl BinOp for SubOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l - r
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(_: f64, _: f64) -> f64 {
        1.0
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(_: f64, _: f64) -> f64 {
        -1.0
    }
}
#[derive(Clone, Copy, Debug)]
/// Binary multiplication operator.
pub struct MulOp;
impl BinOp for MulOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l * r
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(_: f64, r: f64) -> f64 {
        r
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(l: f64, _: f64) -> f64 {
        l
    }
}
#[derive(Clone, Copy, Debug)]
/// Binary division operator.
pub struct DivOp;
impl BinOp for DivOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l / r
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(_: f64, r: f64) -> f64 {
        1.0 / r
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(l: f64, r: f64) -> f64 {
        -l / (r * r)
    }
}
#[derive(Clone, Copy, Debug)]
/// Binary power operator.
pub struct PowOp;
impl BinOp for PowOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l.powf(r)
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(l: f64, r: f64) -> f64 {
        r * l.powf(r - 1.0)
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(l: f64, r: f64) -> f64 {
        l.powf(r) * l.ln()
    }
}

#[derive(Clone, Copy, Debug)]
/// Binary maximum operator.
pub struct MaxOp;
impl BinOp for MaxOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l.max(r)
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(l: f64, r: f64) -> f64 {
        if l > r {
            1.0
        } else {
            0.0
        }
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(l: f64, r: f64) -> f64 {
        if r > l {
            1.0
        } else {
            0.0
        }
    }
}
#[derive(Clone, Copy, Debug)]
/// Binary minimum operator.
pub struct MinOp;
impl BinOp for MinOp {
    #[inline]
    /// Evaluates the operator on the input values.
    fn eval(l: f64, r: f64) -> f64 {
        l.min(r)
    }
    #[inline]
    /// Returns the derivative with respect to the left operand.
    fn d_left(l: f64, r: f64) -> f64 {
        if l < r {
            1.0
        } else {
            0.0
        }
    }
    #[inline]
    /// Returns the derivative with respect to the right operand.
    fn d_right(l: f64, r: f64) -> f64 {
        if r < l {
            1.0
        } else {
            0.0
        }
    }
}

#[derive(Clone)]
/// A binary expression over two child expressions.
pub struct BinExpr<L, R, O> {
    l: L,
    r: R,
    val: f64,
    _ph: std::marker::PhantomData<O>,
}
impl<L: Expr, R: Expr, O: BinOp> BinExpr<L, R, O> {
    #[inline]
    /// Constructs a new binary expression and caches its value.
    fn new(l: L, r: R) -> Self {
        let val = O::eval(l.inner_value(), r.inner_value());
        Self {
            l,
            r,
            val,
            _ph: std::marker::PhantomData,
        }
    }
}
impl<L: Expr, R: Expr, O: BinOp + Clone> Expr for BinExpr<L, R, O> {
    #[inline]
    /// Returns the cached scalar value of the expression.
    fn inner_value(&self) -> f64 {
        self.val
    }
    /// Pushes adjoint contributions for the left and right child expressions.
    fn push_adj(&self, parent: &mut TapeNode, adj: f64) {
        self.l.push_adj(
            parent,
            adj * O::d_left(self.l.inner_value(), self.r.inner_value()),
        );
        self.r.push_adj(
            parent,
            adj * O::d_right(self.l.inner_value(), self.r.inner_value()),
        );
    }
}

/// A unary operation definition for the expression system.
pub trait UnOp {
    /// Evaluates the operator on the input value.
    fn eval(x: f64) -> f64;
    /// Computes the derivative with respect to the input.
    fn deriv(x: f64, v: f64) -> f64;
}

macro_rules! un_op {
    ($name:ident, $doc:expr, $eval:expr, $d:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        impl UnOp for $name {
            #[inline]
            /// Evaluates the unary operator.
            fn eval(x: f64) -> f64 {
                $eval(x)
            }
            #[inline]
            /// Returns the derivative of the unary operator.
            fn deriv(x: f64, v: f64) -> f64 {
                $d(x, v)
            }
        }
    };
}

un_op!(ExpOp, "Unary exponential operator.", f64::exp, |_x, v| v);
un_op!(
    LogOp,
    "Unary natural logarithm operator.",
    f64::ln,
    |x, _| 1.0 / x
);
un_op!(
    SqrtOp,
    "Unary square root operator.",
    f64::sqrt,
    |_x, v| 0.5 / v
);
un_op!(
    FabsOp,
    "Unary absolute value operator (alias).",
    f64::abs,
    |x, _| if x >= 0.0 { 1.0 } else { -1.0 }
);
un_op!(SinOp, "Unary sine operator.", f64::sin, |x, _v| f64::cos(x));
un_op!(
    CosOp,
    "Unary cosine operator.",
    f64::cos,
    |x, _v| -f64::sin(x)
);
un_op!(
    AbsOp,
    "Unary absolute value operator.",
    f64::abs,
    |x, _v| if x >= 0.0 { 1.0 } else { -1.0 }
);

#[derive(Clone)]
/// A unary expression over a child expression.
pub struct UnExpr<A, O> {
    a: A,
    val: f64,
    _ph: std::marker::PhantomData<O>,
}
impl<A: Expr, O: UnOp> UnExpr<A, O> {
    #[inline]
    /// Constructs a new unary expression and caches its value.
    fn new(a: A) -> Self {
        let val = O::eval(a.inner_value());
        Self {
            a,
            val,
            _ph: std::marker::PhantomData,
        }
    }
}
impl<A: Expr, O: UnOp + Clone> Expr for UnExpr<A, O> {
    #[inline]
    /// Returns the cached scalar value of the expression.
    fn inner_value(&self) -> f64 {
        self.val
    }
    /// Pushes adjoint contributions for the child expression.
    fn push_adj(&self, parent: &mut TapeNode, adj: f64) {
        self.a
            .push_adj(parent, adj * O::deriv(self.a.inner_value(), self.val));
    }
}

macro_rules! impl_bin_ops_local {
    ($Self:ty) => {
        impl<Rhs> Add<Rhs> for $Self
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, AddOp>;
            fn add(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl Add<f64> for $Self
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, AddOp>;
            fn add(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        impl<Rhs> Sub<Rhs> for $Self
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, SubOp>;
            fn sub(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl Sub<f64> for $Self
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, SubOp>;
            fn sub(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        impl<Rhs> Mul<Rhs> for $Self
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, MulOp>;
            fn mul(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl Mul<f64> for $Self
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, MulOp>;
            fn mul(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        impl<Rhs> Div<Rhs> for $Self
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, DivOp>;
            fn div(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl Div<f64> for $Self
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, DivOp>;
            fn div(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        impl Neg for $Self
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Const, Self, SubOp>;
            fn neg(self) -> Self::Output {
                BinExpr::new(Const(0.0), self)
            }
        }
    };
}

impl_bin_ops_local!(ADReal);
impl_bin_ops_local!(Const);

macro_rules! impl_bin_ops_expr {
    ($Expr:ident) => {
        impl<L, R, O, Rhs> Add<Rhs> for $Expr<L, R, O>
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, AddOp>;
            fn add(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<L, R, O> Add<f64> for $Expr<L, R, O>
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, AddOp>;
            fn add(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        /* Sub ------------------------------------------------------- */
        impl<L, R, O, Rhs> Sub<Rhs> for $Expr<L, R, O>
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, SubOp>;
            fn sub(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<L, R, O> Sub<f64> for $Expr<L, R, O>
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, SubOp>;
            fn sub(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        /* Mul ------------------------------------------------------- */
        impl<L, R, O, Rhs> Mul<Rhs> for $Expr<L, R, O>
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, MulOp>;
            fn mul(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<L, R, O> Mul<f64> for $Expr<L, R, O>
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, MulOp>;
            fn mul(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        /* Div ------------------------------------------------------- */
        impl<L, R, O, Rhs> Div<Rhs> for $Expr<L, R, O>
        where
            Rhs: Expr + Clone,
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Rhs, DivOp>;
            fn div(self, rhs: Rhs) -> Self::Output {
                BinExpr::new(self, rhs)
            }
        }
        impl<L, R, O> Div<f64> for $Expr<L, R, O>
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Self, Const, DivOp>;
            fn div(self, rhs: f64) -> Self::Output {
                BinExpr::new(self, Const(rhs))
            }
        }

        /* Neg ------------------------------------------------------- */
        impl<L, R, O> Neg for $Expr<L, R, O>
        where
            Self: Expr + Clone,
        {
            type Output = BinExpr<Const, Self, SubOp>;
            fn neg(self) -> Self::Output {
                BinExpr::new(Const(0.0), self)
            }
        }
    };
}

impl_bin_ops_expr!(BinExpr);

macro_rules! impl_assign {
    ($Trait:ident, $func:ident, $Op:ident, $sym:tt) => {
        impl<E> $Trait<E> for ADReal
        where
            E: Expr + Clone,
        {
            fn $func(&mut self, rhs: E) {
                *self = flatten(&(self.clone() $sym rhs));
            }
        }
        impl $Trait<f64> for ADReal {
            fn $func(&mut self, rhs: f64) {
                *self = flatten(&(self.clone() $sym Const(rhs)));
            }
        }
    };
}

impl_assign!(AddAssign, add_assign, AddOp, +);
impl_assign!(SubAssign, sub_assign, SubOp, -);
impl_assign!(MulAssign, mul_assign, MulOp, *);
impl_assign!(DivAssign, div_assign, DivOp, /);

impl<A, O> PartialEq for UnExpr<A, O>
where
    A: Expr,
    O: UnOp + Clone,
{
    fn eq(&self, rhs: &Self) -> bool {
        self.inner_value() == rhs.inner_value()
    }
}
impl<A, O> PartialOrd for UnExpr<A, O>
where
    A: Expr,
    O: UnOp + Clone,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.inner_value().partial_cmp(&rhs.inner_value())
    }
}
impl<A, O> PartialEq<f64> for UnExpr<A, O>
where
    A: Expr,
    O: UnOp + Clone,
{
    fn eq(&self, rhs: &f64) -> bool {
        self.inner_value() == *rhs
    }
}
impl<A, O> PartialOrd<f64> for UnExpr<A, O>
where
    A: Expr,
    O: UnOp + Clone,
{
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.inner_value().partial_cmp(rhs)
    }
}

/* BinExpr ≷ BinExpr & f64 */
impl<L, R, O> PartialEq for BinExpr<L, R, O>
where
    L: Expr,
    R: Expr,
    O: BinOp + Clone,
{
    fn eq(&self, rhs: &Self) -> bool {
        self.inner_value() == rhs.inner_value()
    }
}
impl<L, R, O> PartialOrd for BinExpr<L, R, O>
where
    L: Expr,
    R: Expr,
    O: BinOp + Clone,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.inner_value().partial_cmp(&rhs.inner_value())
    }
}
impl<L, R, O> PartialEq<f64> for BinExpr<L, R, O>
where
    L: Expr,
    R: Expr,
    O: BinOp + Clone,
{
    fn eq(&self, rhs: &f64) -> bool {
        self.inner_value() == *rhs
    }
}
impl<L, R, O> PartialOrd<f64> for BinExpr<L, R, O>
where
    L: Expr,
    R: Expr,
    O: BinOp + Clone,
{
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.inner_value().partial_cmp(rhs)
    }
}

#[inline]
/// Returns the exponential of an expression.
pub fn exp<A: Expr + Clone>(a: A) -> UnExpr<A, ExpOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the natural logarithm of an expression.
pub fn log<A: Expr + Clone>(a: A) -> UnExpr<A, LogOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the square root of an expression.
pub fn sqrt<A: Expr + Clone>(a: A) -> UnExpr<A, SqrtOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the absolute value of an expression.
pub fn fabs<A: Expr + Clone>(a: A) -> UnExpr<A, FabsOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the sine of an expression.
pub fn sin<A: Expr + Clone>(a: A) -> UnExpr<A, SinOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the cosine of an expression.
pub fn cos<A: Expr + Clone>(a: A) -> UnExpr<A, CosOp> {
    UnExpr::new(a)
}
#[inline]
/// Returns the absolute value of an expression.
pub fn abs<A: Expr + Clone>(a: A) -> UnExpr<A, AbsOp> {
    UnExpr::new(a)
}

#[inline]
/// Raises one expression to the power of another.
pub fn pow<L: Expr + Clone, R: Expr + Clone>(l: L, r: R) -> BinExpr<L, R, PowOp> {
    BinExpr::new(l, r)
}
#[inline]
/// Returns the maximum of two expressions.
pub fn max<L: Expr + Clone, R: Expr + Clone>(l: L, r: R) -> BinExpr<L, R, MaxOp> {
    BinExpr::new(l, r)
}
#[inline]
/// Returns the minimum of two expressions.
pub fn min<L: Expr + Clone, R: Expr + Clone>(l: L, r: R) -> BinExpr<L, R, MinOp> {
    BinExpr::new(l, r)
}

impl<L, R, O> From<BinExpr<L, R, O>> for ADReal
where
    L: Expr + Clone,
    R: Expr + Clone,
    O: BinOp + Clone,
{
    /// Flattens a binary expression into an `ADReal` and records it.
    fn from(e: BinExpr<L, R, O>) -> Self {
        flatten(&e)
    }
}
impl<A, O> From<UnExpr<A, O>> for ADReal
where
    A: Expr + Clone,
    O: UnOp + Clone,
{
    /// Flattens a unary expression into an `ADReal` and records it.
    fn from(e: UnExpr<A, O>) -> Self {
        flatten(&e)
    }
}
impl From<f64> for ADReal {
    /// Converts a `f64` into an `ADReal`, recording if the tape is active.
    fn from(v: f64) -> Self {
        Self::new(v)
    }
}
impl From<f32> for ADReal {
    /// Converts a `f32` into an `ADReal`, recording if the tape is active.
    fn from(v: f32) -> Self {
        Self::new(f64::from(v))
    }
}
impl From<i32> for ADReal {
    /// Converts an `i32` into an `ADReal`, recording if the tape is active.
    fn from(v: i32) -> Self {
        Self::new(f64::from(v))
    }
}
impl From<Const> for ADReal {
    /// Converts a `Const` expression into an `ADReal`.
    fn from(v: Const) -> Self {
        Self::new(v.0)
    }
}

/// Convenience methods for common floating-point operations on expressions.
pub trait FloatExt: Expr + Clone + Sized {
    #[inline]
    /// Returns `e^x` for the expression.
    fn exp(self) -> UnExpr<Self, ExpOp> {
        UnExpr::new(self)
    }
    #[inline]
    /// Returns the natural logarithm of the expression.
    fn ln(self) -> UnExpr<Self, LogOp> {
        UnExpr::new(self)
    }
    #[inline]
    /// Returns the sine of the expression.
    fn sin(self) -> UnExpr<Self, SinOp> {
        UnExpr::new(self)
    }
    #[inline]
    /// Returns the cosine of the expression.
    fn cos(self) -> UnExpr<Self, CosOp> {
        UnExpr::new(self)
    }
    #[inline]
    /// Returns the absolute value of the expression.
    fn abs(self) -> UnExpr<Self, AbsOp> {
        UnExpr::new(self)
    }

    #[inline]
    /// Raises the expression to a constant power.
    fn powf(self, p: f64) -> BinExpr<Self, Const, PowOp> {
        BinExpr::new(self, Const(p))
    }

    #[inline]
    /// Returns the square root of the expression.
    fn sqrt(self) -> UnExpr<Self, SqrtOp> {
        UnExpr::new(self)
    }

    #[inline]
    /// Raises the expression to the power of another expression.
    fn pow_expr<R: Expr + Clone>(self, p: R) -> BinExpr<Self, R, PowOp> {
        BinExpr::new(self, p)
    }

    #[inline]
    /// Returns the minimum of two expressions.
    fn min<R: Expr + Clone>(self, r: R) -> BinExpr<Self, R, MinOp> {
        BinExpr::new(self, r)
    }

    #[inline]
    /// Returns the maximum of two expressions.
    fn max<R: Expr + Clone>(self, r: R) -> BinExpr<Self, R, MaxOp> {
        BinExpr::new(self, r)
    }
}
impl<T: Expr + Clone> FloatExt for T {}

impl<A, O, Rhs> Add<Rhs> for UnExpr<A, O>
where
    Rhs: Expr + Clone,
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Rhs, AddOp>;
    fn add(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<A, O> Add<f64> for UnExpr<A, O>
where
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Const, AddOp>;
    fn add(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(rhs))
    }
}

impl<A, O, Rhs> Sub<Rhs> for UnExpr<A, O>
where
    Rhs: Expr + Clone,
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Rhs, SubOp>;
    fn sub(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<A, O> Sub<f64> for UnExpr<A, O>
where
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Const, SubOp>;
    fn sub(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(rhs))
    }
}

impl<A, O, Rhs> Mul<Rhs> for UnExpr<A, O>
where
    Rhs: Expr + Clone,
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Rhs, MulOp>;
    fn mul(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<A, O> Mul<f64> for UnExpr<A, O>
where
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Const, MulOp>;
    fn mul(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(rhs))
    }
}

impl<A, O, Rhs> Div<Rhs> for UnExpr<A, O>
where
    Rhs: Expr + Clone,
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Rhs, DivOp>;
    fn div(self, rhs: Rhs) -> Self::Output {
        BinExpr::new(self, rhs)
    }
}
impl<A, O> Div<f64> for UnExpr<A, O>
where
    Self: Expr + Clone,
{
    type Output = BinExpr<Self, Const, DivOp>;
    fn div(self, rhs: f64) -> Self::Output {
        BinExpr::new(self, Const(rhs))
    }
}

impl<A, O> Neg for UnExpr<A, O>
where
    Self: Expr + Clone,
{
    type Output = BinExpr<Const, Self, SubOp>;
    fn neg(self) -> Self::Output {
        BinExpr::new(Const(0.0), self)
    }
}

impl PartialEq<f64> for ADReal {
    #[inline]
    fn eq(&self, rhs: &f64) -> bool {
        self.value() == *rhs
    }
}

impl PartialOrd<f64> for ADReal {
    #[inline]
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.value().partial_cmp(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_tape_test<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().expect("test mutex poisoned");
        Tape::stop_recording();
        Tape::rewind_to_init();
        f();
        Tape::stop_recording();
    }

    #[test]
    fn compare_and_flatten() {
        with_tape_test(|| {
            let x = ADReal::new(5.0);
            let y = abs(x - 2.0);
            assert!(y > 2.0); // value-based comparison
            let z: ADReal = (y + 1.0).into();
            assert_eq!(z.value(), 4.0);
        });
    }

    #[test]
    fn backprop_basic() {
        with_tape_test(|| {
            Tape::start_recording();
            let a = ADReal::new(3.0);
            let b = ADReal::new(4.0);
            let expr = (a * b).sin();
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn test_late_tape_recording() {
        with_tape_test(|| {
            let mut a = ADReal::new(3.0);
            println!("a: {:?}", a);
            Tape::start_recording(); // start recording
            a.put_on_tape();
            let expr = a * a;
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(a.adjoint().unwrap(), 6.0);
        });
    }

    #[test]
    fn backprop_with_const() {
        with_tape_test(|| {
            Tape::start_recording();
            let a = ADReal::new(3.0);
            let b = Const(4.0);
            let expr = (a * b).sin();
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn tape_reset() {
        with_tape_test(|| {
            Tape::start_recording();
            let a = ADReal::new(3.0);
            let b = ADReal::new(4.0);
            let expr = (a * b).sin();
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap(), 1.0);

            Tape::reset_adjoints(); // reset adjoints
            assert_eq!(out.adjoint().unwrap(), 0.0); // should be zero now
        });
    }

    #[test]
    fn tape_propagate_mark() {
        with_tape_test(|| {
            Tape::start_recording();
            let a = ADReal::new(3.0);
            let b = ADReal::new(4.0);
            let expr = (a * b).sin();
            let out: ADReal = expr.into();
            out.backward_to_mark().unwrap(); // propagate to the current mark
            assert_eq!(out.adjoint().unwrap(), 1.0); // should be 1.0
        });
    }

    #[test]
    fn tape_backward_to_mark() {
        with_tape_test(|| {
            Tape::start_recording();
            let a = ADReal::new(3.0);
            let b = ADReal::new(4.0);
            let expr = (a * b).sin();
            let out: ADReal = expr.into();
            out.backward_to_mark().unwrap(); // propagate to the current mark
            assert_eq!(out.adjoint().unwrap(), 1.0); // should be 1.0

            out.backward().unwrap(); // propagate from mark to start
            assert_eq!(out.adjoint().unwrap(), 1.0); // should still be 1.0
        });
    }

    #[test]
    fn check_exp_derivate() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let expr = exp(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), f64::exp(2.0)); // derivative of exp(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_log_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let expr = log(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0 / 2.0); // derivative of log(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_sqrt_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(4.0);
            let expr = sqrt(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 0.5 / 2.0); // derivative of sqrt(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_sin_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(0.0);
            let expr = sin(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0); // derivative of sin(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_cos_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(0.0);
            let expr = cos(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 0.0); // derivative of cos(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_abs_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(-3.0);
            let expr = abs(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), -1.0); // derivative of abs(x) wrt x
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_pow_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let expr = pow(x, Const(3.0));
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 3.0 * 2.0f64.powf(2.0)); // derivative of x^3 wrt x at x=2
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_max_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let y = ADReal::new(3.0);
            let expr = max(x, y);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 0.0); // derivative wrt x
            assert_eq!(y.adjoint().unwrap(), 1.0); // derivative wrt y
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_min_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let y = ADReal::new(3.0);
            let expr = min(x, y);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0); // derivative wrt x
            assert_eq!(y.adjoint().unwrap(), 0.0); // derivative wrt y
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }
    #[test]
    fn check_flattening() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(5.0);
            let y = ADReal::new(3.0);
            let expr = (x + y) * 2.0;
            let out: ADReal = expr.into();
            assert_eq!(out.value(), 16.0); // (5 + 3) * 2 = 16
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap(), 1.0); // should be 1.0 after propagation
        });
    }

    #[test]
    fn check_add_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let y = ADReal::new(3.0);
            let expr = x + y;
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0);
            assert_eq!(y.adjoint().unwrap(), 1.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_sub_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(5.0);
            let y = ADReal::new(2.0);
            let expr = x - y;
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0);
            assert_eq!(y.adjoint().unwrap(), -1.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_mul_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(4.0);
            let y = ADReal::new(2.0);
            let expr = x * y;
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 2.0);
            assert_eq!(y.adjoint().unwrap(), 4.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_div_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(6.0);
            let y = ADReal::new(3.0);
            let expr = x / y;
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert!((x.adjoint().unwrap() - (1.0 / 3.0)).abs() < 1e-12);
            assert!((y.adjoint().unwrap() + (6.0 / 9.0)).abs() < 1e-12);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_fabs_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(-2.0);
            let expr = fabs(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), -1.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_pow_variable_exponent() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(2.0);
            let y = ADReal::new(3.0);
            let expr = pow(x, y);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 3.0 * 2.0f64.powf(2.0));
            assert!((y.adjoint().unwrap() - (8.0 * 2.0f64.ln())).abs() < 1e-12);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_max_derivative_x_greater() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(5.0);
            let y = ADReal::new(3.0);
            let expr = max(x, y);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0);
            assert_eq!(y.adjoint().unwrap(), 0.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_min_derivative_y_less() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(5.0);
            let y = ADReal::new(3.0);
            let expr = min(x, y);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 0.0);
            assert_eq!(y.adjoint().unwrap(), 1.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn check_abs_positive_derivative() {
        with_tape_test(|| {
            Tape::start_recording();
            let x = ADReal::new(3.0);
            let expr = abs(x);
            let out: ADReal = expr.into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap(), 1.0);
            assert_eq!(out.adjoint().unwrap(), 1.0);
        });
    }

    #[test]
    fn test_reassigning() {
        with_tape_test(|| {
            Tape::start_recording();

            let a0 = ADReal::new(5.0); // keep an immutable handle to the leaf
            let b = ADReal::new(3.0);
            let mut a = a0; // mutable alias
            a *= b; // 5 * 3 = 15
            let c = a;
            assert_eq!(c.value(), 15.0); // value is correct

            c.backward().unwrap();

            assert_eq!(a0.adjoint().unwrap(), 3.0); // ∂c/∂a0  = b = 3
            assert_eq!(b.adjoint().unwrap(), 5.0); // ∂c/∂b   = a0 = 5
            assert_eq!(c.adjoint().unwrap(), 1.0); // seed stays 1
        });
    }

    #[test]
    fn multithread_recording_derivatives() {
        with_tape_test(|| {
            let handle = std::thread::spawn(|| {
                Tape::start_recording();
                let x = ADReal::new(2.0);
                let y = ADReal::new(3.0);
                let expr = x * y + x;
                let out: ADReal = expr.into();
                out.backward().unwrap();
                (
                    x.adjoint().unwrap(),
                    y.adjoint().unwrap(),
                    out.adjoint().unwrap(),
                )
            });

            let (dx, dy, dout) = handle.join().unwrap();
            assert_eq!(dx, 4.0); // d/dx (x*y + x) = y + 1 = 4
            assert_eq!(dy, 2.0); // d/dy (x*y + x) = x = 2
            assert_eq!(dout, 1.0); // seed stays 1
        });
    }
}
