# Integration Design: Wiring klyntbot Components

**Document Version:** 1.0
**Author:** Architecture Engineer (Task #2)
**Date:** 2026-02-11
**Status:** Design Specification

---

## Executive Summary

This document provides the **complete integration architecture** for klyntbot, showing exactly how to wire the MessageBus, AgentLoop, ChannelManager, CronService, HeartbeatService, and tools together into a working system. It includes:

- Concrete Rust code for each integration point
- Data flow diagrams showing message routing
- Tool context injection strategy
- Streaming response architecture
- Error recovery patterns

**Key Finding:** The Rust codebase is 80% complete. The missing 20% is wiring code, tool context injection, and streaming integration.

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Gateway Mode Integration](#gateway-mode-integration)
3. [Agent Loop Message Processing Pipeline](#agent-loop-message-processing-pipeline)
4. [Tool Context Injection](#tool-context-injection)
5. [Subagent Integration](#subagent-integration)
6. [Streaming Architecture](#streaming-architecture)
7. [CLI Interactive Mode](#cli-interactive-mode)
8. [Error Recovery Strategy](#error-recovery-strategy)
9. [Implementation Checklist](#implementation-checklist)

---

## System Overview

### Component Dependencies

```
┌──────────────────────────────────────────────────────────┐
│                       main.rs                            │
│                    (Entry Point)                         │
└────────────┬─────────────────────────┬──────────────────┘
             │                         │
             │ Gateway                 │ Chat
             │ Mode                    │ Mode
             │                         │
┌────────────▼────────────┐   ┌────────▼────────────┐
│   handle_serve()        │   │  handle_chat()      │
│                         │   │                     │
│  1. MessageBus          │   │  1. MessageBus      │
│  2. Provider            │   │  2. Provider        │
│  3. AgentLoop           │   │  3. AgentLoop       │
│  4. ChannelManager      │   │  4. process_direct()│
│  5. CronService         │   │                     │
│  6. HeartbeatService    │   │                     │
└─────────────────────────┘   └─────────────────────┘
```

### Data Flow: Gateway Mode

```
Inbound:
Telegram/Discord/WhatsApp → MessageBus.inbound_tx
                              │
                              ├→ AgentLoop.run()
                              │    ├→ bus.consume_inbound()
                              │    ├→ process_message()
                              │    │   ├→ SessionManager (load/save)
                              │    │   ├→ ContextBuilder.build_messages()
                              │    │   ├→ Provider.chat()
                              │    │   └→ ToolRegistry.execute()
                              │    └→ bus.publish_outbound()
                              │
                              └→ ChannelManager outbound dispatcher
                                   └→ bus.consume_outbound()
                                        └→ channel.send()
```

### Data Flow: CLI Mode

```
User Input (REPL):
stdin → AgentLoop.process_direct()
         ├→ SessionManager (load/save)
         ├→ ContextBuilder.build_messages()
         ├→ Provider.chat_stream() [streaming enabled]
         │   └→ print chunks real-time
         └→ ToolRegistry.execute()
              └→ print result
```

---

## Gateway Mode Integration

### Current State (main.rs:169-267)

The `handle_serve()` function already creates all components but doesn't wire them correctly:

```rust
// CURRENT CODE (mostly correct)
let bus = Arc::new(MessageBus::new(100));
let provider = klyntbot::providers::create_provider(&config)?;
let agent_loop = Arc::new(AgentLoop::new(bus.clone(), provider, config.clone()).await?);
let channel_manager = Arc::new(ChannelManager::new(Arc::new(config.clone()), bus.clone()));
let cron_service = Arc::new(CronService::new(cron_store_path));
let heartbeat_service = Arc::new(HeartbeatService::new(workspace_path, 1800, true));

// Spawn agent loop
let agent_loop_handle = {
    let agent = agent_loop.clone();
    tokio::spawn(async move {
        if let Err(e) = agent.run().await {
            error!("Agent loop error: {}", e);
        }
    })
};

// Start channels (includes outbound dispatcher)
let channel_manager_handle = {
    let cm = channel_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = cm.start_all().await {
            error!("Channel manager error: {}", e);
        }
    })
};
```

### Issues to Fix

1. **CronService and HeartbeatService** need bus reference to publish messages
2. **CronService** should be passed to AgentLoop for cron tool integration
3. **Graceful shutdown** needs to call `.stop()` methods, not just `.abort()`

### Corrected Gateway Integration

```rust
// src/main.rs: handle_serve() improvements

async fn handle_serve(port: u16) -> anyhow::Result<()> {
    use klyntbot::{AgentLoop, ChannelManager, CronService, HeartbeatService, MessageBus};
    use std::sync::Arc;
    use tokio::signal;

    info!("Starting klyntbot gateway on port {}", port);

    let config = config::load()?;
    info!("Configuration loaded from: {:?}", config::config_path());

    // Initialize LLM provider
    let provider = klyntbot::providers::create_provider(&config)?;
    info!("Provider ready: {}", provider.name());

    // Initialize message bus
    let bus = Arc::new(MessageBus::new(100));
    info!("Message bus initialized");

    // Initialize cron service BEFORE agent loop
    let cron_store_path = config.workspace_path().join(".klyntbot").join("cron.json");
    let cron_service = Arc::new(CronService::new(cron_store_path));
    cron_service.start().await?;
    info!("Cron service started");

    // Initialize heartbeat service
    let workspace_path = config.workspace_path();
    let heartbeat_service = Arc::new(HeartbeatService::new(
        workspace_path,
        bus.inbound_sender(),  // ← Pass bus sender for publishing heartbeat messages
        1800, // 30 minutes
        true,
    ));
    heartbeat_service.start().await;
    info!("Heartbeat service started");

    // Initialize agent loop WITH cron service
    let agent_loop = Arc::new(
        AgentLoop::new_with_cron(
            bus.clone(),
            provider,
            config.clone(),
            Some(cron_service.clone()),  // ← Pass cron service
        )
        .await?,
    );
    info!("Agent loop initialized");

    // Initialize channel manager
    let channel_manager = Arc::new(ChannelManager::new(Arc::new(config.clone()), bus.clone()));

    // Start agent loop in background
    let agent_loop_handle = {
        let agent = agent_loop.clone();
        tokio::spawn(async move {
            if let Err(e) = agent.run().await {
                error!("Agent loop error: {}", e);
            }
        })
    };

    // Start channel manager in background (includes outbound dispatcher)
    let channel_manager_handle = {
        let cm = channel_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = cm.start_all().await {
                error!("Channel manager error: {}", e);
            }
        })
    };

    println!("\nklyntbot gateway running on port {}", port);
    println!("\nActive services:");
    println!("  Agent loop (processing messages)");
    println!("  Cron scheduler");
    println!("  Heartbeat monitor");
    println!("\nChannels:");
    for (name, enabled) in [
        ("Telegram", config.channels.telegram.enabled),
        ("Discord", config.channels.discord.enabled),
        ("WhatsApp", config.channels.whatsapp.enabled),
        ("Slack", config.channels.slack.enabled),
        ("QQ", config.channels.qq.enabled),
        ("Email", config.channels.email.enabled),
    ] {
        if enabled {
            println!("  + {}", name);
        }
    }
    println!("\nPress Ctrl+C to stop");

    // Wait for shutdown signal
    signal::ctrl_c().await?;
    info!("Shutting down gracefully...");

    // Stop all services gracefully (IMPORTANT: call .stop() methods, not .abort())
    agent_loop.stop().await;
    channel_manager.stop_all().await?;
    cron_service.stop().await;
    heartbeat_service.stop().await;

    // Wait for tasks to finish (with timeout)
    let shutdown_timeout = tokio::time::Duration::from_secs(5);
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(agent_loop_handle, channel_manager_handle);
    })
    .await;

    info!("All services stopped");
    println!("\nklyntbot stopped");
    Ok(())
}
```

### Changes Needed in AgentLoop

Add constructor variant that accepts CronService:

```rust
// src/agent/agent_loop.rs

impl AgentLoop {
    /// Create agent loop with optional cron service
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<CronService>>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // ... existing initialization ...

        // Register cron tool if service provided
        if let Some(cron_svc) = cron_service {
            tool_registry.register(CronTool::new_with_service(cron_svc));
        } else {
            tool_registry.register(CronTool::new());
        }

        // ... rest of initialization ...
    }

    /// Existing new() method calls new_with_cron() with None
    pub async fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Result<Self> {
        Self::new_with_cron(bus, provider, config, None).await
    }
}
```

### Changes Needed in HeartbeatService

Modify constructor to accept bus sender:

```rust
// src/heartbeat/service.rs

use tokio::sync::mpsc;
use crate::bus::InboundMessage;

pub struct HeartbeatService {
    workspace_path: PathBuf,
    interval_secs: u64,
    running: Arc<RwLock<bool>>,
    inbound_sender: Option<mpsc::Sender<InboundMessage>>,  // ← Add this
}

impl HeartbeatService {
    pub fn new(
        workspace_path: PathBuf,
        inbound_sender: mpsc::Sender<InboundMessage>,  // ← Add parameter
        interval_secs: u64,
        enabled: bool,
    ) -> Self {
        Self {
            workspace_path,
            interval_secs,
            running: Arc::new(RwLock::new(false)),
            inbound_sender: if enabled { Some(inbound_sender) } else { None },
        }
    }

    pub async fn start(&self) {
        if self.inbound_sender.is_none() {
            info!("Heartbeat service disabled");
            return;
        }

        let mut running = self.running.write().await;
        *running = true;
        drop(running);

        let workspace = self.workspace_path.clone();
        let interval = self.interval_secs;
        let running_ref = self.running.clone();
        let sender = self.inbound_sender.clone().unwrap();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(interval));

            while *running_ref.read().await {
                ticker.tick().await;

                // Read HEARTBEAT.md
                let heartbeat_file = workspace.join("HEARTBEAT.md");
                if let Ok(content) = tokio::fs::read_to_string(&heartbeat_file).await {
                    if !content.trim().is_empty() {
                        // Publish heartbeat message to agent
                        let msg = InboundMessage::new(
                            "system",
                            "heartbeat",
                            "cli:heartbeat",
                            format!("[Heartbeat Check]\n\n{}", content),
                        );

                        if let Err(e) = sender.send(msg).await {
                            error!("Failed to send heartbeat message: {}", e);
                        }
                    }
                }
            }
        });
    }
}
```

---

## Agent Loop Message Processing Pipeline

### Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. InboundMessage arrives from bus                              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                    ┌───────▼────────┐
                    │ Is system msg? │
                    └───────┬────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
         Yes  │                           │ No
              │                           │
    ┌─────────▼──────────┐    ┌─────────▼──────────┐
    │ process_system_msg │    │ process_message    │
    │                    │    │                    │
    │ Parse origin from  │    │ 1. Get/create      │
    │ chat_id            │    │    session         │
    │                    │    │ 2. Add user msg    │
    │ Route back to      │    │ 3. Get history     │
    │ original context   │    │                    │
    └────────────────────┘    └─────────┬──────────┘
                                        │
                              ┌─────────▼──────────┐
                              │ 4. Inject tool     │
                              │    context         │
                              │    (channel,       │
                              │     chat_id)       │
                              └─────────┬──────────┘
                                        │
                              ┌─────────▼──────────┐
                              │ 5. Build messages  │
                              │    (ContextBuilder)│
                              │    - System prompt │
                              │    - History       │
                              │    - Current msg   │
                              └─────────┬──────────┘
                                        │
                              ┌─────────▼──────────┐
                              │ 6. Agent loop      │
                              │    (max iterations)│
                              └─────────┬──────────┘
                                        │
                            ┌───────────▼──────────┐
                            │ Call LLM             │
                            │ provider.chat() or   │
                            │ provider.chat_stream()│
                            └───────────┬──────────┘
                                        │
                            ┌───────────▼──────────┐
                            │ Has tool calls?      │
                            └───────────┬──────────┘
                                        │
                      ┌─────────────────┴─────────────────┐
                      │                                   │
                 Yes  │                                   │ No
                      │                                   │
          ┌───────────▼──────────┐          ┌────────────▼─────────┐
          │ Execute tools        │          │ Final response       │
          │ 1. Add assistant msg │          │ Break loop           │
          │    with tool_calls   │          └──────────────────────┘
          │ 2. For each tool:    │
          │    - Execute         │
          │    - Add tool result │
          │ 3. Continue loop     │
          └──────────────────────┘
                                              ┌──────────────────────┐
                                              │ 7. Save to session   │
                                              │ 8. Publish outbound  │
                                              └──────────────────────┘
```

### Current Implementation Issues

1. **Tool context not injected** - Tools like MessageTool, SpawnTool, CronTool need to know current channel/chat_id
2. **System message handling incomplete** - System messages (subagent announces, heartbeats) need special routing

### Corrected process_message()

```rust
// src/agent/agent_loop.rs

/// Process a single inbound message
async fn process_message(&self, msg: InboundMessage) -> Result<()> {
    // Handle system messages (subagent results, heartbeats)
    if msg.channel == "system" {
        return self.process_system_message(msg).await;
    }

    let preview = if msg.content.len() > 80 {
        format!("{}...", &msg.content[..80])
    } else {
        msg.content.clone()
    };

    info!(
        "Processing message from {}:{}: {}",
        msg.channel, msg.sender_id, preview
    );

    // Get or create session
    let session_key = msg.session_key();
    let mut session_manager = self.session_manager.write().await;
    let session = session_manager.get_or_create(&session_key)?;

    // Add user message to session
    session.add_message("user", &msg.content);

    // Get session history
    let history = session.get_history(50);

    // Drop the write lock
    drop(session_manager);

    // ┌──────────────────────────────────────────────────────────┐
    // │ CRITICAL: Inject tool contexts BEFORE calling LLM        │
    // └──────────────────────────────────────────────────────────┘
    self.inject_tool_contexts(&msg.channel, &msg.chat_id).await;

    // Build messages for LLM
    let context_builder = self.context_builder.read().await;
    let media = if msg.media.is_empty() {
        None
    } else {
        Some(msg.media.clone())
    };
    let messages = context_builder
        .build_messages(history, &msg.content, media, &msg.channel, &msg.chat_id)
        .await;

    drop(context_builder);

    // Agent loop (max iterations)
    let max_iterations = self.config.agents.defaults.max_tool_iterations as usize;
    let mut current_messages = messages;
    let mut final_content = None;

    for iteration in 0..max_iterations {
        debug!("Agent iteration {}/{}", iteration + 1, max_iterations);

        // Get tool definitions
        let tool_registry = self.tool_registry.read().await;
        let tools = tool_registry.get_definitions();
        drop(tool_registry);

        // Call LLM (non-streaming for gateway mode)
        let response = self
            .provider
            .chat(
                &current_messages,
                Some(&tools),
                Some(&self.config.agents.defaults.model),
            )
            .await?;

        // Check for tool calls
        if !response.tool_calls.is_empty() {
            debug!("LLM requested {} tool calls", response.tool_calls.len());

            // Add assistant message with tool calls
            let tool_call_messages: Vec<ToolCallMessage> = response
                .tool_calls
                .iter()
                .map(|tc| ToolCallMessage {
                    id: tc.id.clone(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: tc.name.clone(),
                        arguments: serde_json::to_string(&tc.arguments).unwrap_or_default(),
                    },
                })
                .collect();

            current_messages.push(Message::assistant_with_tools(tool_call_messages));

            // Execute tools
            for tool_call in response.tool_calls {
                debug!("Executing tool: {}", tool_call.name);

                let result = {
                    let registry = self.tool_registry.read().await;
                    registry
                        .execute(&tool_call.name, tool_call.arguments.clone())
                        .await
                };

                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                // Add tool result to messages
                current_messages.push(Message::tool(tool_call.id, tool_call.name, result_str));
            }

            // Continue loop to get next LLM response
            continue;
        }

        // No tool calls - we have the final response
        if let Some(content) = response.content {
            final_content = Some(content);
            break;
        }

        // No content and no tool calls - this shouldn't happen
        warn!("LLM returned no content and no tool calls");
        break;
    }

    // Get final content or use default
    let response_content = final_content.unwrap_or_else(|| {
        "I've completed my work but don't have a specific message to share.".to_string()
    });

    // Save assistant response to session
    {
        let mut session_manager = self.session_manager.write().await;
        if let Ok(session) = session_manager.get_or_create(&session_key) {
            session.add_message("assistant", &response_content);
            // Clone the session to avoid borrowing issues
            let session_clone = session.clone();
            if let Err(e) = session_manager.save(&session_clone) {
                warn!("Failed to save session: {}", e);
            }
        }
    }

    // Send response
    let out_msg = OutboundMessage::new(msg.channel, msg.chat_id, response_content);
    self.bus.publish_outbound(out_msg).await?;

    Ok(())
}
```

---

## Tool Context Injection

### Why Tool Context Injection is Critical

Tools like **MessageTool**, **SpawnTool**, and **CronTool** need to know the **current conversation context** (channel and chat_id) so they can:

1. **MessageTool**: Send responses back to the correct channel
2. **SpawnTool**: Route subagent results back to the original conversation
3. **CronTool**: Schedule jobs that deliver to the correct chat

### Python nanobot's Approach

```python
# nanobot/agent/loop.py (lines 166-176)

# Update tool contexts
message_tool = self.tools.get("message")
if isinstance(message_tool, MessageTool):
    message_tool.set_context(msg.channel, msg.chat_id)

spawn_tool = self.tools.get("spawn")
if isinstance(spawn_tool, SpawnTool):
    spawn_tool.set_context(msg.channel, msg.chat_id)

cron_tool = self.tools.get("cron")
if isinstance(cron_tool, CronTool):
    cron_tool.set_context(msg.channel, msg.chat_id)
```

### Rust Implementation Strategy

**Option 1: Dynamic Context Injection (Recommended)**

Add a `set_context()` method to the Tool trait (with default no-op implementation):

```rust
// src/tools/mod.rs

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value) -> Result<String>;
    fn to_schema(&self) -> Value { /* ... */ }
    fn validate_params(&self, params: &Value) -> Vec<String> { /* ... */ }

    /// Set the current conversation context for tools that need routing info.
    /// Default implementation is a no-op (most tools don't need context).
    fn set_context(&self, _channel: &str, _chat_id: &str) {
        // No-op by default
    }
}
```

**Option 2: Arc<RwLock<Context>> (Alternative)**

Store context in a shared state:

```rust
// src/tools/context.rs

#[derive(Clone, Default)]
pub struct ToolContext {
    pub channel: String,
    pub chat_id: String,
}

// In AgentLoop
pub struct AgentLoop {
    // ...
    tool_context: Arc<RwLock<ToolContext>>,  // Shared context
}

// Tools that need context hold a reference
pub struct MessageTool {
    bus_sender: mpsc::Sender<OutboundMessage>,
    context: Arc<RwLock<ToolContext>>,  // Reference to shared context
}
```

**Recommendation: Use Option 1 (set_context method)**

- Simpler to implement
- No shared mutable state
- Follows Python nanobot's pattern
- Cleaner tool API

### Implementation

```rust
// src/agent/agent_loop.rs

impl AgentLoop {
    /// Inject current conversation context into tools that need routing info
    async fn inject_tool_contexts(&self, channel: &str, chat_id: &str) {
        let registry = self.tool_registry.write().await;

        // Context-aware tools need to be updated
        if let Some(tool) = registry.get("message") {
            tool.set_context(channel, chat_id);
        }

        if let Some(tool) = registry.get("spawn") {
            tool.set_context(channel, chat_id);
        }

        if let Some(tool) = registry.get("cron") {
            tool.set_context(channel, chat_id);
        }
    }
}
```

### MessageTool Implementation

```rust
// src/tools/message.rs

use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use crate::bus::OutboundMessage;

pub struct MessageTool {
    bus_sender: mpsc::Sender<OutboundMessage>,
    context: Arc<RwLock<(String, String)>>,  // (channel, chat_id)
}

impl MessageTool {
    pub fn new(bus_sender: mpsc::Sender<OutboundMessage>) -> Self {
        Self {
            bus_sender,
            context: Arc::new(RwLock::new((String::new(), String::new()))),
        }
    }
}

#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str {
        "message"
    }

    fn description(&self) -> &str {
        "Send a message to the user in the current conversation"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The message content to send"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let content = args["content"].as_str()
            .ok_or_else(|| ToolError::InvalidParams("Missing 'content'".to_string()))?;

        // Get current context
        let (channel, chat_id) = {
            let ctx = self.context.read().unwrap();
            ctx.clone()
        };

        // Send message via bus
        let msg = OutboundMessage::new(channel, chat_id, content);
        self.bus_sender
            .send(msg)
            .await
            .map_err(|_| ToolError::ExecutionFailed("Bus disconnected".to_string()))?;

        Ok("Message sent".to_string())
    }

    /// IMPORTANT: This method is called by AgentLoop before processing
    fn set_context(&self, channel: &str, chat_id: &str) {
        let mut ctx = self.context.write().unwrap();
        *ctx = (channel.to_string(), chat_id.to_string());
    }
}
```

### SpawnTool Implementation

```rust
// src/tools/spawn.rs

use std::sync::{Arc, RwLock};

pub struct SpawnTool {
    manager: Arc<SubagentManager>,
    context: Arc<RwLock<(String, String)>>,  // (channel, chat_id)
}

impl SpawnTool {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SubagentManager::default()),
            context: Arc::new(RwLock::new((String::new(), String::new()))),
        }
    }

    pub fn with_manager(manager: Arc<SubagentManager>) -> Self {
        Self {
            manager,
            context: Arc::new(RwLock::new((String::new(), String::new()))),
        }
    }
}

#[async_trait]
impl Tool for SpawnTool {
    fn name(&self) -> &str {
        "spawn"
    }

    fn description(&self) -> &str {
        "Spawn a background subagent to work on a task independently"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task for the subagent to complete"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Optional agent ID (auto-generated if not provided)"
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let task = args["task"].as_str()
            .ok_or_else(|| ToolError::InvalidParams("Missing 'task'".to_string()))?;

        let agent_id = args["agent_id"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Get current context (this is where results will be routed back)
        let (channel, chat_id) = {
            let ctx = self.context.read().unwrap();
            ctx.clone()
        };

        // Spawn subagent with origin context
        self.manager
            .spawn(agent_id.clone(), task.to_string(), channel, chat_id)
            .await?;

        Ok(format!("Subagent '{}' spawned for task", agent_id))
    }

    /// Set the origin context for routing subagent results
    fn set_context(&self, channel: &str, chat_id: &str) {
        let mut ctx = self.context.write().unwrap();
        *ctx = (channel.to_string(), chat_id.to_string());
    }
}
```

---

## Subagent Integration

### How Subagents Work

1. **Spawn**: SpawnTool creates an isolated AgentLoop with limited tools (no message, spawn, cron)
2. **Execute**: Subagent runs independently in a tokio task
3. **Announce**: When done, subagent publishes a system message with results
4. **Route back**: System message's chat_id contains "channel:chat_id" for routing

### SubagentManager Architecture

```rust
// src/agent/subagent.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub struct SubagentManager {
    provider: DynProvider,
    workspace: PathBuf,
    outbound_sender: mpsc::Sender<OutboundMessage>,
    inbound_sender: mpsc::Sender<InboundMessage>,
    model: String,
    active_subagents: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl SubagentManager {
    pub fn new(
        provider: DynProvider,
        workspace: PathBuf,
        outbound_sender: mpsc::Sender<OutboundMessage>,
        inbound_sender: mpsc::Sender<InboundMessage>,
        model: String,
    ) -> Self {
        Self {
            provider,
            workspace,
            outbound_sender,
            inbound_sender,
            model,
            active_subagents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a subagent to work on a task
    pub async fn spawn(
        &self,
        agent_id: String,
        task: String,
        origin_channel: String,
        origin_chat_id: String,
    ) -> Result<()> {
        info!("Spawning subagent '{}' for task", agent_id);

        let provider = Arc::clone(&self.provider);
        let workspace = self.workspace.clone();
        let inbound_sender = self.inbound_sender.clone();
        let model = self.model.clone();
        let agent_id_clone = agent_id.clone();

        // Create isolated agent loop (no message/spawn/cron tools)
        let handle = tokio::spawn(async move {
            let result = Self::run_subagent(
                agent_id_clone.clone(),
                task.clone(),
                provider,
                workspace,
                model,
                origin_channel.clone(),
                origin_chat_id.clone(),
            )
            .await;

            // Announce result back to main agent
            let announce_content = match result {
                Ok(output) => format!("Subagent '{}' completed:\n\n{}", agent_id_clone, output),
                Err(e) => format!("Subagent '{}' failed: {}", agent_id_clone, e),
            };

            // Publish system message with origin routing
            let system_msg = InboundMessage::new(
                "system",
                agent_id_clone,
                format!("{}:{}", origin_channel, origin_chat_id),  // ← Route back
                announce_content,
            );

            if let Err(e) = inbound_sender.send(system_msg).await {
                error!("Failed to send subagent announce: {}", e);
            }
        });

        // Track the subagent
        let mut subagents = self.active_subagents.write().await;
        subagents.insert(agent_id, handle);

        Ok(())
    }

    /// Run a subagent (isolated agent loop)
    async fn run_subagent(
        agent_id: String,
        task: String,
        provider: DynProvider,
        workspace: PathBuf,
        model: String,
        _origin_channel: String,
        _origin_chat_id: String,
    ) -> Result<String> {
        debug!("Subagent '{}' starting task", agent_id);

        // Create minimal config for subagent
        let config = Config::default(); // Or load from workspace

        // Create a dummy bus (subagents don't use the main bus)
        let dummy_bus = Arc::new(MessageBus::new(10));

        // Create isolated agent loop WITHOUT message/spawn/cron tools
        let mut agent_loop = AgentLoop::new(dummy_bus, provider, config).await?;

        // Remove context-dependent tools
        {
            let mut registry = agent_loop.tool_registry.write().await;
            registry.unregister("message");
            registry.unregister("spawn");
            registry.unregister("cron");
        }

        // Process the task directly
        let result = agent_loop
            .process_direct(task, format!("subagent:{}", agent_id))
            .await?;

        debug!("Subagent '{}' completed task", agent_id);

        Ok(result)
    }
}
```

### System Message Handling

```rust
// src/agent/agent_loop.rs

/// Process system messages (e.g., subagent results, heartbeats)
async fn process_system_message(&self, msg: InboundMessage) -> Result<()> {
    info!("Processing system message from {}", msg.sender_id);

    // Parse origin from chat_id (format: "channel:chat_id")
    let parts: Vec<&str> = msg.chat_id.split(':').collect();
    if parts.len() != 2 {
        warn!("Invalid system message chat_id format: {}", msg.chat_id);
        return Ok(());
    }

    let origin_channel = parts[0];
    let origin_chat_id = parts[1];

    // Add to origin session as system message
    let session_key = format!("{}:{}", origin_channel, origin_chat_id);
    let mut session_manager = self.session_manager.write().await;
    if let Ok(session) = session_manager.get_or_create(&session_key) {
        let system_msg = format!("[System: {}] {}", msg.sender_id, msg.content);
        session.add_message("system", &system_msg);
        let session_clone = session.clone();
        if let Err(e) = session_manager.save(&session_clone) {
            warn!("Failed to save session: {}", e);
        }
    }

    // Optionally: Process the system message with LLM and send response
    // For now, just log and save to session

    Ok(())
}
```

---

## Streaming Architecture

### Provider Trait Extension

Add streaming support with default fallback:

```rust
// src/providers/types.rs

use futures_util::Stream;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamChunk {
    pub content: Option<String>,
    pub tool_call_delta: Option<ToolCallDelta>,
    pub is_final: bool,
    pub finish_reason: Option<String>,
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmResponse>;

    /// Stream chat completion (default: falls back to non-streaming)
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmStream> {
        let response = self.chat(messages, tools, model).await?;
        let chunk = LlmStreamChunk {
            content: response.content,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason),
            reasoning_content: response.reasoning_content,
        };
        Ok(Box::pin(futures_util::stream::once(async move { Ok(chunk) })))
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
}
```

### CLI Streaming Integration

Already implemented in `agent_loop.rs:390-502` - just needs to be tested.

Key points:
- Uses `provider.supports_streaming()` to check capability
- Falls back to non-streaming if not supported
- Accumulates tool calls across stream chunks
- Prints content in real-time with `print!()` and `flush()`

---

## CLI Interactive Mode

### Current Implementation (handle_chat)

The CLI mode in `main.rs:70-166` already works but can be improved:

```rust
// src/main.rs

async fn handle_chat(
    message: Option<String>,
    session: String,
    render_markdown: bool,  // ← Use this
) -> anyhow::Result<()> {
    use klyntbot::{AgentLoop, MessageBus};
    use std::io::{self, Write};
    use std::sync::Arc;

    println!("🐈 klyntbot chat mode");
    println!("Session: {}", session);

    // Load config
    let config = config::load()?;

    // Initialize LLM provider
    let provider = klyntbot::providers::create_provider(&config)?;
    info!("Provider ready: {}", provider.name());

    // Create a minimal message bus (not used in CLI mode, but required for AgentLoop)
    let bus = Arc::new(MessageBus::new(10));

    // Initialize agent loop
    let agent_loop = Arc::new(AgentLoop::new(bus, provider, config).await?);
    info!("Agent loop initialized");

    // Session key for CLI
    let session_key = format!("cli:{}", session);

    // Handle single message or interactive mode
    if let Some(msg) = message {
        // Single message mode
        println!("\nYou: {}", msg);

        match agent_loop.process_direct(msg, session_key).await {
            Ok(response) => {
                println!("\nAgent: {}", response);
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                return Err(e.into());
            }
        }
    } else {
        // Interactive REPL mode
        println!("\nInteractive chat mode. Type 'exit' or 'quit' to end.\n");

        loop {
            // Print prompt
            print!("You: ");
            io::stdout().flush()?;

            // Read user input
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => {
                    // EOF (Ctrl+D)
                    println!("\nGoodbye!");
                    break;
                }
                Ok(_) => {
                    let trimmed = input.trim();

                    // Check for exit commands
                    if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
                        println!("Goodbye!");
                        break;
                    }

                    // Skip empty lines
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Process the message (streaming is handled inside process_direct)
                    match agent_loop
                        .process_direct(trimmed.to_string(), session_key.clone())
                        .await
                    {
                        Ok(response) => {
                            // Response already printed during streaming
                            // Just add a newline for spacing
                            println!();
                        }
                        Err(e) => {
                            eprintln!("\nError: {}\n", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("\nError reading input: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
```

### Improvements Needed

1. **Markdown rendering** - Use `termimad` crate (already in Cargo.toml)
2. **Better prompt** - Use `rustyline` crate (already in Cargo.toml)
3. **Streaming indication** - Show "..." while waiting for first chunk

---

## Error Recovery Strategy

### Error Handling at Each Level

```
┌─────────────────────────────────────────────────────────┐
│ Level 1: main.rs (handle_serve)                        │
│ - Catch all panics in spawned tasks                    │
│ - Log errors but keep other services running           │
│ - Graceful shutdown on Ctrl+C                          │
└─────────────────────────────────────────────────────────┘
                       │
┌─────────────────────▼─────────────────────────────────┐
│ Level 2: AgentLoop.run()                              │
│ - Catch message processing errors                     │
│ - Send error response to user                         │
│ - Continue processing next message                    │
└─────────────────────────────────────────────────────────┘
                       │
┌─────────────────────▼─────────────────────────────────┐
│ Level 3: process_message()                            │
│ - Tool execution errors → return as text to LLM       │
│ - LLM provider errors → retry once, then fail         │
│ - Session save errors → log warning, continue         │
└─────────────────────────────────────────────────────────┘
                       │
┌─────────────────────▼─────────────────────────────────┐
│ Level 4: Tool.execute()                               │
│ - Return Result<String> with descriptive error        │
│ - Never panic                                         │
└─────────────────────────────────────────────────────────┘
```

### Retry Logic for LLM Calls

```rust
// src/agent/agent_loop.rs

async fn call_llm_with_retry(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    model: &str,
) -> Result<LlmResponse> {
    const MAX_RETRIES: usize = 2;
    let mut last_error = None;

    for attempt in 1..=MAX_RETRIES {
        match self.provider.chat(messages, tools, Some(model)).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                warn!("LLM call failed (attempt {}/{}): {}", attempt, MAX_RETRIES, e);
                last_error = Some(e);

                if attempt < MAX_RETRIES {
                    // Wait before retry
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}
```

---

## Implementation Checklist

### Phase 1: Core Wiring (Priority 1)

- [ ] **main.rs: handle_serve() improvements**
  - [ ] Pass `bus.inbound_sender()` to HeartbeatService
  - [ ] Add `new_with_cron()` constructor to AgentLoop
  - [ ] Pass CronService to AgentLoop
  - [ ] Fix shutdown logic (call `.stop()` methods, not `.abort()`)

- [ ] **AgentLoop: Tool context injection**
  - [ ] Add `inject_tool_contexts()` method
  - [ ] Call it before building messages in `process_message()`
  - [ ] Implement `set_context()` for MessageTool, SpawnTool, CronTool

- [ ] **AgentLoop: System message handling**
  - [ ] Improve `process_system_message()` routing logic
  - [ ] Parse origin channel:chat_id correctly
  - [ ] Add system messages to origin session

### Phase 2: Subagent Integration (Priority 2)

- [ ] **SubagentManager implementation**
  - [ ] Implement `spawn()` method
  - [ ] Implement `run_subagent()` with isolated tool registry
  - [ ] Implement announce mechanism (system message)

- [ ] **SpawnTool context injection**
  - [ ] Store origin channel/chat_id
  - [ ] Pass to SubagentManager.spawn()

### Phase 3: Streaming (Priority 3)

- [ ] **Provider trait extension**
  - [ ] Add `LlmStreamChunk` and `ToolCallDelta` types
  - [ ] Add `chat_stream()` method with default fallback
  - [ ] Add `supports_streaming()` method

- [ ] **OpenAiCompatProvider streaming**
  - [ ] Implement SSE parsing
  - [ ] Implement tool call accumulation
  - [ ] Test with Anthropic/OpenAI/DeepSeek

- [ ] **CLI streaming integration**
  - [ ] Already implemented, just needs testing
  - [ ] Add markdown rendering

### Phase 4: Testing & Polish (Priority 4)

- [ ] **Integration tests**
  - [ ] Gateway mode: send message → receive response
  - [ ] Subagent: spawn → announce
  - [ ] Tool context: verify routing works

- [ ] **Error handling**
  - [ ] Add retry logic to LLM calls
  - [ ] Test graceful degradation

- [ ] **Documentation**
  - [ ] Update README with examples
  - [ ] Add architecture diagrams

---

## Success Criteria

✅ **Gateway mode works end-to-end:**
- Message from Telegram → AgentLoop → Tool execution → Response back to Telegram

✅ **Tool routing works correctly:**
- MessageTool sends to correct channel
- SpawnTool routes subagent results back
- CronTool schedules to correct chat

✅ **Subagents work:**
- Spawn background task
- Execute independently
- Announce results to origin

✅ **Streaming works in CLI:**
- Real-time output during generation
- Tool calls accumulate correctly

✅ **Error recovery:**
- LLM failures retry
- Tool errors returned as text
- System keeps running after errors

---

## Conclusion

The klyntbot Rust codebase is **80% complete**. The missing 20% is:

1. **Wiring code** in main.rs and agent_loop.rs (2-3 hours)
2. **Tool context injection** mechanism (2-3 hours)
3. **Subagent integration** (4-6 hours)
4. **Streaming implementation** in OpenAiCompatProvider (4-6 hours)
5. **Testing and polish** (4-8 hours)

**Total estimated time: 16-26 hours of focused work.**

The architecture is sound. The components are well-designed. The integration points are clear. This document provides concrete Rust code for each integration point, making implementation straightforward.

---

**End of Integration Design Document**
