//! Area context source — available areas for the LLM.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

/// Default TTL for cached area context (seconds).
const AREA_CACHE_TTL_SECS: i64 = 60;

/// Provides available areas summary with TTL caching.
pub struct AreaSource {
    repo: storage::AreaRepo,
    cache: Mutex<Option<CachedValue>>,
}

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

impl AreaSource {
    pub fn new(repo: storage::AreaRepo) -> Self {
        Self {
            repo,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for AreaSource {
    fn name(&self) -> &str {
        "area"
    }

    fn priority(&self) -> u8 {
        75
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
        let content = match self.repo.list(Some("active")).await {
            Ok(areas) => {
                if areas.is_empty() {
                    String::new()
                } else {
                    let mut out = String::from("Available areas:\n");
                    for area in &areas {
                        out.push_str(&format!("- {} (ID: {})\n", area.name, area.id));
                    }
                    out
                }
            }
            Err(e) => {
                warn!("SQL area context failed: {}", e);
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
                expires_at: Utc::now() + Duration::seconds(AREA_CACHE_TTL_SECS),
            });
        }

        result
    }
}
