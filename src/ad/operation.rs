//! Replayable scalar instructions.

use super::operands::Operands;
use std::mem::discriminant;
/// One scalar operation in a validated, topologically ordered numerical graph.
#[derive(Clone, Debug)]
pub enum Operation {
    /// Load a runtime input by index.
    Input(usize),
    /// A literal numerical constant.
    Constant(f64),
    /// Add two graph values.
    Add(usize, usize),
    /// Subtract the second graph value.
    Sub(usize, usize),
    /// Multiply two graph values.
    Mul(usize, usize),
    /// Divide the first graph value by the second.
    Div(usize, usize),
    /// Negate a graph value.
    Neg(usize),
    /// Natural exponential.
    Exp(usize),
    /// Natural logarithm.
    Ln(usize),
    /// Nonnegative square root.
    Sqrt(usize),
    /// Sine, with an argument in radians.
    Sin(usize),
    /// Cosine, with an argument in radians.
    Cos(usize),
    /// Absolute value, with zero derivative at zero.
    Abs(usize),
    /// Raise a base to a graph-valued exponent.
    Pow(usize, usize),
    /// Maximum, ignoring a single NaN; both derivatives are zero at ties.
    Max(usize, usize),
    /// Minimum, ignoring a single NaN; both derivatives are zero at ties.
    Min(usize, usize),
    /// Standard normal CDF, with analytic Gaussian first and second derivatives.
    NormalCdf(usize),
    /// Select the third value when the first exceeds the second, otherwise the fourth.
    /// Comparisons have zero derivative, including at ties and unordered inputs.
    SelectGreater(usize, usize, usize, usize),
}

impl Operation {
    /// Compares program structure and exact constant bits for safe native-code reuse.
    #[must_use]
    pub fn same_instruction(&self, other: &Self) -> bool {
        if discriminant(self) != discriminant(other) {
            return false;
        }
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => a.to_bits() == b.to_bits(),
            (Self::Input(a), Self::Input(b)) => a == b,
            _ => self.operands() == other.operands(),
        }
    }

    /// Remaps instruction references while preserving literal values and input slots.
    #[must_use]
    pub fn remap(&self, mut map: impl FnMut(usize) -> usize) -> Self {
        match *self {
            Self::Input(i) => Self::Input(i),
            Self::Constant(value) => Self::Constant(value),
            Self::Add(a, b) => Self::Add(map(a), map(b)),
            Self::Sub(a, b) => Self::Sub(map(a), map(b)),
            Self::Mul(a, b) => Self::Mul(map(a), map(b)),
            Self::Div(a, b) => Self::Div(map(a), map(b)),
            Self::Pow(a, b) => Self::Pow(map(a), map(b)),
            Self::Max(a, b) => Self::Max(map(a), map(b)),
            Self::Min(a, b) => Self::Min(map(a), map(b)),
            Self::Neg(a) => Self::Neg(map(a)),
            Self::Exp(a) => Self::Exp(map(a)),
            Self::Ln(a) => Self::Ln(map(a)),
            Self::Sqrt(a) => Self::Sqrt(map(a)),
            Self::Sin(a) => Self::Sin(map(a)),
            Self::Cos(a) => Self::Cos(map(a)),
            Self::Abs(a) => Self::Abs(map(a)),
            Self::NormalCdf(a) => Self::NormalCdf(map(a)),
            Self::SelectGreater(a, b, yes, no) => {
                Self::SelectGreater(map(a), map(b), map(yes), map(no))
            }
        }
    }

    /// Returns preceding operation indices read by this instruction.
    #[must_use]
    pub fn operands(&self) -> Operands {
        match *self {
            Self::Input(_) | Self::Constant(_) => Operands::from([]),
            Self::Neg(a)
            | Self::Exp(a)
            | Self::Ln(a)
            | Self::Sqrt(a)
            | Self::Sin(a)
            | Self::Cos(a)
            | Self::Abs(a)
            | Self::NormalCdf(a) => Operands::from([a]),
            Self::Add(a, b)
            | Self::Sub(a, b)
            | Self::Mul(a, b)
            | Self::Div(a, b)
            | Self::Pow(a, b)
            | Self::Max(a, b)
            | Self::Min(a, b) => Operands::from([a, b]),
            Self::SelectGreater(a, b, yes, no) => Operands::from([a, b, yes, no]),
        }
    }
}
