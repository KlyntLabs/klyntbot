//! Todo context source — active tasks summary.

use async_trait::async_trait;
use context_engine::TtlCache;
use context_engine::source::{ContextSource, SourceContext};
use tracing::warn;

/// Default TTL for cached todo context (seconds).
const TODO_CACHE_TTL_SECS: i64 = 60;

/// Provides active tasks summary with TTL caching.
pub struct TodoSource {
    repo: storage::TaskRepo,
    cache: TtlCache,
}

impl TodoSource {
    pub fn new(repo: storage::TaskRepo) -> Self {
        Self {
            repo,
            cache: TtlCache::new(TODO_CACHE_TTL_SECS),
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
        if let Some(cached) = self.cache.get() {
            return if cached.trim().is_empty() {
                None
            } else {
                Some(cached)
            };
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

        self.cache.set(content);
        result
    }

    fn estimated_tokens(&self) -> usize {
        600
    }
}
