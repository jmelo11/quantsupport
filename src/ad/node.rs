//! Tape node definitions for the reverse-mode tape.

use std::{fmt, ptr::NonNull};

/// A node recorded on the tape, with child links and adjoint values.
#[derive(Clone)]
pub struct TapeNode {
    /// Child nodes that receive propagated adjoints.
    pub childs: Vec<NonNull<TapeNode>>,
    /// Local derivatives for each child.
    pub derivs: Vec<f64>,
    /// The accumulated adjoint for this node.
    pub adj: f64,
}

impl fmt::Debug for TapeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TapeNode {{ addr: {:?}, childs: {:?}, derivs: {:?}, adj: {} }}",
            self as *const Self as *const (), self.childs, self.derivs, self.adj
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
    #[inline(always)]
    /// Propagates this node's adjoint into each child using stored derivatives.
    pub fn propagate_into(&self) {
        debug_assert_eq!(self.childs.len(), self.derivs.len());
        let a = self.adj;
        for (&child, &d) in self.childs.iter().zip(&self.derivs) {
            unsafe { (*child.as_ptr()).adj += a * d };
        }
    }
}
