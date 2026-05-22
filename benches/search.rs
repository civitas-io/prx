use criterion::{Criterion, criterion_group, criterion_main};

// TODO: implement real search benchmarks (NDCG@10, latency, token efficiency)
// See docs/design/BENCHMARKS.md for the plan
fn bench_search_placeholder(c: &mut Criterion) {
    c.bench_function("search_placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_search_placeholder);
criterion_main!(benches);
