//! Core `#![no_std]` algorithms, cache-padded atomics, and sequence management for `wire-ring`.

#![no_std]

pub mod atomic;
pub mod sequence;

pub use atomic::CachePaddedAtomicUsize;
pub use sequence::SequenceController;

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::Ordering;

    #[test]
    fn test_sequence_controller_basics() {
        let seq = SequenceController::new(1024);
        assert_eq!(seq.capacity(), 1024);
        assert_eq!(seq.mask(), 1023);
        assert_eq!(seq.index_of(1025), 1);

        assert_eq!(SequenceController::available_read_from(0, 0), 0);
        assert_eq!(SequenceController::available_write_from(1024, 0, 0), 1024);

        seq.store_head(100, Ordering::Relaxed);
        assert_eq!(seq.load_head(Ordering::Relaxed), 100);
        assert_eq!(SequenceController::available_read_from(100, 0), 100);
        assert_eq!(SequenceController::available_write_from(1024, 100, 0), 924);
    }
}
