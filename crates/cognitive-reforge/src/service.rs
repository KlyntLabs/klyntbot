//! Phase orchestrator for the Reforge cycle.
//!
//! `run_reforge` drives **14 phase markers** with 3 LLM calls at the
//! handler level (Synthesize, Review, Narrate). The core path:
//!
//!   1   Collect
//!   2   Synthesize  [LLM #1]    — ReforgeHandler::synthesize
//!   2.6 Cross-CLI transfer      — CrossCliPhaseRunner hook
//!   3   Review      [LLM #2]    — ReforgeHandler::review
//!   3.6 Skill discovery         — SkillDiscoveryRunner hook
//!   4   Narrate     [LLM #3]    — ReforgeHandler::narrate
//!   5   Apply                   — persist suggestions + rewrite strategies
//!   6   Optimize                — AutotunerBridge
//!   6.5 Graph Consolidation     — GraphEnrichmentHandler hook
//!   6.5b Community Intelligence — CommunityIntelligenceHandler hook (Louvain)
//!   7   Compact                 — trim retired data; rebuild indexes
//!
//! Each phase is isolated so a single failure does not abort the
//! remaining phases. The 6 extension hook traits are all
//! `Option<&dyn Trait>` parameters on `run_reforge` — the cycle
//! degrades gracefully when a handler isn't installed.
//!
//! History: previously included Phase 6.7 (Community Summaries) and
//! Phase 7.7 (Compression) which were env-gated under
//! `KCA_COMMUNITY_SUMMARIES` / `KCA_REFORGE_COMPRESS`. Both were
//! removed 2026-05-17 — features had been off in production for months.

use std::collections::HashMap;

use jiff::Timestamp;
use sqlx;
use tracing::{debug, info, warn};

use crate::skill_files::{compute_diff, content_hash, SkillFile, SkillFileManager};
use crate::{types::*, AutotunerBridge, Phase6Result};
use cognitive_memory::repos::{ProceduralRuleRepo, SemanticFactRepo};
use cognitive_memory::types::{ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};

use common::helpers::truncate_chars;

// ---------------------------------------------------------------------------
// Standalone Phase 6 autotuner evaluation (also called by cron)
// ---------------------------------------------------------------------------

/// Run Phase 6 autotuner evaluation through the bridge.
///
/// Exported so the nightly cron callback can invoke it independently of
/// the full Reforge cycle.
pub async fn run_phase6_autotuner(bridge: &dyn AutotunerBridge) -> common::Result<Phase6Result> {
    bridge.run_evaluation().await
}

// ---------------------------------------------------------------------------
// Phase input builders
// ---------------------------------------------------------------------------

pub(crate) fn build_synthesize_input(collected: &ReforgeCollected) -> SynthesizeInput {
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

pub(crate) fn build_review_input(
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

pub(crate) fn build_narrate_input(
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

pub(crate) async fn apply_knowledge(
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
                        effectiveness_score: 0.5,
                        stability: 1.0,
                        scope_repo_id: None,
                        last_applied: None,
                        application_count: 0,
                        metadata: None,
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

pub(crate) async fn apply_skill_edits(
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
        scope_repo_id: None,
        metadata: None,
        speaker: None,
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

fn format_user_model(model: &cognitive_memory::types::UserModel) -> String {
    let mut parts = Vec::new();
    let domains = [
        ("identity", &model.identity),
        ("energy", &model.energy),
        ("work", &model.work),
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
pub(crate) async fn create_trials_from_suggestions(
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
pub(crate) async fn record_knowledge_snapshot(
    entity_repo: &cognitive_memory::repos::EntityRepo,
    fact_repo: &SemanticFactRepo,
    snapshot_repo: &cognitive_memory::repos::KnowledgeSnapshotRepo,
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
        let model = cognitive_memory::types::UserModel::default();
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
