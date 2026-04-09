//! Phase orchestrator for the Reforge cycle.
//!
//! `run_reforge` drives all 7 phases: Collect → Synthesize → Review →
//! Narrate → Apply → Optimize → Compact.  Each phase is isolated so that
//! a single failure does not abort the remaining phases.

use std::collections::HashMap;

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::services::reforge::skill_files::{compute_diff, content_hash, SkillFile, SkillFileManager};
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
    )
    .await;

    let collected = match collected {
        Some(c) => c,
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
        occurred_at: Utc::now().to_rfc3339(),
        recorded_at: Utc::now().to_rfc3339(),
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

    // 5d. Record run in reforge_state.
    let stats_json = serde_json::json!({
        "facts_added": result.facts_added,
        "facts_updated": result.facts_updated,
        "facts_stale_flagged": result.facts_stale_flagged,
        "rules_added": result.rules_added,
        "rules_reinforced": result.rules_reinforced,
        "skills_edited": result.skills_edited,
        "skipped_skill_edits": result.skipped_skill_edits.len(),
        "phase_errors": result.phase_errors.len(),
    });
    if let Err(e) = reforge_state_repo.record_run(&stats_json.to_string()).await {
        warn!("Reforge: failed to record run: {e}");
    }

    // ------------------------------------------------------------------
    // Phase 6: Optimize (deferred)
    // ------------------------------------------------------------------
    info!("Reforge Phase 6: skipped (autotuner integration deferred)");

    // ------------------------------------------------------------------
    // Phase 7: Compact
    // ------------------------------------------------------------------
    info!("Reforge Phase 7: Compact");
    match crate::services::compaction::run_compaction(
        fact_repo,
        episodic_repo,
        Some(rule_repo),
        None,
        None,
        Some(session_memory_repo),
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
        }
        Err(e) => {
            warn!("Reforge Phase 7 failed: {e}");
            result.phase_errors.push(format!("compact: {e}"));
        }
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

    ReviewInput {
        pending_meta_rules: collected.pending_meta_rules.clone(),
        routing_summaries: collected.routing_summaries.clone(),
        skill_contents,
        new_facts_summary,
        retrieval_precision: collected.retrieval_precision,
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
    let now = Utc::now().to_rfc3339();

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
            created_at: Utc::now().to_rfc3339(),
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
}
