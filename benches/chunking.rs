use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn generate_rust_source(n_functions: usize) -> String {
    let mut src = String::from("use std::collections::HashMap;\n\n");
    for i in 0..n_functions {
        src.push_str(&format!(
            "pub fn function_{i}(x: i32, y: i32) -> i32 {{\n\
             \x20   let result = x + y + {i};\n\
             \x20   if result > 0 {{\n\
             \x20       result * 2\n\
             \x20   }} else {{\n\
             \x20       result\n\
             \x20   }}\n\
             }}\n\n"
        ));
    }
    src
}

fn generate_python_source(n_functions: usize) -> String {
    let mut src = String::from("import os\nimport sys\n\n");
    for i in 0..n_functions {
        src.push_str(&format!(
            "def function_{i}(x, y):\n\
             \x20   result = x + y + {i}\n\
             \x20   if result > 0:\n\
             \x20       return result * 2\n\
             \x20   return result\n\n"
        ));
    }
    src
}

fn bench_chunk_rust(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_rust");
    for n in [10, 50, 100, 500] {
        let src = generate_rust_source(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
            b.iter(|| prx::chunking::chunk_file(src, "bench.rs", Some("rs")))
        });
    }
    group.finish();
}

fn bench_chunk_python(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_python");
    for n in [10, 50, 100, 500] {
        let src = generate_python_source(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
            b.iter(|| prx::chunking::chunk_file(src, "bench.py", Some("py")))
        });
    }
    group.finish();
}

fn bench_chunk_plaintext(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_plaintext");
    for n in [100, 1000, 5000] {
        let src: String = (0..n)
            .map(|i| format!("line {i}: some content here\n"))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
            b.iter(|| prx::chunking::chunk_file(src, "bench.txt", Some("txt")))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_chunk_rust,
    bench_chunk_python,
    bench_chunk_plaintext
);
criterion_main!(benches);
