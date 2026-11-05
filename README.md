# wire-ring

[![CI](https://github.com/ulquiorracode/wire-ring-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ulquiorracode/wire-ring-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/wire-ring.svg)](https://crates.io/crates/wire-ring)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

Ultra-high performance, zero-allocation virtual mirrored byte ring buffer with unbreakable contiguous slice guarantees for Network & IPC.

---

## Features

- **Virtual Mirrored Pages**: Hardware MMU page mirroring eliminates wrap-around slice fragmentation. Leases are **always contiguous** without `memcpy`.
- **Zero Allocations on Hot Path**: Pre-allocated virtual or heap memory with zero allocations during read/write cycles.
- **Scrooge Mechanical Sympathy**: 64-byte L1D cache-line padding and local shadow pointers reduce cross-core atomic snooping by >95%.
- **RAII Transaction Grants**: `WriteGrant` and `ReadGrant` ensure panic-safe operations with automatic rollback if dropped without `commit()`.
- **Platform Native**: Direct OS virtual memory APIs on Windows (`VirtualAlloc2` / `MapViewOfFile3`) and Linux (`memfd_create` / `mmap`).

---

## Quickstart

```rust
use wire_ring::Ring;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a 64 KiB network ring buffer with hardware MMU mirroring
    let (mut tx, mut rx) = Ring::network_default(64 * 1024)?;

    // 2. Producer: Lease contiguous memory, write data, and commit
    {
        let mut grant = tx.grant(512).expect("available buffer space");
        grant[..5].copy_from_slice(b"HELLO");
        // Commit exactly the written bytes. Dropping without commit auto-rolls back.
        grant.commit(5);
    }

    // 3. Consumer: Lease committed memory without copying
    {
        let grant = rx.grant().expect("available data");
        assert_eq!(&grant[..5], b"HELLO");
        grant.commit(5);
    }

    Ok(())
}
```

---

## Fluent Builder

```rust
use wire_ring::RingBuilder;

let (mut tx, mut rx) = RingBuilder::new()
    .capacity(128 * 1024)
    .mirrored()
    .build_mirrored()?;
```

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
