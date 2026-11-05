//! RAII Grant tokens providing zero-copy leases and panic-safe auto-rollback.

use core::ops::{Deref, DerefMut};

/// RAII token leasing a contiguous slice of memory for reading.
///
/// Data remains leased until committed via `commit()`. If dropped without commit,
/// no bytes are consumed from the ring buffer.
pub struct ReadGrant<'a> {
    slice: &'a [u8],
    committed: bool,
    commit_fn: Option<Box<dyn FnOnce(usize) + 'a>>,
}

impl<'a> ReadGrant<'a> {
    /// Creates a new read grant.
    #[inline(always)]
    pub(crate) fn new(slice: &'a [u8], commit: impl FnOnce(usize) + 'a) -> Self {
        Self {
            slice,
            committed: false,
            commit_fn: Some(Box::new(commit)),
        }
    }

    /// Commits and consumes `count` bytes from the ring buffer.
    ///
    /// # Panics
    /// Panics if `count > self.len()`.
    #[inline]
    pub fn commit(mut self, count: usize) {
        assert!(
            count <= self.slice.len(),
            "commit count exceeds leased slice length"
        );
        if let Some(cb) = self.commit_fn.take() {
            cb(count);
            self.committed = true;
        }
    }

    /// Commits the entire leased slice.
    #[inline(always)]
    pub fn commit_all(self) {
        let len = self.slice.len();
        self.commit(len);
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
    committed: bool,
    commit_fn: Option<Box<dyn FnOnce(usize) + 'a>>,
}

impl<'a> WriteGrant<'a> {
    /// Creates a new write grant.
    #[inline(always)]
    pub(crate) fn new(slice: &'a mut [u8], commit: impl FnOnce(usize) + 'a) -> Self {
        Self {
            slice,
            committed: false,
            commit_fn: Some(Box::new(commit)),
        }
    }

    /// Commits `count` written bytes to the ring buffer, making them visible to the consumer.
    ///
    /// # Panics
    /// Panics if `count > self.len()`.
    #[inline]
    pub fn commit(mut self, count: usize) {
        assert!(
            count <= self.slice.len(),
            "commit count exceeds leased slice length"
        );
        if let Some(cb) = self.commit_fn.take() {
            cb(count);
            self.committed = true;
        }
    }

    /// Commits the entire leased slice.
    #[inline(always)]
    pub fn commit_all(self) {
        let len = self.slice.len();
        self.commit(len);
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
