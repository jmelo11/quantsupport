//! Automatic differentiation (AD) support.
//!
//! Provides [`DualFwd`](crate::ad::adreal::DualFwd) for forward-mode AD, a shared
//! [`Tape`](crate::ad::tape::Tape) for recording operations, and graph
//! [`TapeNode`](crate::ad::node::TapeNode)s for backward-mode adjoint propagation.

/// Constant wrapper (Const<T>).
pub mod constant;
/// Backward-mode AD wrapper (Dual<T>) and DualFwd alias.
pub mod dual;
/// Expression-template system (Expr, operators, BinExpr, UnExpr, FloatExt, free fns).
pub mod expr;
/// Forward-mode AD type (ADForward).
pub mod forward;
/// Scalar and InnerScalar traits.
pub mod scalar;

/// Re-export hub — preserves `crate::ad::adreal::*` import paths.
pub mod adreal;
/// Node module.
pub mod node;
/// Tape node module.
pub mod tape;
