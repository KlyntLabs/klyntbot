//! Opencode opt-in marker — no settings file write needed.
//!
//! Opencode is poll-only; enabling it just records the choice in config.
//! No hook installation is required because the adapter reads opencode's
//! SQLite DB directly.

use common::Result;
use std::path::Path;

/// Opencode installer — no-op for install/uninstall, diagnose pings the DB.
pub struct OpencodeInstaller;

impl OpencodeInstaller {
    /// No settings to write; opencode is poll-only.
    pub fn install(_config_path: &Path) -> Result<()> {
        Ok(())
    }

    /// No settings to remove.
    pub fn uninstall(_config_path: &Path) -> Result<()> {
        Ok(())
    }

    /// Verify the SQLite DB exists and is readable.
    pub fn diagnose(db_path: &Path) -> Result<()> {
        if !db_path.exists() {
            return Err(common::KlyntbotError::Storage(format!(
                "opencode DB not found at {}",
                db_path.display()
            )));
        }
        // Try a quick pragma to confirm it's a valid SQLite file.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| common::KlyntbotError::Storage(format!("runtime: {e}")))?;
        rt.block_on(async {
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(db_path)
                .read_only(true);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(2))
                .connect_with(opts)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("opencode DB open: {e}")))?;
            let _count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message")
                .fetch_one(&pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("opencode DB query: {e}")))?;
            Ok(())
        })
    }
}
