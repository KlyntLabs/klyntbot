use crate::AppCore;
use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ResumeResult {
    pub session_key: String,
    pub title: String,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_resume(&self, prefix: &str) -> Result<ResumeResult> {
        let pool = self.repos.pool();
        let pat = format!("{}%", prefix);
        let row = sqlx::query(
            "SELECT key, json_extract(metadata, '$.title') as title FROM sessions WHERE LOWER(json_extract(metadata, '$.title')) LIKE LOWER(?) ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(&pat)
        .fetch_optional(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        match row {
            Some(r) => {
                let key: String = r.try_get("key").unwrap_or_default();
                let title: Option<String> = r.try_get("title").ok();
                Ok(ResumeResult {
                    session_key: key,
                    title: title.unwrap_or_default(),
                })
            }
            None => Err(KlyntbotError::Session(common::SessionError::NotFound(
                format!("no thread title starts with `{prefix}`"),
            ))),
        }
    }
}
