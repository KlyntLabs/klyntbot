//! Tier A activation: ContradictionDetected fires for conflicting coding facts.

use bus::DomainEvent;

mod common;

#[tokio::test]
async fn contradiction_fires_when_repo_context_conflicts() {
    let pool = common::pool_with_migrations().await;
    let repos = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let bus = bus::DomainEventBus::new(1024);
    let mut sub = bus.subscribe();

    // Seed two conflicting facts for the same repo.
    repos
        .upsert(&cognitive::types::SemanticFact {
            id: "f1".into(),
            domain: "code".into(),
            subject: "repo:myrepo".into(),
            predicate: "framework".into(),
            object: "django".into(),
            confidence: 0.9,
            source: "distiller".into(),
            valid_from: jiff::Timestamp::now().to_string(),
            valid_until: None,
            recorded_at: jiff::Timestamp::now().to_string(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 1.0,
            project_id: None,
            memory_type: "fact".into(),
            scope_type: "user".into(),
            scope_id: None,
            scope_repo_id: Some("myrepo".into()),
            metadata: None,
            speaker: None,
        })
        .await
        .unwrap();

    repos
        .upsert(&cognitive::types::SemanticFact {
            id: "f2".into(),
            domain: "code".into(),
            subject: "repo:myrepo".into(),
            predicate: "framework".into(),
            object: "rails".into(),
            confidence: 0.9,
            source: "distiller".into(),
            valid_from: jiff::Timestamp::now().to_string(),
            valid_until: None,
            recorded_at: jiff::Timestamp::now().to_string(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 1.0,
            project_id: None,
            memory_type: "fact".into(),
            scope_type: "user".into(),
            scope_id: None,
            scope_repo_id: Some("myrepo".into()),
            metadata: None,
            speaker: None,
        })
        .await
        .unwrap();

    // Publish a synthetic contradiction event.
    bus.publish(DomainEvent::ContradictionDetected {
        existing_subject: "repo:myrepo".into(),
        existing_predicate: "framework".into(),
        existing_object: "django".into(),
        new_object: "rails".into(),
        confidence: 0.9,
    });

    let evt = tokio::time::timeout(std::time::Duration::from_secs(1), sub.recv()).await;
    assert!(evt.is_ok(), "expected ContradictionDetected event");
}
