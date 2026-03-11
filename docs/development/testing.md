# Testing Guide

## Test Commands

### Run all tests (parallel)

```bash
cargo nextest run --workspace
```

`cargo-nextest` runs each test in its own process, enabling true parallelism. All tests use ephemeral SQLite via `StoragePool::connect_in_memory()`, so no external database is needed.

### Run tests for a single crate

```bash
cargo nextest run -p agent
cargo nextest run -p config
cargo nextest run -p scheduling
```

### Run tests matching a pattern

```bash
cargo nextest run -E 'test(session_persistence)'
cargo nextest run -E 'test(budget)'
```

The `-E` flag accepts nextest filter expressions. `test(pattern)` matches test function names containing the given string.

### Doctests

```bash
cargo test --workspace --doc
```

`cargo-nextest` does not support doctests, so these must be run separately with the standard `cargo test` command.

### Linting (clippy)

```bash
cargo clippy --workspace --all-targets --all-features
```

The project enforces a **zero clippy warnings** policy. All warnings are treated as errors in CI. The `desktop` crate has some pre-existing exceptions, but new code must not introduce warnings.

### Format check

```bash
cargo fmt --all --check
```

Verifies that all Rust source files conform to `rustfmt` formatting. Run `cargo fmt --all` to auto-fix.

## Test Patterns

### Ephemeral SQLite with `connect_in_memory()`

All tests that need a database use `StoragePool::connect_in_memory()`, which creates an in-memory SQLite pool with all core migrations applied automatically. This ensures tests are fast, isolated, and require no external setup.

```rust
#[tokio::test]
async fn test_something_with_storage() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SomeRepo::new(pool.inner().clone());
    // ... test logic
}
```

**Important:** Never use `StoragePool::from_existing()` in tests. That constructor skips migrations and is only for wrapping already-migrated pools.

For crates with feature-owned migrations (like `cognitive`), there is a dedicated test pool helper:

```rust
// crates/cognitive/src/repos/mod.rs
#[cfg(test)]
pub(crate) async fn cognitive_test_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await.unwrap();
    sqlx::migrate!("../storage/migrations").run(&pool).await.unwrap();
    let migrations = cognitive_migrations();
    storage::StoragePool::run_feature_migrations(&pool, &migrations).await.unwrap();
    pool
}
```

This runs both core and feature migrations, giving the test a fully-provisioned database.

### Async test setup

Tests that need async setup use the `#[tokio::test]` attribute. Some crates use a `setup()` helper function:

```rust
async fn setup() -> SqlitePool {
    crate::repos::cognitive_test_pool().await
}

#[tokio::test]
async fn test_upsert_and_get() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool);
    // ... assertions
}
```

### Test-only constructors

Several types provide `#[cfg(test)]` constructors that bypass external dependencies. For example, `CronService::new_for_test()` creates an in-memory-only instance with no SQL persistence:

```rust
#[cfg(test)]
fn new_for_test() -> Self {
    Self {
        store: Arc::new(RwLock::new(CronStore::default())),
        repo: None,   // No SQL backend
        // ...
    }
}
```

This pattern avoids needing a database connection for unit tests that only exercise in-memory logic.

### Mock patterns

The codebase uses manual trait-based mocks rather than a mocking framework. Implement the trait for a test struct:

```rust
struct MockRetriever {
    entries: Vec<(String, f64)>,
}

#[async_trait]
impl MemoryRetriever for MockRetriever {
    async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.entries.iter().take(limit).map(|(content, score)| {
            MemoryEntry { id: "test".into(), content: content.clone(), score: *score }
        }).collect()
    }
}
```

Fake tool implementations follow the same pattern, implementing the `Tool` trait with controlled return values:

```rust
struct FakeSearchTool;

#[async_trait]
impl Tool for FakeSearchTool {
    fn name(&self) -> &str { "search" }
    fn description(&self) -> &str { "Search for files" }
    fn parameters(&self) -> Value { json!({"type": "object"}) }
    async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
        Ok("ok".into())
    }
}
```

### Compile-time tests

Some tests verify type-level properties without executing any logic. These use the "assert trait bound" pattern:

```rust
#[test]
fn test_session_manager_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<SessionManager>();
}

#[test]
fn test_session_manager_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SessionManager>();
}
```

These tests break the build if someone accidentally removes a `Clone`, `Send`, or `Sync` implementation from a critical type.

### Synchronous vs. async tests

Pure data structure tests use `#[test]` (synchronous). Tests that involve database access, async I/O, or tokio timers use `#[tokio::test]`:

- `#[test]` -- Config serialization, computation functions, type properties
- `#[tokio::test]` -- Repository CRUD, session management, service lifecycle

## Integration Tests

Integration tests live in `tests/` at the workspace root and use the `klyntbot` facade crate for imports. This gives them access to all public types via re-exports:

```rust
use klyntbot::AgentLoop;
use klyntbot::Config;
```

## Compile-Time Parity Tests (Dev Server)

The `desktop` crate contains a compile-time parity test in `crates/desktop/src/dev_server.rs` that ensures every Tauri IPC command has a corresponding handler in the dev HTTP server.

Two tests enforce this:

1. **`dev_server_covers_all_tauri_commands`** -- Parses Tauri command names from `main.rs` source (via `include_str!`) and compares them against `DEV_COMMANDS` arrays in each command module. Fails if a new Tauri command is missing from the dev server dispatch.

2. **`dev_server_has_no_orphan_commands`** -- Checks the reverse: no dev server command should exist without a corresponding Tauri command registration. Catches stale entries after command removal.

Desktop-only commands (accessibility permissions, window management, quit) are explicitly excluded via a `TAURI_ONLY` allow list.

This mechanism prevents drift between the Tauri desktop app and the browser-based dev mode, which share the same `AppCore` business logic layer.

## Zero Clippy Warnings Policy

All code must pass `cargo clippy --workspace --all-targets --all-features` with zero warnings. This is enforced in CI.

When adding new code:
- Fix all clippy suggestions before committing
- The `desktop` crate has some pre-existing `#[allow(...)]` exceptions; do not add new ones without good reason
- If a clippy lint is genuinely wrong for your case, use a targeted `#[allow(clippy::specific_lint)]` with a comment explaining why

## Running Specific Tests

| Goal | Command |
|------|---------|
| All workspace tests | `cargo nextest run --workspace` |
| Single crate | `cargo nextest run -p agent` |
| Pattern match | `cargo nextest run -E 'test(session_persistence)'` |
| Doctests only | `cargo test --workspace --doc` |
| Lint check | `cargo clippy --workspace --all-targets --all-features` |
| Format check | `cargo fmt --all --check` |
| Single test file | `cargo nextest run -p config -E 'test(config_round_trip)'` |
