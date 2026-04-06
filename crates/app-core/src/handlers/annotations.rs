use cognitive::repos::AnnotationRepo;
use cognitive::types::Annotation;
use desktop_shared::commands::{
    AiSuggestionResponse, AnnotationCreateParams, AnnotationResponse, AnnotationUpdateParams,
    LinkedContextParams, LinkedContextResponse, LinkedFact, LinkedMemory, LinkedRule,
};
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn annotation_create(
        &self,
        params: AnnotationCreateParams,
    ) -> Result<AnnotationResponse, ApiError> {
        let repo = AnnotationRepo::new(self.storage_pool.inner().clone());
        let now = chrono::Utc::now().to_rfc3339();
        let annotation = Annotation {
            id: params.mark_id.clone(),
            target_type: "note".into(),
            target_id: params.note_id.clone(),
            content: params.content,
            tags: params.tags.unwrap_or_default(),
            author: "user".into(),
            priority: 0,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
            access_count: 0,
            mark_id: Some(params.mark_id),
            quoted_text: params.quoted_text,
            range_start: params.range_start,
            range_end: params.range_end,
            ai_suggestion: params.ai_suggestion,
        };
        repo.upsert(&annotation).await.map_err(map_cognitive_err)?;
        Ok(annotation_to_response(&annotation))
    }

    pub async fn annotation_update(
        &self,
        params: AnnotationUpdateParams,
    ) -> Result<AnnotationResponse, ApiError> {
        let repo = AnnotationRepo::new(self.storage_pool.inner().clone());
        let mut annotation = repo
            .get_by_id(&params.id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("Annotation {} not found", params.id))
            })?;

        if let Some(content) = params.content {
            annotation.content = content;
        }
        if let Some(tags) = params.tags {
            annotation.tags = tags;
        }
        annotation.updated_at = chrono::Utc::now().to_rfc3339();

        repo.upsert(&annotation).await.map_err(map_cognitive_err)?;
        Ok(annotation_to_response(&annotation))
    }

    pub async fn annotation_delete(&self, id: String) -> Result<(), ApiError> {
        let repo = AnnotationRepo::new(self.storage_pool.inner().clone());
        repo.delete(&id).await.map_err(map_cognitive_err)?;
        Ok(())
    }

    pub async fn annotation_list_for_note(
        &self,
        note_id: String,
        limit: Option<i64>,
    ) -> Result<Vec<AnnotationResponse>, ApiError> {
        let repo = AnnotationRepo::new(self.storage_pool.inner().clone());
        let annotations = repo
            .list_for_note(&note_id, limit.unwrap_or(100).min(500))
            .await
            .map_err(map_cognitive_err)?;
        Ok(annotations.iter().map(annotation_to_response).collect())
    }

    pub async fn annotation_get_ai_suggestion(
        &self,
        _note_id: String,
        selected_text: String,
    ) -> Result<AiSuggestionResponse, ApiError> {
        let pool = self.storage_pool.inner().clone();
        let sf_repo = cognitive::repos::SemanticFactRepo::new(pool);
        let facts = sf_repo
            .search_fts(&selected_text, None, 5)
            .await
            .map_err(map_cognitive_err)?;

        let suggestion = facts
            .first()
            .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object));
        let confidence = if facts.is_empty() {
            0.0
        } else {
            facts[0].confidence
        };
        let related_fact_ids = facts.iter().map(|f| f.id.clone()).collect();

        Ok(AiSuggestionResponse {
            suggestion,
            confidence,
            related_fact_ids,
        })
    }

    pub async fn note_get_linked_context(
        &self,
        params: LinkedContextParams,
    ) -> Result<LinkedContextResponse, ApiError> {
        let pool = self.storage_pool.inner().clone();
        let sf_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
        let em_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
        let pr_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
        let ann_repo = AnnotationRepo::new(pool);

        let (facts, memories, rules, annotations) = tokio::join!(
            sf_repo.search_fts(&params.section_text, None, 10),
            em_repo.search_fts(&params.section_text, None, 10),
            pr_repo.search_fts(&params.section_text, None, 10),
            ann_repo.search(&params.section_text, 10),
        );

        let semantic_facts = facts
            .unwrap_or_default()
            .into_iter()
            .map(|f| LinkedFact {
                id: f.id,
                subject: f.subject,
                predicate: f.predicate,
                object: f.object,
                confidence: f.confidence,
                source_note: f.project_id,
            })
            .collect();

        let episodic_memories = memories
            .unwrap_or_default()
            .into_iter()
            .map(|m| LinkedMemory {
                id: m.id,
                content: m.content,
                domain: m.domain,
                created_at: m.recorded_at,
            })
            .collect();

        let procedural_rules = rules
            .unwrap_or_default()
            .into_iter()
            .map(|r| LinkedRule {
                id: r.id,
                rule_text: r.rule_text,
                domain: r.domain,
                signal_count: r.signal_count,
            })
            .collect();

        let related_annotations = annotations
            .unwrap_or_default()
            .iter()
            .map(annotation_to_response)
            .collect();

        Ok(LinkedContextResponse {
            semantic_facts,
            episodic_memories,
            related_annotations,
            procedural_rules,
        })
    }
}

fn annotation_to_response(a: &Annotation) -> AnnotationResponse {
    AnnotationResponse {
        id: a.id.clone(),
        note_id: a.target_id.clone(),
        mark_id: a.mark_id.clone(),
        content: a.content.clone(),
        quoted_text: a.quoted_text.clone(),
        range_start: a.range_start,
        range_end: a.range_end,
        ai_suggestion: a.ai_suggestion.clone(),
        tags: a.tags.clone(),
        created_at: a.created_at.clone(),
        updated_at: a.updated_at.clone(),
    }
}
