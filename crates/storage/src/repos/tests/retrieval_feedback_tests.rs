use crate::pool::StoragePool;
use crate::repos::retrieval_feedback::RetrievalFeedbackRepo;

async fn setup() -> (RetrievalFeedbackRepo, sqlx::SqlitePool) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();
    // Create semantic_facts table inline (minimal schema for testing)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS semantic_facts (
            id TEXT PRIMARY KEY,
            domain TEXT NOT NULL,
            subject TEXT NOT NULL,
            predicate TEXT NOT NULL,
            object TEXT NOT NULL,
            confidence REAL NOT NULL,
            source TEXT NOT NULL,
            valid_from TEXT NOT NULL,
            valid_until TEXT,
            recorded_at TEXT NOT NULL,
            superseded_at TEXT,
            superseded_by TEXT,
            stability REAL NOT NULL,
            last_accessed TEXT,
            access_count INTEGER NOT NULL,
            convergence_score REAL NOT NULL,
            project_id TEXT,
            memory_type TEXT NOT NULL,
            scope_type TEXT NOT NULL,
            scope_id TEXT
        )",
    )
    .execute(&inner)
    .await
    .expect("create semantic_facts table");
    (RetrievalFeedbackRepo::new(inner.clone()), inner)
}

/// Insert two rows with known INTEGER created_at values and verify that
/// count_since filters correctly — proving the INTEGER comparison works.
#[tokio::test]
async fn count_since_filters_by_integer_created_at() {
    let (repo, pool) = setup().await;

    let now_ms = jiff::Timestamp::now().as_millisecond();
    let old_ms = now_ms - 10_000; // 10 seconds ago
    let recent_ms = now_ms - 1_000; // 1 second ago

    // Insert one "old" row directly with a known created_at.
    sqlx::query(
        "INSERT INTO retrieval_feedback
         (id, retrieved_fact_ids, referenced_fact_ids, precision, session_key, created_at)
         VALUES ('row-old', '[]', '[]', 0.5, 'sess', ?1)",
    )
    .bind(old_ms)
    .execute(&pool)
    .await
    .unwrap();

    // Insert one "recent" row directly.
    sqlx::query(
        "INSERT INTO retrieval_feedback
         (id, retrieved_fact_ids, referenced_fact_ids, precision, session_key, created_at)
         VALUES ('row-recent', '[]', '[]', 1.0, 'sess', ?1)",
    )
    .bind(recent_ms)
    .execute(&pool)
    .await
    .unwrap();

    // count_since(0) should count rows from within the past 0 days — i.e. cutoff
    // is ~now, so neither row qualifies.  Use a large window to get both.
    // count_since uses: cutoff = now - days*24h.  With days=1 cutoff is ~24h ago,
    // both rows are within that window.
    let both = repo.count_since(1).await.unwrap();
    assert_eq!(both, 2, "both rows should be within the past 1 day");

    // Now manually query with a cutoff between old_ms and recent_ms to confirm
    // the INTEGER comparison really filters correctly.
    let cutoff = now_ms - 5_000; // 5 s ago — excludes old_ms, includes recent_ms
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM retrieval_feedback WHERE created_at >= ?1")
            .bind(cutoff)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row.0, 1,
        "only the recent row should survive the INTEGER cutoff filter"
    );
}

/// Regression: before the fix, created_at was TEXT and the i64 bind always
/// evaluated to false (SQLite type-coercion bug).  avg_precision_since must
/// return the correct average, not 0.0 (the COALESCE fallback).
#[tokio::test]
async fn avg_precision_since_returns_nonzero_for_recent_rows() {
    let (repo, _pool) = setup().await;

    // Insert via the public API (which now binds created_at as i64).
    repo.insert(
        &["fact-1".to_string()],
        &["fact-1".to_string()],
        "sess-avg",
        None,
    )
    .await
    .unwrap();

    let avg = repo.avg_precision_since(1).await.unwrap();
    assert!(
        avg > 0.0,
        "avg_precision should be > 0.0, got {avg}; was 0.0 before fix (TEXT vs INTEGER bug)"
    );
}

#[tokio::test]
async fn avg_precision_by_domain_returns_typed_recall_domain() {
    let (repo, pool) = setup().await;

    // Insert semantic facts with a known domain.
    for i in 0..3 {
        sqlx::query(
            "INSERT INTO semantic_facts (id, domain, subject, predicate, object, confidence, source, valid_from, recorded_at, stability, access_count, convergence_score, memory_type, scope_type)
             VALUES (?1, 'tasks', 'user', 'has', 'deadline', 0.8, 'test', '2026-01-01', '2026-01-01', 1.0, 0, 0.0, 'fact', 'system')",
        )
        .bind(format!("fact-{}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    // Insert retrieval feedback referencing those facts (need 3+ rows for HAVING clause).
    for i in 0..3 {
        repo.insert(
            &[format!("fact-{}", i)],
            &[format!("fact-{}", i)],
            "sess-domain",
            None,
        )
        .await
        .unwrap();
    }

    let rows: Vec<(ai_core::RecallDomain, f64)> =
        repo.avg_precision_by_domain_since(1).await.unwrap();
    assert!(rows.iter().any(|(d, _)| *d == ai_core::RecallDomain::Tasks));
}
