//! Memory promotion pipeline — elevate observations from persona→squad→global scope.

use uuid::Uuid;

use crate::repos::{BlackboardEntry, SemanticFactRepo};
use crate::types::SemanticFact;

/// Promote a fact from one scope to a higher scope.
///
/// Creates a new fact in the target scope with a new ID, preserving the content.
/// The original fact is kept (not superseded) — both scopes retain the knowledge.
pub async fn promote_fact(
    repo: &SemanticFactRepo,
    fact_id: &str,
    target_scope_type: &str,
    target_scope_id: Option<&str>,
) -> Result<Option<SemanticFact>, sqlx::Error> {
    let Some(original) = repo.get(fact_id).await? else {
        return Ok(None);
    };

    let promoted = SemanticFact {
        id: Uuid::new_v4().to_string(),
        scope_type: target_scope_type.to_string(),
        scope_id: target_scope_id.map(|s| s.to_string()),
        source: format!("promoted:{}", original.source),
        recorded_at: jiff::Timestamp::now().to_string(),
        ..original
    };

    repo.upsert(&promoted).await?;

    Ok(Some(promoted))
}

/// Promote high-confidence blackboard entries to squad-scoped semantic facts.
///
/// Entries with confidence >= threshold and entry_type "observation", "claim", or "agreement"
/// are converted to semantic facts in squad scope.
/// Returns promoted facts. Event emission is the caller's responsibility (agent crate).
pub async fn promote_from_blackboard(
    fact_repo: &SemanticFactRepo,
    entries: &[BlackboardEntry],
    squad_id: &str,
    confidence_threshold: f64,
) -> Vec<SemanticFact> {
    let mut promoted = Vec::new();
    for entry in entries {
        if entry.confidence < confidence_threshold {
            continue;
        }
        if !matches!(
            entry.entry_type.as_str(),
            "observation" | "claim" | "agreement"
        ) {
            continue;
        }

        let fact = SemanticFact {
            id: Uuid::new_v4().to_string(),
            domain: "debate".into(),
            subject: entry.persona_name.clone(),
            predicate: "observed".into(),
            object: entry.content.clone(),
            confidence: entry.confidence,
            source: format!("debate:{}", entry.session_key),
            valid_from: jiff::Timestamp::now().to_string(),
            valid_until: None,
            recorded_at: jiff::Timestamp::now().to_string(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: "squad_knowledge".into(),
            scope_type: "squad".into(),
            scope_id: Some(squad_id.to_string()),
            scope_repo_id: None,
            metadata: None,
        };

        if fact_repo.upsert(&fact).await.is_ok() {
            promoted.push(fact);
        }
    }
    promoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_promote_fact_to_squad() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = SemanticFactRepo::new(pool);

        let fact = SemanticFact {
            id: "persona-fact-1".into(),
            domain: "finance".into(),
            subject: "index funds".into(),
            predicate: "risk_level".into(),
            object: "low".into(),
            confidence: 0.95,
            source: "debate".into(),
            valid_from: jiff::Timestamp::now().to_string(),
            valid_until: None,
            recorded_at: jiff::Timestamp::now().to_string(),
            superseded_at: None,
            superseded_by: None,
            stability: 2.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: "observation".into(),
            scope_type: "persona".into(),
            scope_id: Some("builtin-deep-analyst".into()),
            scope_repo_id: None,
            metadata: None,
        };
        repo.upsert(&fact).await.unwrap();

        let promoted = promote_fact(
            &repo,
            "persona-fact-1",
            "squad",
            Some("builtin-squad-finance"),
        )
        .await
        .unwrap();
        assert!(promoted.is_some());
        let p = promoted.unwrap();
        assert_eq!(p.scope_type, "squad");
        assert_eq!(p.scope_id.as_deref(), Some("builtin-squad-finance"));
        assert_ne!(p.id, "persona-fact-1"); // New ID
    }

    #[tokio::test]
    async fn test_promote_nonexistent_fact() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = SemanticFactRepo::new(pool);
        let result = promote_fact(&repo, "nonexistent", "squad", None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_promote_from_blackboard_filters_by_confidence() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = SemanticFactRepo::new(pool);

        let entries = vec![
            BlackboardEntry {
                id: "e1".into(),
                session_key: "debate-1".into(),
                squad_id: "sq1".into(),
                round: 1,
                persona_id: "p1".into(),
                persona_name: "Analyst".into(),
                entry_type: "observation".into(),
                content: "High confidence claim".into(),
                confidence: 0.9,
                references_entry_id: None,
                created_at: "now".into(),
            },
            BlackboardEntry {
                id: "e2".into(),
                session_key: "debate-1".into(),
                squad_id: "sq1".into(),
                round: 1,
                persona_id: "p2".into(),
                persona_name: "Skeptic".into(),
                entry_type: "challenge".into(), // Not promotable
                content: "Challenge".into(),
                confidence: 0.95,
                references_entry_id: None,
                created_at: "now".into(),
            },
            BlackboardEntry {
                id: "e3".into(),
                session_key: "debate-1".into(),
                squad_id: "sq1".into(),
                round: 1,
                persona_id: "p3".into(),
                persona_name: "Strategist".into(),
                entry_type: "observation".into(),
                content: "Low confidence claim".into(),
                confidence: 0.5, // Below threshold
                references_entry_id: None,
                created_at: "now".into(),
            },
        ];

        let promoted = promote_from_blackboard(&repo, &entries, "sq1", 0.85).await;
        // Only e1 passes: observation + confidence >= 0.85
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0].object, "High confidence claim");
        assert_eq!(promoted[0].scope_type, "squad");
    }
}
