use std::{
    fmt::{Debug, Formatter, Result as fmtResult},
    ops::{Add, AddAssign, Mul},
    ptr::NonNull,
};

/// A node recorded on the tape, with child links and adjoint values.
///
/// Generic over the inner scalar `T`, which is `f64` for first-order
/// backward-mode AD, or [`ADForward`](crate::ad::forward::ADForward) for
/// mixed backward+forward second-order AD.
///
/// - [`Self::childs`]: Pointers to child nodes that receive propagated adjoints.
/// - [`Self::derivs`]: Local derivatives (type `T`) for each child.
/// - [`Self::adj`]: Accumulated adjoint (type `T`) for this node.
#[derive(Clone)]
pub struct TapeNode<T> {
    /// Child nodes that receive propagated adjoints.
    pub childs: Vec<NonNull<Self>>,
    /// Local derivatives for each child.
    pub derivs: Vec<T>,
    /// The accumulated adjoint for this node.
    pub adj: T,
}

impl<T: Debug> Debug for TapeNode<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        write!(
            f,
            "TapeNode {{ childs: {:?}, derivs: {:?}, adj: {:?} }}",
            self.childs, self.derivs, self.adj
        )
    }
}

impl<T: Default> Default for TapeNode<T> {
    /// Constructs an empty tape node with zero adjoint.
    fn default() -> Self {
        Self {
            childs: Vec::new(),
            derivs: Vec::new(),
            adj: T::default(),
        }
    }
}

impl<T: Copy + Add<Output = T> + Mul<Output = T> + AddAssign> TapeNode<T> {
    /// Propagates this node's adjoint into each child using stored derivatives.
    #[inline]
    pub fn propagate_into(&self) {
        debug_assert_eq!(self.childs.len(), self.derivs.len());
        let a = self.adj;
        for (&child, &d) in self.childs.iter().zip(&self.derivs) {
            unsafe { (*child.as_ptr()).adj += a * d };
        }
    }
}
