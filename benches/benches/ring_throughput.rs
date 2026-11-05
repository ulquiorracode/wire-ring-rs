use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use wire_ring::Ring;

fn bench_ring_write_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_throughput");
    const CHUNK_SIZE: usize = 512;
    group.throughput(Throughput::Bytes(CHUNK_SIZE as u64));

    group.bench_function(BenchmarkId::new("mirrored_grant_commit", CHUNK_SIZE), |b| {
        let (mut tx, mut rx) = Ring::network_default(64 * 1024).expect("mirrored ring");
        let payload = [0x5Au8; CHUNK_SIZE];

        b.iter(|| {
            // Producer write
            let mut w_grant = tx.grant(CHUNK_SIZE).unwrap();
            w_grant[..CHUNK_SIZE].copy_from_slice(black_box(&payload));
            w_grant.commit(CHUNK_SIZE);

            // Consumer read
            let r_grant = rx.grant().unwrap();
            black_box(&r_grant[..CHUNK_SIZE]);
            r_grant.commit(CHUNK_SIZE);
        });
    });

    group.bench_function(BenchmarkId::new("heap_grant_commit", CHUNK_SIZE), |b| {
        let (mut tx, mut rx) = Ring::heap(64 * 1024).expect("heap ring");
        let payload = [0x5Au8; CHUNK_SIZE];

        b.iter(|| {
            // Producer write
            let mut w_grant = tx.grant(CHUNK_SIZE).unwrap();
            let len = w_grant.len().min(CHUNK_SIZE);
            w_grant[..len].copy_from_slice(black_box(&payload[..len]));
            w_grant.commit(len);

            // Consumer read
            let r_grant = rx.grant().unwrap();
            let r_len = r_grant.len().min(len);
            black_box(&r_grant[..r_len]);
            r_grant.commit(r_len);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_ring_write_read);
criterion_main!(benches);
