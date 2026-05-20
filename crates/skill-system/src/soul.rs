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

## Memory recovery

When you genuinely don't recall something from prior conversation memory, you MAY call the `memory` tool with `action=search_all` and the named entities from the user's question (people, places, dates, specific topics) before answering. Search before refusing. If the search returns nothing relevant, say so plainly — never fabricate details.
"#;

/// Context source that loads KLYNTBOT.md from the data directory.
pub struct SoulContextSource {
    assistant: Arc<RwLock<String>>,
    assistant_path: PathBuf,
    last_assistant_mtime: Arc<RwLock<Option<SystemTime>>>,
}

impl SoulContextSource {
    /// Create and load the soul file. Installs default if missing.
    pub fn load(data_dir: &Path) -> common::Result<Self> {
        let assistant_path = data_dir.join("KLYNTBOT.md");

        if !assistant_path.exists() {
            std::fs::write(&assistant_path, DEFAULT_SOUL)?;
            debug!(path = %assistant_path.display(), "Installed default KLYNTBOT.md");
        }

        let assistant_content = std::fs::read_to_string(&assistant_path)?;
        let assistant_mtime = std::fs::metadata(&assistant_path)
            .and_then(|m| m.modified())
            .ok();

        Ok(Self {
            assistant: Arc::new(RwLock::new(assistant_content)),
            assistant_path,
            last_assistant_mtime: Arc::new(RwLock::new(assistant_mtime)),
        })
    }

    /// Reload from disk (for hot-reload via config watcher).
    pub async fn reload(&self) -> common::Result<()> {
        let assistant_content = tokio::fs::read_to_string(&self.assistant_path).await?;
        *self.assistant.write().await = assistant_content;
        if let Ok(meta) = tokio::fs::metadata(&self.assistant_path).await {
            if let Ok(mtime) = meta.modified() {
                *self.last_assistant_mtime.write().await = Some(mtime);
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
        let (path, content, last_mtime) = (
            &self.assistant_path,
            &self.assistant,
            &self.last_assistant_mtime,
        );

        // Live-read so edits to KLYNTBOT.md take effect on the next turn
        // without restarting the agent. The cached copy is the fallback
        // for transient read failures (e.g. atomic mid-write swap).
        //
        // Phase-2 perf optimisation: skip the disk read when mtime hasn't
        // changed since the last successful load.
        // Read metadata first to avoid TOCTOU: if the file changes between
        // metadata and read_to_string, the *next* turn will see the newer
        // mtime and reload. We never store an mtime newer than the content.
        let (needs_reload, current_mtime) = match tokio::fs::metadata(path).await {
            Ok(meta) => match meta.modified() {
                Ok(mtime) => {
                    let last = *last_mtime.read().await;
                    (last != Some(mtime), Some(mtime))
                }
                Err(_) => (true, None),
            },
            Err(_) => (true, None),
        };

        if !needs_reload {
            let cached = content.read().await;
            return if cached.is_empty() {
                None
            } else {
                Some(cached.clone())
            };
        }

        match tokio::fs::read_to_string(path).await {
            Ok(fresh) => {
                {
                    let mut cached = content.write().await;
                    if *cached != fresh {
                        *cached = fresh.clone();
                    }
                }
                if let Some(mtime) = current_mtime {
                    *last_mtime.write().await = Some(mtime);
                }
                if fresh.is_empty() {
                    None
                } else {
                    Some(fresh)
                }
            }
            Err(_) => {
                let cached = content.read().await;
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
            project_id: None,
            session_mode: common::SessionMode::Assistant,
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
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        // First call populates the cache.
        let first = source.provide(&ctx).await;
        assert!(first.is_some());

        // Second call with unchanged mtime should hit the cache and
        // still return the same content.
        let second = source.provide(&ctx).await;
        assert_eq!(first, second);

        // Mutate the file on disk.
        tokio::fs::write(&source.assistant_path, "# Mutated soul\n")
            .await
            .unwrap();

        // Third call should detect the mtime change and reload.
        let third = source.provide(&ctx).await;
        assert!(third.is_some());
        assert!(third.unwrap().contains("Mutated soul"));
    }
}
