use criterion::{Criterion, criterion_group, criterion_main};

// TODO: implement real chunking benchmarks (throughput, chunk quality)
fn bench_chunking_placeholder(c: &mut Criterion) {
    c.bench_function("chunking_placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_chunking_placeholder);
criterion_main!(benches);
