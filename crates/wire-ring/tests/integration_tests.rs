use std::thread;
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
    // 64 KiB page-aligned
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("build mirrored ring");
    let cap = tx.capacity();

    // 1. Advance head close to the end of physical boundary: cap - 16 bytes
    let prefill = cap - 16;
    let dummy = vec![0xAAu8; prefill];
    assert_eq!(tx.write_slice(&dummy), prefill);

    // Consume all prefill
    let mut consume_dummy = vec![0u8; prefill];
    assert_eq!(rx.read_slice(&mut consume_dummy), prefill);

    // 2. Head is now at cap - 16.
    // Request a grant of 32 bytes!
    // In a normal ring, grant() would be truncated to 16 bytes (the end of the physical buffer).
    // In our Mirrored ring, grant() MUST return the full 32 bytes as a SINGLE CONTIGUOUS SLICE!
    {
        let mut grant = tx.grant(32).expect("grant across boundary");
        assert_eq!(grant.len(), 32);

        // Write 32 bytes spanning across the wrap-around boundary
        for (i, b) in grant.iter_mut().enumerate() {
            *b = (i + 1) as u8;
        }
        grant.commit(32);
    }

    // 3. Consumer reads the cross-boundary slice
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
fn test_grant_auto_rollback_on_drop() {
    let (mut tx, mut rx) = Ring::heap(512).expect("heap ring");

    // Producer gets a lease but drops it without committing
    {
        let mut grant = tx.grant(128).expect("grant lease");
        grant[0] = 42;
        // Dropped without .commit()
    }

    // Consumer should see 0 readable bytes because grant was rolled back!
    assert_eq!(rx.available_read(), 0);
    assert!(rx.grant().is_none());
}

#[test]
fn test_spsc_threaded_streaming_integrity() {
    let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");

    const TOTAL_MESSAGES: usize = 100_000;
    const MSG_SIZE: usize = 64;

    let producer_handle = thread::spawn(move || {
        let mut msg = [0u8; MSG_SIZE];
        for i in 0..TOTAL_MESSAGES {
            msg[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            msg[8..16].copy_from_slice(&(i as u64 ^ 0xDEADBEEFCAFE).to_le_bytes());

            loop {
                if let Some(mut grant) = tx.grant(MSG_SIZE) {
                    if grant.len() >= MSG_SIZE {
                        grant[..MSG_SIZE].copy_from_slice(&msg);
                        grant.commit(MSG_SIZE);
                        break;
                    }
                }
                thread::yield_now();
            }
        }
    });

    let consumer_handle = thread::spawn(move || {
        let mut received = 0;
        let mut expected_seq = 0u64;

        while received < TOTAL_MESSAGES {
            if let Some(grant) = rx.grant() {
                let mut offset = 0;
                while offset + MSG_SIZE <= grant.len() {
                    let chunk = &grant[offset..offset + MSG_SIZE];
                    let seq = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
                    let hash = u64::from_le_bytes(chunk[8..16].try_into().unwrap());

                    assert_eq!(seq, expected_seq);
                    assert_eq!(hash, expected_seq ^ 0xDEADBEEFCAFE);

                    expected_seq += 1;
                    received += 1;
                    offset += MSG_SIZE;
                }
                grant.commit(offset);
            } else {
                thread::yield_now();
            }
        }
    });

    producer_handle.join().expect("producer thread");
    consumer_handle.join().expect("consumer thread");
}
