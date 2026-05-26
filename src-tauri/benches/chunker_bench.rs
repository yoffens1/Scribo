use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use scribo_lib::fragmenter::{fragment_for_embedding, fragment_for_generation, fragment_paired, FragmentOptions};
use std::env;
use std::fs;

fn bench_fragmenter(c: &mut Criterion) {
    let small = include_str!("../test_data/small.md");   // ~5KB
    let medium = include_str!("../test_data/medium.md"); // ~100KB
    let large = include_str!("../test_data/large.md");   // ~1MB
    
    let default_opts = FragmentOptions::default();
    
    // 1. Benchmarking Embedding Mode
    let mut group_emb = c.benchmark_group("fragmenter/embedding");
    for (name, text) in [("small", small), ("medium", medium), ("large", large)] {
        group_emb.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
            b.iter(|| fragment_for_embedding(black_box(t), black_box(&default_opts)));
        });
    }
    group_emb.finish();

    // 2. Benchmarking Generation Mode
    let mut group_gen = c.benchmark_group("fragmenter/generation");
    for (name, text) in [("small", small), ("medium", medium), ("large", large)] {
        group_gen.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
            b.iter(|| fragment_for_generation(black_box(t), black_box(&default_opts)));
        });
    }
    group_gen.finish();

    // 3. Benchmarking Structural Mode (fragment_paired)
    let mut group_struct = c.benchmark_group("fragmenter/paired_parallel");
    for (name, text) in [("small", small), ("medium", medium), ("large", large)] {
        group_struct.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
            b.iter(|| fragment_paired(black_box(t.to_string()), black_box(&default_opts)));
        });
    }
    group_struct.finish();

    // 4. Benchmarking on Real Inbox files (Optional)
    let inbox_path = env::var("SCRIBO_INBOX").unwrap_or_else(|_| "/home/yoffens/obsidian2026/1-INBOX/".to_string());
    if let Ok(entries) = fs::read_dir(&inbox_path) {
        let mut inbox_files = Vec::new();
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            inbox_files.push((path.file_name().unwrap().to_string_lossy().to_string(), content));
                        }
                    }
                }
            }
        }
        
        if !inbox_files.is_empty() {
            let mut group_inbox = c.benchmark_group("fragmenter/inbox_paired");
            // Only benchmark the first few files to avoid excessively long benchmark times
            for (name, text) in inbox_files.iter().take(5) {
                group_inbox.bench_with_input(BenchmarkId::from_parameter(name), text, |b, t| {
                    b.iter(|| fragment_paired(black_box(t.to_string()), black_box(&default_opts)));
                });
            }
            group_inbox.finish();
        }
    }
}

criterion_group!(benches, bench_fragmenter);
criterion_main!(benches);
