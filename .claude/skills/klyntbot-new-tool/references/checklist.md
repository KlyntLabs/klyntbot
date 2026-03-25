---
name: new-tool-checklist
description: Step-by-step file-by-file guide for creating a new klyntbot tool
metadata:
  always: false
---

# New Tool Wiring Checklist

## Step 1: Create the feature crate

```
crates/feature-{name}/
  Cargo.toml
  src/
    lib.rs      — FeaturePackage impl
    tool.rs     — Tool impl
  migrations/
    001_{name}_tables.sql  — (if needed)
```

**Cargo.toml** must depend on:
```toml
[dependencies]
common = { path = "../common" }
tools-core = { path = "../tools-core" }
storage = { path = "../storage" }
async-trait = "0.1"
serde_json = "1"
tracing = "0.1"
```

Add to workspace members in root `Cargo.toml`.

## Step 2: Implement the Tool

**Using `#[tool_actions]` (preferred):**
```rust
use tools_core::{tool_actions, ActionParams, RoutingContext};

#[derive(Debug, ActionParams)]
pub struct MyActionParams {
    /// Description for JSON schema
    #[param(required)]
    pub field: String,
}

pub struct MyTool { /* dependencies */ }

#[tool_actions(
    name = "my_tool",
    description = "What the tool does",
    category = "General",
    tags = "tag1,tag2",
    cost = "Free"
)]
impl MyTool {
    #[action(name = "do_thing")]
    async fn do_thing(&self, params: MyActionParams, ctx: &RoutingContext) -> common::Result<String> {
        Ok("done".to_string())
    }
}
```

Valid categories: General, FileSystem, Search, Web, Communication,
TaskManagement, Memory, Finance, Productivity, System, Mcp, Plugin.

## Step 3: Implement FeaturePackage

```rust
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct MyFeature { /* deps */ }

impl MyFeature {
    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "my_feature",
            version: 1,
            sql: include_str!("../migrations/001_my_tables.sql"),
        }]
    }
}

#[async_trait::async_trait]
impl FeaturePackage for MyFeature {
    fn name(&self) -> &str { "my_feature" }
    fn tools(&self) -> Vec<DynTool> { vec![Arc::new(self.tool.clone())] }
    fn migration_sql(&self) -> Option<&str> { Some(include_str!("../migrations/001_my_tables.sql")) }
    async fn health_check(&self) -> HealthStatus { HealthStatus::Healthy }
}
```

## Step 4: Register in agent builder

In `crates/agent/src/agent_loop/builder.rs`, inside the tool registration block:

```rust
// Register my_tool
let my_tool = MyTool::new(pool.clone());
tool_registry.register(my_tool);
```

## Step 5: Add to MCP whitelist

In `crates/config/src/schema/mcp.rs`, in `default_exposed_tools()`:

```rust
vec![
    // ... existing tools ...
    "my_tool".to_string(),   // <- add here
]
```

## Step 6: Add Tauri commands

Create `crates/desktop/src/commands/{name}.rs`:

```rust
use crate::state::AppCore;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn my_command(state: State<'_, Arc<AppCore>>) -> Result<serde_json::Value, String> {
    // delegate to state.my_method().await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["my_command"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, crate::commands::ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "my_command" => dev::val(core.my_method().await),
        _ => return None,
    })
}
```

Then wire:
1. `commands/mod.rs` — add `pub mod {name};`
2. `main.rs` — add to `generate_handler![...]`
3. `dev_server/mod.rs` — add to `dev_command_names()` modules list
4. `dev_server/dispatch.rs` — add dispatch arm

## Step 7: Create Claude Code skill

Create `.claude/skills/klyntbot-{name}/SKILL.md` with YAML frontmatter
matching the tool's registry name, actions, and common mistakes.
