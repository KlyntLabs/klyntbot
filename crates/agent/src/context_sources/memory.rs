//! Memory context source — long-term memory and today's notes.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;

use crate::memory::MemoryStore;

/// Default TTL for cached memory context (seconds).
const MEMORY_CACHE_TTL_SECS: i64 = 60;

/// Provides memory context (long-term + daily notes) with TTL caching.
pub struct MemorySource {
    memory: MemoryStore,
    cache: Mutex<Option<CachedValue>>,
}

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
    /// Query used to generate this cached value (None = unfiltered).
    query: Option<String>,
}

impl MemorySource {
    pub fn new(memory: MemoryStore) -> Self {
        Self {
            memory,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for MemorySource {
    fn name(&self) -> &str {
        "memory"
    }

    fn priority(&self) -> u8 {
        80
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // Check TTL cache — cache hit only if query matches
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at && cached.query == ctx.message {
                    return if cached.content.trim().is_empty() {
                        None
                    } else {
                        Some(format!("# Memory\n\n{}", cached.content))
                    };
                }
            }
        }

        // Cache miss — fetch fresh, using relevance filtering if message available
        let content = if let Some(ref query) = ctx.message {
            self.memory.get_relevant_memory(query, 5).await
        } else {
            self.memory.get_memory_context().await
        };

        let result = if content.trim().is_empty() {
            None
        } else {
            Some(format!("# Memory\n\n{}", content))
        };

        // Store in cache
        {
            let mut cache = self.cache.lock().await;
            *cache = Some(CachedValue {
                content,
                expires_at: Utc::now() + Duration::seconds(MEMORY_CACHE_TTL_SECS),
                query: ctx.message.clone(),
            });
        }

        result
    }
}
