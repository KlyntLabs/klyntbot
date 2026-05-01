use crate::AppCore;
use common::Result;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_star(&self, session_key: &str) -> Result<()> {
        let pool = self.repos.pool();
        sqlx::query("UPDATE sessions SET pinned = 1 WHERE key = ?")
            .bind(session_key)
            .execute(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_unstar(&self, session_key: &str) -> Result<()> {
        let pool = self.repos.pool();
        sqlx::query("UPDATE sessions SET pinned = 0 WHERE key = ?")
            .bind(session_key)
            .execute(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}
