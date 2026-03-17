//! Area context source — available areas for the LLM.

use std::fmt::Write;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use context_engine::TtlCache;
use tracing::warn;

/// Default TTL for cached area context (seconds).
const AREA_CACHE_TTL_SECS: i64 = 60;

/// Provides available areas summary with TTL caching.
pub struct AreaSource {
    repo: storage::AreaRepo,
    cache: TtlCache,
}

impl AreaSource {
    pub fn new(repo: storage::AreaRepo) -> Self {
        Self {
            repo,
            cache: TtlCache::new(AREA_CACHE_TTL_SECS),
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
        if let Some(cached) = self.cache.get() {
            return if cached.trim().is_empty() {
                None
            } else {
                Some(cached)
            };
        }

        // Cache miss — fetch fresh
        let content = match self.repo.list(Some("active")).await {
            Ok(areas) => {
                if areas.is_empty() {
                    String::new()
                } else {
                    let mut out = String::from("Available areas:\n");
                    for area in &areas {
                        let _ = writeln!(out, "- {} (ID: {})", area.name, area.id);
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

        self.cache.set(content);
        result
    }

    fn estimated_tokens(&self) -> usize {
        400
    }
}
