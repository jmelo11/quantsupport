//! Replay guards for ordinary Rust comparison traits during numerical recording.

/// The boolean comparison observed while recording a Rust control-flow decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// IEEE equality; NaN is unequal to every value.
    Equal,
    /// IEEE inequality; NaN is unequal to every value.
    NotEqual,
    /// Strict less-than.
    Less,
    /// Less-than or equal.
    LessEqual,
    /// Strict greater-than.
    Greater,
    /// Greater-than or equal.
    GreaterEqual,
    /// An unordered `PartialOrd::partial_cmp` result.
    Unordered,
}

impl Comparison {
    /// Applies the same floating-point comparison used by Rust's comparison traits.
    #[must_use]
    #[allow(clippy::float_cmp)] // Guards must preserve exact Rust comparison semantics.
    pub fn evaluate(self, left: f64, right: f64) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::Less => left < right,
            Self::LessEqual => left <= right,
            Self::Greater => left > right,
            Self::GreaterEqual => left >= right,
            Self::Unordered => left.partial_cmp(&right).is_none(),
        }
    }
}

/// A captured comparison whose boolean result must remain unchanged on replay.
///
/// Rust operators return `bool`, so a recorder observes only the executed arm.
/// A replay guard prevents silently evaluating that arm for inputs that would
/// take a different branch. Use numerical selection for per-path branches
/// that need to vary inside one compiled program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComparisonGuard {
    operands: [usize; 2],
    comparison: Comparison,
    expected: bool,
}

impl ComparisonGuard {
    /// Records the operand operations, comparison and boolean result.
    #[must_use]
    pub const fn new(left: usize, right: usize, comparison: Comparison, expected: bool) -> Self {
        Self {
            operands: [left, right],
            comparison,
            expected,
        }
    }

    /// Returns the operand indices whose current values are required to check this comparison.
    #[must_use]
    pub const fn operands(&self) -> [usize; 2] {
        self.operands
    }

    /// Checks whether the current operands preserve the comparison result observed during capture.
    #[must_use]
    pub fn matches(&self, left: f64, right: f64) -> bool {
        self.comparison.evaluate(left, right) == self.expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks comparisons preserve ieee nan and boundary semantics.
    #[test]
    #[allow(clippy::float_cmp)] // The test oracle is the comparison trait itself.
    fn comparisons_preserve_ieee_nan_and_boundary_semantics() {
        for left in [-1.0, -0.0, 0.0, 1.0, f64::MAX, f64::INFINITY, f64::NAN] {
            assert_eq!(Comparison::LessEqual.evaluate(left, 0.0), left <= 0.0);
            assert_eq!(Comparison::Greater.evaluate(left, 0.0), left > 0.0);
            assert_eq!(Comparison::Equal.evaluate(left, 0.0), left == 0.0);
            assert_eq!(Comparison::NotEqual.evaluate(left, 0.0), left != 0.0);
        }
    }
}
