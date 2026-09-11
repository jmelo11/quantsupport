//! Automatic differentiation (AD) support.
//!
//! Provides the generic nestable [`Fwd`](crate::ad::forward::Fwd) for
//! forward-mode AD of arbitrary order (`Fwd<f64>` = 1st, `Fwd<Fwd<f64>>` =
//! 2nd, …), a shared [`Tape`](crate::ad::tape::Tape) for recording
//! operations, and graph [`TapeNode`](crate::ad::node::TapeNode)s for
//! backward-mode adjoint propagation. Mixed mode is available as
//! `Dual<FwdN>` (e.g. [`DualFwd`](crate::ad::dual::DualFwd)).

pub mod blocklist;
pub mod constant;
pub mod dual;
pub mod expr;
pub mod forward;
/// Reverse-mode tape node representation.
pub mod node;
pub mod scalar;
/// Thread-local reverse-mode tape implementation.
pub mod tape;
