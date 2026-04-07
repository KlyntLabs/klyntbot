//! Stage 3: executes PromotionOps against repos.

use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::embedder::SemanticFactEmbedder;
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact};
use super::consolidator::PromotionOp;

pub async fn execute_promotions(
    ops: &[PromotionOp],
    fact_repo: &SemanticFactRepo,
    rule_repo: &ProceduralRuleRepo,
    episodic_repo: &Option<EpisodicMemoryRepo>,
    embedder: Option<&dyn SemanticFactEmbedder>,
) {
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
            } => {
                match fact_repo.find_similar(subject, predicate).await {
                    Ok(existing) if !existing.is_empty() => {
                        let best = &existing[0];
                        let _ = fact_repo
                            .update_confidence(&best.id, best.confidence.max(*confidence))
                            .await;
                        let _ = fact_repo.update_convergence(&best.id, *convergence).await;
                        debug!("Writer: reinforced fact '{}'", best.id);
                    }
                    _ => {
                        let now = Utc::now().to_rfc3339();
                        let fact = SemanticFact {
                            id: Uuid::new_v4().to_string(),
                            domain: domain.clone(),
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
                        }
                    }
                }
            }
            PromotionOp::CreateRule {
                rule_text,
                domain,
                confidence,
            } => match rule_repo.find_similar(rule_text, domain).await {
                Ok(Some(existing)) => {
                    let _ = rule_repo.increment_signal_count(&existing.id).await;
                    debug!("Writer: reinforced rule '{}'", existing.id);
                }
                _ => {
                    let now = Utc::now().to_rfc3339();
                    let rule = ProceduralRule {
                        id: Uuid::new_v4().to_string(),
                        domain: domain.clone(),
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
                    let now = Utc::now().to_rfc3339();
                    let memory = EpisodicMemory {
                        id: Uuid::new_v4().to_string(),
                        domain: domain.clone(),
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
                    }
                }
            }
        }
    }
    info!("Writer: {facts} facts, {rules} rules, {episodes} episodes");
}
