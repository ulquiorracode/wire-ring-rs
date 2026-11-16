//! Static slice storage for `#![no_std]` or stack allocations.

use crate::storage::StorageBackend;

/// Static slice-backed storage.
pub struct StaticStorage<'a> {
    ptr: *mut u8,
    capacity: usize,
    _marker: core::marker::PhantomData<&'a mut [u8]>,
}

// SAFETY: StaticStorage owns unique access to the underlying memory for lifetime 'a.
// SPSC synchronization coordinates non-overlapping memory access.
unsafe impl<'a> Send for StaticStorage<'a> {}
unsafe impl<'a> Sync for StaticStorage<'a> {}

impl<'a> StaticStorage<'a> {
    /// Creates a new static storage from an existing mutable slice.
    pub fn new(slice: &'a mut [u8]) -> Self {
        let capacity = slice.len();
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        let ptr = slice.as_mut_ptr();
        Self {
            ptr,
            capacity,
            _marker: core::marker::PhantomData,
        }
    }
}

// SAFETY:
// - capacity is verified to be a power of two in new().
// - ptr is non-null, aligned, and valid for reads and writes of capacity bytes.
// - as_mut_ptr(&self) returns ptr without taking &mut self, avoiding aliasing UB.
unsafe impl<'a> StorageBackend for StaticStorage<'a> {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    #[inline(always)]
    fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
    }

    #[inline(always)]
    fn is_mirrored(&self) -> bool {
        false
    }
}
