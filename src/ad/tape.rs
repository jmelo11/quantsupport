use bumpalo::Bump;
use std::{
    cell::RefCell,
    fmt::Debug,
    ops::{Add, AddAssign, Mul},
    ptr::NonNull,
};

use crate::utils::errors::Result;
use crate::{ad::node::TapeNode, utils::errors::QSError};

// ---------------------------------------------------------------------------
// TapeHolder — the trait that links an inner scalar to its thread-local tape
// ---------------------------------------------------------------------------

/// Trait implemented by types that can serve as the inner scalar of a
/// [`Dual`](crate::ad::dual::Dual).
///
/// Each implementing type owns a thread-local [`Tape`] that records
/// operations for reverse-mode differentiation.
pub trait TapeHolder:
    Sized + Copy + Default + Add<Output = Self> + Mul<Output = Self> + AddAssign + Debug + Send + Sync
{
    /// Execute `f` with a mutable borrow of the thread-local tape for `Self`.
    fn with_tape<R>(f: impl FnOnce(&mut Tape<Self>) -> R) -> R;
}

// ---------------------------------------------------------------------------
// Tape<T>
// ---------------------------------------------------------------------------

/// A tape holding all recorded nodes for reverse-mode differentiation.
///
/// The tape is implemented as a bump arena for efficient allocation and
/// deallocation of nodes, and a book (vector) of pointers to the nodes for
/// indexing and propagation.  The tape supports marking and rewinding to
/// enable nested operations without interference.
///
/// The default type parameter `T = f64` preserves backward compatibility:
/// `Tape` (without turbofish) is `Tape<f64>`.
pub struct Tape<T = f64> {
    /// Bump arena for efficient node allocation.
    pub bump: Bump,
    /// Ordered list of recorded node pointers.
    pub book: Vec<NonNull<TapeNode<T>>>,
    /// Current mark index for nested operations.
    pub mark: usize,
    /// Whether the tape is currently recording.
    pub active: bool,
}

// -- Generic methods (work for any TapeHolder T) -----------------------------

impl<T: TapeHolder> Tape<T> {
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
    fn push(&mut self, n: TapeNode<T>) -> NonNull<TapeNode<T>> {
        let ptr = NonNull::from(self.bump.alloc(n));
        self.book.push(ptr);
        ptr
    }

    /// Returns the current mark index for this tape.
    pub const fn mark(&self) -> usize {
        self.mark
    }

    /// Returns the index of a node in the tape book, if it exists.
    #[inline]
    fn index_of(&self, p: NonNull<TapeNode<T>>) -> Option<usize> {
        self.book.iter().position(|&q| q == p)
    }

    /// Allocates and records a leaf node.
    #[inline]
    pub fn new_leaf(&mut self) -> Option<NonNull<TapeNode<T>>> {
        self.record(TapeNode::default())
    }

    /// Records a node if recording is active, returning its pointer.
    #[inline]
    pub fn record(&mut self, n: TapeNode<T>) -> Option<NonNull<TapeNode<T>>> {
        self.active.then(|| self.push(n))
    }

    /// Retrieves an immutable reference to a node by pointer.
    pub fn node(&self, p: NonNull<TapeNode<T>>) -> Option<&TapeNode<T>> {
        self.index_of(p).map(|i| unsafe { self.book[i].as_ref() })
    }

    /// Retrieves a mutable reference to a node by pointer.
    pub fn mut_node(&mut self, p: NonNull<TapeNode<T>>) -> Option<&mut TapeNode<T>> {
        self.index_of(p).map(|i| unsafe { self.book[i].as_mut() })
    }

    /// Propagates adjoints from the given root node back to the start of the tape.
    ///
    /// # Errors
    /// Returns an error if the given node is not indexed in the tape.
    pub fn propagate_from(&mut self, root: NonNull<TapeNode<T>>) -> Result<()> {
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
            return Ok(());
        }
        for i in (start..=end).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
        Ok(())
    }

    /// Resets all adjoints on this tape to the default (zero) value.
    pub fn reset_adjoints_inner(&self) {
        for &ptr in &self.book {
            unsafe { (*ptr.as_ptr()).adj = T::default() };
        }
    }

    /// Clears the tape and begins recording.
    pub fn start_inner(&mut self) {
        self.bump.reset();
        self.book.clear();
        self.mark = 0;
        self.active = true;
    }
}

impl<T: TapeHolder> Default for Tape<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// f64 TapeHolder + Tape<f64> static methods  (backward compat)
// ---------------------------------------------------------------------------

impl TapeHolder for f64 {
    fn with_tape<R>(f: impl FnOnce(&mut Tape<Self>) -> R) -> R {
        TAPE.with(|tc| {
            let mut t = tc.borrow_mut();
            f(&mut t)
        })
    }
}

thread_local! {
    /// Thread-local tape used by [`Dual<f64>`](crate::ad::dual::Dual) (aka `DualFwd`).
    pub static TAPE: RefCell<Tape<f64>> = RefCell::new(Tape {
        bump:   Bump::new(),
        book:   Vec::new(),
        mark:   0,
        active: false,
    });
}

/// Static convenience methods for the default `f64` tape.
///
/// These preserve backward compatibility: `Tape::start_recording()`,
/// `Tape::stop_recording()`, etc. continue to work without turbofish.
impl Tape<f64> {
    /// Clears the tape and begins recording nodes in the thread-local tape.
    pub fn start_recording() {
        TAPE.with(|tc| tc.borrow_mut().start_inner());
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

    /// Resets all adjoints on the thread-local tape to zero.
    pub fn reset_adjoints() {
        TAPE.with(|tc| tc.borrow().reset_adjoints_inner());
    }
}
