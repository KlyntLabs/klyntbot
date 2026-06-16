use std::fs;
use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use feature_launcher::search::inverted_index::{InvertedFileIndex, SkipSet};
use std::hint::black_box;
use tempfile::TempDir;

fn seed_tree(root: &std::path::Path, files: usize) {
    let words = [
        "report", "notes", "todo", "design", "spec", "draft", "image", "video", "config", "main",
        "lib", "test", "bench", "data", "log", "readme",
    ];
    let exts = ["md", "rs", "ts", "json", "txt", "py"];
    for i in 0..files {
        let depth = (i % 5) + 1;
        let mut p: PathBuf = root.to_path_buf();
        for d in 0..depth {
            p.push(format!("dir_{}_{}", d, (i / 16) % 8));
        }
        fs::create_dir_all(&p).unwrap();
        let w = words[i % words.len()];
        let e = exts[i % exts.len()];
        p.push(format!("{}_{}.{}", w, i, e));
        fs::write(&p, b"").unwrap();
    }
}

fn build_bench(c: &mut Criterion) {
    let mut sizes = vec![500usize, 5_000, 50_000];
    if std::env::var("BENCH_LARGE").is_ok() {
        sizes.push(200_000);
    }
    let mut group = c.benchmark_group("inverted_index_build");
    for size in sizes.iter() {
        let dir = TempDir::new().unwrap();
        seed_tree(dir.path(), *size);
        let roots = vec![dir.path().to_path_buf()];
        let skip = SkipSet::defaults();
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let idx = InvertedFileIndex::build(black_box(&roots), black_box(&skip), 50_000);
                black_box(idx.len());
            });
        });
    }
    group.finish();
}

fn search_bench(c: &mut Criterion) {
    let mut sizes = vec![5_000usize, 50_000];
    if std::env::var("BENCH_LARGE").is_ok() {
        sizes.push(200_000);
    }
    for &corpus in &sizes {
        let dir = TempDir::new().unwrap();
        seed_tree(dir.path(), corpus);
        let idx = InvertedFileIndex::build(
            &[dir.path().to_path_buf()],
            &SkipSet::defaults(),
            corpus + 1_000,
        );

        let mut group = c.benchmark_group(format!("inverted_index_search_{corpus}"));
        for query in ["re", "report", "co", "config", "ma"].iter() {
            group.bench_with_input(BenchmarkId::from_parameter(query), query, |b, q| {
                b.iter(|| {
                    let r = idx.search(black_box(q), 20);
                    black_box(r.len());
                });
            });
        }
        group.finish();
    }
}

fn ranking_quality_bench(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let base = dir.path();
    fs::write(base.join("parser_module_test.rs"), b"").unwrap();
    fs::write(base.join("test_parser_module.rs"), b"").unwrap();
    for i in 0..17 {
        fs::write(base.join(format!("test_module_{i}.rs")), b"").unwrap();
    }
    let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);

    c.bench_function("inverted_index_ranking_parser_test", |b| {
        b.iter(|| {
            let r = idx.search(black_box("parser test"), 10);
            assert!(!r.is_empty());
            assert_eq!(
                idx.entry_name(r[0].entry_idx).unwrap(),
                "parser_module_test.rs"
            );
            black_box(r.len());
        });
    });
}

criterion_group!(benches, build_bench, search_bench, ranking_quality_bench);
criterion_main!(benches);
