//! Split Single-Producer Single-Consumer (SPSC) channel handles.

use crate::grant::{ReadGrant, WriteGrant};
use crate::storage::StorageBackend;
use core::cell::UnsafeCell;
use core::sync::atomic::Ordering;
use std::sync::Arc;
use wire_ring_core::SequenceController;

struct SharedRing<S> {
    storage: UnsafeCell<S>,
    sequence: SequenceController,
}

// SAFETY: Synchronization of non-overlapping read/write slices is enforced via SequenceController (SPSC Acquire/Release).
unsafe impl<S: StorageBackend + Send> Sync for SharedRing<S> {}

/// Producer half of the ring buffer.
pub struct Producer<S> {
    shared: Arc<SharedRing<S>>,
    cached_tail: usize,
}

// SAFETY: Producer has sole authority to write to the uncommitted head region.
unsafe impl<S: StorageBackend + Send> Send for Producer<S> {}

impl<S: StorageBackend> Producer<S> {
    /// Returns the total usable capacity of the ring.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.shared.sequence.capacity()
    }

    /// Returns the number of writable bytes currently available.
    #[inline]
    pub fn available_write(&mut self) -> usize {
        let head = self.shared.sequence.load_head(Ordering::Relaxed);
        let tail = self.shared.sequence.load_tail(Ordering::Acquire);
        self.cached_tail = tail;
        let cap = self.shared.sequence.capacity();
        SequenceController::available_write_from(cap, head, tail)
    }

    /// Leases a contiguous mutable slice of bytes for zero-copy writing.
    ///
    /// If backed by `MirroredStorage`, the granted slice is NEVER truncated at the ring wrap-around boundary.
    pub fn grant(&mut self, requested: usize) -> Option<WriteGrant<'_>> {
        if requested == 0 {
            return Some(WriteGrant::new(&mut [], &self.shared.sequence));
        }

        let cap = self.shared.sequence.capacity();
        let head = self.shared.sequence.load_head(Ordering::Relaxed);

        let mut free = SequenceController::available_write_from(cap, head, self.cached_tail);
        if free < requested {
            self.cached_tail = self.shared.sequence.load_tail(Ordering::Acquire);
            free = SequenceController::available_write_from(cap, head, self.cached_tail);
        }

        if free == 0 {
            return None;
        }

        let grant_len = requested.min(free);
        let head_idx = self.shared.sequence.index_of(head);

        // If not mirrored, we must bound to end of physical buffer
        let contiguous_len = if unsafe { (*self.shared.storage.get()).is_mirrored() } {
            grant_len
        } else {
            grant_len.min(cap - head_idx)
        };

        if contiguous_len == 0 {
            return None;
        }

        // SAFETY: The producer uniquely writes to [head .. head + contiguous_len].
        // Storage is held in an UnsafeCell and the consumer has committed past this region with Acquire/Release.
        let slice = unsafe {
            let base = (*self.shared.storage.get()).as_mut_ptr();
            core::slice::from_raw_parts_mut(base.add(head_idx), contiguous_len)
        };

        Some(WriteGrant::new(slice, &self.shared.sequence))
    }

    /// Writes the entire `src` slice into the ring buffer.
    ///
    /// Returns the number of bytes written.
    pub fn write_slice(&mut self, mut src: &[u8]) -> usize {
        let mut total_written = 0;
        while !src.is_empty() {
            let Some(mut grant) = self.grant(src.len()) else {
                break;
            };
            let to_write = grant.len().min(src.len());
            grant[..to_write].copy_from_slice(&src[..to_write]);
            grant.commit(to_write);
            total_written += to_write;
            src = &src[to_write..];
        }
        total_written
    }
}

/// Consumer half of the ring buffer.
pub struct Consumer<S> {
    shared: Arc<SharedRing<S>>,
    cached_head: usize,
}

// SAFETY: Consumer has sole authority to read from the committed tail region.
unsafe impl<S: StorageBackend + Send> Send for Consumer<S> {}

impl<S: StorageBackend> Consumer<S> {
    /// Returns the total usable capacity of the ring.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.shared.sequence.capacity()
    }

    /// Returns the number of readable bytes currently available.
    #[inline]
    pub fn available_read(&mut self) -> usize {
        let tail = self.shared.sequence.load_tail(Ordering::Relaxed);
        let head = self.shared.sequence.load_head(Ordering::Acquire);
        self.cached_head = head;
        SequenceController::available_read_from(head, tail)
    }

    /// Leases a contiguous slice of committed bytes for zero-copy reading.
    ///
    /// If backed by `MirroredStorage`, the granted slice is NEVER truncated at the ring wrap-around boundary.
    pub fn grant(&mut self) -> Option<ReadGrant<'_>> {
        let tail = self.shared.sequence.load_tail(Ordering::Relaxed);

        let mut available = SequenceController::available_read_from(self.cached_head, tail);
        if available == 0 {
            self.cached_head = self.shared.sequence.load_head(Ordering::Acquire);
            available = SequenceController::available_read_from(self.cached_head, tail);
        }

        if available == 0 {
            return None;
        }

        let cap = self.shared.sequence.capacity();
        let tail_idx = self.shared.sequence.index_of(tail);

        let contiguous_len = if unsafe { (*self.shared.storage.get()).is_mirrored() } {
            available.min(cap)
        } else {
            available.min(cap - tail_idx)
        };

        if contiguous_len == 0 {
            return None;
        }

        // SAFETY: The consumer uniquely reads from [tail .. tail + contiguous_len].
        // Storage is accessed via UnsafeCell and producer has committed bytes with Release ordering.
        let slice = unsafe {
            let base = (*self.shared.storage.get()).as_ptr();
            core::slice::from_raw_parts(base.add(tail_idx), contiguous_len)
        };

        Some(ReadGrant::new(slice, &self.shared.sequence))
    }

    /// Reads committed bytes into `dst`.
    ///
    /// Returns the number of bytes read.
    pub fn read_slice(&mut self, mut dst: &mut [u8]) -> usize {
        let mut total_read = 0;
        while !dst.is_empty() {
            let Some(grant) = self.grant() else {
                break;
            };
            let to_read = grant.len().min(dst.len());
            dst[..to_read].copy_from_slice(&grant[..to_read]);
            grant.commit(to_read);
            total_read += to_read;
            dst = &mut dst[to_read..];
        }
        total_read
    }
}

/// Splits a storage backend into an SPSC `(Producer, Consumer)` pair.
pub fn split_ring<S: StorageBackend>(storage: S) -> (Producer<S>, Consumer<S>) {
    let capacity = storage.capacity();
    let shared = Arc::new(SharedRing {
        storage: UnsafeCell::new(storage),
        sequence: SequenceController::new(capacity),
    });

    let producer = Producer {
        shared: Arc::clone(&shared),
        cached_tail: 0,
    };
    let consumer = Consumer {
        shared,
        cached_head: 0,
    };
    (producer, consumer)
}
