//! Ultra-high performance, zero-allocation virtual mirrored ring buffer
//! with unbreakable contiguous slice guarantees for Network & IPC.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub use wire_ring_core as wire_core;

#[cfg(feature = "builder")]
pub mod builder;
#[cfg(feature = "alloc")]
pub mod channel;
pub mod grant;
pub mod presets;
pub mod storage;

#[cfg(feature = "builder")]
pub use builder::{RingBuilder, RingError};
#[cfg(feature = "alloc")]
pub use channel::{Consumer, Producer, split_ring};
pub use grant::{ReadGrant, WriteGrant};
pub use presets::Ring;
pub use storage::{MemoryError, StaticStorage, StorageBackend};
#[cfg(feature = "alloc")]
pub use storage::HeapStorage;
#[cfg(feature = "std")]
pub use storage::MirroredStorage;
