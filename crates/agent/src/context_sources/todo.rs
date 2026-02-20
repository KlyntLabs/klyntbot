//! Todo context source — active tasks summary.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

/// Default TTL for cached todo context (seconds).
const TODO_CACHE_TTL_SECS: i64 = 60;

/// Provides active tasks summary with TTL caching.
pub struct TodoSource {
    repo: storage::TodoRepo,
    cache: Mutex<Option<CachedValue>>,
}

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

impl TodoSource {
    pub fn new(repo: storage::TodoRepo) -> Self {
        Self {
            repo,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for TodoSource {
    fn name(&self) -> &str {
        "todo"
    }

    fn priority(&self) -> u8 {
        70
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
        let content = match self.repo.to_context_string().await {
            Ok(ctx) => ctx,
            Err(e) => {
                warn!("SQL todo context failed: {}", e);
                String::new()
            }
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
                expires_at: Utc::now() + Duration::seconds(TODO_CACHE_TTL_SECS),
            });
        }

        result
    }
}
