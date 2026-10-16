// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use cyber_hemera::WIDTH;
use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::permute;

fn bench_permutation(c: &mut Criterion) {
    let mut state = [Goldilocks::ZERO; WIDTH];

    c.bench_function("permute", |b| {
        b.iter(|| {
            permute(black_box(&mut state));
        });
    });
}

criterion_group!(benches, bench_permutation);
criterion_main!(benches);
