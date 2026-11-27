//! Derivative work selected when compiling a numerical program.

/// Highest derivative order generated for a compiled kernel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DerivativeOrder {
    /// A value and first gradient, without higher-order seeds or kernels.
    #[default]
    First,
    /// First derivatives and an exact Hessian-vector product kernel.
    Second,
}
