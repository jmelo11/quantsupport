//! Allocation-free operand indices for scalar compiler instructions.

use std::{array::IntoIter, iter::Take, ops::Deref};

/// Up to four ordered operation references, stored inline during compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Operands {
    values: [usize; 4],
    len: usize,
}

impl<const N: usize> From<[usize; N]> for Operands {
    /// Stores operand indices inline and retains the active operand count.
    fn from(values: [usize; N]) -> Self {
        assert!(N <= 4, "scalar instructions accept at most four operands");
        let mut result = Self {
            values: [0; 4],
            len: N,
        };
        result.values[..N].copy_from_slice(&values);
        result
    }
}
impl Deref for Operands {
    type Target = [usize];
    /// Borrows the active operand indices without allocating.
    fn deref(&self) -> &Self::Target {
        &self.values[..self.len]
    }
}
impl IntoIterator for Operands {
    type Item = usize;
    type IntoIter = Take<IntoIter<usize, 4>>;
    /// Consumes the inline operand list in dependency order.
    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter().take(self.len)
    }
}
