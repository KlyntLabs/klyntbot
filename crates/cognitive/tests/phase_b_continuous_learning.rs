//! KCA Phase B integration test — verifies Tracks 4 (Micro-Reforge), 5 (Extraction Critic), 11 (Online Community).

use cognitive::repos::*;
use cognitive::services::community_membership_online as cmo;
use cognitive::services::extraction_critic_types;
use cognitive::services::micro_reforge_types;
use cognitive::types::{EpisodicMemory, ProceduralRule, SemanticFact};
use storage::StoragePool;

async fn setup_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let migrations = cognitive::repos::cognitive_migrations();
    StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn phase_b_session_drives_micro_reforge_critic_and_community() {
    let pool = setup_pool().await;
    let db = pool.inner().clone();
    let fact_repo = semantic_fact::SemanticFactRepo::new(db.clone());
    let entity_repo = entity::EntityRepo::new(db.clone());
    let rule_repo = procedural_rule::ProceduralRuleRepo::new(db.clone());
    let ep_repo = episodic_memory::EpisodicMemoryRepo::new(db.clone());
    let obs_repo = accumulated_observation::AccumulatedObservationRepo::new(db.clone());
    let crit_log = extraction_critic_log::ExtractionCriticLogRepo::new(pool.clone());
    let community_repo = community::CommunityRepo::new(db.clone());

    // Seed an existing rule so the dedup path is exercised.
    rule_repo
        .upsert(&ProceduralRule {
            id: "r0".into(),
            domain: "coding".into(),
            rule_text: "When the user says 'lint', run cargo clippy".into(),
            confidence: 0.8,
            signal_count: 10,
            source: "reflected".into(),
            active: true,
            ..Default::default()
        })
        .await
        .unwrap();

    // Insert 10 episodics suggesting a new pattern.
    for i in 0..10 {
        ep_repo
            .insert(&EpisodicMemory {
                id: format!("ep{i}"),
                domain: "coding".into(),
                content: format!("user ran cargo nextest then asked for clippy in session {i}"),
                summary: Some("user runs nextest before clippy".into()),
                importance: 0.75,
                stability: 1.0,
                scope_type: "project".into(),
                tier: "raw".into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Run micro-Reforge with scripted handler that proposes the new pattern.
    let handler = std::sync::Arc::new(ScriptedMicroHandler(
        micro_reforge_types::MicroReforgeOutput {
            proposed_rules: vec![micro_reforge_types::ProposedRule {
                domain: "coding".into(),
                rule_text: "After cargo nextest passes, run clippy.".into(),
                confidence: 0.85,
                signal_count: 10,
                evidence_episodic_ids: (0..10).map(|i| format!("ep{i}")).collect(),
            }],
            notes: None,
        },
    ));

    let cfg = config::schema::MicroReforgeConfig::default();
    let svc = cognitive::services::micro_reforge::MicroReforgeService::new(pool.clone(), cfg);
    for _ in 0..15 {
        svc.note_turn().await.unwrap();
    }
    let accepted = svc
        .run("turn_threshold", handler, &rule_repo, &ep_repo, &obs_repo)
        .await
        .unwrap();

    assert_eq!(accepted, 1);
    let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
    assert_eq!(
        rules.len(),
        2,
        "rules: {:?}",
        rules.iter().map(|r| &r.rule_text).collect::<Vec<_>>()
    );

    // Critic round.
    let f1 = make_fact("user", "uses", "cargo-nextest", 0.85);
    let f2 = make_fact("user", "loves", "Pluto", 0.6); // hallucination
    fact_repo.upsert(&f1).await.unwrap();
    fact_repo.upsert(&f2).await.unwrap();

    #[derive(Debug)]
    struct StrictCritic;
    #[async_trait::async_trait]
    impl cognitive::services::extraction_critic::ExtractionCriticHandler for StrictCritic {
        async fn judge(
            &self,
            input: extraction_critic_types::ExtractionCriticInput,
        ) -> common::Result<extraction_critic_types::ExtractionCriticOutput> {
            Ok(extraction_critic_types::ExtractionCriticOutput {
                verdicts: input
                    .extracted_facts
                    .iter()
                    .map(|f| extraction_critic_types::Verdict {
                        fact_id: f.fact_id.clone(),
                        verdict: if f.object == "Pluto" {
                            "hallucinated".into()
                        } else {
                            "grounded".into()
                        },
                        reason: "test".into(),
                    })
                    .collect(),
                missed_facts: vec![],
            })
        }
    }
    cognitive::services::background::run_critic_judgment(
        "user uses cargo-nextest for testing.",
        vec![f1.clone(), f2.clone()],
        std::sync::Arc::new(StrictCritic),
        &fact_repo,
        &crit_log,
    )
    .await;

    let f2_after = fact_repo.get(&f2.id).await.unwrap().unwrap();
    let f1_after = fact_repo.get(&f1.id).await.unwrap().unwrap();
    assert!(f2_after.stability < f1_after.stability);
    assert_eq!(crit_log.list_unreviewed(10).await.unwrap().len(), 2);

    // Community: assign new entity to existing community.
    let nextest = entity_repo
        .upsert_entity(&entity::NewEntity {
            name: "cargo-nextest".into(),
            entity_type: "tool".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let cargo = entity_repo
        .upsert_entity(&entity::NewEntity {
            name: "cargo".into(),
            entity_type: "tool".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let rust_tools = community_repo
        .create("rust_tools", "Rust toolchain", None)
        .await
        .unwrap();
    community_repo
        .add_member(&rust_tools.id, &cargo.id)
        .await
        .unwrap();

    co_activation::CoActivationRepo::new(db.clone())
        .increment_pair(&nextest.id, &cargo.id)
        .await
        .unwrap();

    #[derive(Debug)]
    struct AlwaysConfirm;
    #[async_trait::async_trait]
    impl cmo::AsyncConfirmFn for AlwaysConfirm {
        async fn confirm(&self, _e: &str, _t: &str, _c: &str, _s: &str, _x: f64) -> bool {
            true
        }
    }
    cmo::run_for_session(
        &entity_repo,
        &community_repo,
        vec![nextest.id.clone()],
        std::sync::Arc::new(AlwaysConfirm),
    )
    .await;

    let cs = community_repo.list_for_entity(&nextest.id).await.unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].id, rust_tools.id);
}

struct ScriptedMicroHandler(micro_reforge_types::MicroReforgeOutput);
#[async_trait::async_trait]
impl cognitive::services::micro_reforge::MicroReforgeHandler for ScriptedMicroHandler {
    async fn synthesize(
        &self,
        _: micro_reforge_types::MicroReforgeInput,
    ) -> common::Result<micro_reforge_types::MicroReforgeOutput> {
        Ok(self.0.clone())
    }
}

fn make_fact(subject: &str, predicate: &str, object: &str, confidence: f64) -> SemanticFact {
    let now = jiff::Timestamp::now().to_string();
    SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "test".into(),
        subject: subject.into(),
        predicate: predicate.into(),
        object: object.into(),
        confidence,
        source: "t".into(),
        valid_from: now.clone(),
        recorded_at: now,
        stability: 1.0,
        ..Default::default()
    }
}
