use std::sync::Arc;

use feature_notes::repo::NoteRepo;
use storage::{Repos, StoragePool, VectorStore};
use tools_core::FeaturePackage;
use tracing::{info, warn};

/// Results from the storage initialization phase.
pub(super) struct StorageResult {
    pub config: config::Config,
    pub storage_pool: StoragePool,
    pub repos: Repos,
    pub vector_store: Option<VectorStore>,
    pub note_repo: NoteRepo,
    pub provider: providers::DynProvider,
    /// The inner [`ProviderManager`] when failover is configured — held so callers
    /// can attach additional callbacks (e.g. provider-degraded event forwarding).
    pub provider_manager: Option<std::sync::Arc<providers::ProviderManager>>,
}

/// Initialize config, storage, migrations, and LLM provider.
pub(super) async fn init_storage(
    config_override: Option<config::Config>,
) -> Result<StorageResult, String> {
    // 1. Load config
    let mut config = match config_override {
        Some(c) => c,
        None => config::load_with_env_overrides()
            .await
            .map_err(|e| format!("config load failed: {e}"))?,
    };
    info!(path = ?config::config_path(), "configuration loaded");

    // 2. Connect storage
    let data_dir = config.data_dir_path();
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("failed to create data dir: {e}"))?;

    let storage_pool = StoragePool::connect(&data_dir)
        .await
        .map_err(|e| format!("storage connect failed: {e}"))?;
    let repos = Repos::from_pool(&storage_pool);
    let vector_store = VectorStore::connect(&data_dir).await.ok();
    // Immediately compact high-write-frequency tables before the agent starts.
    // work_context_embeddings accumulates fragments rapidly (Append+Delete per
    // centroid update) and can reach 2000+ versions, each memory-mapped.
    if let Some(vs) = &vector_store {
        if let Err(e) = vs.optimize_table("work_context_embeddings").await {
            warn!("Immediate work_context compaction failed (non-fatal): {e}");
        }
    }
    // Create ANN indexes + compact remaining tables in the background.
    // Deferred by 5 minutes so the app starts lean (~250MB) and the user's
    // first interactions aren't competing with compaction memory spikes.
    // The periodic 30-minute compaction cron handles ongoing maintenance.
    if let Some(vs) = &vector_store {
        let vs_bg = vs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            if let Err(e) = vs_bg.optimize_all_tables().await {
                warn!("LanceDB startup compaction failed (non-fatal): {e}");
            }
            common::memory::purge_freed_memory();

            // Prune large tables to prevent unbounded growth.
            const MAX_CONV_ROWS: usize = 10_000;
            const MAX_ACTIVITY_ROWS: usize = 50_000;
            const MAX_COGNITIVE_ROWS: usize = 20_000;
            let _ = vs_bg
                .prune_table("conv_embeddings", "created_at", MAX_CONV_ROWS)
                .await;
            let _ = vs_bg
                .prune_table("activity_embeddings", "updated_at", MAX_ACTIVITY_ROWS)
                .await;
            let _ = vs_bg
                .prune_table(
                    "cognitive_fact_embeddings",
                    "updated_at",
                    MAX_COGNITIVE_ROWS,
                )
                .await;
            let _ = vs_bg
                .prune_table("work_context_embeddings", "updated_at", 5_000)
                .await;
            common::memory::purge_freed_memory();

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(e) = vs_bg.ensure_indexes(256).await {
                warn!("ANN index creation failed (non-fatal): {e}");
            }
            common::memory::purge_freed_memory();
        });
    }
    info!("storage connected");

    macro_rules! run_migration {
        ($name:expr, $migrations:expr) => {
            StoragePool::run_feature_migrations(storage_pool.inner(), &$migrations)
                .await
                .map_err(|e| format!(concat!($name, " migration failed: {}"), e))?
        };
    }

    // Run notes feature migrations and create repo.
    let notes_pool = storage_pool.inner().clone();
    run_migration!("notes", feature_notes::notes_migrations());
    let note_repo = NoteRepo::new(notes_pool);

    // Run tasks feature migrations.
    let tasks_feature = feature_tasks::TasksFeature::new();
    run_migration!("tasks", tasks_feature.migrations());

    // Run scheduling feature migrations (Phase 2: scheduled_fires table).
    run_migration!(
        "scheduling",
        [tools_core::FeatureMigration {
            feature_name: "scheduling".to_string(),
            version: 1,
            description: "Create scheduled_fires table".to_string(),
            sql: include_str!("../../../scheduling/migrations/001_scheduled_fires.sql").to_string(),
        }]
    );

    // Run language-learning feature migrations.
    run_migration!(
        "language-learning",
        feature_language_learning::language_learning_migrations()
    );

    // Run finance feature migrations.
    run_migration!("finance", feature_finance::finance_migrations());

    // Run focus feature migrations (DND sessions).
    run_migration!(
        "focus",
        <feature_focus::FocusFeature as tools_core::FeaturePackage>::migrations(
            &feature_focus::FocusFeature,
        )
    );

    // Run learning feature migrations (placeholder in v3; tables live in cognitive).
    run_migration!(
        "learning",
        <feature_learning::LearningFeature as tools_core::FeaturePackage>::migrations(
            &feature_learning::LearningFeature::default(),
        )
    );

    // Run coding-todo feature migrations.
    run_migration!(
        "coding_todo",
        [feature_coding_todo::migrations::coding_todo_migration()]
    );

    // Run cognitive migrations up-front so downstream crates (coding-memory) can
    // ALTER their tables. Cognitive migrations are idempotent; the agent builder
    // also invokes them later without conflict.
    run_migration!("cognitive", cognitive::cognitive_migrations());

    // Run coding-memory migrations (scope/provenance columns on semantic_facts,
    // episodic_memories, skill_versions; causal-edge / ingest-log / klynt_sessions
    // tables). Must run AFTER cognitive migrations create the base tables.
    run_migration!("coding-memory", coding_memory::coding_memory_migrations());

    // 3. Create LLM provider (graceful — falls back to noop for setup wizard).
    // Use the "full" variant to get the inner ProviderManager (when a fallback is configured)
    // so we can wire circuit breaker persistence before the manager starts handling calls.
    let (provider, provider_manager, resolved_model) =
        match providers::create_provider_with_failover_full(&config) {
            Ok((p, maybe_manager, m)) => {
                info!(provider = %p.name(), "provider ready");

                if let Some(ref manager) = maybe_manager {
                    // Ensure the table exists and restore any unexpired breaker state.
                    if let Err(e) = storage::circuit_breaker::ensure_table(&storage_pool).await {
                        warn!("circuit breaker table init failed (non-fatal): {e}");
                    } else {
                        if let Ok(Some(dt)) = storage::circuit_breaker::load(&storage_pool).await {
                            manager.restore_circuit_state(dt).await;
                        }

                        // Callback: persist each circuit-open event for future restarts.
                        let pool = storage_pool.clone();
                        let cb: providers::OnCircuitOpen = Arc::new(move |open_until| {
                            let pool = pool.clone();
                            tokio::spawn(async move {
                                if let Err(e) =
                                    storage::circuit_breaker::save(&pool, open_until).await
                                {
                                    tracing::warn!("circuit breaker persist failed: {e}");
                                }
                            });
                        });
                        manager.set_circuit_open_callback(cb).await;
                    }
                }

                (p, maybe_manager, m)
            }
            Err(e) => {
                warn!(
                    "No LLM provider configured ({e}), using noop — setup wizard will handle configuration"
                );
                let noop: providers::DynProvider = Arc::new(providers::NoopProvider);
                (noop, None, config.agents.defaults.model.clone())
            }
        };
    config.agents.defaults.model = resolved_model;

    Ok(StorageResult {
        config,
        storage_pool,
        repos,
        vector_store,
        note_repo,
        provider,
        provider_manager,
    })
}
