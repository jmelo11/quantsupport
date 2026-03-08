use bumpalo::Bump;
use std::{cell::RefCell, ptr::NonNull};

use crate::utils::errors::Result;
use crate::{ad::node::TapeNode, utils::errors::QSError};

/// # `Tape`
///
/// A tape holding all recorded nodes for reverse-mode differentiation.
pub struct Tape {
    bump: Bump,
    book: Vec<NonNull<TapeNode>>,
    mark: usize,
    active: bool,
}

impl Tape {
    /// Creates an empty tape with recording disabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bump: Bump::new(),
            book: Vec::new(),
            mark: 0,
            active: false,
        }
    }

    /// Allocates a node in the bump arena and records it in the tape book.
    #[inline]
    fn push(&mut self, n: TapeNode) -> NonNull<TapeNode> {
        let ptr = NonNull::from(self.bump.alloc(n));
        self.book.push(ptr);
        ptr
    }

    /// Resets all adjoints on the current thread's tape.
    #[inline]
    pub fn reset_adjoints() {
        TAPE.with(|tc| {
            for &ptr in &tc.borrow().book {
                unsafe { (*ptr.as_ptr()).adj = 0.0 };
            }
        });
    }

    /// Returns the current mark index for this tape.
    pub const fn mark(&self) -> usize {
        self.mark
    }

    /// Returns the index of a node in the tape book, if it exists.
    ///
    /// This is a linear scan; the cost grows with the number of recorded nodes.
    #[inline]
    fn index_of(&self, p: NonNull<TapeNode>) -> Option<usize> {
        self.book.iter().position(|&q| q == p)
    }

    /// Allocates and records a leaf node.
    #[inline]
    pub fn new_leaf(&mut self) -> Option<NonNull<TapeNode>> {
        self.record(TapeNode::default())
    }

    /// Records a node if recording is active, returning its pointer.
    #[inline]
    pub fn record(&mut self, n: TapeNode) -> Option<NonNull<TapeNode>> {
        self.active.then(|| self.push(n))
    }

    /// Retrieves an immutable reference to a node by pointer.
    pub fn node(&self, p: NonNull<TapeNode>) -> Option<&TapeNode> {
        self.index_of(p).map(|i| unsafe { self.book[i].as_ref() })
    }
    /// Retrieves a mutable reference to a node by pointer.
    pub fn mut_node(&mut self, p: NonNull<TapeNode>) -> Option<&mut TapeNode> {
        self.index_of(p).map(|i| unsafe { self.book[i].as_mut() })
    }

    /// Propagates adjoints from the given root node back to the start of the tape.
    ///
    /// # Errors
    /// Returns an error if the given node is not indexed in the tape.
    pub fn propagate_from(&mut self, root: NonNull<TapeNode>) -> Result<()> {
        let start = self
            .index_of(root)
            .ok_or(QSError::NodeNotIndexedInTapeErr)?;
        for i in (0..=start).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
        Ok(())
    }

    /// Propagates adjoints from the current mark back to the start.
    ///
    /// ## Errors
    /// Returns an error if the tape is empty.
    pub fn propagate_mark_to_start(&mut self) -> Result<()> {
        let end = self.mark.saturating_sub(1);
        for i in (0..=end).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
        Ok(())
    }

    /// Propagates adjoints from the end of the tape down to the current mark.
    ///
    /// ## Errors
    /// Returns an error if the tape is empty.
    pub fn propagate_to_mark(&mut self) -> Result<()> {
        let start = self.mark;
        let end = self.book.len().saturating_sub(1);
        if start > end {
            return Ok(()); // Nothing to propagate
        }
        for i in (start..=end).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
        Ok(())
    }

    /// Clears the tape and begins recording nodes in the thread-local tape.
    pub fn start_recording() {
        TAPE.with(|tc| {
            let mut t = tc.borrow_mut();
            t.bump.reset();
            t.book.clear();
            t.mark = 0;
            t.active = true;
        });
    }

    /// Stops recording nodes on the thread-local tape.
    pub fn stop_recording() {
        TAPE.with(|tc| tc.borrow_mut().active = false);
    }

    /// Returns whether the thread-local tape is active.
    #[inline]
    #[must_use]
    pub fn is_active() -> bool {
        TAPE.with(|tc| tc.borrow().active)
    }

    /// Sets the current mark to the end of the tape.
    pub fn set_mark() {
        TAPE.with(|tc| {
            let len = tc.borrow().book.len();
            tc.borrow_mut().mark = len;
        });
    }

    /// Truncates the tape back to the current mark.
    pub fn rewind_to_mark() {
        TAPE.with(|tc| {
            let mark = tc.borrow().mark;
            tc.borrow_mut().book.truncate(mark);
        });
    }

    /// Resets the mark to the beginning of the tape.
    ///
    /// This is useful when a nested operation (e.g. an AD-based Jacobian
    /// inside a solver) advances the mark so that a subsequent
    /// `backward_to_mark` would not cover the full tape.  Calling
    /// `reset_mark` after the outer computation restores full coverage.
    pub fn reset_mark() {
        TAPE.with(|tc| {
            tc.borrow_mut().mark = 0;
        });
    }

    /// Clears the tape and resets the mark without changing active state.
    pub fn rewind_to_init() {
        TAPE.with(|tc| {
            let mut t = tc.borrow_mut();
            t.bump.reset();
            t.book.clear();
            t.mark = 0;
        });
    }
}

impl Default for Tape {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    /// Thread-local tape used by default by `ADNumber`.
    pub static TAPE: RefCell<Tape> = RefCell::new(Tape {
        bump:   Bump::new(),
        book:   Vec::new(),
        mark:   0,
        active: false,
    });
}
