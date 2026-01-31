//! Tape storage and propagation utilities.

use bumpalo::Bump;
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use crate::utils::errors::Result;
use crate::{ad::node::TapeNode, utils::errors::AtlasError};

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

    #[inline]
    /// Allocates a node in the bump arena and records it in the tape book.
    fn push(&mut self, n: TapeNode) -> NonNull<TapeNode> {
        let ptr = NonNull::from(self.bump.alloc(n));
        self.book.push(ptr);
        ptr
    }

    #[inline]
    /// Resets all adjoints on the current thread's tape.
    pub fn reset_adjoints() {
        Self::ensure_thread_tape();
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

    #[inline]
    /// Returns the index of a node in the tape book, if it exists.
    ///
    /// This is a linear scan; the cost grows with the number of recorded nodes.
    fn index_of(&self, p: NonNull<TapeNode>) -> Option<usize> {
        self.book.iter().position(|&q| q == p)
    }

    #[inline]
    /// Allocates and records a leaf node.
    pub fn new_leaf(&mut self) -> Option<NonNull<TapeNode>> {
        self.record(TapeNode::default())
    }

    #[inline]
    /// Records a node if recording is active, returning its pointer.
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
    pub fn propagate_from(&mut self, root: NonNull<TapeNode>) -> Result<()> {
        let start = self
            .index_of(root)
            .ok_or(AtlasError::NodeNotIndexedInTapeErr)?;
        for i in (0..=start).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
        Ok(())
    }

    /// Propagates adjoints from the current mark back to the start.
    pub fn propagate_mark_to_start(&mut self) {
        let end = self.mark.saturating_sub(1);
        for i in (0..=end).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
    }

    /// Propagates adjoints from the end of the tape down to the current mark.
    pub fn propagate_to_mark(&mut self) {
        let start = self.mark;
        let end = self.book.len().saturating_sub(1);
        if start > end {
            return;
        }
        for i in (start..=end).rev() {
            let node = unsafe { self.book[i].as_ref().clone() };
            node.propagate_into();
        }
    }

    /// Clears the tape and begins recording nodes in the thread-local tape.
    pub fn start_recording() {
        RECORDING_EPOCH.fetch_add(1, Ordering::AcqRel);
        RECORDING_ACTIVE.store(true, Ordering::Release);
        TAPE.with(|tc| {
            let mut t = tc.borrow_mut();
            t.bump.reset();
            t.book.clear();
            t.mark = 0;
            t.active = true;
        });
        TAPE_EPOCH.with(|epoch| {
            let global = RECORDING_EPOCH.load(Ordering::Acquire);
            epoch.set(global);
        });
    }

    /// Stops recording nodes on the thread-local tape.
    pub fn stop_recording() {
        RECORDING_ACTIVE.store(false, Ordering::Release);
        TAPE.with(|tc| tc.borrow_mut().active = false);
    }

    #[inline]
    /// Returns whether the thread-local tape is active.
    #[must_use]
    pub fn is_active() -> bool {
        Self::ensure_thread_tape();
        TAPE.with(|tc| tc.borrow().active)
    }

    /// Sets the current mark to the end of the tape.
    pub fn set_mark() {
        Self::ensure_thread_tape();
        TAPE.with(|tc| {
            let len = tc.borrow().book.len();
            tc.borrow_mut().mark = len;
        });
    }

    /// Truncates the tape back to the current mark.
    pub fn rewind_to_mark() {
        Self::ensure_thread_tape();
        TAPE.with(|tc| {
            let mark = tc.borrow().mark;
            tc.borrow_mut().book.truncate(mark);
        });
    }

    /// Clears the tape and resets the mark without changing active state.
    pub fn rewind_to_init() {
        Self::ensure_thread_tape();
        TAPE.with(|tc| {
            let mut t = tc.borrow_mut();
            t.bump.reset();
            t.book.clear();
            t.mark = 0;
        });
    }

    /// Ensures the thread-local tape matches the global recording state.
    pub fn ensure_thread_tape() -> bool {
        let active = RECORDING_ACTIVE.load(Ordering::Acquire);
        if !active {
            TAPE.with(|tc| tc.borrow_mut().active = false);
            return false;
        }

        let global_epoch = RECORDING_EPOCH.load(Ordering::Acquire);
        TAPE_EPOCH.with(|epoch| {
            if epoch.get() != global_epoch {
                TAPE.with(|tc| {
                    let mut t = tc.borrow_mut();
                    t.bump.reset();
                    t.book.clear();
                    t.mark = 0;
                    t.active = true;
                });
                epoch.set(global_epoch);
            } else {
                TAPE.with(|tc| tc.borrow_mut().active = true);
            }
        });
        true
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

thread_local! {
    static TAPE_EPOCH: Cell<usize> = Cell::new(0);
}

static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
static RECORDING_EPOCH: AtomicUsize = AtomicUsize::new(0);
