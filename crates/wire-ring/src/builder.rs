//! Fluent builder for configuring and constructing wire-ring instances.

use crate::channel::{Consumer, Producer, split_ring};
use crate::storage::{HeapStorage, MemoryError, MirroredStorage};

/// Construction errors for `RingBuilder`.
#[derive(Debug)]
pub enum RingError {
    /// Capacity error.
    InvalidCapacity(usize),
    /// Memory allocation or virtual mapping failure.
    Memory(MemoryError),
}

impl core::fmt::Display for RingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidCapacity(cap) => write!(
                f,
                "Invalid ring capacity {cap}: must be power-of-two and page-aligned"
            ),
            Self::Memory(e) => write!(f, "Ring memory error: {e}"),
        }
    }
}

impl std::error::Error for RingError {}

impl From<MemoryError> for RingError {
    fn from(err: MemoryError) -> Self {
        Self::Memory(err)
    }
}

/// Fluent builder for creating wire rings.
pub struct RingBuilder {
    capacity: usize,
    mirrored: bool,
}

impl Default for RingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RingBuilder {
    /// Creates a new ring builder with default 64 KiB capacity.
    pub const fn new() -> Self {
        Self {
            capacity: 64 * 1024,
            mirrored: false,
        }
    }

    /// Sets the usable buffer capacity in bytes.
    ///
    /// Must be a power of two.
    pub const fn capacity(mut self, bytes: usize) -> Self {
        self.capacity = bytes;
        self
    }

    /// Enables hardware MMU-accelerated Virtual Mirrored Memory.
    ///
    /// Guarantees that any leased slice up to `capacity` is contiguous
    /// in virtual memory without wrap-around splits.
    pub const fn mirrored(mut self) -> Self {
        self.mirrored = true;
        self
    }

    /// Builds a heap-backed ring.
    pub fn build_heap(self) -> Result<(Producer<HeapStorage>, Consumer<HeapStorage>), RingError> {
        if !self.capacity.is_power_of_two() {
            return Err(RingError::InvalidCapacity(self.capacity));
        }
        let storage = HeapStorage::new(self.capacity);
        Ok(split_ring(storage))
    }

    /// Builds an MMU-mirrored virtual ring.
    pub fn build_mirrored(
        self,
    ) -> Result<(Producer<MirroredStorage>, Consumer<MirroredStorage>), RingError> {
        let storage = MirroredStorage::new(self.capacity)?;
        Ok(split_ring(storage))
    }
}
