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

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Check TTL cache
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at {
                    return if cached.content.trim().is_empty() {
                        None
                    } else {
                        Some(format!("# Memory\n\n{}", cached.content))
                    };
                }
            }
        }

        // Cache miss — fetch fresh
        let content = self.memory.get_memory_context().await;
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
            });
        }

        result
    }
}
