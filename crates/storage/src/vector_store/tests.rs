//! Tests for VectorStore — all concerns.

use std::sync::Arc;

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::Field;
use tempfile::TempDir;

use super::VectorStore;
use crate::vector_store::cognitive::CognitiveFactParams;
use crate::vector_store::crud::{sanitize_predicate_value, validate_predicate};

async fn test_store() -> (VectorStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = VectorStore::connect(dir.path()).await.unwrap();
    (store, dir)
}

#[test]
fn test_sanitize_predicate_value_escapes_quotes() {
    let result = sanitize_predicate_value("it's a test").unwrap();
    assert_eq!(result, "it''s a test");
}

#[test]
fn test_sanitize_predicate_value_rejects_semicolons() {
    assert!(sanitize_predicate_value("foo; DROP TABLE").is_err());
}

#[test]
fn test_sanitize_predicate_value_rejects_comments() {
    assert!(sanitize_predicate_value("foo -- comment").is_err());
    assert!(sanitize_predicate_value("foo /* block */").is_err());
}

#[test]
fn test_sanitize_predicate_value_rejects_newlines() {
    assert!(sanitize_predicate_value("foo\nbar").is_err());
    assert!(sanitize_predicate_value("foo\rbar").is_err());
}

#[test]
fn test_sanitize_predicate_value_allows_clean_values() {
    assert!(sanitize_predicate_value("2024-01-01T00:00:00Z").is_ok());
    assert!(sanitize_predicate_value("session-key-123").is_ok());
    assert!(sanitize_predicate_value("identity").is_ok());
}

// ── validate_predicate ──────────────────────────────────────────

#[test]
fn test_validate_predicate_allows_properly_escaped() {
    // Properly escaped quotes from sanitize_predicate_value
    assert!(validate_predicate("session_key = 'sess-123'").is_ok());
    assert!(validate_predicate("created_at < '2024-01-01T00:00:00Z'").is_ok());
    assert!(validate_predicate("name = 'it''s escaped'").is_ok());
    assert!(validate_predicate("id IS NOT NULL").is_ok());
}

#[test]
fn test_validate_predicate_rejects_semicolons() {
    assert!(validate_predicate("id = '1'; DROP TABLE conv_embeddings").is_err());
}

#[test]
fn test_validate_predicate_rejects_comments() {
    assert!(validate_predicate("id = '1' -- always true").is_err());
    assert!(validate_predicate("id = '1' /* comment */").is_err());
}

#[test]
fn test_validate_predicate_rejects_newlines() {
    assert!(validate_predicate("id = '1'\nOR 1=1").is_err());
}

#[tokio::test]
async fn test_connect_creates_directory() {
    let dir = TempDir::new().unwrap();
    let lance_dir = dir.path().join("lance");
    assert!(!lance_dir.exists());
    VectorStore::connect(dir.path()).await.unwrap();
    assert!(lance_dir.exists());
}

#[tokio::test]
async fn test_upsert_and_search() {
    let (store, _dir) = test_store().await;
    let vec1 = vec![1.0f32; 384];
    store
        .upsert_embedding(
            "todo_embeddings",
            "test-1",
            &vec1,
            &[("model", "paraphrase-multilingual")],
        )
        .await
        .unwrap();

    let count = store.count("todo_embeddings").await.unwrap();
    assert_eq!(count, 1);

    let results = store
        .search_similar("todo_embeddings", &vec1, 5, 0.0)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, "test-1");
}

#[tokio::test]
async fn test_delete_removes_entry() {
    let (store, _dir) = test_store().await;
    let vec1 = vec![0.5f32; 384];
    store
        .upsert_embedding("todo_embeddings", "del-1", &vec1, &[("model", "test")])
        .await
        .unwrap();
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);
    store.delete("todo_embeddings", "del-1").await.unwrap();
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 0);
}

#[tokio::test]
async fn test_upsert_replaces_existing() {
    let (store, _dir) = test_store().await;
    let v = vec![0.1f32; 384];
    store
        .upsert_embedding("todo_embeddings", "upsert-1", &v, &[("model", "m1")])
        .await
        .unwrap();
    store
        .upsert_embedding("todo_embeddings", "upsert-1", &v, &[("model", "m2")])
        .await
        .unwrap();
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);
}

#[tokio::test]
async fn test_upsert_inserts_before_deleting() {
    let (store, _dir) = test_store().await;
    let v1 = vec![0.1f32; 384];
    let v2 = vec![0.9f32; 384];

    // Insert initial
    store
        .upsert_embedding("todo_embeddings", "safe-1", &v1, &[("model", "m1")])
        .await
        .unwrap();
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);

    // Upsert with new vector — should still have exactly 1 row after
    store
        .upsert_embedding("todo_embeddings", "safe-1", &v2, &[("model", "m2")])
        .await
        .unwrap();
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);

    // Verify the new vector is searchable (v2 should match itself better)
    let results = store
        .search_similar("todo_embeddings", &v2, 5, 0.0)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, "safe-1");
}

#[tokio::test]
async fn test_conv_table_created() {
    let (store, _dir) = test_store().await;
    // Should return 0 (empty table exists) rather than error.
    assert_eq!(store.count("conv_embeddings").await.unwrap(), 0);
}

#[tokio::test]
async fn test_cognitive_table_creation() {
    let (store, _dir) = test_store().await;
    // Table should exist after connect — empty search returns no results
    let results = store
        .search_cognitive_facts(&[0.1; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_cognitive_upsert_and_search() {
    let (store, _dir) = test_store().await;

    store
        .upsert_cognitive_fact(CognitiveFactParams {
            fact_id: "fact1",
            vector: &[0.5; 384],
            domain: "identity",
            text: "user name Jayden",
            importance: 0.9,
            stability: 1.0,
            confidence: 0.85,
        })
        .await
        .unwrap();

    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "fact1");
    assert!(results[0].1 > 0.99); // Near-identical vector
}

#[tokio::test]
async fn test_cognitive_domain_filter() {
    let (store, _dir) = test_store().await;

    store
        .upsert_cognitive_fact(CognitiveFactParams {
            fact_id: "f1",
            vector: &[0.5; 384],
            domain: "identity",
            text: "text1",
            importance: 0.9,
            stability: 1.0,
            confidence: 0.8,
        })
        .await
        .unwrap();
    store
        .upsert_cognitive_fact(CognitiveFactParams {
            fact_id: "f2",
            vector: &[0.5; 384],
            domain: "finance",
            text: "text2",
            importance: 0.8,
            stability: 1.0,
            confidence: 0.7,
        })
        .await
        .unwrap();

    // Search only identity domain
    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "f1");
}

#[tokio::test]
async fn test_cognitive_delete() {
    let (store, _dir) = test_store().await;

    store
        .upsert_cognitive_fact(CognitiveFactParams {
            fact_id: "f1",
            vector: &[0.5; 384],
            domain: "identity",
            text: "text",
            importance: 0.9,
            stability: 1.0,
            confidence: 0.8,
        })
        .await
        .unwrap();
    store
        .delete("cognitive_fact_embeddings", "f1")
        .await
        .unwrap();

    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_dedup_removes_duplicates() {
    let (store, _dir) = test_store().await;
    let v = vec![0.3f32; 384];

    // Simulate a crash scenario: manually insert two rows with the same ID
    // by calling add() directly (bypassing the delete step of upsert).
    // Use get_table() first to ensure the table is created lazily.
    let tbl = store.get_table("todo_embeddings").await.unwrap();

    for ts in ["2024-01-01T00:00:00Z", "2024-01-02T00:00:00Z"] {
        let schema = tbl.schema().await.unwrap();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["dup-1"])) as ArrayRef,
                Arc::new(
                    FixedSizeListArray::try_new(
                        Arc::new(Field::new("item", arrow_schema::DataType::Float32, true)),
                        384,
                        Arc::new(Float32Array::from(v.clone())) as ArrayRef,
                        None,
                    )
                    .unwrap(),
                ) as ArrayRef,
                Arc::new(StringArray::from(vec!["test-model"])) as ArrayRef,
                Arc::new(StringArray::from(vec![ts])) as ArrayRef,
            ],
        )
        .unwrap();
        let reader = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);
        tbl.add(Box::new(reader)).execute().await.unwrap();
    }

    assert_eq!(store.count("todo_embeddings").await.unwrap(), 2);

    let deduped = store
        .dedup_table("todo_embeddings", "updated_at")
        .await
        .unwrap();
    assert_eq!(deduped, 1); // 1 ID had duplicates

    assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);
}

#[tokio::test]
async fn test_dedup_no_duplicates_is_noop() {
    let (store, _dir) = test_store().await;
    let v = vec![0.5f32; 384];

    store
        .upsert_embedding("todo_embeddings", "a", &v, &[("model", "m")])
        .await
        .unwrap();
    store
        .upsert_embedding("todo_embeddings", "b", &v, &[("model", "m")])
        .await
        .unwrap();

    assert_eq!(store.count("todo_embeddings").await.unwrap(), 2);

    let deduped = store
        .dedup_table("todo_embeddings", "updated_at")
        .await
        .unwrap();
    assert_eq!(deduped, 0);
    assert_eq!(store.count("todo_embeddings").await.unwrap(), 2);
}
