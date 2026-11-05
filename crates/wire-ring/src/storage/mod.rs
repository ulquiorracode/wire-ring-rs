//! Storage backend abstraction and implementations for wire-ring.

pub mod heap;
pub mod mirrored;
pub mod static_slice;

pub use heap::HeapStorage;
pub use mirrored::{MemoryError, MirroredStorage};
pub use static_slice::StaticStorage;

/// Trait defining the backing memory region for a ring buffer.
pub trait StorageBackend {
    /// Returns the logical usable capacity of the ring.
    fn capacity(&self) -> usize;

    /// Returns a raw pointer to the start of the memory region.
    fn as_ptr(&self) -> *const u8;

    /// Returns a raw mutable pointer to the start of the memory region.
    fn as_mut_ptr(&mut self) -> *mut u8;

    /// Returns whether this storage backend guarantees contiguous mirror indexing across wrap-around boundaries.
    fn is_mirrored(&self) -> bool;
}
