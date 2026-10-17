use std::{
    fmt::{Debug, Formatter, Result as fmtResult},
    ops::{Add, AddAssign, Mul},
    ptr::NonNull,
};

/// Number of child slots stored inline in a [`TapeNode`].
///
/// Nodes with at most this many children (the overwhelmingly common case)
/// require no heap allocation. Longer fused expressions spill to a `Vec`.
const INLINE_CAP: usize = 4;

/// A node recorded on the tape, with child links and adjoint values.
///
/// Generic over the inner scalar `T`, which is `f64` for first-order
/// backward-mode AD, or [`ADForward`](crate::ad::forward::ADForward) for
/// mixed backward+forward second-order AD.
///
/// Up to four children and their local derivatives are stored inline.
/// Longer fused expressions allocate overflow storage.
#[derive(Clone)]
pub struct TapeNode<T> {
    /// Inline child pointers; the first `min(len, INLINE_CAP)` are valid.
    childs: [NonNull<Self>; INLINE_CAP],
    /// Inline local derivatives, parallel to `childs`.
    derivs: [T; INLINE_CAP],
    /// Overflow storage for nodes with more than `INLINE_CAP` children.
    spill: Vec<(NonNull<Self>, T)>,
    /// Total number of children (inline + spill).
    len: u32,
    /// The accumulated adjoint for this node.
    pub adj: T,
    /// Position of this node in the tape book, set when recorded.
    /// Enables O(1) lookup instead of a linear scan.
    pub idx: usize,
}

impl<T> TapeNode<T> {
    /// Appends a child with its local derivative.
    #[inline]
    pub fn push_child(&mut self, child: NonNull<Self>, deriv: T) {
        let i = self.len as usize;
        if i < INLINE_CAP {
            self.childs[i] = child;
            self.derivs[i] = deriv;
        } else {
            self.spill.push((child, deriv));
        }
        self.len += 1;
    }

    /// Returns the number of children of this node.
    #[inline]
    #[must_use]
    pub const fn num_children(&self) -> usize {
        self.len as usize
    }
}

impl<T: Debug> Debug for TapeNode<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        let n_inline = (self.len as usize).min(INLINE_CAP);
        write!(f, "TapeNode {{ childs: [")?;
        for i in 0..n_inline {
            write!(f, "({:?}, {:?}), ", self.childs[i], self.derivs[i])?;
        }
        for (c, d) in &self.spill {
            write!(f, "({c:?}, {d:?}), ")?;
        }
        write!(f, "], adj: {:?} }}", self.adj)
    }
}

impl<T: Copy + Default> Default for TapeNode<T> {
    /// Constructs an empty tape node with zero adjoint.
    fn default() -> Self {
        Self {
            childs: [NonNull::dangling(); INLINE_CAP],
            derivs: [T::default(); INLINE_CAP],
            spill: Vec::new(),
            len: 0,
            adj: T::default(),
            idx: usize::MAX,
        }
    }
}

impl<T: Copy + Add<Output = T> + Mul<Output = T> + AddAssign> TapeNode<T> {
    /// Propagates this node's adjoint into each child using stored derivatives.
    #[inline]
    pub fn propagate_into(&self) {
        let a = self.adj;
        let n_inline = (self.len as usize).min(INLINE_CAP);
        for i in 0..n_inline {
            // SAFETY: children are live nodes recorded earlier on the tape.
            unsafe { (*self.childs[i].as_ptr()).adj += a * self.derivs[i] };
        }
        for &(child, d) in &self.spill {
            // SAFETY: as above.
            unsafe { (*child.as_ptr()).adj += a * d };
        }
    }
}
