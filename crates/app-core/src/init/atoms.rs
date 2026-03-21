use cognitive::repos::{KnowledgeAtomRepo, NewKnowledgeAtom};
use sqlx::SqlitePool;
use tracing::{info, warn};

fn map_db(e: sqlx::Error) -> common::KlyntbotError {
    common::KlyntbotError::Storage(e.to_string())
}

/// Migration flag stored as a knowledge_atom with a sentinel subject.
const MIGRATION_SENTINEL: &str = "__atoms_migration_v1__";

/// Row type for vocabulary semantic facts.
#[derive(sqlx::FromRow)]
struct VocabFactRow {
    id: String,
    subject: String,
    object: String,
    source: String,
}

/// One-time migration: create Knowledge Atoms from existing vocabulary SemanticFacts.
pub async fn migrate_vocab_to_atoms(pool: &SqlitePool) -> common::Result<usize> {
    // Check if already migrated
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM knowledge_atoms WHERE subject = ?1")
            .bind(MIGRATION_SENTINEL)
            .fetch_optional(pool)
            .await
            .map_err(map_db)?;
    if existing.is_some() {
        return Ok(0);
    }

    let atom_repo = KnowledgeAtomRepo::new(pool.clone());

    // Fetch all active vocabulary semantic facts
    let facts: Vec<VocabFactRow> = sqlx::query_as(
        r#"
        SELECT id, subject, object, source
        FROM semantic_facts
        WHERE memory_type = 'vocabulary'
          AND valid_until IS NULL
          AND superseded_at IS NULL
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db)?;

    if facts.is_empty() {
        info!("No vocabulary facts to migrate — skipping atom migration");
        // Still set the sentinel so we don't re-check
        set_sentinel(pool).await?;
        return Ok(0);
    }

    info!(
        "Migrating {} vocabulary items to Knowledge Atoms",
        facts.len()
    );

    let mut count = 0;
    for fact in &facts {
        let domain = "language:unknown".to_string();
        let topic = atom_repo
            .get_or_create_topic(&domain, &domain)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let note_id = parse_note_source(&fact.source);

        let atom = atom_repo
            .create(&NewKnowledgeAtom {
                subject: fact.subject.clone(),
                atom_type: "vocabulary".to_string(),
                domain,
                source_note_id: note_id,
                source_context: Some(fact.object.clone()),
                semantic_fact_id: Some(fact.id.clone()),
                personal_importance: 0.7,
                status: "active".to_string(),
                metadata: None,
                topic_id: Some(topic.id.clone()),
                ..Default::default()
            })
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        // Link existing flashcard if found
        if let Err(e) = sqlx::query(
            "UPDATE flashcards SET atom_id = ?1 WHERE front = ?2 AND card_type = 'vocabulary' AND atom_id IS NULL",
        )
        .bind(&atom.id)
        .bind(&fact.subject)
        .execute(pool)
        .await
        {
            warn!("Failed to link flashcard for atom {}: {e}", atom.id);
        }

        count += 1;
    }

    // Update topic aggregates
    if let Err(e) = atom_repo.update_all_topic_aggregates().await {
        warn!("Failed to update topic aggregates: {e}");
    }

    // Set migration flag
    set_sentinel(pool).await?;

    info!("Migration complete: {count} atoms created");
    Ok(count)
}

async fn set_sentinel(pool: &SqlitePool) -> common::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO knowledge_atoms (id, subject, atom_type, domain, retention_pct, stability, difficulty, personal_importance, status, salience, created_at, updated_at) VALUES (?1, ?2, 'fact', 'system', 1.0, 1.0, 5.0, 0.0, 'active', 0.0, ?3, ?3)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(MIGRATION_SENTINEL)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(map_db)?;
    Ok(())
}

fn parse_note_source(source: &str) -> Option<String> {
    source.strip_prefix("note:").map(|s| s.to_string())
}
