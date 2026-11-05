//! Static slice storage for `#![no_std]` or stack allocations.

use crate::storage::StorageBackend;

/// Static slice-backed storage.
pub struct StaticStorage<'a> {
    slice: &'a mut [u8],
    capacity: usize,
}

impl<'a> StaticStorage<'a> {
    /// Creates a new static storage from an existing mutable slice.
    pub fn new(slice: &'a mut [u8]) -> Self {
        let capacity = slice.len();
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        Self { slice, capacity }
    }
}

impl<'a> StorageBackend for StaticStorage<'a> {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const u8 {
        self.slice.as_ptr()
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.slice.as_mut_ptr()
    }

    #[inline(always)]
    fn is_mirrored(&self) -> bool {
        false
    }
}
