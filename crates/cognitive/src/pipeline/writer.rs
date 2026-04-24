//! Stage 3: executes PromotionOps against repos.

use jiff::Timestamp;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::consolidator::PromotionOp;
use crate::embedder::SemanticFactEmbedder;
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::services::background::PipelineEvent;
use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact};

pub async fn execute_promotions(
    ops: &[PromotionOp],
    fact_repo: &SemanticFactRepo,
    rule_repo: &ProceduralRuleRepo,
    episodic_repo: &Option<EpisodicMemoryRepo>,
    embedder: Option<&dyn SemanticFactEmbedder>,
    pipeline_tx: Option<&broadcast::Sender<PipelineEvent>>,
) {
    let emit = |operation: &str, fact: String| {
        if let Some(tx) = pipeline_tx {
            let _ = tx.send(PipelineEvent::Consolidation {
                operation: operation.to_string(),
                fact,
            });
        }
    };
    let (mut facts, mut rules, mut episodes) = (0u32, 0u32, 0u32);

    for op in ops {
        match op {
            PromotionOp::CreateFact {
                subject,
                predicate,
                object,
                domain,
                confidence,
                convergence,
                source,
            } => match fact_repo.find_similar(subject, predicate).await {
                Ok(existing) if !existing.is_empty() => {
                    let best = &existing[0];
                    let _ = fact_repo
                        .update_confidence(&best.id, best.confidence.max(*confidence))
                        .await;
                    let _ = fact_repo.update_convergence(&best.id, *convergence).await;
                    debug!("Writer: reinforced fact '{}'", best.id);
                    emit("update", format!("{subject}.{predicate} = {object}"));
                }
                _ => {
                    let now = Timestamp::now().to_string();
                    let fact = SemanticFact {
                        id: Uuid::new_v4().to_string(),
                        domain: domain.as_str().to_string(),
                        subject: subject.clone(),
                        predicate: predicate.clone(),
                        object: object.clone(),
                        confidence: *confidence,
                        source: source.clone(),
                        valid_from: now.clone(),
                        valid_until: None,
                        recorded_at: now.clone(),
                        superseded_at: None,
                        superseded_by: None,
                        stability: 1.0,
                        last_accessed: Some(now),
                        access_count: 0,
                        convergence_score: *convergence,
                        project_id: None,
                        memory_type: "fact".into(),
                        scope_type: "system".into(),
                        scope_id: None,
                    };
                    if let Err(e) = fact_repo.upsert(&fact).await {
                        warn!("Writer: failed to create fact: {e}");
                    } else {
                        if let Some(emb) = embedder {
                            let _ = emb.embed_and_store_fact(&fact).await;
                        }
                        facts += 1;
                        emit("add", format!("{subject}.{predicate} = {object}"));
                    }
                }
            },
            PromotionOp::CreateRule {
                rule_text,
                domain,
                confidence,
            } => match rule_repo.find_similar(rule_text, domain.as_str()).await {
                Ok(Some(existing)) => {
                    let _ = rule_repo.increment_signal_count(&existing.id).await;
                    debug!("Writer: reinforced rule '{}'", existing.id);
                    emit("update", format!("rule/{}: {rule_text}", domain.as_str()));
                }
                _ => {
                    let now = Timestamp::now().to_string();
                    let rule = ProceduralRule {
                        id: Uuid::new_v4().to_string(),
                        domain: domain.as_str().to_string(),
                        rule_text: rule_text.clone(),
                        confidence: *confidence,
                        source: "pipeline".into(),
                        signal_count: 1,
                        created_at: now.clone(),
                        updated_at: now,
                        active: true,
                        project_id: None,
                        scope_type: "system".into(),
                        scope_id: None,
                    };
                    if let Err(e) = rule_repo.upsert(&rule).await {
                        warn!("Writer: failed to create rule: {e}");
                    } else {
                        rules += 1;
                        emit("add", format!("rule/{}: {rule_text}", domain.as_str()));
                    }
                }
            },
            PromotionOp::CreateEpisode {
                content,
                summary,
                domain,
                importance,
            } => {
                if let Some(ep_repo) = episodic_repo {
                    let now = Timestamp::now().to_string();
                    let memory = EpisodicMemory {
                        id: Uuid::new_v4().to_string(),
                        domain: domain.as_str().to_string(),
                        content: content.clone(),
                        summary: Some(summary.clone()),
                        importance: *importance,
                        occurred_at: now.clone(),
                        recorded_at: now,
                        stability: 1.0,
                        last_accessed: None,
                        access_count: 0,
                        project_id: None,
                        scope_type: "system".into(),
                        scope_id: None,
                    };
                    if let Err(e) = ep_repo.insert(&memory).await {
                        warn!("Writer: failed to create episode: {e}");
                    } else {
                        episodes += 1;
                        emit("add", format!("episode/{}: {summary}", domain.as_str()));
                    }
                }
            }
        }
    }
    info!("Writer: {facts} facts, {rules} rules, {episodes} episodes");
}

/// Persist extracted entities and relationships via EntityRepo.
///
/// Upserts entities (deduplicating by name+type), then upserts relationships
/// between resolved entities. Failures are logged and skipped (non-fatal).
pub async fn persist_entities(
    entity_repo: &crate::repos::EntityRepo,
    entities: &[crate::extraction::ExtractedEntity],
    relationships: &[crate::extraction::ExtractedRelationship],
) {
    use std::collections::HashMap;

    let mut name_to_id: HashMap<String, String> = HashMap::new();
    for entity in entities {
        let new_entity = crate::repos::entity::NewEntity {
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            description: entity.description.clone(),
            source: "extracted".to_string(),
            source_id: None,
            metadata: None,
        };
        match entity_repo.upsert_entity(&new_entity).await {
            Ok(row) => {
                name_to_id.insert(entity.name.to_lowercase(), row.id);
            }
            Err(e) => {
                debug!("Failed to upsert entity '{}': {e}", entity.name);
            }
        }
    }

    for rel in relationships {
        let source_id = name_to_id.get(&rel.source_name.to_lowercase());
        let target_id = name_to_id.get(&rel.target_name.to_lowercase());
        if let (Some(src), Some(tgt)) = (source_id, target_id) {
            let new_rel = crate::repos::entity::NewRelationship {
                source_entity_id: src.clone(),
                target_entity_id: tgt.clone(),
                relationship_type: rel.relationship_type.clone(),
                evidence: None,
                source: "extracted".to_string(),
            };
            if let Err(e) = entity_repo.upsert_relationship(&new_rel).await {
                debug!("Failed to upsert relationship: {e}");
            }
        }
    }
}
