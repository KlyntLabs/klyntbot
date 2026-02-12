# klyntbot-agent

**Core agent orchestration and loop.**

## Overview

`klyntbot-agent` is the brain of klyntbot:
- Agent loop (receive → think → act → respond)
- Context building with memory and skills
- Memory store (long-term + daily notes)
- Skill manager (built-in + custom skills)
- Subagent manager for background tasks

## Contents

### Agent Loop

```rust
use klyntbot_agent::AgentLoop;
use klyntbot_config::Config;
use klyntbot_bus::MessageBus;

// Create agent
let config = Config::load()?;
let (bus, inbound_rx, outbound_tx) = MessageBus::new(100);
let agent = AgentLoop::new(config, bus, workspace).await?;

// Start agent loop
agent.start(inbound_rx, outbound_tx).await?;
```

### Agent Cycle

```
┌─────────────────────────────────────────┐
│  1. Receive InboundMessage from bus     │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  2. Build context:                      │
│     - Load session history              │
│     - Read memory files (MEMORY.md)     │
│     - Load relevant skills              │
│     - Add system prompt                 │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  3. Call LLM with context + tools       │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  4. Execute tool calls (if any)         │
│     - Repeat up to maxToolIterations    │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  5. Send OutboundMessage to bus         │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  6. Update session and memory           │
└─────────────────────────────────────────┘
```

### Context Builder

```rust
use klyntbot_agent::ContextBuilder;

let context = ContextBuilder::new(workspace, config);

// Build context for message
let messages = context.build_context(
    &session,
    &user_message,
    &memory_store,
    &skill_manager,
).await?;

// Includes:
// - System prompt with agent instructions
// - Memory context (MEMORY.md, daily notes)
// - Loaded skills
// - Session history
// - Current user message
```

### Memory Store

```rust
use klyntbot_agent::MemoryStore;

let memory = MemoryStore::new(workspace);

// Read long-term memory
let memory_content = memory.read_memory().await?;

// Write to memory
memory.write_memory("Important fact: ...").await?;

// Read today's notes
let daily_notes = memory.read_daily_notes().await?;

// Append to daily notes
memory.append_daily_notes("Today I learned...").await?;
```

**Files**:
```
workspace/
  memory/
    MEMORY.md           ← Long-term persistent memory
    2026-02-12.md       ← Daily notes (auto-dated)
    2026-02-11.md
    2026-02-10.md
```

### Skill Manager

```rust
use klyntbot_agent::SkillManager;

let skills = SkillManager::new(workspace)?;

// List available skills
let all_skills = skills.list();
for skill in all_skills {
    println!("{}: {}", skill.name, skill.description);
}

// Load skill content
let cron_skill = skills.load_skill("cron")?;
println!("{}", cron_skill.content);
```

**Built-in skills** (embedded in binary):
- `cron` — Natural language scheduling
- `github` — Repository operations
- `weather` — Weather forecasts
- `summarize` — Document summarization
- `tmux` — Terminal multiplexer integration
- `skill-creator` — Create new skills

**Custom skills** (in workspace):
```
workspace/skills/
  my-skill/
    SKILL.md
```

### Subagent Manager

```rust
use klyntbot_agent::SubagentManager;

let subagent_mgr = SubagentManager::new(config.clone());

// Spawn background subagent
let task_id = subagent_mgr.spawn(
    "Analyze codebase and generate report".into(),
    Some("code-analysis".into()),
    "telegram".into(),
    "user123".into(),
).await;

// Subagent runs independently with restricted tools
// Results sent back to origin channel+chat
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-agent.workspace = true
```

Example:

```rust
use klyntbot_agent::AgentLoop;
use klyntbot_config::Config;
use klyntbot_bus::MessageBus;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let workspace = PathBuf::from("~/.klyntbot/workspace");
    let (bus, inbound_rx, outbound_tx) = MessageBus::new(100);

    let agent = AgentLoop::new(config, bus.clone(), workspace).await?;

    tokio::spawn(async move {
        agent.start(inbound_rx, outbound_tx).await
    });

    // Send test message
    bus.inbound_sender().send(InboundMessage {
        channel: ChannelName::Cli,
        chat_id: ChatId("test".into()),
        user_id: Some("test".into()),
        content: "Hello!".into(),
        attachments: vec![],
        timestamp: Utc::now(),
    }).await?;

    Ok(())
}
```

## Workspace Layout

```
~/.klyntbot/workspace/
  AGENTS.md           ← Agent instructions (behavior)
  SOUL.md             ← Personality definition
  USER.md             ← User preferences and info
  TOOLS.md            ← Tool usage guidelines
  IDENTITY.md         ← Identity overrides
  HEARTBEAT.md        ← Periodic task definitions
  memory/
    MEMORY.md         ← Long-term persistent memory
    2026-02-12.md     ← Daily notes (auto-dated)
  skills/
    custom-skill/
      SKILL.md        ← User-defined skills
```

## Skill Format

```markdown
---
description: "Your skill description"
metadata: '{"nanobot": {"requires": {"bins": ["tool"]}, "always": false}}'
---

# Skill Name

Instructions for the agent when this skill is active...
```

**Metadata fields**:
- `always: true` — Skill always loaded into system prompt
- `always: false` — Skill summary in prompt, full content on-demand
- `requires.bins` — Skill only available if binaries in PATH
- `requires.env` — Skill only available if env vars set

## Tool Iteration Limit

Prevents infinite loops:

```rust
// In config.json
{
  "agents": {
    "defaults": {
      "maxToolIterations": 20  // Default
    }
  }
}
```

If agent exceeds limit, loop stops and returns error message.

## Handler Implementations

`SubagentManager` implements `SpawnHandler` from `klyntbot-tools`:

```rust
impl SpawnHandler for SubagentManager {
    async fn spawn(&self, task: String, ...) -> String {
        // Create background agent with restricted tools
        // Return task ID
    }
}
```

`AgentLoop` implements `CronHandler` from `klyntbot-tools`:

```rust
impl CronHandler for AgentLoop {
    async fn create_job(&self, ...) -> Result<String> {
        // Delegate to CronService
    }
}
```

## Design Principles

1. **Single loop** — One agent handles all messages sequentially
2. **Context-rich** — Memory + skills + history for every response
3. **Tool-first** — Agent capabilities defined by tools
4. **Async I/O** — Non-blocking file/network operations
5. **Subagent isolation** — Background tasks run independently

## Dependencies

- `klyntbot-core` — Error types, shared types
- `klyntbot-bus` — Message bus integration
- `klyntbot-config` — Configuration loading
- `klyntbot-providers` — LLM calls
- `klyntbot-session` — Session persistence
- `klyntbot-tools` — Tool registry and execution
- `klyntbot-cron` — Cron job management
- `tokio` — Async runtime
- `serde_yaml` — Skill YAML parsing
- `chrono` — Timestamps
- `uuid` — Subagent IDs
- `base64`, `mime_guess` — Attachment handling
- `which` — Binary checking for skills
- `dirs` — Directory resolution

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Context Building](../../docs/ARCHITECTURE.md#core-abstractions)
- [Skills](../../README.md#skills)
