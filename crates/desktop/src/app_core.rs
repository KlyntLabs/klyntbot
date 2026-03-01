use storage::{Repos, StoragePool};

/// Central application state managed by Tauri.
/// Holds Repos for database access. AgentLoop integration will be added later.
pub struct AppCore {
    pub repos: Repos,
}

impl AppCore {
    pub async fn init() -> Result<Self, String> {
        let data_dir = dirs::home_dir()
            .ok_or("could not determine home directory")?
            .join(".klyntbot");

        std::fs::create_dir_all(&data_dir)
            .map_err(|e| format!("failed to create data dir: {e}"))?;

        let pool = StoragePool::connect(&data_dir)
            .await
            .map_err(|e| format!("failed to connect to database: {e}"))?;

        let repos = Repos::from_pool(&pool);

        tracing::info!(data_dir = %data_dir.display(), "desktop app core initialized");

        Ok(Self { repos })
    }
}
