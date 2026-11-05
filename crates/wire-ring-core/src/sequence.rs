//! Sequence protocol with shadow pointers for ultra-low inter-core snooping.

use crate::atomic::CachePaddedAtomicUsize;
use core::sync::atomic::Ordering;

/// Lock-free sequence controller managing SPSC head and tail pointers.
///
/// Implements mechanical sympathy through power-of-two capacity masking
/// and decoupled atomic coordinates.
pub struct SequenceController {
    capacity: usize,
    mask: usize,
    head: CachePaddedAtomicUsize,
    tail: CachePaddedAtomicUsize,
}

impl SequenceController {
    /// Creates a new sequence controller for the specified `capacity`.
    ///
    /// # Panics
    /// Panics if `capacity` is not a power of two.
    pub const fn new(capacity: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        Self {
            capacity,
            mask: capacity - 1,
            head: CachePaddedAtomicUsize::new(0),
            tail: CachePaddedAtomicUsize::new(0),
        }
    }

    /// Returns the buffer capacity.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the bitmask for fast modulo indexing.
    #[inline(always)]
    pub const fn mask(&self) -> usize {
        self.mask
    }

    /// Returns the head position index within the circular capacity.
    #[inline(always)]
    pub const fn index_of(&self, seq: usize) -> usize {
        seq & self.mask
    }

    /// Loads the current head pointer with Acquire ordering.
    #[inline(always)]
    pub fn load_head(&self, order: Ordering) -> usize {
        self.head.load(order)
    }

    /// Stores a new head pointer with Release ordering.
    #[inline(always)]
    pub fn store_head(&self, seq: usize, order: Ordering) {
        self.head.store(seq, order);
    }

    /// Loads the current tail pointer with Acquire ordering.
    #[inline(always)]
    pub fn load_tail(&self, order: Ordering) -> usize {
        self.tail.load(order)
    }

    /// Stores a new tail pointer with Release ordering.
    #[inline(always)]
    pub fn store_tail(&self, seq: usize, order: Ordering) {
        self.tail.store(seq, order);
    }

    /// Computes the readable bytes from explicit sequence numbers.
    #[inline(always)]
    pub const fn available_read_from(head: usize, tail: usize) -> usize {
        head.wrapping_sub(tail)
    }

    /// Computes the writable bytes from explicit sequence numbers.
    #[inline(always)]
    pub const fn available_write_from(capacity: usize, head: usize, tail: usize) -> usize {
        capacity.saturating_sub(head.wrapping_sub(tail))
    }
}
