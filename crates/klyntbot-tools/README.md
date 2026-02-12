# klyntbot-tools

**Tool trait and implementations for agent capabilities.**

## Overview

`klyntbot-tools` provides the tool system for klyntbot:
- `Tool` trait for extensible capabilities
- 10 built-in tool implementations
- Handler traits for dependency inversion
- Parameter validation and JSON schema
- Workspace sandboxing for safety

## Contents

### Tool Trait

```rust
use klyntbot_tools::Tool;
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

### Built-in Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents with optional workspace restriction |
| `write_file` | Write files, auto-creating parent directories |
| `edit_file` | Find-and-replace with uniqueness validation |
| `list_dir` | List directory contents with type indicators |
| `exec` | Execute shell commands with safety guards |
| `web_search` | Search the web via Brave Search API |
| `web_fetch` | Fetch and extract readable content from URLs |
| `message` | Send messages to users through any channel |
| `spawn` | Create background subagents for complex tasks |
| `cron` | Schedule recurring tasks |

### Tool Registry

```rust
use klyntbot_tools::{ToolRegistry, DynTool};

// Create registry
let mut registry = ToolRegistry::new();

// Register tools
registry.register(Arc::new(ReadFileTool::new(workspace)));
registry.register(Arc::new(WriteFileTool::new(workspace)));
registry.register(Arc::new(ExecTool::new(timeout)));

// Get tool
let tool = registry.get("read_file")?;
let result = tool.execute(args, &ctx).await?;

// List all tools
let tools: Vec<String> = registry.list();
```

### Filesystem Tools

```rust
use klyntbot_tools::{ReadFileTool, WriteFileTool, EditFileTool, ListDirTool};

// Read file
let read_tool = ReadFileTool::new(Some("/workspace"));
let content = read_tool.execute(
    serde_json::json!({"path": "src/main.rs"}),
    &ctx
).await?;

// Write file
let write_tool = WriteFileTool::new(Some("/workspace"));
write_tool.execute(
    serde_json::json!({
        "path": "output.txt",
        "content": "Hello, world!"
    }),
    &ctx
).await?;

// Edit file (find-and-replace)
let edit_tool = EditFileTool::new(Some("/workspace"));
edit_tool.execute(
    serde_json::json!({
        "path": "config.json",
        "old": "\"port\": 8080",
        "new": "\"port\": 3000"
    }),
    &ctx
).await?;

// List directory
let list_tool = ListDirTool::new(Some("/workspace"));
let listing = list_tool.execute(
    serde_json::json!({"path": "."}),
    &ctx
).await?;
```

### Shell Tool

```rust
use klyntbot_tools::ExecTool;

let exec_tool = ExecTool::new(60);  // 60 second timeout
let output = exec_tool.execute(
    serde_json::json!({
        "command": "ls -la",
        "cwd": "/workspace"
    }),
    &ctx
).await?;

// Safety: Blocks destructive commands like rm -rf, fork bombs, etc.
```

### Web Tools

```rust
use klyntbot_tools::{WebSearchTool, WebFetchTool};

// Search the web
let search_tool = WebSearchTool::new(api_key);
let results = search_tool.execute(
    serde_json::json!({
        "query": "Rust programming language",
        "max_results": 5
    }),
    &ctx
).await?;

// Fetch URL content
let fetch_tool = WebFetchTool::new();
let content = fetch_tool.execute(
    serde_json::json!({"url": "https://example.com"}),
    &ctx
).await?;
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-tools.workspace = true
```

Example:

```rust
use klyntbot_tools::{ToolRegistry, ReadFileTool, WriteFileTool, ExecTool};
use std::sync::Arc;

fn create_tools(workspace: &str) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Filesystem tools
    registry.register(Arc::new(ReadFileTool::new(Some(workspace.into()))));
    registry.register(Arc::new(WriteFileTool::new(Some(workspace.into()))));

    // Shell tool
    registry.register(Arc::new(ExecTool::new(60)));

    registry
}

#[tokio::main]
async fn main() -> Result<()> {
    let registry = create_tools("/workspace");

    let tool = registry.get("read_file")?;
    let content = tool.execute(
        serde_json::json!({"path": "README.md"}),
        &RoutingContext::default()
    ).await?;

    println!("{}", content);
    Ok(())
}
```

## Handler Traits

For tools that need access to higher-layer services:

### SpawnHandler

```rust
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;
}

pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}
```

**Usage**: Implemented by `SubagentManager` in `klyntbot-agent`.

### CronHandler

```rust
#[async_trait]
pub trait CronHandler: Send + Sync {
    async fn create_job(&self, ...) -> Result<String>;
    async fn list_jobs(&self) -> Result<String>;
    async fn delete_job(&self, id: &str) -> Result<String>;
}

pub struct CronTool {
    handler: Option<Arc<dyn CronHandler>>,
}
```

**Usage**: Implemented by `CronService` in `klyntbot-cron`.

## Safety Features

### Command Deny Patterns

Blocks destructive commands via regex:

```rust
// Blocked patterns (case-insensitive)
r"rm\s+-rf"           // rm -rf
r"del\s+/f"           // del /f
r"format\s"           // format
r"mkfs\."             // mkfs.ext4
r"dd\s+if="           // dd if=
r"shutdown|reboot"    // system shutdown
r":\(\)\{.*:\|:"      // fork bomb
```

### Workspace Sandboxing

Confine all file/shell operations to workspace:

```rust
let read_tool = ReadFileTool::new(Some("/workspace"));

// OK: /workspace/file.txt
// ERROR: /etc/passwd (outside workspace)
// ERROR: /workspace/../etc/passwd (path traversal)
```

### Output Truncation

Limits to prevent memory exhaustion:

```rust
ExecTool::new(60)       // 10 KB output limit
WebFetchTool::new()     // 50 KB content limit
```

## Parameter Validation

Tools define JSON schemas for parameters:

```rust
fn parameters(&self) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path to read"
            }
        },
        "required": ["path"]
    })
}
```

Validation happens before `execute()`:

```rust
// Invalid args rejected early
let result = tool.execute(
    serde_json::json!({}),  // Missing "path"
    &ctx
).await;
// Returns ToolError::InvalidParameters
```

## Routing Context

Tools receive contextual information:

```rust
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub user_id: Option<String>,
}
```

Used for:
- Message tool (knows where to send)
- Spawn tool (tracks origin for subagent responses)

## Design Principles

1. **Trait-based** — Easy to add custom tools
2. **JSON schema** — Self-documenting parameters
3. **Async execution** — Non-blocking I/O
4. **Safety first** — Deny patterns, sandboxing, truncation
5. **Dependency inversion** — Handler traits break cycles

## Dependencies

- `klyntbot-core` — Error types, shared types
- `klyntbot-bus` — Message sending (MessageTool)
- `async-trait` — Async trait support
- `tokio` — Async runtime
- `serde_json` — JSON handling
- `reqwest` — HTTP client (web tools)
- `regex` — Command deny patterns
- `shellexpand` — Tilde expansion
- `scraper`, `html2text` — HTML to markdown
- `url`, `urlencoding` — URL handling

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Agent Loop](../klyntbot-agent/README.md)
- [Extending klyntbot](../../docs/ARCHITECTURE.md#extension-points)
