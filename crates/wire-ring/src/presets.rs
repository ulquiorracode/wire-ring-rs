//! High-level ergonomics, one-call presets, and facades for `wire-ring`.

use crate::builder::{RingBuilder, RingError};
use crate::channel::{Consumer, Producer, split_ring};
use crate::storage::{HeapStorage, MirroredStorage, StaticStorage};

/// Main facade providing ready-to-use factory constructors and presets.
pub struct Ring;

impl Ring {
    /// Returns a fluent builder for customized configuration.
    pub const fn builder() -> RingBuilder {
        RingBuilder::new()
    }

    /// Creates an ultra-low-latency MMU-mirrored ring buffer for network UDP/TCP streaming.
    ///
    /// # Default Capacity
    /// Defaults to `capacity` bytes (must be multiple of 64 KiB page allocation granularity).
    pub fn network_default(
        capacity: usize,
    ) -> Result<(Producer<MirroredStorage>, Consumer<MirroredStorage>), RingError> {
        RingBuilder::new()
            .capacity(capacity)
            .mirrored()
            .build_mirrored()
    }

    /// Creates a standard heap-allocated SPSC ring buffer.
    pub fn heap(
        capacity: usize,
    ) -> Result<(Producer<HeapStorage>, Consumer<HeapStorage>), RingError> {
        RingBuilder::new().capacity(capacity).build_heap()
    }

    /// Creates a static slice-backed SPSC ring buffer for `#![no_std]` or stack buffers.
    pub fn from_static(
        slice: &mut [u8],
    ) -> (Producer<StaticStorage<'_>>, Consumer<StaticStorage<'_>>) {
        let storage = StaticStorage::new(slice);
        split_ring(storage)
    }
}
