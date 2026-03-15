use std::sync::Arc;

use feature_launcher::{AppIndex, ClipboardRepo, FrequencyRepo, ScriptRunner};
use storage::StoragePool;
use tracing::{error, info};

use crate::handlers::launcher::LauncherSearchEngine;

/// Results from launcher initialization phase.
pub(super) struct LauncherResult {
    pub launcher_engine: Option<Arc<LauncherSearchEngine>>,
    pub clipboard_repo: Option<ClipboardRepo>,
}

/// Initialize the launcher feature (always enabled).
pub(super) async fn init_launcher(
    config: &config::Config,
    storage_pool: &StoragePool,
) -> LauncherResult {
    let pool = storage_pool.inner().clone();

    // Run feature migrations
    if let Err(e) = StoragePool::run_feature_migrations(
        &pool,
        &feature_launcher::LauncherFeature::migrations_static(),
    )
    .await
    {
        error!("launcher migration failed — feature disabled: {e}");
        return LauncherResult {
            launcher_engine: None,
            clipboard_repo: None,
        };
    }

    let frequency_repo = FrequencyRepo::new(pool.clone());
    let clipboard_repo = ClipboardRepo::new(pool);
    let app_index = AppIndex::new();
    let script_runner = ScriptRunner::new();

    // Discover scripts from data dir
    let scripts_dir = config.data_dir_path().join("scripts");
    let scripts_path = scripts_dir.as_path();
    if scripts_path.exists() {
        let scripts = ScriptRunner::discover(scripts_path);
        info!("discovered {} launcher scripts", scripts.len());
        script_runner.set_scripts(scripts);
    }

    let engine = Arc::new(LauncherSearchEngine {
        app_index: app_index.clone(),
        script_runner,
        frequency_repo,
        clipboard_repo: clipboard_repo.clone(),
    });

    // Spawn background app indexing
    let index = app_index;
    tokio::spawn(async move {
        index.index_applications().await;
    });

    info!("launcher feature initialized");
    LauncherResult {
        launcher_engine: Some(engine),
        clipboard_repo: Some(clipboard_repo),
    }
}
