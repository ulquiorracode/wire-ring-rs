//! High-level ergonomics, one-call presets, and facades for `wire-ring`.

#[cfg(feature = "builder")]
use crate::builder::{RingBuilder, RingError};
#[cfg(feature = "alloc")]
use crate::channel::{Consumer, Producer, split_ring};
#[cfg(all(feature = "alloc", feature = "builder"))]
use crate::storage::HeapStorage;
#[cfg(all(feature = "std", feature = "builder"))]
use crate::storage::MirroredStorage;
#[cfg(feature = "alloc")]
use crate::storage::StaticStorage;

/// Main facade providing ready-to-use factory constructors and presets.
pub struct Ring;

impl Ring {
    /// Returns a fluent builder for customized configuration.
    #[cfg(feature = "builder")]
    pub const fn builder() -> RingBuilder {
        RingBuilder::new()
    }

    /// Creates an ultra-low-latency MMU-mirrored ring buffer for network UDP/TCP streaming.
    ///
    /// # Default Capacity
    /// Defaults to `capacity` bytes (must be multiple of 64 KiB page allocation granularity).
    #[cfg(all(feature = "builder", feature = "std"))]
    pub fn network_default(
        capacity: usize,
    ) -> Result<(Producer<MirroredStorage>, Consumer<MirroredStorage>), RingError> {
        RingBuilder::new()
            .capacity(capacity)
            .mirrored()
            .build_mirrored()
    }

    /// Creates a standard heap-allocated SPSC ring buffer.
    #[cfg(all(feature = "builder", feature = "alloc"))]
    pub fn heap(
        capacity: usize,
    ) -> Result<(Producer<HeapStorage>, Consumer<HeapStorage>), RingError> {
        RingBuilder::new().capacity(capacity).build_heap()
    }

    /// Creates a static slice-backed SPSC ring buffer for `#![no_std]` or stack buffers.
    #[cfg(feature = "alloc")]
    pub fn from_static(
        slice: &mut [u8],
    ) -> (Producer<StaticStorage<'_>>, Consumer<StaticStorage<'_>>) {
        let storage = StaticStorage::new(slice);
        split_ring(storage)
    }
}
