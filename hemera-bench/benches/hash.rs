use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn bench_hash(c: &mut Criterion) {
    for size in [0, 64, 256, 1024, 4096, 65536] {
        let data = vec![0x42u8; size];
        let mut group = c.benchmark_group(format!("hash/{size}B"));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function("flat", |b| {
            b.iter(|| cyber_hemera::hash(black_box(&data)));
        });
        group.finish();
    }
}

criterion_group!(benches, bench_hash);
criterion_main!(benches);
