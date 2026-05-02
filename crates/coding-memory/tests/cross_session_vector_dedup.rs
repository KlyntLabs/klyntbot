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

#[tokio::test]
async fn exact_match_dedup_works_without_embedder() {
    let pool = common::pool_with_migrations().await;
    let repo = SemanticFactRepo::new(pool.inner().clone());

    let f1 = make_fact("f1", "user", "prefers", "rust");
    let f2 = make_fact("f2", "user", "prefers", "rust");
    repo.upsert(&f1).await.unwrap();
    repo.upsert(&f2).await.unwrap();

    let applied = CrossSessionDedup::run(&repo, 0.92, None).await.unwrap();
    assert_eq!(applied, 1);
}

#[tokio::test]
async fn vector_similarity_dedup_with_embedder() {
    let pool = common::pool_with_migrations().await;
    let repo = SemanticFactRepo::new(pool.inner().clone());

    let f1 = make_fact("f1", "user", "prefers", "rust language");
    let f2 = make_fact("f2", "user", "prefers", "rust language");
    repo.upsert(&f1).await.unwrap();
    repo.upsert(&f2).await.unwrap();

    let embedder = MockEmbedder;
    let applied = CrossSessionDedup::run(&repo, 0.92, Some(&embedder))
        .await
        .unwrap();
    assert_eq!(applied, 1);
}
