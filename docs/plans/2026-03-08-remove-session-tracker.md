# Remove Session Tracker Feature

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Completely remove the session tracker feature (Claude Code session monitoring, brainstorm panel, session mirror) from the codebase.

**Architecture:** Delete the `feature-session-tracker` crate entirely. Remove all integration points from `app-core`, `desktop`, `desktop-shared`, and `desktop-ui`. The core `session` crate (chat session management for LLM conversations) remains untouched.

**Tech Stack:** Rust workspace, Tauri 2, React/TypeScript (Vite + Biome)

---

### Task 1: Remove `feature-session-tracker` from workspace

**Files:**
- Modify: `Cargo.toml` (root workspace)
- Delete: `crates/feature-session-tracker/` (entire directory)

**Step 1: Remove from workspace members and dependencies**

In root `Cargo.toml`, remove these lines:
- Line 30: `"crates/feature-session-tracker",` from `[workspace.members]`
- Line 64: `feature-session-tracker = { path = "crates/feature-session-tracker" }` from `[workspace.dependencies]`

**Step 2: Delete the crate directory**

```bash
rm -rf crates/feature-session-tracker
```

**Step 3: Verify workspace compiles without the crate**

Skip this — dependents will break. We fix them in subsequent tasks.

---

### Task 2: Remove session tracker from `app-core`

**Files:**
- Modify: `crates/app-core/Cargo.toml` — remove `feature-session-tracker` dependency
- Modify: `crates/app-core/src/state.rs` — remove `session_tracker_repos` field and import
- Modify: `crates/app-core/src/init.rs` — remove session tracker migration, repos creation, watcher service startup, and `session_watcher_rx` from EventChannels
- Delete: `crates/app-core/src/handlers/session_tracker.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` — remove `pub mod session_tracker;`
- Delete: `crates/app-core/src/services/session_watcher.rs`
- Modify: `crates/app-core/src/services/mod.rs` — remove `pub mod session_watcher;`

**Step 1: Remove dependency from Cargo.toml**

In `crates/app-core/Cargo.toml`, remove line 16: `feature-session-tracker = { workspace = true }`

**Step 2: Clean up `state.rs`**

Remove the import (line 14): `use feature_session_tracker::repos::SessionTrackerRepos;`

Remove the field (lines 68-69):
```rust
    /// Session tracker repos (always available).
    pub session_tracker_repos: SessionTrackerRepos,
```

**Step 3: Clean up `init.rs`**

Remove the import (line 13): `use feature_session_tracker::repos::SessionTrackerRepos;`

Remove the session tracker migration block (lines 78-86):
```rust
        // Run session tracker migrations.
        let st_pool = storage_pool.inner().clone();
        StoragePool::run_feature_migrations(
            &st_pool,
            &feature_session_tracker::SessionTrackerFeature::migrations_static(),
        )
        .await
        .map_err(|e| format!("session tracker migration failed: {e}"))?;
        let session_tracker_repos = SessionTrackerRepos::new(st_pool);
```

Remove session watcher startup (lines 182-189):
```rust
        // Start session watcher service (optional — graceful if ~/.claude missing).
        let session_watcher_rx = crate::services::session_watcher::start(
            session_tracker_repos.clone(),
            shutdown_token.clone(),
        );
        if session_watcher_rx.is_some() {
            info!("session watcher service started");
        }
```

Remove `session_tracker_repos` from the `AppCore` struct literal (line 344):
```rust
            session_tracker_repos,
```

Remove `session_watcher_rx` from `EventChannels` struct definition (lines 32-33):
```rust
    pub session_watcher_rx:
        Option<mpsc::Receiver<crate::services::session_watcher::SessionWatcherEvent>>,
```

Remove `session_watcher_rx` from the `EventChannels` literal (line 359):
```rust
            session_watcher_rx,
```

**Step 4: Delete handler and service files**

```bash
rm crates/app-core/src/handlers/session_tracker.rs
rm crates/app-core/src/services/session_watcher.rs
```

**Step 5: Remove module declarations**

In `crates/app-core/src/handlers/mod.rs`, remove: `pub mod session_tracker;`
In `crates/app-core/src/services/mod.rs`, remove: `pub mod session_watcher;`

If `services/mod.rs` becomes empty, delete the file and remove `pub mod services;` from `lib.rs` (if applicable). Otherwise leave the empty mod.

**Step 6: Verify `app-core` compiles**

```bash
cargo check -p app-core
```

---

### Task 3: Remove session tracker from `desktop` crate

**Files:**
- Modify: `crates/desktop/Cargo.toml` — remove `feature-session-tracker` dependency
- Delete: `crates/desktop/src/commands/session_tracker.rs`
- Modify: `crates/desktop/src/commands/mod.rs` — remove `pub mod session_tracker;`
- Modify: `crates/desktop/src/main.rs` — remove session tracker command registrations (lines 309-320)
- Modify: `crates/desktop/src/app_core.rs` — remove session watcher event forwarding (lines 184-215)
- Modify: `crates/desktop/src/dev_server.rs` — remove session tracker command handlers + `feature_session_tracker` import

**Step 1: Remove dependency**

In `crates/desktop/Cargo.toml`, remove: `feature-session-tracker = { workspace = true }`

**Step 2: Delete commands file**

```bash
rm crates/desktop/src/commands/session_tracker.rs
```

**Step 3: Remove module declaration**

In `crates/desktop/src/commands/mod.rs`, remove: `pub mod session_tracker;`

**Step 4: Remove command registrations from `main.rs`**

Remove lines 309-320 (the `// Session Tracker` comment and all 11 `commands::session_tracker::*` entries).

**Step 5: Remove session watcher forwarding from `app_core.rs`**

Remove lines 184-215 (the `// Session watcher → Tauri events` block).

**Step 6: Remove session tracker handlers from `dev_server.rs`**

Remove the entire `// ── Session Tracker ──` section (lines 1101-1239), including all command handlers: `get_tracked_sessions`, `get_session_messages`, `sync_sessions`, `pin_session_message`, `unpin_session_message`, `get_pinned_messages`, `send_to_claude_code`, `create_brainstorm`, `list_brainstorms`, `get_brainstorm_messages`, `get_session_context`, `send_brainstorm_message`, `edit_brainstorm_message`.

Also remove the `feature_session_tracker` import if present.

**Step 7: Verify desktop crate compiles**

```bash
cargo check -p desktop
```

---

### Task 4: Remove session tracker types from `desktop-shared`

**Files:**
- Modify: `crates/desktop-shared/src/events.rs` — remove session/brainstorm event constants and payload structs
- Modify: `crates/desktop-shared/src/commands.rs` — remove session tracker param types (lines 957-990)

**Step 1: Remove events**

In `events.rs`, remove lines 287-328:
- Comment `// ── Session Tracker Events ──`
- Constants: `SESSION_NEW`, `SESSION_MESSAGE`, `SESSION_STATUS`, `BRAINSTORM_TOKEN`, `BRAINSTORM_COMPLETE`
- Structs: `SessionMessagePayload`, `SessionNewPayload`, `SessionStatusChangedPayload`, `BrainstormTokenPayload`, `BrainstormCompletePayload`

**Step 2: Remove command types**

In `commands.rs`, remove lines 957-990:
- Comment `// ── Session Tracker ──`
- Structs: `PinMessageParams`, `CreateBrainstormParams`, `SendBrainstormParams`, `SendToClaudeCodeParams`

**Step 3: Verify `desktop-shared` compiles**

```bash
cargo check -p desktop-shared
```

---

### Task 5: Remove session tracker from `desktop-ui` frontend

**Files:**
- Delete: `desktop-ui/src/components/sessions/` (entire directory — 7 files)
- Delete: `desktop-ui/src/components/views/Sessions.tsx`
- Delete: `desktop-ui/src/components/views/SessionDetail.tsx`
- Delete: `desktop-ui/src/components/views/BrainstormChat.tsx`
- Delete: `desktop-ui/src/hooks/useSessionStream.ts`
- Delete: `desktop-ui/src/hooks/useBrainstormStream.ts`
- Delete: `desktop-ui/src/lib/session-types.ts`
- Modify: `desktop-ui/src/App.tsx` — remove session lazy imports and routes
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` — remove Sessions nav item and Monitor icon import
- Modify: `desktop-ui/src/lib/types.ts` — remove `"Sessions"` from `SidebarItem` union type

**Step 1: Delete all session component/hook/type files**

```bash
rm -rf desktop-ui/src/components/sessions
rm desktop-ui/src/components/views/Sessions.tsx
rm desktop-ui/src/components/views/SessionDetail.tsx
rm desktop-ui/src/components/views/BrainstormChat.tsx
rm desktop-ui/src/hooks/useSessionStream.ts
rm desktop-ui/src/hooks/useBrainstormStream.ts
rm desktop-ui/src/lib/session-types.ts
```

**Step 2: Clean up `App.tsx`**

Remove lazy imports (lines 112-120):
```tsx
const Sessions = lazy(() => ...);
const SessionDetail = lazy(() => ...);
const BrainstormChat = lazy(() => ...);
```

Remove routes (lines 191-193):
```tsx
{ path: "/sessions", element: <Sessions /> },
{ path: "/sessions/:sessionId", element: <SessionDetail /> },
{ path: "/sessions/:sessionId/brainstorm/:brainstormId", element: <BrainstormChat /> },
```

**Step 3: Clean up `Sidebar.tsx`**

Remove `Monitor` from the lucide-react import.

Remove the Sessions item from the `items` array:
```tsx
{ key: "Sessions", icon: Monitor, path: "/sessions" },
```

**Step 4: Clean up `lib/types.ts`**

Remove `| "Sessions"` from the `SidebarItem` type union (line 851).

**Step 5: Verify frontend builds**

```bash
cd desktop-ui && bun run build
```

---

### Task 6: Full workspace verification

**Step 1: Build full workspace**

```bash
cargo build --workspace
```

**Step 2: Run all tests**

```bash
cargo nextest run --workspace
```

**Step 3: Check clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

**Step 4: Check formatting**

```bash
cargo fmt --all --check
```

**Step 5: Verify frontend lint**

```bash
cd desktop-ui && bun run lint:fix
```

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: remove session tracker feature"
```
