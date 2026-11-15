//! Automatic differentiation compiled to native code with Cranelift.
//!
//! A calculation starts with [`Recorded`] scalar arithmetic. [`Graph`] captures
//! that arithmetic as numerical instructions, and [`CompiledKernel`] compiles
//! the instructions and their derivatives into reusable machine code. Executing
//! the kernel with new input values reuses the code without calling the original
//! Rust closure again.
//!
//! Compilation selects the derivative order explicitly. The default
//! [`DerivativeOrder::First`] generates a value and gradient; selecting
//! [`DerivativeOrder::Second`] also provides Hessian-vector products and dense
//! Hessians. A first-order execution does not calculate second derivatives.
//!
//! For calculations with several outputs, [`CompiledFunction`] generates
//! Jacobians and optional output Hessians. [`ImplicitDerivatives`] connects those
//! partial derivatives through a calibrated system. Ordinary library pricers
//! manage their own [`Recording`] scopes and market-input registration, so an
//! application can request sensitivities through the existing pricing interface.
//!
//! ```
//! use quantsupport::prelude::*;
//! let graph = Graph::trace(&[2.0, 3.0], |x| x[0] * x[0] + x[0] * x[1])?;
//! let first = graph.compile()?;
//! let mut gradient = [0.0; 2];
//! assert_eq!(first.value_and_grad(&[2.0, 3.0], &mut gradient)?, 10.0);
//! assert_eq!(gradient, [7.0, 2.0]);
//! let second = graph.compile_with_order(DerivativeOrder::Second)?;
//! let mut hessian = [0.0; 4];
//! second.value_grad_hessian(&[2.0, 3.0], &mut gradient, &mut hessian)?;
//! assert_eq!(hessian, [2.0, 1.0, 1.0, 0.0]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

/// Checks that a reused program still follows the Rust comparisons observed during capture.
pub mod comparisonguard;
/// Evaluates residual vectors and their generated Jacobians or optional Hessians.
pub mod compiledfunction;
/// Owns a reusable native scalar calculation and the requested derivative kernels.
pub mod compiledkernel;
/// Runs multiple native outputs with explicit active inputs and comparison guards.
pub mod compiledprogram;
/// Reports failures in capture, graph validation, native compilation and execution.
pub mod compileerror;
/// Selects whether a kernel provides first derivatives or also second derivatives.
pub mod derivativeorder;
/// Divides large derivative graphs into bounded native compilation units.
pub mod gradientkernel;
/// Stores validated numerical instructions and generates derivative expressions from them.
pub mod graph;
/// Connects calibrated states to their inputs through implicit first and second derivatives.
pub mod implicitderivatives;
/// Lowers the numerical instructions and derivative calculations to Cranelift machine code.
pub mod jit;
/// Owns executable memory and checks the buffers passed to a native function.
pub mod nativekernel;
/// Stores instruction dependencies inline while numerical programs are constructed.
pub mod operands;
/// Defines the numerical instructions accepted by the AD graph.
pub mod operation;
/// Stores unnormalized path values and gradient sums from parallel reductions.
pub mod pathsums;
/// Represents persistent numerical values and arithmetic inside a compiler capture.
pub mod recorded;
/// Manages pricing captures and restores enclosing captures after isolated work.
pub mod recording;

pub use compiledfunction::CompiledFunction;
pub use compiledkernel::CompiledKernel;
pub use compiledprogram::CompiledProgram;
pub use compileerror::CompileError;
pub use derivativeorder::DerivativeOrder;
pub use graph::Graph;
pub use implicitderivatives::ImplicitDerivatives;
pub use pathsums::PathSums;
pub use recorded::Recorded;
pub use recording::{Recording, RecordingScope};

/// Defines the numerical operations shared by plain and recorded scalar calculations.
pub mod scalar;
