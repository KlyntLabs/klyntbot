use bus::DomainEvent;
use desktop_shared::commands::{
    AtomAcceptParams, AtomBulkAcceptParams, AtomDismissParams, AtomMigrationStatusResponse,
    AtomNextCardParams, AtomRestoreParams, AtomsForNoteParams, FlashcardResponse,
    KnowledgeAtomResponse,
};
use desktop_shared::errors::ApiError;

use super::notes::flashcard::flashcard_to_response;
use crate::state::AppCore;

pub(crate) fn map_db(e: sqlx::Error) -> ApiError {
    ApiError::new("INTERNAL_ERROR", e.to_string())
}

pub(crate) fn atom_row_to_response(row: cognitive::KnowledgeAtomRow) -> KnowledgeAtomResponse {
    atom_row_to_response_enriched(row, None, 0)
}

fn atom_row_to_response_enriched(
    row: cognitive::KnowledgeAtomRow,
    topic_name: Option<String>,
    linked_card_count: i64,
) -> KnowledgeAtomResponse {
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
        topic_name,
        linked_card_count,
        created_at: row.created_at,
    }
}

impl AppCore {
    /// Enrich a single atom row with its topic name and linked flashcard count.
    async fn enrich_atom(
        &self,
        atom: cognitive::KnowledgeAtomRow,
    ) -> KnowledgeAtomResponse {
        let topic_name = if let Some(tid) = &atom.topic_id {
            if let Ok(repo) = self.knowledge_atom_repo() {
                repo.get_topic_names(std::slice::from_ref(tid))
                    .await
                    .ok()
                    .and_then(|m| m.into_values().next())
            } else {
                None
            }
        } else {
            None
        };

        let card_count = if let Ok(fc_repo) = self.flashcard_repo() {
            fc_repo
                .count_by_atom_ids(std::slice::from_ref(&atom.id))
                .await
                .ok()
                .and_then(|m| m.get(&atom.id).copied())
                .unwrap_or(0)
        } else {
            0
        };

        atom_row_to_response_enriched(atom, topic_name, card_count)
    }

    pub async fn atoms_for_note(
        &self,
        params: AtomsForNoteParams,
    ) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
        let atom_repo = self.knowledge_atom_repo()?;
        let atoms = atom_repo
            .list_for_note(&params.note_id)
            .await
            .map_err(map_db)?;

        // Batch-fetch topic names and linked card counts concurrently.
        let topic_ids: Vec<String> = atoms
            .iter()
            .filter_map(|a| a.topic_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let atom_ids: Vec<String> = atoms.iter().map(|a| a.id.clone()).collect();

        let topic_names_fut = atom_repo.get_topic_names(&topic_ids);
        let card_counts_fut = async {
            if let Ok(fc_repo) = self.flashcard_repo() {
                fc_repo.count_by_atom_ids(&atom_ids).await.ok()
            } else {
                None
            }
        };

        let (topic_names, card_counts) = tokio::join!(topic_names_fut, card_counts_fut);
        let topic_names = topic_names.unwrap_or_default();
        let card_counts = card_counts.unwrap_or_default();

        Ok(atoms
            .into_iter()
            .map(|a| {
                let tname = a
                    .topic_id
                    .as_ref()
                    .and_then(|tid| topic_names.get(tid).cloned());
                let count = card_counts.get(&a.id).copied().unwrap_or(0);
                atom_row_to_response_enriched(a, tname, count)
            })
            .collect())
    }

    pub async fn atom_accept(
        &self,
        params: AtomAcceptParams,
    ) -> Result<KnowledgeAtomResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let importance = params.personal_importance.unwrap_or(0.7);
        let mut atom = repo
            .accept(&params.atom_id, importance)
            .await
            .map_err(map_db)?;

        // Ensure atom has a topic — auto-extracted atoms may not have one yet.
        if atom.topic_id.is_none() {
            let topic = repo
                .get_or_create_topic(&atom.domain, &atom.domain)
                .await
                .map_err(map_db)?;
            sqlx::query("UPDATE knowledge_atoms SET topic_id = ?1 WHERE id = ?2")
                .bind(&topic.id)
                .bind(&atom.id)
                .execute(self.storage_pool.inner())
                .await
                .map_err(map_db)?;
            atom.topic_id = Some(topic.id.clone());
        }

        if let Some(bus) = &self.domain_event_bus {
            let _ = bus.publish(DomainEvent::KnowledgeAtomAccepted {
                atom_id: atom.id.clone(),
                atom_type: atom.atom_type.clone(),
            });
        }

        if let Some(topic_id) = &atom.topic_id {
            let _ = repo.update_topic_aggregates(topic_id).await;
        }

        Ok(self.enrich_atom(atom).await)
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

        Ok(self.enrich_atom(atom).await)
    }

    pub async fn atom_next_card(
        &self,
        params: AtomNextCardParams,
    ) -> Result<Option<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = repo.next_for_atom(&params.atom_id).await.map_err(map_db)?;
        Ok(card.map(flashcard_to_response))
    }

    pub async fn atoms_bulk_accept(
        &self,
        params: AtomBulkAcceptParams,
    ) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let mut accepted_atoms = Vec::new();
        let mut topic_ids_to_update = std::collections::HashSet::new();

        for id in &params.atom_ids {
            let mut atom = repo
                .accept(id, params.personal_importance)
                .await
                .map_err(map_db)?;

            // Ensure atom has a topic — auto-extracted atoms may not have one yet.
            if atom.topic_id.is_none() {
                let topic = repo
                    .get_or_create_topic(&atom.domain, &atom.domain)
                    .await
                    .map_err(map_db)?;
                sqlx::query("UPDATE knowledge_atoms SET topic_id = ?1 WHERE id = ?2")
                    .bind(&topic.id)
                    .bind(&atom.id)
                    .execute(self.storage_pool.inner())
                    .await
                    .map_err(map_db)?;
                atom.topic_id = Some(topic.id.clone());
            }

            if let Some(bus) = &self.domain_event_bus {
                let _ = bus.publish(DomainEvent::KnowledgeAtomAccepted {
                    atom_id: atom.id.clone(),
                    atom_type: atom.atom_type.clone(),
                });
            }

            if let Some(tid) = &atom.topic_id {
                topic_ids_to_update.insert(tid.clone());
            }

            accepted_atoms.push(atom);
        }

        // Update topic aggregates once per unique topic (instead of per atom).
        for tid in &topic_ids_to_update {
            let _ = repo.update_topic_aggregates(tid).await;
        }

        // Batch-fetch topic names and linked card counts concurrently.
        let topic_ids: Vec<String> = topic_ids_to_update.into_iter().collect();
        let atom_ids: Vec<String> = accepted_atoms.iter().map(|a| a.id.clone()).collect();

        let topic_names_fut = repo.get_topic_names(&topic_ids);
        let card_counts_fut = async {
            if let Ok(fc_repo) = self.flashcard_repo() {
                fc_repo.count_by_atom_ids(&atom_ids).await.ok()
            } else {
                None
            }
        };

        let (topic_names, card_counts) = tokio::join!(topic_names_fut, card_counts_fut);
        let topic_names = topic_names.unwrap_or_default();
        let card_counts = card_counts.unwrap_or_default();

        Ok(accepted_atoms
            .into_iter()
            .map(|a| {
                let tname = a
                    .topic_id
                    .as_ref()
                    .and_then(|tid| topic_names.get(tid).cloned());
                let count = card_counts.get(&a.id).copied().unwrap_or(0);
                atom_row_to_response_enriched(a, tname, count)
            })
            .collect())
    }

    pub async fn atoms_migration_status(&self) -> Result<AtomMigrationStatusResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let (migrated, count) = repo.migration_status().await.map_err(map_db)?;
        Ok(AtomMigrationStatusResponse { migrated, count })
    }
}
