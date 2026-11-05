//! Heap-allocated standard storage fallback.

use crate::storage::StorageBackend;
use std::vec::Vec;

/// Standard heap-allocated ring storage.
pub struct HeapStorage {
    buffer: Vec<u8>,
    capacity: usize,
}

impl HeapStorage {
    /// Creates a new heap-allocated storage with the requested capacity.
    ///
    /// If `duplicate` is true, allocates $2 \times capacity$ to allow manual linearization.
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        let buffer = vec![0u8; capacity];
        Self { buffer, capacity }
    }
}

impl StorageBackend for HeapStorage {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }

    #[inline(always)]
    fn is_mirrored(&self) -> bool {
        false
    }
}
