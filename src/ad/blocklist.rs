//! Block-based slab allocator for tape nodes.
//!
//! [`BlockList<T>`] stores elements in fixed-capacity blocks (`Box<[MaybeUninit<T>]>`).
//! Pointers into blocks remain stable because the heap data behind each `Box` never
//! moves, even when the outer `Vec<Box<…>>` grows.  This makes it safe to hand out
//! `NonNull<T>` pointers that survive across subsequent allocations.
//!
//! The allocator supports mark/rewind: [`rewind_to`](BlockList::rewind_to) drops
//! elements past the mark (running destructors) while keeping the blocks for reuse,
//! so the next batch of allocations incurs no heap traffic.

use std::mem::MaybeUninit;
use std::ptr::NonNull;

/// Default number of elements per block.
const DEFAULT_BLOCK_CAP: usize = 4096;

/// A typed slab allocator that stores `T` values in fixed-capacity blocks.
///
/// Element `i` lives at `blocks[i / block_cap][i % block_cap]`.
pub struct BlockList<T> {
    blocks: Vec<Box<[MaybeUninit<T>]>>,
    block_cap: usize,
    len: usize,
}

impl<T> BlockList<T> {
    /// Creates an empty `BlockList` with the given per-block capacity.
    #[must_use]
    pub fn new(block_cap: usize) -> Self {
        assert!(block_cap > 0, "block_cap must be > 0");
        Self {
            blocks: Vec::new(),
            block_cap,
            len: 0,
        }
    }

    /// Creates an empty `BlockList` with the default block capacity (4096).
    #[must_use]
    pub fn with_default_cap() -> Self {
        Self::new(DEFAULT_BLOCK_CAP)
    }

    /// Returns the number of live elements.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if there are no live elements.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Allocates a new element, returning a stable pointer to it.
    ///
    /// The pointer remains valid until the element is rewound past or the
    /// `BlockList` is dropped.
    #[inline]
    pub fn alloc(&mut self, val: T) -> NonNull<T> {
        let block_idx = self.len / self.block_cap;
        let slot_idx = self.len % self.block_cap;

        // Grow: add a new block if we've filled all existing ones.
        if block_idx >= self.blocks.len() {
            let block: Box<[MaybeUninit<T>]> = (0..self.block_cap)
                .map(|_| MaybeUninit::uninit())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            self.blocks.push(block);
        }

        let slot = &mut self.blocks[block_idx][slot_idx];
        slot.write(val);
        self.len += 1;

        // SAFETY: `slot` is now initialised and lives in heap-allocated
        // `Box<[MaybeUninit<T>]>` which doesn't move.
        unsafe { NonNull::new_unchecked(slot.as_mut_ptr()) }
    }

    /// Returns an immutable reference to the element at `index`.
    ///
    /// # Panics
    /// Panics if `index >= self.len`.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> &T {
        assert!(
            index < self.len,
            "BlockList index {index} out of bounds (len {})",
            self.len
        );
        let block_idx = index / self.block_cap;
        let slot_idx = index % self.block_cap;
        // SAFETY: all slots in [0..len) have been initialised via `alloc`.
        unsafe { self.blocks[block_idx][slot_idx].assume_init_ref() }
    }

    /// Returns a mutable reference to the element at `index`.
    ///
    /// # Panics
    /// Panics if `index >= self.len`.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> &mut T {
        assert!(
            index < self.len,
            "BlockList index {index} out of bounds (len {})",
            self.len
        );
        let block_idx = index / self.block_cap;
        let slot_idx = index % self.block_cap;
        // SAFETY: all slots in [0..len) have been initialised via `alloc`.
        unsafe { self.blocks[block_idx][slot_idx].assume_init_mut() }
    }

    /// Drops elements in `[mark .. len)` and sets `len = mark`.
    ///
    /// Blocks are kept for reuse — no heap deallocation occurs.
    pub fn rewind_to(&mut self, mark: usize) {
        assert!(mark <= self.len, "mark {mark} exceeds len {}", self.len);
        // Drop elements in reverse order (matches stack-like semantics).
        for i in (mark..self.len).rev() {
            let block_idx = i / self.block_cap;
            let slot_idx = i % self.block_cap;
            // SAFETY: slots in [0..len) are initialised.
            unsafe {
                self.blocks[block_idx][slot_idx].assume_init_drop();
            }
        }
        self.len = mark;
    }

    /// Drops all elements and resets length to zero.  Blocks are kept.
    pub fn reset(&mut self) {
        self.rewind_to(0);
    }
}

impl<T> Drop for BlockList<T> {
    fn drop(&mut self) {
        // Drop all live elements before the blocks themselves are freed.
        self.reset();
    }
}

impl<T> Default for BlockList<T> {
    fn default() -> Self {
        Self::new(DEFAULT_BLOCK_CAP)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn basic_alloc_and_get() {
        let mut bl: BlockList<u64> = BlockList::new(4);
        for i in 0..10u64 {
            bl.alloc(i);
        }
        assert_eq!(bl.len(), 10);
        for i in 0..10 {
            assert_eq!(*bl.get(i), i as u64);
        }
    }

    #[test]
    fn pointer_stability() {
        let mut bl: BlockList<u64> = BlockList::new(2);
        let p0 = bl.alloc(42);
        let p1 = bl.alloc(43);
        // Force growth across blocks.
        let p2 = bl.alloc(44);
        let p3 = bl.alloc(45);
        let _p4 = bl.alloc(46);

        // Earlier pointers must still be valid.
        assert_eq!(unsafe { *p0.as_ptr() }, 42);
        assert_eq!(unsafe { *p1.as_ptr() }, 43);
        assert_eq!(unsafe { *p2.as_ptr() }, 44);
        assert_eq!(unsafe { *p3.as_ptr() }, 45);
    }

    #[test]
    fn rewind_drops_elements() {
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counted(u32);
        impl Drop for Counted {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROP_COUNT.store(0, Ordering::SeqCst);
        let mut bl = BlockList::new(4);
        for i in 0..6u32 {
            bl.alloc(Counted(i));
        }
        assert_eq!(bl.len(), 6);

        bl.rewind_to(2);
        assert_eq!(bl.len(), 2);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 4); // dropped 4 elements

        // Surviving elements are still valid.
        assert_eq!(bl.get(0).0, 0);
        assert_eq!(bl.get(1).0, 1);

        // Can allocate again in the same blocks.
        bl.alloc(Counted(99));
        assert_eq!(bl.len(), 3);
        assert_eq!(bl.get(2).0, 99);
    }

    #[test]
    fn reset_drops_all() {
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counted;
        impl Drop for Counted {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROP_COUNT.store(0, Ordering::SeqCst);
        let mut bl = BlockList::new(4);
        for _ in 0..10 {
            bl.alloc(Counted);
        }
        bl.reset();
        assert_eq!(bl.len(), 0);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 10);
        assert!(!bl.blocks.is_empty()); // blocks kept for reuse
    }

    #[test]
    fn rewind_reuse_blocks() {
        let mut bl: BlockList<u64> = BlockList::new(4);
        for i in 0..8 {
            bl.alloc(i);
        }
        assert_eq!(bl.blocks.len(), 2);
        bl.rewind_to(0);
        assert_eq!(bl.blocks.len(), 2); // blocks kept

        // Re-allocate — should reuse existing blocks.
        for i in 0..8 {
            bl.alloc(i + 100);
        }
        assert_eq!(bl.blocks.len(), 2);
        assert_eq!(*bl.get(0), 100);
    }
}
