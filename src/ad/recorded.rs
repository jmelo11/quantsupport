//! One-time numerical recording frontend for the Cranelift backend.
//!
//! `Recorded` implements the existing QS `Scalar` and `InnerScalar`
//! contracts so scalar-generic numerical code can be executed *once* to
//! capture operations without pointer-linked nodes or stored local derivatives.
//! `Graph` rejects input-dependent Rust branches. `CompiledProgram` retains
//! ordinary comparison results as replay guards. Numerical selections allow
//! branches to vary during execution. Reusable captures reject primal-value
//! extraction; per-call pricing captures permit it and recapture current branches.

use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::{Hash, Hasher},
    mem::{discriminant, take, DropGuard},
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    sync::Arc,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    ad::scalar::{InnerScalar, Scalar},
    math::probability::normcdf::NormCDF,
    utils::errors::{QSError, Result},
};

use super::{
    comparisonguard::{Comparison, ComparisonGuard},
    gradientkernel::GradientKernel,
    graph::Graph,
    operation::Operation as Op,
};

struct Trace {
    token: u64,
    ops: Vec<Op>,
    inputs: usize,
    observed_primal: bool,
    comparison_guards: Vec<ComparisonGuard>,
    strict: bool,
    enabled: bool,
    input_values: Vec<f64>,
    gradients: Vec<f64>,
    checkpoint: (usize, usize, usize),
    aliases: HashMap<usize, usize>,
    dynamic: Vec<bool>,
}

/// An opaque saved compiler capture, owned by an isolated recording scope.
pub struct SavedRecording(Option<Trace>);

struct CachedProgram {
    graph: Graph,
    active: usize,
    kernel: Arc<GradientKernel>,
}

thread_local! {
    static PROGRAMS: RefCell<HashMap<u64, Vec<CachedProgram>>> = RefCell::new(HashMap::new());
}

thread_local! {
    static CURRENT: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

static NEXT_TRACE: AtomicU64 = AtomicU64::new(1);

/// A persistent numerical market value or an expression in a compiler capture.
///
/// `new` registers an independent input while a pricing capture is active;
/// `from`, `constant` and `detached` construct constants. Market pillars are
/// registered at pricing boundaries. Strict `Graph` capture values must stay
/// within their capture lifetime. Derivatives execute native code.
#[derive(Clone, Copy)]
pub struct Recorded {
    id: usize,
    token: u64,
    primal: f64,
    strict: bool,
}

impl Recorded {
    /// Returns this value's operation index while its capture is active.
    ///
    /// # Panics
    /// Panics when called outside the value's owning capture.
    #[must_use]
    pub fn operation_index(self) -> usize {
        self.resolve().id
    }

    /// Selects a graph value at execution time using a strict greater-than comparison.
    ///
    /// Comparison inputs have zero derivative. Unlike a Rust branch, this can
    /// change its selected arm when a compiled program is replayed.
    ///
    /// # Panics
    /// Panics if any operand belongs to another or an inactive capture.
    #[must_use]
    pub fn select_greater(self, rhs: Self, yes: Self, no: Self) -> Self {
        let (left, rhs, yes, no) = (self.resolve(), rhs.resolve(), yes.resolve(), no.resolve());
        emit(
            Op::SelectGreater(left.id, rhs.id, yes.id, no.id),
            if self.primal > rhs.primal {
                yes.primal
            } else {
                no.primal
            },
        )
    }

    /// Records a scalar-generic numerical function using sample inputs.
    ///
    /// The samples supply numerical values while the closure is captured.
    /// Each input becomes a runtime slot, so later executions can supply new
    /// values. Input-dependent Rust branches and calls to `Scalar::value`
    /// invalidate this reusable capture and cause it to return an error.
    ///
    /// # Errors
    /// Rejects nested strict captures, observed primal values and foreign outputs.
    ///
    /// # Panics
    /// Panics if a `Recorded` value from a different recording is mixed in,
    /// or if a numerical operation is attempted outside an active recording.
    pub fn capture<F>(example_inputs: &[f64], f: F) -> Result<Graph>
    where
        F: FnOnce(&[Self]) -> Self,
    {
        let saved = CURRENT.with(|slot| {
            let mut current = slot.borrow_mut();
            if current.as_ref().is_some_and(|trace| trace.strict) {
                return Err(QSError::NestedCapture);
            }
            Ok(current.take())
        })?;
        let token = NEXT_TRACE.fetch_add(1, AtomicOrdering::Relaxed);
        CURRENT.with(|slot| *slot.borrow_mut() = Some(Trace::new(token, true)));
        let _cleanup = DropGuard::new(saved, |previous| {
            CURRENT.with(|slot| *slot.borrow_mut() = previous);
        });
        let inputs: Vec<Self> = example_inputs
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                CURRENT.with(|slot| {
                    if let Some(trace) = slot.borrow_mut().as_mut() {
                        trace.inputs += 1;
                        trace.input_values.push(value);
                    }
                });
                emit(Op::Input(index), value)
            })
            .collect();
        let output = f(&inputs);
        let trace = CURRENT.with(|slot| slot.borrow_mut().take().ok_or(QSError::CaptureMissing))?;
        if output.token != token {
            return Err(QSError::ForeignValue);
        }
        if trace.observed_primal || !trace.comparison_guards.is_empty() {
            return Err(QSError::DynamicControlFlow);
        }
        Graph::new(trace.ops, trace.inputs, output.id)
    }
}

impl Trace {
    /// Creates an empty compiler capture with a fresh owner token and capture policy.
    fn new(token: u64, strict: bool) -> Self {
        Self {
            token,
            ops: Vec::new(),
            inputs: 0,
            observed_primal: false,
            comparison_guards: Vec::new(),
            strict,
            enabled: true,
            input_values: Vec::new(),
            gradients: Vec::new(),
            checkpoint: (0, 0, 0),
            aliases: HashMap::new(),
            dynamic: Vec::new(),
        }
    }
}

/// Appends an instruction to the active capture or returns a detached constant.
fn emit(op: Op, primal: f64) -> Recorded {
    CURRENT.with(|slot| {
        let mut active = slot.borrow_mut();
        let Some(trace) = active.as_mut().filter(|trace| trace.enabled) else {
            return Recorded::detached(primal);
        };
        let id = trace.ops.len();
        let dynamic = matches!(op, Op::Input(_))
            || op.operands().iter().any(|&operand| trace.dynamic[operand]);
        trace.dynamic.push(dynamic);
        trace.ops.push(op);
        Recorded {
            id,
            token: trace.token,
            primal,
            strict: trace.strict,
        }
    })
}

/// Marks forbidden observations of input-dependent values in strict captures.
fn observe_primal(value: &Recorded) {
    CURRENT.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() {
            trace.observed_primal |= trace.strict
                && value.token == trace.token
                && trace.dynamic.get(value.id).copied().unwrap_or(false);
        }
    });
}

impl Recorded {
    /// Transfers observed Rust comparisons to a program's replay checks.
    #[must_use]
    pub fn take_comparison_guards() -> Vec<ComparisonGuard> {
        CURRENT.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .map_or_else(Vec::new, |trace| take(&mut trace.comparison_guards))
        })
    }

    /// Records the operand identities and observed boolean result of a Rust comparison.
    fn compare(self, rhs: Self, comparison: Comparison) -> bool {
        let left = self.resolve();
        let rhs = rhs.resolve();
        let expected = comparison.evaluate(self.primal, rhs.primal);
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace
                    .comparison_guards
                    .push(ComparisonGuard::new(left.id, rhs.id, comparison, expected));
            }
        });
        expected
    }

    /// Records a unary instruction when its operand belongs to the active capture.
    fn unary(self, op: impl FnOnce(usize) -> Op, primal: f64) -> Self {
        if self.token == 0 {
            return Self::constant(primal);
        }
        let value = self.resolve();
        emit(op(value.id), primal)
    }

    /// Records a binary instruction or folds arithmetic between detached constants.
    fn binary(self, rhs: Self, op: impl FnOnce(usize, usize) -> Op, primal: f64) -> Self {
        if self.token == 0 && rhs.token == 0 {
            return Self::constant(primal);
        }
        let left = self.resolve();
        let right = rhs.resolve();
        emit(op(left.id, right.id), primal)
    }

    /// Checks capture ownership and lifts persistent market numbers as current constants.
    fn resolve(self) -> Self {
        let current = CURRENT.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|trace| (trace.token, trace.enabled, trace.strict, trace.ops.len()))
        });
        if let Some((token, enabled, strict, len)) = current {
            if enabled && token == self.token && self.id < len {
                return self;
            }
            assert!(
                !(self.strict || strict && self.token != 0),
                "Recorded value used outside its graph capture"
            );
            if enabled {
                return emit(Op::Constant(self.primal), self.primal);
            }
        } else {
            assert!(
                !self.strict,
                "Recorded value used outside its graph capture"
            );
        }
        Self::detached(self.primal)
    }
}

impl Default for Recorded {
    /// Creates a constant zero without registering a market input.
    fn default() -> Self {
        Self::from(0.0)
    }
}
impl From<f32> for Recorded {
    /// Converts a plain numerical value into a constant compiler scalar.
    fn from(value: f32) -> Self {
        Self::from(f64::from(value))
    }
}
impl From<i32> for Recorded {
    /// Converts a plain numerical value into a constant compiler scalar.
    fn from(value: i32) -> Self {
        Self::from(f64::from(value))
    }
}
impl From<u32> for Recorded {
    /// Converts a plain numerical value into a constant compiler scalar.
    fn from(value: u32) -> Self {
        Self::from(f64::from(value))
    }
}
impl From<f64> for Recorded {
    /// Converts a plain numerical value into a constant compiler scalar.
    fn from(value: f64) -> Self {
        Self::constant(value)
    }
}

impl fmt::Display for Recorded {
    /// Formats the primal value and tracks strict-capture observation violations.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        observe_primal(self);
        self.primal.fmt(f)
    }
}
impl fmt::Debug for Recorded {
    /// Formats the primal value and tracks strict-capture observation violations.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        observe_primal(self);
        f.debug_struct("Recorded")
            .field("id", &self.id)
            .field("token", &self.token)
            .field("primal", &self.primal)
            .field("strict", &self.strict)
            .finish()
    }
}
impl PartialEq for Recorded {
    /// Observes an equality decision and records any required replay guard.
    fn eq(&self, rhs: &Self) -> bool {
        self.compare(*rhs, Comparison::Equal)
    }
}
impl PartialOrd for Recorded {
    /// Observes IEEE ordering and records the corresponding replay decision.
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        let ordering = self.primal.partial_cmp(&rhs.primal);
        let comparison = match ordering {
            Some(Ordering::Less) => Comparison::Less,
            Some(Ordering::Equal) => Comparison::Equal,
            Some(Ordering::Greater) => Comparison::Greater,
            None => Comparison::Unordered,
        };
        let _ = self.compare(*rhs, comparison);
        ordering
    }
    /// Observes strict less-than and records any required replay guard.
    fn lt(&self, rhs: &Self) -> bool {
        self.compare(*rhs, Comparison::Less)
    }
    /// Observes less-than-or-equal and records any required replay guard.
    fn le(&self, rhs: &Self) -> bool {
        self.compare(*rhs, Comparison::LessEqual)
    }
    /// Observes strict greater-than and records any required replay guard.
    fn gt(&self, rhs: &Self) -> bool {
        self.compare(*rhs, Comparison::Greater)
    }
    /// Observes greater-than-or-equal and records any required replay guard.
    fn ge(&self, rhs: &Self) -> bool {
        self.compare(*rhs, Comparison::GreaterEqual)
    }
}
impl PartialEq<f64> for Recorded {
    /// Observes an equality decision and records any required replay guard.
    fn eq(&self, rhs: &f64) -> bool {
        self.compare(Self::from(*rhs), Comparison::Equal)
    }
}
impl PartialOrd<f64> for Recorded {
    /// Observes IEEE ordering and records the corresponding replay decision.
    fn partial_cmp(&self, rhs: &f64) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*rhs))
    }
    /// Observes strict less-than and records any required replay guard.
    fn lt(&self, rhs: &f64) -> bool {
        self.compare(Self::from(*rhs), Comparison::Less)
    }
    /// Observes less-than-or-equal and records any required replay guard.
    fn le(&self, rhs: &f64) -> bool {
        self.compare(Self::from(*rhs), Comparison::LessEqual)
    }
    /// Observes strict greater-than and records any required replay guard.
    fn gt(&self, rhs: &f64) -> bool {
        self.compare(Self::from(*rhs), Comparison::Greater)
    }
    /// Observes greater-than-or-equal and records any required replay guard.
    fn ge(&self, rhs: &f64) -> bool {
        self.compare(Self::from(*rhs), Comparison::GreaterEqual)
    }
}

macro_rules! binary_op {
    ($trait:ident, $method:ident, $variant:ident, $operation:tt) => {
        impl $trait for Recorded {
            type Output = Self;
            /// Applies this arithmetic operator while preserving compiler capture dependencies.
            fn $method(self, rhs: Self) -> Self {
                self.binary(rhs, Op::$variant, self.primal $operation rhs.primal)
            }
        }
        impl $trait<f64> for Recorded {
            type Output = Self;
            /// Applies this arithmetic operator while preserving compiler capture dependencies.
            fn $method(self, rhs: f64) -> Self {
                self.$method(Self::constant(rhs))
            }
        }
        impl $trait<Recorded> for f64 {
            type Output = Recorded;
            /// Applies this arithmetic operator while preserving compiler capture dependencies.
            fn $method(self, rhs: Recorded) -> Recorded {
                Recorded::constant(self).$method(rhs)
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
    /// Records unary negation or folds a detached constant.
    fn neg(self) -> Self {
        self.unary(Op::Neg, -self.primal)
    }
}

macro_rules! assign_op {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for Recorded {
            /// Applies this arithmetic operator while preserving compiler capture dependencies.
            fn $method(&mut self, rhs: Self) {
                *self = *self $operator rhs;
            }
        }
        impl $trait<f64> for Recorded {
            /// Applies this arithmetic operator while preserving compiler capture dependencies.
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
    /// Selects a numerical arm using a strict greater-than predicate.
    fn select_greater(self, rhs: Self, yes: Self, no: Self) -> Self {
        Self::select_greater(self, rhs, yes, no)
    }
    /// Constructs a constant from a plain numerical value.
    fn scalar(v: f64) -> Self {
        Self::from(v)
    }
    /// Returns the current numerical value under the scalar observation rules.
    fn value(&self) -> f64 {
        observe_primal(self);
        self.primal
    }
    /// Returns the additive identity as a constant scalar.
    fn zero() -> Self {
        Self::from(0.0)
    }
    /// Returns the multiplicative identity as a constant scalar.
    fn one() -> Self {
        Self::from(1.0)
    }
    /// Calculates the exponential with the scalar implementation.
    fn exp(self) -> Self {
        self.unary(Op::Exp, self.primal.exp())
    }
    /// Calculates the natural logarithm with the scalar implementation.
    fn ln(self) -> Self {
        self.unary(Op::Ln, self.primal.ln())
    }
    /// Calculates the square root with the scalar implementation.
    fn sqrt(self) -> Self {
        self.unary(Op::Sqrt, self.primal.sqrt())
    }
    /// Calculates the sine of an angle in radians.
    fn sin(self) -> Self {
        self.unary(Op::Sin, self.primal.sin())
    }
    /// Calculates the cosine of an angle in radians.
    fn cos(self) -> Self {
        self.unary(Op::Cos, self.primal.cos())
    }
    /// Calculates the absolute value under the scalar derivative convention.
    fn abs(self) -> Self {
        self.unary(Op::Abs, self.primal.abs())
    }
    /// Raises this scalar to a constant floating-point exponent.
    #[allow(clippy::float_cmp)] // Exact literal powers are algebraic identities.
    fn powf(self, p: f64) -> Self {
        if p == 0.0 {
            Self::from(1.0)
        } else if p == 1.0 {
            self
        } else {
            self.pows(Self::from(p))
        }
    }
    /// Raises this scalar to a scalar exponent, retaining both input dependencies.
    fn pows(self, p: Self) -> Self {
        self.binary(p, Op::Pow, self.primal.powf(p.primal))
    }
    /// Selects the larger value under the library's NaN and derivative conventions.
    fn max_val(self, other: Self) -> Self {
        self.binary(other, Op::Max, self.primal.max(other.primal))
    }
    /// Selects the smaller value under the library's NaN and derivative conventions.
    fn min_val(self, other: Self) -> Self {
        self.binary(other, Op::Min, self.primal.min(other.primal))
    }
    /// Adds two scalars while retaining their numerical dependencies.
    fn add_val(self, other: Self) -> Self {
        self + other
    }
    /// Subtracts two scalars while retaining their numerical dependencies.
    fn sub_val(self, other: Self) -> Self {
        self - other
    }
    /// Multiplies two scalars while retaining their numerical dependencies.
    fn mul_val(self, other: Self) -> Self {
        self * other
    }
    /// Divides two scalars while retaining their numerical dependencies.
    fn div_val(self, other: Self) -> Self {
        self / other
    }
    /// Negates a scalar while retaining its numerical dependencies.
    fn neg_val(self) -> Self {
        -self
    }
}

impl InnerScalar for Recorded {}

impl NormCDF for Recorded {
    /// Calculates the standard normal CDF with compiled analytic derivatives.
    fn norm_cdf(self) -> Self {
        self.unary(Op::NormalCdf, self.primal.norm_cdf())
    }
}

impl Recorded {
    /// Creates an independent input in an active pricing capture, or a detached market value.
    #[must_use]
    pub fn new(value: f64) -> Self {
        let index = CURRENT.with(|slot| {
            let mut state = slot.borrow_mut();
            state
                .as_mut()
                .filter(|state| state.enabled && !state.strict)
                .map(|state| {
                    let index = state.inputs;
                    state.inputs += 1;
                    state.input_values.push(value);
                    state.gradients.push(0.0);
                    index
                })
        });
        index.map_or_else(
            || Self::constant(value),
            |index| emit(Op::Input(index), value),
        )
    }
    /// Creates a numerical constant with no market derivative.
    #[must_use]
    pub fn constant(value: f64) -> Self {
        if CURRENT.with(|slot| slot.borrow().as_ref().is_some_and(|trace| trace.strict)) {
            emit(Op::Constant(value), value)
        } else {
            Self::detached(value)
        }
    }
    /// Returns a persistent numerical value without capture ownership.
    #[must_use]
    pub const fn detached(value: f64) -> Self {
        Self {
            id: usize::MAX,
            token: 0,
            primal: value,
            strict: false,
        }
    }
    /// Current numeric value. Strict graph captures reject observing a primal.
    #[must_use]
    pub fn value(&self) -> f64 {
        observe_primal(self);
        self.primal
    }
    /// Registers this market observation as an input of the current capture.
    pub fn register_input(&mut self) {
        *self = Self::new(self.primal);
    }
    /// Whether this observation belongs to the current pricing capture.
    #[must_use]
    pub fn is_recorded(&self) -> bool {
        CURRENT.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_some_and(|trace| self.token == trace.token && self.id < trace.ops.len())
        })
    }
    /// Capture-local observation identity; no pointer or node allocation is exposed.
    #[must_use]
    pub fn identity(&self) -> Option<(u64, usize)> {
        self.is_recorded().then_some((self.token, self.id))
    }
    /// Attaches a detached observation to the current capture when needed.
    #[must_use]
    pub fn ensure_recorded(&self) -> Self {
        if self.is_recorded() {
            *self
        } else {
            Self::new(self.primal)
        }
    }
    /// Returns the derivative produced by the last native differentiation.
    ///
    /// # Errors
    /// Rejects observations outside the active capture or non-input observations.
    pub fn derivative(&self) -> Result<Self> {
        CURRENT.with(|slot| {
            let state = slot.borrow();
            let trace = state
                .as_ref()
                .ok_or_else(|| QSError::EvaluationErr("no pricing capture".into()))?;
            if trace.token != self.token {
                return Err(QSError::EvaluationErr(
                    "observation belongs to another capture".into(),
                ));
            }
            match trace.ops.get(self.id) {
                Some(Op::Input(index)) => Ok(Self::detached(
                    trace.gradients.get(*index).copied().unwrap_or(0.0),
                )),
                _ => Err(QSError::EvaluationErr(
                    "derivative requested for a non-input observation".into(),
                )),
            }
        })
    }
    /// Executes native reverse differentiation, replacing the current input gradient.
    ///
    /// # Errors
    /// Propagates capture, graph validation, compilation and buffer errors.
    pub fn differentiate(&self) -> Result<()> {
        Self::reset_derivatives();
        self.differentiate_accumulating()
    }
    /// Executes native reverse differentiation and adds the result to input derivatives.
    ///
    /// # Errors
    /// Propagates capture, graph validation and native execution errors.
    pub fn differentiate_accumulating(&self) -> Result<()> {
        let gradients = self
            .native_gradient()
            .map_err(|error| error.context("differentiating recorded value"))?;
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace.gradients.resize(trace.inputs, 0.0);
                for (sum, value) in trace.gradients.iter_mut().zip(gradients) {
                    *sum += value;
                }
            }
        });
        Ok(())
    }
    /// Exports the reachable expression and executes its cached native gradient.
    fn native_gradient(&self) -> Result<Vec<f64>> {
        let program = CURRENT.with(|slot| {
            let state = slot.borrow();
            let trace = state.as_ref().ok_or(QSError::CaptureMissing)?;
            if self.token == 0 {
                return Ok(None);
            }
            if trace.token != self.token {
                return Err(QSError::ForeignValue);
            }
            trace.export(self.id).map(Some)
        })?;
        let Some((graph, inputs, active)) = program else {
            return Ok(CURRENT.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .map_or_else(Vec::new, |trace| vec![0.0; trace.inputs])
            }));
        };
        let kernel = cached_program(&graph, active)?;
        let mut gradient = vec![0.0; active];
        kernel.execute(&inputs, &mut gradient)?;
        Ok(gradient)
    }
    /// Calculates a setup/calibration Jacobian using compiled reverse kernels.
    ///
    /// # Errors
    /// Propagates graph and native differentiation errors.
    pub fn jacobian(inputs: &[Self], outputs: &[Self]) -> Result<Vec<Vec<f64>>> {
        let indices = CURRENT.with(|slot| {
            let state = slot.borrow();
            inputs
                .iter()
                .map(|input| {
                    state.as_ref().and_then(|trace| {
                        (trace.token == input.token)
                            .then(|| trace.ops.get(input.id))
                            .flatten()
                            .and_then(|op| {
                                if let Op::Input(index) = op {
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                    })
                })
                .collect::<Vec<_>>()
        });
        outputs
            .iter()
            .map(|output| {
                if !output.is_recorded() {
                    return Ok(vec![0.0; inputs.len()]);
                }
                let gradient = output
                    .native_gradient()
                    .map_err(|error| error.context("calculating recorded Jacobian"))?;
                Ok(indices
                    .iter()
                    .map(|index| {
                        index
                            .and_then(|index| gradient.get(index).copied())
                            .unwrap_or(0.0)
                    })
                    .collect())
            })
            .collect()
    }
    /// Installs a calibrated first-order map using ordinary compiler arithmetic.
    pub fn set_linearization(&self, inputs: &[Self], derivatives: &[f64]) {
        let mut expression = Self::constant(self.primal);
        for (&input, &derivative) in inputs.iter().zip(derivatives) {
            if derivative != 0.0 {
                expression += (input - Self::constant(input.primal)) * derivative;
            }
        }
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                if trace.token == self.token {
                    trace.aliases.insert(self.id, expression.id);
                }
            }
        });
    }
    /// Starts a fresh compiler capture for an existing pricing operation.
    pub fn begin_recording() {
        let token = NEXT_TRACE.fetch_add(1, AtomicOrdering::Relaxed);
        CURRENT.with(|slot| *slot.borrow_mut() = Some(Trace::new(token, false)));
    }
    /// Ends IR construction while retaining native derivative results.
    pub fn end_recording() {
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace.enabled = false;
            }
        });
    }
    /// Returns whether a pricing capture currently records numerical operations.
    #[must_use]
    pub fn recording_enabled() -> bool {
        CURRENT.with(|slot| slot.borrow().as_ref().is_some_and(|trace| trace.enabled))
    }
    /// Clears a pricing capture while preserving whether recording is enabled.
    pub fn clear_recording() {
        let enabled = Self::recording_enabled();
        Self::begin_recording();
        if !enabled {
            Self::end_recording();
        }
    }
    /// Marks the immutable setup prefix so path IR can be discarded after execution.
    pub fn checkpoint_recording() {
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace.checkpoint = (trace.ops.len(), trace.inputs, trace.comparison_guards.len());
            }
        });
    }
    /// Discards transient path IR while retaining setup inputs and accumulated native risks.
    pub fn restart_recording() {
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                let (ops, inputs, guards) = trace.checkpoint;
                trace.ops.truncate(ops);
                trace.dynamic.truncate(ops);
                trace.inputs = inputs;
                trace.input_values.truncate(inputs);
                trace.gradients.truncate(inputs);
                trace.comparison_guards.truncate(guards);
                trace
                    .aliases
                    .retain(|&key, value| key < ops && *value < ops);
            }
        });
    }
    /// Releases a setup checkpoint without changing its accumulated input risks.
    pub fn release_recording_checkpoint() {
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace.checkpoint = (0, 0, 0);
            }
        });
    }
    /// Clears previously calculated input derivatives.
    pub fn reset_derivatives() {
        CURRENT.with(|slot| {
            if let Some(trace) = slot.borrow_mut().as_mut() {
                trace.gradients.fill(0.0);
            }
        });
    }
    /// Temporarily removes an enclosing pricing capture for isolated market construction.
    #[must_use]
    pub fn suspend_recording() -> SavedRecording {
        SavedRecording(CURRENT.with(|slot| slot.borrow_mut().take()))
    }
    /// Restores the capture preserved by an isolated recording scope.
    pub fn restore_recording(saved: SavedRecording) {
        CURRENT.with(|slot| *slot.borrow_mut() = saved.0);
    }
    /// Returns the operation count, checkpoint position and recording state.
    #[must_use]
    pub fn recording_state() -> (usize, usize, bool) {
        CURRENT.with(|slot| {
            slot.borrow().as_ref().map_or((0, 0, false), |trace| {
                (trace.ops.len(), trace.checkpoint.0, trace.enabled)
            })
        })
    }
    /// Returns the operation count in the current capture for diagnostic reporting.
    #[must_use]
    pub fn recording_operation_count() -> usize {
        CURRENT.with(|slot| slot.borrow().as_ref().map_or(0, |trace| trace.ops.len()))
    }
    /// Calculates the absolute value using the corresponding compiler instruction.
    #[must_use]
    pub fn abs(self) -> Self {
        <Self as Scalar>::abs(self)
    }
    /// Calculates the exponential using the corresponding compiler instruction.
    #[must_use]
    pub fn exp(self) -> Self {
        <Self as Scalar>::exp(self)
    }
    /// Calculates the natural logarithm using the corresponding compiler instruction.
    #[must_use]
    pub fn ln(self) -> Self {
        <Self as Scalar>::ln(self)
    }
    /// Calculates the square root using the corresponding compiler instruction.
    #[must_use]
    pub fn sqrt(self) -> Self {
        <Self as Scalar>::sqrt(self)
    }
    /// Calculates the sine using the corresponding compiler instruction.
    #[must_use]
    pub fn sin(self) -> Self {
        <Self as Scalar>::sin(self)
    }
    /// Calculates the cosine using the corresponding compiler instruction.
    #[must_use]
    pub fn cos(self) -> Self {
        <Self as Scalar>::cos(self)
    }
    /// Raises a numerical expression to a constant power.
    #[must_use]
    pub fn powf(self, value: f64) -> Self {
        <Self as Scalar>::powf(self, value)
    }
    /// Raises a numerical expression to another numerical expression.
    #[must_use]
    pub fn pow_expr(self, value: impl Into<Self>) -> Self {
        <Self as Scalar>::pows(self, value.into())
    }
    /// Selects the larger value, with zero derivatives for both operands at a tie.
    #[must_use]
    pub fn max(self, value: Self) -> Self {
        <Self as Scalar>::max_val(self, value)
    }
    /// Selects the smaller value, with zero derivatives for both operands at a tie.
    #[must_use]
    pub fn min(self, value: Self) -> Self {
        <Self as Scalar>::min_val(self, value)
    }
    /// Creates a constant with value zero.
    #[must_use]
    pub fn zero() -> Self {
        Self::constant(0.0)
    }
    /// Creates a constant with value one.
    #[must_use]
    pub fn one() -> Self {
        Self::constant(1.0)
    }
}

impl Trace {
    /// Exports dependency-ordered IR with canonical active inputs and lifted inactive constants.
    #[allow(clippy::too_many_lines)] // Reachability and aliases share one canonical input layout.
    fn export(&self, root: usize) -> Result<(Graph, Vec<f64>, usize)> {
        if root >= self.ops.len() {
            return Err(QSError::ForeignValue);
        }
        // Canonicalize interspersed input registration into the compiler's input prefix.
        // Literal values become inactive runtime inputs, so indexed random draws can
        // reuse executable code without receiving unnecessary adjoints.
        let mut numerical = self.input_values.clone();
        let mut mapping = vec![usize::MAX; self.ops.len()];
        for (index, operation) in self.ops.iter().enumerate() {
            if let Op::Input(input) = operation {
                mapping[index] = *input;
            }
        }
        let mut reachable = vec![false; self.ops.len()];
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            if reachable[index] {
                continue;
            }
            reachable[index] = true;
            if let Some(&alias) = self.aliases.get(&index) {
                pending.push(alias);
            } else {
                pending.extend(self.ops[index].operands());
            }
        }
        for (index, operation) in self.ops.iter().enumerate() {
            if reachable[index] {
                if let Op::Constant(value) = operation {
                    mapping[index] = numerical.len();
                    numerical.push(*value);
                }
            }
        }
        let mut operations = (0..numerical.len()).map(Op::Input).collect::<Vec<_>>();
        // DFS also handles calibrated input substitutions whose expression was
        // assembled after the leaf was registered. The exported graph is topological.
        let mut states = vec![0_u8; self.ops.len()];
        let mut stack = vec![(root, false)];
        while let Some((index, expanded)) = stack.pop() {
            if states[index] == 2 {
                continue;
            }
            if expanded {
                if let Some(&alias) = self.aliases.get(&index) {
                    mapping[index] = mapping[alias];
                } else {
                    mapping[index] = operations.len();
                    operations.push(self.ops[index].remap(|child| mapping[child]));
                }
                states[index] = 2;
            } else if states[index] == 1 {
                return Err(QSError::InvalidGraph("cyclic calibrated input map".into()));
            } else if !self.aliases.contains_key(&index) && mapping[index] != usize::MAX {
                states[index] = 2;
            } else {
                states[index] = 1;
                stack.push((index, true));
                if let Some(&alias) = self.aliases.get(&index) {
                    stack.push((alias, false));
                } else {
                    stack.extend(
                        self.ops[index]
                            .operands()
                            .into_iter()
                            .map(|child| (child, false)),
                    );
                }
            }
        }
        let output = mapping[root];
        Ok((
            Graph::new(operations, numerical.len(), output)?,
            numerical,
            self.inputs,
        ))
    }
}

/// Reuses structurally verified native code within the thread-local cache budget.
fn cached_program(graph: &Graph, active: usize) -> Result<Arc<GradientKernel>> {
    let mut hasher = DefaultHasher::new();
    active.hash(&mut hasher);
    graph.output().hash(&mut hasher);
    graph.input_count().hash(&mut hasher);
    for op in graph.operations() {
        discriminant(op).hash(&mut hasher);
        match op {
            Op::Input(index) => index.hash(&mut hasher),
            Op::Constant(value) => value.to_bits().hash(&mut hasher),
            _ => op.operands().hash(&mut hasher),
        }
    }
    let hash = hasher.finish();
    PROGRAMS.with(|slot| {
        let mut programs = slot.borrow_mut();
        if let Some(program) = programs.get(&hash).and_then(|programs| {
            programs.iter().find(|program| {
                program.active == active
                    && program.graph.output() == graph.output()
                    && program.graph.input_count() == graph.input_count()
                    && program.graph.operation_count() == graph.operation_count()
                    && program
                        .graph
                        .operations()
                        .iter()
                        .zip(graph.operations())
                        .all(|(a, b)| a.same_instruction(b))
            })
        }) {
            return Ok(Arc::clone(&program.kernel));
        }
        let kernel = Arc::new(GradientKernel::compile(graph, active)?);
        let cached_operations: usize = programs
            .values()
            .flatten()
            .map(|program| program.graph.operation_count())
            .sum();
        if programs.len() >= 512
            || cached_operations.saturating_add(graph.operation_count()) > 1_000_000
        {
            programs.clear();
        }
        programs.entry(hash).or_default().push(CachedProgram {
            graph: graph.clone(),
            active,
            kernel: Arc::clone(&kernel),
        });
        Ok(kernel)
    })
}

impl Serialize for Recorded {
    /// Serializes the numerical value under the strict-capture observation rules.
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        observe_primal(self);
        serializer.serialize_f64(self.primal)
    }
}
impl<'de> Deserialize<'de> for Recorded {
    /// Loads a detached numerical value without registering a market input.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        f64::deserialize(deserializer).map(Self::detached)
    }
}

impl From<Recorded> for f64 {
    /// Extracts a numerical primal under the strict-capture observation rules.
    fn from(value: Recorded) -> Self {
        value.value()
    }
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use crate::{
        ad::Graph,
        utils::errors::{QSError, Result},
    };

    /// Checks capture state is cleared after unwinding.
    #[test]
    fn capture_state_is_cleared_after_unwinding() -> Result<()> {
        let failed = catch_unwind(|| {
            let _ = Graph::trace(&[1.0], |_| panic!("numerical capture failed"));
        });
        assert!(failed.is_err());
        let graph = Graph::trace(&[2.0], |x| x[0] * x[0])?;
        assert_eq!(graph.compile()?.value_and_grad(&[3.0], &mut [0.0])?, 9.0);
        Ok(())
    }

    /// Checks rejection of input dependent branches.
    #[test]
    fn rejects_input_dependent_branches() {
        let result = Graph::trace(&[1.0], |x| if x[0] > 0.0 { x[0] } else { -x[0] });
        assert!(matches!(result, Err(QSError::DynamicControlFlow)));
    }

    /// Checks rejection of formatted primal values.
    #[test]
    fn rejects_formatted_primal_values() {
        let result = Graph::trace(&[1.0], |x| {
            let _ = x[0].to_string();
            x[0]
        });
        assert!(matches!(result, Err(QSError::DynamicControlFlow)));
        let result = Graph::trace(&[1.0], |x| {
            let _ = format!("{:?}", x[0]);
            x[0]
        });
        assert!(matches!(result, Err(QSError::DynamicControlFlow)));
    }
}
