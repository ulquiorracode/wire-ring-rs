//! RAII Grant tokens providing zero-copy leases and panic-safe auto-rollback.

use core::ops::{Deref, DerefMut};
use wire_ring_core::SequenceController;

/// RAII token leasing a contiguous slice of memory for reading.
///
/// Data remains leased until committed via `commit()`. If dropped without commit,
/// no bytes are consumed from the ring buffer.
pub struct ReadGrant<'a> {
    slice: &'a [u8],
    seq: &'a SequenceController,
}

impl<'a> ReadGrant<'a> {
    /// Creates a new read grant.
    #[inline(always)]
    pub(crate) fn new(slice: &'a [u8], seq: &'a SequenceController) -> Self {
        Self { slice, seq }
    }

    /// Commits and consumes `count` bytes from the ring buffer.
    ///
    /// # Panics
    /// Panics if `count > self.len()`.
    #[inline]
    pub fn commit(self, count: usize) {
        assert!(
            count <= self.slice.len(),
            "commit count exceeds leased slice length"
        );
        let cur = self.seq.load_tail(core::sync::atomic::Ordering::Relaxed);
        self.seq.store_tail(
            cur.wrapping_add(count),
            core::sync::atomic::Ordering::Release,
        );
        core::mem::forget(self);
    }

    /// Commits the entire leased slice.
    #[inline(always)]
    pub fn commit_all(self) {
        let len = self.slice.len();
        self.commit(len);
    }
}

impl<'a> Drop for ReadGrant<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        // Auto-rollback: tail is not incremented
    }
}

impl<'a> Deref for ReadGrant<'a> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'a> AsRef<[u8]> for ReadGrant<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.slice
    }
}

/// RAII token leasing a contiguous mutable slice of memory for writing.
///
/// If dropped before calling `.commit(n)`, the grant safely rolls back,
/// preventing corrupted or half-written packets from entering the stream on panics.
pub struct WriteGrant<'a> {
    slice: &'a mut [u8],
    seq: &'a SequenceController,
}

impl<'a> WriteGrant<'a> {
    /// Creates a new write grant.
    #[inline(always)]
    pub(crate) fn new(slice: &'a mut [u8], seq: &'a SequenceController) -> Self {
        Self { slice, seq }
    }

    /// Commits `count` written bytes to the ring buffer, making them visible to the consumer.
    ///
    /// # Panics
    /// Panics if `count > self.len()`.
    #[inline]
    pub fn commit(self, count: usize) {
        assert!(
            count <= self.slice.len(),
            "commit count exceeds leased slice length"
        );
        let cur = self.seq.load_head(core::sync::atomic::Ordering::Relaxed);
        self.seq.store_head(
            cur.wrapping_add(count),
            core::sync::atomic::Ordering::Release,
        );
        core::mem::forget(self);
    }

    /// Commits the entire leased slice.
    #[inline(always)]
    pub fn commit_all(self) {
        let len = self.slice.len();
        self.commit(len);
    }
}

impl<'a> Drop for WriteGrant<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        // Auto-rollback: head is not incremented
    }
}

impl<'a> Deref for WriteGrant<'a> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'a> DerefMut for WriteGrant<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

impl<'a> AsMut<[u8]> for WriteGrant<'a> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        self.slice
    }
}
