use std::sync::Arc;

use feature_notes::repo::NoteRepo;
use storage::{Repos, StoragePool, VectorStore};
use tracing::{info, warn};

/// Results from the storage initialization phase.
pub(super) struct StorageResult {
    pub config: config::Config,
    pub storage_pool: StoragePool,
    pub repos: Repos,
    pub vector_store: Option<VectorStore>,
    pub note_repo: NoteRepo,
    pub provider: providers::DynProvider,
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
    // Create ANN indexes in the background (requires 256+ rows to train).
    if let Some(vs) = &vector_store {
        let vs_bg = vs.clone();
        tokio::spawn(async move {
            if let Err(e) = vs_bg.ensure_indexes(256).await {
                warn!("ANN index creation failed (non-fatal): {e}");
            }
        });
    }
    info!("storage connected");

    // Run notes feature migrations and create repo.
    let notes_pool = storage_pool.inner().clone();
    StoragePool::run_feature_migrations(
        &notes_pool,
        &feature_notes::NotesFeature::migrations_static(),
    )
    .await
    .map_err(|e| format!("notes migration failed: {e}"))?;
    let note_repo = NoteRepo::new(notes_pool);

    // Run tasks feature migrations.
    StoragePool::run_feature_migrations(
        storage_pool.inner(),
        &[tools_core::FeatureMigration {
            feature_name: "tasks".to_string(),
            version: 1,
            description: "Create agentic task tables".to_string(),
            sql: feature_tasks::TasksFeature::migration_sql().to_string(),
        }],
    )
    .await
    .map_err(|e| format!("tasks migration failed: {e}"))?;

    // 3. Create LLM provider (graceful — falls back to noop for setup wizard)
    let (provider, resolved_model) = match providers::create_provider(&config) {
        Ok((p, m)) => {
            info!(provider = %p.name(), "provider ready");
            (p, m)
        }
        Err(e) => {
            warn!("No LLM provider configured ({e}), using noop — setup wizard will handle configuration");
            let noop: providers::DynProvider = Arc::new(providers::NoopProvider);
            (noop, config.agents.defaults.model.clone())
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
    })
}
