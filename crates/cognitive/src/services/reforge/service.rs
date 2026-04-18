//! Phase orchestrator for the Reforge cycle.
//!
//! `run_reforge` drives all 8 phases: Collect → Synthesize → Review →
//! Narrate → Apply → Optimize → Graph Consolidation → Compact.  Each phase is isolated so that
//! a single failure does not abort the remaining phases.

use std::collections::HashMap;

use jiff::Timestamp;
use sqlx;
use tracing::{debug, info, warn};

use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::services::reforge::skill_files::{
    compute_diff, content_hash, SkillFile, SkillFileManager,
};
use crate::services::reforge::types::*;
use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};

use common::helpers::truncate_chars;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the full Reforge cycle, returning `None` when the collector decides
/// there is nothing new to process.
#[allow(clippy::too_many_arguments)]
pub async fn run_reforge(
    reforge_state_repo: &storage::repos::ReforgeStateRepo,
    skill_version_repo: &storage::repos::SkillVersionRepo,
    session_memory_repo: &storage::SessionMemoryRepo,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    handler: &dyn super::ReforgeHandler,
    skill_mgr: &SkillFileManager,
    pre_read_skill_files: Option<HashMap<String, Vec<SkillFile>>>,
    mirror_repo: Option<&crate::mirror::MirrorRepo>,
    feedback_repo: Option<&storage::RetrievalFeedbackRepo>,
    autotuner_bridge: Option<&dyn super::AutotunerBridge>,
    autotuner_ctx: Option<AutotunerContext>,
    feedback_sources: Option<&super::collector::FeedbackSources<'_>>,
    graph_enrichment_handler: Option<&dyn super::GraphEnrichmentHandler>,
    density_repo: Option<&crate::repos::ConversationDensityRepo>,
    entity_repo: Option<&crate::repos::EntityRepo>,
    snapshot_repo: Option<&crate::repos::KnowledgeSnapshotRepo>,
    community_intelligence_handler: Option<&dyn super::CommunityIntelligenceHandler>,
    community_repo: Option<&crate::repos::CommunityRepo>,
    co_activation_repo_for_split: Option<&crate::repos::CoActivationRepo>,
) -> Option<ReforgeResult> {
    let mut result = ReforgeResult::default();

    // ------------------------------------------------------------------
    // Fetch last run timestamp
    // ------------------------------------------------------------------
    let last_run_at = match reforge_state_repo.get().await {
        Ok(state) => state.last_run_at,
        Err(e) => {
            warn!("Reforge: failed to read reforge_state: {e}");
            None
        }
    };

    // ------------------------------------------------------------------
    // Phase 1: Collect
    // ------------------------------------------------------------------
    info!("Reforge Phase 1: Collect");
    let collected = super::collector::collect(
        last_run_at.as_deref(),
        session_memory_repo,
        fact_repo,
        episodic_repo,
        rule_repo,
        skill_mgr,
        pre_read_skill_files,
        mirror_repo,
        feedback_repo,
        feedback_sources,
    )
    .await;

    let collected = match collected {
        Some(mut c) => {
            // Inject autotuner context from the cron handler (where metric_source is available).
            c.autotuner_ctx = autotuner_ctx;
            c
        }
        None => {
            info!("Reforge: skipped — no new data");
            return None;
        }
    };

    // Snapshot content hashes at collection time for conflict detection.
    let collected_hashes: HashMap<(String, String), String> = collected
        .skill_files
        .iter()
        .flat_map(|(_, files)| {
            files.iter().map(|f| {
                (
                    (f.skill_name.clone(), f.file_path.clone()),
                    f.content_hash.clone(),
                )
            })
        })
        .collect();

    // ------------------------------------------------------------------
    // Phase 2: Synthesize (LLM call #1)
    // ------------------------------------------------------------------
    info!("Reforge Phase 2: Synthesize");
    let synthesize_input = build_synthesize_input(&collected);
    let synthesize_output = match handler.synthesize(&synthesize_input).await {
        Ok(output) => {
            debug!(
                facts = output.fact_updates.len(),
                rules = output.rule_updates.len(),
                stale = output.stale_facts.len(),
                "Reforge Phase 2 complete"
            );
            Some(output)
        }
        Err(e) => {
            warn!("Reforge Phase 2 failed: {e}");
            result.phase_errors.push(format!("synthesize: {e}"));
            None
        }
    };

    // Persist high-confidence cross-session patterns as episodic memories.
    if let Some(ref syn) = synthesize_output {
        for pattern in &syn.cross_session_patterns {
            if pattern.confidence >= 0.7 {
                let mem = EpisodicMemory {
                    id: uuid::Uuid::new_v4().to_string(),
                    domain: SOURCE_REFORGE.to_string(),
                    content: pattern.pattern.clone(),
                    summary: Some("Cross-session pattern".to_string()),
                    importance: pattern.confidence,
                    occurred_at: Timestamp::now().to_string(),
                    recorded_at: Timestamp::now().to_string(),
                    stability: 3.0,
                    last_accessed: None,
                    access_count: 0,
                    project_id: None,
                    scope_type: "system".to_string(),
                    scope_id: None,
                };
                if let Err(e) = episodic_repo.insert(&mem).await {
                    warn!("Reforge: failed to persist cross-session pattern: {e}");
                } else {
                    result.patterns_persisted += 1;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Phase 3: Review (LLM call #2)
    // ------------------------------------------------------------------
    info!("Reforge Phase 3: Review");
    let review_input = build_review_input(&collected, &synthesize_output);
    let review_output = match handler.review(&review_input).await {
        Ok(output) => {
            debug!(
                skill_edits = output.skill_edits.len(),
                routing_insights = output.routing_insights.len(),
                "Reforge Phase 3 complete"
            );
            Some(output)
        }
        Err(e) => {
            warn!("Reforge Phase 3 failed: {e}");
            result.phase_errors.push(format!("review: {e}"));
            None
        }
    };

    // Persist context priority suggestions for the next cycle's feedback loop.
    if let Some(ref review) = review_output {
        if let Some(repo) = feedback_sources.and_then(|fb| fb.suggestion_repo) {
            let now = Timestamp::now().to_string();
            for suggestion in &review.context_priority_suggestions {
                let row = storage::repos::reforge_suggestion::ReforgeSuggestionRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    suggestion_type: "context_priority".to_string(),
                    content: suggestion.suggestion.clone(),
                    reason: suggestion.reason.clone(),
                    confidence: 0.8,
                    cycle_run_at: now.clone(),
                    acted_upon: false,
                    created_at: now.clone(),
                };
                if let Err(e) = repo.insert(&row).await {
                    warn!("Reforge: failed to persist context priority suggestion: {e}");
                } else {
                    result.suggestions_persisted += 1;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Phase 4: Narrate (LLM call #3)
    // ------------------------------------------------------------------
    info!("Reforge Phase 4: Narrate");
    let narrate_input = build_narrate_input(&synthesize_output, &review_output);
    let narrative = match handler.narrate(&narrate_input).await {
        Ok(text) => {
            debug!(len = text.len(), "Reforge Phase 4 complete");
            text
        }
        Err(e) => {
            warn!("Reforge Phase 4 failed: {e}");
            result.phase_errors.push(format!("narrate: {e}"));
            "Reforge cycle completed with partial results.".to_string()
        }
    };
    result.narrative = narrative.clone();

    // ------------------------------------------------------------------
    // Phase 5: Apply
    // ------------------------------------------------------------------
    info!("Reforge Phase 5: Apply");

    // 5a. Apply knowledge (facts + rules) from Phase 2.
    if let Some(ref syn) = synthesize_output {
        apply_knowledge(syn, fact_repo, rule_repo, &mut result).await;
    }

    // 5b. Apply skill edits from Phase 3.
    if let Some(ref rev) = review_output {
        apply_skill_edits(
            &rev.skill_edits,
            &collected_hashes,
            skill_mgr,
            skill_version_repo,
            &mut result,
        )
        .await;
    }

    // 5c. Store narrative as episodic memory.
    let narrative_mem = EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: SOURCE_REFORGE.to_string(),
        content: narrative,
        summary: Some("Reforge cycle narrative".to_string()),
        importance: 0.9,
        occurred_at: Timestamp::now().to_string(),
        recorded_at: Timestamp::now().to_string(),
        stability: 5.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "system".to_string(),
        scope_id: None,
    };
    if let Err(e) = episodic_repo.insert(&narrative_mem).await {
        warn!("Reforge: failed to store narrative memory: {e}");
        result.phase_errors.push(format!("narrative_store: {e}"));
    }

    // ------------------------------------------------------------------
    // Phase 6: Optimize
    // ------------------------------------------------------------------
    info!("Reforge Phase 6: Optimize");
    if let Some(bridge) = autotuner_bridge {
        // Step 1: Evaluate existing trials
        match bridge.run_evaluation().await {
            Ok(eval) => {
                result.champion_promoted = eval.promoted;
                result.regression_detected = eval.regression;
                if let Some(ref summary) = eval.promotion_summary {
                    info!("Reforge Phase 6: {summary}");
                }
                if eval.regression {
                    warn!("Reforge Phase 6: champion regression detected");
                }
                if !eval.failed_constraints.is_empty() {
                    debug!(
                        "Reforge Phase 6: {} trial(s) failed constraints: {}",
                        eval.failed_constraints.len(),
                        eval.failed_constraints.join("; "),
                    );
                }
                debug!(
                    evaluated = eval.evaluated_count,
                    promoted = eval.promoted,
                    regression = eval.regression,
                    "Reforge Phase 6 evaluation complete"
                );
            }
            Err(e) => {
                warn!("Reforge Phase 6 evaluation failed: {e}");
                result.phase_errors.push(format!("optimize/evaluate: {e}"));
            }
        }

        // Step 2: Create new trials from Phase 3 suggestions
        if let Some(ref review) = review_output {
            let created = create_trials_from_suggestions(&review.trial_suggestions, bridge).await;
            result.trials_created = created;
        }
    } else {
        debug!("Reforge Phase 6: skipped (no autotuner bridge)");
    }

    // ------------------------------------------------------------------
    // Phase 6.5: Graph Consolidation
    // ------------------------------------------------------------------
    if let (Some(enricher), Some(density_repo), Some(entity_repo)) =
        (graph_enrichment_handler, density_repo, entity_repo)
    {
        info!("Reforge Phase 6.5: Graph Consolidation");

        // Step 1: Load medium-density turns queued since last cycle
        let pending_turns = match density_repo.load_pending_medium(50).await {
            Ok(turns) => turns,
            Err(e) => {
                warn!("Reforge Phase 6.5: failed to load pending turns: {e}");
                result
                    .phase_errors
                    .push(format!("graph_consolidation/load: {e}"));
                Vec::new()
            }
        };

        // Step 2: Find duplicate entity candidates
        let dup_candidates = match entity_repo.find_duplicate_candidates(30).await {
            Ok(dups) => dups,
            Err(e) => {
                warn!("Reforge Phase 6.5: failed to find duplicates: {e}");
                Vec::new()
            }
        };

        // Step 3: Run LLM enrichment (single call) if there's work to do
        if !pending_turns.is_empty() || !dup_candidates.is_empty() {
            let input = crate::services::graph_enrichment::GraphEnrichmentInput {
                turn_previews: pending_turns
                    .iter()
                    .map(|t| t.content_preview.clone())
                    .collect(),
                duplicate_candidates: dup_candidates
                    .iter()
                    .map(|(a_id, b_id, a_name, b_name)| {
                        crate::services::graph_enrichment::DuplicateCandidate {
                            entity_a_id: a_id.clone(),
                            entity_b_id: b_id.clone(),
                            entity_a_name: a_name.clone(),
                            entity_b_name: b_name.clone(),
                        }
                    })
                    .collect(),
            };

            match enricher.enrich_graph(&input).await {
                Ok(output) => {
                    // Apply merge decisions
                    for decision in &output.merge_decisions {
                        if decision.should_merge {
                            if let Err(e) = entity_repo
                                .merge_entities(&decision.entity_a_id, &decision.entity_b_id)
                                .await
                            {
                                debug!("Phase 6.5: merge failed: {e}");
                            } else {
                                result.entities_merged += 1;
                            }
                        }
                    }

                    // Apply discovered relationships
                    for rel in &output.discovered_relationships {
                        let source_entities = entity_repo
                            .find_by_name(&rel.source_entity_name)
                            .await
                            .unwrap_or_default();
                        let target_entities = entity_repo
                            .find_by_name(&rel.target_entity_name)
                            .await
                            .unwrap_or_default();

                        if let (Some(src), Some(tgt)) =
                            (source_entities.first(), target_entities.first())
                        {
                            let new_rel = crate::repos::entity::NewRelationship {
                                source_entity_id: src.id.clone(),
                                target_entity_id: tgt.id.clone(),
                                relationship_type: rel.relationship_type.clone(),
                                evidence: None,
                                source: "reforge_phase_6.5".to_string(),
                            };
                            if entity_repo.upsert_relationship(&new_rel).await.is_ok() {
                                result.relationships_discovered += 1;
                            }
                        }
                    }

                    // Mark processed turns as enriched
                    let turn_ids: Vec<String> =
                        pending_turns.iter().map(|t| t.id.clone()).collect();
                    if let Err(e) = density_repo.mark_enriched(&turn_ids).await {
                        debug!("Phase 6.5: mark_enriched failed: {e}");
                    }

                    info!(
                        merged = result.entities_merged,
                        relationships = result.relationships_discovered,
                        turns_processed = pending_turns.len(),
                        "Reforge Phase 6.5 complete"
                    );
                }
                Err(e) => {
                    warn!("Reforge Phase 6.5 enrichment failed: {e}");
                    result
                        .phase_errors
                        .push(format!("graph_consolidation/enrich: {e}"));
                }
            }
        } else {
            debug!("Reforge Phase 6.5: nothing to consolidate");
        }

        // Step 4: Record knowledge snapshot
        if let Some(snapshot_repo) = snapshot_repo {
            if let Err(e) = record_knowledge_snapshot(entity_repo, fact_repo, snapshot_repo).await {
                debug!("Phase 6.5: snapshot failed: {e}");
            } else {
                result.snapshot_recorded = true;
            }
        }
    } else {
        debug!("Reforge Phase 6.5: skipped (missing repos or handler)");
    }

    // ------------------------------------------------------------------
    // Phase 6.5b: Community Intelligence — LLM naming, merge, split
    // ------------------------------------------------------------------
    if let (Some(ci_handler), Some(community_repo)) =
        (community_intelligence_handler, community_repo)
    {
        match crate::services::community_intelligence::build_intelligence_input(community_repo)
            .await
        {
            Ok(input) if !input.communities.is_empty() => {
                match ci_handler.analyze_communities(&input).await {
                    Ok(output) => {
                        let (renamed, merged, split_count) = if let Some(co_act) =
                            co_activation_repo_for_split
                        {
                            crate::services::community_intelligence::apply_intelligence(
                                &output,
                                community_repo,
                                co_act,
                            )
                            .await
                        } else {
                            // No co-activation repo — can rename/merge but not split
                            let no_splits =
                                    crate::services::community_intelligence::CommunityIntelligenceOutput {
                                        names: output.names.clone(),
                                        merges: output.merges.clone(),
                                        splits: Vec::new(),
                                    };
                            let fallback_co_act =
                                crate::repos::CoActivationRepo::new(community_repo.pool().clone());
                            crate::services::community_intelligence::apply_intelligence(
                                &no_splits,
                                community_repo,
                                &fallback_co_act,
                            )
                            .await
                        };
                        result.communities_renamed = renamed;
                        result.communities_merged = merged;
                        result.communities_split = split_count;
                        info!(
                            renamed,
                            merged,
                            split = split_count,
                            "Phase 6.5b: community intelligence complete"
                        );
                    }
                    Err(e) => {
                        warn!("Phase 6.5b community intelligence failed: {e}");
                        result
                            .phase_errors
                            .push(format!("community_intelligence: {e}"));
                    }
                }
            }
            Ok(_) => {
                debug!("Phase 6.5b: no active communities for intelligence");
            }
            Err(e) => {
                debug!("Phase 6.5b: failed to build community input: {e}");
            }
        }
    }

    // ------------------------------------------------------------------
    // Phase 7: Compact
    // ------------------------------------------------------------------
    info!("Reforge Phase 7: Compact");
    let mut compaction_stats: Option<serde_json::Value> = None;
    match crate::services::compaction::run_compaction(
        fact_repo,
        episodic_repo,
        Some(rule_repo),
        None,
        None,
        Some(session_memory_repo),
        None,
        None,
    )
    .await
    {
        Ok(cr) => {
            debug!(
                facts_archived = cr.facts_archived,
                episodic_deleted = cr.episodic_deleted,
                rules_deactivated = cr.rules_deactivated,
                "Reforge Phase 7 complete"
            );
            compaction_stats = Some(serde_json::json!({
                "facts_archived": cr.facts_archived,
                "episodic_deleted": cr.episodic_deleted,
                "low_stability_archived": cr.low_stability_archived,
                "rules_deactivated": cr.rules_deactivated,
            }));
        }
        Err(e) => {
            warn!("Reforge Phase 7 failed: {e}");
            result.phase_errors.push(format!("compact: {e}"));
        }
    }

    // Record run in reforge_state (after all phases, including compaction).
    let stats_json = serde_json::json!({
        "facts_added": result.facts_added,
        "facts_updated": result.facts_updated,
        "facts_stale_flagged": result.facts_stale_flagged,
        "rules_added": result.rules_added,
        "rules_reinforced": result.rules_reinforced,
        "skills_edited": result.skills_edited,
        "skipped_skill_edits": result.skipped_skill_edits.len(),
        "phase_errors": result.phase_errors.len(),
        "trials_created": result.trials_created,
        "champion_promoted": result.champion_promoted,
        "regression_detected": result.regression_detected,
        "suggestions_persisted": result.suggestions_persisted,
        "patterns_persisted": result.patterns_persisted,
        "communities_renamed": result.communities_renamed,
        "communities_merged": result.communities_merged,
        "communities_split": result.communities_split,
        "compaction": compaction_stats,
    });
    if let Err(e) = reforge_state_repo.record_run(&stats_json.to_string()).await {
        warn!("Reforge: failed to record run: {e}");
    }

    info!(
        facts_added = result.facts_added,
        facts_updated = result.facts_updated,
        rules_added = result.rules_added,
        skills_edited = result.skills_edited,
        errors = result.phase_errors.len(),
        "Reforge cycle complete"
    );

    Some(result)
}

// ---------------------------------------------------------------------------
// Phase input builders
// ---------------------------------------------------------------------------

fn build_synthesize_input(collected: &ReforgeCollected) -> SynthesizeInput {
    let sessions = collected.sessions.clone();

    let episodic_memories: Vec<EpisodicSummary> = collected
        .episodic_memories
        .iter()
        .map(|m| {
            let summary = match m.summary.as_deref() {
                Some(s) => s.to_string(),
                None => truncate_chars(&m.content, 200, ""),
            };
            EpisodicSummary {
                domain: m.domain.clone(),
                summary,
                occurred_at: m.occurred_at.clone(),
            }
        })
        .collect();

    let user_model_summary = format_user_model(&collected.user_model);
    let rules_summary = format_rules(&collected.rules);

    SynthesizeInput {
        sessions,
        episodic_memories,
        user_model_summary,
        rules_summary,
        retrieval_precision: collected.retrieval_precision,
    }
}

fn build_review_input(
    collected: &ReforgeCollected,
    synthesize_output: &Option<SynthesizeOutput>,
) -> ReviewInput {
    let skill_contents: Vec<SkillContent> = collected
        .skill_files
        .iter()
        .flat_map(|(_, files)| {
            files.iter().map(|f| SkillContent {
                skill_name: f.skill_name.clone(),
                file_path: f.file_path.clone(),
                content: f.content.clone(),
            })
        })
        .collect();

    let new_facts_summary = match synthesize_output {
        Some(syn) => {
            let parts: Vec<String> = syn
                .fact_updates
                .iter()
                .map(|f| {
                    format!(
                        "{} {} {}: {} ({})",
                        f.action, f.subject, f.predicate, f.object, f.domain
                    )
                })
                .collect();
            if parts.is_empty() {
                "No new facts extracted.".to_string()
            } else {
                parts.join("; ")
            }
        }
        None => "Phase 2 did not produce results.".to_string(),
    };

    // Format autotuner context as a string for the prompt.
    let autotuner_context = collected
        .autotuner_ctx
        .as_ref()
        .map(format_autotuner_context);

    // Format feedback strings for the Review prompt
    let tool_failure_summary = if !collected.tool_failures.is_empty() {
        Some(
            collected
                .tool_failures
                .iter()
                .map(|f| {
                    format!(
                        "- {} — {}/{} calls failed ({:.0}%) — errors: {}",
                        f.tool_name,
                        f.failure_count,
                        f.total_calls,
                        f.failure_rate * 100.0,
                        f.error_types.join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    let correction_summary = if !collected.correction_summaries.is_empty() {
        Some(
            collected
                .correction_summaries
                .iter()
                .map(|c| {
                    let samples = c
                        .sample_corrections
                        .iter()
                        .map(|s| format!("    \"{s}\""))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "- {} skill: {} corrections\n{samples}",
                        c.skill_name, c.correction_count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    let previous_suggestions_summary = if !collected.previous_suggestions.is_empty() {
        Some(
            collected
                .previous_suggestions
                .iter()
                .map(|s| {
                    format!(
                        "- [{}] {}: {} (confidence: {:.2})",
                        s.suggestion_type, s.content, s.reason, s.confidence
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    ReviewInput {
        pending_meta_rules: collected.pending_meta_rules.clone(),
        routing_summaries: collected.routing_summaries.clone(),
        skill_contents,
        new_facts_summary,
        retrieval_precision: collected.retrieval_precision,
        autotuner_context,
        tool_failure_summary,
        correction_summary,
        previous_suggestions_summary,
    }
}

fn build_narrate_input(
    synthesize_output: &Option<SynthesizeOutput>,
    review_output: &Option<ReviewOutput>,
) -> NarrateInput {
    let synthesize_summary = match synthesize_output {
        Some(syn) => format!(
            "{} fact updates, {} rule updates, {} stale facts flagged",
            syn.fact_updates.len(),
            syn.rule_updates.len(),
            syn.stale_facts.len()
        ),
        None => "Synthesis phase did not run.".to_string(),
    };

    let review_summary = match review_output {
        Some(rev) => format!(
            "{} skill edits proposed, {} routing insights",
            rev.skill_edits.len(),
            rev.routing_insights.len()
        ),
        None => "Review phase did not run.".to_string(),
    };

    let routing_summary = match review_output {
        Some(rev) => rev.routing_insights.join("; "),
        None => String::new(),
    };

    NarrateInput {
        synthesize_summary,
        review_summary,
        routing_summary,
    }
}

// ---------------------------------------------------------------------------
// Phase 5a: Apply knowledge
// ---------------------------------------------------------------------------

async fn apply_knowledge(
    syn: &SynthesizeOutput,
    fact_repo: &SemanticFactRepo,
    rule_repo: &ProceduralRuleRepo,
    result: &mut ReforgeResult,
) {
    let now = Timestamp::now().to_string();

    // --- Fact updates ---
    for fu in &syn.fact_updates {
        match fu.action {
            FactAction::Add | FactAction::Update => {
                upsert_fact(fu, fact_repo, &now, result).await;
            }
            FactAction::Remove => {
                let existing = fact_repo
                    .find_similar(&fu.subject, &fu.predicate)
                    .await
                    .ok()
                    .and_then(|v| v.into_iter().next());
                if let Some(old) = existing {
                    if let Err(e) = fact_repo.supersede(&old.id, "removed-by-reforge").await {
                        warn!("Reforge: failed to remove fact {}: {e}", old.id);
                    } else {
                        result.facts_updated += 1;
                    }
                }
            }
        }
    }

    // --- Rule updates ---
    for ru in &syn.rule_updates {
        match ru.action {
            RuleAction::Add => {
                let existing = rule_repo
                    .find_similar(&ru.rule_text, &ru.domain)
                    .await
                    .ok()
                    .flatten();
                if let Some(existing_rule) = existing {
                    // Already exists — just reinforce.
                    if let Err(e) = rule_repo.increment_signal_count(&existing_rule.id).await {
                        warn!("Reforge: failed to reinforce existing rule: {e}");
                    } else {
                        result.rules_reinforced += 1;
                    }
                } else {
                    let rule = ProceduralRule {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: ru.domain.clone(),
                        rule_text: ru.rule_text.clone(),
                        confidence: 0.6,
                        source: SOURCE_REFORGE.to_string(),
                        signal_count: 1,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                        active: true,
                        project_id: None,
                        scope_type: "system".to_string(),
                        scope_id: None,
                    };
                    if let Err(e) = rule_repo.upsert(&rule).await {
                        warn!("Reforge: failed to add rule: {e}");
                    } else {
                        result.rules_added += 1;
                    }
                }
            }
            RuleAction::Update | RuleAction::Reinforce => {
                let existing = rule_repo
                    .find_similar(&ru.rule_text, &ru.domain)
                    .await
                    .ok()
                    .flatten();
                if let Some(existing_rule) = existing {
                    if let Err(e) = rule_repo.increment_signal_count(&existing_rule.id).await {
                        warn!("Reforge: failed to reinforce rule: {e}");
                    } else {
                        result.rules_reinforced += 1;
                    }
                } else {
                    debug!(
                        "Reforge: reinforce target not found for rule in domain '{}'",
                        ru.domain
                    );
                }
            }
        }
    }

    // --- Stale facts: reduce confidence by 50% ---
    for sf in &syn.stale_facts {
        match fact_repo.get(&sf.fact_id).await {
            Ok(Some(fact)) => {
                let new_confidence = (fact.confidence * 0.5).max(0.1);
                if let Err(e) = fact_repo.update_confidence(&fact.id, new_confidence).await {
                    warn!(
                        "Reforge: failed to reduce confidence for {}: {e}",
                        sf.fact_id
                    );
                } else {
                    result.facts_stale_flagged += 1;
                }
            }
            Ok(None) => {
                debug!("Reforge: stale fact {} not found, skipping", sf.fact_id);
            }
            Err(e) => {
                warn!("Reforge: failed to fetch stale fact {}: {e}", sf.fact_id);
            }
        }
    }
}

/// Shared logic for fact add/update: find similar, supersede if exists, create new.
async fn upsert_fact(
    fu: &FactUpdate,
    fact_repo: &SemanticFactRepo,
    now: &str,
    result: &mut ReforgeResult,
) {
    let existing = fact_repo
        .find_similar(&fu.subject, &fu.predicate)
        .await
        .ok()
        .and_then(|v| v.into_iter().next());
    let new_id = uuid::Uuid::new_v4().to_string();
    let had_existing = existing.is_some();
    if let Some(old) = existing {
        if let Err(e) = fact_repo.supersede(&old.id, &new_id).await {
            warn!("Reforge: failed to supersede fact {}: {e}", old.id);
        }
    }
    let fact = new_semantic_fact(&new_id, fu, now);
    if let Err(e) = fact_repo.upsert(&fact).await {
        warn!("Reforge: failed to upsert fact: {e}");
    } else if had_existing {
        result.facts_updated += 1;
    } else {
        result.facts_added += 1;
    }
}

// ---------------------------------------------------------------------------
// Phase 5b: Apply skill edits
// ---------------------------------------------------------------------------

async fn apply_skill_edits(
    edits: &[SkillEdit],
    collected_hashes: &HashMap<(String, String), String>,
    skill_mgr: &SkillFileManager,
    skill_version_repo: &storage::repos::SkillVersionRepo,
    result: &mut ReforgeResult,
) {
    // Read all skill files once upfront instead of per-edit.
    let mut cached_files: HashMap<String, Vec<SkillFile>> = skill_mgr.read_all();

    for edit in edits {
        let key = (edit.skill_name.clone(), edit.file_path.clone());

        let current_content = match cached_files
            .get(&edit.skill_name)
            .and_then(|files| files.iter().find(|f| f.file_path == edit.file_path))
        {
            Some(f) => f.content.clone(),
            None => {
                warn!(
                    "Reforge: skill file {}/{} not found on disk, skipping edit",
                    edit.skill_name, edit.file_path
                );
                result.skipped_skill_edits.push(format!(
                    "{}/{}: file not found",
                    edit.skill_name, edit.file_path
                ));
                continue;
            }
        };

        // Conflict detection: compare current hash against collection-time hash.
        let current_hash = content_hash(&current_content);
        if let Some(collected_hash) = collected_hashes.get(&key) {
            if &current_hash != collected_hash {
                info!(
                    "Reforge: skill file {}/{} modified since collection, skipping edit",
                    edit.skill_name, edit.file_path
                );
                result.skipped_skill_edits.push(format!(
                    "{}/{}: modified since collection",
                    edit.skill_name, edit.file_path
                ));
                continue;
            }
        }

        // Apply the edit.
        let new_content = match apply_single_edit(&current_content, edit) {
            Some(c) => c,
            None => {
                warn!(
                    "Reforge: edit failed for {}/{}: {}",
                    edit.skill_name, edit.file_path, edit.reason
                );
                result.skipped_skill_edits.push(format!(
                    "{}/{}: edit could not be applied",
                    edit.skill_name, edit.file_path
                ));
                continue;
            }
        };

        // Write to disk.
        if let Err(e) = skill_mgr.write_file(&edit.skill_name, &edit.file_path, &new_content) {
            warn!(
                "Reforge: failed to write {}/{}: {e}",
                edit.skill_name, edit.file_path
            );
            result.skipped_skill_edits.push(format!(
                "{}/{}: write failed: {e}",
                edit.skill_name, edit.file_path
            ));
            continue;
        }

        // Update the in-memory cache so subsequent edits to the same file see
        // the new content without re-reading from disk.
        if let Some(files) = cached_files.get_mut(&edit.skill_name) {
            if let Some(cached) = files.iter_mut().find(|f| f.file_path == edit.file_path) {
                cached.content_hash = content_hash(&new_content);
                cached.content = new_content.clone();
            }
        }

        // Record version in DB.
        let diff = compute_diff(&current_content, &new_content);
        let next_version = match skill_version_repo
            .latest_version(&edit.skill_name, &edit.file_path)
            .await
        {
            Ok(Some(latest)) => latest.version + 1,
            _ => 1,
        };

        let version_row = storage::rows::skill_version::SkillVersionRow {
            id: uuid::Uuid::new_v4().to_string(),
            skill_name: edit.skill_name.clone(),
            version: next_version,
            file_path: edit.file_path.clone(),
            content: new_content,
            diff: Some(diff),
            source: SOURCE_REFORGE.to_string(),
            reason: Some(edit.reason.clone()),
            created_at: Timestamp::now().to_string(),
        };

        if let Err(e) = skill_version_repo.insert(&version_row).await {
            warn!(
                "Reforge: failed to record version for {}/{}: {e}",
                edit.skill_name, edit.file_path
            );
        }

        result.skills_edited += 1;
    }
}

/// Apply a single `SkillEdit` to file content, returning the modified content
/// or `None` if the edit cannot be applied.
pub(crate) fn apply_single_edit(content: &str, edit: &SkillEdit) -> Option<String> {
    match edit.edit_type {
        SkillEditType::Frontmatter => {
            let field = edit.field.as_deref()?;
            let new_value = edit.new_value.as_deref()?;
            apply_frontmatter_edit(content, field, new_value)
        }
        SkillEditType::BodyReplace => {
            let old_text = edit.old_text.as_deref()?;
            let new_text = edit.new_text.as_deref()?;
            if !content.contains(old_text) {
                return None;
            }
            Some(content.replacen(old_text, new_text, 1))
        }
        SkillEditType::BodyInsert => {
            let section = edit.section.as_deref()?;
            let new_text = edit.new_text.as_deref()?;
            Some(apply_body_insert(content, section, new_text))
        }
        SkillEditType::BodyRemove => {
            let old_text = edit.old_text.as_deref()?;
            if !content.contains(old_text) {
                return None;
            }
            Some(content.replacen(old_text, "", 1))
        }
    }
}

// ---------------------------------------------------------------------------
// Edit helpers
// ---------------------------------------------------------------------------

/// Replace or insert a YAML frontmatter field.
///
/// Frontmatter is delimited by `---` on lines 1 and N. If the field already
/// exists, its value is replaced. Otherwise a new `field: value` line is
/// appended before the closing `---`.
fn apply_frontmatter_edit(content: &str, field: &str, new_value: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || lines[0].trim() != "---" {
        // No frontmatter block — cannot apply.
        return None;
    }

    // Find the closing `---`.
    let closing_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim() == "---")
        .map(|(i, _)| i)?;

    let mut new_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut replaced = false;

    for (i, line) in lines.iter().enumerate() {
        if i > 0 && i < closing_idx {
            // Inside frontmatter — check if this line is the target field.
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim();
                if key == field {
                    new_lines.push(format!("{field}: {new_value}"));
                    replaced = true;
                    continue;
                }
            }
        }
        if i == closing_idx && !replaced {
            // Insert the new field before the closing delimiter.
            new_lines.push(format!("{field}: {new_value}"));
        }
        new_lines.push(line.to_string());
    }

    Some(new_lines.join("\n"))
}

/// Insert `new_text` after the first occurrence of a section header matching
/// `section`. Falls back to appending at the end of the document.
fn apply_body_insert(content: &str, section: &str, new_text: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Find a markdown header line that contains the section text.
    let header_idx = lines.iter().enumerate().find(|(_, line)| {
        let trimmed = line.trim();
        trimmed.starts_with('#') && trimmed.contains(section)
    });

    match header_idx {
        Some((idx, _)) => {
            let mut result = Vec::with_capacity(lines.len() + 2);
            result.extend(lines[..=idx].iter().map(|s| s.to_string()));
            result.push(new_text.to_string());
            if idx + 1 < lines.len() {
                result.extend(lines[idx + 1..].iter().map(|s| s.to_string()));
            }
            result.join("\n")
        }
        None => {
            // Fall back to appending.
            format!("{content}\n{new_text}")
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn new_semantic_fact(id: &str, fu: &FactUpdate, now: &str) -> SemanticFact {
    SemanticFact {
        id: id.to_string(),
        domain: fu.domain.clone(),
        subject: fu.subject.clone(),
        predicate: fu.predicate.clone(),
        object: fu.object.clone(),
        confidence: fu.confidence,
        source: SOURCE_REFORGE.to_string(),
        valid_from: now.to_string(),
        valid_until: None,
        recorded_at: now.to_string(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        project_id: None,
        memory_type: DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    }
}

fn format_autotuner_context(ctx: &AutotunerContext) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(&mut out, "### Current Champion Parameters").unwrap();
    // champion_summary contains category headers even for defaults, so check
    // if any actual key=value pair is present (contains '=').
    if ctx.champion_summary.contains('=') {
        writeln!(&mut out, "{}", ctx.champion_summary).unwrap();
    } else {
        writeln!(&mut out, "(default parameters — no champion promoted yet)").unwrap();
    }

    writeln!(&mut out, "\n### Performance Metrics").unwrap();
    let m24 = &ctx.metrics_24h;
    let m7d = &ctx.metrics_7d;

    // Helper: compute trend arrow comparing 24h to 7d average.
    // For correction_rate and response_time, higher is worse (↑ = worsening).
    let trend = |current: f64, baseline: f64, higher_is_worse: bool| -> &'static str {
        let delta = current - baseline;
        if delta.abs() < 0.005 {
            "→"
        } else if (delta > 0.0) == higher_is_worse {
            "↑ worsening"
        } else {
            "↓ improving"
        }
    };

    writeln!(
        &mut out,
        "Metrics (last 24h vs 7-day avg):\n\
         - correction_rate: {:.3} (7d: {:.3}) {}\n\
         - retrieval_precision: {:.3} (7d: {:.3}) {}\n\
         - avg_response_time: {:.0}ms (7d: {:.0}ms) {}\n\
         - avg_tokens: {:.0} (7d: {:.0}) {}\n\
         - routing_stability: {:.3} (7d: {:.3}) {}\n\
         - memory_relevance: {:.3} (7d: {:.3}) {}",
        m24.correction_rate,
        m7d.correction_rate,
        trend(m24.correction_rate, m7d.correction_rate, true),
        m24.retrieval_precision,
        m7d.retrieval_precision,
        trend(m24.retrieval_precision, m7d.retrieval_precision, false),
        m24.avg_response_time_ms,
        m7d.avg_response_time_ms,
        trend(m24.avg_response_time_ms, m7d.avg_response_time_ms, true),
        m24.avg_tokens_per_message,
        m7d.avg_tokens_per_message,
        trend(m24.avg_tokens_per_message, m7d.avg_tokens_per_message, true),
        m24.routing_stability,
        m7d.routing_stability,
        trend(m24.routing_stability, m7d.routing_stability, false),
        m24.memory_relevance,
        m7d.memory_relevance,
        trend(m24.memory_relevance, m7d.memory_relevance, false),
    )
    .unwrap();

    if !ctx.trial_history.is_empty() {
        writeln!(&mut out, "\n### Recent Experiment History").unwrap();
        for entry in &ctx.trial_history {
            writeln!(
                &mut out,
                "Experiment {} ({} days ago):",
                entry.experiment_id, entry.days_ago
            )
            .unwrap();
            for trial in &entry.trials {
                let improvement_str = trial
                    .improvement
                    .map(|v| format!(" ({:+.1}%)", v * 100.0))
                    .unwrap_or_default();
                write!(
                    &mut out,
                    "  {} → {}{}",
                    trial.params_summary, trial.result, improvement_str
                )
                .unwrap();
                if !trial.constraint_failures.is_empty() {
                    write!(
                        &mut out,
                        " [failed: {}]",
                        trial.constraint_failures.join(", ")
                    )
                    .unwrap();
                }
                writeln!(&mut out).unwrap();
            }
        }
    }

    writeln!(
        &mut out,
        "\n### Active Trials: {}/6 (cap)",
        ctx.active_trial_count
    )
    .unwrap();

    out
}

fn format_user_model(model: &crate::types::UserModel) -> String {
    let mut parts = Vec::new();
    let domains = [
        ("identity", &model.identity),
        ("energy", &model.energy),
        ("work", &model.work),
        ("finance", &model.finance),
        ("learning", &model.learning),
        ("preferences", &model.preferences),
    ];
    for (name, facts) in domains {
        if !facts.is_empty() {
            let fact_strs: Vec<String> = facts
                .iter()
                .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object))
                .collect();
            parts.push(format!("{name}: {}", fact_strs.join("; ")));
        }
    }
    if !model.other.is_empty() {
        let fact_strs: Vec<String> = model
            .other
            .iter()
            .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object))
            .collect();
        parts.push(format!("other: {}", fact_strs.join("; ")));
    }
    if parts.is_empty() {
        "No user model facts yet.".to_string()
    } else {
        parts.join("\n")
    }
}

fn format_rules(rules: &[ProceduralRule]) -> String {
    if rules.is_empty() {
        return "No active rules.".to_string();
    }
    rules
        .iter()
        .map(|r| {
            format!(
                "[{}] {} (signals: {})",
                r.domain, r.rule_text, r.signal_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Phase 6: Trial creation from LLM suggestions
// ---------------------------------------------------------------------------

/// Validate, deduplicate, and create trials from LLM suggestions.
async fn create_trials_from_suggestions(
    suggestions: &[TrialSuggestion],
    bridge: &dyn super::AutotunerBridge,
) -> u32 {
    if suggestions.is_empty() {
        return 0;
    }

    // Expire stale trials (>7 days, <20 messages) before checking the cap.
    bridge.expire_stale_trials().await;

    // Guardrail: active trial cap (max 6) — partial: only create as many as slots allow.
    let active = bridge.active_trial_count().await;
    if active >= 6 {
        info!("Reforge Phase 6: skipping trial creation — {active} active trials (cap: 6)");
        return 0;
    }
    let slots_available = (6 - active) as usize;

    let champion_map = bridge.champion_params_map();

    // Validate each suggestion's params
    let mut validated: Vec<super::ValidatedTrial> = Vec::new();
    for suggestion in suggestions {
        if let Some(params) = validate_param_overrides(&suggestion.param_overrides) {
            validated.push(super::ValidatedTrial {
                hypothesis: suggestion.hypothesis.clone(),
                pace: suggestion.pace.clone(),
                params,
            });
        }
    }

    if validated.is_empty() {
        warn!("Reforge Phase 6: all trial suggestions rejected by param validation");
        return 0;
    }

    // Diversity gate: iteratively prune non-diverse trials instead of rejecting the whole batch.
    validated = prune_for_diversity(validated, &champion_map);

    if validated.is_empty() {
        warn!("Reforge Phase 6: all trials pruned by diversity gate");
        return 0;
    }

    // Respect slot cap.
    validated.truncate(slots_available);

    // Create trials via bridge
    match bridge.create_trials(validated).await {
        Ok(count) => {
            info!("Reforge Phase 6: created {count} new trial(s)");
            count
        }
        Err(e) => {
            warn!("Reforge Phase 6: trial creation failed: {e}");
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 6.5 helpers
// ---------------------------------------------------------------------------

/// Record a nightly knowledge graph snapshot.
async fn record_knowledge_snapshot(
    entity_repo: &crate::repos::EntityRepo,
    fact_repo: &SemanticFactRepo,
    snapshot_repo: &crate::repos::KnowledgeSnapshotRepo,
) -> common::Result<()> {
    let facts = fact_repo.count_active().await.unwrap_or(0) as u32;

    let entity_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities")
        .fetch_one(entity_repo.pool())
        .await
        .unwrap_or((0,));
    let rel_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entity_relationships")
        .fetch_one(entity_repo.pool())
        .await
        .unwrap_or((0,));

    let domain_counts = fact_repo.count_by_domain().await.unwrap_or_default();
    let domain_json = serde_json::to_string(
        &domain_counts
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
    )
    .ok();

    let orphan_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         WHERE NOT EXISTS (
             SELECT 1 FROM entity_relationships r
             WHERE r.source_entity_id = e.id OR r.target_entity_id = e.id
         )",
    )
    .fetch_one(entity_repo.pool())
    .await
    .unwrap_or((0,));

    let ec = entity_count.0 as u32;
    let rc = rel_count.0 as u32;
    let orphan_rate = if ec > 0 {
        orphan_count.0 as f64 / ec as f64
    } else {
        0.0
    };
    let avg_degree = if ec > 0 {
        rc as f64 * 2.0 / ec as f64
    } else {
        0.0
    };

    let metrics = serde_json::json!({
        "orphan_rate": orphan_rate,
        "avg_degree": avg_degree,
        "orphan_count": orphan_count.0,
    });

    snapshot_repo
        .insert(
            facts,
            ec,
            rc,
            domain_json.as_deref(),
            None,
            Some(&metrics.to_string()),
        )
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 6 guardrails
// ---------------------------------------------------------------------------

/// Validate and clamp trial param overrides against defined ranges.
/// Returns None if more than half the overrides are invalid.
fn validate_param_overrides(overrides: &HashMap<String, f64>) -> Option<HashMap<String, f64>> {
    let ranges: HashMap<&str, (f64, f64)> = HashMap::from([
        ("skill_keyword_weight", (0.0, 1.0)),
        ("skill_semantic_weight", (0.0, 1.0)),
        ("skill_activation_threshold", (0.40, 0.95)),
        ("heuristic_confidence_threshold", (0.50, 0.95)),
        ("llm_classifier_timeout_ms", (500.0, 5000.0)),
        ("relevance_weight_semantic", (0.10, 0.60)),
        ("relevance_weight_retrievability", (0.05, 0.40)),
        ("relevance_weight_situation", (0.05, 0.40)),
        ("relevance_weight_importance", (0.05, 0.40)),
        ("relevance_weight_frequency", (0.02, 0.30)),
        ("relevance_weight_temporal", (0.01, 0.20)),
        ("relevance_weight_hierarchy", (0.0, 0.25)),
        ("relevance_weight_path_coherence", (0.0, 0.20)),
        ("relevance_weight_community", (0.0, 0.30)),
        ("relevance_weight_cross_note", (0.0, 0.20)),
        ("fsrs_desired_retention", (0.70, 0.99)),
        ("accumulate_promote_threshold", (2.0, 15.0)),
        ("accumulate_min_days", (1.0, 10.0)),
        ("vector_top_k", (10.0, 100.0)),
        ("min_similarity", (0.30, 0.80)),
        ("rewrite_confidence_threshold", (0.30, 0.95)),
        ("rewrite_max_signals", (1.0, 6.0)),
        ("rewrite_min_enrichment_length", (5.0, 30.0)),
        ("tree_top_k", (5.0, 30.0)),
        ("tree_min_similarity", (0.30, 0.70)),
        ("hybrid_bias", (0.0, 1.0)),
        ("community_top_k", (3.0, 15.0)),
        ("community_min_similarity", (0.30, 0.70)),
    ]);

    let mut validated = HashMap::new();
    let mut unknown_count = 0;

    for (key, &value) in overrides {
        if let Some(&(min, max)) = ranges.get(key.as_str()) {
            let clamped = value.clamp(min, max);
            if (clamped - value).abs() > f64::EPSILON {
                warn!("Reforge: clamped param {key} from {value} to {clamped}");
            }
            validated.insert(key.clone(), clamped);
        } else {
            warn!("Reforge: unknown param {key}, skipping");
            unknown_count += 1;
        }
    }

    if !overrides.is_empty() && unknown_count * 2 > overrides.len() {
        warn!(
            "Reforge: rejecting trial — >50% params unknown ({unknown_count}/{})",
            overrides.len()
        );
        return None;
    }

    Some(validated)
}

/// Key params used for diversity distance calculation.
const DIVERSITY_KEYS: &[&str] = &[
    "skill_keyword_weight",
    "skill_semantic_weight",
    "skill_activation_threshold",
    "relevance_weight_semantic",
    "relevance_weight_retrievability",
    "relevance_weight_situation",
    "fsrs_desired_retention",
    "vector_top_k",
    "min_similarity",
];

fn diversity_vec(params: &HashMap<String, f64>) -> Vec<f64> {
    DIVERSITY_KEYS
        .iter()
        .map(|k| *params.get(*k).unwrap_or(&0.0))
        .collect()
}

fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn normalized_distance(a: &[f64], b: &[f64]) -> f64 {
    let max_distance = (DIVERSITY_KEYS.len() as f64).sqrt();
    euclidean_distance(a, b) / max_distance
}

/// Iteratively prune trials that are too similar to each other or all too
/// close to the champion. Returns the surviving subset (may be empty).
fn prune_for_diversity(
    mut trials: Vec<super::ValidatedTrial>,
    champion_params: &HashMap<String, f64>,
) -> Vec<super::ValidatedTrial> {
    let champion_vec = diversity_vec(champion_params);

    // Remove trials that are too close to champion (< 0.10 normalized distance).
    // Keep at least those that are far enough from champion.
    trials.retain(|t| {
        let dist = normalized_distance(&diversity_vec(&t.params), &champion_vec);
        if dist < 0.10 {
            debug!(
                "Reforge: pruning trial '{}' — too close to champion (distance {dist:.3})",
                t.hypothesis
            );
            false
        } else {
            true
        }
    });

    if trials.len() <= 1 {
        return trials;
    }

    // Remove pairwise duplicates: greedily keep trials, skip if too close to
    // any already-kept trial.
    let mut kept: Vec<super::ValidatedTrial> = Vec::with_capacity(trials.len());
    for trial in trials {
        let trial_vec = diversity_vec(&trial.params);
        let too_close = kept.iter().any(|existing| {
            normalized_distance(&diversity_vec(&existing.params), &trial_vec) < 0.05
        });
        if too_close {
            debug!(
                "Reforge: pruning trial '{}' — too similar to another suggestion",
                trial.hypothesis
            );
        } else {
            kept.push(trial);
        }
    }

    kept
}

/// Check that trial suggestions are sufficiently diverse from each other and
/// from the champion. Returns true if diversity is sufficient.
#[allow(dead_code)]
fn check_diversity(
    suggestions: &[&HashMap<String, f64>],
    champion_params: &HashMap<String, f64>,
) -> bool {
    let champion_vec = diversity_vec(champion_params);

    // Check each pair of suggestions
    let vecs: Vec<Vec<f64>> = suggestions.iter().map(|s| diversity_vec(s)).collect();
    for i in 0..vecs.len() {
        for j in (i + 1)..vecs.len() {
            let dist = normalized_distance(&vecs[i], &vecs[j]);
            if dist < 0.05 {
                warn!("Reforge: trial pair {i}/{j} too similar (distance {dist:.3})");
                return false;
            }
        }
    }

    // Check all against champion
    let all_close = vecs
        .iter()
        .all(|v| normalized_distance(v, &champion_vec) < 0.10);
    if all_close {
        warn!("Reforge: all trials too close to champion");
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edit(edit_type: SkillEditType) -> SkillEdit {
        SkillEdit {
            skill_name: "test".to_string(),
            file_path: "SKILL.md".to_string(),
            edit_type,
            field: None,
            new_value: None,
            old_text: None,
            new_text: None,
            section: None,
            reason: "test edit".to_string(),
        }
    }

    #[test]
    fn test_apply_frontmatter_replace() {
        let content = "---\nname: old-name\nversion: 1\n---\n# Body\nHello";
        let mut edit = make_edit(SkillEditType::Frontmatter);
        edit.field = Some("name".to_string());
        edit.new_value = Some("new-name".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("name: new-name"));
        assert!(!result.contains("name: old-name"));
        // Other fields preserved.
        assert!(result.contains("version: 1"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn test_apply_frontmatter_insert_new_field() {
        let content = "---\nname: test\n---\n# Body";
        let mut edit = make_edit(SkillEditType::Frontmatter);
        edit.field = Some("priority".to_string());
        edit.new_value = Some("high".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("priority: high"));
        assert!(result.contains("name: test"));
    }

    #[test]
    fn test_apply_body_replace() {
        let content = "---\nname: test\n---\n# Body\nold text here\nmore content";
        let mut edit = make_edit(SkillEditType::BodyReplace);
        edit.old_text = Some("old text here".to_string());
        edit.new_text = Some("new text here".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("new text here"));
        assert!(!result.contains("old text here"));
        assert!(result.contains("more content"));
    }

    #[test]
    fn test_apply_body_replace_not_found() {
        let content = "---\nname: test\n---\n# Body\nsome content";
        let mut edit = make_edit(SkillEditType::BodyReplace);
        edit.old_text = Some("nonexistent text".to_string());
        edit.new_text = Some("replacement".to_string());

        let result = apply_single_edit(content, &edit);
        assert!(result.is_none());
    }

    #[test]
    fn test_apply_body_insert() {
        let content = "---\nname: test\n---\n# Section A\nContent A\n# Section B\nContent B";
        let mut edit = make_edit(SkillEditType::BodyInsert);
        edit.section = Some("Section A".to_string());
        edit.new_text = Some("Inserted line".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        // The inserted line should appear right after "# Section A".
        let section_idx = lines
            .iter()
            .position(|l| l.contains("# Section A"))
            .unwrap();
        assert_eq!(lines[section_idx + 1], "Inserted line");
        // Original content should still be there.
        assert!(result.contains("Content A"));
        assert!(result.contains("Content B"));
    }

    #[test]
    fn test_apply_body_insert_fallback_append() {
        let content = "---\nname: test\n---\n# Body\nSome content";
        let mut edit = make_edit(SkillEditType::BodyInsert);
        edit.section = Some("Nonexistent Section".to_string());
        edit.new_text = Some("Appended line".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.ends_with("Appended line"));
    }

    #[test]
    fn test_apply_body_remove() {
        let content = "---\nname: test\n---\n# Body\nremove this\nkeep this";
        let mut edit = make_edit(SkillEditType::BodyRemove);
        edit.old_text = Some("remove this\n".to_string());

        let result = apply_single_edit(content, &edit).unwrap();
        assert!(!result.contains("remove this"));
        assert!(result.contains("keep this"));
    }

    #[test]
    fn test_format_rules_empty() {
        assert_eq!(format_rules(&[]), "No active rules.");
    }

    #[test]
    fn test_format_user_model_empty() {
        let model = crate::types::UserModel::default();
        assert_eq!(format_user_model(&model), "No user model facts yet.");
    }

    // ── Guardrail tests ─────────────────────────────────────────

    #[test]
    fn test_validate_valid_params() {
        let overrides = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.35),
            ("min_similarity".to_string(), 0.60),
        ]);
        let result = validate_param_overrides(&overrides);
        assert!(result.is_some());
        let v = result.unwrap();
        assert!((v["relevance_weight_semantic"] - 0.35).abs() < f64::EPSILON);
        assert!((v["min_similarity"] - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_clamps_out_of_range() {
        let overrides = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.90), // max is 0.60
        ]);
        let result = validate_param_overrides(&overrides).unwrap();
        assert!((result["relevance_weight_semantic"] - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_rejects_mostly_invalid() {
        let overrides = HashMap::from([
            ("unknown_param_1".to_string(), 0.5),
            ("unknown_param_2".to_string(), 0.5),
            ("relevance_weight_semantic".to_string(), 0.35),
        ]);
        // 2/3 invalid → rejected
        assert!(validate_param_overrides(&overrides).is_none());
    }

    #[test]
    fn test_diversity_gate_passes_diverse() {
        // Two trials with substantially different params across multiple dimensions.
        let a = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.15),
            ("min_similarity".to_string(), 0.40),
            ("vector_top_k".to_string(), 20.0),
        ]);
        let b = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.50),
            ("min_similarity".to_string(), 0.70),
            ("vector_top_k".to_string(), 80.0),
        ]);
        let champion = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.30),
            ("min_similarity".to_string(), 0.55),
            ("vector_top_k".to_string(), 50.0),
        ]);
        assert!(check_diversity(&[&a, &b], &champion));
    }

    #[test]
    fn test_diversity_gate_rejects_identical() {
        let a = HashMap::from([("relevance_weight_semantic".to_string(), 0.35)]);
        let b = HashMap::from([("relevance_weight_semantic".to_string(), 0.35)]);
        let champion = HashMap::from([("relevance_weight_semantic".to_string(), 0.30)]);
        assert!(!check_diversity(&[&a, &b], &champion));
    }
}
