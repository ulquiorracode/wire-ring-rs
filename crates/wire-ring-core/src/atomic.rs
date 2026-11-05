//! Cache-line padded atomic primitives preventing false sharing on multi-core CPUs.

use core::sync::atomic::{AtomicUsize, Ordering};

/// 64-byte L1 data cache-line padded `AtomicUsize`.
///
/// Ensures the atomic integer occupies its own dedicated cache line, eliminating
/// false sharing and cache-line invalidation traffic between producer and consumer cores.
#[repr(align(64))]
#[derive(Debug, Default)]
pub struct CachePaddedAtomicUsize {
    value: AtomicUsize,
}

impl CachePaddedAtomicUsize {
    /// Creates a new cache-padded atomic usize with the given initial value.
    #[inline(always)]
    pub const fn new(val: usize) -> Self {
        Self {
            value: AtomicUsize::new(val),
        }
    }

    /// Loads the value with the specified atomic memory ordering.
    #[inline(always)]
    pub fn load(&self, order: Ordering) -> usize {
        self.value.load(order)
    }

    /// Stores a new value with the specified atomic memory ordering.
    #[inline(always)]
    pub fn store(&self, val: usize, order: Ordering) {
        self.value.store(val, order);
    }
}
