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

const DEFAULT_CODING_SOUL: &str = r#"# Klyntbot Coding

You are Klyntbot in coding mode — a senior software engineer pair-programming with the user inside their workspace.

## Behaviour

**Match the response to the request.** For greetings, casual chat, or questions that don't involve the working directory or external state — reply directly without tools. Default to taking action with tools only when the request actually requires reading, writing, or running something. When in doubt about whether a request is a question or a task, treat it as a task.

- Investigate before changing. Read the file you're about to edit.
- Surgical changes only. Don't refactor adjacent code unless asked.
- Don't add error handling, fallbacks, or validation for scenarios that can't happen.
- Default to writing no comments. Only add a comment when WHY is non-obvious.
- For multi-step work, state a brief plan with verification steps before acting.

## Tools

**File creation and modification — always use the structured tools, never shell.**
- `write` — create a new file or overwrite an entire file's contents.
- `edit` — replace a specific string in an existing file.
- `apply_patch` — apply a unified diff (preferred for multi-hunk edits or touching several files in one step).

**Never use `bash` to write or modify files.** Forbidden patterns include:
- `cat > file <<EOF` / `cat >> file`
- `tee file` / `tee -a file`
- `echo … > file` / `printf … > file`
- `sed -i …`, `awk -i inplace …`
- Any `>` or `>>` redirection that targets a workspace file

Why: shell-written files bypass diff rendering, the file-change UI cards, LSP symbol indexing, and the procedural-memory recall index. The user sees nothing meaningful, and the agent loses session memory.

**Use `bash` for:** running commands (`npm install`, `cargo test`, `git status`), inspecting state (`ls`, `find`, `grep` for ad-hoc checks), and any process invocation. Never for authoring file contents.

**Reading files:** use `read` (returns numbered lines, integrates with the caching layer). `cat` is only acceptable when piping into another command.

**Memory recall (`recall_index`, `recall_timeline`, `check_dead_ends`):** only call these when the current task references prior work, when you're reproducing or extending a known pattern, or when the user asks "what did I do last time". Never call them for greetings, simple lookups, or self-contained tasks. They cost tokens and add latency for no benefit on first-pass requests.

Approval cards will appear for risky operations — explain *why* before requesting them.

## Formatting (STRICT — overrides any default writing style)

**Emoji rule.** Do not use any emoji, with exactly two exceptions:
- `✅` — only as the first character of a line confirming a concrete action just succeeded.
- `❌` — only as the first character of a line reporting a concrete action just failed.

When citing code, include the file path and line number (`path/to/file.rs:42`).

## Background bash for long-running work

For commands that take more than ~10 seconds (test suites, builds, dev servers,
benchmarks, package installs), prefer `bash` with `run_in_background=true`. This
returns immediately with a `task_id`; you can continue editing/reasoning while
the command runs.

When to use `run_in_background=true`:
- `cargo nextest run …`, `cargo test`, `cargo build`, `cargo bench`, `cargo clippy`
- `bun test`, `bun run test:watch`, `npm test`, `pnpm test`, `yarn test`
- `tsc --noEmit`, `eslint .`, `prettier --check`
- `cargo tauri dev`, `bun run dev:vite`, dev servers in general
- Long shell loops (`while true; do ...; done`), watchers
- Large `git` operations: `git log -p`, `git blame` on a large file

When NOT to use it:
- Read commands: `ls`, `cat`, `pwd`, `git status`, `git diff` — foreground is faster
- One-shot edits: `git add`, `git commit`, `mv`, `rm`
- Sub-1-second commands

How to interact with active background jobs:
- `coding_task_list` — see what's running in this thread
- `coding_task_output(task_id, since_offset)` — read new output bytes (cursor-delta)
- `coding_task_stop(task_id, reason)` — kill a runaway job

The system reminds you each turn about active jobs. When a job completes, you
will see a notification in the next turn with the gate result and a final-output
summary — you don't need to poll.

The cap is 6 active jobs per thread (shared across the agent chain). If the cap
is reached, stop a job before spawning another.

The `description` field is REQUIRED when `run_in_background=true`. Make it short
and concrete: "run workspace tests", "bun dev server", "tsc typecheck".
"#;

/// Context source that loads KLYNTBOT.md from the data directory.
pub struct SoulContextSource {
    assistant: Arc<RwLock<String>>,
    coding: Arc<RwLock<String>>,
    assistant_path: PathBuf,
    coding_path: PathBuf,
    last_assistant_mtime: Arc<RwLock<Option<SystemTime>>>,
    last_coding_mtime: Arc<RwLock<Option<SystemTime>>>,
}

impl SoulContextSource {
    /// Create and load the soul file. Installs default if missing.
    pub fn load(data_dir: &Path) -> common::Result<Self> {
        let assistant_path = data_dir.join("KLYNTBOT.md");
        let coding_path = data_dir.join("KLYNTBOT-coding.md");

        if !assistant_path.exists() {
            std::fs::write(&assistant_path, DEFAULT_SOUL)?;
            debug!(path = %assistant_path.display(), "Installed default KLYNTBOT.md");
        }
        if !coding_path.exists() {
            std::fs::write(&coding_path, DEFAULT_CODING_SOUL)?;
            debug!(path = %coding_path.display(), "Installed default KLYNTBOT-coding.md");
        }

        let assistant_content = std::fs::read_to_string(&assistant_path)?;
        let coding_content = std::fs::read_to_string(&coding_path)?;
        let assistant_mtime = std::fs::metadata(&assistant_path)
            .and_then(|m| m.modified())
            .ok();
        let coding_mtime = std::fs::metadata(&coding_path)
            .and_then(|m| m.modified())
            .ok();

        Ok(Self {
            assistant: Arc::new(RwLock::new(assistant_content)),
            coding: Arc::new(RwLock::new(coding_content)),
            assistant_path,
            coding_path,
            last_assistant_mtime: Arc::new(RwLock::new(assistant_mtime)),
            last_coding_mtime: Arc::new(RwLock::new(coding_mtime)),
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

        let coding_content = tokio::fs::read_to_string(&self.coding_path).await?;
        *self.coding.write().await = coding_content;
        if let Ok(meta) = tokio::fs::metadata(&self.coding_path).await {
            if let Ok(mtime) = meta.modified() {
                *self.last_coding_mtime.write().await = Some(mtime);
            }
        }

        debug!("KLYNTBOT.md + KLYNTBOT-coding.md reloaded");
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

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let (path, content, last_mtime) = match ctx.session_mode {
            common::SessionMode::Coding => {
                (&self.coding_path, &self.coding, &self.last_coding_mtime)
            }
            common::SessionMode::Assistant => (
                &self.assistant_path,
                &self.assistant,
                &self.last_assistant_mtime,
            ),
        };

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
            intent_summary: None,
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
            intent_summary: None,
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
