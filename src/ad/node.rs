use std::{
    fmt::{Debug, Formatter, Result as fmtResult},
    ptr::NonNull,
};

/// A node recorded on the tape, with child links and adjoint values.
///
/// The [`TapeNode`] struct represents a single node in the computational graph recorded on the tape. It contains:
/// - [`Self::childs`]: A vector of non-null pointers to child nodes that receive propagated adjoints during backpropagation.
/// - [`Self::derivs`]: A vector of local derivatives corresponding to each child, used to compute the contribution to each child's adjoint.
/// - [`Self::adj`]: The accumulated adjoint value for this node, which is updated during backpropagation and used to propagate gradients to child nodes.
#[derive(Clone)]
pub struct TapeNode {
    /// Child nodes that receive propagated adjoints.
    pub childs: Vec<NonNull<Self>>,
    /// Local derivatives for each child.
    pub derivs: Vec<f64>,
    /// The accumulated adjoint for this node.
    pub adj: f64,
}

/// ideally this should print its own address
impl Debug for TapeNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        write!(
            f,
            "TapeNode {{ childs: {:?}, derivs: {:?}, adj: {} }}",
            self.childs, self.derivs, self.adj
        )
    }
}

impl Default for TapeNode {
    /// Constructs an empty tape node with zero adjoint.
    fn default() -> Self {
        Self {
            childs: Vec::new(),
            derivs: Vec::new(),
            adj: 0.0,
        }
    }
}

impl TapeNode {
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
