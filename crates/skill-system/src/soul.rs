//! SoulContextSource — loads KLYNTBOT.md as always-present context.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::debug;

use context_engine::source::{ContextSource, SourceContext};

/// Default KLYNTBOT.md content, installed on first run.
const DEFAULT_SOUL: &str = r#"# Klyntbot

You are Klyntbot, a personal AI assistant.

## Personality
- Helpful, concise, and proactive
- Speak naturally, not robotically
- Match the user's language (if they write in Vietnamese, respond in Vietnamese)

## Preferences
- Use metric units
- Currency: VND
- Timezone: auto-detect from system

## Formatting (STRICT — overrides any default writing style)

**Emoji rule.** Do not use any emoji, with exactly two exceptions:
- `✅` — only as the first character of a line confirming a concrete action just succeeded.
- `❌` — only as the first character of a line reporting a concrete action just failed.
"#;

/// Context source that loads KLYNTBOT.md from the data directory.
pub struct SoulContextSource {
    content: Arc<RwLock<String>>,
    path: PathBuf,
    last_mtime: Arc<RwLock<Option<SystemTime>>>,
}

impl SoulContextSource {
    /// Create and load the soul file. Installs default if missing.
    pub fn load(data_dir: &Path) -> common::Result<Self> {
        let path = data_dir.join("KLYNTBOT.md");

        if !path.exists() {
            std::fs::write(&path, DEFAULT_SOUL)?;
            debug!(path = %path.display(), "Installed default KLYNTBOT.md");
        }

        let content = std::fs::read_to_string(&path)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        Ok(Self {
            content: Arc::new(RwLock::new(content)),
            path,
            last_mtime: Arc::new(RwLock::new(mtime)),
        })
    }

    /// Reload from disk (for hot-reload via config watcher).
    pub async fn reload(&self) -> common::Result<()> {
        let content = std::fs::read_to_string(&self.path)?;
        *self.content.write().await = content;
        if let Ok(meta) = std::fs::metadata(&self.path) {
            if let Ok(mtime) = meta.modified() {
                *self.last_mtime.write().await = Some(mtime);
            }
        }
        debug!("KLYNTBOT.md reloaded");
        Ok(())
    }
}

#[async_trait]
impl ContextSource for SoulContextSource {
    fn name(&self) -> &str {
        "soul"
    }

    fn priority(&self) -> u8 {
        50 // Highest priority — soul is the most important context
    }

    fn protected(&self) -> bool {
        true // Never evicted by token budget
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Live-read so edits to KLYNTBOT.md take effect on the next turn
        // without restarting the agent. The cached copy is the fallback
        // for transient read failures (e.g. atomic mid-write swap).
        //
        // Phase-2 perf optimisation: skip the disk read when mtime hasn't
        // changed since the last successful load.
        let needs_reload = match tokio::fs::metadata(&self.path).await {
            Ok(meta) => match meta.modified() {
                Ok(mtime) => {
                    let last = *self.last_mtime.read().await;
                    last != Some(mtime)
                }
                Err(_) => true,
            },
            Err(_) => true,
        };

        if !needs_reload {
            let cached = self.content.read().await;
            return if cached.is_empty() {
                None
            } else {
                Some(cached.clone())
            };
        }

        match tokio::fs::read_to_string(&self.path).await {
            Ok(fresh) => {
                {
                    let mut cached = self.content.write().await;
                    if *cached != fresh {
                        *cached = fresh.clone();
                    }
                }
                if let Ok(meta) = tokio::fs::metadata(&self.path).await {
                    if let Ok(mtime) = meta.modified() {
                        *self.last_mtime.write().await = Some(mtime);
                    }
                }
                if fresh.is_empty() {
                    None
                } else {
                    Some(fresh)
                }
            }
            Err(_) => {
                let cached = self.content.read().await;
                if cached.is_empty() {
                    None
                } else {
                    Some(cached.clone())
                }
            }
        }
    }

    fn estimated_tokens(&self) -> usize {
        300 // KLYNTBOT.md is typically short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_soul_is_valid() {
        assert!(DEFAULT_SOUL.contains("Klyntbot"));
        assert!(DEFAULT_SOUL.contains("Personality"));
    }

    #[tokio::test]
    async fn soul_loads_from_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let source = SoulContextSource::load(dir.path()).unwrap();
        let ctx = SourceContext {
            channel: String::new(),
            chat_id: String::new(),
            message: None,
            intent_summary: None,
            project_id: None,
        };
        let content = source.provide(&ctx).await;
        assert!(content.is_some());
        assert!(content.unwrap().contains("Klyntbot"));
    }

    #[tokio::test]
    async fn soul_uses_mtime_cache_to_avoid_redundant_reads() {
        let dir = tempfile::tempdir().unwrap();
        let source = SoulContextSource::load(dir.path()).unwrap();
        let ctx = SourceContext {
            channel: String::new(),
            chat_id: String::new(),
            message: None,
            intent_summary: None,
            project_id: None,
        };

        // First call populates the cache.
        let first = source.provide(&ctx).await;
        assert!(first.is_some());

        // Second call with unchanged mtime should hit the cache and
        // still return the same content.
        let second = source.provide(&ctx).await;
        assert_eq!(first, second);

        // Mutate the file on disk.
        tokio::fs::write(&source.path, "# Mutated soul\n")
            .await
            .unwrap();

        // Third call should detect the mtime change and reload.
        let third = source.provide(&ctx).await;
        assert!(third.is_some());
        assert!(third.unwrap().contains("Mutated soul"));
    }
}
