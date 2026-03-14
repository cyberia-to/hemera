use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn bench_tree(c: &mut Criterion) {
    for size in [4096, 16384, 65536, 262144] {
        let data = vec![0x42u8; size];
        let mut group = c.benchmark_group(format!("tree/{size}B"));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function("root_hash", |b| {
            b.iter(|| cyber_hemera::tree::root_hash(black_box(&data)));
        });
        group.finish();
    }
}

criterion_group!(benches, bench_tree);
criterion_main!(benches);
