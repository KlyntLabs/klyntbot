use bus::DomainEvent;
use desktop_shared::commands::{
    AtomAcceptParams, AtomDismissParams, AtomMigrationStatusResponse, AtomNextCardParams,
    AtomRestoreParams, AtomsForNoteParams, FlashcardResponse, KnowledgeAtomResponse,
};
use desktop_shared::errors::ApiError;

use super::notes::flashcard::flashcard_to_response;
use crate::state::AppCore;

fn map_db(e: sqlx::Error) -> ApiError {
    ApiError::new("INTERNAL_ERROR", e.to_string())
}

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
            .map_err(map_db)?;
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
            .map_err(map_db)?;

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

        let atom = repo.get(&params.atom_id).await.map_err(map_db)?;

        repo.dismiss(&params.atom_id).await.map_err(map_db)?;

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
        let atom = repo.restore(&params.atom_id).await.map_err(map_db)?;

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
        let card = repo.next_for_atom(&params.atom_id).await.map_err(map_db)?;
        Ok(card.map(flashcard_to_response))
    }

    pub async fn atoms_migration_status(&self) -> Result<AtomMigrationStatusResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let (migrated, count) = repo.migration_status().await.map_err(map_db)?;
        Ok(AtomMigrationStatusResponse { migrated, count })
    }
}
