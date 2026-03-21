use bus::DomainEvent;
use chrono::Utc;
use desktop_shared::commands::{
    AtomAcceptParams, AtomDismissParams, AtomMigrationStatusResponse, AtomNextCardParams,
    AtomRestoreParams, AtomsForNoteParams, FlashcardResponse, KnowledgeAtomResponse,
};
use desktop_shared::errors::ApiError;

use super::notes::flashcard::flashcard_to_response;
use crate::state::AppCore;

fn atom_row_to_response(row: cognitive::KnowledgeAtomRow) -> KnowledgeAtomResponse {
    KnowledgeAtomResponse {
        id: row.id,
        subject: row.subject,
        atom_type: row.atom_type,
        domain: row.domain,
        source_note_id: row.source_note_id,
        source_range: row.source_range,
        source_context: row.source_context,
        semantic_fact_id: row.semantic_fact_id,
        retention_pct: row.retention_pct,
        personal_importance: row.personal_importance,
        status: row.status,
        salience: row.salience,
        last_interaction_ts: row.last_interaction_ts,
        metadata: row.metadata,
        topic_name: None, // TODO: join topic name in Phase 2
        linked_card_count: 0, // TODO: count linked cards in Phase 2
        created_at: row.created_at,
    }
}

impl AppCore {
    pub async fn atoms_for_note(
        &self,
        params: AtomsForNoteParams,
    ) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let atoms = repo
            .list_for_note(&params.note_id)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        Ok(atoms.into_iter().map(atom_row_to_response).collect())
    }

    pub async fn atom_accept(
        &self,
        params: AtomAcceptParams,
    ) -> Result<KnowledgeAtomResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let importance = params.personal_importance.unwrap_or(0.7);
        let atom = repo
            .accept(&params.atom_id, importance)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        if let Some(bus) = &self.domain_event_bus {
            let _ = bus.publish(DomainEvent::KnowledgeAtomAccepted {
                atom_id: atom.id.clone(),
                atom_type: atom.atom_type.clone(),
            });
        }

        if let Some(topic_id) = &atom.topic_id {
            let _ = repo.update_topic_aggregates(topic_id).await;
        }

        Ok(atom_row_to_response(atom))
    }

    pub async fn atom_dismiss(&self, params: AtomDismissParams) -> Result<(), ApiError> {
        let repo = self.knowledge_atom_repo()?;

        let atom = repo
            .get(&params.atom_id)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        repo.dismiss(&params.atom_id)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        if let Some(bus) = &self.domain_event_bus {
            let _ = bus.publish(DomainEvent::KnowledgeAtomArchived {
                atom_id: params.atom_id.clone(),
                reason: "user_dismissed".to_string(),
            });
        }

        if let Some(atom) = atom {
            if let Some(topic_id) = &atom.topic_id {
                let _ = repo.update_topic_aggregates(topic_id).await;
            }
        }

        Ok(())
    }

    pub async fn atom_restore(
        &self,
        params: AtomRestoreParams,
    ) -> Result<KnowledgeAtomResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let atom = repo
            .restore(&params.atom_id)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        if let Some(topic_id) = &atom.topic_id {
            let _ = repo.update_topic_aggregates(topic_id).await;
        }

        Ok(atom_row_to_response(atom))
    }

    pub async fn atom_next_card(
        &self,
        params: AtomNextCardParams,
    ) -> Result<Option<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let now = Utc::now().to_rfc3339();

        // Find the next due card linked to this atom
        let card: Option<cognitive::FlashcardRow> = sqlx::query_as(
            r#"
            SELECT * FROM flashcards
            WHERE atom_id = ?1
              AND suspended = 0
              AND (due_at IS NULL OR due_at <= ?2)
            ORDER BY due_at ASC
            LIMIT 1
            "#,
        )
        .bind(&params.atom_id)
        .bind(&now)
        .fetch_optional(repo.pool())
        .await
        .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        // Fallback: most recently created card for this atom
        let card = match card {
            Some(c) => Some(c),
            None => {
                sqlx::query_as::<_, cognitive::FlashcardRow>(
                    "SELECT * FROM flashcards WHERE atom_id = ?1 ORDER BY created_at DESC LIMIT 1",
                )
                .bind(&params.atom_id)
                .fetch_optional(repo.pool())
                .await
                .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?
            }
        };

        Ok(card.map(flashcard_to_response))
    }

    pub async fn atoms_migration_status(&self) -> Result<AtomMigrationStatusResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let pool = repo.pool();

        let sentinel: Option<(String,)> =
            sqlx::query_as("SELECT id FROM knowledge_atoms WHERE subject = ?1")
                .bind("__atoms_migration_v1__")
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_atoms WHERE subject != '__atoms_migration_v1__'")
                .fetch_one(pool)
                .await
                .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;

        Ok(AtomMigrationStatusResponse {
            migrated: sentinel.is_some(),
            count: count.0 as usize,
        })
    }
}
