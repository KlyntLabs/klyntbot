//! Goal context source — active goals summary.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use goal::GoalStatus;
use tokio::sync::Mutex;

/// Default TTL for cached goal context (seconds).
const GOAL_CACHE_TTL_SECS: i64 = 60;

/// Provides active goals summary with TTL caching.
pub struct GoalSource {
    repo: storage::GoalRepo,
    cache: Mutex<Option<CachedValue>>,
}

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

impl GoalSource {
    pub fn new(repo: storage::GoalRepo) -> Self {
        Self {
            repo,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for GoalSource {
    fn name(&self) -> &str {
        "goals"
    }

    fn priority(&self) -> u8 {
        60
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Check TTL cache
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at {
                    return if cached.content.trim().is_empty() {
                        None
                    } else {
                        Some(cached.content.clone())
                    };
                }
            }
        }

        // Cache miss — fetch fresh
        let status_str = GoalStatus::Active.to_string();
        let content = match self.repo.list(Some(&status_str)).await {
            Ok(rows) if !rows.is_empty() => {
                let mut buf = String::from("# Goals\n\n");
                for row in &rows {
                    buf.push_str(&format!(
                        "- [P{}] {}: {}\n",
                        row.priority, row.title, row.description
                    ));
                }
                buf
            }
            _ => String::new(),
        };

        let result = if content.trim().is_empty() {
            None
        } else {
            Some(content.clone())
        };

        // Store in cache
        {
            let mut cache = self.cache.lock().await;
            *cache = Some(CachedValue {
                content,
                expires_at: Utc::now() + Duration::seconds(GOAL_CACHE_TTL_SECS),
            });
        }

        result
    }
}
