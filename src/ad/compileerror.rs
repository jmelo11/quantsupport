//! Errors from graph capture, compilation and execution.

/// Errors raised by recording, native compilation or execution.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// A fallible numerical pricing function rejected its contract or inputs.
    #[error("numerical capture failed: {0}")]
    Capture(String),
    /// Current inputs take a different Rust branch from the recorded inputs.
    #[error("a recorded comparison changed; prepare the calculation again for the current inputs")]
    ComparisonChanged,
    /// An operation refers to a missing or later value, or the input layout is invalid.
    #[error("invalid numerical graph: {0}")]
    InvalidGraph(String),
    /// Second derivatives were requested from a first-order kernel.
    #[error("compile with DerivativeOrder::Second before requesting second derivatives")]
    SecondOrderUnavailable,
    /// A direction or Hessian buffer has the wrong length.
    #[error("expected {expected} {buffer} slots, got {actual}")]
    BufferCount {
        /// Description of the incorrectly sized buffer.
        buffer: &'static str,
        /// Required number of slots.
        expected: usize,
        /// Supplied number of slots.
        actual: usize,
    },
    /// Recording cannot be recursively nested on the same thread.
    #[error("a QuantSupport compiler recording is already active")]
    NestedCapture,
    /// Internal recording state disappeared.
    #[error("numerical capture was not active")]
    CaptureMissing,
    /// A scalar from another recording was returned.
    #[error("the graph returned a value from a different recording")]
    ForeignValue,
    /// Data-dependent Rust branching or extracted values cannot be replayed.
    #[error("the captured function observes a numerical value or branches on a traced value; use replay-safe numerical operations instead")]
    DynamicControlFlow,
    /// Input array length does not match the graph signature.
    #[error("expected {expected} numerical inputs, got {actual}")]
    InputCount {
        /// Number of expected inputs.
        expected: usize,
        /// Number of supplied inputs.
        actual: usize,
    },
    /// Gradient output array length does not match the graph signature.
    #[error("expected {expected} gradient slots, got {actual}")]
    GradientCount {
        /// Number of expected gradient output slots.
        expected: usize,
        /// Number of supplied gradient slots.
        actual: usize,
    },
    /// A graph input offset does not fit Cranelift's addressing format.
    #[error("the graph input layout exceeds supported address offsets")]
    OffsetOverflow,
    /// Cranelift compilation or JIT code generation failed.
    #[error("Cranelift compilation failed: {0}")]
    Jit(String),
}
