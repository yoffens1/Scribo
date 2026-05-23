use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use scribo_lib::chunker::{chunk_for_embedding, ChunkOptions};

fn bench_chunker(c: &mut Criterion) {
    let small = include_str!("../test_data/small.md");   // ~5KB
    let medium = include_str!("../test_data/medium.md"); // ~100KB
    let large = include_str!("../test_data/large.md");   // ~1MB
    
    let opts = ChunkOptions::default();
    
    let mut group = c.benchmark_group("chunker");
    for (name, text) in [("small", small), ("medium", medium), ("large", large)] {
        group.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
            b.iter(|| chunk_for_embedding(black_box(t), black_box(&opts)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_chunker);
criterion_main!(benches);
