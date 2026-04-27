use desktop_shared::commands::ReviewSessionSaveParams;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn flashcard_save_session(
        &self,
        params: ReviewSessionSaveParams,
    ) -> Result<(), ApiError> {
        let repo = self.review_session_repo()?;

        // Create session first (in case it doesn't exist yet)
        let _ = repo.create(&params.session_id).await;

        if params.status == "abandoned" {
            repo.abandon(&params.session_id, params.cards_reviewed as i64)
                .await
                .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        } else {
            let modes_json = serde_json::to_string(&params.modes_used)
                .map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string()))?;
            let weak_json = serde_json::to_string(&params.weak_card_ids)
                .map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string()))?;
            repo.complete(
                &params.session_id,
                params.cards_reviewed as i64,
                Some(params.avg_score),
                Some(params.duration_seconds as i64),
                Some(modes_json.as_str()),
                params.propagation_count as i64,
                Some(weak_json.as_str()),
                Some(params.session_data.as_str()),
            )
            .await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        }

        // Publish domain event
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::FlashcardSessionCompleted {
                session_id: params.session_id,
                cards_reviewed: params.cards_reviewed as usize,
                avg_score: params.avg_score,
                weak_domains: vec![],
                propagation_count: params.propagation_count as usize,
            });
        }

        Ok(())
    }
}
