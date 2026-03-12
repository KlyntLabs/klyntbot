# AI Coding Rules for Klyntbot
> Version: 1.0 | Created: 2026-03-12
> These rules apply to ALL contributors — human and AI agents alike.
> Violating any rule tagged 🔴 MUST will cause a build, test, or review failure.

---

## 0. The Prime Directive

When performing a **structural refactor**: NEVER change logic, function signatures, trait method signatures, return types, SQL queries, struct field names, or observable behavior. Move code. Do not rewrite it.

When adding a **new feature**: follow the patterns in this document exactly. Do not invent new patterns without an ADR.

---

## 1. Module Visibility Hierarchy

```
pub             → consumed by other workspace crates or the klyntbot facade binary
pub(crate)      → shared within one crate, not part of its public contract
pub(super)      → implementation helpers visible only to the parent module
(default private) → module-internal state only
```

**Rules:**
- 🔴 MUST: Every type, function, and trait exported from `lib.rs` must be `pub`. Everything else defaults to `pub(crate)` or lower.
- 🔴 MUST: Repository row types (`*Row` structs) must be `pub(crate)` — they are never part of any crate's public API.
- 🟡 SHOULD: Concrete tool structs should be `pub(crate)`. External callers use `ToolRegistry`.
- 🟡 SHOULD: Handler trait implementations should be `pub(crate)` — they are injected via `Arc<dyn Trait>`.

---

## 2. Dependency Layer Rules (9-Layer Architecture)

```
L0: common           ← no workspace imports
L1: config, bus, tools-core, tools-core-macros
L2: storage, domain, providers, session, scheduling
L3: context_engine, cognitive
L4: tools, channels, feature-*, plugin-runtime, activity-log, mcp
L5: agent
L6: app-core
L7: desktop-shared, desktop
L8: klyntbot (facade)
```

- 🔴 MUST: A layer may only import from **lower-numbered** layers. No circular deps.
- 🔴 MUST: `common` imports zero workspace crates.
- 🔴 MUST: `feature-*` crates do NOT import `agent`. If agent-level behavior is needed, define a handler trait in the feature crate and implement it in `agent`.
- 🟡 SHOULD: Prefer importing from the nearest lower layer, not skipping layers.

---

## 3. Adding a New Tool

**Location:** `crates/tools/src/{domain}/{tool_name}.rs`

**Domains:**
- `ai/` — memory, learning, annotation, context expansion, delegation
- `system/` — filesystem, shell spawn, glob, grep
- `web/` — browser automation, web fetch
- `productivity/` — OKR, project, area, cron scheduler
- `interaction/` — user questions, docs lookup
- `agent/` — agent spawning and management

**Checklist:**
1. Create the struct in the correct domain folder
2. Add `#[derive(Tool, ToolParams)]` — never manually implement `Tool`
3. Implement `ToolExecute<P>` with a meaningful `NAME` and `DESCRIPTION` (used by LLM)
4. Keep the `DESCRIPTION` under 200 characters — be specific about what the tool does and does NOT do
5. For multi-action tools: use `#[tool_actions]` + `#[derive(ActionParams)]` — see `crates/tools/src/system/filesystem/` for the canonical example
6. Register in `ToolRegistry::default()` in `crates/tools/src/registry.rs`
7. Add to the relevant agent profile's `tools:` list in `agents/{name}/AGENT.md`
8. Write at least one unit test using `MockRoutingContext` — test both success and error cases
9. Run: `cargo nextest run -p tools`

**Do NOT:**
- Hard-code user IDs, chat IDs, or channel names in tool logic
- Access `StoragePool` directly — receive `Repos` via `RoutingContext`
- Call `tokio::spawn` inside a tool — use the provided async context

---

## 4. Adding a New Feature Package

A feature package bundles: tools + database migrations + config defaults + health check.

**Checklist:**
1. Create `crates/feature-{name}/`
2. Add to `Cargo.toml` workspace `members` + `[workspace.dependencies]`
3. Add `feature-{name}.workspace = true` to any crate that needs it (`app-core`, `klyntbot`)
4. Implement `FeaturePackage` trait (4 required methods: `tools`, `migrations`, `config_default`, `health_check`)
5. Define `FeatureMigration` with:
   - `feature_name`: unique snake_case string (e.g., `"feature_tasks"`)
   - `version`: monotonically increasing integer starting at 1
   - SQL: idempotent (`CREATE TABLE IF NOT EXISTS`, `INSERT OR IGNORE`)
   - Add `-- Migration N: [human-readable description]` comment to each SQL block
6. Register in `crates/app-core/src/init/features.rs`
7. Add handler methods to `AppCore` in `app-core/src/handlers/`
8. Add IPC command types to `desktop-shared/src/commands/{domain}.rs`
9. Add thin Tauri adapter in `desktop/src/commands/{domain}.rs`
10. Run: `cargo nextest run --workspace`

**Migration Rules:**
- 🔴 MUST: Never alter an existing migration that has already been deployed. Add a new version instead.
- 🔴 MUST: Use `INSERT OR IGNORE` for seed data inserts to maintain idempotency.
- ✅ OK (pre-1.0 only): Drop and recreate tables for schema changes — no backwards-compat migrations needed until first public release.

---

## 5. Adding a New Channel (Platform Integration)

**Location:** `crates/channels/src/adapters/{name}/`

**Checklist:**
1. Create `adapters/{name}/mod.rs` with the channel struct
2. Implement the `Channel` trait (all 5 methods: `start`, `stop`, `send`, `send_typing`, `send_structured`)
3. Add config struct in `crates/config/src/schema/channels.rs`
4. Register in `ChannelManager::from_config()` in `channels/src/manager.rs`
5. Add `ChannelName::{Name}` variant to `crates/common/src/types/core.rs`
6. If stub exists in `channels/src/stubs/`, remove or replace it
7. Add access control via `check_allowlist()` helper — empty list means open access
8. For structured interactions (buttons/selects), implement `InteractionChannel` sub-trait
9. Run: `cargo nextest run -p channels`

**Do NOT:**
- Store message state in-memory — all persistence goes through `Repos`
- Use `unwrap()` on message sends — channels must handle failures gracefully and log with `tracing::error!`

---

## 6. Adding a New LLM Provider

**Location:** `crates/providers/src/adapters/{name}/`

**Checklist:**
1. Implement `LlmProvider` trait: `chat()`, `stream_chat()`, `models()`, `name()`
2. Add detection logic in `crates/providers/src/registry/detection.rs` (API key prefix or explicit config flag)
3. Add config struct in `crates/config/src/schema/providers.rs`
4. Add pricing table entry in `crates/agent/src/infrastructure/output/cost_tracker/pricing.rs` with `updated_at` comment
5. If using OpenAI-compatible API, prefer extending `OpenAiCompatProvider` with a custom `api_base` — don't duplicate the HTTP client code
6. Add the provider to the provider registry test matrix in `providers/tests/`
7. Run: `cargo nextest run -p providers`

---

## 7. Handler Trait Pattern (Dependency Inversion)

This pattern breaks circular dependencies between lower-layer feature crates and the `agent` crate.

**Step 1:** Define the trait in the LOWER-layer crate that needs the behavior:
```rust
// In: crates/feature-tasks/src/lib.rs (or tools-core)
#[async_trait]
pub trait DecompositionHandler: Send + Sync {
    async fn decompose(&self, task_id: Uuid, context: &str) -> Result<Vec<SubTask>>;
}
```

**Step 2:** Implement in `agent`:
```rust
// In: crates/agent/src/handlers/decomposition.rs
pub struct AgentDecompositionHandler { /* repos, provider, etc. */ }

#[async_trait]
impl DecompositionHandler for AgentDecompositionHandler {
    async fn decompose(&self, task_id: Uuid, context: &str) -> Result<Vec<SubTask>> { ... }
}
```

**Step 3:** Inject via `Arc<dyn Trait>` in constructors:
```rust
// In: crates/feature-tasks/src/application/tool/decompose.rs
pub struct DecomposeTool {
    pub handler: Arc<dyn DecompositionHandler>,
}
```

**Step 4:** Wire in `app-core/src/init/agents.rs`:
```rust
let decomposition_handler = Arc::new(AgentDecompositionHandler::new(&repos, &provider));
let decompose_tool = DecomposeTool { handler: decomposition_handler };
```

- 🔴 MUST: Never import `agent` from any crate at layer 4 or below.
- 🔴 MUST: Handler traits defined in feature crates must be in `pub` position in `lib.rs`.

---

## 8. Storage & Testing Rules

**Pool usage:**
- 🔴 MUST: ALL tests use `StoragePool::connect_in_memory()` — never a file path or `~/.klyntbot/data.db`
- `StoragePool::from_existing()` — skips migrations; only use for already-initialized pools in the binary
- `StoragePool::connect()` — runs all migrations; use for fresh pools in the binary

**Repository pattern:**
- All data access through `Repos` struct — never construct a repo directly outside of `Repos::from_pool()`
- Repos are `Clone` (via `StoragePool` Clone) — pass `repos.clone()` to async tasks freely
- SQL queries: use `sqlx::query!()` (compile-time checked) for non-dynamic queries; `sqlx::query()` only for dynamic WHERE clauses
- 🔴 MUST: Never call `.unwrap()` on SQL results — propagate errors with `?`

**Test placement:**
- Unit tests: `#[cfg(test)] mod tests` inline at the bottom of the file
- Integration tests: `crates/{name}/tests/` directory
- All test data creation must use `StoragePool::connect_in_memory()` + run migrations

---

## 9. Async and Concurrency Rules

- 🔴 MUST: All public async functions must be `Send` — no `Rc`, `RefCell`, or `*mut T` across await points
- 🔴 MUST: No `tokio::runtime::Runtime::block_on()` inside async context — causes deadlock
- 🟡 SHOULD: Use `tokio::spawn` for independent background tasks, not for sequential operations
- 🟡 SHOULD: Prefer `tokio::sync::Mutex` over `std::sync::Mutex` in async code
- Use `Arc<RwLock<T>>` for shared state that is read-mostly
- `StoragePool` is `Clone + Send + Sync` — no additional locking needed for DB access

---

## 10. Error Handling

```rust
// Use common::Result<T> everywhere (alias for Result<T, KlyntbotError>)
use common::Result;

// Convert domain errors with From impl or ?
fn get_task(id: Uuid, repos: &Repos) -> Result<Task> {
    repos.task_repo.get(id).map_err(KlyntbotError::Storage)
}

// Never swallow errors in tool implementations
impl ToolExecute<Params> for MyTool {
    async fn execute(&self, p: Params, ctx: &RoutingContext) -> Result<String> {
        // propagate with ?
        let result = self.repos.do_something(p.id)?;
        Ok(format!("Done: {result}"))
    }
}
```

- 🔴 MUST: No `unwrap()` or `expect()` in non-test production code
- 🔴 MUST: No `panic!()` except in `stubs/` (unimplemented channels)
- 🟡 SHOULD: Use `tracing::warn!` for recoverable errors, `tracing::error!` for failures that may affect user experience
- Use `anyhow::Context` for adding context to errors that span multiple layers

---

## 11. Timestamps

- 🔴 MUST: Always use `chrono::Utc::now()` — all timestamps are UTC, always
- Never use `chrono::Local::now()` in the backend
- Frontend: parse via `new Date(iso)` and format via `toLocaleTimeString()` — never `.slice()` ISO strings
- Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`
- Store as `TEXT` in SQLite (ISO 8601 / RFC 3339 format via `chrono` serde)

---

## 12. Configuration Rules

- All config structs: `#[serde(rename_all = "camelCase")]`
- All secret fields: `Secret<String>` — access via `.expose()`, NEVER log or serialize the exposed value
- Env override format: `KLYNTBOT_{SECTION}__{FIELD}=value` (double underscore for nesting)
- Config changes require app restart — no hot-reload (except `development.hot_reload_skills`, if implemented)
- Add new config section in `config/src/schema/{domain}.rs`, reference from root `Config` struct
- Add validation logic in `config/src/loader/` `Config::validate()` (see A-017 in BACKLOG.md)

---

## 13. Commit and PR Conventions

- 🔴 MUST: Zero `cargo clippy` warnings before merging
- 🔴 MUST: `cargo fmt --all --check` passes
- 🔴 MUST: `cargo nextest run --workspace` passes
- Commit format: `type(scope): description` where type is one of:
  - `feat` — new behavior/feature
  - `refactor` — structural change, zero behavior change
  - `fix` — bug fix
  - `test` — tests only
  - `docs` — documentation only
  - `chore` — tooling, CI, dependencies
- Scope examples: `agent`, `channels`, `storage`, `feature-tasks`, `desktop`

---

## 14. Desktop UI (TypeScript / Tailwind v4)

- All theming in `desktop-ui/src/styles/theme.css` via CSS variables + `@theme inline`
- No `tailwind.config.js` — use CSS variable token utilities (`bg-surface-base`, `text-muted`, `border-border`)
- Never hardcode hex/rgba colors — always use token utilities
- Glassmorphism class: `glass-panel` (uses `backdrop-blur-[80px] backdrop-saturate-150`)
- Never write raw `backdrop-filter: blur() saturate()` — use Tailwind's `@apply` directives
- Never use `overflow-x-auto`/`overflow: hidden` on containers with absolute dropdown children — use portals
- Package manager: always `bun`, never `npm` or `yarn`
- Linting: `bun run lint:fix` (Biome 2.0 — lint + format + imports in one pass)
- Testing: `bun run test` (Vitest, run once — not watch mode in CI)

---

## 15. LLM Prompt Writing Guidelines

When writing prompts embedded in Rust string literals:

- Keep system prompts under 2,000 tokens (measure with `token_counter.rs`)
- Use XML tags for structured sections: `<context>`, `<task>`, `<examples>`, `<format>`
- For tool descriptions: be specific about what the tool does AND what it does not do
- For extraction prompts: always include a "return empty array if none found" instruction
- Never include PII or secrets in prompts — use placeholder references
- Test prompts with at least 3 examples: nominal, edge case, failure case

---

## Quick Reference: Where Does X Go?

| What | Where |
|---|---|
| New error variant | `crates/common/src/errors/` |
| New core type (ChatId, etc.) | `crates/common/src/types/core.rs` |
| New config section | `crates/config/src/schema/{domain}.rs` |
| New domain event | `crates/bus/src/domain_events.rs` |
| New tool | `crates/tools/src/{domain}/` |
| New feature (tools+migrations+config) | New `crates/feature-{name}/` |
| New channel | `crates/channels/src/adapters/{name}/` |
| New LLM provider | `crates/providers/src/adapters/{name}/` |
| New handler trait | Lower-layer crate that needs it |
| Handler trait implementation | `crates/agent/src/handlers/{domain}/` |
| New app-core handler | `crates/app-core/src/handlers/{domain}.rs` |
| New IPC command | `crates/desktop-shared/src/commands/{domain}.rs` |
| New Tauri command | `crates/desktop/src/commands/{domain}.rs` |
| Database repo | `crates/storage/src/repos/{domain}/` |
| Cognitive memory logic | `crates/cognitive/src/application/` |
| Agent profile | `agents/{name}/AGENT.md` |
| Agent skill | `agents/{name}/skills/{skill}.md` |
