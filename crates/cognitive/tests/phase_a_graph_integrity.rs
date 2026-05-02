//! KCA Phase A integration test — verifies Tracks 1, 2, 3, 9-typing as a system.

use cognitive::repos::entity::{EntityRepo, NewEntity};
use cognitive::repos::semantic_fact::SemanticFactRepo;
use cognitive::services::background::run_post_consolidation_linker;
use cognitive::services::graph_linker::GraphLinkHandler;
use cognitive::services::graph_linker_types::{
    DiscoveredRelationship, GraphLinkInput, GraphLinkOutput, SupersedeOp,
};
use storage::StoragePool;

struct ScriptedLinker(GraphLinkOutput);

#[async_trait::async_trait]
impl GraphLinkHandler for ScriptedLinker {
    async fn link(&self, _input: GraphLinkInput) -> common::Result<GraphLinkOutput> {
        Ok(self.0.clone())
    }
}

fn make_fact(subject: &str, predicate: &str, object: &str) -> cognitive::types::SemanticFact {
    let now = jiff::Timestamp::now().to_string();
    cognitive::types::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "test".into(),
        subject: subject.into(),
        predicate: predicate.into(),
        object: object.into(),
        confidence: 0.8,
        source: "t".into(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now,
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        memory_type: "fact".into(),
        project_id: None,
        scope_type: String::new(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
        speaker: None,
    }
}

#[tokio::test]
async fn phase_a_chat_turn_produces_typed_edges() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();

    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let entity_repo = EntityRepo::new(pool.inner().clone());

    // Seed: Alice—knows—Bob
    let alice = entity_repo
        .upsert_entity(&NewEntity {
            name: "Alice".into(),
            entity_type: "person".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let bob = entity_repo
        .upsert_entity(&NewEntity {
            name: "Bob".into(),
            entity_type: "person".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    entity_repo
        .upsert_relationship_typed(&alice.id, &bob.id, "knows", "correlational", 0.8, None, "t")
        .await
        .unwrap();

    // New fact: Alice prefers Rust
    let new_fact = make_fact("Alice", "prefers", "Rust");
    fact_repo.upsert(&new_fact).await.unwrap();

    // Linker scripted to discover causal edge: Alice collaborates_with Bob
    let linker = std::sync::Arc::new(ScriptedLinker(GraphLinkOutput {
        merges: vec![],
        discovered_relationships: vec![DiscoveredRelationship {
            source_entity_name: "Alice".into(),
            target_entity_name: "Bob".into(),
            relationship_type: "collaborates_with".into(),
            edge_type: "causal".into(),
            strength: 0.7,
            evidence: "stated in turn".into(),
        }],
        superseded: vec![],
    }));

    run_post_consolidation_linker(
        &fact_repo,
        &entity_repo,
        linker as std::sync::Arc<dyn GraphLinkHandler>,
        vec![(
            new_fact.clone(),
            cognitive::types::MemoryOp::Add {
                id: new_fact.id.clone(),
            },
        )],
        Some("Alice and Bob will pair on the Rust migration this week".into()),
    )
    .await;

    // Verify: typed edge persisted
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT edge_type FROM entity_relationships WHERE source_entity_id = ?1 AND target_entity_id = ?2 AND relationship_type = 'collaborates_with'",
    )
    .bind(&alice.id)
    .bind(&bob.id)
    .fetch_all(pool.inner())
    .await
    .unwrap();
    assert!(
        rows.iter().any(|r| r.0 == "causal"),
        "expected causal edge_type, got: {:?}",
        rows
    );
}

#[tokio::test]
async fn phase_a_supersede_marks_old_fact_invalid() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();

    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let old_fact = make_fact("Alice", "works_at", "Google");
    fact_repo.upsert(&old_fact).await.unwrap();

    let new_fact = make_fact("Alice", "works_at", "Anthropic");
    fact_repo.upsert(&new_fact).await.unwrap();

    let linker = std::sync::Arc::new(ScriptedLinker(GraphLinkOutput {
        merges: vec![],
        discovered_relationships: vec![],
        superseded: vec![SupersedeOp {
            old_fact_id: old_fact.id.clone(),
            valid_until: "2026-04-29T00:00:00Z".into(),
            reason: "Alice changed jobs".into(),
        }],
    }));

    run_post_consolidation_linker(
        &fact_repo,
        &entity_repo,
        linker as std::sync::Arc<dyn GraphLinkHandler>,
        vec![(
            new_fact.clone(),
            cognitive::types::MemoryOp::Add {
                id: new_fact.id.clone(),
            },
        )],
        None,
    )
    .await;

    let old = fact_repo
        .get(&old_fact.id)
        .await
        .unwrap()
        .expect("old fact exists");
    assert!(old.superseded_at.is_some(), "old fact must be superseded");
    assert_eq!(old.superseded_by.as_deref(), Some(new_fact.id.as_str()));
}

#[tokio::test]
async fn phase_a_graph_boosts_weight_causal_edges_higher() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();

    let entity_repo = EntityRepo::new(pool.inner().clone());

    // Entities: DeadlinePressure --causes--> Stress --correlates--> Coffee
    // Capitalized so extract_query_entities picks them up.
    let deadline = entity_repo
        .upsert_entity(&NewEntity {
            name: "DeadlinePressure".into(),
            entity_type: "concept".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let stress = entity_repo
        .upsert_entity(&NewEntity {
            name: "Stress".into(),
            entity_type: "concept".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let coffee = entity_repo
        .upsert_entity(&NewEntity {
            name: "Coffee".into(),
            entity_type: "concept".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();

    entity_repo
        .upsert_relationship_typed(&deadline.id, &stress.id, "causes", "causal", 0.9, None, "t")
        .await
        .unwrap();
    entity_repo
        .upsert_relationship_typed(
            &stress.id,
            &coffee.id,
            "correlates_with",
            "correlational",
            0.6,
            None,
            "t",
        )
        .await
        .unwrap();

    // Query mentions DeadlinePressure; facts mentioning Stress (causal=1.5) should score higher
    // than facts mentioning Coffee (correlational=1.0).
    let contents: &[(&str, &str)] = &[
        ("f1", "Stress increases when deadlines approach"),
        ("f2", "Coffee consumption is high in the office"),
    ];

    let boosts = cognitive::services::graph_retrieval::compute_graph_boosts(
        &entity_repo,
        "my DeadlinePressure is looming",
        contents,
    )
    .await;

    let stress_boost = boosts.get("f1").copied().unwrap_or(0.0);
    let coffee_boost = boosts.get("f2").copied().unwrap_or(0.0);

    assert!(
        stress_boost > coffee_boost,
        "causal edge should weight higher: stress_boost={stress_boost}, coffee_boost={coffee_boost}"
    );
}
