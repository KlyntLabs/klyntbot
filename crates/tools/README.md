# tools

**Domain tools and shared infrastructure for klyntbot.**

## Overview

This crate retains:
- **Domain tools**: learning, memory, project, area, okr, delegation, cron, etc.
- **Embedding infrastructure**: engine (fastembed), store (LanceDB)
- **Tool registry and parameter utilities**

Primitive tools (read, write, edit, grep, glob, bash, web_fetch, ask_user, etc.) now live in `klynt-core`.

Feature-specific tools (tasks, finance) live in their own crates (`feature-tasks`, `feature-finance`) and depend on `tools-core` directly.

## Contents

### Tool Trait

```rust
use tools::Tool;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;  // JSON schema
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
}
```

### Domain Tools

| Tool | Description |
|------|-------------|
| `spawn` | Create background subagents for complex tasks |
| `cron` | Schedule recurring tasks |
| `agent_task` | Task board coordination (list, claim, complete) |
| `memory` | Store and retrieve user facts and preferences |
| `learning` | Adaptive tool threshold updates |
| `project` | Project management |
| `area` | Life area tracking |
| `okr` | Objective and key result management |

### Tool Registry

```rust
use tools::{ToolRegistry, DynTool};

// Create registry
let mut registry = ToolRegistry::new();

// Register a domain tool
registry.register(MyDomainTool::new());

// Get tool
let tool = registry.get("my_tool")?;
let result = tool.execute(args, &ctx).await?;

// List all tools
let tools: Vec<String> = registry.list();
```

## Architecture

```
tools/
├── src/
│   ├── domain/          # Domain-specific tools (okr, project, learning, ...)
│   ├── embedding/       # fastembed + LanceDB wrappers
│   ├── registry.rs      # ToolRegistry (dynamic dispatch)
│   └── params.rs        # ParamExtractor for JSON args
```

## Dependencies

- `tools-core` — trait definitions and macros
- `common` — shared types (ChannelMask, KlyntbotError, etc.)
- `storage` — SQLite repositories
- `cognitive` — memory and reasoning services
- `fastembed` — ONNX embedding model (optional, `semantic-search` feature)
