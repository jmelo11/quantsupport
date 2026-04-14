//! Automatic differentiation (AD) support.
//!
//! Provides [`DualFwd`](crate::ad::dual::DualFwd) for forward-mode AD, a shared
//! [`Tape`](crate::ad::tape::Tape) for recording operations, and graph
//! [`TapeNode`](crate::ad::node::TapeNode)s for backward-mode adjoint propagation.

/// Block-based slab allocator for tape nodes.
pub mod blocklist;
/// Constant wrapper (Const<T>).
pub mod constant;
/// Backward-mode AD wrapper (Dual<T>) and DualFwd alias.
pub mod dual;
/// Expression-template system (Expr, operators, BinExpr, UnExpr, FloatExt, free fns).
pub mod expr;
/// Forward-mode AD type (ADForward).
pub mod forward;
/// Node module.
pub mod node;
/// Scalar and InnerScalar traits.
pub mod scalar;
/// Tape node module.
pub mod tape;
