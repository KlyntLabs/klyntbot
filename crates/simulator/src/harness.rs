//! Central orchestrator that ties together all simulator components:
//! SimulatedEpoch, PersonaRunner, ActionExecutor, MetricCollector,
//! GroundTruthVerifier, and ReportGenerator.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bus::{ContextUpdateQueue, CorrectionKind, DomainEvent, DomainEventBus};
use chrono::{Duration, TimeZone, Utc};
use context_engine::MemoryRetriever;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::actions::ActionExecutor;
use crate::epoch::{CronTrigger, EpochStep, SimulatedEpoch};
use crate::metrics::ground_truth::{CheckpointResult, GroundTruthVerifier};
use crate::metrics::memory::{measure_knowledge_retention, measure_retrieval_quality};
use crate::metrics::system::{count_brain_versions_since, measure_community_stability};
use crate::metrics::MetricCollector;
use crate::persona::types::SimulatedToolAction;
use crate::persona::PersonaRunner;
use crate::providers::FtsMemoryRetriever;
use crate::report::{compute_improvements, ReportSummary, SimulationReport};
use crate::scenario::Scenario;

/// The main simulation harness that orchestrates an end-to-end simulation run.
pub struct SimulationHarness {
    scenario: Scenario,
    /// Kept alive to prevent the in-memory database from being dropped.
    #[allow(dead_code)]
    pool: storage::StoragePool,
    inner_pool: sqlx::SqlitePool,
    bus: Arc<DomainEventBus>,
    context_queue: Arc<ContextUpdateQueue>,
    fact_repo: cognitive::SemanticFactRepo,
    episodic_repo: cognitive::EpisodicMemoryRepo,
    retriever: FtsMemoryRetriever,
    extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
    consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
    /// Used by Phase 2 reflection cycle.
    #[allow(dead_code)]
    rule_repo: cognitive::ProceduralRuleRepo,
    /// Used by Phase 2 mirror narrative generation and cleanup.
    #[allow(dead_code)]
    mirror_repo: cognitive::mirror::MirrorRepo,
    /// Used by Phase 2 weekly reflection cron.
    #[allow(dead_code)]
    reflection_handler: Arc<dyn cognitive::ReflectionHandler>,
    /// Used by Phase 2 mirror weekly narrative cron.
    #[allow(dead_code)]
    narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
}

impl SimulationHarness {
    /// Create a new harness for the given scenario.
    ///
    /// Sets up an in-memory SQLite database with all cognitive migrations,
    /// domain event bus, context update queue, and memory repos.
    pub async fn new(
        scenario: Scenario,
        extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
        consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
        reflection_handler: Arc<dyn cognitive::ReflectionHandler>,
        narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
    ) -> common::Result<Self> {
        // 1. Create in-memory storage pool with base migrations.
        let pool = storage::StoragePool::connect_in_memory().await?;
        let inner_pool = pool.inner().clone();

        // 2. Apply cognitive feature migrations.
        storage::StoragePool::run_feature_migrations(
            &inner_pool,
            &cognitive::cognitive_migrations(),
        )
        .await?;

        // 3. Create bus and context queue.
        let bus = Arc::new(DomainEventBus::new(512));
        let context_queue = Arc::new(ContextUpdateQueue::new());

        // 4. Create repos.
        let fact_repo = cognitive::SemanticFactRepo::new(inner_pool.clone());
        let episodic_repo = cognitive::EpisodicMemoryRepo::new(inner_pool.clone());

        // 5. Create FTS-based memory retriever for retrieval precision/recall.
        let retriever =
            FtsMemoryRetriever::new(cognitive::SemanticFactRepo::new(inner_pool.clone()));

        // 6. Create repos for reflection and mirror subsystems.
        let rule_repo = cognitive::ProceduralRuleRepo::new(inner_pool.clone());
        let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());

        Ok(Self {
            scenario,
            pool,
            inner_pool,
            bus,
            context_queue,
            fact_repo,
            episodic_repo,
            retriever,
            extraction_handler,
            consolidation_handler,
            rule_repo,
            mirror_repo,
            reflection_handler,
            narrative_handler,
        })
    }

    /// Run the full simulation and return a report.
    pub async fn run(&self) -> common::Result<SimulationReport> {
        let run_start = Instant::now();

        let total_days = self.scenario.total_days();
        let step = parse_epoch_step(&self.scenario.simulation.epoch_step);
        let start_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let end_date = start_date + Duration::days(i64::from(total_days));

        let mut epoch = SimulatedEpoch::new(start_date, end_date, step);
        let mut persona_runner = PersonaRunner::new(self.scenario.persona.clone());
        let mut metrics = MetricCollector::new(30);
        let action_executor = ActionExecutor::new(Arc::clone(&self.bus));

        // Subscribe to bus for ContradictionDetected events.
        let contradiction_count = Arc::new(AtomicU32::new(0));
        let mut bus_rx = self.bus.subscribe();
        let cc = Arc::clone(&contradiction_count);
        let contradiction_listener = tokio::spawn(async move {
            while let Ok(event) = bus_rx.recv().await {
                if matches!(event, bus::DomainEvent::ContradictionDetected { .. }) {
                    cc.fetch_add(1, Ordering::Relaxed);
                }
            }
        });

        // Collect all known facts for retention measurement.
        let mut known_facts = self.scenario.persona.profile.known_facts.clone();
        for phase_facts in [
            &self.scenario.persona.phases.onboarding.new_facts,
            &self.scenario.persona.phases.routine.new_facts,
            &self.scenario.persona.phases.power_user.new_facts,
            &self.scenario.persona.phases.behavior_shift.new_facts,
        ] {
            known_facts.extend(phase_facts.iter().cloned());
        }

        let mut checkpoint_results: Vec<CheckpointResult> = Vec::new();
        let mut day_counter: u32 = 0;
        let mut total_messages: u32 = 0;
        let mut total_facts_extracted: u32 = 0;

        info!(
            persona = %self.scenario.persona.name,
            total_days = total_days,
            step = ?step,
            "Starting simulation"
        );

        while let Some(plan) = epoch.advance() {
            let epoch_start = Instant::now();
            day_counter = plan.day_of_simulation;

            // Phase 2: PRE-MESSAGE CRONS
            for trigger in &plan.cron_pre_message {
                self.execute_cron(trigger, plan.simulated_now).await;
            }

            // Phase 3: MESSAGE PHASE
            let mut messages = persona_runner.generate_day(plan.simulated_now);

            // Backfill retrieval annotations: for each message, annotate with
            // recently extracted fact IDs from the same topic so retrieval
            // precision/recall can be measured against real DB state.
            for msg in &mut messages {
                let relevant = persona_runner.relevant_facts_for_topic(&msg.topic);
                if !relevant.is_empty() {
                    if let Some(ref mut gt) = msg.ground_truth {
                        gt.relevant_facts = relevant;
                    } else {
                        msg.ground_truth = Some(crate::persona::GroundTruthAnnotation {
                            introduces_fact: None,
                            relevant_facts: relevant,
                            expected_skill: None,
                        });
                    }
                }
            }

            total_messages += messages.len() as u32;
            for msg in &messages {
                // Track accumulator metrics.
                metrics.accumulator_mut().messages_processed += 1;
                if msg.is_correction {
                    metrics.accumulator_mut().corrections += 1;
                    self.bus.publish(DomainEvent::UserCorrectedAI {
                        original: String::new(),
                        correction: msg.content.clone(),
                        kind: CorrectionKind::Reaction,
                        strength: 1.0,
                        session_key: "sim-session".to_string(),
                        active_skill: Some(msg.topic.clone()),
                    });
                }
                if let Some(ref gt) = msg.ground_truth {
                    if gt.introduces_fact.is_some() {
                        metrics.accumulator_mut().facts_introduced += 1;
                    }
                }

                // Execute tool actions.
                for action in &msg.tool_actions {
                    if let Err(e) = action_executor.execute(action, msg.simulated_at).await {
                        warn!(error = %e, "Failed to execute tool action");
                    }
                    // Track tasks_created / tasks_completed.
                    match action {
                        SimulatedToolAction::CreateTask { .. } => {
                            metrics.accumulator_mut().tasks_created += 1;
                        }
                        SimulatedToolAction::CompleteTask { .. } => {
                            metrics.accumulator_mut().tasks_completed += 1;
                        }
                        _ => {}
                    }
                }

                // Drive cognitive pipeline: extract facts from message.
                self.run_cognitive_pipeline(msg, &mut metrics).await;

                // Record extracted fact IDs for future retrieval annotation backfill.
                if msg
                    .ground_truth
                    .as_ref()
                    .and_then(|gt| gt.introduces_fact.as_ref())
                    .is_some()
                {
                    if let Ok(facts) = self
                        .fact_repo
                        .search_fts(&msg.content, Some(&msg.topic), 1)
                        .await
                    {
                        for fact in facts {
                            persona_runner.record_extracted_fact(&msg.topic, &fact.id);
                        }
                    }
                }

                // Step 3d: Drive retrieval for precision/recall metrics.
                let retrieved = self.retriever.retrieve(&msg.content, 10).await;
                let retrieved_ids: Vec<String> = retrieved.iter().map(|e| e.id.clone()).collect();

                if let Some(ref gt) = msg.ground_truth {
                    if !gt.relevant_facts.is_empty() {
                        let (precision, recall) =
                            measure_retrieval_quality(&retrieved_ids, &gt.relevant_facts);
                        metrics.accumulator_mut().retrieval_precision_sum += precision;
                        metrics.accumulator_mut().retrieval_recall_sum += recall;
                        metrics.accumulator_mut().retrieval_count += 1;
                    }
                }

                // Routing stability: check if message content matches expected topic keywords
                if message_matches_topic_keywords(&msg.content, &msg.topic) {
                    metrics.accumulator_mut().routing_matches += 1;
                }

                // Token tracking (150 per scripted call).
                metrics.accumulator_mut().total_tokens += 150;
            }

            // Read contradiction events detected during this epoch's message processing.
            let contradictions = contradiction_count.swap(0, Ordering::Relaxed);
            metrics.accumulator_mut().contradictions_detected += contradictions;

            // Seed a routing snapshot for the day's messages.
            if !messages.is_empty() {
                let mut topic_counts: HashMap<String, u32> = HashMap::new();
                for m in &messages {
                    *topic_counts.entry(m.topic.clone()).or_default() += 1;
                }
                let total = messages.len() as f64;
                let distribution: HashMap<String, f64> = topic_counts
                    .iter()
                    .map(|(k, &v)| (k.clone(), v as f64 / total))
                    .collect();
                let distribution_json =
                    serde_json::to_string(&distribution).unwrap_or_default();
                let snapshot_id = Uuid::new_v4().to_string();
                let captured_at = plan.simulated_now.to_rfc3339();
                let total_msgs = messages.len() as i64;
                let _ = sqlx::query(
                    r#"INSERT INTO mirror_routing_snapshots
                       (id, captured_at, window_hours, total_messages, distribution_json,
                        fallback_rate, avg_routing_confidence, low_confidence_count)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                )
                .bind(&snapshot_id)
                .bind(&captured_at)
                .bind(24_i64)
                .bind(total_msgs)
                .bind(&distribution_json)
                .bind(0.0_f64) // fallback_rate -- no fallbacks in simulation
                .bind(0.85_f64) // avg_routing_confidence -- simulated default
                .bind(0_i64) // low_confidence_count
                .execute(&self.inner_pool)
                .await;
            }

            // Phase 3.5: CONTEXT UPDATE DRAIN
            let _ = self.context_queue.drain();

            // Phase 4: POST-MESSAGE CRONS
            for trigger in &plan.cron_post_message {
                self.execute_cron(trigger, plan.simulated_now).await;
            }

            // Phase 5: CHECKPOINTS
            for checkpoint in &self.scenario.checkpoints {
                if checkpoint.at_day == plan.day_of_simulation {
                    let latest = metrics.timeline.last().cloned().unwrap_or_default();
                    let result = GroundTruthVerifier::verify_checkpoint(
                        checkpoint,
                        &self.fact_repo,
                        &latest,
                        metrics.baselines.as_ref(),
                    )
                    .await;
                    info!(
                        day = checkpoint.at_day,
                        passed = result.all_passed,
                        "Checkpoint evaluated"
                    );
                    checkpoint_results.push(result);
                }
            }

            // Phase 6: METRIC SNAPSHOT
            let knowledge_retention =
                measure_knowledge_retention(&self.fact_repo, &known_facts).await;
            let community_stability = measure_community_stability(&self.inner_pool).await;
            let brain_versions =
                count_brain_versions_since(&self.inner_pool, &plan.previous.to_rfc3339()).await;
            let wall_time_ms = epoch_start.elapsed().as_secs_f64() * 1000.0;

            // Capture accumulator totals before snapshot resets them.
            total_facts_extracted += metrics.accumulator_mut().facts_extracted;

            metrics.snapshot(
                plan.simulated_now,
                plan.day_of_simulation,
                knowledge_retention,
                0.0, // autotuner_promotion_success (stub for Phase 1)
                community_stability,
                brain_versions,
                0.0, // insight_usefulness (stub for Phase 1)
                wall_time_ms,
            );

            // Progress logging every 30 days.
            if plan.day_of_simulation % 30 == 0 {
                info!(
                    day = plan.day_of_simulation,
                    total = total_days,
                    phase = %persona_runner.current_phase(),
                    "Simulation progress"
                );
            }
        }

        // Stop the contradiction listener before building the report.
        contradiction_listener.abort();

        // Build the final report.
        let wall_time_secs = run_start.elapsed().as_secs_f64();
        let final_metrics = metrics.timeline.last().cloned().unwrap_or_default();
        let regression_alerts = metrics.check_regressions(10.0);

        let improvement_pct = metrics
            .baselines
            .as_ref()
            .map(|bl| compute_improvements(bl, &final_metrics))
            .unwrap_or_default();

        let checkpoint_pass_count = checkpoint_results.iter().filter(|c| c.all_passed).count();
        let checkpoint_pass_rate = if checkpoint_results.is_empty() {
            1.0
        } else {
            checkpoint_pass_count as f64 / checkpoint_results.len() as f64
        };

        let summary = ReportSummary {
            total_messages,
            total_facts_extracted,
            total_facts_superseded: 0,
            total_brain_versions: metrics
                .timeline
                .iter()
                .map(|s| s.brain_version_velocity)
                .sum(),
            total_autotuner_promotions: 0,
            total_autotuner_reverts: 0,
            final_metrics,
            baseline_metrics: metrics.baselines.clone(),
            improvement_pct,
            checkpoint_pass_rate,
            regression_alerts,
        };

        let report = SimulationReport {
            scenario: self.scenario.persona.name.clone(),
            persona: self.scenario.persona.name.clone(),
            simulated_days: day_counter,
            wall_time_secs,
            seed: self.scenario.persona.seed,
            metric_timeline: metrics.timeline,
            checkpoints: checkpoint_results,
            summary,
        };

        info!(
            days = report.simulated_days,
            wall_time = format!("{:.2}s", report.wall_time_secs),
            passed = report.passed(),
            "Simulation complete"
        );

        Ok(report)
    }

    /// Run the cognitive extraction and consolidation pipeline for a single message.
    async fn run_cognitive_pipeline(
        &self,
        msg: &crate::persona::types::AnnotatedMessage,
        metrics: &mut MetricCollector,
    ) {
        // Create an Observation from the message.
        // Use "UserStatedFact" for messages that introduce facts (heuristic handler
        // extracts these at confidence 1.0), "ChatTurnCompleted" for regular messages
        // (heuristic handler extracts these at confidence 0.8 if > 10 chars and not a question).
        let introduces_fact = msg
            .ground_truth
            .as_ref()
            .and_then(|gt| gt.introduces_fact.as_ref());
        let (source_event, importance) = if introduces_fact.is_some() {
            ("UserStatedFact".to_string(), 0.9)
        } else {
            ("ChatTurnCompleted".to_string(), 0.5)
        };
        let observation = cognitive::Observation {
            domain: msg.topic.clone(),
            content: msg.content.clone(),
            importance,
            source_event,
            timestamp: msg.simulated_at,
        };

        // Extract facts from the observation.
        let extraction_result = match self
            .extraction_handler
            .extract_facts_batch(std::slice::from_ref(&observation))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                debug!(error = %e, "Extraction failed for message");
                return;
            }
        };

        // Collect all extracted facts and build consolidation candidates.
        let mut total_extracted = 0u32;
        for batch in &extraction_result.extractions {
            for extracted_fact in &batch.facts {
                total_extracted += 1;

                // Convert ExtractedFact to SemanticFact.
                let semantic_fact =
                    cognitive::extraction::to_semantic_fact(extracted_fact, &observation);

                // Look up existing similar facts for consolidation.
                let existing = self
                    .fact_repo
                    .find_similar(&semantic_fact.subject, &semantic_fact.predicate)
                    .await
                    .unwrap_or_default();

                let candidate = cognitive::ConsolidationCandidate {
                    candidate: semantic_fact,
                    existing,
                };

                // Run consolidation for this single candidate.
                match self
                    .consolidation_handler
                    .decide_batch(std::slice::from_ref(&candidate))
                    .await
                {
                    Ok(ops) => {
                        cognitive::execute_memory_ops(&ops, &[candidate], &self.fact_repo, None)
                            .await;
                    }
                    Err(e) => {
                        debug!(error = %e, "Consolidation failed, falling back to direct add");
                        // Fallback: just upsert the fact directly.
                        if let Err(e) = self.fact_repo.upsert(&candidate.candidate).await {
                            warn!(error = %e, "Failed to upsert fact in fallback path");
                        }
                    }
                }
            }
        }

        metrics.accumulator_mut().facts_extracted += total_extracted;

        // Write episodic memory for high-importance messages (feeds weekly reflection).
        if observation.importance >= 0.7 {
            let ts = msg.simulated_at.to_rfc3339();
            let episode = cognitive::EpisodicMemory {
                id: Uuid::new_v4().to_string(),
                domain: msg.topic.clone(),
                content: msg.content.clone(),
                summary: None,
                importance: observation.importance,
                occurred_at: ts.clone(),
                recorded_at: ts,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
            };
            let _ = self.episodic_repo.insert(&episode).await;
        }
    }

    /// Execute a simulated cron trigger.
    ///
    /// For Phase 1, most crons are stubs with debug logging. AtomDecay
    /// attempts to run the actual decay cycle if available.
    async fn execute_cron(
        &self,
        trigger: &CronTrigger,
        simulated_now: chrono::DateTime<chrono::Utc>,
    ) {
        match trigger {
            CronTrigger::AtomDecay => {
                debug!(trigger = "AtomDecay", %simulated_now, "Executing cron");
                // AtomDecay: attempt actual decay cycle.
                // The decay service may not exist in our dependency graph yet;
                // log and continue if it fails.
                if let Err(e) = self.run_atom_decay().await {
                    debug!(error = %e, "AtomDecay cron skipped (service unavailable)");
                }
            }
            CronTrigger::AutotunerNightly => {
                debug!(trigger = "AutotunerNightly", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::CognitiveReflection => {
                debug!(trigger = "CognitiveReflection", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::MirrorWeeklyNarrative => {
                debug!(trigger = "MirrorWeeklyNarrative", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::MirrorCleanup => {
                debug!(trigger = "MirrorCleanup", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::CrossDomainInsight => {
                debug!(trigger = "CrossDomainInsight", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::AnalyticsCleanup => {
                debug!(trigger = "AnalyticsCleanup", %simulated_now, "Cron stub (Phase 1)");
            }
            CronTrigger::MemoryMaintenance => {
                debug!(trigger = "MemoryMaintenance", %simulated_now, "Cron stub (Phase 1)");
            }
        }
    }

    /// Attempt to run the atom decay cycle.
    async fn run_atom_decay(&self) -> common::Result<()> {
        // In Phase 1, atom decay is a stub. The actual service may have a
        // different API surface. We'll wire this up properly when the service
        // is available in the simulation context.
        debug!("AtomDecay: stub — no decay cycle executed in Phase 1");
        Ok(())
    }
}

/// Check if a message's content contains keywords associated with its topic.
/// This is a simplified version of what SkillRouter does with Aho-Corasick matching.
fn message_matches_topic_keywords(content: &str, topic: &str) -> bool {
    let lower = content.to_lowercase();
    match topic {
        "tasks" => {
            lower.contains("task")
                || lower.contains("todo")
                || lower.contains("done")
                || lower.contains("prioritize")
        }
        "finance" => {
            lower.contains("expense")
                || lower.contains("budget")
                || lower.contains("spend")
                || lower.contains("income")
        }
        "notes" => lower.contains("note") || lower.contains("summarize") || lower.contains("write"),
        "productivity" => {
            lower.contains("focus") || lower.contains("productive") || lower.contains("time")
        }
        "learning" => {
            lower.contains("learn") || lower.contains("flashcard") || lower.contains("quiz")
        }
        "automation" => {
            lower.contains("remind") || lower.contains("recurring") || lower.contains("automate")
        }
        "insights" => {
            lower.contains("pattern") || lower.contains("connection") || lower.contains("insight")
        }
        _ => true, // "chat" and unknown topics — always match
    }
}

/// Parse an epoch step string from scenario config into an `EpochStep`.
fn parse_epoch_step(s: &str) -> EpochStep {
    match s.to_lowercase().as_str() {
        "hour" | "hours" => EpochStep::Hours(4),
        "week" => EpochStep::Week,
        _ => EpochStep::Day,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_epoch_step_variants() {
        assert!(matches!(parse_epoch_step("day"), EpochStep::Day));
        assert!(matches!(parse_epoch_step("Day"), EpochStep::Day));
        assert!(matches!(parse_epoch_step("week"), EpochStep::Week));
        assert!(matches!(parse_epoch_step("Week"), EpochStep::Week));
        assert!(matches!(parse_epoch_step("hour"), EpochStep::Hours(4)));
        assert!(matches!(parse_epoch_step("hours"), EpochStep::Hours(4)));
        assert!(matches!(parse_epoch_step("unknown"), EpochStep::Day));
    }
}
