//! Automatic differentiation (AD) support.
//!
//! Provides [`ADReal`](crate::ad::adreal::ADReal) for forward-mode AD, a shared
//! [`Tape`](crate::ad::tape::Tape) for recording operations, and graph
//! [`TapeNode`](crate::ad::node::TapeNode)s for backward-mode adjoint propagation.

/// `ADReal` module.
pub mod adreal;
/// Node module.
pub mod node;
/// Tape node module.
pub mod tape;
