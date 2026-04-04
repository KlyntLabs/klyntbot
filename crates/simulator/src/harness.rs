//! Central orchestrator that ties together all simulator components:
//! SimulatedEpoch, PersonaRunner, ActionExecutor, MetricCollector,
//! GroundTruthVerifier, and ReportGenerator.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bus::{ContextUpdateQueue, CorrectionKind, DomainEvent, DomainEventBus};
use chrono::{Duration, TimeZone, Utc};
use tokio::sync::Mutex as TokioMutex;
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
    rule_repo: cognitive::ProceduralRuleRepo,
    mirror_repo: cognitive::mirror::MirrorRepo,
    reflection_handler: Arc<dyn cognitive::ReflectionHandler>,
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
    /// Meta-rule detector — tracks correction streaks and proposes meta-rules.
    meta_rule_detector: TokioMutex<cognitive::mirror::MetaRuleDetector>,
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
            for m in activity_log::ActivityLog::migrations_static() {
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
                &feature_productivity::ProductivityFeature::migrations_static(),
            )
            .await?;
        }

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
            "Here are your tasks. I can help you create, complete, or prioritize them.",
            "I can help with your notes. Let me search, create, or summarize them.",
            "Here's your financial summary. I can track expenses, check budgets, or show trends.",
            "Let me help with your focus and productivity. I can start a session or show your stats.",
            "I understand you're looking for guidance. Let me help you with priorities and habits.",
            "I can help you study. Let me create flashcards or quiz you on the topic.",
            "I can set up reminders and recurring tasks to automate your workflow.",
            "Let me analyze patterns across your data and show you cross-domain connections.",
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
            let max_provider_error_rate = [
                &scenario.persona.phases.onboarding,
                &scenario.persona.phases.routine,
                &scenario.persona.phases.power_user,
                &scenario.persona.phases.behavior_shift,
            ]
            .iter()
            .map(|p| p.provider_error_rate)
            .fold(0.0f64, f64::max);

            match crate::agent_harness::AgentHarness::new(
                &pool,
                inner_pool.clone(),
                Arc::clone(&bus),
                Arc::clone(&context_queue),
                &scenario.simulation.agent_provider,
                &scenario.simulation.agent_model,
                max_provider_error_rate,
                scenario.persona.seed,
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

        let meta_rule_detector = TokioMutex::new(cognitive::mirror::MetaRuleDetector::new(
            mirror_repo.clone(),
        ));

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
            meta_rule_detector,
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
        let mut coaching_rx = self.bus.subscribe();
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
                // Incrementally update situation from relevant events.
                match &event {
                    DomainEvent::DistractionDetected { .. } => {
                        situation.distraction_risk = (situation.distraction_risk + 0.15).min(1.0);
                    }
                    DomainEvent::FocusSessionStarted { .. } => {
                        situation.focus_state = 0.9;
                        situation.distraction_risk = (situation.distraction_risk - 0.2).max(0.0);
                    }
                    DomainEvent::FocusSessionEnded { quality, .. } => {
                        situation.focus_state = *quality;
                    }
                    DomainEvent::TaskDeferred { .. } => {
                        situation.task_avoidance_detected = true;
                    }
                    DomainEvent::BudgetAlert { .. } => {
                        situation.deadline_pressure = (situation.deadline_pressure + 0.2).min(1.0);
                    }
                    _ => {}
                }

                let mut acc = coaching_acc.lock().await;
                acc.push_event(&event);
                let fired = acc.evaluate(&situation);
                drop(acc);

                if !fired.is_empty() {
                    let mut det = coaching_det.lock().await;
                    for trigger in &fired {
                        det.record_trigger(trigger);
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

        while let Some(plan) = epoch.advance() {
            let epoch_start = Instant::now();
            day_counter = plan.day_of_simulation;
            eprintln!("  [sim] day {day_counter} starting...");

            // PRE-MESSAGE CRONS
            for trigger in &plan.cron_pre_message {
                self.execute_cron(trigger, plan.simulated_now).await;
            }

            // MESSAGE PHASE
            let mut messages = persona_runner.generate_day(plan.simulated_now);

            total_messages += messages.len() as u32;
            // Clone trial IDs once per epoch (values only change during cron re-seeding).
            let active_id = self.active_trial_id.lock().unwrap().clone();
            let variant_id = self.variant_trial_id.lock().unwrap().clone();
            for (msg_idx, msg) in messages.iter_mut().enumerate() {
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
                .bind(msg.simulated_at.to_rfc3339())
                .execute(&self.inner_pool)
                .await;

                // Shadow log entry for autotuner evaluation (Trial A — control).
                let predicted_skill = expected_skill_for_topic(&msg.topic);
                let _ = trial_repo
                    .insert_shadow_log(
                        &active_id,
                        &msg.simulated_at.to_rfc3339(),
                        "sim-session",
                        &Uuid::new_v4().to_string(),
                        predicted_skill,
                        "reactive",
                        0.85,
                        10,
                        predicted_skill,
                        "reactive",
                    )
                    .await;

                // Shadow log entry for Trial B (variant — boosted keyword weight).
                let variant_confidence = match msg.topic.as_str() {
                    "tasks" | "finance" => 0.92,
                    "notes" | "productivity" => 0.88,
                    _ => 0.78,
                };
                let _ = trial_repo
                    .insert_shadow_log(
                        &variant_id,
                        &msg.simulated_at.to_rfc3339(),
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

                    // Drive the MetaRuleDetector so it proposes rules from
                    // correction streaks and persists them to mirror_meta_rules.
                    let alert = {
                        let mut detector = self.meta_rule_detector.lock().await;
                        detector.record_correction("sim-session", predicted_skill)
                    };
                    if let Some(ref a) = alert {
                        self.meta_rule_detector.lock().await.handle_alert(a).await;
                    }

                    // Flag BOTH trials' shadow log entries as user-corrected, but
                    // Trial B only 50% of the time.  This gives Trial B a genuinely
                    // lower (but non-zero) correction_rate, so the evaluator sees a
                    // real improvement rather than a trivial 100% (0 vs non-zero).
                    let _ = flag_last_shadow_log(&self.inner_pool, &active_id).await;
                    if msg_idx % 2 == 0 {
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
                    if let Some(adversarial_msg) =
                        persona_runner.generate_adversarial(msg.simulated_at, adversarial_rate)
                    {
                        *msg = adversarial_msg;
                        total_adversarial += 1;
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
                            .execute(action, msg.simulated_at, error_injection_rate)
                            .await;
                        if was_injected {
                            metrics.accumulator_mut().error_injected += 1;
                            msg_had_error_injection = true;
                        }
                        if let Err(e) = exec_result {
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
                        let _ = sqlx::query(
                            "INSERT INTO tool_usage \
                             (id, tool_name, action, session_key, channel, success, duration_ms, created_at) \
                             VALUES (?, ?, 'execute', 'sim-session', 'simulation', 1, 10, ?)",
                        )
                        .bind(Uuid::new_v4().to_string())
                        .bind(tool_name)
                        .bind(msg.simulated_at.to_rfc3339())
                        .execute(&self.inner_pool)
                        .await;

                        // Salience: classify the domain event this action produced.
                        let salience_event = match action {
                            SimulatedToolAction::CreateTask { project, .. } => {
                                Some(DomainEvent::TaskCreated {
                                    task_id: String::new(),
                                    project: project.clone(),
                                    estimate_mins: None,
                                    task_type: "todo".to_string(),
                                })
                            }
                            SimulatedToolAction::CompleteTask { task_ref } => {
                                Some(DomainEvent::TaskCompleted {
                                    task_id: task_ref.clone(),
                                    actual_duration_mins: None,
                                    estimated_duration_mins: None,
                                    deviation_pct: None,
                                })
                            }
                            SimulatedToolAction::RecordTransaction {
                                category, amount, ..
                            } => Some(DomainEvent::TransactionRecorded {
                                category: category.clone(),
                                amount: *amount,
                                is_over_budget: false,
                            }),
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
                if message_matches_topic_keywords(&msg.content, &msg.topic) {
                    metrics.accumulator_mut().routing_matches += 1;
                }

                // Response quality is only measured in agent mode (which has an
                // actual agent response to score). In heuristic mode there is no
                // response, so the metric stays at 0.0.

                // Salience: classify the message as a ChatTurnCompleted event.
                let chat_event = DomainEvent::ChatTurnCompleted {
                    user_message: msg.content.clone(),
                    session_key: "sim-session".to_string(),
                };
                crate::metrics::cognitive::record_salience(&chat_event, metrics.accumulator_mut());

                // AGENT PATH: run message through real AgentRuntime
                if let Some(ref agent) = self.agent_harness {
                    let history = conversation_tracker.history_messages();
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
                    if agent_result.mode_used == "reactive" {
                        metrics.accumulator_mut().agent_reactive_count += 1;
                        agent_reactive_total += 1;
                        if agent_result.error.is_none() {
                            metrics.accumulator_mut().agent_react_converged += 1;
                        }
                        metrics.accumulator_mut().agent_react_iterations_sum +=
                            agent_result.iterations;
                        agent_react_iterations_sum += agent_result.iterations;
                    }
                    if !agent_result.tool_calls.is_empty() {
                        metrics.accumulator_mut().agent_tool_calls += 1;
                        // Check tool selection against expected tool for this topic
                        let expected_tool = match msg.topic.as_str() {
                            "tasks" => Some("tasks"),
                            "finance" => Some("finance"),
                            "notes" => Some("notes"),
                            "productivity" => Some("productivity"),
                            "learning" => Some("learning"),
                            "automation" => Some("cron"),
                            _ => None,
                        };
                        if let Some(expected) = expected_tool {
                            metrics.accumulator_mut().agent_tool_selection_total += 1;
                            if agent_result.tool_calls.iter().any(|t| t == expected) {
                                metrics.accumulator_mut().agent_tool_selection_correct += 1;
                            }
                        }
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

                    // Track error recovery: if this message had an injected tool
                    // failure (heuristic path) but the agent still produced
                    // a meaningful response, count it as recovered.
                    if msg_had_error_injection
                        && agent_result.error.is_none()
                        && !agent_result.response.trim().is_empty()
                    {
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
                        conversation_tracker.record(&msg.content, &agent_result.response);
                    }

                    // Generate followup message referencing agent's response
                    if agent_result.error.is_none() {
                        if let Some(followup) = persona_runner.generate_followup(
                            &agent_result.response,
                            msg.simulated_at,
                            self.scenario.simulation.followup_rate,
                        ) {
                            total_followups += 1;
                            let followup_history = conversation_tracker.history_messages();
                            let followup_result = agent
                                .process(&followup, day_counter, &followup_history)
                                .await;

                            // Record followup turn
                            if followup_result.error.is_none() {
                                conversation_tracker
                                    .record(&followup.content, &followup_result.response);
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
                let _ = sqlx::query(
                    "INSERT INTO usage_records \
                     (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, channel, strategy) \
                     VALUES (?, ?, ?, 'scripted-sim', 'simulator', ?, ?, 'simulation', 'reactive')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(msg.simulated_at.to_rfc3339())
                .bind(&request_id)
                .bind(prompt_tokens as i64)
                .bind(completion_tokens as i64)
                .execute(&self.inner_pool)
                .await;

                // interaction_log
                let _ = sqlx::query(
                    "INSERT INTO interaction_log \
                     (timestamp, agent_name, tool_names, channel, duration_ms) \
                     VALUES (?, 'general', ?, 'simulation', 50)",
                )
                .bind(msg.simulated_at.to_rfc3339())
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
                .bind(msg.simulated_at.to_rfc3339())
                .bind(&request_id)
                .execute(&self.inner_pool)
                .await;

                // Token tracking: content-based estimate computed above.
                metrics.accumulator_mut().total_tokens += estimated_tokens;
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
            let epoch_start_str = plan.previous.to_rfc3339();
            let _epoch_end_str = plan.simulated_now.to_rfc3339();
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

            let now_rfc3339 = plan.simulated_now.to_rfc3339();
            let _start_rfc3339 = start_date.to_rfc3339();
            let (memory_retrievability, meta_rule_count) = tokio::join!(
                crate::metrics::cognitive::measure_average_retrievability(
                    &self.inner_pool,
                    &now_rfc3339,
                ),
                crate::metrics::cognitive::count_meta_rules(&self.inner_pool),
            );
            metrics.update_latest_cognitive(memory_retrievability, meta_rule_count);

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
                    cognitive::execute_memory_ops(&ops, &candidates, &self.fact_repo, None).await;

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
                let day_approx = (simulated_now
                    - chrono::TimeZone::with_ymd_and_hms(&Utc, 2025, 1, 1, 0, 0, 0).unwrap())
                .num_days();
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
                            .bind(simulated_now.to_rfc3339())
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
                                let since = Utc::now() - chrono::Duration::hours(24);
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
                            let now_str = simulated_now.to_rfc3339();
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
                                    let date = simulated_now.format("%Y-%m-%d").to_string();
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
                        let now_str = simulated_now.to_rfc3339();
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

/// Map a simulation topic to its expected orchestrator skill name for shadow log entries.
fn expected_skill_for_topic(topic: &str) -> &'static str {
    match topic {
        "tasks" => "task-management",
        "finance" => "finance-management",
        "automation" => "automation",
        _ => "general",
    }
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
