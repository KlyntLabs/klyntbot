//! Cross-session dedup must merge facts whose embedding similarity > 0.92.

use coding_memory::reforge::cross_session_dedup::CrossSessionDedup;
use cognitive::{SemanticFact, SemanticFactRepo};
use jiff::Timestamp;

mod common;

struct MockEmbedder;

#[async_trait::async_trait]
impl cognitive::TextEmbedder for MockEmbedder {
    async fn embed(&self, text: &str) -> ::common::Result<Vec<f32>> {
        let hash = blake3::hash(text.as_bytes());
        let bytes = hash.as_bytes();
        Ok(vec![
            bytes[0] as f32 / 255.0,
            bytes[1] as f32 / 255.0,
            bytes[2] as f32 / 255.0,
        ])
    }
}

fn make_fact(id: &str, subject: &str, predicate: &str, object: &str) -> SemanticFact {
    let now = Timestamp::now().to_string();
    SemanticFact {
        id: id.into(),
        domain: "coding".into(),
        subject: subject.into(),
        predicate: predicate.into(),
        object: object.into(),
        confidence: 0.9,
        source: "test".into(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now.clone(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        project_id: None,
        memory_type: "fact".into(),
        scope_type: "user".into(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
        speaker: None,
    }
}

/// Insert a fact via raw SQL, bypassing `SemanticFactRepo::upsert`'s
/// triple-level dedup so cross-session dedup has something to find.
async fn raw_insert(pool: &sqlx::SqlitePool, f: &SemanticFact) {
    sqlx::query(
        "INSERT INTO semantic_facts (id, domain, subject, predicate, object, confidence, source,
            valid_from, valid_until, recorded_at, superseded_at, superseded_by,
            stability, last_accessed, access_count, convergence_score, project_id, memory_type,
            scope_type, scope_id, speaker)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
    )
    .bind(&f.id)
    .bind(&f.domain)
    .bind(&f.subject)
    .bind(&f.predicate)
    .bind(&f.object)
    .bind(f.confidence)
    .bind(&f.source)
    .bind(&f.valid_from)
    .bind(&f.valid_until)
    .bind(&f.recorded_at)
    .bind(&f.superseded_at)
    .bind(&f.superseded_by)
    .bind(f.stability)
    .bind(&f.last_accessed)
    .bind(f.access_count)
    .bind(f.convergence_score)
    .bind(&f.project_id)
    .bind(&f.memory_type)
    .bind(&f.scope_type)
    .bind(&f.scope_id)
    .bind(&f.speaker)
    .execute(pool)
    .await
    .unwrap();
}

/// Two staggered timestamps within the last 7 days (so they pass the
/// vector-dedup cutoff filter) with a clear older/newer ordering.
fn staggered_recent() -> (String, String) {
    let now = Timestamp::now();
    let older = now.checked_sub(jiff::ToSpan::hours(2)).unwrap().to_string();
    let newer = now.checked_sub(jiff::ToSpan::hours(1)).unwrap().to_string();
    (older, newer)
}

#[tokio::test]
async fn exact_match_dedup_works_without_embedder() {
    let pool = common::pool_with_migrations().await;
    let repo = SemanticFactRepo::new(pool.inner().clone());

    let mut f1 = make_fact("f1", "user", "prefers", "rust");
    let mut f2 = make_fact("f2", "user", "prefers", "rust");
    let (older, newer) = staggered_recent();
    f1.valid_from = older;
    f2.valid_from = newer;
    raw_insert(pool.inner(), &f1).await;
    raw_insert(pool.inner(), &f2).await;

    let applied = CrossSessionDedup::run(&repo, 0.92, None).await.unwrap();
    assert_eq!(applied, 1);
}

#[tokio::test]
async fn vector_similarity_dedup_with_embedder() {
    let pool = common::pool_with_migrations().await;
    let repo = SemanticFactRepo::new(pool.inner().clone());

    let mut f1 = make_fact("f1", "user", "prefers", "rust language");
    let mut f2 = make_fact("f2", "user", "prefers", "rust language");
    let (older, newer) = staggered_recent();
    f1.valid_from = older;
    f2.valid_from = newer;
    raw_insert(pool.inner(), &f1).await;
    raw_insert(pool.inner(), &f2).await;

    let embedder = MockEmbedder;
    let applied = CrossSessionDedup::run(&repo, 0.92, Some(&embedder))
        .await
        .unwrap();
    assert_eq!(applied, 1);
}
