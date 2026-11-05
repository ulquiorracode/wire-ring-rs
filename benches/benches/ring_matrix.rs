use bbqueue::BBBuffer;
use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use wire_ring::Ring;

const BENCH_PAYLOAD_SIZES: [usize; 4] = [64, 512, 4096, 65536]; // 64 B (tick), 512 B (snapshot), 4 KB (page), 64 KB (state)

fn bench_ring_matrix_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_throughput_matrix");

    for &size in &BENCH_PAYLOAD_SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        // 1. wire-ring (Virtual Mirrored MMU) - 64 KiB or 128 KiB
        let ring_cap = (size * 2).max(64 * 1024).next_power_of_two();
        group.bench_function(BenchmarkId::new("wire-ring-mirrored", size), |b| {
            let (mut tx, mut rx) = Ring::network_default(ring_cap).expect("mirrored ring");
            let payload = vec![0x5Au8; size];

            b.iter(|| {
                let mut w_grant = tx.grant(size).unwrap();
                w_grant[..size].copy_from_slice(black_box(&payload));
                w_grant.commit(size);

                let r_grant = rx.grant().unwrap();
                black_box(&r_grant[..size]);
                r_grant.commit(size);
            });
        });

        // 2. wire-ring (Heap Linear Fallback)
        group.bench_function(BenchmarkId::new("wire-ring-heap", size), |b| {
            let (mut tx, mut rx) = Ring::heap(ring_cap).expect("heap ring");
            let payload = vec![0x5Au8; size];

            b.iter(|| {
                let mut w_grant = tx.grant(size).unwrap();
                let len = w_grant.len().min(size);
                w_grant[..len].copy_from_slice(black_box(&payload[..len]));
                w_grant.commit(len);

                let r_grant = rx.grant().unwrap();
                let r_len = r_grant.len().min(len);
                black_box(&r_grant[..r_len]);
                r_grant.commit(r_len);
            });
        });

        // 3. rtrb (Standard SPSC slice-by-slice)
        group.bench_function(BenchmarkId::new("rtrb", size), |b| {
            let (mut tx, mut rx) = rtrb::RingBuffer::<u8>::new(ring_cap);
            let payload = vec![0x5Au8; size];

            b.iter(|| {
                let mut chunks = tx.write_chunk(size).unwrap();
                let (first, second) = chunks.as_mut_slices();
                let first_len = first.len().min(size);
                first[..first_len].copy_from_slice(&payload[..first_len]);
                if first_len < size {
                    let rem = size - first_len;
                    second[..rem].copy_from_slice(&payload[first_len..]);
                }
                chunks.commit_all();

                let chunks_r = rx.read_chunk(size).unwrap();
                let (r1, r2) = chunks_r.as_slices();
                black_box(r1);
                black_box(r2);
                chunks_r.commit_all();
            });
        });

        // 4. bbqueue (Bipartite Buffer)
        // BBBuffer capacity is statically allocated or const generic
        group.bench_function(BenchmarkId::new("bbqueue", size), |b| {
            // Allocate BBBuffer on heap
            let bb: Box<BBBuffer<{ 128 * 1024 }>> = Box::new(BBBuffer::new());
            let (mut prod, mut cons) = bb.try_split().unwrap();
            let payload = vec![0x5Au8; size];

            b.iter(|| {
                let mut w_grant = prod.grant_exact(size).unwrap();
                w_grant.buf().copy_from_slice(black_box(&payload));
                w_grant.commit(size);

                let r_grant = cons.read().unwrap();
                black_box(&r_grant.buf()[..size]);
                r_grant.release(size);
            });
        });

        // 5. bytes::BytesMut (Standard Tokio / Async Network Pattern)
        group.bench_function(BenchmarkId::new("bytes-mut", size), |b| {
            let mut buf = bytes::BytesMut::with_capacity(ring_cap);
            let payload = vec![0x5Au8; size];

            b.iter(|| {
                buf.extend_from_slice(black_box(&payload));
                let read_view = buf.split_to(size);
                black_box(&read_view[..]);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_ring_matrix_throughput);
criterion_main!(benches);
