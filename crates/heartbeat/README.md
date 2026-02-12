# klyntbot-heartbeat

**Periodic agent wake-up service.**

## Overview

`klyntbot-heartbeat` provides scheduled agent wake-ups for proactive tasks:
- Configurable interval (default: 1 hour)
- Reads `HEARTBEAT.md` for actionable tasks
- Sends wake-up messages to agent
- Runs as background async task

## Contents

### Heartbeat Service

```rust
use klyntbot_heartbeat::HeartbeatService;
use std::time::Duration;

// Create heartbeat service
let interval = Duration::from_secs(3600);  // 1 hour
let workspace = PathBuf::from("/workspace");
let heartbeat = HeartbeatService::new(interval, workspace);

// Start service (runs in background)
heartbeat.start(bus.inbound_sender()).await;
```

### Service Behavior

1. **Wait** for configured interval
2. **Read** `workspace/HEARTBEAT.md`
3. **Check** if file contains actionable tasks
4. **Send** wake-up message to agent via bus
5. **Repeat**

### HEARTBEAT.md Format

```markdown
# Heartbeat Tasks

## Periodic Reminders
- Check for pending GitHub PRs every 2 hours
- Summarize daily activity at 5 PM
- Send weekly report on Fridays at 9 AM

## Conditions
- If unread email count > 10, notify user
- If disk space < 10%, send alert
```

Agent reads this file and decides which tasks to execute based on current conditions.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-heartbeat.workspace = true
```

Example:

```rust
use klyntbot_heartbeat::HeartbeatService;
use klyntbot_bus::MessageBus;
use std::time::Duration;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let (bus, mut inbound_rx, _) = MessageBus::new(100);
    let workspace = PathBuf::from("/workspace");

    // Wake up every 30 minutes
    let heartbeat = HeartbeatService::new(
        Duration::from_secs(1800),
        workspace
    );

    // Start service
    tokio::spawn(async move {
        heartbeat.start(bus.inbound_sender()).await;
    });

    // Agent receives heartbeat messages
    while let Some(msg) = inbound_rx.recv().await {
        println!("Heartbeat: {}", msg.content);
    }

    Ok(())
}
```

## Integration with Agent

### Agent Side

When agent receives a heartbeat message:

1. Read `HEARTBEAT.md` from workspace
2. Parse task list
3. Evaluate conditions (time, state, etc.)
4. Execute applicable tasks
5. Continue normal operation

### Example Workflow

```
[10:00] Heartbeat fires
        ↓
[10:00] Send InboundMessage to agent
        ↓
[10:00] Agent reads HEARTBEAT.md
        ↓
[10:00] Agent sees: "Check PRs every 2 hours"
        ↓
[10:00] Agent runs: gh pr list --state=open
        ↓
[10:01] Agent sends summary to user
```

## Configuration

In `config.json`:

```json
{
  "heartbeat": {
    "enabled": true,
    "interval": 3600  // seconds
  }
}
```

If not configured, heartbeat defaults to 1 hour.

## Use Cases

### Periodic Checks

```markdown
# HEARTBEAT.md

## Every Hour
- Check for new emails
- Monitor disk space
- Verify service health
```

### Time-Based Actions

```markdown
# HEARTBEAT.md

## Daily at 9 AM
- Send morning briefing
- Summarize overnight activity

## Weekly on Fridays
- Generate weekly report
- Clean up old sessions
```

### Conditional Tasks

```markdown
# HEARTBEAT.md

## Conditions
- If TODO.md has items, remind user
- If calendar has events today, send schedule
- If metrics exceed threshold, alert
```

## Design Principles

1. **Simple interval** — No complex cron expressions
2. **File-based config** — Tasks defined in markdown
3. **Agent decides** — Service just wakes up, agent interprets
4. **Non-blocking** — Runs as background async task
5. **Workspace-scoped** — Reads from agent's workspace

## Difference from Cron

| Feature | Heartbeat | Cron |
|---------|-----------|------|
| **Scheduling** | Fixed interval | Cron expressions |
| **Task definition** | Markdown file | Structured jobs |
| **Execution** | Agent interprets | Direct execution |
| **Flexibility** | Agent decides what to do | Rigid payload |
| **Use case** | Proactive checks | Scheduled messages |

**Use heartbeat for**: "Wake up periodically and check if anything needs doing"

**Use cron for**: "Execute this specific action at this specific time"

## Performance

- **Minimal overhead** — Sleeps between intervals, no polling
- **No persistence** — Stateless service, no file I/O
- **Async-native** — Non-blocking sleep with Tokio

## Dependencies

- `klyntbot-core` — Error types
- `tokio` — Async runtime and timers
- `tracing` — Logging

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Cron Service](../klyntbot-cron/README.md)
- [Agent Loop](../klyntbot-agent/README.md)
