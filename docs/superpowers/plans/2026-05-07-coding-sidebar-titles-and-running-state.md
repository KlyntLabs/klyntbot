# Coding Sidebar — Auto-titles, Running State, Real-Time Catch-Up — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace "Untitled session" with LLM-generated titles, group the coding sidebar into Running / Recently completed / Chats sections with real-time updates, and ensure switching back to a running session replays buffered deltas with zero loss.

**Architecture:** Three independent slices — (A) a Rust `tokio::spawn` background task on first user message that calls the cognitive provider and emits `coding:thread_updated`; (B) a frontend `ThreadEventBuffer` global singleton (built on `useSyncExternalStore`) that owns the single `agent:thread_event` Tauri listener and exposes derived `runningIds` and `recentlyCompleted` reactive hooks; (C) a new `CodingSidebarThreadsSection` component rendering the three groups, gated on `mode === "code"`.

**Tech Stack:** Rust 1.93 (workspace crates), Tauri 2, React 19 + TypeScript, Vitest + `@testing-library/react`, plain CSS.

**Spec:** `docs/superpowers/specs/2026-05-07-coding-sidebar-titles-and-running-state-design.md`

---

## File Structure

### New Rust files
- `crates/app-core/src/coding/title_service.rs` — `autogenerate_title()` background task; calls `cognitive_provider`, sanitizes, persists, emits.

### Modified Rust files
- `crates/app-core/src/coding/mod.rs` — add `pub mod title_service;`
- `crates/app-core/src/coding/turn_handler.rs` — spawn titling on first user message of unnamed session
- `crates/app-core/src/coding/thread_handler.rs` — emit `coding:thread_updated` after `rename_session`

### New TypeScript files
- `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts` — global `useSyncExternalStore` singleton
- `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`
- `desktop-ui/src/features/coding/state/partitionCodingThreads.ts` — pure partition fn (testable)
- `desktop-ui/src/features/coding/state/partitionCodingThreads.test.ts`
- `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.tsx`
- `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.test.tsx`
- `desktop-ui/src/styles/coding-sidebar.css`

### Modified TypeScript files
- `desktop-ui/src/styles/index.css` — `@import "./coding-sidebar.css";`
- `desktop-ui/src/styles/ds-tokens.css` — `--success-muted`, `--sidebar-row-active-bg`
- `desktop-ui/src/features/app/components/ThreadRow.tsx` — accept `dataStatus`, `dataActive` props; emit as data-attrs
- `desktop-ui/src/features/app/components/Sidebar.tsx` — conditionally render `CodingSidebarThreadsSection` when `mode === "code"`
- `desktop-ui/src/features/coding/hooks/useThreadEvents.ts` — refactor onto `ThreadEventBuffer.subscribeToThread`
- `desktop-ui/src/features/coding/hooks/useCodingThreadStatus.ts` — replaced by `useRunningCodingIds()`; delete after consumers updated, OR keep as a thin re-export

---

# Phase 1 — Backend: Title autogen

## Task 1: Add `title_service` module skeleton

**Files:**
- Create: `crates/app-core/src/coding/title_service.rs`
- Modify: `crates/app-core/src/coding/mod.rs`

- [ ] **Step 1: Create the module file with a stub function**

```rust
// crates/app-core/src/coding/title_service.rs
//! Background title generation for coding sessions.
//!
//! Spawned from `coding_message_send` when a session has no title yet and
//! the incoming message is its first. Calls the cognitive provider,
//! sanitizes the response, persists via `rename_session`, and emits
//! `coding:thread_updated` so the sidebar refetches.

use crate::events::AppEventEmitter;
use common::Result;
use providers::DynProvider;
use std::sync::Arc;
use storage::repos::SessionRepo;

const TITLE_TIMEOUT_SECS: u64 = 5;
const MAX_TITLE_LEN: usize = 60;
const MAX_PROMPT_LEN: usize = 500;

#[tracing::instrument(
    skip(repo, provider, emitter, first_user_message),
    fields(session_key = %session_key)
)]
pub async fn autogenerate_title(
    repo: SessionRepo,
    provider: DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    session_key: String,
    first_user_message: String,
    model: String,
) -> Result<()> {
    // TODO: implement in Task 3+
    Ok(())
}
```

- [ ] **Step 2: Wire module in `coding/mod.rs`**

Open `crates/app-core/src/coding/mod.rs` and add the line in alphabetic position:

```rust
pub mod thread_handler;
pub mod title_service;       // ← ADD THIS LINE
pub mod turn_handler;
```

- [ ] **Step 3: Verify compile**

Run: `cargo check -p app-core`
Expected: clean (no warnings beyond the existing baseline).

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs crates/app-core/src/coding/mod.rs
git commit -m "$(cat <<'EOF'
feat(coding): add title_service module skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add `sanitize_title` helper + tests

**Files:**
- Modify: `crates/app-core/src/coding/title_service.rs`

- [ ] **Step 1: Write failing test for sanitization**

Append to `crates/app-core/src/coding/title_service.rs`:

```rust
fn sanitize_title(raw: &str) -> String {
    // Implemented in Step 3
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_wrapping_quotes() {
        assert_eq!(sanitize_title("\"Refactor Sidebar\""), "Refactor Sidebar");
        assert_eq!(sanitize_title("'Refactor Sidebar'"), "Refactor Sidebar");
    }

    #[test]
    fn sanitize_strips_trailing_punctuation() {
        assert_eq!(sanitize_title("Refactor sidebar."), "Refactor sidebar");
        assert_eq!(sanitize_title("Fix bug!"), "Fix bug");
        assert_eq!(sanitize_title("What broke?"), "What broke");
    }

    #[test]
    fn sanitize_trims_whitespace_and_newlines() {
        assert_eq!(sanitize_title("  Refactor sidebar\n"), "Refactor sidebar");
    }

    #[test]
    fn sanitize_caps_at_60_chars() {
        let long = "a".repeat(120);
        assert_eq!(sanitize_title(&long).len(), MAX_TITLE_LEN);
    }

    #[test]
    fn sanitize_preserves_inner_punctuation() {
        assert_eq!(sanitize_title("Fix: parser bug"), "Fix: parser bug");
    }
}
```

- [ ] **Step 2: Run test and verify it fails**

Run: `cargo nextest run -p app-core sanitize_strips_wrapping_quotes`
Expected: FAIL — `assertion failed: left: "", right: "Refactor Sidebar"`.

- [ ] **Step 3: Implement `sanitize_title`**

Replace the stub:

```rust
fn sanitize_title(raw: &str) -> String {
    let trimmed = raw.trim();

    // Strip exactly one wrapping pair of quotes
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // Drop trailing sentence-end punctuation
    let trimmed_punct = unquoted.trim_end_matches(|c: char| matches!(c, '.' | '!' | '?'));

    // Hard-cap length (char-boundary safe)
    let mut out = String::new();
    for c in trimmed_punct.chars() {
        if out.len() + c.len_utf8() > MAX_TITLE_LEN {
            break;
        }
        out.push(c);
    }
    out.trim().to_string()
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo nextest run -p app-core --test-threads 1 sanitize_`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs
git commit -m "$(cat <<'EOF'
feat(coding): add sanitize_title helper

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Idempotency check — skip when title already set

**Files:**
- Modify: `crates/app-core/src/coding/title_service.rs`

- [ ] **Step 1: Write failing integration test**

Append to the `tests` mod in `title_service.rs`:

```rust
    use crate::events::NoopEmitter;
    use std::sync::{Arc, Mutex};

    /// Recording emitter that captures every emit_event call for assertions.
    struct RecordingEmitter(Mutex<Vec<(String, serde_json::Value)>>);

    impl RecordingEmitter {
        fn new() -> Self { Self(Mutex::new(Vec::new())) }
        fn events(&self) -> Vec<(String, serde_json::Value)> {
            self.0.lock().unwrap().clone()
        }
    }

    impl AppEventEmitter for RecordingEmitter {
        fn emit_event(&self, name: &str, payload: serde_json::Value) {
            self.0.lock().unwrap().push((name.into(), payload));
        }
    }

    async fn setup_repo() -> SessionRepo {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::repos::SessionRepo::new(pool.pool().clone())
    }

    async fn insert_session_with_title(repo: &SessionRepo, key: &str, title: Option<&str>) {
        let metadata = match title {
            Some(t) => serde_json::json!({ "title": t }),
            None => serde_json::json!({}),
        };
        repo.upsert_session_with_mode(
            key,
            "coding",
            metadata,
        ).await.unwrap();
    }

    #[tokio::test]
    async fn skips_when_title_already_set() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t1", Some("Existing Title")).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider = Arc::new(providers::FakeProvider::default());

        autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t1".into(),
            "first message".into(),
            "fake-model".into(),
        ).await.unwrap();

        let row = repo.get_session("coding:t1").await.unwrap();
        assert_eq!(
            row.metadata.get("title").and_then(|v| v.as_str()),
            Some("Existing Title")
        );
        assert!(emitter.events().is_empty(), "no emit when title preset");
    }
```

> **Note:** `providers::FakeProvider::default()` and `repo.upsert_session_with_mode()` may have slightly different signatures. If `cargo check` fails on those calls, run `grep -rn "FakeProvider\|upsert_session_with_mode" crates/providers crates/storage` to find the correct signature and update the test. Also confirm `storage::repos::SessionRepo::new` is `pub` — if not, expose it or use `Repos::from_pool(...).sessions`.

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo nextest run -p app-core skips_when_title_already_set`
Expected: FAIL — likely a "no titling occurred but no skip path implemented either" — which still passes the assertion. If so, this test is a regression net only — keep it. Move to next test where the failure is meaningful.

- [ ] **Step 3: Implement idempotency check in `autogenerate_title`**

Replace the stub body:

```rust
pub async fn autogenerate_title(
    repo: SessionRepo,
    provider: DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    session_key: String,
    first_user_message: String,
    model: String,
) -> Result<()> {
    // Re-check: user may have manually renamed in the meantime.
    let session = match repo.get_session(&session_key).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "title autogen: session lookup failed");
            return Ok(());
        }
    };
    if session
        .metadata
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        tracing::debug!("title autogen: skipping — title already set");
        return Ok(());
    }

    // TODO: LLM call (Task 4)
    let _ = (provider, emitter, first_user_message, model);
    Ok(())
}
```

- [ ] **Step 4: Run test, verify pass**

Run: `cargo nextest run -p app-core skips_when_title_already_set`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs
git commit -m "$(cat <<'EOF'
feat(coding): title autogen idempotent skip when title preset

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Happy-path LLM call + emit on success

**Files:**
- Modify: `crates/app-core/src/coding/title_service.rs`

- [ ] **Step 1: Write failing test — successful generation calls rename + emits**

Append to the `tests` mod:

```rust
    /// Test provider that returns a fixed completion.
    #[derive(Clone)]
    struct CannedProvider(String);

    #[async_trait::async_trait]
    impl providers::LlmProvider for CannedProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            params: &providers::ChatParams,
            _cache: &[providers::CacheBreakpoint],
        ) -> std::result::Result<providers::LlmResponse, providers::ProviderError> {
            Ok(providers::LlmResponse {
                content: Some(self.0.clone()),
                tool_calls: vec![],
                finish_reason: "stop".into(),
                usage: providers::Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str { "canned-model" }
    }

    #[tokio::test]
    async fn happy_path_renames_and_emits() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t2", None).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider = Arc::new(CannedProvider("\"Refactor Sidebar Layout.\"".into()));

        autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t2".into(),
            "Help me refactor the sidebar layout".into(),
            "canned-model".into(),
        ).await.unwrap();

        let row = repo.get_session("coding:t2").await.unwrap();
        assert_eq!(
            row.metadata.get("title").and_then(|v| v.as_str()),
            Some("Refactor Sidebar Layout"),
            "title should be sanitized + persisted"
        );

        let events = emitter.events();
        assert_eq!(events.len(), 1, "exactly one event emitted");
        assert_eq!(events[0].0, "coding:thread_updated");
        assert_eq!(
            events[0].1.get("thread_id").and_then(|v| v.as_str()),
            Some("coding:t2")
        );
    }
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo nextest run -p app-core happy_path_renames_and_emits`
Expected: FAIL — title is `None` (not yet implemented).

- [ ] **Step 3: Implement happy-path LLM call**

Replace the `// TODO: LLM call` block in `autogenerate_title`:

```rust
    let prompt_capped = if first_user_message.len() > MAX_PROMPT_LEN {
        let mut end = MAX_PROMPT_LEN;
        while !first_user_message.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        &first_user_message[..end]
    } else {
        &first_user_message[..]
    };

    let messages = vec![
        providers::Message::system(
            "Generate a concise 3 to 6 word title for this coding session based on the user's first request. \
             Output only the title — no quotes, no period, in Title Case."
        ),
        providers::Message::user(prompt_capped),
    ];

    let params = providers::ChatParams::new(model)
        .with_temperature(0.2)
        .with_max_tokens(24);

    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(TITLE_TIMEOUT_SECS),
        provider.chat(&messages, None, &params, &[]),
    ).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "title autogen: provider error");
            return Ok(());
        }
        Err(_) => {
            tracing::warn!("title autogen: provider timeout after {}s", TITLE_TIMEOUT_SECS);
            return Ok(());
        }
    };

    let raw_title = response.content.unwrap_or_default();
    let title = sanitize_title(&raw_title);
    if title.is_empty() {
        tracing::warn!("title autogen: empty/whitespace response, skipping persist");
        return Ok(());
    }

    if let Err(e) = repo.rename_session(&session_key, &title).await {
        tracing::error!(error = %e, "title autogen: rename failed");
        return Ok(());
    }

    emitter.emit_event(
        "coding:thread_updated",
        serde_json::json!({ "thread_id": session_key }),
    );
    tracing::info!(title = %title, "title autogen: persisted and emitted");
    Ok(())
```

- [ ] **Step 4: Run test, verify pass**

Run: `cargo nextest run -p app-core happy_path_renames_and_emits`
Expected: PASS.

- [ ] **Step 5: Run all tests in title_service**

Run: `cargo nextest run -p app-core title_service`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs
git commit -m "$(cat <<'EOF'
feat(coding): autogenerate_title LLM call + emit on success

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Provider error handling

**Files:**
- Modify: `crates/app-core/src/coding/title_service.rs`

- [ ] **Step 1: Add failing test — provider error doesn't abort, doesn't rename, doesn't emit**

```rust
    struct ErrorProvider;

    #[async_trait::async_trait]
    impl providers::LlmProvider for ErrorProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &providers::ChatParams,
            _cache: &[providers::CacheBreakpoint],
        ) -> std::result::Result<providers::LlmResponse, providers::ProviderError> {
            Err(providers::ProviderError::new("upstream 500"))
        }
        fn default_model(&self) -> &str { "err-model" }
    }

    #[tokio::test]
    async fn provider_error_is_non_fatal() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t3", None).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider = Arc::new(ErrorProvider);

        let result = autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t3".into(),
            "anything".into(),
            "err-model".into(),
        ).await;

        assert!(result.is_ok(), "provider errors must not bubble");
        let row = repo.get_session("coding:t3").await.unwrap();
        assert!(
            row.metadata.get("title").and_then(|v| v.as_str()).is_none(),
            "title remains None on provider error"
        );
        assert!(emitter.events().is_empty());
    }
```

> **Note:** `providers::ProviderError::new` may have a different constructor — adapt to whatever exists (could be `ProviderError::Other("...".into())` or similar; run `grep -rn "ProviderError" crates/providers/src/types.rs | head` to find).

- [ ] **Step 2: Run test, verify it passes**

The error path was already implemented in Task 4. This is a regression test.
Run: `cargo nextest run -p app-core provider_error_is_non_fatal`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs
git commit -m "$(cat <<'EOF'
test(coding): cover provider-error non-fatal path

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: `coding_thread_set_name` emits `coding:thread_updated`

**Files:**
- Modify: `crates/app-core/src/coding/thread_handler.rs`

- [ ] **Step 1: Locate the existing `coding_thread_set_name` (around line 330)**

Current body (verbatim from the file):
```rust
    /// Set the name/title of a coding thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_set_name(&self, thread_id: &str, name: &str) -> Result<()> {
        self.repos.sessions.rename_session(thread_id, name).await?;
        Ok(())
    }
```

- [ ] **Step 2: Modify to emit on success**

Replace with:

```rust
    /// Set the name/title of a coding thread.
    ///
    /// Emits `coding:thread_updated` so the sidebar can refetch immediately.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_set_name(&self, thread_id: &str, name: &str) -> Result<()> {
        self.repos.sessions.rename_session(thread_id, name).await?;
        self.event_emitter.emit_event(
            "coding:thread_updated",
            serde_json::json!({ "thread_id": thread_id }),
        );
        Ok(())
    }
```

- [ ] **Step 3: Verify compile**

Run: `cargo check -p app-core`
Expected: clean.

- [ ] **Step 4: Verify existing tests still pass**

Run: `cargo nextest run -p app-core thread_handler`
Expected: all PASS (no test for set_name's emit behavior since it's a one-line addition with simple semantics).

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/thread_handler.rs
git commit -m "$(cat <<'EOF'
feat(coding): emit coding:thread_updated on manual rename

Sidebar listener at useCodingSessions.ts:60 already drives a refetch
when this event arrives — wire the producer.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Spawn title autogen on first user message

**Files:**
- Modify: `crates/app-core/src/coding/turn_handler.rs`

- [ ] **Step 1: Locate the existing first-message echo**

Around lines 43–82 of `coding_message_send`, after the `TurnStarted` publish (line 73–78). The session_key is `thread_id` (already available as `&str`).

- [ ] **Step 2: Add the autogen spawn block**

Insert this **immediately after** the `TurnStarted` publish block (after the closing `});` of `self.thread_events.publish(ThreadEvent::TurnStarted { ... });`):

```rust
        // ── Autogenerate title on first user message of an unnamed session ──
        // Cheap fast-path checks before spawning: skip if cognitive provider
        // unavailable, skip if session already titled, skip if not first message.
        if let Some(provider) = self.cognitive_provider.clone() {
            let title_repo = self.repos.sessions.clone();
            let emitter = self.event_emitter.clone();
            let session_key = thread_id.to_string();
            let first_msg = text.to_string();
            let title_model = resolved_model.clone();
            tokio::spawn(async move {
                // Re-check title and message count from inside the task to avoid
                // races with concurrent first messages on the same session.
                let session = match title_repo.get_session(&session_key).await {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let already_titled = session
                    .metadata
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if already_titled {
                    return;
                }
                let msg_count = title_repo.count_messages(&session_key).await.unwrap_or(i64::MAX);
                // The user message has not been persisted yet at this point in
                // coding_message_send; count == 0 (or 1 if a system msg was inserted).
                if msg_count > 1 {
                    return;
                }
                let _ = crate::coding::title_service::autogenerate_title(
                    title_repo,
                    provider,
                    emitter,
                    session_key,
                    first_msg,
                    title_model,
                ).await;
            });
        }
```

> **Notes for the implementer:**
> - `resolved_model` is the local variable defined at lines ~62–73 of `coding_message_send` (the model resolution block). Insert *after* that block; if your editor placed the autogen before, move it down.
> - `self.repos.sessions` is `Clone` because `SessionRepo` holds an `SqlitePool` which is `Clone`.
> - If `count_messages` returns `Err`, we conservatively bail (fall through `unwrap_or(i64::MAX)` → `> 1` → return). Better to skip a title than to retry on a broken session.

- [ ] **Step 3: Verify compile**

Run: `cargo check -p app-core`
Expected: clean.

- [ ] **Step 4: Verify clippy clean**

Run: `cargo clippy -p app-core --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Run all app-core tests**

Run: `cargo nextest run -p app-core`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding/turn_handler.rs
git commit -m "$(cat <<'EOF'
feat(coding): spawn title autogen on first user message

Idempotent re-check inside the spawned task guards against concurrent
first messages racing on the same thread.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: End of Phase 1 — workspace check

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: clean.

- [ ] **Step 2: Full workspace lint**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 3: Full workspace tests**

Run: `cargo nextest run --workspace`
Expected: all PASS.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: clean. If not, run `cargo fmt --all` and amend the prior commit (or make a small fmt commit).

---

# Phase 2 — Frontend: ThreadEventBuffer

## Task 9: Pure partition function + tests

**Files:**
- Create: `desktop-ui/src/features/coding/state/partitionCodingThreads.ts`
- Create: `desktop-ui/src/features/coding/state/partitionCodingThreads.test.ts`

- [ ] **Step 1: Write failing tests**

Create `desktop-ui/src/features/coding/state/partitionCodingThreads.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { partitionCodingThreads } from "./partitionCodingThreads";
import type { ChatThread } from "@/features/chat/types";

function thread(id: string): ChatThread {
  return {
    sessionKey: id,
    title: id,
    updatedAt: new Date().toISOString(),
    messageCount: 0,
  };
}

describe("partitionCodingThreads", () => {
  it("partitions into three disjoint groups", () => {
    const sessions = [thread("a"), thread("b"), thread("c"), thread("d")];
    const running = new Set(["a", "b"]);
    const recent = new Map<string, number>([["c", Date.now()]]);

    const { running: r, recent: rc, chats } = partitionCodingThreads(
      sessions,
      running,
      recent,
    );

    expect(r.map((t) => t.sessionKey)).toEqual(["a", "b"]);
    expect(rc.map((t) => t.sessionKey)).toEqual(["c"]);
    expect(chats.map((t) => t.sessionKey)).toEqual(["d"]);
  });

  it("running takes precedence over recent (same id in both)", () => {
    const sessions = [thread("a")];
    const running = new Set(["a"]);
    const recent = new Map<string, number>([["a", Date.now()]]);

    const { running: r, recent: rc, chats } = partitionCodingThreads(
      sessions,
      running,
      recent,
    );

    expect(r.map((t) => t.sessionKey)).toEqual(["a"]);
    expect(rc).toEqual([]);
    expect(chats).toEqual([]);
  });

  it("preserves original order within each group", () => {
    const sessions = [thread("z"), thread("a"), thread("m")];
    const running = new Set<string>();
    const recent = new Map<string, number>();

    const { chats } = partitionCodingThreads(sessions, running, recent);
    expect(chats.map((t) => t.sessionKey)).toEqual(["z", "a", "m"]);
  });

  it("empty inputs produce empty groups", () => {
    const { running, recent, chats } = partitionCodingThreads(
      [],
      new Set(),
      new Map(),
    );
    expect(running).toEqual([]);
    expect(recent).toEqual([]);
    expect(chats).toEqual([]);
  });
});
```

- [ ] **Step 2: Verify test fails (file does not exist)**

Run: `cd desktop-ui && bun run test partitionCodingThreads`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `partitionCodingThreads`**

Create `desktop-ui/src/features/coding/state/partitionCodingThreads.ts`:

```ts
import type { ChatThread } from "@/features/chat/types";

export interface CodingThreadGroups {
  running: ChatThread[];
  recent: ChatThread[];
  chats: ChatThread[];
}

/**
 * Partition the flat coding-sessions list into the three sidebar groups.
 *
 * Rules:
 * - A session id in `runningIds` always lands in `running`, even if it is
 *   also present in `recent` (running takes precedence — turn restarted).
 * - A session id in `recent` (and not in `runningIds`) lands in `recent`.
 * - Everything else lands in `chats`.
 * - Within each group, the input order from `sessions` is preserved.
 */
export function partitionCodingThreads(
  sessions: ChatThread[],
  runningIds: ReadonlySet<string>,
  recent: ReadonlyMap<string, number>,
): CodingThreadGroups {
  const running: ChatThread[] = [];
  const recentArr: ChatThread[] = [];
  const chats: ChatThread[] = [];
  for (const t of sessions) {
    if (runningIds.has(t.sessionKey)) {
      running.push(t);
    } else if (recent.has(t.sessionKey)) {
      recentArr.push(t);
    } else {
      chats.push(t);
    }
  }
  return { running, recent: recentArr, chats };
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd desktop-ui && bun run test partitionCodingThreads`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/state/partitionCodingThreads.ts \
        desktop-ui/src/features/coding/state/partitionCodingThreads.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-ui): pure partition fn for sidebar groups

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: ThreadEventBuffer — initial state and reactive read hooks

**Files:**
- Create: `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- Create: `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`

- [ ] **Step 1: Write failing test for empty initial state**

Create `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`:

```ts
/** @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => {
  vi.useRealTimers();
});

describe("ThreadEventBuffer — initial state", () => {
  it("starts with empty running and recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
    expect(mod.getRunningIds().size).toBe(0);
    expect(mod.getRecentlyCompleted().size).toBe(0);
  });
});
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement skeleton**

Create `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`:

```ts
import { useSyncExternalStore } from "react";
import type { ThreadEvent } from "../hooks/useThreadEvents";

const RING_BUFFER_CAP = 500;
const RECENT_WINDOW_MS = 30 * 60 * 1000;

type RecentEntry = {
  finishedAt: number;
  timer: ReturnType<typeof setTimeout> | null;
};

type Subscriber = (event: ThreadEvent) => void;

const eventsByThread = new Map<string, ThreadEvent[]>();
const runningIds = new Set<string>();
const recentlyCompleted = new Map<string, RecentEntry>();
const threadSubscribers = new Map<string, Set<Subscriber>>();

const storeListeners = new Set<() => void>();
let runningSnapshot: ReadonlySet<string> = new Set();
let recentSnapshot: ReadonlyMap<string, number> = new Map();

function rebuildSnapshots(): void {
  runningSnapshot = new Set(runningIds);
  const m = new Map<string, number>();
  for (const [id, entry] of recentlyCompleted) m.set(id, entry.finishedAt);
  recentSnapshot = m;
}

function notify(): void {
  rebuildSnapshots();
  for (const l of storeListeners) l();
}

function subscribeStore(listener: () => void): () => void {
  storeListeners.add(listener);
  return () => storeListeners.delete(listener);
}

export function getRunningIds(): ReadonlySet<string> {
  return runningSnapshot;
}

export function getRecentlyCompleted(): ReadonlyMap<string, number> {
  return recentSnapshot;
}

export function useRunningCodingIds(): ReadonlySet<string> {
  return useSyncExternalStore(subscribeStore, getRunningIds, getRunningIds);
}

export function useRecentlyCompletedCodingIds(): ReadonlyMap<string, number> {
  return useSyncExternalStore(subscribeStore, getRecentlyCompleted, getRecentlyCompleted);
}

export const __testing = {
  reset(): void {
    eventsByThread.clear();
    runningIds.clear();
    for (const entry of recentlyCompleted.values()) {
      if (entry.timer) clearTimeout(entry.timer);
    }
    recentlyCompleted.clear();
    threadSubscribers.clear();
    storeListeners.clear();
    rebuildSnapshots();
  },
};
```

- [ ] **Step 4: Run test, verify pass**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: PASS (1).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.ts \
        desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-ui): ThreadEventBuffer skeleton + initial state

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: applyEvent — turn_started / turn_completed transitions

**Files:**
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`

- [ ] **Step 1: Add failing tests**

Append to `ThreadEventBuffer.test.ts`:

```ts
describe("ThreadEventBuffer — turn lifecycle", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("turn_started adds id to running set", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      model: "x",
      started_at: 0,
    });
    expect(mod.getRunningIds().has("coding:t1")).toBe(true);
  });

  it("turn_completed removes id from running and adds to recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      model: "x",
      started_at: 0,
    });
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 100,
      duration_ms: 100,
    });
    expect(mod.getRunningIds().has("coding:t1")).toBe(false);
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
  });

  it("turn_started after turn_completed clears recently-completed entry", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 100,
      duration_ms: 100,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-2",
      model: "x",
      started_at: 200,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
    expect(mod.getRunningIds().has("coding:t1")).toBe(true);
  });
});
```

- [ ] **Step 2: Run, verify failures**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: 3 failing — `__testing.applyEvent` not exported.

- [ ] **Step 3: Implement applyEvent**

In `ThreadEventBuffer.ts`, add this private function above `__testing`:

```ts
function pushToRingBuffer(threadId: string, event: ThreadEvent): void {
  let buf = eventsByThread.get(threadId);
  if (!buf) {
    buf = [];
    eventsByThread.set(threadId, buf);
  }
  buf.push(event);
  if (buf.length > RING_BUFFER_CAP) {
    buf.splice(0, buf.length - RING_BUFFER_CAP);
  }
}

function applyEvent(event: ThreadEvent): void {
  const threadId = (event as { thread_id?: string }).thread_id;
  if (!threadId) return;

  pushToRingBuffer(threadId, event);

  if (event.kind === "turn_started") {
    runningIds.add(threadId);
    const prev = recentlyCompleted.get(threadId);
    if (prev) {
      if (prev.timer) clearTimeout(prev.timer);
      recentlyCompleted.delete(threadId);
    }
  } else if (event.kind === "turn_completed") {
    runningIds.delete(threadId);
    const prev = recentlyCompleted.get(threadId);
    if (prev?.timer) clearTimeout(prev.timer);
    const timer = setTimeout(() => {
      recentlyCompleted.delete(threadId);
      notify();
    }, RECENT_WINDOW_MS);
    recentlyCompleted.set(threadId, {
      finishedAt: (event as { completed_at?: number }).completed_at ?? Date.now(),
      timer,
    });
  }

  // Fan-out to per-thread subscribers
  const subs = threadSubscribers.get(threadId);
  if (subs) {
    for (const cb of subs) cb(event);
  }

  notify();
}
```

Then update the `__testing` export to expose it:

```ts
export const __testing = {
  reset(): void { /* unchanged */ },
  applyEvent(event: ThreadEvent): void {
    applyEvent(event);
  },
};
```

- [ ] **Step 4: Run, verify pass**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: all 4 PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.ts \
        desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-ui): apply turn_started/turn_completed transitions

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: markThreadOpened + 30-min timer expiry

**Files:**
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`

- [ ] **Step 1: Add failing tests**

Append to `ThreadEventBuffer.test.ts`:

```ts
describe("ThreadEventBuffer — recently completed lifecycle", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("markThreadOpened removes from recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    mod.markThreadOpened("coding:t1");
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
  });

  it("30-minute timer auto-removes from recently completed", async () => {
    vi.useFakeTimers();
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    vi.advanceTimersByTime(30 * 60 * 1000 + 1);
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
  });
});
```

- [ ] **Step 2: Run, verify failures**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: 2 failing — `markThreadOpened` not exported.

- [ ] **Step 3: Implement `markThreadOpened`**

Add this exported function in `ThreadEventBuffer.ts` above `__testing`:

```ts
export function markThreadOpened(threadId: string): void {
  const entry = recentlyCompleted.get(threadId);
  if (!entry) return;
  if (entry.timer) clearTimeout(entry.timer);
  recentlyCompleted.delete(threadId);
  notify();
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.ts \
        desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-ui): markThreadOpened and 30-min recent expiry timer

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: subscribeToThread with buffered drain

**Files:**
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`

- [ ] **Step 1: Add failing test**

Append to `ThreadEventBuffer.test.ts`:

```ts
describe("ThreadEventBuffer — subscribe", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("drains buffered events first, then delivers live ones", async () => {
    const mod = await import("./ThreadEventBuffer");
    // Buffer 3 events for thread t1 BEFORE subscribing
    for (let i = 0; i < 3; i++) {
      mod.__testing.applyEvent({
        kind: "item_delta",
        thread_id: "coding:t1",
        turn_id: "turn-1",
        item_id: "i1",
        part_idx: 0,
        delta: { type: "text", append: `chunk-${i}` },
      });
    }
    const received: string[] = [];
    const unsubscribe = mod.subscribeToThread("coding:t1", (e) => {
      const d = (e as { delta?: { append?: string } }).delta;
      if (d?.append) received.push(d.append);
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2"]);

    // Now apply a live event
    mod.__testing.applyEvent({
      kind: "item_delta",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      item_id: "i1",
      part_idx: 0,
      delta: { type: "text", append: "chunk-live" },
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2", "chunk-live"]);

    unsubscribe();
    mod.__testing.applyEvent({
      kind: "item_delta",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      item_id: "i1",
      part_idx: 0,
      delta: { type: "text", append: "after-unsub" },
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2", "chunk-live"]);
  });

  it("filters by threadId — events for other threads do not reach subscriber", async () => {
    const mod = await import("./ThreadEventBuffer");
    const received: string[] = [];
    mod.subscribeToThread("coding:a", (e) => {
      received.push((e as { thread_id?: string }).thread_id ?? "");
    });
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:b",
      turn_id: "x",
      model: "m",
      started_at: 0,
    });
    expect(received).toEqual([]);
  });
});
```

- [ ] **Step 2: Run, verify failure**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: failures — `subscribeToThread` not exported.

- [ ] **Step 3: Implement subscribeToThread**

Add to `ThreadEventBuffer.ts`:

```ts
export function subscribeToThread(
  threadId: string,
  onEvent: Subscriber,
): () => void {
  // Drain buffered events synchronously
  const buf = eventsByThread.get(threadId);
  if (buf) {
    for (const e of buf) onEvent(e);
  }
  // Register for live events
  let subs = threadSubscribers.get(threadId);
  if (!subs) {
    subs = new Set();
    threadSubscribers.set(threadId, subs);
  }
  subs.add(onEvent);
  return () => {
    const cur = threadSubscribers.get(threadId);
    if (!cur) return;
    cur.delete(onEvent);
    if (cur.size === 0) threadSubscribers.delete(threadId);
  };
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.ts \
        desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-ui): subscribeToThread drains buffered then live events

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Ring buffer cap

**Files:**
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`

- [ ] **Step 1: Add failing test**

Append:

```ts
describe("ThreadEventBuffer — ring buffer", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("caps events at 500 per thread (oldest dropped)", async () => {
    const mod = await import("./ThreadEventBuffer");
    for (let i = 0; i < 600; i++) {
      mod.__testing.applyEvent({
        kind: "item_delta",
        thread_id: "coding:t1",
        turn_id: "turn",
        item_id: "i",
        part_idx: 0,
        delta: { type: "text", append: `c${i}` },
      });
    }
    const received: string[] = [];
    mod.subscribeToThread("coding:t1", (e) => {
      const d = (e as { delta?: { append?: string } }).delta;
      if (d?.append) received.push(d.append);
    });
    expect(received).toHaveLength(500);
    // Oldest 100 dropped — first remaining is c100
    expect(received[0]).toBe("c100");
    expect(received[499]).toBe("c599");
  });
});
```

- [ ] **Step 2: Run, verify it passes**

The ring buffer was implemented in Task 11 (`pushToRingBuffer`). This test confirms the cap.
Run: `cd desktop-ui && bun run test ThreadEventBuffer`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts
git commit -m "$(cat <<'EOF'
test(coding-ui): cover ring buffer 500-event cap

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: Wire global Tauri listener (init function)

**Files:**
- Modify: `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx` (or `App.tsx` — see Step 4)

- [ ] **Step 1: Add `initThreadEventBuffer` export**

Append to `ThreadEventBuffer.ts`:

```ts
import { listen } from "@tauri-apps/api/event";

let listenerAttached = false;
let unlistenFn: (() => void) | null = null;

/**
 * Attaches the single global `agent:thread_event` Tauri listener that
 * powers running-state, recently-completed, and per-thread subscribers.
 * Idempotent — calling twice is a no-op.
 *
 * Returns a cleanup function (mainly useful in tests / hot-reload).
 */
export function initThreadEventBuffer(): () => void {
  if (listenerAttached) return () => {};
  listenerAttached = true;
  let cancelled = false;
  listen<ThreadEvent>("agent:thread_event", (msg) => {
    applyEvent(msg.payload);
  }).then((un) => {
    if (cancelled) un();
    else unlistenFn = un;
  });
  return () => {
    cancelled = true;
    unlistenFn?.();
    unlistenFn = null;
    listenerAttached = false;
  };
}
```

- [ ] **Step 2: Verify compile**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Identify the right mount point**

Open `desktop-ui/src/features/app/components/MainApp.tsx` and locate where `useCodingThreadStatus()` is currently called (around line 662). The buffer init should mount once at MainApp top-level before any `useThreadEvents` consumer.

- [ ] **Step 4: Add `useEffect` mount**

Near the top of the `MainApp` function body (e.g. just under existing `useEffect`s, around the `useCodingThreadStatus()` call at line 662), add:

```tsx
import { initThreadEventBuffer } from "@/features/coding/state/ThreadEventBuffer";

// ...

useEffect(() => {
  return initThreadEventBuffer();
}, []);
```

- [ ] **Step 5: Verify build + typecheck + lint**

Run:
```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test
```
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding/state/ThreadEventBuffer.ts \
        desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "$(cat <<'EOF'
feat(coding-ui): mount global agent:thread_event listener at MainApp

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase 3 — Sidebar grouping UI

## Task 16: Design tokens (`ds-tokens.css`)

**Files:**
- Modify: `desktop-ui/src/styles/ds-tokens.css`

- [ ] **Step 1: Append new tokens**

Locate the existing `:root` declaration (typically around lines 1–80). At the end of the `:root { ... }` block but before the closing brace, append:

```css
  /* ── Coding sidebar — running state + recent completion ── */
  --success-muted: color-mix(in srgb, #22c55e 32%, transparent);
  --sidebar-row-active-bg: color-mix(in srgb, var(--brand) 10%, transparent);
  --sidebar-row-active-accent: var(--brand);
  --sidebar-running-dot: var(--brand);
  --sidebar-recent-dot: var(--success-muted);
```

- [ ] **Step 2: Verify CSS still parses (no test target — visual check)**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/ds-tokens.css
git commit -m "$(cat <<'EOF'
feat(ui): ds-tokens for coding sidebar running/recent indicators

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 17: Coding sidebar CSS

**Files:**
- Create: `desktop-ui/src/styles/coding-sidebar.css`
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Create the CSS file**

Create `desktop-ui/src/styles/coding-sidebar.css`:

```css
/* Coding sidebar — three-group layout, active highlight, running dot.
 * Activated by data-status / data-active attributes on .thread-row in
 * code mode (CodingSidebarThreadsSection). Tokens: ds-tokens.css. */

@keyframes klynt-pulse-dot {
  0%, 100% { opacity: 0.55; transform: scale(1); }
  50%      { opacity: 1.00; transform: scale(1.15); }
}

.coding-sidebar-section {
  display: flex;
  flex-direction: column;
}

.coding-sidebar-section + .coding-sidebar-section {
  margin-top: 12px;
}

.coding-sidebar-section-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  font-size: var(--fs-2xs);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--text-subtle);
}

.coding-sidebar-section-count {
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

/* Per-row indicators (data attributes set in CodingSidebarThreadRow) */
.thread-row[data-status="running"] .thread-status,
.thread-row[data-status="recent"] .thread-status {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.thread-row[data-status="running"] .thread-status {
  background: var(--sidebar-running-dot);
  animation: klynt-pulse-dot 1.4s var(--ds-ease-out) infinite;
}

.thread-row[data-status="recent"] .thread-status {
  background: var(--sidebar-recent-dot);
}

/* Stronger active highlight for code mode */
.thread-row[data-active="true"] {
  background: var(--sidebar-row-active-bg);
  box-shadow: inset 3px 0 0 0 var(--sidebar-row-active-accent);
}

.thread-row[data-active="true"] .thread-name {
  font-weight: 600;
}
```

- [ ] **Step 2: Wire in `index.css`**

Open `desktop-ui/src/styles/index.css` and add after `@import "./coding.css";`:

```css
@import "./coding-sidebar.css";
```

- [ ] **Step 3: Verify build**

Run: `cd desktop-ui && bun run build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/coding-sidebar.css \
        desktop-ui/src/styles/index.css
git commit -m "$(cat <<'EOF'
feat(ui): coding sidebar styles — group headers, dot, active highlight

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 18: ThreadRow accepts `dataStatus` and `dataActive` props

**Files:**
- Modify: `desktop-ui/src/features/app/components/ThreadRow.tsx`

- [ ] **Step 1: Add props to type**

In `ThreadRow.tsx`, in the `ThreadRowProps` type (around lines 43–67), add at the end:

```ts
  /**
   * Optional data attribute reflecting the row's runtime status. When set,
   * styles in coding-sidebar.css render a colored dot. Values: "running" |
   * "recent". Independent of `threadStatusById`, which drives the assistant-
   * mode `.thread-status` class.
   */
  dataStatus?: "running" | "recent";
  /**
   * Optional override for the row's active-state CSS attribute. When set
   * to true, applies the stronger code-mode active highlight. Defaults
   * to the existing assistant-mode `active` class behavior when omitted.
   */
  dataActive?: boolean;
```

- [ ] **Step 2: Destructure in fn signature**

In the function destructure (around lines 70–88), add:

```ts
  dataStatus,
  dataActive,
```

- [ ] **Step 3: Render data attrs on the button**

In the JSX `<button>` opening tag (around line 128), add the two attributes:

```tsx
    <button
      type="button"
      className={`thread-row ${...existing...}`}
      style={indentStyle}
      data-status={dataStatus}
      data-active={dataActive ? "true" : undefined}
      onClick={...}
      ...
```

> **Note:** `data-active` is rendered as `"true"` *or omitted* (undefined) so CSS attribute selectors `[data-active="true"]` match exactly when intended.

- [ ] **Step 4: Verify typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Verify existing tests still pass**

Run: `cd desktop-ui && bun run test ThreadRow`
Expected: clean (props are optional).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/components/ThreadRow.tsx
git commit -m "$(cat <<'EOF'
feat(ui): ThreadRow accepts optional dataStatus/dataActive props

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 19: CodingSidebarThreadsSection component + tests

**Files:**
- Create: `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.tsx`
- Create: `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.test.tsx`

- [ ] **Step 1: Write failing component test**

Create `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.test.tsx`:

```tsx
/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { ChatThread } from "@/features/chat/types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => {
  vi.restoreAllMocks();
});

function thread(id: string, title: string): ChatThread {
  return { sessionKey: id, title, updatedAt: new Date().toISOString(), messageCount: 0 };
}

describe("CodingSidebarThreadsSection", () => {
  it("renders three sections when groups are populated", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Refactor"), thread("b", "Build login"), thread("c", "Fix bug"), thread("d", "Idle one")]}
        runningIds={new Set(["a", "b"])}
        recentlyCompleted={new Map([["c", Date.now()]])}
        activeThreadId="a"
        onSelectThread={() => {}}
      />,
    );
    expect(screen.getByText(/Running/i)).toBeInTheDocument();
    expect(screen.getByText(/Recently completed/i)).toBeInTheDocument();
    expect(screen.getByText(/^Chats$/)).toBeInTheDocument();
    expect(screen.getByText("Refactor")).toBeInTheDocument();
    expect(screen.getByText("Build login")).toBeInTheDocument();
    expect(screen.getByText("Fix bug")).toBeInTheDocument();
    expect(screen.getByText("Idle one")).toBeInTheDocument();
  });

  it("collapses empty group headers", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Idle")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId={null}
        onSelectThread={() => {}}
      />,
    );
    expect(screen.queryByText(/Running/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Recently completed/i)).not.toBeInTheDocument();
    expect(screen.getByText(/^Chats$/)).toBeInTheDocument();
  });

  it("clicking a recently-completed row calls markThreadOpened and onSelectThread", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    const buf = await import("@/features/coding/state/ThreadEventBuffer");
    buf.__testing.reset();
    buf.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "a",
      turn_id: "t",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(buf.getRecentlyCompleted().has("a")).toBe(true);

    const onSelect = vi.fn();
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Done one")]}
        runningIds={new Set()}
        recentlyCompleted={buf.getRecentlyCompleted()}
        activeThreadId={null}
        onSelectThread={onSelect}
      />,
    );
    fireEvent.click(screen.getByText("Done one"));
    expect(onSelect).toHaveBeenCalledWith("a");
    expect(buf.getRecentlyCompleted().has("a")).toBe(false);
  });
});
```

- [ ] **Step 2: Run, verify failure**

Run: `cd desktop-ui && bun run test CodingSidebarThreadsSection`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the component**

Create `desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.tsx`:

```tsx
import { useMemo } from "react";
import type { ChatThread } from "@/features/chat/types";
import { partitionCodingThreads } from "../state/partitionCodingThreads";
import { markThreadOpened } from "../state/ThreadEventBuffer";

type Props = {
  sessions: ChatThread[];
  runningIds: ReadonlySet<string>;
  recentlyCompleted: ReadonlyMap<string, number>;
  activeThreadId: string | null;
  onSelectThread: (sessionKey: string) => void;
};

/**
 * Coding-mode sidebar threads section. Renders three groups (Running /
 * Recently completed / Chats), collapsing any group whose count is zero.
 *
 * Uses a minimal local row component instead of the assistant-mode
 * `ThreadRow` because coding rows have no workspace, no subagent tree,
 * no pinning, and no `threadStatusById` pipeline — just title + dot +
 * active highlight.
 */
export function CodingSidebarThreadsSection({
  sessions,
  runningIds,
  recentlyCompleted,
  activeThreadId,
  onSelectThread,
}: Props) {
  const { running, recent, chats } = useMemo(
    () => partitionCodingThreads(sessions, runningIds, recentlyCompleted),
    [sessions, runningIds, recentlyCompleted],
  );

  const handleSelect = (sessionKey: string) => {
    markThreadOpened(sessionKey);
    onSelectThread(sessionKey);
  };

  return (
    <>
      {running.length > 0 && (
        <Section
          title="Running"
          count={running.length}
          rows={running}
          status="running"
          activeThreadId={activeThreadId}
          onSelect={handleSelect}
        />
      )}
      {recent.length > 0 && (
        <Section
          title="Recently completed"
          count={recent.length}
          rows={recent}
          status="recent"
          activeThreadId={activeThreadId}
          onSelect={handleSelect}
        />
      )}
      <Section
        title="Chats"
        count={chats.length}
        rows={chats}
        status={undefined}
        activeThreadId={activeThreadId}
        onSelect={handleSelect}
      />
    </>
  );
}

function Section({
  title,
  count,
  rows,
  status,
  activeThreadId,
  onSelect,
}: {
  title: string;
  count: number;
  rows: ChatThread[];
  status: "running" | "recent" | undefined;
  activeThreadId: string | null;
  onSelect: (sessionKey: string) => void;
}) {
  return (
    <div className="coding-sidebar-section">
      <div className="coding-sidebar-section-header">
        <span>{title}</span>
        <span className="coding-sidebar-section-count">{count}</span>
      </div>
      {rows.map((t) => (
        <button
          type="button"
          key={t.sessionKey}
          className={`thread-row${activeThreadId === t.sessionKey ? " active" : ""}`}
          data-status={status}
          data-active={activeThreadId === t.sessionKey ? "true" : undefined}
          onClick={() => onSelect(t.sessionKey)}
        >
          <span className="thread-status" aria-hidden />
          <div className="thread-content">
            <div className="thread-headline">
              <span className="thread-name">{t.title}</span>
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd desktop-ui && bun run test CodingSidebarThreadsSection`
Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.tsx \
        desktop-ui/src/features/coding/components/CodingSidebarThreadsSection.test.tsx
git commit -m "$(cat <<'EOF'
feat(coding-ui): CodingSidebarThreadsSection 3-group component

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 20: Mount CodingSidebarThreadsSection in Sidebar.tsx when mode === "code"

**Files:**
- Modify: `desktop-ui/src/features/app/components/Sidebar.tsx`

- [ ] **Step 1: Locate the existing branch**

Around line 932 in `Sidebar.tsx`:

```tsx
{isThreadsOnlyMode ? (
  <SidebarThreadsOnlySection ... />
) : (
  <SidebarWorkspaceGroups ... />
)}
```

- [ ] **Step 2: Add a code-mode branch**

Replace with:

```tsx
{mode === "code" ? (
  <CodingSidebarThreadsSection
    sessions={codingThreads}
    runningIds={runningCodingIds}
    recentlyCompleted={recentlyCompletedCodingIds}
    activeThreadId={activeThreadId}
    onSelectThread={(sessionKey) => onSelectThread(sessionKey, /* fall-through workspace handled in caller */ "")}
  />
) : isThreadsOnlyMode ? (
  <SidebarThreadsOnlySection ... />
) : (
  <SidebarWorkspaceGroups ... />
)}
```

> **Implementer notes:**
> - `mode`, `codingThreads`, `runningCodingIds`, `recentlyCompletedCodingIds`, `activeThreadId`, `onSelectThread` must be in scope. Add them to the `SidebarProps` type and destructure in the fn body if not already present.
> - `mode` flows from `useAppMode()` and is already threaded into `useMainAppLayoutSurfaces.ts`. Add it to the layout-surfaces context (it likely already is) and pass as a prop to `Sidebar`.
> - For `runningCodingIds` and `recentlyCompletedCodingIds`, the cleanest pattern is for `Sidebar.tsx` to call `useRunningCodingIds()` and `useRecentlyCompletedCodingIds()` directly inside its body. Do this — don't prop-drill the whole reactive set.

- [ ] **Step 3: Add new imports at top of Sidebar.tsx**

```tsx
import { useAppMode } from "@/features/app/hooks/useAppMode";
import { useRunningCodingIds, useRecentlyCompletedCodingIds } from "@/features/coding/state/ThreadEventBuffer";
import { CodingSidebarThreadsSection } from "@/features/coding/components/CodingSidebarThreadsSection";
```

- [ ] **Step 4: Inside the Sidebar function body**

Add near the top of the function:

```tsx
const { mode } = useAppMode();
const runningCodingIds = useRunningCodingIds();
const recentlyCompletedCodingIds = useRecentlyCompletedCodingIds();
const codingThreads = mode === "code" ? threadsList /* the flat list */ : [];
```

> **Implementer note:** Replace `threadsList` with whatever flat list of `ChatThread[]` is being supplied to the existing thread-only section. In code mode this list IS the coding sessions (per `useMainAppLayoutSurfaces.ts:1023`).

- [ ] **Step 5: Adjust onSelectThread for code mode**

In the same fn body, the existing `onSelectThread(workspaceId, threadId)` signature mismatches `CodingSidebarThreadsSection`'s `(sessionKey: string)`. Wrap:

```tsx
const handleCodingSelect = (sessionKey: string) => {
  // workspaceId for coding sessions is resolved by the layout surfaces handler
  // (useMainAppLayoutSurfaces.ts:1027 looks it up via workspaceIdBySessionKey).
  // Pass empty string so the existing handler's `if (mode === "code")` branch
  // takes over; that branch ignores the workspaceId arg.
  onSelectThread("", sessionKey);
};
```

Then wire `handleCodingSelect` into the `<CodingSidebarThreadsSection onSelectThread={handleCodingSelect} />` in Step 2 (replace the inline arrow).

- [ ] **Step 6: Verify build + typecheck**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 7: Run sidebar-related tests**

Run: `cd desktop-ui && bun run test Sidebar`
Expected: existing tests still pass.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/app/components/Sidebar.tsx
git commit -m "$(cat <<'EOF'
feat(ui): mount CodingSidebarThreadsSection when mode === code

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase 4 — useThreadEvents refactor + cleanup

## Task 21: Refactor `useThreadEvents` onto `subscribeToThread`

**Files:**
- Modify: `desktop-ui/src/features/coding/hooks/useThreadEvents.ts`

- [ ] **Step 1: Replace the listener block**

Locate the `useEffect` body in `useThreadEvents` (lines ~244–290). Currently it calls `listen<ThreadEvent>("agent:thread_event", ...)` directly. Replace with `subscribeToThread`:

```ts
import { subscribeToThread } from "../state/ThreadEventBuffer";

// ...inside useThreadEvents useEffect, replacing the listen() block:

useEffect(() => {
  setState(initialState);
  if (!threadId) return;
  let cancelled = false;

  // Seed persisted history from DB (pre-buffer events).
  invoke<{ items?: MessageDto[] }>("coding_thread_resume", {
    threadId,
    includeItems: true,
  })
    .then((thread) => {
      if (cancelled) return;
      const items = thread?.items ?? [];
      if (items.length === 0) return;
      setState((prev) => ({ ...prev, items }));
    })
    .catch(() => {});

  // Subscribe via the global buffer — drains buffered events first,
  // then receives live ones. Replaces the per-mount listen() call.
  const unsubscribe = subscribeToThread(threadId, (evt) => {
    if (cancelled) return;
    setState((prev) => applyThreadEvent(prev, evt));
  });

  return () => {
    cancelled = true;
    unsubscribe();
  };
}, [threadId]);
```

> **Implementer note:** Remove the now-unused `listen` import from `@tauri-apps/api/event` (the buffer owns the global listener now). The `applyThreadEvent` reducer keeps its existing exported signature.

- [ ] **Step 2: Verify typecheck + tests**

Run: `cd desktop-ui && bun run typecheck && bun run test useThreadEvents`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useThreadEvents.ts
git commit -m "$(cat <<'EOF'
refactor(coding-ui): useThreadEvents subscribes via ThreadEventBuffer

Replaces per-mount listen() with global buffer subscription, eliminating
the gap where late deltas arriving before subscriber attach were dropped.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 22: Replace `useCodingThreadStatus` with `useRunningCodingIds`

**Files:**
- Modify: `desktop-ui/src/features/coding/hooks/useCodingThreadStatus.ts`
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`

- [ ] **Step 1: Replace hook body to read from buffer**

Replace `useCodingThreadStatus.ts` contents:

```ts
import { useMemo } from "react";
import { useRunningCodingIds } from "@/features/coding/state/ThreadEventBuffer";

type CodingStatus = {
  isProcessing: boolean;
};

/**
 * Per-thread "is a turn in flight?" map for coding threads, derived from
 * the global ThreadEventBuffer running set. Replaces the standalone
 * `agent:thread_event` listener that lived in this file pre-Phase 4.
 *
 * Keeps the same `Record<threadId, { isProcessing }>` shape so existing
 * call sites (the assistant-mode threadStatusById merge in MainApp) need
 * no further changes.
 */
export function useCodingThreadStatus(): Record<string, CodingStatus> {
  const running = useRunningCodingIds();
  return useMemo(() => {
    const map: Record<string, CodingStatus> = {};
    for (const id of running) map[id] = { isProcessing: true };
    return map;
  }, [running]);
}
```

- [ ] **Step 2: Verify typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean (consumers' shapes unchanged).

- [ ] **Step 3: Run all frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useCodingThreadStatus.ts
git commit -m "$(cat <<'EOF'
refactor(coding-ui): useCodingThreadStatus reads from ThreadEventBuffer

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase 5 — Verification + manual smoke test

## Task 23: Workspace verification gates

- [ ] **Step 1: Format**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 3: Rust tests**

Run: `cargo nextest run --workspace`
Expected: all PASS.

- [ ] **Step 4: Doctest**

Run: `cargo test --workspace --doc`
Expected: clean.

- [ ] **Step 5: Frontend typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 6: Frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 7: Frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: all PASS.

- [ ] **Step 8: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: clean.

If any of these fail, fix the underlying issue and re-run before proceeding to Task 24.

---

## Task 24: Manual smoke test in `cargo tauri dev`

This task is not automated — it's a manual end-to-end check that the wiring works in practice.

- [ ] **Step 1: Set up dev environment**

In a fresh terminal:
```bash
cd desktop-ui && bun run dev
```

In another terminal:
```bash
cargo tauri dev
```

(Use `KLYNTBOT_HOME=~/.klyntbot-dev` in `.env` to keep config separate from production.)

- [ ] **Step 2: Verify auto-titling**

1. Switch to **Code** mode in the UI.
2. Click "New chat".
3. Send the message: `Help me refactor the sidebar layout`.
4. Within ~2 seconds, the sidebar entry should change from "Untitled session" to a 3–6 word title (e.g. "Refactor Sidebar Layout").

Expected: title appears in the sidebar without page reload.

- [ ] **Step 3: Verify running indicator**

1. With the same session active, send another message.
2. While the model is responding, confirm:
   - The session row shows a pulsing dot.
   - The sidebar has a "Running (1)" header above the row.
3. After the turn completes, confirm:
   - Pulsing dot becomes a solid dot.
   - Header changes to "Recently completed (1)" (since you're still on the session — the session moves into Chats once you switch away).

> **Note:** For the "Recently completed" group to appear, you must switch *away* from the active session and have a turn complete on a *different* session. Easier test: open two sessions, send a long-running message in session A, switch to session B, wait for A to finish — A should appear in "Recently completed" in B's view.

- [ ] **Step 4: Verify catch-up replay**

1. In session A, send a message that produces a long stream (e.g. "list 100 things").
2. Mid-stream, switch to session B.
3. Wait ~3 seconds.
4. Switch back to session A.
5. Confirm all the chunks that streamed while you were on B are visible (no gap, no truncation, no "Connecting…" lag).

- [ ] **Step 5: Verify active-state highlight**

1. In Code mode, click various sidebar entries.
2. Confirm the selected row has:
   - A vertical accent bar on the left (3px wide, brand color).
   - A tinted background.
   - Bold title text.

- [ ] **Step 6: Verify manual rename**

1. Right-click a session → Rename → enter a new title.
2. Confirm the new title appears in the sidebar within ~100 ms (no page reload).

- [ ] **Step 7: Verify graceful degradation when cognitive provider is unset**

1. In `~/.klyntbot-dev/config.json`, set `cognitive.provider` to `null` (or remove the section).
2. Restart `cargo tauri dev`.
3. Create a new coding session and send a first message.
4. Confirm: the session keeps showing "Untitled session" (no error, no crash). Manual rename still works.

- [ ] **Step 8: If all manual checks pass — final commit**

If any commits remain unstaged from manual fix-ups during smoke testing, commit them now. Otherwise, this task is complete.

```bash
git status
# (should be clean)
```

---

# Self-Review Checklist

This is the implementer's responsibility — run through it before declaring the plan complete.

**Spec coverage:**
- [x] (A) Auto-titling: Tasks 1–7 (title_service module, sanitize, idempotency, happy path, error handling, set_name emit, turn_handler spawn)
- [x] (B) Sidebar grouping UI: Tasks 16–20 (tokens, CSS, ThreadRow data attrs, CodingSidebarThreadsSection, Sidebar.tsx wire-up)
- [x] (C) Real-time catch-up: Tasks 9–15 (partition fn, ThreadEventBuffer skeleton/lifecycle/timer/subscribe/cap/init)
- [x] useThreadEvents refactor: Task 21
- [x] useCodingThreadStatus replacement: Task 22
- [x] Verification gates: Task 23
- [x] Manual smoke test: Task 24

**Type/method consistency:**
- `markThreadOpened(threadId)` — exported from ThreadEventBuffer (Task 12), called by CodingSidebarThreadsSection (Task 19) and verified in tests (Task 19)
- `subscribeToThread(threadId, onEvent): () => void` — exported from ThreadEventBuffer (Task 13), consumed by useThreadEvents (Task 21)
- `useRunningCodingIds(): ReadonlySet<string>` and `useRecentlyCompletedCodingIds(): ReadonlyMap<string, number>` — exported from ThreadEventBuffer (Task 10), consumed by Sidebar.tsx (Task 20) and useCodingThreadStatus (Task 22)
- `partitionCodingThreads(sessions, runningIds, recent)` — Task 9, consumed in CodingSidebarThreadsSection (Task 19)
- `autogenerate_title(repo, provider, emitter, session_key, first_user_message, model)` — Task 1 signature, used throughout Tasks 2–7
- `event_emitter.emit_event("coding:thread_updated", json!({...}))` — used in Task 4 (title_service success path) and Task 6 (manual rename) — same shape

**Open notes for the implementer:**
- If `providers::FakeProvider` does not exist (Task 3 test setup), search `grep -rn "fake.*Provider\|MockProvider" crates/providers` for the equivalent. The test uses it only as a fallback when no LLM call should occur, so any no-op provider impl works.
- If `SessionRepo::new` is not `pub`, expose it (or use `Repos::from_pool(pool).sessions` instead inside the test).
- The `ThreadEvent` TS union does not currently include a `kind: "error"` variant; if the spec's "removed on `turn_completed` / `error`" path becomes important, add the variant to the union and a corresponding case in `applyEvent` — but it's not needed for the basic running-state lifecycle.

---

# Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-07-coding-sidebar-titles-and-running-state.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration. Best for a 24-task plan with mixed Rust/TS/CSS surfaces.

**2. Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints for review.

Which approach?
