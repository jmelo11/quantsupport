//! One-time numerical recording frontend for the Cranelift backend.
//!
//! `Recorded` implements the existing QS `Scalar` and `InnerScalar`
//! contracts so scalar-generic numerical code can be executed *once* to
//! capture operations, without constructing a runtime reverse-mode tape.
//! Input-dependent Rust branches and extraction of primal values are rejected:
//! the initial compiler only supports replay-safe straight-line arithmetic.

use std::{
    cell::RefCell,
    cmp::Ordering,
    fmt,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

use crate::ad::scalar::Scalar;

use super::CompileError;

#[derive(Clone, Debug)]
pub(crate) enum Op {
    Input(usize),
    Constant(f64),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Div(usize, usize),
    Neg(usize),
    Exp(usize),
    Ln(usize),
    Sqrt(usize),
    Sin(usize),
    Cos(usize),
    Abs(usize),
    Pow(usize, usize),
    Max(usize, usize),
    Min(usize, usize),
}

/// Immutable, replayable numerical program recorded once from scalar-generic
/// Rust arithmetic. The graph contains operations, not derivative weights.
#[derive(Clone, Debug)]
pub struct Graph {
    pub(crate) ops: Vec<Op>,
    pub(crate) inputs: usize,
    pub(crate) output: usize,
}

struct Trace {
    token: u64,
    ops: Vec<Op>,
    inputs: usize,
    observed_primal: bool,
}

thread_local! {
    static CURRENT: RefCell<Option<Trace>> = const { RefCell::new(None) };
}
static NEXT_TRACE: AtomicU64 = AtomicU64::new(1);

struct TraceCleanup;
impl Drop for TraceCleanup {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = None);
    }
}

/// A numerical value only valid during `Graph::trace`.
///
/// `Recorded` is `Copy` and implements the existing QS scalar contracts,
/// allowing ordinary generic numerical formulas to be captured. Its `id`
/// refers to a compiler IR operation, not a tape node or adjoint buffer.
#[derive(Clone, Copy, Debug)]
pub struct Recorded {
    id: usize,
    token: u64,
    primal: f64,
}

impl Graph {
    /// Records a scalar-generic numerical function using sample inputs.
    ///
    /// The samples determine primal values during recording but are NOT baked
    /// into the graph: each input becomes a load from a runtime input slot.
    /// Branching on recorded values, including calling `Scalar::value`,
    /// returns an error rather than silently specializing an unsafe trace.
    ///
    /// # Errors
    /// Rejects nested recordings, observed primal values and foreign outputs.
    ///
    /// # Panics
    /// Panics if a `Recorded` value from a different recording is mixed in,
    /// or if a numerical operation is attempted outside an active recording.
    pub fn trace<F>(example_inputs: &[f64], f: F) -> Result<Self, CompileError>
    where
        F: FnOnce(&[Recorded]) -> Recorded,
    {
        let token = NEXT_TRACE.fetch_add(1, AtomicOrdering::Relaxed);
        CURRENT.with(|slot| {
            let mut active = slot.borrow_mut();
            if active.is_some() {
                return Err(CompileError::NestedCapture);
            }
            *active = Some(Trace {
                token,
                ops: Vec::new(),
                inputs: example_inputs.len(),
                observed_primal: false,
            });
            Ok(())
        })?;
        let _cleanup = TraceCleanup;
        let inputs: Vec<Recorded> = example_inputs
            .iter()
            .enumerate()
            .map(|(idx, &v)| emit(Op::Input(idx), v))
            .collect();
        let output = f(&inputs);
        let trace = CURRENT.with(|slot| {
            slot.borrow_mut()
                .take()
                .ok_or(CompileError::CaptureMissing)
        })?;
        if output.token != token {
            return Err(CompileError::ForeignValue);
        }
        if trace.observed_primal {
            return Err(CompileError::DynamicControlFlow);
        }
        Ok(Self {
            ops: trace.ops,
            inputs: trace.inputs,
            output: output.id,
        })
    }

    /// Number of runtime numerical input slots.
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.inputs
    }

    /// Number of recorded operations, including inputs/constants.
    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.ops.len()
    }
}

fn emit(op: Op, primal: f64) -> Recorded {
    CURRENT.with(|slot| {
        let mut active = slot.borrow_mut();
        let trace = active.as_mut().expect("Recorded requires an active graph capture");
        let id = trace.ops.len();
        trace.ops.push(op);
        Recorded {
            id,
            token: trace.token,
            primal,
        }
    })
}

fn observe_primal() {
    CURRENT.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() {
            trace.observed_primal = true;
        }
    });
}

impl Recorded {
    fn unary(self, op: impl FnOnce(usize) -> Op, primal: f64) -> Self {
        self.assert_current();
        emit(op(self.id), primal)
    }

    fn binary(self, rhs: Self, op: impl FnOnce(usize, usize) -> Op, primal: f64) -> Self {
        self.assert_current();
        rhs.assert_current();
        assert_eq!(self.token, rhs.token, "cannot mix graph captures");
        emit(op(self.id, rhs.id), primal)
    }

    fn assert_current(self) {
        CURRENT.with(|slot| {
            assert_eq!(
                slot.borrow().as_ref().map(|s| s.token),
                Some(self.token),
                "Recorded value used outside its graph capture"
            );
        });
    }
}

impl Default for Recorded {
    fn default() -> Self {
        Self::from(0.0)
    }
}
impl From<f64> for Recorded {
    fn from(value: f64) -> Self {
        emit(Op::Constant(value), value)
    }
}
impl fmt::Display for Recorded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.primal.fmt(f)
    }
}
impl PartialEq for Recorded {
    fn eq(&self, rhs: &Self) -> bool {
        observe_primal();
        self.primal == rhs.primal
    }
}
impl PartialOrd for Recorded {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        observe_primal();
        self.primal.partial_cmp(&rhs.primal)
    }
}
impl PartialEq<f64> for Recorded {
    fn eq(&self, rhs: &f64) -> bool {
        observe_primal();
        self.primal == *rhs
    }
}
impl PartialOrd<f64> for Recorded {
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        observe_primal();
        self.primal.partial_cmp(rhs)
    }
}

macro_rules! binary_op {
    ($trait:ident, $method:ident, $variant:ident, $operation:tt) => {
        impl $trait for Recorded {
            type Output = Self;
            fn $method(self, rhs: Self) -> Self {
                self.binary(rhs, Op::$variant, self.primal $operation rhs.primal)
            }
        }
        impl $trait<f64> for Recorded {
            type Output = Self;
            fn $method(self, rhs: f64) -> Self {
                self.$method(Self::from(rhs))
            }
        }
        impl $trait<Recorded> for f64 {
            type Output = Recorded;
            fn $method(self, rhs: Recorded) -> Recorded {
                Recorded::from(self).$method(rhs)
            }
        }
    };
}

binary_op!(Add, add, Add, +);
binary_op!(Sub, sub, Sub, -);
binary_op!(Mul, mul, Mul, *);
binary_op!(Div, div, Div, /);

impl Neg for Recorded {
    type Output = Self;
    fn neg(self) -> Self {
        self.unary(Op::Neg, -self.primal)
    }
}

macro_rules! assign_op {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for Recorded {
            fn $method(&mut self, rhs: Self) {
                *self = *self $operator rhs;
            }
        }
        impl $trait<f64> for Recorded {
            fn $method(&mut self, rhs: f64) {
                *self = *self $operator rhs;
            }
        }
    };
}
assign_op!(AddAssign, add_assign, +);
assign_op!(SubAssign, sub_assign, -);
assign_op!(MulAssign, mul_assign, *);
assign_op!(DivAssign, div_assign, /);

impl Scalar for Recorded {
    fn scalar(v: f64) -> Self { Self::from(v) }
    fn value(&self) -> f64 {
        observe_primal();
        self.primal
    }
    fn zero() -> Self { Self::from(0.0) }
    fn one() -> Self { Self::from(1.0) }
    fn exp(self) -> Self { self.unary(Op::Exp, self.primal.exp()) }
    fn ln(self) -> Self { self.unary(Op::Ln, self.primal.ln()) }
    fn sqrt(self) -> Self { self.unary(Op::Sqrt, self.primal.sqrt()) }
    fn sin(self) -> Self { self.unary(Op::Sin, self.primal.sin()) }
    fn cos(self) -> Self { self.unary(Op::Cos, self.primal.cos()) }
    fn abs(self) -> Self { self.unary(Op::Abs, self.primal.abs()) }
    fn powf(self, p: f64) -> Self { self.pows(Self::from(p)) }
    fn pows(self, p: Self) -> Self {
        self.binary(p, Op::Pow, self.primal.powf(p.primal))
    }
    fn max_val(self, other: Self) -> Self {
        self.binary(other, Op::Max, self.primal.max(other.primal))
    }
    fn min_val(self, other: Self) -> Self {
        self.binary(other, Op::Min, self.primal.min(other.primal))
    }
    fn add_val(self, other: Self) -> Self { self + other }
    fn sub_val(self, other: Self) -> Self { self - other }
    fn mul_val(self, other: Self) -> Self { self * other }
    fn div_val(self, other: Self) -> Self { self / other }
    fn neg_val(self) -> Self { -self }
}

impl crate::ad::scalar::InnerScalar for Recorded {}
