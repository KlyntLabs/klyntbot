# Config Hot-Reload + Progressive Skill Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable config.json changes to take effect without restart, and reduce token usage by loading skill content progressively (summaries first, full body on demand).

**Architecture:** Two independent features sharing a plan. Feature 1 adds a `ConfigWatcher` file-watching service and a shared `HotConfig` struct that propagates config changes to the live agent pipeline. Feature 2 adds a `summary` field to skills, makes `SkillContextSource` inject summaries for activated skills, makes always-loaded references conditional on message relevance, and adds a `SkillReferenceTool` for on-demand loading.

**Tech Stack:** Rust, tokio, notify (already a workspace dep), serde, async-trait

---

## File Structure

### Feature 1: Config Hot-Reload

| File | Action | Responsibility |
|---|---|---|
| `crates/config/src/schema/hot.rs` | Create | `HotConfig` struct — hot-reloadable subset of Config |
| `crates/config/src/schema/mod.rs` | Modify | Re-export `HotConfig` |
| `crates/config/src/lib.rs` | Modify | Re-export `HotConfig` |
| `crates/config/src/loader.rs` | Modify | Add `reload_if_changed()` function for hot-reload diff |
| `crates/agent/src/agent_loop/mod.rs` | Modify | Add `hot_config` field, read in pipeline |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `hot_config` into AgentLoop and AgentRuntime |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Add `hot_config` field, read per-message for model/temperature/iterations |
| `crates/app-core/src/state.rs` | Modify | Change `config` to `Arc<RwLock<Config>>`, add `hot_config: Arc<RwLock<HotConfig>>` |
| `crates/app-core/src/init/mod.rs` | Modify | Create shared `Arc<RwLock<Config>>` before AppCore construction, create HotConfig, start watcher, wire to agent |
| `crates/app-core/src/handlers/settings/config.rs` | Modify | Push to `hot_config` after `config_update_section` |
| `crates/app-core/src/infrastructure/config_watcher.rs` | Create | `ConfigWatcherService` — watches config.json, reloads into AppCore |
| `crates/app-core/src/infrastructure/mod.rs` | Modify | Declare `config_watcher` module |

### Feature 2: Progressive Skill Loading

| File | Action | Responsibility |
|---|---|---|
| `crates/skill-system/src/types.rs` | Modify | Add `summary` field to `KlyntbotMeta` and `SkillPackage` |
| `crates/skill-system/src/parser.rs` | Modify | Parse `summary` from frontmatter, fallback to first sentence of body |
| `crates/skill-system/src/context.rs` | Modify | Inject summary for activated skills, conditional always-skills |
| `crates/tools/src/domain/skill_reference.rs` | Create | `SkillReferenceTool` — serves skill body + reference files on demand |
| `crates/tools/src/domain/mod.rs` | Modify | Declare `skill_reference` module |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Register `SkillReferenceTool` in tool registry |
| `skills/general/SKILL.md` | Modify | Add `summary` field to frontmatter |
| `skills/task-management/SKILL.md` | Modify | Add `summary` field to frontmatter |
| `skills/finance-management/SKILL.md` | Modify | Add `summary` field to frontmatter |
| `skills/automation/SKILL.md` | Modify | Add `summary` field to frontmatter |
| `skills/communication/SKILL.md` | Modify | Add `summary` field to frontmatter |

---

## Feature 1: Config Hot-Reload

### Task 1: Define HotConfig type

**Files:**
- Create: `crates/config/src/schema/hot.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: Write the test for HotConfig**

In `crates/config/src/schema/hot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Config;

    #[test]
    fn test_hot_config_from_config() {
        let mut config = Config::default();
        config.agents.defaults.model = "test-model".to_string();
        config.agents.defaults.temperature = 0.5;
        config.agents.defaults.max_tokens = 4096;
        config.agents.defaults.max_tool_iterations = 10;
        config.agents.defaults.pipeline_timeout_secs = 120;
        config.agents.monthly_budget_usd = Some(50.0);

        let hot = HotConfig::from(&config);
        assert_eq!(hot.model, "test-model");
        assert_eq!(hot.temperature, 0.5);
        assert_eq!(hot.max_tokens, 4096);
        assert_eq!(hot.max_tool_iterations, 10);
        assert_eq!(hot.pipeline_timeout_secs, 120);
        assert_eq!(hot.monthly_budget_usd, Some(50.0));
    }

    #[test]
    fn test_hot_config_diff_detects_model_change() {
        let a = HotConfig {
            model: "model-a".into(),
            ..HotConfig::from(&Config::default())
        };
        let b = HotConfig {
            model: "model-b".into(),
            ..HotConfig::from(&Config::default())
        };
        assert!(a.diff(&b).model_changed);
        assert!(!a.diff(&b).temperature_changed);
    }

    #[test]
    fn test_hot_config_diff_no_changes() {
        let a = HotConfig::from(&Config::default());
        let b = HotConfig::from(&Config::default());
        let diff = a.diff(&b);
        assert!(!diff.has_changes());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p config --no-capture -E 'test(hot_config)'`
Expected: FAIL — `HotConfig` type does not exist yet.

- [ ] **Step 3: Implement HotConfig**

In `crates/config/src/schema/hot.rs`:

```rust
//! Hot-reloadable configuration subset.
//!
//! Fields here take effect immediately without restart. The full `Config`
//! still requires restart for structural changes (channels, provider init,
//! feature enable/disable flags).

use super::Config;

/// Hot-reloadable subset of Config.
///
/// Extracted from `Config` via `From<&Config>`. Shared as
/// `Arc<RwLock<HotConfig>>` between AppCore and the agent pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct HotConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_tool_iterations: u32,
    pub pipeline_timeout_secs: u64,
    pub monthly_budget_usd: Option<f64>,
}

/// Describes which fields changed between two HotConfig snapshots.
#[derive(Debug, Default)]
pub struct HotConfigDiff {
    pub model_changed: bool,
    pub temperature_changed: bool,
    pub max_tokens_changed: bool,
    pub max_tool_iterations_changed: bool,
    pub pipeline_timeout_changed: bool,
    pub budget_changed: bool,
}

impl HotConfigDiff {
    pub fn has_changes(&self) -> bool {
        self.model_changed
            || self.temperature_changed
            || self.max_tokens_changed
            || self.max_tool_iterations_changed
            || self.pipeline_timeout_changed
            || self.budget_changed
    }
}

impl From<&Config> for HotConfig {
    fn from(config: &Config) -> Self {
        Self {
            model: config.agents.defaults.model.clone(),
            temperature: config.agents.defaults.temperature,
            max_tokens: config.agents.defaults.max_tokens,
            max_tool_iterations: config.agents.defaults.max_tool_iterations,
            pipeline_timeout_secs: config.agents.defaults.pipeline_timeout_secs,
            monthly_budget_usd: config.agents.monthly_budget_usd,
        }
    }
}

impl HotConfig {
    /// Compare two snapshots and return which fields changed.
    pub fn diff(&self, other: &HotConfig) -> HotConfigDiff {
        HotConfigDiff {
            model_changed: self.model != other.model,
            temperature_changed: (self.temperature - other.temperature).abs() > f32::EPSILON,
            max_tokens_changed: self.max_tokens != other.max_tokens,
            max_tool_iterations_changed: self.max_tool_iterations != other.max_tool_iterations,
            pipeline_timeout_changed: self.pipeline_timeout_secs != other.pipeline_timeout_secs,
            budget_changed: self.monthly_budget_usd != other.monthly_budget_usd,
        }
    }
}

// tests at bottom of file (see Step 1)
```

- [ ] **Step 4: Add module declaration and re-exports**

In `crates/config/src/schema/mod.rs`, add:
```rust
pub mod hot;
```

In `crates/config/src/lib.rs`, add to the `pub use schema::` block:
```rust
pub use schema::hot::{HotConfig, HotConfigDiff};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p config --no-capture -E 'test(hot_config)'`
Expected: All 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/config/src/schema/hot.rs crates/config/src/schema/mod.rs crates/config/src/lib.rs
git commit -m "feat(config): add HotConfig type for hot-reloadable config subset"
```

---

### Task 2: Add config file watcher to the config crate

**Files:**
- Modify: `crates/config/Cargo.toml`
- Modify: `crates/config/src/loader.rs`

- [ ] **Step 1: Write the test for config reload on file change**

In `crates/config/src/loader.rs`, add to the `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn test_reload_detects_changes() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    // Write initial config
    let mut config = Config::default();
    config.agents.defaults.model = "model-v1".to_string();
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&config_path, &json).unwrap();

    // Load and verify
    let content = std::fs::read_to_string(&config_path).unwrap();
    let loaded: Config = serde_json::from_str(&content).unwrap();
    assert_eq!(loaded.agents.defaults.model, "model-v1");

    // Overwrite with different model
    config.agents.defaults.model = "model-v2".to_string();
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&config_path, &json).unwrap();

    // Re-load and verify change detected
    let content = std::fs::read_to_string(&config_path).unwrap();
    let reloaded: Config = serde_json::from_str(&content).unwrap();
    assert_eq!(reloaded.agents.defaults.model, "model-v2");

    let hot_old = config::HotConfig::from(&loaded);
    let hot_new = config::HotConfig::from(&reloaded);
    let diff = hot_old.diff(&hot_new);
    assert!(diff.model_changed);
}
```

- [ ] **Step 2: Run test to verify it passes** (this test just validates the reload-and-diff pattern, no watcher yet)

Run: `cargo nextest run -p config --no-capture -E 'test(reload_detects)'`
Expected: PASS.

- [ ] **Step 3: Add `reload_if_changed` function to loader.rs**

In `crates/config/src/loader.rs`, add a public function for reloading config and computing the hot diff:

```rust
use super::schema::hot::{HotConfig, HotConfigDiff};

/// Reload config from disk and return (new_config, diff) compared to the previous HotConfig.
///
/// Returns `None` if the file hasn't changed or can't be parsed (logs a warning).
pub async fn reload_if_changed(previous: &HotConfig) -> Option<(Config, HotConfigDiff)> {
    match load().await {
        Ok(config) => {
            let new_hot = HotConfig::from(&config);
            let diff = previous.diff(&new_hot);
            if diff.has_changes() {
                Some((config, diff))
            } else {
                None
            }
        }
        Err(e) => {
            tracing::warn!("config reload failed, keeping current config: {e}");
            None
        }
    }
}
```

- [ ] **Step 4: Re-export from lib.rs**

In `crates/config/src/lib.rs`, add:
```rust
pub use loader::reload_if_changed;
```

- [ ] **Step 5: Run full config tests**

Run: `cargo nextest run -p config`
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/config/src/loader.rs crates/config/src/lib.rs
git commit -m "feat(config): add reload_if_changed for hot-reload support"
```

---

### Task 3: Create ConfigWatcherService in app-core

**Files:**
- Create: `crates/app-core/src/infrastructure/config_watcher.rs`
- Modify: `crates/app-core/src/infrastructure/mod.rs`

- [ ] **Step 1: Write the ConfigWatcherService**

Create `crates/app-core/src/infrastructure/config_watcher.rs`:

```rust
//! Watches config.json for external changes and propagates hot-reloadable
//! fields to the live agent pipeline.

use std::sync::Arc;

use config::HotConfig;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Spawns a background task that polls config.json for changes every 5 seconds.
///
/// When a change is detected in hot-reloadable fields, updates the shared
/// `HotConfig` and the `AppCore.config` RwLock.
///
/// Returns a `CancellationToken` to stop the watcher.
pub fn start_config_watcher(
    app_config: Arc<RwLock<config::Config>>,
    hot_config: Arc<RwLock<HotConfig>>,
    shutdown_token: CancellationToken,
) -> CancellationToken {
    let watcher_token = shutdown_token.child_token();
    let token = watcher_token.clone();

    tokio::spawn(async move {
        info!("config watcher started (5s poll interval)");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    debug!("config watcher shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let previous = hot_config.read().await.clone();
                    if let Some((new_config, diff)) = config::reload_if_changed(&previous).await {
                        info!(
                            model_changed = diff.model_changed,
                            temp_changed = diff.temperature_changed,
                            tokens_changed = diff.max_tokens_changed,
                            iterations_changed = diff.max_tool_iterations_changed,
                            timeout_changed = diff.pipeline_timeout_changed,
                            budget_changed = diff.budget_changed,
                            "config hot-reload: applying changes"
                        );

                        // Update shared HotConfig (read by agent pipeline)
                        {
                            let mut guard = hot_config.write().await;
                            *guard = HotConfig::from(&new_config);
                        }

                        // Update full config in AppCore (read by settings handlers)
                        {
                            let mut guard = app_config.write().await;
                            *guard = new_config;
                        }
                    }
                }
            }
        }
    });

    watcher_token
}
```

- [ ] **Step 2: Declare the module**

In `crates/app-core/src/infrastructure/mod.rs`, add:
```rust
pub mod config_watcher;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p app-core`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/infrastructure/config_watcher.rs crates/app-core/src/infrastructure/mod.rs
git commit -m "feat(app-core): add ConfigWatcherService for config.json hot-reload"
```

---

### Task 4: Wire HotConfig into AgentLoop and AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Add `hot_config` field to AgentLoop**

In `crates/agent/src/agent_loop/mod.rs`, add to the `AgentLoop` struct fields (after `embedding_engine`):

```rust
    /// Shared hot-reloadable config — updated by ConfigWatcherService without restart.
    pub(crate) hot_config: Arc<RwLock<config::HotConfig>>,
```

- [ ] **Step 2: Add `hot_config` field to AgentRuntime**

In `crates/agent/src/agent_runtime/runtime.rs`, add to the `AgentRuntime` struct:

```rust
    /// Shared hot-reloadable config — read per-message for model, temperature, iterations.
    hot_config: Arc<RwLock<config::HotConfig>>,
```

- [ ] **Step 3: Update AgentRuntime::process_message to read from hot_config**

In `crates/agent/src/agent_runtime/runtime.rs`, inside `process_message`, find the section where `PipelineConfig` / pipeline params are used (the execution step). Add a hot config read near the top of the method, after the `pipeline_start` line:

```rust
        // Read hot-reloadable config for this message
        let hot = self.hot_config.read().await.clone();
```

Then where `self.pipeline_config.max_tool_iterations` is referenced for the iteration budget cap (around step 4b), use `hot.max_tool_iterations` instead. Similarly for `pipeline_timeout_secs`.

Specifically, find the line that reads `self.pipeline_config.pipeline_timeout_secs` and replace with `hot.pipeline_timeout_secs`. Find iteration budget cap and replace `self.pipeline_config.max_tool_iterations` with `hot.max_tool_iterations`.

Also update the `ChatParams` construction to use `hot.temperature` and `hot.max_tokens` instead of the baked-in values.

**Note to implementer:** Search for all uses of `self.pipeline_config.max_tool_iterations`, `self.pipeline_config.pipeline_timeout_secs`, `self.pipeline_config.temperature`, and `self.pipeline_config.max_tokens` inside `runtime.rs` and replace with reads from `hot`. The `PipelineConfig` struct remains as a fallback/default container but the hot values take precedence.

- [ ] **Step 4: Update the builder to wire hot_config**

In `crates/agent/src/agent_loop/builder.rs`:

1. Add a field to `AgentLoopBuilder`:
```rust
    hot_config: Option<Arc<RwLock<config::HotConfig>>>,
```

2. Add a builder method:
```rust
    pub fn with_hot_config(mut self, hot_config: Arc<RwLock<config::HotConfig>>) -> Self {
        self.hot_config = Some(hot_config);
        self
    }
```

3. In the `build()` method, extract or create the hot_config:
```rust
        let hot_config = self.hot_config.unwrap_or_else(|| {
            Arc::new(RwLock::new(config::HotConfig::from(&config)))
        });
```

4. Pass `Arc::clone(&hot_config)` when constructing `AgentRuntime`.

5. Set `hot_config` on the `AgentLoop` struct at the end of `build()`.

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p agent`
Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire HotConfig into AgentLoop and AgentRuntime"
```

---

### Task 5: Wire HotConfig into AppCore init and settings handlers

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/handlers/settings/config.rs`

- [ ] **Step 1: Change `AppCore.config` to `Arc<RwLock<Config>>` and add `hot_config`**

In `crates/app-core/src/state.rs`, change the `config` field from:
```rust
    pub config: RwLock<config::Config>,
```
to:
```rust
    pub config: Arc<RwLock<config::Config>>,
    /// Shared hot-reloadable config subset — updated by file watcher and settings handlers.
    pub hot_config: Arc<RwLock<config::HotConfig>>,
```

**Call-site migration:** All existing `self.config.read().await` and `self.config.write().await` calls continue to work because `Arc<RwLock<T>>` implements `Deref` to `RwLock<T>`. No changes needed in handler files. Only the struct literal initialization in `init/mod.rs` changes.

- [ ] **Step 2: Create shared Arc before AppCore construction and wire everything**

In `crates/app-core/src/init/mod.rs`, **before** the `AppCore { ... }` struct literal:

1. Create the shared config Arc and HotConfig:
```rust
        let shared_config = Arc::new(RwLock::new(config));
        let hot_config = Arc::new(RwLock::new(config::HotConfig::from(
            &*shared_config.read().await,
        )));
```

2. Pass hot_config to the agent builder (in the agent builder chain):
```rust
        .with_hot_config(Arc::clone(&hot_config))
```

3. In the `AppCore { ... }` struct literal, change:
```rust
        config: RwLock::new(config),
```
to:
```rust
        config: Arc::clone(&shared_config),
        hot_config: Arc::clone(&hot_config),
```

4. Start the config watcher (after the AppCore struct literal, before returning):
```rust
        let _config_watcher_token = crate::infrastructure::config_watcher::start_config_watcher(
            Arc::clone(&shared_config),
            Arc::clone(&hot_config),
            shutdown_token.clone(),
        );
```

**Note:** Since `config` is consumed by `shared_config`, all references to `config` after this point in init must use `shared_config.read().await` instead. The implementer should search for `&config` usages after the `shared_config` creation point and update them.

- [ ] **Step 3: Push to hot_config after config_update_section**

In `crates/app-core/src/handlers/settings/config.rs`, at the end of `config_update_section`, after `*cfg = updated;`, add:

```rust
        // Propagate hot-reloadable changes to the live pipeline
        let new_hot = config::HotConfig::from(&*cfg);
        {
            let mut hot = self.hot_config.write().await;
            *hot = new_hot;
        }
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p app-core`
Expected: Compiles without errors.

- [ ] **Step 5: Run existing tests**

Run: `cargo nextest run -p app-core`
Expected: All existing tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/mod.rs crates/app-core/src/handlers/settings/config.rs
git commit -m "feat(app-core): wire HotConfig into AppCore init and settings handlers"
```

---

### Task 6: Integration test for config hot-reload

**Files:**
- Modify: `crates/config/src/loader.rs` (add test)

- [ ] **Step 1: Write integration test**

In `crates/config/src/loader.rs` tests:

```rust
/// Test reload_if_changed by directly testing the diff logic
/// without touching env vars (avoids parallel test unsafety).
#[test]
fn test_reload_if_changed_logic() {
    use super::super::schema::hot::HotConfig;

    // Simulate: config loaded at startup
    let mut config_v1 = Config::default();
    config_v1.agents.defaults.model = "model-v1".to_string();
    let hot_v1 = HotConfig::from(&config_v1);

    // Simulate: same config reloaded (no changes)
    let hot_v1b = HotConfig::from(&config_v1);
    assert!(!hot_v1.diff(&hot_v1b).has_changes(), "no changes expected");

    // Simulate: config changed on disk
    let mut config_v2 = Config::default();
    config_v2.agents.defaults.model = "model-v2".to_string();
    config_v2.agents.defaults.temperature = 0.9;
    let hot_v2 = HotConfig::from(&config_v2);

    let diff = hot_v1.diff(&hot_v2);
    assert!(diff.has_changes());
    assert!(diff.model_changed);
    assert!(diff.temperature_changed);
    assert!(!diff.max_tokens_changed);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo nextest run -p config --no-capture -E 'test(reload_if_changed_logic)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/loader.rs
git commit -m "test(config): add integration test for reload_if_changed"
```

---

### Task 7: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the "Gotchas" section**

Replace the line:
```
- **Config changes require restart** of the desktop app.
```

With:
```
- **Config hot-reload**: Model, temperature, max_tokens, max_iterations, pipeline_timeout, and monthly_budget changes take effect within 5 seconds (file watcher) or immediately (via settings UI). Structural changes (channels, provider init, feature enable/disable) still require restart.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with config hot-reload behavior"
```

---

## Feature 2: Progressive Skill Loading

### Task 8: Add `summary` field to SkillPackage

**Files:**
- Modify: `crates/skill-system/src/types.rs`
- Modify: `crates/skill-system/src/parser.rs`

- [ ] **Step 1: Write the test for summary parsing**

In `crates/skill-system/src/parser.rs`, add to the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_parse_summary_from_frontmatter() {
    let md = "---\nname: test-skill\ndescription: A test skill.\nmetadata:\n  klyntbot:\n    summary: Handles task CRUD and project management.\n---\nFull body here.";
    let pkg = parse_skill_md(md, std::path::PathBuf::from("test"), crate::types::SkillScope::BuiltIn).unwrap();
    assert_eq!(pkg.summary, "Handles task CRUD and project management.");
}

#[test]
fn test_parse_summary_fallback_to_first_sentence() {
    let md = "---\nname: test-skill\ndescription: A test skill.\n---\nThis is the first sentence. This is the second sentence.";
    let pkg = parse_skill_md(md, std::path::PathBuf::from("test"), crate::types::SkillScope::BuiltIn).unwrap();
    assert_eq!(pkg.summary, "This is the first sentence.");
}

#[test]
fn test_parse_summary_fallback_short_body() {
    let md = "---\nname: test-skill\ndescription: A test skill.\n---\nShort body";
    let pkg = parse_skill_md(md, std::path::PathBuf::from("test"), crate::types::SkillScope::BuiltIn).unwrap();
    assert_eq!(pkg.summary, "Short body");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p skill-system --no-capture -E 'test(summary)'`
Expected: FAIL — `summary` field doesn't exist.

- [ ] **Step 3: Add `summary` to `RawKlyntbotMeta` (parser.rs)**

`KlyntbotMeta` is NOT deserialized directly from YAML — it's constructed from `RawKlyntbotMeta` in `parse_metadata_block()`. Both structs need the field.

In `crates/skill-system/src/parser.rs`, add to the `RawKlyntbotMeta` struct:

```rust
    #[serde(default)]
    summary: Option<String>,
```

In the `parse_metadata_block()` function, add `summary` to the `KlyntbotMeta` construction (inside the `Some(KlyntbotMeta { ... })` block):

```rust
    Some(KlyntbotMeta {
        skill_type,
        tools: raw_km.tools,
        mcp_tools: raw_km.mcp_tools,
        can_delegate_to: raw_km.can_delegate_to,
        max_iterations: raw_km.max_iterations,
        always_skills: raw_km.always_skills,
        invokes: raw_km.invokes,
        triggers: raw_km.triggers,
        summary: raw_km.summary,  // <-- add this line
    })
```

- [ ] **Step 4: Add `summary` field to `KlyntbotMeta` (types.rs)**

In `crates/skill-system/src/types.rs`, add to the `KlyntbotMeta` struct:

```rust
    /// Short summary for progressive loading (1-2 sentences).
    /// Parsed from frontmatter `metadata.klyntbot.summary`.
    pub summary: Option<String>,
```

- [ ] **Step 5: Add `summary` field to `SkillPackage` (types.rs)**

In `crates/skill-system/src/types.rs`, add to the `SkillPackage` struct:

```rust
    /// Short summary for progressive loading (injected instead of full body for activated skills).
    pub summary: String,
```

- [ ] **Step 6: Update parser to compute summary and set on SkillPackage**

In `crates/skill-system/src/parser.rs`, in the `parse_skill_md` function, after extracting `body` and `klyntbot_meta`, compute the summary:

```rust
    // Summary: from klyntbot metadata, or first sentence of body
    let summary = klyntbot_meta
        .as_ref()
        .and_then(|k| k.summary.clone())
        .unwrap_or_else(|| extract_first_sentence(&body));
```

Add the field to the `SkillPackage` constructor:

```rust
    Ok(SkillPackage {
        name: raw.name,
        description: raw.description,
        skill_type,
        scope,
        location,
        body: body.trim().to_string(),
        summary,  // <-- add this line
        // ... rest unchanged
    })
```

Add the helper function:

```rust
/// Extract the first sentence from a text body (up to first period + space, or first newline).
fn extract_first_sentence(body: &str) -> String {
    let trimmed = body.trim();
    // Try period followed by space or end
    if let Some(idx) = trimmed.find(". ") {
        return trimmed[..=idx].to_string();
    }
    // Try first line
    if let Some(idx) = trimmed.find('\n') {
        let first_line = trimmed[..idx].trim();
        if !first_line.is_empty() {
            return first_line.to_string();
        }
    }
    // Fallback: entire body (truncated)
    trimmed.chars().take(200).collect()
}
```

- [ ] **Step 7: Update ALL other SkillPackage construction sites**

Adding `summary: String` (non-optional) to `SkillPackage` will break other construction sites. Update these:

1. **`crates/skill-system/src/discovery.rs`** — in `process_persona_entries()` (around line 240), the `SkillPackage { ... }` struct literal needs:
   ```rust
   summary: extract_first_sentence(&body),
   ```
   Import `extract_first_sentence` from `parser.rs` (may need to make it `pub(crate)`).

2. **`crates/skill-system/src/types.rs`** — all inline test `SkillPackage` literals (around lines 163, 180, 207, 230) need:
   ```rust
   summary: String::new(),
   ```

Search for all struct literal constructions: `rg "SkillPackage \{" crates/skill-system/src/` and verify each one includes `summary`.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system --no-capture`
Expected: All tests PASS (including the 3 new summary tests and all existing tests).

- [ ] **Step 9: Commit**

```bash
git add crates/skill-system/src/types.rs crates/skill-system/src/parser.rs
git commit -m "feat(skill-system): add summary field to SkillPackage for progressive loading"
```

---

### Task 9: Update SkillContextSource for progressive injection

**Files:**
- Modify: `crates/skill-system/src/context.rs`

- [ ] **Step 1: Write the test for summary-only activated skills**

In `crates/skill-system/src/context.rs`, add to tests:

```rust
#[tokio::test]
async fn test_activated_skills_inject_summary_not_full_body() {
    let skills = vec![
        (
            "general".to_string(),
            "---\nname: general\ndescription: General.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nOrchestrator body.".to_string(),
        ),
        (
            "helper".to_string(),
            "---\nname: helper\ndescription: Helper skill.\nmetadata:\n  klyntbot:\n    type: skill\n    summary: Helper does X and Y.\n---\nThis is a very long full body that should NOT appear in activated skill context. It contains detailed instructions.".to_string(),
        ),
    ];
    let source = SkillSource::BuiltIn(skills);
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

    let orchestrator = catalog.get("general").unwrap().clone();
    let helper = catalog.get("helper").unwrap().clone();

    let active = Arc::new(RwLock::new(Some(orchestrator)));
    let activated = Arc::new(RwLock::new(vec![helper]));
    let source = SkillContextSource::new(active, activated, Arc::new(HashMap::new()));

    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "1".into(),
        message: None,
        intent_summary: None,
        project_id: None,
    };
    let result = source.provide(&ctx).await.unwrap();

    // Orchestrator body should be present (full)
    assert!(result.contains("Orchestrator body"), "orchestrator gets full body");
    // Activated skill should show summary, not full body
    assert!(result.contains("Helper does X and Y"), "activated skill shows summary");
    assert!(!result.contains("very long full body"), "activated skill should NOT show full body");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p skill-system --no-capture -E 'test(activated_skills_inject_summary)'`
Expected: FAIL — currently injects full body for activated skills.

- [ ] **Step 3: Update `provide()` to use summary for activated skills**

In `crates/skill-system/src/context.rs`, modify the activated skills loop in `provide()`. Change the section that iterates `skills_guard` (around line 134-148):

```rust
        // Per-message activated skills: inject SUMMARY only (progressive loading)
        let skills_guard = self.activated_skills.read().await;
        for pkg in skills_guard.iter() {
            let already_active = !names_guard.insert(pkg.name.clone());
            if already_active {
                tracing::debug!(skill = %pkg.name, "Skipping duplicate skill activation");
                continue;
            }
            // Progressive: summary + resource listing (not full body)
            sections.push(format!(
                "<skill_content name=\"{}\" mode=\"summary\">\n# Skill: {} (activated)\n\n{}\n\nUse the `skill_reference` tool to load full instructions if needed.{}",
                pkg.name,
                pkg.name,
                pkg.summary,
                Self::resource_listing(pkg)
            ));
            sections.push("</skill_content>".to_string());
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system --no-capture`
Expected: All tests PASS (including the new one and all existing ones).

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/src/context.rs
git commit -m "feat(skill-system): inject summary for activated skills instead of full body"
```

---

### Task 10: Make always-loaded references conditional on relevance

**Files:**
- Modify: `crates/skill-system/src/context.rs`

- [ ] **Step 1: Write the test for conditional always-skills**

In `crates/skill-system/src/context.rs` tests:

```rust
#[tokio::test]
async fn test_always_skills_filtered_by_relevance() {
    let skills = vec![(
        "task-mgmt".to_string(),
        "---\nname: task-mgmt\ndescription: Task management.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    always_skills: [todo, daily-planner]\n---\nYou are the task agent.".to_string(),
    )];
    let source_data = SkillSource::BuiltIn(skills);
    let catalog = SkillCatalog::discover_sync(&[source_data]).unwrap();

    let mut reference_files = HashMap::new();
    reference_files.insert(
        "builtin::task-mgmt/references/todo.md".to_string(),
        "# Todo Workflow\nCreate and manage tasks.".to_string(),
    );
    reference_files.insert(
        "builtin::task-mgmt/references/daily-planner.md".to_string(),
        "# Daily Planner\nPlan your day with time blocking.".to_string(),
    );

    let pkg = catalog.get("task-mgmt").unwrap().clone();
    let active = Arc::new(RwLock::new(Some(pkg)));
    let activated = Arc::new(RwLock::new(vec![]));
    let ctx_source = SkillContextSource::new(active, activated, Arc::new(reference_files));

    // Message about completing a task — "todo" is a single-token ref (always loaded),
    // "daily-planner" is multi-token and neither "daily" nor "planner" appear in the message.
    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "1".into(),
        message: Some("mark my task as done".to_string()),
        intent_summary: None,
        project_id: None,
    };
    let result = ctx_source.provide(&ctx).await.unwrap();

    // "todo" is single-token → always loaded
    assert!(result.contains("Todo Workflow"), "single-token ref should always load");
    // "daily-planner" tokens are "daily" and "planner" — neither in "mark my task as done"
    assert!(!result.contains("Daily Planner"), "multi-token ref should be filtered when irrelevant");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p skill-system --no-capture -E 'test(always_skills_filtered)'`
Expected: FAIL — currently all always-skills are loaded unconditionally.

- [ ] **Step 3: Add relevance check to `always_skill_content`**

Update `always_skill_content` in `crates/skill-system/src/context.rs`:

```rust
    /// Load always_skills reference files, filtered by message relevance.
    ///
    /// A reference is "relevant" if any word from the reference name appears
    /// in the user message, or if no message is available (load all).
    fn always_skill_content(&self, orchestrator: &SkillPackage, message: Option<&str>) -> Vec<String> {
        let mut content = Vec::new();
        let msg_lower = message.map(|m| m.to_lowercase());

        for skill_name in orchestrator.always_skills() {
            // Relevance check: if we have a message, check if the reference name
            // tokens appear in it. Always load if no message context.
            if let Some(ref msg) = msg_lower {
                if !is_reference_relevant(skill_name, msg) {
                    tracing::debug!(
                        skill = %skill_name,
                        "Skipping always-skill reference (not relevant to message)"
                    );
                    continue;
                }
            }

            // (existing lookup logic unchanged)
            let fs_key = format!(
                "{}/references/{}.md",
                orchestrator.location.display(),
                skill_name
            );
            let builtin_key = format!(
                "builtin::{}/references/{}.md",
                orchestrator.name, skill_name
            );
            let text = self
                .reference_files
                .get(&fs_key)
                .or_else(|| self.reference_files.get(&builtin_key));

            if let Some(text) = text {
                content.push(format!("# Skill: {}\n\n{}", skill_name, text));
            } else {
                tracing::debug!(
                    skill = %skill_name,
                    orchestrator = %orchestrator.name,
                    "Always-skill reference not found (tried: {fs_key}, {builtin_key})"
                );
            }
        }
        content
    }
```

Add the helper function:

```rust
/// Check if a reference name is relevant to the user message.
/// Splits the reference name on hyphens and checks for token presence.
/// Single-token references (no hyphens) are always loaded to avoid false negatives.
/// Multi-token references (e.g., "daily-planner") require at least one token match.
fn is_reference_relevant(reference_name: &str, message_lower: &str) -> bool {
    let tokens: Vec<&str> = reference_name.split('-').collect();
    // Single-token references (e.g., "todo") — always load (too short to filter reliably)
    if tokens.len() == 1 {
        return true;
    }
    // Multi-token references — check if any token appears in the message
    tokens.iter().any(|t| t.len() > 2 && message_lower.contains(t))
}
```

Update the `provide()` call to pass the message:

```rust
        // Always-loaded skills (filtered by message relevance)
        let message_text = _ctx.message.as_deref();
        sections.extend(self.always_skill_content(orchestrator, message_text));
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p skill-system --no-capture`
Expected: All tests PASS. Verify the existing `test_context_source_injects_always_skills` still passes (it has `message: None` so all references load).

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/src/context.rs
git commit -m "feat(skill-system): make always-loaded references conditional on message relevance"
```

---

### Task 11: Create SkillReferenceTool

**Files:**
- Create: `crates/tools/src/domain/skill_reference.rs`
- Modify: `crates/tools/src/domain/mod.rs`

- [ ] **Step 1: Write the test**

In `crates/tools/src/domain/skill_reference.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_serve_builtin_skill_body() {
        let mut bodies = HashMap::new();
        bodies.insert("task-management".to_string(), "Full task management instructions...".to_string());
        let index = SkillReferenceIndex {
            skill_bodies: bodies,
            reference_files: HashMap::new(),
        };

        let result = index.get_skill_body("task-management");
        assert_eq!(result, Some("Full task management instructions..."));
    }

    #[test]
    fn test_serve_reference_file() {
        let mut refs = HashMap::new();
        refs.insert(
            "task-management/todo".to_string(),
            "# Todo workflow content".to_string(),
        );
        let index = SkillReferenceIndex {
            skill_bodies: HashMap::new(),
            reference_files: refs,
        };

        let result = index.get_reference("task-management", "todo");
        assert_eq!(result, Some("# Todo workflow content"));
    }

    #[test]
    fn test_list_available() {
        let mut bodies = HashMap::new();
        bodies.insert("general".to_string(), "body".to_string());
        bodies.insert("task-management".to_string(), "body".to_string());
        let mut refs = HashMap::new();
        refs.insert("task-management/todo".to_string(), "content".to_string());
        refs.insert("task-management/daily-planner".to_string(), "content".to_string());

        let index = SkillReferenceIndex {
            skill_bodies: bodies,
            reference_files: refs,
        };

        let listing = index.list_available();
        assert!(listing.contains("general"));
        assert!(listing.contains("task-management"));
        assert!(listing.contains("todo"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools --no-capture -E 'test(skill_reference)'`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement SkillReferenceTool**

Create `crates/tools/src/domain/skill_reference.rs`:

```rust
//! Tool for on-demand loading of skill instructions and reference files.
//!
//! Part of the progressive skill loading system: activated skills inject
//! summaries into context, and the agent calls this tool when it needs
//! the full instructions.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tools_core::Tool;

use crate::RoutingContext;

/// Read-only index of skill bodies and reference files.
/// Built at startup from the SkillCatalog, shared via Arc.
pub struct SkillReferenceIndex {
    /// Full skill body keyed by skill name.
    pub skill_bodies: HashMap<String, String>,
    /// Reference file content keyed by "skill_name/reference_name".
    pub reference_files: HashMap<String, String>,
}

impl SkillReferenceIndex {
    pub fn get_skill_body(&self, skill_name: &str) -> Option<&str> {
        self.skill_bodies.get(skill_name).map(|s| s.as_str())
    }

    pub fn get_reference(&self, skill_name: &str, ref_name: &str) -> Option<&str> {
        let key = format!("{}/{}", skill_name, ref_name);
        self.reference_files.get(&key).map(|s| s.as_str())
    }

    pub fn list_available(&self) -> String {
        let mut lines = vec!["Available skill content:".to_string()];
        for name in self.skill_bodies.keys() {
            lines.push(format!("  - skill_body: {name}"));
            // List references for this skill
            for ref_key in self.reference_files.keys() {
                if ref_key.starts_with(&format!("{name}/")) {
                    let ref_name = ref_key.strip_prefix(&format!("{name}/")).unwrap_or(ref_key);
                    lines.push(format!("    - reference: {ref_name}"));
                }
            }
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillReferenceParams {
    /// Action: "get_body", "get_reference", or "list"
    pub action: String,
    /// Skill name (required for get_body and get_reference)
    #[serde(default)]
    pub skill_name: Option<String>,
    /// Reference file name without .md (required for get_reference)
    #[serde(default)]
    pub reference_name: Option<String>,
}

pub struct SkillReferenceTool {
    index: Arc<SkillReferenceIndex>,
}

impl SkillReferenceTool {
    pub fn new(index: Arc<SkillReferenceIndex>) -> Self {
        Self { index }
    }
}

/// Note: The `Tool` trait uses `#[async_trait]`, so `execute` must be `async fn`.
#[async_trait::async_trait]
impl Tool for SkillReferenceTool {
    fn name(&self) -> &str {
        "skill_reference"
    }

    fn description(&self) -> &str {
        "Load full skill instructions or reference files. Use when a skill summary indicates you need detailed instructions for a task."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get_body", "get_reference", "list"],
                    "description": "Action: get_body (full skill instructions), get_reference (specific reference file), list (available content)"
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name of the skill (e.g., 'task-management')"
                },
                "reference_name": {
                    "type": "string",
                    "description": "Reference file name without .md extension (e.g., 'todo')"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> common::Result<String> {
        let params: SkillReferenceParams = serde_json::from_value(args)
            .map_err(|e| common::KlyntbotError::Tool(common::ToolError::InvalidParams(e.to_string())))?;

        match params.action.as_str() {
            "list" => Ok(self.index.list_available()),
            "get_body" => {
                let name = params.skill_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "skill_name required for get_body".into(),
                    ))
                })?;
                match self.index.get_skill_body(&name) {
                    Some(body) => Ok(body.to_string()),
                    None => Ok(format!("Skill '{name}' not found. Use action 'list' to see available skills.")),
                }
            }
            "get_reference" => {
                let skill = params.skill_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "skill_name required for get_reference".into(),
                    ))
                })?;
                let ref_name = params.reference_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "reference_name required for get_reference".into(),
                    ))
                })?;
                match self.index.get_reference(&skill, &ref_name) {
                    Some(content) => Ok(content.to_string()),
                    None => Ok(format!("Reference '{ref_name}' not found for skill '{skill}'. Use action 'list' to see available content.")),
                }
            }
            other => Ok(format!("Unknown action '{other}'. Use 'list', 'get_body', or 'get_reference'.")),
        }
    }
}

// tests at bottom (see Step 1)
```

- [ ] **Step 4: Declare the module**

In `crates/tools/src/domain/mod.rs`, add:
```rust
pub mod skill_reference;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p tools --no-capture -E 'test(skill_reference)'`
Expected: All 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/tools/src/domain/skill_reference.rs crates/tools/src/domain/mod.rs
git commit -m "feat(tools): add SkillReferenceTool for on-demand skill content loading"
```

---

### Task 12: Register SkillReferenceTool in the agent builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Build the SkillReferenceIndex from SkillCatalog**

In the builder's `build()` method, **after** the `reference_files` HashMap is built from `builtin_reference_map()` and **after** the `SkillCatalog` is constructed, create the index:

```rust
        // Build skill reference index for progressive loading
        let skill_reference_index = {
            let catalog = skill_catalog.read().await;
            let mut skill_bodies = std::collections::HashMap::new();
            let mut ref_files = std::collections::HashMap::new();

            // catalog.all_skills() returns impl Iterator<Item = &Arc<SkillPackage>>
            for pkg in catalog.all_skills() {
                skill_bodies.insert(pkg.name.clone(), pkg.body.clone());
            }

            // Re-key reference files from "builtin::skill/references/name.md" to "skill/name"
            // `reference_files` is the HashMap<String, String> already built above from
            // builtin_reference_map() — it must be in scope at this point.
            for (key, content) in reference_files.iter() {
                if let Some(captures) = parse_reference_key(key) {
                    ref_files.insert(
                        format!("{}/{}", captures.0, captures.1),
                        content.clone(),
                    );
                }
            }

            Arc::new(tools::domain::skill_reference::SkillReferenceIndex {
                skill_bodies,
                reference_files: ref_files,
            })
        };
```

Add a helper:

```rust
/// Parse a reference file key into (skill_name, reference_name).
fn parse_reference_key(key: &str) -> Option<(String, String)> {
    // Format: "builtin::{skill}/references/{name}.md"
    if let Some(rest) = key.strip_prefix("builtin::") {
        let parts: Vec<&str> = rest.splitn(2, "/references/").collect();
        if parts.len() == 2 {
            let name = parts[1].strip_suffix(".md").unwrap_or(parts[1]);
            return Some((parts[0].to_string(), name.to_string()));
        }
    }
    // Format: "{path}/references/{name}.md"
    if let Some(idx) = key.find("/references/") {
        let skill_path = &key[..idx];
        let skill_name = std::path::Path::new(skill_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(skill_path);
        let name = &key[idx + "/references/".len()..];
        let name = name.strip_suffix(".md").unwrap_or(name);
        return Some((skill_name.to_string(), name.to_string()));
    }
    None
}
```

- [ ] **Step 2: Register the tool**

```rust
        let skill_ref_tool = tools::domain::skill_reference::SkillReferenceTool::new(
            Arc::clone(&skill_reference_index),
        );
        tool_registry.write().await.register(Box::new(skill_ref_tool));
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p agent`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): register SkillReferenceTool in agent builder"
```

---

### Task 13: Add summaries to built-in skills

**Files:**
- Modify: `skills/general/SKILL.md`
- Modify: `skills/task-management/SKILL.md`
- Modify: `skills/finance-management/SKILL.md`
- Modify: `skills/automation/SKILL.md`
- Modify: `skills/communication/SKILL.md`

- [ ] **Step 1: Read each skill file and add summary to frontmatter**

For each skill, add a `summary` field under `metadata.klyntbot`:

**general/SKILL.md:**
```yaml
    summary: General conversation, greetings, and fallback orchestrator for uncategorized requests.
```

**task-management/SKILL.md:**
```yaml
    summary: Task CRUD, project management, OKR tracking, PARA methodology, weekly reviews, and daily planning.
```

**finance-management/SKILL.md:**
```yaml
    summary: Expense tracking, budgeting, 6-jar allocation, FIRE analytics, and financial goal management.
```

**automation/SKILL.md:**
```yaml
    summary: Cron job scheduling, reminders, recurring tasks, and time-based automation.
```

**communication/SKILL.md:**
```yaml
    summary: Cross-platform messaging via Telegram, Discord, Slack, and email.
```

- [ ] **Step 2: Verify skills still parse correctly**

Run: `cargo nextest run -p skill-system`
Expected: All tests PASS.

- [ ] **Step 3: Commit**

```bash
git add skills/
git commit -m "feat(skills): add summary fields to all built-in skills for progressive loading"
```

---

### Task 14: Full integration test

**Files:**
- This task uses existing test infrastructure.

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo nextest run --workspace`
Expected: All tests PASS. Zero regressions.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

- [ ] **Step 4: Commit any fixups**

If any tests failed or clippy caught issues, fix them and commit:

```bash
git commit -m "fix: address clippy warnings and test regressions"
```

---

### Task 15: Update CLAUDE.md with progressive skill loading

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add documentation**

In the "Skill system & MCP" section, add:

```
**Progressive skill loading:** Orchestrator skills inject their full body on first activation (deduplicated per session). Activated (non-orchestrator) skills inject a summary only — the agent calls `skill_reference` tool to load full instructions when needed. Always-loaded references are filtered by message relevance (keyword match on reference name). This reduces token usage by ~30% for simple messages.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document progressive skill loading in CLAUDE.md"
```
