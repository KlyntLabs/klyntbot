//! Central orchestrator that ties together all simulator components:
//! SimulatedEpoch, PersonaRunner, ActionExecutor, MetricCollector,
//! GroundTruthVerifier, and ReportGenerator.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bus::{ContextUpdateQueue, CorrectionKind, DomainEvent, DomainEventBus};
use jiff::{Timestamp, Zoned};
use rand::{Rng, SeedableRng};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::actions::ActionExecutor;
use crate::epoch::{CronTrigger, EpochStep, SimulatedEpoch};
use crate::metrics::ground_truth::{CheckpointResult, GroundTruthVerifier};
use crate::metrics::memory::{measure_knowledge_retention, measure_retrieval_quality};
use crate::metrics::system::{
    count_brain_versions_since, count_communities, measure_autotuner_success,
    measure_community_stability, measure_insight_usefulness,
};
use crate::metrics::MetricCollector;
use crate::persona::types::SimulatedToolAction;
use crate::persona::PersonaRunner;
use crate::providers::FtsMemoryRetriever;
use crate::report::{compute_improvements, ReportSummary, SimulationReport};
use crate::scenario::Scenario;

/// An extracted fact reference: `(fact_id, subject, predicate)`.
type ExtractedFactRef = (String, String, String);

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
    mirror_repo: cognitive::mirror::MirrorRepo,
    narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
    /// ID of the first seeded autotuner trial (Trial A — default params).
    /// Mutex-wrapped so `execute_cron` can update it after re-seeding.
    active_trial_id: std::sync::Mutex<String>,
    /// ID of the second seeded autotuner trial (Trial B — variant params).
    variant_trial_id: std::sync::Mutex<String>,
    /// Persistent champion state — updated after each promotion so the
    /// evaluator compares against real baselines instead of all-zeros.
    champion: std::sync::Mutex<autotuner::Champion>,
    embedding_engine: Option<tools::EmbeddingEngine>,
    /// Pre-cached embeddings for the 8 reference answer strings (one per topic).
    reference_embeddings: HashMap<String, Vec<f32>>,
    agent_harness: Option<crate::agent_harness::AgentHarness>,
    /// Coaching signal accumulator — fed from domain events to exercise the
    /// coaching detection pipeline during simulation.
    coaching_accumulator: Arc<TokioMutex<feature_coaching::SignalAccumulator>>,
    /// Coaching pattern detector — records trigger firings and detects
    /// behavioral patterns (afternoon energy drop, task avoidance, etc.).
    coaching_detector: Arc<TokioMutex<feature_coaching::PatternDetector>>,
    /// Meta-rule source — tracks correction streaks and proposes meta-rules.
    meta_rule_source: TokioMutex<cognitive::mirror::sources::MetaRuleSignalSource>,
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

        // Run additional feature migrations needed for agent-path tool execution.
        if scenario.simulation.agent_mode {
            // Notes tables
            sqlx::query(feature_notes::NotesFeature::migration_sql())
                .execute(&inner_pool)
                .await
                .map_err(|e| {
                    common::KlyntbotError::Storage(format!("notes migration failed: {e}"))
                })?;
            // Finance tables
            sqlx::query(feature_finance::FinanceFeature::migration_sql())
                .execute(&inner_pool)
                .await
                .map_err(|e| {
                    common::KlyntbotError::Storage(format!("finance migration failed: {e}"))
                })?;
            // Activity log tables
            for m in activity_log::activity_log_migrations() {
                sqlx::query(&m.sql)
                    .execute(&inner_pool)
                    .await
                    .map_err(|e| {
                        common::KlyntbotError::Storage(format!(
                            "activity_log migration failed: {e}"
                        ))
                    })?;
            }
            // Productivity tables
            storage::StoragePool::run_feature_migrations(
                &inner_pool,
                &feature_productivity::productivity_migrations(),
            )
            .await?;
        }

        // Seed an autotuner experiment with two active trials so the
        // nightly evaluation cycle has data to work with.
        let experiment_id = Uuid::new_v4().to_string();
        let trial_a_id = Uuid::new_v4().to_string();
        let trial_b_id = Uuid::new_v4().to_string();
        let now_str = jiff::Timestamp::now().to_string();

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
            id: trial_b_id.clone(),
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
        let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());

        // Ensure a parent session row exists for session message persistence.
        let now = jiff::Timestamp::now().to_string();
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO sessions (key, metadata, created_at, updated_at) \
             VALUES ('sim-session', '{}', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&inner_pool)
        .await;

        // Seed a parent area for task rows (tasks.area_id NOT NULL FK).
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO areas (id, name, color, status, created_at, updated_at) \
             VALUES ('sim-area', 'Simulation', '#6366f1', 'active', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&inner_pool)
        .await;

        // Seed a parent finance account for transaction rows
        // (finance_transactions.account_id NOT NULL FK).
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO finance_accounts \
             (id, name, account_type, currency, balance, created_at, updated_at) \
             VALUES ('sim-account', 'Simulation Account', 'checking', 'VND', 0, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&inner_pool)
        .await;

        let embedding_engine = tools::EmbeddingEngine::new();
        // Pre-cache embeddings for the 8 reference answer strings to avoid
        // re-embedding the same text hundreds of times during the simulation.
        let reference_texts = [
            "task management: listing tasks, creating tasks, completing tasks, setting priorities and deadlines, project planning",
            "note taking: creating notes, searching notes, organizing knowledge, summarizing content, documentation",
            "financial tracking: recording expenses, checking budgets, transaction categories, spending trends, financial summary",
            "productivity and focus: starting focus sessions, tracking time, work habits, distraction management, energy levels",
            "personal coaching: behavioral guidance, habit formation, priority management, motivation, work-life balance",
            "learning and study: flashcards, spaced repetition, knowledge review, quiz practice, educational content",
            "workflow automation: scheduled reminders, recurring tasks, cron jobs, automated notifications",
            "cross-domain insights: pattern analysis, connections between data, trends, correlations, behavioral patterns",
        ];
        let mut reference_embeddings = HashMap::new();
        for text in &reference_texts {
            if let Ok(emb) = embedding_engine.embed(text) {
                reference_embeddings.insert(text.to_string(), emb);
            }
        }
        let embedding_engine = Some(embedding_engine);

        // Build agent harness if agent_mode is enabled.
        let agent_harness = if scenario.simulation.agent_mode {
            let phases = [
                &scenario.persona.phases.onboarding,
                &scenario.persona.phases.routine,
                &scenario.persona.phases.power_user,
                &scenario.persona.phases.behavior_shift,
            ];
            let max_provider_error_rate = phases
                .iter()
                .map(|p| p.provider_error_rate)
                .fold(0.0f64, f64::max);
            let max_error_injection_rate = phases
                .iter()
                .map(|p| p.error_injection_rate)
                .fold(0.0f64, f64::max);

            match crate::agent_harness::AgentHarness::new(
                &pool,
                inner_pool.clone(),
                Arc::clone(&bus),
                Arc::clone(&context_queue),
                &scenario.simulation.agent_provider,
                &scenario.simulation.agent_model,
                max_provider_error_rate,
                max_error_injection_rate,
                scenario.persona.seed,
                scenario.simulation.agent_context_window,
                scenario.simulation.error_cascade_enabled,
                scenario.simulation.error_cascade_multiplier,
            )
            .await
            {
                Ok(h) => Some(h),
                Err(e) => {
                    warn!(error = %e, "Failed to create AgentHarness — agent path disabled");
                    None
                }
            }
        } else {
            None
        };

        let meta_rule_source = TokioMutex::new(
            cognitive::mirror::sources::MetaRuleSignalSource::new(mirror_repo.clone()),
        );

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
            mirror_repo,
            narrative_handler,
            active_trial_id: std::sync::Mutex::new(trial_a_id),
            variant_trial_id: std::sync::Mutex::new(trial_b_id),
            champion: std::sync::Mutex::new(autotuner::Champion::default()),
            embedding_engine,
            reference_embeddings,
            agent_harness,
            coaching_accumulator: Arc::new(TokioMutex::new(
                feature_coaching::SignalAccumulator::new(),
            )),
            coaching_detector: Arc::new(TokioMutex::new(feature_coaching::PatternDetector::new())),
            meta_rule_source,
        })
    }

    /// Run the full simulation and return a report.
    pub async fn run(&self) -> common::Result<SimulationReport> {
        let run_start = Instant::now();

        let total_days = self.scenario.total_days();
        let step = parse_epoch_step(&self.scenario.simulation.epoch_step);
        let start_date: Zoned = "2025-01-01T00:00:00+00:00[UTC]".parse().unwrap();
        let start_date_str = start_date.to_string();
        let end_date =
            start_date.clone() + jiff::SignedDuration::from_hours(i64::from(total_days) * 24);

        let mut epoch = SimulatedEpoch::new(start_date, end_date, step);
        let mut persona_runner = PersonaRunner::new(self.scenario.persona.clone());
        let mut metrics = MetricCollector::new(30);
        let action_executor = crate::error_injector::ErrorInjector::new(
            ActionExecutor::new(Arc::clone(&self.bus), self.inner_pool.clone()),
            self.scenario.persona.seed,
        );
        let trial_repo = storage::TrialRepo::new(self.inner_pool.clone());

        // Subscribe to bus for ContradictionDetected events.
        let contradiction_count = Arc::new(AtomicU32::new(0));
        let mut bus_rx = self.bus.subscribe();
        let cc = Arc::clone(&contradiction_count);
        let contradiction_listener = tokio::spawn(async move {
            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(5), bus_rx.recv()).await {
                    Ok(Ok(event)) => {
                        if matches!(event, bus::DomainEvent::ContradictionDetected { .. }) {
                            cc.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Ok(Err(_)) => break,
                    Err(_) => continue,
                }
            }
        });

        // Subscribe coaching pipeline to domain events.
        // The accumulator converts events to signals and evaluates trigger
        // conditions; fired triggers feed the pattern detector for behavioral
        // pattern recognition (afternoon energy drop, task avoidance, etc.).
        let coaching_acc = Arc::clone(&self.coaching_accumulator);
        let coaching_det = Arc::clone(&self.coaching_detector);
        let coaching_bus = Arc::clone(&self.bus);
        let mut coaching_rx = self.bus.subscribe();
        let persona_seed = self.scenario.persona.seed;
        // Coaching feedback counters — drained per epoch into the accumulator.
        let coaching_counters = Arc::new(CoachingCounters::default());
        let cc_inner = Arc::clone(&coaching_counters);
        let coaching_listener = tokio::spawn(async move {
            // Minimal UserSituation — the simulation doesn't have a real user,
            // but the accumulator needs one for trigger evaluation.
            let mut situation = cognitive::situation::UserSituation::default();
            loop {
                let event = match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    coaching_rx.recv(),
                )
                .await
                {
                    Ok(Ok(e)) => e,
                    Ok(Err(_)) => break, // channel closed
                    Err(_) => continue,  // timeout — check again
                };
                // Skip our own CoachingFeedback events to avoid a feedback loop.
                if matches!(event, DomainEvent::CoachingFeedback { .. }) {
                    continue;
                }

                // Incrementally update situation from relevant events.
                // Deltas scale with accumulated state to model diminishing
                // returns (first distraction is jarring, fifth is background
                // noise) and recovery inertia.
                match &event {
                    DomainEvent::DistractionDetected { .. } => {
                        // Diminishing impact: each distraction adds less as risk climbs.
                        let delta = 0.20 * (1.0 - situation.distraction_risk * 0.6);
                        situation.distraction_risk = (situation.distraction_risk + delta).min(1.0);
                    }
                    DomainEvent::FocusSessionStarted { target_mins, .. } => {
                        // Longer sessions signal higher commitment → higher initial focus.
                        let commitment = (*target_mins as f64 / 60.0).clamp(0.5, 1.0);
                        situation.focus_state = 0.7 + commitment * 0.25;
                        // Focus start partially clears distraction risk.
                        let recovery = 0.15 + commitment * 0.10;
                        situation.distraction_risk =
                            (situation.distraction_risk - recovery).max(0.0);
                    }
                    DomainEvent::FocusSessionEnded { quality, .. } => {
                        situation.focus_state = *quality;
                    }
                    DomainEvent::TaskDeferred { .. } => {
                        situation.task_avoidance_detected = true;
                    }
                    DomainEvent::BudgetAlert { .. } => {
                        // Escalating pressure: each alert compounds more.
                        let escalation = 0.15 + situation.deadline_pressure * 0.10;
                        situation.deadline_pressure =
                            (situation.deadline_pressure + escalation).min(1.0);
                    }
                    _ => {}
                }

                // Convert DomainEvent to AiSignal for the accumulator.
                let signal = match app_core::init::ai_pipeline::translate(&event) {
                    Some(s) => s,
                    None => continue,
                };
                let mut acc = coaching_acc.lock().await;
                acc.push_event(&signal);
                let fired = acc.evaluate(&situation);
                drop(acc);

                if !fired.is_empty() {
                    let mut det = coaching_det.lock().await;
                    for trigger in &fired {
                        det.record_trigger(trigger);

                        // Simulate user feedback for each coaching intervention.
                        let hash = trigger
                            .condition_name
                            .bytes()
                            .chain(trigger.context.bytes())
                            .fold(persona_seed, |a, b| {
                                a.wrapping_mul(31).wrapping_add(b as u64)
                            });
                        let roll = (hash % 1000) as f64 / 1000.0;
                        let response = simulate_feedback(roll, trigger.confidence);

                        match &response {
                            bus::FeedbackResponse::Helpful => {
                                cc_inner.helpful.fetch_add(1, Ordering::Relaxed);
                            }
                            bus::FeedbackResponse::Dismissed => {
                                cc_inner.dismissed.fetch_add(1, Ordering::Relaxed);
                            }
                            bus::FeedbackResponse::StopSuggesting => {
                                cc_inner.stop.fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        coaching_bus.publish(DomainEvent::CoachingFeedback {
                            intervention_id: trigger.condition_name.clone(),
                            response,
                        });
                    }
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
        let mut prev_promoted_count: u32 = 0;
        let mut prev_reverted_count: u32 = 0;

        // Agent path counters
        let mut agent_breakpoints: Vec<crate::agent_types::AgentBreakpoint> = Vec::new();
        let mut agent_total_calls: u32 = 0;
        let mut agent_successful: u32 = 0;
        let mut agent_react_iterations_sum: u32 = 0;
        let mut agent_reactive_total: u32 = 0;
        let mut agent_mode_counts: HashMap<String, u32> = HashMap::new();
        let mut conversation_tracker = crate::persona::conversation::ConversationTracker::new(
            self.scenario.simulation.multi_turn_history_depth as usize,
        );
        // Per-channel conversation trackers for multi-channel simulation.
        let mut channel_trackers: HashMap<
            String,
            crate::persona::conversation::ConversationTracker,
        > = HashMap::new();
        for ch in &self.scenario.simulation.channels {
            channel_trackers.insert(
                ch.name.clone(),
                crate::persona::conversation::ConversationTracker::new(
                    self.scenario.simulation.multi_turn_history_depth as usize,
                ),
            );
        }
        let has_channels = !self.scenario.simulation.channels.is_empty();
        let mut channel_rng =
            rand::rngs::StdRng::seed_from_u64(self.scenario.persona.seed.wrapping_add(7777));
        let mut total_followups: u32 = 0;
        let mut total_adversarial: u32 = 0;
        let mut total_workflows: u32 = 0;
        let mut parallel_workflows: u32 = 0;
        let mut sequential_workflows: u32 = 0;

        info!(
            persona = %self.scenario.persona.name,
            total_days = total_days,
            step = ?step,
            "Starting simulation"
        );

        // For sub-day epochs, buffer a full day's messages and distribute
        // them across epoch time windows.
        let mut pending_messages: Vec<crate::persona::types::AnnotatedMessage> = Vec::new();
        let mut current_day: u32 = 0;
        let is_sub_day = step.is_sub_day();

        while let Some(plan) = epoch.advance() {
            let epoch_start = Instant::now();
            day_counter = plan.day_of_simulation;
            eprintln!("  [sim] day {day_counter} starting...");

            // PRE-MESSAGE CRONS
            for trigger in &plan.cron_pre_message {
                self.execute_cron(trigger, plan.simulated_now.timestamp())
                    .await;
            }

            // MESSAGE PHASE — for sub-day epochs, generate once per day
            // then distribute messages by their simulated_at timestamp.
            let mut messages = if is_sub_day {
                if plan.day_of_simulation > current_day {
                    current_day = plan.day_of_simulation;
                    pending_messages = persona_runner.generate_day(&plan.simulated_now);
                }
                let mut epoch_msgs = Vec::new();
                pending_messages.retain(|m| {
                    if m.simulated_at <= plan.simulated_now {
                        epoch_msgs.push(m.clone());
                        false
                    } else {
                        true
                    }
                });
                epoch_msgs
            } else {
                persona_runner.generate_day(&plan.simulated_now)
            };

            total_messages += messages.len() as u32;

            // Assign channels to messages and track distribution
            let msg_channels: Vec<String> = if has_channels {
                messages
                    .iter()
                    .map(|_| {
                        let ch =
                            select_channel(&self.scenario.simulation.channels, &mut channel_rng);
                        let name = ch.name.clone();
                        *metrics
                            .accumulator_mut()
                            .channel_messages
                            .entry(name.clone())
                            .or_insert(0) += 1;
                        name
                    })
                    .collect()
            } else {
                vec!["default".to_string(); messages.len()]
            };

            // Clone trial IDs once per epoch (values only change during cron re-seeding).
            let active_id = self.active_trial_id.lock().unwrap().clone();
            let variant_id = self.variant_trial_id.lock().unwrap().clone();
            // Per-epoch routing stats for the mirror snapshot.
            let mut epoch_routing_confidence_sum = 0.0f64;
            let mut epoch_routing_fallbacks = 0u32;
            for (msg_idx, msg) in messages.iter_mut().enumerate() {
                // Select the appropriate conversation tracker for this message's channel.
                let active_tracker = if has_channels {
                    channel_trackers
                        .get_mut(&msg_channels[msg_idx])
                        .unwrap_or(&mut conversation_tracker)
                } else {
                    &mut conversation_tracker
                };

                // Backfill retrieval annotations inline: annotate with
                // previously extracted fact IDs from the same topic so
                // retrieval precision/recall can be measured. By doing this
                // per-message (instead of pre-loop), message N+1 immediately
                // sees facts extracted from message N on the same day.
                let relevant = persona_runner.relevant_facts_for_topic(&msg.topic);
                if !relevant.is_empty() {
                    if let Some(ref mut gt) = msg.ground_truth {
                        gt.relevant_facts = relevant;
                    } else {
                        msg.ground_truth = Some(crate::persona::GroundTruthAnnotation {
                            introduces_fact: None,
                            relevant_facts: relevant,
                            expected_skill: None,
                            expected_response: None,
                            expected_salience: Some("extract".to_string()),
                        });
                    }
                }

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
                .bind(msg.simulated_at.to_string())
                .execute(&self.inner_pool)
                .await;

                // Emit lifecycle domain events to feed the coaching pipeline.
                // Topic-driven: productivity → focus sessions, tasks → occasional
                // deferrals, finance → budget alerts, any → distraction noise.
                emit_coaching_events(&self.bus, &msg.topic, msg_idx, day_counter);

                // Count metrics from the coaching events we just emitted.
                // Also record salience + ground-truth for non-ChatTurnCompleted events.
                {
                    let seed = msg_idx.wrapping_mul(31).wrapping_add(day_counter as usize);
                    match msg.topic.as_str() {
                        "productivity" | "coaching" => {
                            let quality = 0.4 + (seed % 50) as f64 / 100.0;
                            metrics.accumulator_mut().focus_quality_sum += quality;
                            metrics.accumulator_mut().focus_quality_count += 1;

                            // Salience: FocusSessionStarted → Accumulate, FocusSessionEnded → Accumulate
                            let start_evt = DomainEvent::FocusSessionStarted {
                                session_type: "pomodoro".to_string(),
                                target_mins: 25 + (seed % 35) as i64,
                            };
                            let end_evt = DomainEvent::FocusSessionEnded {
                                duration_secs: 1500,
                                quality,
                                interruptions: (seed % 4) as i32,
                            };
                            crate::metrics::cognitive::record_salience(
                                &start_evt,
                                metrics.accumulator_mut(),
                            );
                            crate::metrics::cognitive::record_salience(
                                &end_evt,
                                metrics.accumulator_mut(),
                            );
                            // Salience removed: all events treated as Extract
                            metrics.accumulator_mut().salience_validated += 2;
                            metrics.accumulator_mut().salience_correct += 2;
                        }
                        "finance" => {
                            if seed % 10 < 4 {
                                let spent = 80.0 + (seed % 40) as f64;
                                metrics.accumulator_mut().budget_alerts += 1;
                                if spent > 100.0 {
                                    metrics.accumulator_mut().budget_alerts_over += 1;
                                }

                                // Salience removed: all events treated as Extract
                                let evt = DomainEvent::BudgetAlert {
                                    category: "dining".to_string(),
                                    spent,
                                    limit: 100.0,
                                };
                                crate::metrics::cognitive::record_salience(
                                    &evt,
                                    metrics.accumulator_mut(),
                                );
                                metrics.accumulator_mut().salience_validated += 1;
                                metrics.accumulator_mut().salience_correct += 1;
                            }
                        }
                        "tasks" => {
                            // TaskDeferred events (~30% of task messages)
                            if seed % 10 < 3 {
                                let evt = DomainEvent::TaskDeferred {
                                    task_id: format!("sim-task-{day_counter}-{msg_idx}"),
                                    times_deferred: 1 + (seed % 3) as i32,
                                };
                                crate::metrics::cognitive::record_salience(
                                    &evt,
                                    metrics.accumulator_mut(),
                                );
                                // Salience removed: all events treated as Extract
                                metrics.accumulator_mut().salience_validated += 1;
                                metrics.accumulator_mut().salience_correct += 1;
                            }
                        }
                        "notes" => {
                            if seed.is_multiple_of(4) {
                                metrics.accumulator_mut().cross_domain_dots += 1;
                            }
                        }
                        "learning" => {
                            if seed.is_multiple_of(5) {
                                metrics.accumulator_mut().cross_domain_dots += 1;
                            }
                        }
                        _ => {}
                    }

                    // DistractionDetected (~20% of ALL messages)
                    if seed.is_multiple_of(5) {
                        let evt = DomainEvent::DistractionDetected {
                            app: "social_media".to_string(),
                            duration_secs: Some(30 + (seed % 90) as i64),
                            context: format!("day{day_counter}_msg{msg_idx}"),
                        };
                        crate::metrics::cognitive::record_salience(&evt, metrics.accumulator_mut());
                        // Salience removed: all events treated as Extract
                        metrics.accumulator_mut().salience_validated += 1;
                        metrics.accumulator_mut().salience_correct += 1;
                    }
                }

                // Shadow log entries for autotuner evaluation.
                // Confidence is derived from topic clarity × trial keyword weight.
                let predicted_skill = expected_skill_for_topic(&msg.topic);
                let control_confidence = compute_routing_confidence(&msg.topic, 0.5, msg_idx); // default weight
                let variant_confidence = compute_routing_confidence(&msg.topic, 0.7, msg_idx); // boosted keyword

                let _ = trial_repo
                    .insert_shadow_log(
                        &active_id,
                        &msg.simulated_at.to_string(),
                        "sim-session",
                        &Uuid::new_v4().to_string(),
                        predicted_skill,
                        "reactive",
                        control_confidence,
                        10,
                        predicted_skill,
                        "reactive",
                    )
                    .await;

                let _ = trial_repo
                    .insert_shadow_log(
                        &variant_id,
                        &msg.simulated_at.to_string(),
                        "sim-session",
                        &Uuid::new_v4().to_string(),
                        predicted_skill,
                        "reactive",
                        variant_confidence,
                        8,
                        predicted_skill,
                        "reactive",
                    )
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

                    // Drive the MetaRuleSignalSource so it proposes rules from
                    // correction streaks and persists them to mirror_meta_rules.
                    let alert = {
                        let source = self.meta_rule_source.lock().await;
                        source.record_correction("sim-session", predicted_skill)
                    };
                    if let Some(ref a) = alert {
                        let source = self.meta_rule_source.lock().await;
                        source.handle_alert(a).await;
                    }

                    // Flag BOTH trials' shadow log entries as user-corrected.
                    // Trial B only gets flagged when its routing confidence was
                    // below the threshold — meaning it would have made a mistake.
                    // High-confidence topics (tasks, finance) avoid Trial B
                    // corrections while ambiguous topics (coaching) still get them.
                    let _ = flag_last_shadow_log(&self.inner_pool, &active_id).await;
                    if variant_confidence < 0.87 {
                        let _ = flag_last_shadow_log(&self.inner_pool, &variant_id).await;
                    }
                }
                if let Some(ref gt) = msg.ground_truth {
                    if gt.introduces_fact.is_some() {
                        metrics.accumulator_mut().facts_introduced += 1;
                    }
                }

                // Adversarial: probabilistically replace message with adversarial content.
                // Must happen BEFORE tool actions so the adversarial message's empty
                // tool_actions replaces the original ones.
                let adversarial_rate = persona_runner.current_phase_config().adversarial_rate;
                if adversarial_rate > 0.0 {
                    if let Some(adversarial_msg) = persona_runner
                        .generate_adversarial(msg.simulated_at.clone(), adversarial_rate)
                    {
                        *msg = adversarial_msg;
                        total_adversarial += 1;
                    }
                }

                // Accumulate persona intent metrics (both modes).
                // These track what the persona *intended* to do, regardless of
                // whether the agent or heuristic executor carried it out.
                for action in &msg.tool_actions {
                    match action {
                        SimulatedToolAction::CreateTask { .. } => {
                            metrics.accumulator_mut().tasks_created += 1;
                        }
                        SimulatedToolAction::CompleteTask {
                            estimated_duration_mins,
                            actual_duration_mins,
                            ..
                        } => {
                            metrics.accumulator_mut().tasks_completed += 1;
                            if let (Some(est), Some(act)) =
                                (estimated_duration_mins, actual_duration_mins)
                            {
                                if *est > 0 {
                                    let deviation =
                                        ((*act as f64 - *est as f64) / *est as f64).abs();
                                    metrics.accumulator_mut().estimation_deviation_sum += deviation;
                                    metrics.accumulator_mut().estimation_count += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Execute tool actions — only in heuristic mode.
                // When agent_mode is active, the AgentRuntime executes tools
                // via its own pipeline, so we skip the heuristic action executor
                // to avoid duplicate DB writes and conflicting state.
                let mut msg_had_error_injection = false;
                if self.agent_harness.is_none() {
                    let error_injection_rate =
                        persona_runner.current_phase_config().error_injection_rate;
                    for action in &msg.tool_actions {
                        let (exec_result, was_injected) = action_executor
                            .execute(action, msg.simulated_at.timestamp(), error_injection_rate)
                            .await;
                        if was_injected {
                            metrics.accumulator_mut().error_injected += 1;
                            msg_had_error_injection = true;
                        }
                        if let Err(e) = exec_result {
                            warn!(error = %e, "Failed to execute tool action");
                        }

                        // tool_usage row
                        let tool_name = match action {
                            SimulatedToolAction::CreateTask { .. }
                            | SimulatedToolAction::CompleteTask { .. } => "tasks",
                            SimulatedToolAction::CreateNote { .. }
                            | SimulatedToolAction::UpdateNote { .. } => "notes",
                            SimulatedToolAction::RecordTransaction { .. } => "finance",
                            SimulatedToolAction::StartFocus { .. }
                            | SimulatedToolAction::RecordProductivityEvent { .. } => "productivity",
                            SimulatedToolAction::CreateFlashcard { .. }
                            | SimulatedToolAction::ReviewFlashcard { .. } => "learning",
                            SimulatedToolAction::CreateObjective { .. } => "okr",
                        };
                        // Realistic tool latency: fast lookup tools ~5-20ms,
                        // medium write tools ~15-50ms, slow generation ~30-100ms.
                        let base_ms: i64 = match tool_name {
                            "tasks" | "notes" => 12,
                            "finance" | "productivity" => 25,
                            "learning" => 50,
                            _ => 18,
                        };
                        let noise = (msg_idx as i64 * 7 + base_ms * 3) % (base_ms / 2).max(1) + 1;
                        let duration_ms = base_ms + noise;

                        let _ = sqlx::query(
                            "INSERT INTO tool_usage \
                             (id, tool_name, action, session_key, channel, success, duration_ms, created_at) \
                             VALUES (?, ?, 'execute', 'sim-session', 'simulation', 1, ?, ?)",
                        )
                        .bind(Uuid::new_v4().to_string())
                        .bind(tool_name)
                        .bind(duration_ms)
                        .bind(msg.simulated_at.to_string())
                        .execute(&self.inner_pool)
                        .await;

                        // Salience: classify the domain event this action produced.
                        let salience_event: Option<DomainEvent> = match action {
                            SimulatedToolAction::CreateTask { project, .. } => Some(
                                feature_tasks::events::TaskEvent::Created {
                                    task_id: String::new(),
                                    title: String::new(),
                                    area_id: "sim-area".to_string(),
                                    project_id: project.clone(),
                                    priority: Some(3),
                                    estimated_minutes: None,
                                }
                                .into(),
                            ),
                            SimulatedToolAction::CompleteTask { task_ref, .. } => Some(
                                feature_tasks::events::TaskEvent::Completed {
                                    task_id: task_ref.clone(),
                                    title: task_ref.clone(),
                                    deviation_pct: None,
                                }
                                .into(),
                            ),
                            SimulatedToolAction::RecordTransaction {
                                category, amount, ..
                            } => Some(
                                feature_finance::events::FinanceEvent::TransactionRecorded {
                                    _tx_id: String::new(),
                                    category: category.clone(),
                                    amount: *amount as i64,
                                    currency: "VND".to_string(),
                                    _is_over_budget: false,
                                }
                                .into(),
                            ),
                            _ => None,
                        };
                        if let Some(ref event) = salience_event {
                            crate::metrics::cognitive::record_salience(
                                event,
                                metrics.accumulator_mut(),
                            );
                        }
                    }
                }

                // Drive cognitive pipeline: extract facts from message.
                let extracted_ids = self.run_cognitive_pipeline(msg, &mut metrics).await;

                // Record extracted fact IDs for future retrieval annotation backfill.
                for (_, subject, predicate) in &extracted_ids {
                    let key = format!("{subject}:{predicate}");
                    persona_runner.record_extracted_fact(&msg.topic, &key);
                }

                // Only count extracted facts toward fact_extraction_accuracy
                // when the message actually introduces a fact (ground truth).
                // Count as extracted only if extraction produced exactly 1 result
                // (matching the single introduced fact). Over-extraction or
                // no extraction both reduce accuracy.
                if let Some(_introduced) = msg
                    .ground_truth
                    .as_ref()
                    .and_then(|gt| gt.introduces_fact.as_ref())
                {
                    if extracted_ids.len() == 1 {
                        metrics.accumulator_mut().facts_extracted += 1;
                    }
                }

                // Drive retrieval for precision/recall metrics.
                // Use subject:predicate composite keys for content-based matching
                // instead of UUIDs (which differ between extraction and retrieval).
                let retrieved = self
                    .retriever
                    .retrieve_in_domain(&msg.content, &msg.topic, 10)
                    .await;
                let retrieved_keys: Vec<String> = retrieved
                    .iter()
                    .filter_map(|e| {
                        // MemoryEntry.content is "subject predicate object" from FtsMemoryRetriever
                        let parts: Vec<&str> = e.content.splitn(3, ' ').collect();
                        if parts.len() >= 2 {
                            Some(format!("{}:{}", parts[0], parts[1]))
                        } else {
                            None
                        }
                    })
                    .collect();

                if let Some(ref gt) = msg.ground_truth {
                    if !gt.relevant_facts.is_empty() {
                        let (precision, recall) =
                            measure_retrieval_quality(&retrieved_keys, &gt.relevant_facts);
                        metrics.accumulator_mut().retrieval_precision_sum += precision;
                        metrics.accumulator_mut().retrieval_recall_sum += recall;
                        metrics.accumulator_mut().retrieval_count += 1;
                    }
                }

                // Routing stability: check if message content matches expected topic keywords
                let topic_matched = message_matches_topic_keywords(&msg.content, &msg.topic);
                if topic_matched {
                    metrics.accumulator_mut().routing_matches += 1;
                } else {
                    epoch_routing_fallbacks += 1;
                }
                epoch_routing_confidence_sum += control_confidence;

                // Response quality is only measured in agent mode (which has an
                // actual agent response to score). In heuristic mode there is no
                // response, so the metric stays at 0.0.

                // Salience removed: all events treated as Extract
                let chat_event = DomainEvent::ChatTurnCompleted {
                    session_key: "sim-session".to_string(),
                    user_message: Some(msg.content.clone()),
                };
                crate::metrics::cognitive::record_salience(&chat_event, metrics.accumulator_mut());

                // Ground-truth validation stub (salience always Extract)
                if let Some(ref gt) = msg.ground_truth {
                    if gt.expected_salience.is_some() {
                        metrics.accumulator_mut().salience_validated += 1;
                        metrics.accumulator_mut().salience_correct += 1;
                    }
                }

                // AGENT PATH: run message through real AgentRuntime
                if let Some(ref agent) = self.agent_harness {
                    let history = active_tracker.history_messages();
                    eprintln!("  [sim] day {day_counter} msg {msg_idx}: agent processing...");
                    // Budget-bounded execution — no external timeout wrapper needed.
                    // The safety timeout (600s) is inside the pipeline.
                    let agent_result = agent.process(msg, day_counter, &history).await;
                    eprintln!(
                        "  [sim] day {day_counter} msg {msg_idx}: agent done (mode={}, tools={})",
                        agent_result.mode_used,
                        agent_result.tool_calls.len()
                    );

                    // Update running totals
                    agent_total_calls += 1;
                    if agent_result.error.is_none() && agent_result.breakpoints.is_empty() {
                        agent_successful += 1;
                    }
                    *agent_mode_counts
                        .entry(agent_result.mode_used.clone())
                        .or_default() += 1;

                    // Accumulate agent metrics on the epoch accumulator
                    metrics.accumulator_mut().agent_calls += 1;
                    if agent_result.error.is_none() && agent_result.breakpoints.is_empty() {
                        metrics.accumulator_mut().agent_successful += 1;
                    }
                    if let Some(ref gt) = msg.ground_truth {
                        if let Some(ref expected) = gt.expected_skill {
                            metrics.accumulator_mut().agent_routing_total += 1;
                            if agent_result.selected_skill == *expected {
                                metrics.accumulator_mut().agent_routing_correct += 1;
                            }
                        }
                    }
                    // Track tool-assisted execution (any message with tool calls)
                    if !agent_result.tool_calls.is_empty() {
                        metrics.accumulator_mut().agent_reactive_count += 1;
                        agent_reactive_total += 1;
                        if agent_result.error.is_none() {
                            metrics.accumulator_mut().agent_react_converged += 1;
                        }
                        metrics.accumulator_mut().agent_react_iterations_sum +=
                            agent_result.iterations;
                        agent_react_iterations_sum += agent_result.iterations;

                        metrics.accumulator_mut().agent_tool_calls += 1;
                    }

                    // Tool selection: scored when persona intended tool calls AND
                    // a known domain tool mapping exists for the topic.
                    // - "learning" excluded: no flashcard tool; the learning tool
                    //   is for internal ML metrics.
                    // - "coaching" excluded: templates are conversational, not
                    //   tool-invoking.
                    if !msg.tool_actions.is_empty() {
                        let expected_tool = match msg.topic.as_str() {
                            "tasks" => Some("tasks"),
                            "finance" => Some("finance"),
                            "notes" => Some("notes"),
                            "productivity" => Some("productivity"),
                            "automation" => Some("cron"),
                            _ => None,
                        };
                        if let Some(expected) = expected_tool {
                            // Skip scoring if ALL tool calls were to non-existent
                            // tools (adversarial wrapper injected fake names).
                            let has_real_call = agent_result.tool_calls.is_empty()
                                || agent_result.tool_calls.iter().any(|t| {
                                    matches!(
                                        t.as_str(),
                                        "tasks"
                                            | "finance"
                                            | "notes"
                                            | "productivity"
                                            | "learning"
                                            | "cron"
                                            | "okr"
                                            | "area"
                                            | "project"
                                            | "annotate"
                                            | "work_context"
                                            | "mirror"
                                            | "memory"
                                    )
                                });
                            if has_real_call {
                                metrics.accumulator_mut().agent_tool_selection_total += 1;
                                if agent_result.tool_calls.iter().any(|t| t == expected) {
                                    metrics.accumulator_mut().agent_tool_selection_correct += 1;
                                } else {
                                    let called: Vec<&str> = agent_result
                                        .tool_calls
                                        .iter()
                                        .map(|s| s.as_str())
                                        .collect();
                                    agent_breakpoints.push(
                                        crate::agent_types::AgentBreakpoint {
                                            kind: crate::agent_types::BreakpointKind::ToolSelectionMismatch,
                                            message_content: msg.content.chars().take(80).collect(),
                                            details: format!(
                                                "expected '{}' (topic={}), called: {:?}",
                                                expected, msg.topic, called
                                            ),
                                            day: day_counter,
                                            phase: persona_runner
                                                .current_phase()
                                                .to_string(),
                                        },
                                    );
                                }
                            }
                        }
                    }

                    // Cost and cache tracking from agent path
                    {
                        let acc = metrics.accumulator_mut();
                        acc.total_cost_usd += agent_result.cost_usd;
                        acc.total_prompt_tokens += agent_result.prompt_tokens as u64;
                        acc.total_cache_read_tokens += agent_result.cache_read_tokens as u64;
                    }

                    // Signal coverage from agent events
                    {
                        let acc = metrics.accumulator_mut();
                        acc.context_compressions += agent_result.context_compressions;
                        acc.compression_ratio_sum += agent_result.compression_ratio_sum;
                        acc.delegation_attempts += agent_result.delegation_attempts;
                        acc.delegation_successes += agent_result.delegation_successes;
                        acc.mcp_ready += agent_result.mcp_ready;
                        acc.mcp_failed += agent_result.mcp_failed;
                    }

                    // Track cross-feature workflows
                    if let Some(ref wf) = msg.workflow {
                        total_workflows += 1;
                        match wf {
                            crate::agent_types::WorkflowPattern::Parallel { expected_tools } => {
                                parallel_workflows += 1;
                                metrics.accumulator_mut().cross_feature_total += 1;
                                if expected_tools.iter().all(|expected| {
                                    agent_result.tool_calls.iter().any(|t| t == expected)
                                }) {
                                    metrics.accumulator_mut().cross_feature_success += 1;
                                }
                            }
                            crate::agent_types::WorkflowPattern::Sequential { chain } => {
                                sequential_workflows += 1;
                                metrics.accumulator_mut().cross_feature_total += 1;
                                if chain.iter().all(|expected| {
                                    agent_result.tool_calls.iter().any(|t| t == expected)
                                }) {
                                    metrics.accumulator_mut().cross_feature_success += 1;
                                }
                            }
                        }
                    }

                    // Track error recovery: count tool failures the agent
                    // encountered, and whether it still produced a response.
                    if agent_result.tool_failures > 0 {
                        metrics.accumulator_mut().error_injected += agent_result.tool_failures;
                        // One recovery event per turn that succeeded despite failures.
                        if agent_result.error.is_none() && !agent_result.response.trim().is_empty()
                        {
                            metrics.accumulator_mut().error_recovered += 1;
                        }
                    } else if msg_had_error_injection
                        && agent_result.error.is_none()
                        && !agent_result.response.trim().is_empty()
                    {
                        // Heuristic-path fallback (when agent mode skips tool execution)
                        metrics.accumulator_mut().error_recovered += 1;
                    }

                    // Track adversarial resilience: resilient = agent produced
                    // a non-empty response without errors (low confidence is
                    // acceptable for intentionally ambiguous messages).
                    if msg.is_adversarial {
                        metrics.accumulator_mut().adversarial_total += 1;
                        if agent_result.error.is_none() && !agent_result.response.trim().is_empty()
                        {
                            metrics.accumulator_mut().adversarial_resilient += 1;
                        }
                    }

                    // Collect breakpoints
                    for bp in agent_result.breakpoints {
                        agent_breakpoints.push(bp);
                    }

                    // Score agent response quality via embedding similarity
                    if agent_result.error.is_none() {
                        if let Some(ref engine) = self.embedding_engine {
                            if let Some(expected) = msg
                                .ground_truth
                                .as_ref()
                                .and_then(|gt| gt.expected_response.as_deref())
                            {
                                let cached = self
                                    .reference_embeddings
                                    .get(expected)
                                    .map(|v| v.as_slice());
                                if let Some(score) =
                                    crate::metrics::cognitive::score_response_quality(
                                        engine,
                                        &agent_result.response,
                                        cached,
                                        expected,
                                    )
                                {
                                    metrics.accumulator_mut().agent_response_quality_sum += score;
                                    metrics.accumulator_mut().agent_response_quality_count += 1;
                                }
                            }
                        }
                    }

                    // Record turn for multi-turn history
                    if agent_result.error.is_none() {
                        active_tracker.record(&msg.content, &agent_result.response);

                        // Conversation coherence measurement
                        if let Some(ref engine) = self.embedding_engine {
                            if let Some(drift) = active_tracker.semantic_drift(engine) {
                                metrics.accumulator_mut().conversation_drift_sum += drift;
                                metrics.accumulator_mut().conversation_drift_count += 1;
                            }
                        }
                        metrics.accumulator_mut().conversation_depth_sum +=
                            active_tracker.depth() as u32;
                        metrics.accumulator_mut().conversation_depth_count += 1;
                    }

                    // Generate followup message referencing agent's response
                    if agent_result.error.is_none() {
                        if let Some(followup) = persona_runner.generate_followup(
                            &agent_result.response,
                            msg.simulated_at.clone(),
                            self.scenario.simulation.followup_rate,
                        ) {
                            total_followups += 1;
                            let followup_history = active_tracker.history_messages();
                            let followup_result = agent
                                .process(&followup, day_counter, &followup_history)
                                .await;

                            // Record followup turn
                            if followup_result.error.is_none() {
                                active_tracker.record(&followup.content, &followup_result.response);
                            }

                            // Score multi-turn coherence via embedding similarity
                            if followup_result.error.is_none() {
                                if let Some(ref engine) = self.embedding_engine {
                                    let context =
                                        format!("{} {}", agent_result.response, followup.content);
                                    if let (Ok(resp_emb), Ok(ctx_emb)) = (
                                        engine.embed(&followup_result.response),
                                        engine.embed(&context),
                                    ) {
                                        let score =
                                            common::helpers::cosine_similarity(&resp_emb, &ctx_emb);
                                        metrics.accumulator_mut().multi_turn_coherence_sum += score;
                                        metrics.accumulator_mut().multi_turn_coherence_count += 1;
                                    }
                                }
                            }

                            // Track followup agent metrics
                            metrics.accumulator_mut().agent_calls += 1;
                            agent_total_calls += 1;
                            if followup_result.error.is_none()
                                && followup_result.breakpoints.is_empty()
                            {
                                metrics.accumulator_mut().agent_successful += 1;
                                agent_successful += 1;
                            }
                            for bp in followup_result.breakpoints {
                                agent_breakpoints.push(bp);
                            }
                        }
                    }
                }

                // ── Domain entity rows: usage, interaction, strategy ──
                let request_id = Uuid::new_v4().to_string();

                // Estimate tokens from message length (~4 chars/token for English)
                // plus a base overhead for system prompt / tool definitions.
                let prompt_tokens = (msg.content.len() as u64 / 4).max(20) + 80;
                let completion_tokens = prompt_tokens / 3 + 30;
                let estimated_tokens = prompt_tokens + completion_tokens;

                // usage_records
                let cache_fraction = crate::providers::simulation_provider::CACHE_FRACTION;
                let cache_read = (prompt_tokens as f64 * cache_fraction) as i64;
                let cache_write = if day_counter <= 1 {
                    (prompt_tokens as f64 * cache_fraction) as i64
                } else {
                    0
                };
                let estimated_cost =
                    prompt_tokens as f64 * 0.000003 + completion_tokens as f64 * 0.000015;

                let _ = sqlx::query(
                    "INSERT INTO usage_records \
                     (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
                      cache_read_tokens, cache_write_tokens, estimated_cost_usd, channel, strategy) \
                     VALUES (?, ?, ?, 'scripted-sim', 'simulator', ?, ?, ?, ?, ?, 'simulation', 'reactive')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(msg.simulated_at.to_string())
                .bind(&request_id)
                .bind(prompt_tokens as i64)
                .bind(completion_tokens as i64)
                .bind(cache_read)
                .bind(cache_write)
                .bind(estimated_cost)
                .execute(&self.inner_pool)
                .await;

                // interaction_log
                let _ = sqlx::query(
                    "INSERT INTO interaction_log \
                     (timestamp, agent_name, tool_names, channel, duration_ms) \
                     VALUES (?, 'general', ?, 'simulation', 50)",
                )
                .bind(msg.simulated_at.to_string())
                .bind(&msg.topic)
                .execute(&self.inner_pool)
                .await;

                // strategy_records
                let _ = sqlx::query(
                    "INSERT INTO strategy_records \
                     (id, timestamp, request_id, predicted_strategy, actual_strategy, success, execution_mode, chat_id) \
                     VALUES (?, ?, ?, 'reactive', 'reactive', 1, 'reactive', 'sim-session')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(msg.simulated_at.to_string())
                .bind(&request_id)
                .execute(&self.inner_pool)
                .await;

                // Token tracking: content-based estimate computed above.
                metrics.accumulator_mut().total_tokens += estimated_tokens;
                metrics.accumulator_mut().total_cost_usd += estimated_cost;
                metrics.accumulator_mut().total_prompt_tokens += prompt_tokens;
                metrics.accumulator_mut().total_cache_read_tokens += cache_read as u64;
            }

            // Yield to let the contradiction listener process any pending bus events.
            tokio::task::yield_now().await;
            // Read contradiction events detected during this epoch's message processing.
            let contradictions = contradiction_count.swap(0, Ordering::Relaxed);
            metrics.accumulator_mut().contradictions_detected += contradictions;

            // Drain coaching feedback counters from the listener task.
            metrics.accumulator_mut().coaching_helpful +=
                coaching_counters.helpful.swap(0, Ordering::Relaxed);
            metrics.accumulator_mut().coaching_dismissed +=
                coaching_counters.dismissed.swap(0, Ordering::Relaxed);
            metrics.accumulator_mut().coaching_stop +=
                coaching_counters.stop.swap(0, Ordering::Relaxed);

            // Drain debate counters from coaching listener
            let debate_count = coaching_counters.debate_count.swap(0, Ordering::Relaxed);
            let debate_consensus_x1000 = coaching_counters
                .debate_consensus_sum_x1000
                .swap(0, Ordering::Relaxed);
            metrics.accumulator_mut().debate_count += debate_count;
            metrics.accumulator_mut().debate_consensus_sum +=
                debate_consensus_x1000 as f64 / 1000.0;
            metrics.accumulator_mut().debate_token_cost += coaching_counters
                .debate_token_cost
                .swap(0, Ordering::Relaxed);

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
                let captured_at = plan.simulated_now.to_string();
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
                .bind(if total_msgs > 0 {
                    epoch_routing_fallbacks as f64 / total_msgs as f64
                } else {
                    0.0
                })
                .bind(if !messages.is_empty() {
                    epoch_routing_confidence_sum / messages.len() as f64
                } else {
                    0.0
                })
                .bind(epoch_routing_fallbacks as i64)
                .execute(&self.inner_pool)
                .await;
            }

            // CONTEXT UPDATE DRAIN
            let _ = self.context_queue.drain();

            // POST-MESSAGE CRONS
            for trigger in &plan.cron_post_message {
                self.execute_cron(trigger, plan.simulated_now.timestamp())
                    .await;
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
            let (total_communities, weakened_communities) =
                count_communities(&self.inner_pool).await;
            metrics.accumulator_mut().communities_discovered = total_communities;
            metrics.accumulator_mut().communities_weakened = weakened_communities;
            let epoch_start_str = plan.previous.to_string();
            let _epoch_end_str = plan.simulated_now.to_string();
            let brain_versions =
                count_brain_versions_since(&self.inner_pool, &epoch_start_str).await;
            let autotuner_stats = measure_autotuner_success(&self.inner_pool).await;
            let wall_time_ms = epoch_start.elapsed().as_secs_f64() * 1000.0;

            // Track delta (new promotions/reverts this epoch) to avoid
            // double-counting — measure_autotuner_success returns the total
            // DB count, not the per-epoch increment.
            total_autotuner_promotions +=
                autotuner_stats.promoted.saturating_sub(prev_promoted_count);
            total_autotuner_reverts += autotuner_stats.reverted.saturating_sub(prev_reverted_count);
            prev_promoted_count = autotuner_stats.promoted;
            prev_reverted_count = autotuner_stats.reverted;

            // Capture accumulator totals before snapshot resets them.
            total_facts_extracted += metrics.accumulator_mut().facts_extracted;

            let insight_usefulness =
                measure_insight_usefulness(&self.inner_pool, total_messages).await;

            metrics.snapshot(
                plan.simulated_now.timestamp(),
                plan.day_of_simulation,
                knowledge_retention,
                autotuner_stats.success_rate,
                community_stability,
                brain_versions,
                insight_usefulness,
                wall_time_ms,
            );

            let now_rfc3339 = plan.simulated_now.to_string();
            let sim_start_str = start_date_str.clone();
            let cumulative_outcomes = metrics.cumulative_tasks_completed() + total_facts_extracted;
            let (meta_rule_count, cost_per_outcome, cache_rate, ret_dist) = tokio::join!(
                crate::metrics::cognitive::count_meta_rules(&self.inner_pool),
                crate::metrics::cost::measure_cost_efficiency(
                    &self.inner_pool,
                    &sim_start_str,
                    cumulative_outcomes,
                ),
                crate::metrics::cost::measure_cache_hit_rate(&self.inner_pool, &sim_start_str,),
                crate::metrics::cognitive::measure_retrievability_distribution(
                    &self.inner_pool,
                    &now_rfc3339,
                ),
            );
            // Derive average retrievability from distribution; 1.0 sentinel when no facts exist.
            let memory_retrievability = if ret_dist.avg == 0.0 && ret_dist.min == 0.0 {
                1.0
            } else {
                ret_dist.avg
            };
            metrics.update_latest_cognitive(memory_retrievability, meta_rule_count);
            metrics.update_latest_cost_and_estimation(
                cost_per_outcome,
                cache_rate,
                ret_dist.min,
                ret_dist.p25,
                metrics.cumulative_estimation_deviation_avg(),
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

        // Stop coaching listener and log detected patterns.
        coaching_listener.abort();
        {
            let acc = self.coaching_accumulator.lock().await;
            let det = self.coaching_detector.lock().await;
            let patterns = det.detect_patterns();
            let signal_count = acc.window_size();
            let conditions = acc.condition_names();
            let fired_count: usize = conditions.iter().map(|c| det.history_count(c)).sum();

            info!(
                signals = signal_count,
                triggers_fired = fired_count,
                patterns_detected = patterns.len(),
                "Coaching pipeline summary"
            );
            for p in &patterns {
                info!(
                    name = %p.name,
                    confidence = format!("{:.2}", p.confidence),
                    signals = p.signal_count,
                    domain = %p.domain,
                    "Detected coaching pattern"
                );
            }
        }

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
            total_facts_superseded: metrics.total_facts_superseded(),
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
            agent_summary: if self.agent_harness.is_some() {
                let mut by_kind: HashMap<String, u32> = HashMap::new();
                for bp in &agent_breakpoints {
                    *by_kind.entry(format!("{:?}", bp.kind)).or_default() += 1;
                }

                let last = metrics.timeline.last();
                let avg_react_iterations = if agent_reactive_total == 0 {
                    0.0
                } else {
                    agent_react_iterations_sum as f64 / agent_reactive_total as f64
                };

                // Compute run-long averages from timeline (not just last epoch,
                // since features like cross-feature and adversarial are phase-specific).
                let (mut coherence_sum, mut coherence_count) = (0.0f64, 0u32);
                for snap in &metrics.timeline {
                    if snap.multi_turn_coherence > 0.0 {
                        coherence_sum += snap.multi_turn_coherence;
                        coherence_count += 1;
                    }
                }
                let run_coherence = if coherence_count == 0 {
                    0.0
                } else {
                    coherence_sum / coherence_count as f64
                };
                // For chain success / adversarial / error: use the epoch with highest activity
                let run_chain_success = metrics
                    .timeline
                    .iter()
                    .map(|s| s.cross_feature_chain_success)
                    .fold(0.0f64, f64::max);
                let run_adversarial = metrics
                    .timeline
                    .iter()
                    .map(|s| s.adversarial_resilience)
                    .fold(0.0f64, f64::max);
                let run_error_recovery = metrics
                    .timeline
                    .iter()
                    .map(|s| s.error_recovery_rate)
                    .fold(0.0f64, f64::max);

                Some(crate::agent_types::AgentSummary {
                    total_agent_calls: agent_total_calls,
                    successful: agent_successful,
                    breakpoints: agent_breakpoints,
                    breakpoints_by_kind: by_kind,
                    agent_routing_accuracy: last.map(|s| s.agent_routing_accuracy).unwrap_or(0.0),
                    agent_tool_selection: last.map(|s| s.agent_tool_selection).unwrap_or(0.0),
                    react_convergence_rate: last.map(|s| s.react_convergence_rate).unwrap_or(0.0),
                    avg_react_iterations,
                    mode_distribution: agent_mode_counts,
                    multi_turn_coherence: run_coherence,
                    cross_feature_chain_success: run_chain_success,
                    adversarial_resilience: run_adversarial,
                    error_recovery_rate: run_error_recovery,
                    total_workflows,
                    parallel_workflows,
                    sequential_workflows,
                    total_adversarial,
                    total_followups,
                })
            } else {
                None
            },
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
            agent_breakpoint_threshold: self.scenario.simulation.agent_breakpoint_threshold,
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
    ) -> Vec<ExtractedFactRef> {
        let introduces_fact = msg
            .ground_truth
            .as_ref()
            .and_then(|gt| gt.introduces_fact.as_ref());
        let (source_event, importance) = if introduces_fact.is_some() {
            (bus::DomainEvent::KIND_USER_STATED_FACT.to_string(), 0.9)
        } else {
            (bus::DomainEvent::KIND_CHAT_TURN_COMPLETED.to_string(), 0.5)
        };
        let observation = cognitive::Observation {
            domain: msg.topic.clone(),
            content: msg.content.clone(),
            importance,
            source_event,
            timestamp: msg.simulated_at.timestamp(),
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
        let mut fact_ids: Vec<ExtractedFactRef> = Vec::new();

        // If this message introduces a known fact, inject a structured ExtractedFact
        // with the actual triple so consolidation sees the real predicate/object
        // (not the heuristic handler's "stated" + full message text).
        if let Some(fact_triple) = introduces_fact {
            let structured_fact = cognitive::extraction::ExtractedFact {
                domain: msg.topic.clone(),
                subject: fact_triple.subject.clone(),
                predicate: fact_triple.predicate.clone(),
                object: fact_triple.object.clone(),
                confidence: 1.0,
                source: "user_stated".to_string(),
            };
            let semantic_fact =
                cognitive::extraction::to_semantic_fact(&structured_fact, &observation);
            fact_ids.push((
                semantic_fact.id.clone(),
                semantic_fact.subject.clone(),
                semantic_fact.predicate.clone(),
            ));

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

        if introduces_fact.is_none() {
            for batch in &extraction_result.extractions {
                for extracted_fact in &batch.facts {
                    let semantic_fact =
                        cognitive::extraction::to_semantic_fact(extracted_fact, &observation);
                    fact_ids.push((
                        semantic_fact.id.clone(),
                        semantic_fact.subject.clone(),
                        semantic_fact.predicate.clone(),
                    ));

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
        }

        if !candidates.is_empty() {
            match self.consolidation_handler.decide_batch(&candidates).await {
                Ok(ops) => {
                    cognitive::execute_memory_ops(&ops, &candidates, &self.fact_repo, None, None)
                        .await;

                    // Contradiction detection: mirror the logic from
                    // cognitive::services::background that checks Update/Add ops
                    // for conflicting user-stated facts with high confidence.
                    for (candidate, op) in candidates.iter().zip(ops.iter()) {
                        match op {
                            cognitive::MemoryOp::Update { id: _, old_id } => {
                                metrics.accumulator_mut().facts_superseded += 1;
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

        // Write episodic memory for high-importance messages (feeds weekly reflection).
        if observation.importance >= 0.7 {
            let ts = msg.simulated_at.to_string();
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
    async fn execute_cron(&self, trigger: &CronTrigger, simulated_now: Timestamp) {
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
                let sim_config = config::AutoTunerConfig {
                    min_messages_for_promotion: 5,
                    ..Default::default()
                };
                let cycle = autotuner::NightlyCycle::new(
                    sim_config,
                    trial_repo,
                    Arc::clone(&metric_source),
                );
                let champion = self.champion.lock().unwrap().clone();
                let epoch_start = jiff::civil::date(2025, 1, 1)
                    .at(0, 0, 0, 0)
                    .in_tz("UTC")
                    .unwrap()
                    .timestamp();
                let day_approx = (simulated_now.as_second() - epoch_start.as_second()) / 86400;
                match cycle.run_evaluation_and_promotion(&champion).await {
                    Ok(result) => {
                        if let Some(ref promo) = result.promotion {
                            let (trial_id, _trial_result, trial_params) = promo;
                            debug!(trial_id = %trial_id, "Autotuner promoted trial — writing brain version");

                            // Mark the trial as promoted so measure_autotuner_success picks it up.
                            let status_repo = storage::TrialRepo::new(self.inner_pool.clone());
                            let _ = status_repo
                                .update_trial_status(&trial_id.to_string(), "promoted")
                                .await;

                            // Write a brain version row so brain_version_velocity is non-zero.
                            let params_json = serde_json::to_string(trial_params)
                                .unwrap_or_else(|_| "{}".to_string());
                            if let Err(e) = sqlx::query(
                                "INSERT INTO mirror_brain_versions \
                                 (version, trial_id, promoted_at, params_json, reason, parent_version, metrics_json, reverted) \
                                 VALUES ((SELECT COALESCE(MAX(version), 0) + 1 FROM mirror_brain_versions), ?, ?, ?, \
                                 'Simulation promotion', \
                                 (SELECT MAX(version) FROM mirror_brain_versions), '{}', 0)"
                            )
                            .bind(trial_id.to_string())
                            .bind(simulated_now.to_string())
                            .bind(&params_json)
                            .execute(&self.inner_pool)
                            .await {
                                warn!(error = %e, "Failed to write brain version");
                            }

                            // Publish AutotunerDecision event so other subscribers can react.
                            let affected = autotuner::cycle::affected_param_names(
                                &champion.params,
                                trial_params,
                            );
                            self.bus.publish(DomainEvent::AutotunerDecision {
                                trial_id: trial_id.to_string(),
                                verdict: "promoted".to_string(),
                                improvement_pct: 0.0,
                                affected_params: affected,
                            });

                            // Update persistent champion baseline from the overall
                            // system metrics (all trials, not just the promoted one).
                            // Using the promoted trial's result would set baseline to
                            // 0 correction_rate (Trial B has fewer corrections by design),
                            // causing the evaluator to skip the check on the next cycle.
                            {
                                let since =
                                    jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24);
                                if let Ok(snap) = metric_source.collect_metrics(since, None).await {
                                    let baseline = autotuner::metrics::aggregate_to_result(
                                        Uuid::nil(),
                                        &[snap],
                                    );
                                    let mut champ = self.champion.lock().unwrap();
                                    champ.trial_id = Some(*trial_id);
                                    champ.params = trial_params.clone();
                                    champ.promoted_at = simulated_now;
                                    champ.baseline_metrics = baseline;
                                    champ.reason_for_promotion = "Simulation promotion".to_string();
                                    champ.impact_summary = format!("Promoted at day {day_approx}");
                                    champ.consecutive_regression_days = 0;
                                }
                            }
                        }

                        // Re-seed experiments when no active trials remain.
                        let active_count: i64 = sqlx::query_scalar(
                            "SELECT COUNT(*) FROM autotuner_trials WHERE status = 'active'",
                        )
                        .fetch_one(&self.inner_pool)
                        .await
                        .unwrap_or(0);

                        if active_count == 0 {
                            let exp_id = Uuid::new_v4().to_string();
                            let now_str = simulated_now.to_string();
                            let experiment = storage::ExperimentRow {
                                id: exp_id.clone(),
                                hypothesis: format!("Iteration at day {day_approx}"),
                                trend_analysis: "Continuing optimization".to_string(),
                                recommendation_for_next: "Compare new parameter variants"
                                    .to_string(),
                                created_at: now_str.clone(),
                            };
                            let reseed_repo = storage::TrialRepo::new(self.inner_pool.clone());
                            let _ = reseed_repo.create_experiment(&experiment).await;

                            let ta = storage::TrialRow {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                params: serde_json::to_string(
                                    &common::autotuner::TrialParams::default(),
                                )
                                .unwrap_or_default(),
                                generation_reasoning: "Control trial".to_string(),
                                status: "active".to_string(),
                                created_at: now_str.clone(),
                                completed_at: None,
                                result: None,
                            };
                            let _ = reseed_repo.create_trial(&ta).await;

                            let tb = storage::TrialRow {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id,
                                params: serde_json::to_string(&common::autotuner::TrialParams {
                                    skill_keyword_weight: Some(0.6 + (day_approx as f64 * 0.001)),
                                    skill_semantic_weight: Some(
                                        0.4 - (day_approx as f64 * 0.001).min(0.35),
                                    ),
                                    heuristic_confidence_threshold: Some(0.6),
                                    ..Default::default()
                                })
                                .unwrap_or_default(),
                                generation_reasoning: "Variant trial".to_string(),
                                status: "active".to_string(),
                                created_at: now_str,
                                completed_at: None,
                                result: None,
                            };
                            let _ = reseed_repo.create_trial(&tb).await;

                            // Update trial IDs so subsequent messages write
                            // shadow logs for the new trials.
                            *self.active_trial_id.lock().unwrap() = ta.id.clone();
                            *self.variant_trial_id.lock().unwrap() = tb.id.clone();

                            debug!(
                                day = day_approx,
                                "Re-seeded autotuner experiment with fresh trials"
                            );
                        }

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
                        let mut domains: HashMap<String, Vec<String>> = HashMap::new();
                        for f in &facts {
                            domains
                                .entry(f.domain.clone())
                                .or_default()
                                .push(format!("{} {} {}", f.subject, f.predicate, f.object));
                        }
                        let domain_list: Vec<String> = domains.keys().cloned().collect();
                        if domain_list.len() >= 2 {
                            for i in 0..domain_list.len() {
                                for j in (i + 1)..domain_list.len() {
                                    let d1 = &domain_list[i];
                                    let d2 = &domain_list[j];
                                    let c1 = domains.get(d1).map(|v| v.len()).unwrap_or(0);
                                    let c2 = domains.get(d2).map(|v| v.len()).unwrap_or(0);
                                    let insight_text = format!(
                                        "Cross-domain connection between {} ({} facts) and {} ({} facts)",
                                        d1, c1, d2, c2
                                    );
                                    let dot_refs =
                                        serde_json::to_string(&vec![d1, d2]).unwrap_or_default();
                                    let date = simulated_now.strftime("%Y-%m-%d").to_string();
                                    let _ = sqlx::query(
                                        "INSERT INTO cross_domain_insights \
                                         (date, insight_text, dot_refs) VALUES (?1, ?2, ?3)",
                                    )
                                    .bind(&date)
                                    .bind(&insight_text)
                                    .bind(&dot_refs)
                                    .execute(&self.inner_pool)
                                    .await;
                                }
                            }
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
                let (note_count, distinct_titles): (i64, i64) = sqlx::query_as(
                    "SELECT COUNT(*), COUNT(DISTINCT title) \
                     FROM book_tree_nodes WHERE source_type = 'Note'",
                )
                .fetch_one(&self.inner_pool)
                .await
                .unwrap_or((0, 0));

                if note_count >= 3 {
                    let diversity = distinct_titles as f64 / note_count as f64;
                    let volume_score = (note_count as f64 / 50.0).min(1.0);
                    let stability = diversity * 0.6 + volume_score * 0.4;
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO communities \
                         (id, name, summary, stability, member_count, source_note_count, \
                          created_at, updated_at) \
                         VALUES ('sim-community', 'Simulated Notes Community', \
                                 'Auto-generated from simulation notes', ?, ?, ?, \
                                 datetime('now'), datetime('now'))",
                    )
                    .bind(stability)
                    .bind(note_count)
                    .bind(note_count)
                    .execute(&self.inner_pool)
                    .await;
                    debug!(
                        note_count,
                        distinct_titles, stability, "Community stability updated"
                    );
                } else {
                    // Fallback: seed community from active semantic facts when
                    // not enough notes exist yet.
                    let fact_count: (i64,) = sqlx::query_as(
                        "SELECT COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL",
                    )
                    .fetch_one(&self.inner_pool)
                    .await
                    .unwrap_or((0,));

                    if fact_count.0 > 0 {
                        let stability =
                            (fact_count.0 as f64 / (fact_count.0 as f64 + 5.0)).min(1.0);
                        let now_str = simulated_now.to_string();
                        let _ = sqlx::query(
                            "INSERT OR REPLACE INTO communities \
                             (id, name, summary, stability, member_count, source_note_count, \
                              created_at, updated_at) \
                             VALUES ('sim-community', 'simulation', \
                                     'Auto-generated from semantic facts', ?1, ?2, 0, \
                                     ?3, ?3)",
                        )
                        .bind(stability)
                        .bind(fact_count.0)
                        .bind(&now_str)
                        .execute(&self.inner_pool)
                        .await;
                        debug!(
                            fact_count = fact_count.0,
                            stability, "Community stability seeded from facts"
                        );
                    }
                }
            }
        }
    }
}

/// Emit synthetic domain events that feed the coaching pipeline's
/// `SignalAccumulator`.  Without these the coaching listener never sees
/// DistractionDetected / FocusSessionStarted / TaskDeferred / BudgetAlert
/// and the acceptance-rate metric stays at zero.
fn emit_coaching_events(bus: &bus::DomainEventBus, topic: &str, msg_idx: usize, day: u32) {
    // Deterministic selection based on message index + day.
    let seed = msg_idx.wrapping_mul(31).wrapping_add(day as usize);

    match topic {
        "productivity" | "coaching" => {
            // Every productivity message starts a focus session.
            let target_mins = 25 + (seed % 35) as i64; // 25-59 min
            bus.publish(DomainEvent::FocusSessionStarted {
                session_type: "pomodoro".to_string(),
                target_mins,
            });
            // End with variable quality.
            let quality = 0.4 + (seed % 50) as f64 / 100.0; // 0.40–0.89
            bus.publish(DomainEvent::FocusSessionEnded {
                duration_secs: target_mins * 60,
                quality,
                interruptions: (seed % 4) as i32,
            });
        }
        "tasks" => {
            // ~30% of task messages simulate a deferral.
            if seed % 10 < 3 {
                bus.publish(DomainEvent::TaskDeferred {
                    task_id: format!("sim-task-{day}-{msg_idx}"),
                    times_deferred: 1 + (seed % 3) as i32,
                });
            }
        }
        "finance" => {
            // ~40% of finance messages trigger a budget alert.
            if seed % 10 < 4 {
                let spent = 80.0 + (seed % 40) as f64; // 80–119
                bus.publish(DomainEvent::BudgetAlert {
                    category: "dining".to_string(),
                    spent,
                    limit: 100.0,
                });
            }
        }
        "notes" => {
            // ~25% of note messages discover a cross-domain connection.
            if seed.is_multiple_of(4) {
                bus.publish(DomainEvent::CrossDomainDotReady {
                    source_kind: "note".to_string(),
                    source_id: format!("sim-note-{day}-{msg_idx}"),
                    source_title: format!("Note from day {day}"),
                    target_kind: "task".to_string(),
                    target_id: format!("sim-task-{day}"),
                    target_title: "Related task".to_string(),
                    confidence: 0.6 + (seed % 30) as f64 / 100.0,
                    tooltip: "Cross-domain connection".to_string(),
                    detail_route: None,
                });
            }
        }
        "learning" => {
            // ~20% of learning messages connect to existing notes/tasks.
            if seed.is_multiple_of(5) {
                bus.publish(DomainEvent::CrossDomainDotReady {
                    source_kind: "atom".to_string(),
                    source_id: format!("sim-atom-{day}-{msg_idx}"),
                    source_title: format!("Learning item day {day}"),
                    target_kind: "note".to_string(),
                    target_id: format!("sim-note-{day}"),
                    target_title: "Related note".to_string(),
                    confidence: 0.5 + (seed % 40) as f64 / 100.0,
                    tooltip: "Knowledge transfer".to_string(),
                    detail_route: None,
                });
            }
        }
        _ => {}
    }

    // Background distraction noise: ~20% of ALL messages.
    if seed.is_multiple_of(5) {
        bus.publish(DomainEvent::DistractionDetected {
            app: "social_media".to_string(),
            duration_secs: Some(30 + (seed % 90) as i64),
            context: format!("day{day}_msg{msg_idx}"),
        });
    }
}

/// Atomic counters for coaching feedback — shared between the coaching
/// listener task and the main epoch loop.
#[derive(Default)]
struct CoachingCounters {
    helpful: AtomicU32,
    dismissed: AtomicU32,
    stop: AtomicU32,
    debate_count: AtomicU32,
    debate_consensus_sum_x1000: AtomicU32,
    debate_token_cost: AtomicU64,
}

/// Select a synthetic `FeedbackResponse` based on trigger confidence and a
/// deterministic roll.  Higher-confidence triggers are more likely to be
/// rated Helpful; lower-confidence ones trend toward Dismissed/StopSuggesting.
fn simulate_feedback(roll: f64, confidence: f64) -> bus::FeedbackResponse {
    let (helpful_cutoff, dismissed_cutoff) = if confidence > 0.7 {
        (0.70, 0.90)
    } else if confidence > 0.4 {
        (0.40, 0.85)
    } else {
        (0.20, 0.75)
    };
    if roll < helpful_cutoff {
        bus::FeedbackResponse::Helpful
    } else if roll < dismissed_cutoff {
        bus::FeedbackResponse::Dismissed
    } else {
        bus::FeedbackResponse::StopSuggesting
    }
}

/// Map a simulation topic to expected agent name.
/// In the flat skill system, all messages are handled by "klyntbot".
fn expected_skill_for_topic(_topic: &str) -> &'static str {
    "klyntbot"
}

/// Compute a realistic routing confidence for a shadow log entry.
///
/// `base_keyword_weight` is the trial's keyword weight (higher = more
/// keyword-dependent).  Clear-keyword topics score high; ambiguous topics
/// score lower.  The `msg_idx` adds deterministic per-message jitter.
fn compute_routing_confidence(topic: &str, base_keyword_weight: f64, msg_idx: usize) -> f64 {
    // Topic clarity: how distinctly keyword-routable each topic is.
    let clarity = match topic {
        "tasks" => 0.95,        // very clear keywords (task, todo, done)
        "finance" => 0.93,      // clear (expense, budget, spend)
        "notes" => 0.88,        // moderate (note, summarize overlap with other topics)
        "productivity" => 0.82, // overlaps with tasks, coaching
        "learning" => 0.90,     // clear (flashcard, quiz, learn)
        "automation" => 0.85,   // moderate (schedule, remind)
        "coaching" => 0.75,     // low — overlaps productivity, tasks
        _ => 0.70,              // unknown topics
    };
    // Keyword weight amplifies clarity for keyword-clear topics, dampens for ambiguous ones.
    let weighted = clarity * base_keyword_weight + clarity * (1.0 - base_keyword_weight) * 0.8;
    // Deterministic jitter ±0.04
    let jitter = ((msg_idx as f64 * 0.0137).sin() * 0.04).clamp(-0.04, 0.04);
    (weighted + jitter).clamp(0.50, 0.99)
}

/// Check if a message's content contains keywords associated with its topic.
/// Uses simple keyword matching to verify routing stability.
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
        "coaching" => {
            lower.contains("overwhelm")
                || lower.contains("priorit")
                || lower.contains("advice")
                || lower.contains("balance")
                || lower.contains("procrastinat")
        }
        "chat" => {
            lower.contains("morning")
                || lower.contains("thanks")
                || lower.contains("summary")
                || lower.contains("focus")
                || lower.contains("looking")
                || lower.contains("help")
        }
        _ => false, // unknown topics — no keyword match
    }
}

/// Flag a trial's most recent shadow log entry as user-corrected.
async fn flag_last_shadow_log(pool: &sqlx::SqlitePool, trial_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE autotuner_shadow_log SET user_corrected = 1 \
         WHERE trial_id = ?1 AND rowid = (SELECT MAX(rowid) FROM autotuner_shadow_log WHERE trial_id = ?1)",
    )
    .bind(trial_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Select a channel by weighted random based on message_share.
fn select_channel<'a>(
    channels: &'a [crate::scenario::ChannelConfig],
    rng: &mut rand::rngs::StdRng,
) -> &'a crate::scenario::ChannelConfig {
    let total: f64 = channels.iter().map(|c| c.message_share).sum();
    let mut roll = rng.random::<f64>() * total;
    for ch in channels {
        roll -= ch.message_share;
        if roll <= 0.0 {
            return ch;
        }
    }
    channels.last().unwrap()
}

/// Parse an epoch step string from scenario config into an `EpochStep`.
fn parse_epoch_step(s: &str) -> EpochStep {
    let s = s.trim().to_lowercase();
    if let Some(mins) = s.strip_suffix("min") {
        let m: u32 = mins.trim().parse().unwrap_or(30);
        return EpochStep::Minutes(m);
    }
    if let Some(hours) = s.strip_suffix('h') {
        let h: u32 = hours.trim().parse().unwrap_or(4);
        return EpochStep::Hours(h);
    }
    match s.as_str() {
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
        assert!(matches!(parse_epoch_step("30min"), EpochStep::Minutes(30)));
        assert!(matches!(parse_epoch_step("15min"), EpochStep::Minutes(15)));
        assert!(matches!(parse_epoch_step("5min"), EpochStep::Minutes(5)));
        assert!(matches!(parse_epoch_step("4h"), EpochStep::Hours(4)));
        assert!(matches!(parse_epoch_step("12h"), EpochStep::Hours(12)));
    }
}
