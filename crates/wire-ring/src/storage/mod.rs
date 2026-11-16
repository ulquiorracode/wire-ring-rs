//! Storage backend abstraction and implementations for wire-ring.

#[cfg(feature = "alloc")]
pub mod heap;
pub mod mirrored;
pub mod static_slice;

#[cfg(feature = "alloc")]
pub use heap::HeapStorage;
pub use mirrored::MemoryError;
#[cfg(feature = "std")]
pub use mirrored::MirroredStorage;
pub use static_slice::StaticStorage;

/// Trait defining the backing memory region for a ring buffer.
///
/// # Safety
/// Implementors of this trait must guarantee:
/// - `capacity()` returns a non-zero power-of-two usable capacity.
/// - `as_ptr()` returns a non-null, properly aligned, readable pointer to at least `capacity()` bytes
///   (or `2 * capacity()` bytes if `is_mirrored()` returns `true`).
/// - `as_mut_ptr()` returns a non-null, properly aligned, writable pointer to the same memory region as `as_ptr()`.
/// - Calling `as_mut_ptr(&self)` concurrently with `as_ptr(&self)` is sound because it takes `&self`
///   and does not create unique mutable borrows to the whole storage backend; disjoint slice access
///   within the buffer is coordinated by sequence synchronization.
/// - The memory allocation remains pinned and valid for the lifetime of the storage instance.
pub unsafe trait StorageBackend: Send + Sync {
    /// Returns the logical usable capacity of the ring.
    fn capacity(&self) -> usize;

    /// Returns a raw pointer to the start of the memory region.
    fn as_ptr(&self) -> *const u8;

    /// Returns a raw mutable pointer to the start of the memory region.
    fn as_mut_ptr(&self) -> *mut u8;

    /// Returns whether this storage backend guarantees contiguous mirror indexing across wrap-around boundaries.
    fn is_mirrored(&self) -> bool;
}
