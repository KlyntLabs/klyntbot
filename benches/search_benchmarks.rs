//! Sprint 5 performance benchmarks — Semantic Search
//!
//! Benchmarks for embedding generation and semantic/hybrid search latency.
//! Uses Criterion framework, consistent with existing core_benchmarks.rs.
//!
//! Run with: `cargo bench --bench search_benchmarks`
//!
//! ## Targets (from BA spec BR-6)
//! - Semantic search on 1000 todos: < 500ms
//! - Single todo embedding: < 50ms
//! - Batch embedding 1000 todos: < 30 seconds
//! - Hybrid search on 1000 todos: < 600ms (allows RRF overhead)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
// use std::collections::HashMap;
// use tempfile::TempDir;

// TODO: Uncomment once embedding modules exist
// use tools::embedding_engine::{EmbeddingEngine, EMBEDDING_DIM};
// use tools::embedding_store::EmbeddingStore;
// use tools::todo_store::TodoStore;
// use tools::todo_types::Todo;

// ─── Helpers ──────────────────────────────────────────────────

// TODO: Uncomment once modules exist
//
// fn create_test_todo(title: &str) -> Todo {
//     let mut todo = Todo::default_instance();
//     todo.title = title.to_string();
//     todo
// }
//
// /// Create a store with N todos and pre-generated embeddings.
// /// Uses the real embedding engine for realistic benchmarks.
// async fn create_store_with_n_embedded_todos(n: usize) -> (TodoStore, EmbeddingStore, TempDir) {
//     let temp_dir = TempDir::new().unwrap();
//     let todo_path = temp_dir.path().join("todos.jsonl");
//     let emb_path = temp_dir.path().join("embeddings.jsonl");
//
//     let mut todo_store = TodoStore::new(todo_path);
//     let mut emb_store = EmbeddingStore::new(emb_path);
//     let engine = EmbeddingEngine::new();
//
//     let titles = vec![
//         "Fix authentication bug in login form",
//         "Refactor database connection pooling",
//         "Add OAuth2 support for third-party login",
//         "Update password hashing algorithm",
//         "Security audit for API endpoints",
//         "Implement dark mode toggle",
//         "Optimize search query performance",
//         "Write API documentation for v2",
//         "Fix memory leak in session handler",
//         "Add rate limiting middleware",
//     ];
//
//     for i in 0..n {
//         let title = format!("{} — task #{}", titles[i % titles.len()], i);
//         let todo = create_test_todo(&title);
//         let id = todo.id.clone();
//         todo_store.add(todo).await.unwrap();
//
//         // Generate real embedding
//         let embedding = engine.embed(&title).unwrap();
//         emb_store.upsert(EmbeddingRecord {
//             id,
//             embedding,
//             model: "bench-model".to_string(),
//             embedded_at: chrono::Utc::now(),
//         }).await.unwrap();
//     }
//
//     (todo_store, emb_store, temp_dir)
// }

// ═══════════════════════════════════════════════════════════════
// Embedding Engine Benchmarks
// ═══════════════════════════════════════════════════════════════

fn bench_embed_single_todo(c: &mut Criterion) {
    // TODO: Implement once EmbeddingEngine exists
    //
    // Benchmark: Generate embedding for a single todo title
    // Target: < 50ms per task
    //
    // let engine = EmbeddingEngine::new();
    // // Warm up the model (first call downloads/loads)
    // engine.embed("warmup").unwrap();
    //
    // c.bench_function("embed_single_todo", |b| {
    //     b.iter(|| {
    //         engine.embed(black_box("Fix authentication bug in the login form")).unwrap();
    //     });
    // });
    let _ = (c, black_box(0)); // Placeholder to suppress unused warnings
}

fn bench_embed_batch_32(c: &mut Criterion) {
    // TODO: Implement once EmbeddingEngine exists
    //
    // Benchmark: Batch embed 32 texts (typical backfill batch size)
    //
    // let engine = EmbeddingEngine::new();
    // engine.embed("warmup").unwrap();
    //
    // let texts: Vec<&str> = (0..32)
    //     .map(|i| match i % 5 {
    //         0 => "Fix authentication bug",
    //         1 => "Update database schema",
    //         2 => "Add rate limiting",
    //         3 => "Write API documentation",
    //         _ => "Refactor test infrastructure",
    //     })
    //     .collect();
    //
    // c.bench_function("embed_batch_32", |b| {
    //     b.iter(|| {
    //         engine.embed_batch(black_box(&texts)).unwrap();
    //     });
    // });
    let _ = (c, black_box(0));
}

// ═══════════════════════════════════════════════════════════════
// Search Benchmarks
// ═══════════════════════════════════════════════════════════════

fn bench_semantic_search_1000_todos(c: &mut Criterion) {
    // TODO: Implement once search_semantic is available
    //
    // Benchmark: Semantic search over 1000 todos with pre-generated embeddings
    // Target: < 500ms (from BA spec BR-6)
    //
    // let rt = tokio::runtime::Runtime::new().unwrap();
    // let (todo_store, emb_store, _temp_dir) = rt.block_on(async {
    //     create_store_with_n_embedded_todos(1000).await
    // });
    // let engine = EmbeddingEngine::new();
    //
    // c.bench_function("semantic_search_1000_todos", |b| {
    //     b.iter(|| {
    //         rt.block_on(async {
    //             let query_vec = engine.embed(black_box("authentication security")).unwrap();
    //             let all_embeddings = emb_store.get_all().await.unwrap();
    //
    //             let mut scores: Vec<(String, f64)> = all_embeddings
    //                 .iter()
    //                 .map(|(id, record)| {
    //                     let sim = EmbeddingEngine::cosine_similarity(&query_vec, &record.embedding);
    //                     (id.clone(), sim)
    //                 })
    //                 .filter(|(_, sim)| *sim >= 0.5)
    //                 .collect();
    //
    //             scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    //             scores.truncate(10);
    //             black_box(scores);
    //         });
    //     });
    // });
    let _ = (c, black_box(0));
}

fn bench_hybrid_search_1000_todos(c: &mut Criterion) {
    // TODO: Implement once search_hybrid is available
    //
    // Benchmark: Hybrid search (keyword + semantic + RRF merge) over 1000 todos
    // Target: < 600ms (allows for RRF overhead)
    //
    // let rt = tokio::runtime::Runtime::new().unwrap();
    // ...similar setup as semantic search...
    //
    // c.bench_function("hybrid_search_1000_todos", |b| {
    //     b.iter(|| {
    //         rt.block_on(async {
    //             // 1. Keyword search
    //             // 2. Semantic search
    //             // 3. RRF merge
    //             black_box(());
    //         });
    //     });
    // });
    let _ = (c, black_box(0));
}

fn bench_cosine_similarity(c: &mut Criterion) {
    // TODO: Implement once EmbeddingEngine::cosine_similarity exists
    //
    // Benchmark: Pure cosine similarity computation (384 dims)
    // This is the inner loop of search — should be microseconds
    //
    // let a: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0).sin()).collect();
    // let b: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0).cos()).collect();
    //
    // c.bench_function("cosine_similarity_384d", |b_iter| {
    //     b_iter.iter(|| {
    //         EmbeddingEngine::cosine_similarity(black_box(&a), black_box(&b));
    //     });
    // });
    let _ = (c, black_box(0));
}

fn bench_cosine_similarity_1000_comparisons(c: &mut Criterion) {
    // TODO: Implement once EmbeddingEngine::cosine_similarity exists
    //
    // Benchmark: 1000 cosine similarity computations (simulates searching 1000 todos)
    // This validates the < 500ms target is achievable from pure computation
    //
    // let query: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0).sin()).collect();
    // let embeddings: Vec<Vec<f32>> = (0..1000)
    //     .map(|j| (0..384).map(|i| ((i + j) as f32 / 384.0).cos()).collect())
    //     .collect();
    //
    // c.bench_function("cosine_similarity_1000x", |b| {
    //     b.iter(|| {
    //         let scores: Vec<f64> = embeddings
    //             .iter()
    //             .map(|emb| EmbeddingEngine::cosine_similarity(black_box(&query), emb))
    //             .collect();
    //         black_box(scores);
    //     });
    // });
    let _ = (c, black_box(0));
}

// ═══════════════════════════════════════════════════════════════
// Store I/O Benchmarks
// ═══════════════════════════════════════════════════════════════

fn bench_embedding_store_load_1000(c: &mut Criterion) {
    // TODO: Implement once EmbeddingStore exists
    //
    // Benchmark: Load 1000 embedding records from JSONL
    // Validates that store loading doesn't bottleneck search
    //
    // let rt = tokio::runtime::Runtime::new().unwrap();
    //
    // c.bench_function("embedding_store_load_1000", |b| {
    //     b.iter_batched(
    //         || {
    //             // Setup: create store file with 1000 entries
    //             let (_, store, temp) = rt.block_on(create_store_with_n_embedded_todos(1000));
    //             (store.file_path().to_path_buf(), temp)
    //         },
    //         |(path, _temp)| {
    //             rt.block_on(async {
    //                 let mut store = EmbeddingStore::new(path);
    //                 store.load().await.unwrap();
    //                 black_box(store.get_all().await.unwrap().len());
    //             });
    //         },
    //         BatchSize::SmallInput,
    //     );
    // });
    let _ = (c, black_box(0));
}

// ═══════════════════════════════════════════════════════════════
// Criterion Groups
// ═══════════════════════════════════════════════════════════════

criterion_group!(
    embedding_benches,
    bench_embed_single_todo,
    bench_embed_batch_32,
);

criterion_group!(
    search_benches,
    bench_semantic_search_1000_todos,
    bench_hybrid_search_1000_todos,
    bench_cosine_similarity,
    bench_cosine_similarity_1000_comparisons,
);

criterion_group!(store_benches, bench_embedding_store_load_1000,);

criterion_main!(embedding_benches, search_benches, store_benches);
