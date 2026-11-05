//! Ultra-high performance, zero-allocation virtual mirrored ring buffer
//! with unbreakable contiguous slice guarantees for Network & IPC.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

pub use wire_ring_core as core;

pub mod builder;
pub mod channel;
pub mod grant;
pub mod presets;
pub mod storage;

pub use builder::{RingBuilder, RingError};
pub use channel::{Consumer, Producer, split_ring};
pub use grant::{ReadGrant, WriteGrant};
pub use presets::Ring;
pub use storage::{HeapStorage, MemoryError, MirroredStorage, StaticStorage, StorageBackend};
