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
use crate::metrics::system::{
    count_brain_versions_since, measure_autotuner_success, measure_community_stability,
    measure_insight_usefulness,
};
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
    rule_repo: cognitive::ProceduralRuleRepo,
    mirror_repo: cognitive::mirror::MirrorRepo,
    reflection_handler: Arc<dyn cognitive::ReflectionHandler>,
    narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
    /// ID of the first seeded autotuner trial (Trial A — default params).
    #[allow(dead_code)]
    active_trial_id: String,
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
        let pool = storage::StoragePool::connect_in_memory().await?;
        let inner_pool = pool.inner().clone();

        storage::StoragePool::run_feature_migrations(
            &inner_pool,
            &cognitive::cognitive_migrations(),
        )
        .await?;

        // Migrate autotuner tables (experiments, trials, shadow log).
        let trial_repo = storage::TrialRepo::new(inner_pool.clone());
        trial_repo.migrate().await?;

        // Run feature-tasks migration so the `tasks` table exists for
        // task-related domain entity rows and metrics.
        sqlx::query(feature_tasks::TasksFeature::migration_sql())
            .execute(&inner_pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("tasks migration failed: {e}")))?;

        // Seed an autotuner experiment with two active trials so the
        // nightly evaluation cycle has data to work with.
        let experiment_id = Uuid::new_v4().to_string();
        let trial_a_id = Uuid::new_v4().to_string();
        let trial_b_id = Uuid::new_v4().to_string();
        let now_str = chrono::Utc::now().to_rfc3339();

        let experiment = storage::ExperimentRow {
            id: experiment_id.clone(),
            hypothesis: "Adjusting routing weights improves skill selection accuracy".to_string(),
            trend_analysis: "Baseline routing with default weights".to_string(),
            recommendation_for_next: "Compare default vs tuned keyword/semantic weights"
                .to_string(),
            created_at: now_str.clone(),
        };
        trial_repo.create_experiment(&experiment).await?;

        // Trial A: default params (all None — uses Config defaults).
        let params_a = common::autotuner::TrialParams::default();
        let trial_a = storage::TrialRow {
            id: trial_a_id.clone(),
            experiment_id: experiment_id.clone(),
            params: serde_json::to_string(&params_a).unwrap_or_default(),
            generation_reasoning: "Control trial with default parameters".to_string(),
            status: "active".to_string(),
            created_at: now_str.clone(),
            completed_at: None,
            result: None,
        };
        trial_repo.create_trial(&trial_a).await?;

        // Trial B: modified routing weights.
        let params_b = common::autotuner::TrialParams {
            skill_keyword_weight: Some(0.7),
            skill_semantic_weight: Some(0.3),
            heuristic_confidence_threshold: Some(0.65),
            ..Default::default()
        };
        let trial_b = storage::TrialRow {
            id: trial_b_id,
            experiment_id,
            params: serde_json::to_string(&params_b).unwrap_or_default(),
            generation_reasoning: "Variant trial with boosted keyword weight".to_string(),
            status: "active".to_string(),
            created_at: now_str,
            completed_at: None,
            result: None,
        };
        trial_repo.create_trial(&trial_b).await?;

        let bus = Arc::new(DomainEventBus::new(512));
        let context_queue = Arc::new(ContextUpdateQueue::new());

        let fact_repo = cognitive::SemanticFactRepo::new(inner_pool.clone());
        let episodic_repo = cognitive::EpisodicMemoryRepo::new(inner_pool.clone());
        let retriever =
            FtsMemoryRetriever::new(cognitive::SemanticFactRepo::new(inner_pool.clone()));
        let rule_repo = cognitive::ProceduralRuleRepo::new(inner_pool.clone());
        let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());

        // Ensure a parent session row exists for session message persistence.
        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO sessions (key, metadata, created_at, updated_at) \
             VALUES ('sim-session', '{}', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&inner_pool)
        .await;

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
            active_trial_id: trial_a_id,
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
        let action_executor = ActionExecutor::new(Arc::clone(&self.bus), self.inner_pool.clone());

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
        let mut total_autotuner_promotions: u32 = 0;
        let mut total_autotuner_reverts: u32 = 0;

        info!(
            persona = %self.scenario.persona.name,
            total_days = total_days,
            step = ?step,
            "Starting simulation"
        );

        while let Some(plan) = epoch.advance() {
            let epoch_start = Instant::now();
            day_counter = plan.day_of_simulation;

            // PRE-MESSAGE CRONS
            for trigger in &plan.cron_pre_message {
                self.execute_cron(trigger, plan.simulated_now).await;
            }

            // MESSAGE PHASE
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

                // Persist session message for conversation recall.
                let msg_id = Uuid::new_v4().to_string();
                let role = "user";
                let _ = sqlx::query(
                    "INSERT INTO session_messages \
                     (id, session_key, role, content, timestamp) \
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&msg_id)
                .bind("sim-session")
                .bind(role)
                .bind(&msg.content)
                .bind(msg.simulated_at.to_rfc3339())
                .execute(&self.inner_pool)
                .await;

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
                let extracted_ids = self.run_cognitive_pipeline(msg, &mut metrics).await;

                // Record extracted fact IDs for future retrieval annotation backfill.
                for id in &extracted_ids {
                    persona_runner.record_extracted_fact(&msg.topic, id);
                }

                // Drive retrieval for precision/recall metrics.
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

            // Yield to let the contradiction listener process any pending bus events.
            tokio::task::yield_now().await;
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
                let distribution_json = serde_json::to_string(&distribution).unwrap_or_default();
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

            // CONTEXT UPDATE DRAIN
            let _ = self.context_queue.drain();

            // POST-MESSAGE CRONS
            for trigger in &plan.cron_post_message {
                self.execute_cron(trigger, plan.simulated_now).await;
            }

            // CHECKPOINTS
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

            // METRIC SNAPSHOT
            let knowledge_retention =
                measure_knowledge_retention(&self.fact_repo, &known_facts).await;
            let community_stability = measure_community_stability(&self.inner_pool).await;
            let brain_versions =
                count_brain_versions_since(&self.inner_pool, &plan.previous.to_rfc3339()).await;
            let autotuner_stats = measure_autotuner_success(&self.inner_pool).await;
            let wall_time_ms = epoch_start.elapsed().as_secs_f64() * 1000.0;

            total_autotuner_promotions += autotuner_stats.promoted;
            total_autotuner_reverts += autotuner_stats.reverted;

            // Capture accumulator totals before snapshot resets them.
            total_facts_extracted += metrics.accumulator_mut().facts_extracted;

            let insight_usefulness =
                measure_insight_usefulness(&self.inner_pool, day_counter).await;

            metrics.snapshot(
                plan.simulated_now,
                plan.day_of_simulation,
                knowledge_retention,
                autotuner_stats.success_rate,
                community_stability,
                brain_versions,
                insight_usefulness,
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
        let regression_alerts =
            metrics.check_regressions(self.scenario.simulation.regression_threshold);

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
            total_autotuner_promotions,
            total_autotuner_reverts,
            final_metrics,
            baseline_metrics: metrics.baselines.clone(),
            improvement_pct,
            checkpoint_pass_rate,
            regression_alerts,
        };

        let report = SimulationReport {
            scenario: format!(
                "{}_{}",
                self.scenario.persona.name,
                self.scenario.total_days()
            ),
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
    ///
    /// Returns the IDs of facts that were successfully stored, so the caller
    /// can track them without a redundant FTS query.
    async fn run_cognitive_pipeline(
        &self,
        msg: &crate::persona::types::AnnotatedMessage,
        metrics: &mut MetricCollector,
    ) -> Vec<String> {
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

        let extraction_result = match self
            .extraction_handler
            .extract_facts_batch(std::slice::from_ref(&observation))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                debug!(error = %e, "Extraction failed for message");
                return Vec::new();
            }
        };

        // Build ALL consolidation candidates first (find_similar stays per-fact
        // since it needs the predicate), then call decide_batch once.
        let mut candidates: Vec<cognitive::ConsolidationCandidate> = Vec::new();
        let mut fact_ids: Vec<String> = Vec::new();
        let mut total_extracted = 0u32;

        for batch in &extraction_result.extractions {
            for extracted_fact in &batch.facts {
                total_extracted += 1;

                let semantic_fact =
                    cognitive::extraction::to_semantic_fact(extracted_fact, &observation);
                fact_ids.push(semantic_fact.id.clone());

                let existing = self
                    .fact_repo
                    .find_similar(&semantic_fact.subject, &semantic_fact.predicate)
                    .await
                    .unwrap_or_default();

                candidates.push(cognitive::ConsolidationCandidate {
                    candidate: semantic_fact,
                    existing,
                });
            }
        }

        if !candidates.is_empty() {
            match self.consolidation_handler.decide_batch(&candidates).await {
                Ok(ops) => {
                    cognitive::execute_memory_ops(&ops, &candidates, &self.fact_repo, None).await;

                    // Contradiction detection: mirror the logic from
                    // cognitive::services::background that checks Update/Add ops
                    // for conflicting user-stated facts with high confidence.
                    for (candidate, op) in candidates.iter().zip(ops.iter()) {
                        match op {
                            cognitive::MemoryOp::Update { id: _, old_id } => {
                                let new = &candidate.candidate;
                                if let Ok(Some(old_fact)) = self.fact_repo.get(old_id).await {
                                    if old_fact.confidence < 0.7 || old_fact.source != "user_stated"
                                    {
                                        continue;
                                    }
                                    if old_fact.object != new.object {
                                        self.bus.publish(DomainEvent::ContradictionDetected {
                                            existing_subject: old_fact.subject.clone(),
                                            existing_predicate: old_fact.predicate.clone(),
                                            existing_object: old_fact.object.clone(),
                                            new_object: new.object.clone(),
                                            confidence: new.confidence,
                                        });
                                    }
                                }
                            }
                            cognitive::MemoryOp::Add { .. } => {
                                let new = &candidate.candidate;
                                for existing in &candidate.existing {
                                    if existing.superseded_at.is_some() {
                                        continue;
                                    }
                                    if existing.confidence < 0.7 || existing.source != "user_stated"
                                    {
                                        continue;
                                    }
                                    if existing.object != new.object
                                        && existing.subject == new.subject
                                        && existing.predicate == new.predicate
                                    {
                                        self.bus.publish(DomainEvent::ContradictionDetected {
                                            existing_subject: existing.subject.clone(),
                                            existing_predicate: existing.predicate.clone(),
                                            existing_object: existing.object.clone(),
                                            new_object: new.object.clone(),
                                            confidence: new.confidence,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Consolidation failed, falling back to direct add");
                    for candidate in &candidates {
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

        fact_ids
    }

    /// Execute a simulated cron trigger.
    async fn execute_cron(
        &self,
        trigger: &CronTrigger,
        simulated_now: chrono::DateTime<chrono::Utc>,
    ) {
        match trigger {
            CronTrigger::AtomDecay => {
                debug!(trigger = "AtomDecay", %simulated_now, "Executing cron");
                if let Err(e) =
                    cognitive::services::atom_decay::run_decay_cycle(&self.inner_pool, &self.bus)
                        .await
                {
                    debug!(error = %e, "AtomDecay cron failed");
                }
            }
            CronTrigger::AutotunerNightly => {
                debug!(trigger = "AutotunerNightly", %simulated_now, "Executing cron");
                let metric_source: Arc<dyn autotuner::traits::MetricSource> = Arc::new(
                    crate::providers::SimMetricSource::new(self.inner_pool.clone()),
                );
                let trial_repo = storage::TrialRepo::new(self.inner_pool.clone());
                let cycle = autotuner::NightlyCycle::new(
                    config::AutoTunerConfig::default(),
                    trial_repo,
                    metric_source,
                );
                let champion = autotuner::Champion::default();
                match cycle.run_evaluation_and_promotion(&champion).await {
                    Ok(result) => {
                        debug!(
                            promoted = result.promotion.is_some(),
                            regression = result.regression,
                            "AutotunerNightly completed"
                        );
                    }
                    Err(e) => {
                        debug!(error = %e, "AutotunerNightly failed (non-fatal)");
                    }
                }
            }
            CronTrigger::CognitiveReflection => {
                debug!(trigger = "CognitiveReflection", %simulated_now, "Executing cron");
                match cognitive::services::reflection::run_weekly_reflection(
                    self.reflection_handler.as_ref(),
                    self.consolidation_handler.as_ref(),
                    &self.fact_repo,
                    &self.episodic_repo,
                    &self.rule_repo,
                    None, // no embedder in simulation
                )
                .await
                {
                    Ok(output) => {
                        debug!(
                            summary = %output.summary,
                            facts = output.fact_updates.len(),
                            rules = output.rule_updates.len(),
                            "CognitiveReflection complete"
                        );
                    }
                    Err(e) => {
                        debug!(error = %e, "CognitiveReflection cron failed");
                    }
                }
            }
            CronTrigger::MirrorWeeklyNarrative => {
                debug!(trigger = "MirrorWeeklyNarrative", %simulated_now, "Executing cron");
                let facade = cognitive::mirror::MirrorFacade::new(self.mirror_repo.clone())
                    .with_narrative_handler(Arc::clone(&self.narrative_handler))
                    .with_episodic_repo(self.episodic_repo.clone());
                match facade.generate_weekly_narrative().await {
                    Ok(narrative) => {
                        debug!(
                            narrative_id = %narrative.id,
                            "MirrorWeeklyNarrative complete"
                        );
                    }
                    Err(e) => {
                        debug!(error = %e, "MirrorWeeklyNarrative cron failed");
                    }
                }
            }
            CronTrigger::MirrorCleanup => {
                debug!(trigger = "MirrorCleanup", %simulated_now, "Executing cron");
                let max_age_days = 90u32;
                if let Err(e) = self.mirror_repo.cleanup_old_snapshots(max_age_days).await {
                    debug!(error = %e, "MirrorCleanup: failed to clean snapshots");
                }
                if let Err(e) = self.mirror_repo.cleanup_old_snippets(max_age_days).await {
                    debug!(error = %e, "MirrorCleanup: failed to clean snippets");
                }
                if let Err(e) = self
                    .mirror_repo
                    .cleanup_old_trial_previews(max_age_days)
                    .await
                {
                    debug!(error = %e, "MirrorCleanup: failed to clean trial previews");
                }
            }
            CronTrigger::CrossDomainInsight => {
                debug!(trigger = "CrossDomainInsight", %simulated_now, "Executing cron");
                match self.fact_repo.list_all_active().await {
                    Ok(facts) if !facts.is_empty() => {
                        // Group facts by domain, look for cross-domain connections.
                        let mut domains: std::collections::HashMap<String, Vec<String>> =
                            std::collections::HashMap::new();
                        for f in &facts {
                            domains
                                .entry(f.domain.clone())
                                .or_default()
                                .push(format!("{} {} {}", f.subject, f.predicate, f.object));
                        }
                        if domains.len() >= 2 {
                            let domain_names: Vec<&String> = domains.keys().collect();
                            let insight_text = format!(
                                "Cross-domain pattern: {} domains active ({}) with {} total facts",
                                domains.len(),
                                domain_names
                                    .iter()
                                    .map(|d| d.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                facts.len()
                            );
                            let dot_refs = serde_json::to_string(&domain_names).unwrap_or_default();
                            let date = simulated_now.format("%Y-%m-%d").to_string();
                            let _ = sqlx::query(
                                "INSERT INTO cross_domain_insights (date, insight_text, dot_refs) VALUES (?1, ?2, ?3)",
                            )
                            .bind(&date)
                            .bind(&insight_text)
                            .bind(&dot_refs)
                            .execute(&self.inner_pool)
                            .await;
                        }
                    }
                    Ok(_) => {
                        debug!("CrossDomainInsight: no active facts yet");
                    }
                    Err(e) => {
                        debug!(error = %e, "CrossDomainInsight: failed to query facts");
                    }
                }
            }
            CronTrigger::AnalyticsCleanup => {
                // Ephemeral in-memory DB — cleanup unnecessary in simulation.
                debug!(trigger = "AnalyticsCleanup", %simulated_now, "Skipped (ephemeral in-memory DB)");
            }
            CronTrigger::MemoryMaintenance => {
                debug!(trigger = "MemoryMaintenance", %simulated_now, "Executing cron");
                let node_count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM book_tree_nodes WHERE source_type = 'Note'",
                )
                .fetch_one(&self.inner_pool)
                .await
                .unwrap_or((0,));

                if node_count.0 >= 3 {
                    let stability = (node_count.0 as f64 / 50.0).min(1.0);
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO communities \
                         (id, name, summary, stability, member_count, source_note_count, \
                          created_at, updated_at) \
                         VALUES ('sim-community', 'Simulated Notes Community', \
                                 'Auto-generated from simulation notes', ?, ?, ?, \
                                 datetime('now'), datetime('now'))",
                    )
                    .bind(stability)
                    .bind(node_count.0)
                    .bind(node_count.0)
                    .execute(&self.inner_pool)
                    .await;
                    debug!(
                        nodes = node_count.0,
                        stability, "Community stability updated"
                    );
                }
            }
        }
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
