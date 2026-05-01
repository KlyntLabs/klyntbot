use crate::AppCore;
use common::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run_session_retention_pass(
    repos: &storage::Repos,
    config: &RwLock<config::Config>,
) -> Result<u64> {
    let cfg = config.read().await;
    let retention_days = cfg.coding.sessions.retention_days as i64;
    let preserve_starred = cfg.coding.sessions.preserve_starred;
    drop(cfg);

    let pool = repos.pool();
    let cutoff_ms =
        jiff::Timestamp::now().as_millisecond() - retention_days * 86400 * 1000;
    let result = if preserve_starred {
        sqlx::query("DELETE FROM sessions WHERE created_at < ? AND COALESCE(pinned, 0) = 0")
            .bind(cutoff_ms)
    } else {
        sqlx::query("DELETE FROM sessions WHERE created_at < ?").bind(cutoff_ms)
    };
    let r = result
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    tracing::info!(
        deleted = r.rows_affected(),
        "session retention pass complete"
    );
    Ok(r.rows_affected())
}

impl AppCore {
    pub async fn run_session_retention_pass(&self) -> Result<u64> {
        run_session_retention_pass(&self.repos, &self.config).await
    }
}

pub fn init_coding_retention(core: &AppCore) {
    let rt = tokio::runtime::Handle::current();
    let repos = core.repos.clone();
    let config = Arc::clone(&core.config);
    core.cron_executor.register(
        super::cron::JOB_SESSION_RETENTION,
        Arc::new(move |_job: &scheduling::CronJob| {
            let repos = repos.clone();
            let config = Arc::clone(&config);
            tokio::task::block_in_place(|| {
                rt.block_on(async move {
                    match run_session_retention_pass(&repos, &config).await {
                        Ok(n) => Ok(Some(format!("Deleted {} old sessions", n))),
                        Err(e) => {
                            tracing::error!("session retention failed: {e}");
                            Err(e)
                        }
                    }
                })
            })
        }),
    );
}
