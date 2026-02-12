# Migration Guide: Monolith to Workspace

This guide explains the migration from klyntbot's original monolithic structure to the multi-crate workspace architecture.

## Table of Contents

1. [Overview](#overview)
2. [Import Path Changes](#import-path-changes)
3. [Structural Changes](#structural-changes)
4. [Breaking Changes](#breaking-changes)
5. [New Patterns](#new-patterns)
6. [Migration Checklist](#migration-checklist)

---

## Overview

### What Changed

| Aspect | Before | After |
|--------|--------|-------|
| **Structure** | Single crate with modules | 11 crates in workspace |
| **Dependencies** | Internal `mod` statements | Explicit `Cargo.toml` dependencies |
| **Imports** | `use crate::module::*` | `use klyntbot_module::*` |
| **Compilation** | Serial (one compilation unit) | Parallel (11 crates) |
| **Scope** | Everything visible to everything | Strict layer boundaries |

### What Stayed the Same

- Public API surface (via re-export facade)
- Configuration format
- Tool/channel/provider interfaces
- Test coverage and behavior
- Binary output and CLI commands

---

## Import Path Changes

### Before (Monolith)

```rust
// In src/agent/agent_loop.rs
use crate::bus::{InboundMessage, MessageBus};
use crate::config::Config;
use crate::error::Result;
use crate::providers::{LlmProvider, create_provider};
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
```

### After (Workspace)

```rust
// In crates/klyntbot-agent/src/agent_loop.rs
use klyntbot_bus::{InboundMessage, MessageBus};
use klyntbot_config::Config;
use klyntbot_core::Result;
use klyntbot_providers::{LlmProvider, create_provider};
use klyntbot_session::SessionManager;
use klyntbot_tools::ToolRegistry;
```

### Migration Pattern

1. Change `use crate::module` → `use klyntbot_module`
2. Add crate dependency in `Cargo.toml`:
   ```toml
   [dependencies]
   klyntbot-bus.workspace = true
   klyntbot-config.workspace = true
   klyntbot-core.workspace = true
   ```

---

## Structural Changes

### File Relocations

| Before | After |
|--------|-------|
| `src/error.rs` | `crates/klyntbot-core/src/error.rs` |
| `src/types.rs` | `crates/klyntbot-core/src/types.rs` |
| `src/utils/` | `crates/klyntbot-core/src/utils/` |
| `src/config/` | `crates/klyntbot-config/src/` |
| `src/bus/` | `crates/klyntbot-bus/src/` |
| `src/providers/` | `crates/klyntbot-providers/src/` |
| `src/session/` | `crates/klyntbot-session/src/` |
| `src/cron/` | `crates/klyntbot-cron/src/` |
| `src/tools/` | `crates/klyntbot-tools/src/` |
| `src/channels/` | `crates/klyntbot-channels/src/` |
| `src/heartbeat/` | `crates/klyntbot-heartbeat/src/` |
| `src/agent/` | `crates/klyntbot-agent/src/` |
| `src/cli/` | `crates/klyntbot-cli/src/` |
| `src/lib.rs` | `src/lib.rs` (now a facade) |
| `src/main.rs` | `src/main.rs` (unchanged) |

### Module Hierarchy Changes

**Before:**
```
klyntbot
├── error (module)
├── types (module)
├── config (module)
└── agent (module)
    ├── agent_loop
    ├── context
    └── memory
```

**After:**
```
klyntbot-core (crate)
├── error
├── types
└── utils

klyntbot-config (crate)
├── schema
└── loader

klyntbot-agent (crate)
├── agent_loop
├── context
├── memory
├── skills
└── subagent
```

---

## Breaking Changes

### 1. `ProviderError::Http` Type Change

**Before:**
```rust
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
```

**After:**
```rust
// In klyntbot-core
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),  // Changed to String
}

// In klyntbot-providers
impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        ProviderError::Http(e.to_string())
    }
}
```

**Reason**: Keeps `klyntbot-core` free of heavy dependencies like `reqwest`.

**Migration**: If you're matching on `ProviderError::Http`, update pattern:
```rust
// Before
match err {
    ProviderError::Http(reqwest_err) => reqwest_err.status(),
}

// After
match err {
    ProviderError::Http(msg) => eprintln!("HTTP error: {}", msg),
}
```

### 2. Handler Trait Abstraction

**Before:**
```rust
// In src/tools/spawn.rs
use crate::agent::subagent::SubagentManager;

pub struct SpawnTool {
    subagent_mgr: Arc<SubagentManager>,
}
```

**After:**
```rust
// In crates/klyntbot-tools/src/spawn.rs
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(&self, task: String, ...) -> String;
}

pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}

// In crates/klyntbot-agent/src/subagent.rs
impl SpawnHandler for SubagentManager {
    async fn spawn(&self, task: String, ...) -> String {
        // Implementation
    }
}
```

**Reason**: Breaks circular dependency between `klyntbot-tools` and `klyntbot-agent`.

**Migration**: When constructing `SpawnTool`, pass handler as trait object:
```rust
// Before
let spawn_tool = SpawnTool::new(Arc::clone(&subagent_mgr));

// After
let spawn_tool = SpawnTool::new(Some(Arc::clone(&subagent_mgr) as Arc<dyn SpawnHandler>));
```

### 3. Skill File Path Resolution

**Before:**
```rust
// In src/agent/skills.rs
const CRON_SKILL: &str = include_str!("../../skills/cron/SKILL.md");
```

**After:**
```rust
// In crates/klyntbot-agent/src/skills.rs
const CRON_SKILL: &str = include_str!("../../../skills/cron/SKILL.md");
```

**Reason**: Crate is now nested under `crates/`, so relative path changes.

**Migration**: Add one more `../` to skill paths in `klyntbot-agent`.

---

## New Patterns

### 1. Workspace Dependencies

All workspace crates use `workspace = true` to inherit versions:

```toml
# crates/klyntbot-agent/Cargo.toml
[dependencies]
klyntbot-core.workspace = true
klyntbot-bus.workspace = true
klyntbot-config.workspace = true
klyntbot-providers.workspace = true
klyntbot-session.workspace = true
klyntbot-tools.workspace = true
klyntbot-cron.workspace = true

tokio.workspace = true
async-trait.workspace = true
serde.workspace = true
```

**Benefit**: Version changes happen once in root `Cargo.toml`.

### 2. Re-export Facade

The root `klyntbot` crate re-exports all public types:

```rust
// src/lib.rs
pub use klyntbot_core::{KlyntbotError, Result, ChannelName, ChatId};
pub use klyntbot_config::Config;
pub use klyntbot_agent::AgentLoop;
// ... all public types
```

**Usage**: External code imports from `klyntbot`:
```rust
use klyntbot::{AgentLoop, Config, Result};
```

**Benefit**: Backward compatibility — existing imports don't break.

### 3. Feature Flags

Optional functionality uses feature flags:

```toml
# Root Cargo.toml
[features]
default = ["email"]
email = ["klyntbot-channels/email"]

# crates/klyntbot-channels/Cargo.toml
[features]
email = ["dep:async-imap", "dep:lettre", ...]
```

**Usage**: Build without email:
```bash
cargo build --no-default-features
```

**Benefit**: Reduces compile time and binary size for minimal builds.

---

## Migration Checklist

### For Contributors

If you have local changes in the monolith:

1. **Identify which crate your changes belong to**
   - Error types → `klyntbot-core`
   - Config schema → `klyntbot-config`
   - Tools → `klyntbot-tools`
   - Channels → `klyntbot-channels`
   - Agent logic → `klyntbot-agent`
   - CLI commands → `klyntbot-cli`

2. **Update import paths**
   - Change `use crate::module` → `use klyntbot_module`
   - Add crate dependencies to `Cargo.toml`

3. **Update relative paths**
   - Skill `include_str!` paths: add `../`
   - Test fixture paths: update to workspace-relative

4. **Run tests**
   ```bash
   cargo test -p <crate-name>  # Test your crate
   cargo test --workspace       # Test everything
   ```

5. **Check clippy**
   ```bash
   cargo clippy -p <crate-name>
   cargo clippy --workspace
   ```

### For Workspace Maintainers

When refactoring the workspace:

1. **Always maintain the dependency DAG**
   - Lower layers cannot depend on higher layers
   - Use handler traits for dependency inversion

2. **Test after each crate extraction**
   ```bash
   cargo build -p <new-crate>
   cargo test -p <new-crate>
   ```

3. **Update `src/lib.rs` re-exports**
   - Any new public type should be re-exported
   - Maintain backward compatibility

4. **Document breaking changes**
   - Add to this MIGRATION.md
   - Update CHANGELOG

5. **Verify feature flags**
   ```bash
   cargo build --no-default-features
   cargo build --all-features
   cargo test --all-features
   ```

---

## Common Migration Issues

### Issue: "Cannot find crate `klyntbot_foo`"

**Cause**: Missing dependency in `Cargo.toml`.

**Fix**: Add dependency:
```toml
[dependencies]
klyntbot-foo.workspace = true
```

### Issue: "Circular dependency detected"

**Cause**: Higher-layer crate trying to depend on lower-layer implementation.

**Fix**: Use a handler trait:
1. Define trait in lower-layer crate
2. Implement trait in higher-layer crate
3. Pass as `Arc<dyn Trait>` at construction

### Issue: "include_str!() file not found"

**Cause**: Relative path changed due to crate nesting.

**Fix**: Adjust path (usually add `../`):
```rust
// Before (in src/agent/skills.rs)
include_str!("../../skills/cron/SKILL.md")

// After (in crates/klyntbot-agent/src/skills.rs)
include_str!("../../../skills/cron/SKILL.md")
```

### Issue: "Type not visible from this crate"

**Cause**: Type is private to its crate.

**Fix**: Either:
1. Make type public in its crate: `pub struct Foo { ... }`
2. Re-export via `lib.rs`: `pub use internal::Foo;`
3. Use trait abstraction if cross-layer dependency

---

## Testing the Migration

### Verify Functional Equivalence

1. **Build succeeds:**
   ```bash
   cargo build --workspace --all-features
   ```

2. **All tests pass:**
   ```bash
   cargo test --workspace
   ```

3. **Clippy has no warnings:**
   ```bash
   cargo clippy --workspace --all-targets --all-features
   ```

4. **Binary behaves identically:**
   ```bash
   cargo build --release
   ./target/release/klyntbot status
   ./target/release/klyntbot chat "Hello"
   ```

5. **Feature flags work:**
   ```bash
   cargo build --no-default-features
   cargo build --features email
   ```

### Performance Comparison

Compare build times:

```bash
# Monolith (before)
cargo clean && time cargo build --release

# Workspace (after)
cargo clean && time cargo build --release --workspace
```

**Expected**: Similar or faster (due to parallelism).

---

## Rollback Plan

If the workspace migration causes critical issues:

1. **Revert to last monolith commit:**
   ```bash
   git checkout <monolith-commit-hash>
   ```

2. **Cherry-pick feature commits:**
   ```bash
   git cherry-pick <feature-commit>
   ```

3. **Document issues** in GitHub issues with:
   - Error messages
   - Steps to reproduce
   - Expected vs. actual behavior

---

## Resources

- [Cargo Workspaces Documentation](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Architecture Documentation](./ARCHITECTURE.md)
- [Workspace Structure Design](./WORKSPACE_ARCHITECTURE.md)
- [Contributing Guide](../CONTRIBUTING.md)

---

## Summary

The workspace migration achieves:
- **Better modularity** via crate boundaries
- **Faster incremental builds** via crate isolation
- **Parallel compilation** via dependency layers
- **Clear dependencies** enforced by Cargo

The primary changes are:
- Import paths (`crate::` → `klyntbot_foo::`)
- Handler traits for dependency inversion
- Workspace-level `Cargo.toml` for shared deps

Backward compatibility is maintained via the re-export facade in `src/lib.rs`.
