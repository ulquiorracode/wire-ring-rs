use crc32fast::Hasher;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use wire_ring::Ring;

#[test]
fn test_heap_ring_write_read() {
    let (mut tx, mut rx) = Ring::heap(1024).expect("build heap ring");
    assert_eq!(tx.capacity(), 1024);
    assert_eq!(rx.capacity(), 1024);

    let data = b"Hello Wire Ring Architecture";
    let written = tx.write_slice(data);
    assert_eq!(written, data.len());

    let mut buf = [0u8; 64];
    let read = rx.read_slice(&mut buf);
    assert_eq!(read, data.len());
    assert_eq!(&buf[..read], data);
}

#[test]
fn test_mirrored_ring_contiguous_boundary_lease() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("build mirrored ring");
    let cap = tx.capacity();

    let prefill = cap - 16;
    let dummy = vec![0xAAu8; prefill];
    assert_eq!(tx.write_slice(&dummy), prefill);

    let mut consume_dummy = vec![0u8; prefill];
    assert_eq!(rx.read_slice(&mut consume_dummy), prefill);

    {
        let mut grant = tx.grant(32).expect("grant across boundary");
        assert_eq!(grant.len(), 32);

        for (i, b) in grant.iter_mut().enumerate() {
            *b = (i + 1) as u8;
        }
        grant.commit(32);
    }

    {
        let grant = rx.grant().expect("read grant across boundary");
        assert_eq!(grant.len(), 32);
        for (i, &b) in grant.iter().enumerate() {
            assert_eq!(b, (i + 1) as u8);
        }
        grant.commit(32);
    }
}

#[test]
fn test_exact_capacity_saturation_and_zero_space() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");
    let cap = tx.capacity();

    assert_eq!(tx.available_write(), cap);
    assert_eq!(rx.available_read(), 0);

    // 1. Fill exactly to 100% capacity
    let full_buf = vec![0x5Au8; cap];
    assert_eq!(tx.write_slice(&full_buf), cap);

    // 2. Further write grants MUST return None or 0 available
    assert_eq!(tx.available_write(), 0);
    assert!(tx.grant(1).is_none());
    assert_eq!(tx.write_slice(b"overflow"), 0);

    // 3. Consumer sees exactly full capacity
    assert_eq!(rx.available_read(), cap);
    let mut read_buf = vec![0u8; cap];
    assert_eq!(rx.read_slice(&mut read_buf), cap);
    assert_eq!(read_buf, full_buf);

    // 4. Now ring is empty again
    assert_eq!(rx.available_read(), 0);
    assert_eq!(tx.available_write(), cap);
}

#[test]
fn test_partial_commit_incremental_alignment() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");

    // Producer requests 1024 bytes, but commits in increments: 100, then 200, then 50
    {
        let mut grant = tx.grant(1024).expect("grant 1024");
        grant[..100].fill(0x11);
        grant.commit(100);
    }
    assert_eq!(rx.available_read(), 100);

    {
        let mut grant = tx.grant(1024).expect("grant 1024");
        grant[..200].fill(0x22);
        grant.commit(200);
    }
    assert_eq!(rx.available_read(), 300);

    {
        let mut grant = tx.grant(1024).expect("grant 1024");
        grant[..50].fill(0x33);
        grant.commit(50);
    }
    assert_eq!(rx.available_read(), 350);

    // Consumer reads in non-aligned chunks
    let mut part1 = [0u8; 50];
    assert_eq!(rx.read_slice(&mut part1), 50);
    assert!(part1.iter().all(|&b| b == 0x11));

    let mut part2 = [0u8; 100];
    assert_eq!(rx.read_slice(&mut part2), 100);
    assert!(part2[..50].iter().all(|&b| b == 0x11));
    assert!(part2[50..].iter().all(|&b| b == 0x22));

    let mut remaining = [0u8; 200];
    assert_eq!(rx.read_slice(&mut remaining), 200);
    assert!(remaining[..150].iter().all(|&b| b == 0x22));
    assert!(remaining[150..200].iter().all(|&b| b == 0x33));

    assert_eq!(rx.available_read(), 0);
}

#[test]
fn test_cross_boundary_fuzzing_with_crc32() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");

    // Pseudorandom packet generation (LFSR) with CRC32 verification across boundaries
    let mut lfsr: u32 = 0xACE1;
    let mut next_rand = || -> u32 {
        lfsr = (lfsr >> 1) ^ (-((lfsr & 1) as i32) as u32 & 0xB400);
        lfsr
    };

    const ITERATIONS: usize = 20_000;
    for _ in 0..ITERATIONS {
        let size = ((next_rand() as usize) % 1024) + 1; // 1 to 1024 bytes
        let mut data = vec![0u8; size + 4]; // payload + crc32

        // Fill payload
        for b in &mut data[..size] {
            *b = (next_rand() & 0xFF) as u8;
        }

        // Compute CRC32
        let mut hasher = Hasher::new();
        hasher.update(&data[..size]);
        let crc = hasher.finalize();
        data[size..size + 4].copy_from_slice(&crc.to_le_bytes());

        // Write
        loop {
            if let Some(mut grant) = tx.grant(data.len()) {
                if grant.len() >= data.len() {
                    grant[..data.len()].copy_from_slice(&data);
                    grant.commit(data.len());
                    break;
                }
            }
            // Drain some if full
            let drain_grant = rx.grant().expect("must drain");
            drain_grant.commit_all();
        }

        // Read and verify
        let read_grant = rx.grant().expect("must have data");
        assert!(read_grant.len() >= data.len());
        let read_slice = &read_grant[..data.len()];

        let mut read_hasher = Hasher::new();
        read_hasher.update(&read_slice[..size]);
        let actual_crc = read_hasher.finalize();
        let expected_crc = u32::from_le_bytes(read_slice[size..size + 4].try_into().unwrap());
        assert_eq!(
            actual_crc, expected_crc,
            "CRC32 parity violation across ring boundary!"
        );

        read_grant.commit(data.len());
    }
}

#[test]
fn test_panic_rollback_under_concurrent_pressure() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");

    // Verify panic inside producer grant does NOT corrupt ring state or advance head
    for i in 0..100 {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let mut grant = tx.grant(512).unwrap();
            grant[0..4].copy_from_slice(&(i as u32).to_le_bytes());
            if i % 2 == 0 {
                panic!("Simulated worker panic inside grant!");
            }
            grant.commit(512);
        }));
    }

    // Exactly 50 odd packets (50 * 512 = 25600 bytes) must have survived cleanly
    assert_eq!(rx.available_read(), 50 * 512);
    let mut total_read = 0;
    while total_read < 50 * 512 {
        let grant = rx.grant().unwrap();
        let chunk = grant.len().min(512);
        grant.commit(chunk);
        total_read += chunk;
    }
    assert_eq!(rx.available_read(), 0);
}

#[test]
fn test_asymmetric_burst_traffic() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");
    let done = Arc::new(AtomicBool::new(false));

    let done_tx = Arc::clone(&done);
    let producer = thread::spawn(move || {
        // Burst 20 packets of 1 KB, then sleep 1 ms
        for burst in 0..200 {
            for p in 0..20 {
                let val = ((burst * 20 + p) as u8) ^ 0xA5;
                let payload = [val; 1024];
                loop {
                    if let Some(mut grant) = tx.grant(1024) {
                        grant[..1024].copy_from_slice(&payload);
                        grant.commit(1024);
                        break;
                    }
                    thread::yield_now();
                }
            }
            if burst % 10 == 0 {
                thread::sleep(Duration::from_millis(1));
            }
        }
        done_tx.store(true, Ordering::Release);
    });

    let consumer = thread::spawn(move || {
        let mut total_received = 0;
        let mut expected_idx = 0;

        while !done.load(Ordering::Acquire) || rx.available_read() > 0 {
            if let Some(grant) = rx.grant() {
                let mut offset = 0;
                while offset + 1024 <= grant.len() {
                    let chunk = &grant[offset..offset + 1024];
                    let expected_val = (expected_idx as u8) ^ 0xA5;
                    assert_eq!(chunk[0], expected_val);
                    assert_eq!(chunk[1023], expected_val);

                    expected_idx += 1;
                    total_received += 1;
                    offset += 1024;
                }
                grant.commit(offset);
            } else {
                thread::yield_now();
            }
        }
        assert_eq!(total_received, 4000);
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}
