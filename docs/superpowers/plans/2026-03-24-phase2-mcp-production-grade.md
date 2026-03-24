# Phase 2: MCP Production-Grade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring MCP from 2.5 → 5.0 maturity with retry/circuit breaker, auto-reconnection, per-connection sessions, HTTP transport, resources, and progress streaming.

**Architecture:** Two independent streams — **client-side** (Tasks 1-3: retry, circuit breaker, auto-reconnect for consuming external MCP servers) and **server-side** (Tasks 4-8: session isolation, HTTP transport, resources, progress streaming, cleanup for exposing klyntbot to external AI clients). Streams can execute in parallel since they touch non-overlapping files.

**Tech Stack:** Rust, rmcp (MCP protocol library), tokio, DashMap, axum (via rmcp HTTP transport), serde_json

---

## File Structure

| Task | Files to Modify | Files to Create | Test Files |
|------|----------------|-----------------|------------|
| 1. Circuit Breaker | `crates/mcp/src/client/mod.rs` | `crates/mcp/src/client/circuit_breaker.rs` | Inline |
| 2. Retry + CB Integration | `crates/mcp/src/client/tool_adapter.rs`, `crates/mcp/src/client/manager.rs` | — | Inline in `tool_adapter.rs` |
| 3. Auto-Reconnection | `crates/mcp/src/client/manager.rs` | — | Inline |
| 4. Session Isolation | `crates/klyntbot-server/src/bridge/registry.rs`, `crates/klyntbot-server/src/handler.rs` | — | Inline in `registry.rs` |
| 5. HTTP Transport | `crates/klyntbot-server/src/main.rs`, `crates/desktop/src/main.rs`, `crates/config/src/schema/mcp.rs` | — | Manual verification |
| 6. MCP Resources | `crates/klyntbot-server/src/handler.rs` | — | Manual verification |
| 7. Progress Streaming | `crates/klyntbot-server/src/handler.rs`, `crates/klyntbot-server/src/bridge/agent.rs` | — | Manual verification |
| 8. Dead Code Cleanup | `crates/mcp/src/server/handler.rs` (delete), `crates/mcp/src/server/mod.rs` | — | `cargo build` |

### Dependency Graph

```
Stream A (client-side):              Stream B (server-side):
  Task 1 (Circuit Breaker)             Task 4 (Session Isolation)
       ↓                                    ↓
  Task 2 (Retry)                        Task 5 (HTTP Transport)
       ↓                                    ↓
  Task 3 (Auto-Reconnect)              Task 6 (Resources)
                                             ↓
                                        Task 7 (Progress Streaming)
                                             ↓
                                        Task 8 (Dead Code Cleanup)
```

Stream A and Stream B are independent and can run in parallel.

---

## Task 1: MCP Circuit Breaker

**Problem:** No resilience mechanism for external MCP server failures. A crashed server causes every subsequent tool call to fail immediately.

**Approach:** Create a per-server circuit breaker following the existing `InsightForge::CircuitBreaker` pattern (DashMap-based, threshold + cooldown). Add `record_success()` for explicit reset after successful reconnection (unlike InsightForge which only auto-resets on cooldown).

**Files:**
- Create: `crates/mcp/src/client/circuit_breaker.rs`
- Modify: `crates/mcp/src/client/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/mcp/src/client/circuit_breaker.rs` with tests at the bottom:

```rust
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Per-server circuit breaker for MCP connections.
///
/// Tracks failures per server name. Opens the circuit after `threshold`
/// failures within a window, blocking calls for `cooldown` duration.
/// Auto-resets when cooldown expires. Can be manually reset via `record_success()`.
pub struct McpCircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    state: DashMap<String, CircuitState>,
}

struct CircuitState {
    failure_count: u32,
    first_failure: Instant,
}

impl McpCircuitBreaker {
    pub fn new(threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            threshold,
            cooldown: Duration::from_secs(cooldown_secs),
            state: DashMap::new(),
        }
    }

    /// Check if the circuit is open (calls should be blocked).
    /// Returns `false` if cooldown has expired (auto-resets).
    pub fn is_open(&self, server: &str) -> bool {
        if let Some(entry) = self.state.get(server) {
            if entry.failure_count >= self.threshold {
                if entry.first_failure.elapsed() > self.cooldown {
                    // Cooldown expired — auto-reset
                    drop(entry);
                    self.state.remove(server);
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Record a failure. Returns `true` if the circuit just opened.
    pub fn record_failure(&self, server: &str) -> bool {
        let mut entry = self.state.entry(server.to_string()).or_insert(CircuitState {
            failure_count: 0,
            first_failure: Instant::now(),
        });

        // If window expired, reset the counter
        if entry.first_failure.elapsed() > self.cooldown {
            entry.failure_count = 1;
            entry.first_failure = Instant::now();
            return false;
        }

        entry.failure_count += 1;
        entry.failure_count >= self.threshold
    }

    /// Record a success — explicitly reset the circuit for this server.
    pub fn record_success(&self, server: &str) {
        self.state.remove(server);
    }

    /// Check if cooldown has expired for a previously-open circuit.
    /// Returns `true` if the server had an open circuit whose cooldown just expired.
    pub fn cooldown_expired(&self, server: &str) -> bool {
        if let Some(entry) = self.state.get(server) {
            entry.failure_count >= self.threshold && entry.first_failure.elapsed() > self.cooldown
        } else {
            false
        }
    }

    /// Remove stale entries whose cooldown has expired.
    pub fn cleanup(&self) {
        self.state
            .retain(|_, state| state.first_failure.elapsed() <= self.cooldown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = McpCircuitBreaker::new(3, 60);
        assert!(!cb.is_open("test-server"));
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = McpCircuitBreaker::new(3, 60);
        assert!(!cb.record_failure("srv"));
        assert!(!cb.record_failure("srv"));
        assert!(cb.record_failure("srv")); // 3rd failure opens circuit
        assert!(cb.is_open("srv"));
    }

    #[test]
    fn test_circuit_blocks_when_open() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv");
        cb.record_failure("srv");
        assert!(cb.is_open("srv"));
    }

    #[test]
    fn test_circuit_auto_resets_after_cooldown() {
        let cb = McpCircuitBreaker::new(2, 0); // 0s cooldown = instant reset
        cb.record_failure("srv");
        cb.record_failure("srv");
        // Cooldown is 0s, so it should auto-reset immediately
        assert!(!cb.is_open("srv"));
    }

    #[test]
    fn test_record_success_resets_circuit() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv");
        cb.record_failure("srv");
        assert!(cb.is_open("srv"));

        cb.record_success("srv");
        assert!(!cb.is_open("srv"));
    }

    #[test]
    fn test_per_server_isolation() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv-a");
        cb.record_failure("srv-a");
        assert!(cb.is_open("srv-a"));
        assert!(!cb.is_open("srv-b"));
    }

    #[test]
    fn test_cooldown_expired() {
        let cb = McpCircuitBreaker::new(2, 0); // 0s cooldown
        cb.record_failure("srv");
        cb.record_failure("srv");
        assert!(cb.cooldown_expired("srv"));
    }

    #[test]
    fn test_cleanup_removes_stale() {
        let cb = McpCircuitBreaker::new(2, 0); // 0s cooldown
        cb.record_failure("srv");
        cb.record_failure("srv");
        cb.cleanup();
        assert!(!cb.is_open("srv"));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p mcp -E 'test(circuit)'`
Expected: all 8 tests PASS. (These are self-contained — the struct and tests are in the same file.)

- [ ] **Step 3: Add module to `mod.rs`**

In `crates/mcp/src/client/mod.rs`, add:

```rust
pub mod circuit_breaker;
```

- [ ] **Step 4: Run clippy and full mcp tests**

Run: `cargo nextest run -p mcp && cargo clippy -p mcp --all-targets`
Expected: all pass, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/mcp/src/client/circuit_breaker.rs crates/mcp/src/client/mod.rs
git commit -m "feat(mcp): add per-server circuit breaker for MCP connections"
```

---

## Task 2: McpTool Retry + Circuit Breaker Integration

**Problem:** `McpTool::execute()` makes one attempt and converts any error to `ToolError::ExecutionFailed`. No retry, no circuit breaker.

**Approach:** Inject `Arc<McpCircuitBreaker>` into `McpTool` at construction. Add a retry loop (3 attempts with exponential backoff) inside `execute()`. Check circuit before calling. Record failure/success after.

**Files:**
- Modify: `crates/mcp/src/client/tool_adapter.rs`
- Modify: `crates/mcp/src/client/manager.rs` (construction site)

- [ ] **Step 1: Write failing test for retry behavior**

Add to `crates/mcp/src/client/tool_adapter.rs` in a new `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_transient_mcp_error_timeout() {
        let err = KlyntbotError::Timeout("test".into());
        assert!(is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_io() {
        let err = KlyntbotError::Io(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset"));
        assert!(is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_tool_not_found() {
        let err = KlyntbotError::Tool(common::ToolError::NotFound("test".into()));
        assert!(!is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_invalid_params() {
        let err = KlyntbotError::Tool(common::ToolError::InvalidParams("test".into()));
        assert!(!is_transient_mcp_error(&err));
    }

    #[test]
    fn test_circuit_breaker_blocks_execution() {
        let cb = Arc::new(McpCircuitBreaker::new(1, 60));
        cb.record_failure("test-server");
        assert!(cb.is_open("test-server"));
        // A McpTool with this circuit breaker should refuse to execute
    }
}
```

- [ ] **Step 2: Add the `is_transient_mcp_error` function**

Add to `crates/mcp/src/client/tool_adapter.rs` before the `McpTool` struct:

```rust
use crate::client::circuit_breaker::McpCircuitBreaker;

/// Classify whether an MCP error is transient (worth retrying) or permanent.
fn is_transient_mcp_error(err: &KlyntbotError) -> bool {
    matches!(
        err,
        KlyntbotError::Timeout(_)
            | KlyntbotError::Io(_)
            | KlyntbotError::Bus(_)
            | KlyntbotError::BusDisconnected
            | KlyntbotError::Tool(common::ToolError::ExecutionFailed(_))
    )
}
```

- [ ] **Step 3: Add circuit breaker field to McpTool**

In the `McpTool` struct, add a new field:

```rust
pub struct McpTool {
    // ... existing fields ...
    circuit_breaker: Arc<McpCircuitBreaker>,
}
```

Update `McpTool::new()` to accept and store the circuit breaker:

```rust
pub fn new(
    server_name: &str,
    tool_def: &ToolDefinition,
    peer: Arc<Peer<RoleClient>>,
    tool_timeout: Duration,
    circuit_breaker: Arc<McpCircuitBreaker>,
) -> Self {
    Self {
        // ... existing field initialization ...
        circuit_breaker,
    }
}
```

- [ ] **Step 4: Add retry logic to `execute()`**

Replace the current `execute()` implementation with:

```rust
async fn execute(
    &self,
    arguments: serde_json::Value,
    _ctx: &RoutingContext,
) -> common::Result<String> {
    // Check circuit breaker before attempting
    if self.circuit_breaker.is_open(&self.server_name) {
        return Err(KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            format!(
                "MCP server '{}' circuit breaker is open — server appears unavailable",
                self.server_name
            ),
        )));
    }

    let delays = [
        Duration::from_millis(500),
        Duration::from_secs(1),
        Duration::from_secs(2),
    ];
    let mut last_err = None;

    for (attempt, delay) in delays.iter().enumerate() {
        match self.execute_once(&arguments).await {
            Ok(result) => {
                self.circuit_breaker.record_success(&self.server_name);
                return Ok(result);
            }
            Err(e) if is_transient_mcp_error(&e) => {
                tracing::warn!(
                    "MCP tool '{}' on '{}' attempt {}/3 failed: {}",
                    self.namespaced_name,
                    self.server_name,
                    attempt + 1,
                    e
                );
                let just_opened = self.circuit_breaker.record_failure(&self.server_name);
                if just_opened {
                    tracing::warn!(
                        "Circuit breaker opened for MCP server '{}'",
                        self.server_name
                    );
                    return Err(e);
                }
                last_err = Some(e);
                if attempt < delays.len() - 1 {
                    tokio::time::sleep(*delay).await;
                }
            }
            Err(e) => {
                // Non-transient error — don't retry
                return Err(e);
            }
        }
    }

    // All retries exhausted
    self.circuit_breaker.record_failure(&self.server_name);
    Err(last_err.unwrap())
}
```

- [ ] **Step 5: Extract current execute body into `execute_once()`**

Move the current body of `execute()` into a private `execute_once()` method:

```rust
async fn execute_once(&self, arguments: &serde_json::Value) -> common::Result<String> {
    let params = CallToolRequestParams {
        name: self.original_name.clone(),
        arguments: Some(arguments.as_object().cloned().unwrap_or_default()),
        meta: None,
    };

    let result = tokio::time::timeout(self.tool_timeout, self.peer.call_tool(params))
        .await
        .map_err(|_| {
            KlyntbotError::Timeout(format!(
                "MCP tool '{}' timed out after {}s",
                self.namespaced_name,
                self.tool_timeout.as_secs()
            ))
        })?
        .map_err(|e| {
            KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "MCP tool '{}' failed: {}",
                self.namespaced_name, e
            )))
        })?;

    if result.is_error.unwrap_or(false) {
        return Err(KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            format!("MCP tool '{}' returned error", self.namespaced_name),
        )));
    }

    // Use the existing codebase pattern for extracting text from MCP content
    let text_parts: Vec<&str> = result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.as_ref()))
        .collect();

    Ok(text_parts.join("\n"))
}
```

- [ ] **Step 6: Update `McpManager::connect_one_inner()` to pass circuit breaker**

In `crates/mcp/src/client/manager.rs`, add a `circuit_breaker: Arc<McpCircuitBreaker>` field to `McpManager`:

```rust
pub struct McpManager {
    connections: HashMap<String, McpConnection>,
    config: McpConfig,
    event_tx: Option<mpsc::Sender<McpStartupEvent>>,
    circuit_breaker: Arc<McpCircuitBreaker>,
}
```

Initialize in `connect_all()`:

```rust
pub async fn connect_all(config: &McpConfig, event_tx: Option<mpsc::Sender<McpStartupEvent>>) -> Self {
    let circuit_breaker = Arc::new(McpCircuitBreaker::new(3, 60));
    // ... existing logic ...
    Self {
        connections,
        config: config.clone(),
        event_tx,
        circuit_breaker,
    }
}
```

In `connect_one_inner()`, pass the circuit breaker to each `McpTool::new()`. Since `connect_one_inner` is an **associated function (no `&self`)**, add `circuit_breaker: &Arc<McpCircuitBreaker>` as a new parameter to both `connect_one` and `connect_one_inner`:

```rust
// In connect_one_inner signature, add parameter:
async fn connect_one_inner(
    server_def: &McpServerDef,
    tool_timeout: Duration,
    circuit_breaker: &Arc<McpCircuitBreaker>,  // NEW
) -> Result<McpConnection> {
    // ... existing logic ...
    // Change McpTool construction:
    McpTool::new(&server_def.name, &td, peer.clone(), tool_timeout, Arc::clone(circuit_breaker))
}

// In connect_one, pass it through:
async fn connect_one(
    server_def: &McpServerDef,
    circuit_breaker: &Arc<McpCircuitBreaker>,  // NEW
) -> Result<McpConnection> {
    let tool_timeout = Duration::from_secs(server_def.tool_timeout_sec);
    tokio::time::timeout(
        Duration::from_secs(server_def.startup_timeout_sec),
        Self::connect_one_inner(server_def, tool_timeout, circuit_breaker),
    ).await??
}

// In connect_all's JoinSet loop:
let cb = Arc::clone(&circuit_breaker);
set.spawn(async move {
    Self::connect_one(&def, &cb).await
});
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p mcp`
Expected: all tests pass including the new ones.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -p mcp --all-targets && cargo clippy -p agent --all-targets`
Expected: 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/mcp/src/client/tool_adapter.rs crates/mcp/src/client/manager.rs
git commit -m "feat(mcp): add retry with exponential backoff and circuit breaker to McpTool"
```

---

## Task 3: Auto-Reconnection Background Loop

**Prerequisite:** Task 2 must be completed first — this task depends on the `config` and `circuit_breaker` fields added to `McpManager` in Task 2.

**Problem:** When a circuit breaker's cooldown expires, subsequent calls fail again because the underlying transport (stdio/HTTP) is still dead. Need to reconnect the transport.

**Approach:** Add a background task in `McpManager` that periodically checks cooled-down circuits and attempts reconnection. Uses the existing `reconnect_server()` method. Clean separation — McpTool doesn't need a reference to McpManager.

**Files:**
- Modify: `crates/mcp/src/client/manager.rs`

- [ ] **Step 1: Add `start_health_check` method**

Add to `impl McpManager`:

```rust
/// Start a background task that periodically checks for downed MCP servers
/// and attempts reconnection when the circuit breaker cooldown expires.
pub fn start_health_check(
    manager: Arc<tokio::sync::Mutex<McpManager>>,
    registry: Arc<tokio::sync::RwLock<tools_core::ToolRegistry>>,
    cancel: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::debug!("MCP health check task cancelled");
                    break;
                }
                _ = interval.tick() => {
                    let mgr = manager.lock().await;
                    let cb = Arc::clone(&mgr.circuit_breaker);
                    // Collect servers that need reconnection
                    let servers_to_reconnect: Vec<String> = mgr
                        .config
                        .servers
                        .iter()
                        .filter(|s| s.enabled && cb.cooldown_expired(&s.name))
                        .map(|s| s.name.clone())
                        .collect();
                    drop(mgr);

                    for server_name in servers_to_reconnect {
                        tracing::info!("Attempting auto-reconnect for MCP server '{}'", server_name);
                        let mut mgr = manager.lock().await;
                        // Find the server def
                        let server_def = mgr.config.servers.iter().find(|s| s.name == server_name).cloned();
                        if let Some(def) = server_def {
                            // reconnect_server returns Vec<Arc<McpTool>>, not Result.
                            // Empty vec means reconnection failed.
                            let new_tools = mgr.reconnect_server(&def).await;
                            if !new_tools.is_empty() {
                                let mut reg = registry.write().await;
                                for tool in new_tools {
                                    reg.register_dyn(tool as Arc<dyn tools_core::Tool>);
                                }
                                mgr.circuit_breaker.record_success(&server_name);
                                tracing::info!("Auto-reconnected MCP server '{}'", server_name);
                            } else {
                                tracing::warn!(
                                    "Auto-reconnect failed for MCP server '{}' (no tools returned)",
                                    server_name
                                );
                            }
                        }
                    }

                    // Cleanup stale circuit breaker entries
                    let mgr = manager.lock().await;
                    mgr.circuit_breaker.cleanup();
                }
            }
        }
    })
}
```

- [ ] **Step 2: Add `tools_for_server` helper**

Add to `impl McpManager`:

```rust
/// Get all tools for a specific server (for re-registration after reconnect).
pub fn tools_for_server(&self, server_name: &str) -> Vec<Arc<dyn tools_core::Tool>> {
    self.connections
        .get(server_name)
        .map(|conn| {
            conn.tools
                .iter()
                .map(|t| t.clone() as Arc<dyn tools_core::Tool>)
                .collect()
        })
        .unwrap_or_default()
}
```

- [ ] **Step 3: Expose `circuit_breaker` for external use**

Add a getter:

```rust
pub fn circuit_breaker(&self) -> &Arc<McpCircuitBreaker> {
    &self.circuit_breaker
}
```

- [ ] **Step 4: Wire into the agent loop builder**

In `crates/agent/src/agent_loop/builder.rs`, after the `McpManager` is created and tools are registered, start the health check task. Find where `mcp_manager` is used (around L1188) and add:

```rust
// After MCP tools are registered, start health check
if let Some(ref mcp_mgr) = mcp_manager {
    let mgr_clone = Arc::clone(mcp_mgr);
    let reg_clone = Arc::clone(&tool_registry);
    let cancel = cancel_token.clone(); // Use the agent's cancellation token
    tokio::spawn(async move {
        McpManager::start_health_check(mgr_clone, reg_clone, cancel).await;
    });
}
```

Note: Check how `mcp_manager` is stored in the builder — it may be `Arc<tokio::sync::Mutex<McpManager>>` or similar. Adjust the wiring to match the existing pattern.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p mcp && cargo nextest run -p agent`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/src/client/manager.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(mcp): add background auto-reconnection for failed MCP servers"
```

---

## Task 4: Per-Connection Session Isolation

**Problem:** All non-agent MCP tool calls share `chat_id: "mcp-session"`. Multiple concurrent clients will interfere with each other.

**Approach:** Generate a UUID per-connection in the handler's `initialize()` method. Use `format!("mcp:{}", session_id)` as the `chat_id` for all tool calls from that connection.

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`
- Modify: `crates/klyntbot-server/src/bridge/registry.rs`

- [ ] **Step 1: Write failing test for session isolation**

Add to `crates/klyntbot-server/src/bridge/registry.rs` in a `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mcp_routing_context_uses_session_id() {
        let ctx = build_mcp_routing_context("test-session-123");
        assert_eq!(ctx.chat_id.as_str(), "mcp:test-session-123");
        assert_eq!(ctx.channel.as_str(), common::MCP_CHANNEL);
        assert!(ctx.is_direct_mode);
    }

    #[test]
    fn test_build_mcp_routing_context_different_sessions() {
        let ctx_a = build_mcp_routing_context("session-a");
        let ctx_b = build_mcp_routing_context("session-b");
        assert_ne!(ctx_a.chat_id.as_str(), ctx_b.chat_id.as_str());
    }
}
```

- [ ] **Step 2: Extract context construction into a function**

In `crates/klyntbot-server/src/bridge/registry.rs`, extract the inline `RoutingContext` construction (L89-L100) into a standalone function:

```rust
/// Build a RoutingContext for an MCP session.
fn build_mcp_routing_context(session_id: &str) -> RoutingContext {
    RoutingContext {
        channel: common::ChannelName::new(common::MCP_CHANNEL),
        chat_id: common::ChatId::new(format!("mcp:{}", session_id)),
        interaction_tx: None,
        is_direct_mode: true,
        delegation_depth: 0,
        entity_tx: None,
        interaction_channel: None,
        squad_id: None,
        squad_mode: None,
        champion_params: None,
    }
}
```

- [ ] **Step 3: Update `execute()` to accept session_id**

Change the `execute` method signature to accept a session ID:

```rust
pub async fn execute(
    &self,
    tool_name: &str,
    arguments: serde_json::Value,
    session_id: &str,
) -> Result<CallToolResult, McpError> {
    // ... whitelist check (unchanged) ...
    let ctx = build_mcp_routing_context(session_id);
    // ... rest unchanged ...
}
```

- [ ] **Step 4: Store session ID in the handler**

In `crates/klyntbot-server/src/handler.rs`, add a `session_id` field to `KlyntbotServerHandler`:

```rust
pub struct KlyntbotServerHandler {
    app: Arc<AppCore>,
    bridge: ToolRegistryBridge,
    agent_bridge: AgentBridge,
    status_tool: Tool,
    agent_tool: Option<Tool>,
    session_id: String,  // NEW: per-connection session identifier
}
```

Initialize with a UUID in `new()`:

```rust
pub fn new(app: Arc<AppCore>, whitelist: Vec<String>) -> Self {
    Self {
        // ... existing ...
        session_id: uuid::Uuid::new_v4().to_string(),
    }
}
```

- [ ] **Step 5: Pass session_id through call_tool**

In the handler's `call_tool` method, pass `self.session_id` to `bridge.execute()`:

```rust
// Change:
self.bridge.execute(&name, arguments).await
// To:
self.bridge.execute(&name, arguments, &self.session_id).await
```

Also update the `agent_bridge.execute()` call to use the session_id as the session key prefix.

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p klyntbot-server`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/klyntbot-server/src/bridge/registry.rs crates/klyntbot-server/src/handler.rs
git commit -m "feat(mcp): per-connection session isolation via UUID chat_id"
```

---

## Task 5: HTTP Transport

**Problem:** HTTP transport is a hard stub in both the standalone binary and the desktop embedded server. The `transport-streamable-http-server` rmcp feature is in `Cargo.toml` but not wired.

**Approach:** Wire `rmcp`'s streamable HTTP server transport in both the standalone binary and the desktop app. Add auth enforcement using the existing `McpAuthConfig` (currently defined but unused).

**Files:**
- Modify: `crates/klyntbot-server/src/main.rs:L65-L71`
- Modify: `crates/desktop/src/main.rs:L197-L201`

**Important:** Before implementing, verify the exact rmcp API for HTTP transport:

```bash
# Check what rmcp's streamable HTTP server exports
cargo doc -p rmcp --no-deps --open
# Or grep the rmcp source:
find ~/.cargo/registry/src -path '*/rmcp*/transport/streamable_http_server*' -name '*.rs' | head -5
```

- [ ] **Step 1: Verify rmcp HTTP server API**

Read the rmcp crate's `transport/streamable_http_server` module to understand:
- What types does it export?
- How do you create and serve an HTTP transport?
- Does it provide an axum router/handler?
- What's the session management model?

The rmcp crate with `transport-streamable-http-server` feature typically provides `StreamableHttpService` or similar. Check the exact API before proceeding.

- [ ] **Step 2: Implement standalone HTTP mode**

In `crates/klyntbot-server/src/main.rs`, replace the TODO stub (L69-L71) with the rmcp HTTP wiring. The pattern will be similar to:

```rust
// Replace the TODO at L69-71 with actual HTTP server:
if cli_args.http {
    let host = config.mcp.server.host.clone();
    let port = cli_args.port.unwrap_or(config.mcp.server.port);
    let addr = format!("{}:{}", cli_args.host.as_deref().unwrap_or(&host), port);

    info!("Starting MCP HTTP server on {}", addr);

    // Wire rmcp HTTP transport (exact API depends on rmcp version)
    // The typical pattern uses StreamableHttpService or axum integration
    // provided by the rmcp crate.

    // If rmcp provides an axum-compatible service:
    // let app = rmcp::transport::streamable_http_server::handler(move || {
    //     KlyntbotServerHandler::new(app.clone(), whitelist.clone())
    // });
    // axum::serve(listener, app).await?;

    // If rmcp provides a lower-level transport:
    // let transport = rmcp::transport::streamable_http_server::...;
    // handler.serve(transport).await;
}
```

Note: The exact wiring depends on the rmcp API discovered in Step 1. The implementer MUST read the rmcp docs/source first. Do not guess the API.

- [ ] **Step 3: Implement embedded HTTP mode in desktop**

In `crates/desktop/src/main.rs`, replace the TODO stub (L197-L201) with the same pattern:

```rust
// Replace the warning log at L197-L201 with actual HTTP server:
// Use the same rmcp HTTP transport wiring as the standalone binary.
// Bind to 127.0.0.1:{config.mcp.server.port}.
```

- [ ] **Step 4: Add bearer token auth (if HTTP enabled)**

If `config.mcp.server.auth.enabled` is true and a token is configured, enforce auth. This could be done as axum middleware or within the handler. The `McpAuthConfig` is at `config.mcp.server.auth` with `enabled: bool` and `token: Option<Secret<String>>`.

- [ ] **Step 5: Verify end-to-end**

Start the standalone binary in HTTP mode and test with curl:
```bash
cargo build -p klyntbot-mcp
./target/debug/klyntbot-mcp serve --http --port 3100

# In another terminal:
curl -X POST http://localhost:3100 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

- [ ] **Step 6: Commit**

```bash
git add crates/klyntbot-server/src/main.rs crates/desktop/src/main.rs
git commit -m "feat(mcp): wire HTTP transport for standalone and embedded MCP servers"
```

---

## Task 6: MCP Resources

**Problem:** The MCP server only exposes tools. No resources are available for richer client integration.

**Approach:** Add `resources` capability to the handler and expose key agent state as MCP resources.

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`

- [ ] **Step 1: Add resources capability**

In the handler's `initialize` method (or `ServerHandler` impl), declare resource support:

```rust
fn capabilities(&self) -> ServerCapabilities {
    ServerCapabilities {
        tools: Some(ToolsCapability::default()),
        resources: Some(ResourcesCapability {
            subscribe: Some(false),
            list_changed: Some(false),
        }),
        ..Default::default()
    }
}
```

- [ ] **Step 2: Implement `list_resources`**

**Important:** The rmcp `ServerHandler` trait's `list_resources` method signature is:
```rust
fn list_resources(
    &self,
    request: Option<PaginatedRequestParams>,
    context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<ListResourcesResult, McpError>>
```

Implement accordingly (the `request` and `context` params can be ignored for now):

```rust
fn list_resources(
    &self,
    _request: Option<PaginatedRequestParams>,
    _context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
    async {
        Ok(ListResourcesResult {
            resources: vec![
                Resource {
                    uri: "klyntbot://status".into(),
                    name: "Agent Status".into(),
                    description: Some("Agent health, uptime, and session info".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                },
                Resource {
                    uri: "klyntbot://memory/recent".into(),
                    name: "Recent Memories".into(),
                    description: Some("Recently extracted semantic facts".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                },
                Resource {
                    uri: "klyntbot://tasks/today".into(),
                    name: "Today's Tasks".into(),
                    description: Some("Tasks and deadlines due today".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                },
                Resource {
                    uri: "klyntbot://config/skills".into(),
                    name: "Active Skills".into(),
                    description: Some("Currently loaded skill list".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                },
            ],
            next_cursor: None,
        })
    }
}
```

- [ ] **Step 3: Implement `read_resource`**

**Important:** The rmcp `ServerHandler` trait's `read_resource` method signature is:
```rust
fn read_resource(
    &self,
    request: ReadResourceRequestParams,
    context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<ReadResourceResult, McpError>>
```

The URI is in `request.uri`. Implement accordingly:

```rust
fn read_resource(
    &self,
    request: ReadResourceRequestParams,
    _context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
    async move {
        let uri = request.uri.as_str();
        let content = match uri {
            "klyntbot://status" => {
                // Delegate to the existing get_status tool
                let args = serde_json::json!({});
                self.call_tool_for_resource("get_status", args).await
            }
            "klyntbot://memory/recent" => {
                // Fetch recent semantic facts via the memory tool
                let args = serde_json::json!({"action": "list", "limit": 20});
                self.call_tool_for_resource("memory", args).await
            }
            "klyntbot://tasks/today" => {
                let args = serde_json::json!({"action": "list", "filter": "today"});
                self.call_tool_for_resource("tasks", args).await
            }
            "klyntbot://config/skills" => {
                // Return skill list — this could query the skill catalog
                "Skill list not yet available via resource".into()
            }
            _ => return Err(McpError::resource_not_found(format!("Unknown resource: {}", uri))),
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: request.uri,
                mime_type: Some("application/json".into()),
                text: content,
            }],
        })
    }
}
```

Add a helper method to `KlyntbotServerHandler`:

```rust
/// Call an internal tool and return its text result for resource reads.
async fn call_tool_for_resource(&self, tool_name: &str, args: serde_json::Value) -> String {
    match self.bridge.execute(tool_name, args, &self.session_id).await {
        Ok(result) => result
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("Error fetching {}: {}", tool_name, e),
    }
}
```

Note: Verify the exact rmcp types (`ReadResourceRequestParams`, `ReadResourceResult`, `ResourceContents`) against the rmcp crate. The variant name may be `ResourceContents::Text` or `ResourceContents::TextResourceContents` — check rmcp source.

- [ ] **Step 4: Run tests**

Run: `cargo build -p klyntbot-server && cargo nextest run -p klyntbot-server`
Expected: compiles and all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klyntbot-server/src/handler.rs
git commit -m "feat(mcp): expose agent status and tasks as MCP resources"
```

---

## Task 7: Progress Streaming via MCP Notifications

**Problem:** The `agent` tool collects all streaming events into a single response. External AI clients (Claude Code, Cursor) get no progress feedback during long-running agent calls.

**Approach:** Store the MCP `Peer` handle during `initialize()`. During `agent` tool execution, emit `notifications/progress` for each `ContentChunk` event. The MCP spec's progress notification mechanism allows this without changing the request-response pattern.

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`
- Modify: `crates/klyntbot-server/src/bridge/agent.rs`

- [ ] **Step 1: Store the Peer in the handler**

In `KlyntbotServerHandler`, add a field for the peer:

```rust
pub struct KlyntbotServerHandler {
    // ... existing fields ...
    peer: Option<Arc<rmcp::Peer<rmcp::RoleServer>>>,
}
```

Initialize as `None` in `new()`. Set it in the `initialize` method:

```rust
fn initialize(
    &mut self,
    _capabilities: ClientCapabilities,
    _client_info: Implementation,
    context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
    // Store the peer for progress notifications
    self.peer = Some(context.peer().clone());

    async move {
        Ok(InitializeResult { /* ... existing ... */ })
    }
}
```

Note: Verify the exact rmcp `initialize` signature. `context.peer()` should return a reference to the `Peer` that supports `send_notification()`.

- [ ] **Step 2: Update AgentBridge to accept a notification callback**

In `crates/klyntbot-server/src/bridge/agent.rs`, modify `handle_chat` to accept an optional progress emitter:

```rust
pub async fn execute(
    &self,
    params: serde_json::Value,
    progress_peer: Option<Arc<rmcp::Peer<rmcp::RoleServer>>>,
    progress_token: Option<serde_json::Value>,
) -> Result<CallToolResult, McpError> {
    // ... parse action ...
    self.handle_chat(message, session_key, progress_peer, progress_token).await
}
```

In `collect_agent_stream`, if a peer and token are provided, emit progress:

```rust
async fn collect_agent_stream(
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<InteractionBundle>,
    progress_peer: Option<Arc<rmcp::Peer<rmcp::RoleServer>>>,
    progress_token: Option<serde_json::Value>,
) -> (String, Vec<String>) {
    let mut chunks_sent: u64 = 0;

    loop {
        tokio::select! { biased;
            event = event_rx.recv() => {
                match event {
                    Some(AgentEvent::ContentChunk(text)) => {
                        // Emit progress notification if peer available
                        if let (Some(ref peer), Some(ref token)) = (&progress_peer, &progress_token) {
                            chunks_sent += 1;
                            let _ = peer.send_notification(
                                "notifications/progress",
                                serde_json::json!({
                                    "progressToken": token,
                                    "progress": chunks_sent,
                                    "message": text,
                                }),
                            ).await;
                        }
                        content.push_str(&text);
                    }
                    // ... rest unchanged ...
                }
            }
            // ... interaction handling unchanged ...
        }
    }
}
```

- [ ] **Step 3: Wire peer through call_tool**

In the handler's `call_tool`, pass the peer when calling the agent bridge:

```rust
// When dispatching to agent_bridge:
let progress_token = request.params.meta
    .as_ref()
    .and_then(|m| m.get("progressToken").cloned());

self.agent_bridge.execute(
    arguments,
    self.peer.clone(),
    progress_token,
).await
```

Note: The `meta` field on `CallToolRequestParams` may have a different structure in rmcp. Verify the exact way to extract `progressToken` from the request.

- [ ] **Step 4: Run tests**

Run: `cargo build -p klyntbot-server && cargo nextest run -p klyntbot-server`
Expected: compiles and passes.

- [ ] **Step 5: Commit**

```bash
git add crates/klyntbot-server/src/handler.rs crates/klyntbot-server/src/bridge/agent.rs
git commit -m "feat(mcp): emit progress notifications during agent tool execution"
```

---

## Task 8: Dead Code Cleanup

**Problem:** `crates/mcp/src/server/handler.rs` is an old/dead handler that was superseded by `crates/klyntbot-server/src/handler.rs`. It has no AppCore integration and is never used by the live binary.

**Approach:** Delete the dead handler file and update the module declaration.

**Files:**
- Delete: `crates/mcp/src/server/handler.rs`
- Modify: `crates/mcp/src/server/mod.rs`

- [ ] **Step 1: Verify the dead code is truly unused**

```bash
# Search for any imports or uses of the old handler
grep -r "mcp::server::handler\|mcp::server::KlyntbotServerHandler\|mcp::server::McpServerRunner" crates/ --include='*.rs' | grep -v 'crates/mcp/src/server/'
```

Expected: no matches outside the mcp server module itself.

- [ ] **Step 2: Delete the dead handler**

```bash
rm crates/mcp/src/server/handler.rs
```

- [ ] **Step 3: Update `crates/mcp/src/server/mod.rs`**

Remove the `pub mod handler;` line. If `mod.rs` only had that line, either delete it too or keep it with any remaining re-exports.

- [ ] **Step 4: Verify compilation**

Run: `cargo build --workspace`
Expected: successful compilation. If anything depended on the deleted module, fix the import.

- [ ] **Step 5: Commit**

```bash
git add -A crates/mcp/src/server/
git commit -m "chore(mcp): remove dead server handler superseded by klyntbot-server"
```

---

## Verification

After all 7 tasks are complete:

- [ ] **Run the full workspace test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

- [ ] **Run clippy and fmt**

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

- [ ] **End-to-end verification**

1. **Stdio MCP (circuit breaker test):** Start the MCP server via stdio, call a tool on a disconnected external MCP server, verify the circuit breaker opens after 3 failures and returns a descriptive error.

2. **HTTP MCP (if implemented):** Start in HTTP mode, verify `tools/list` and `resources/list` return valid responses.

3. **Session isolation:** Connect two MCP clients, verify they get different `chat_id`s in tool execution.

---

## Summary of Changes

| Task | Crate | What Changes | Risk |
|------|-------|-------------|------|
| 1. Circuit Breaker | `mcp` | New per-server circuit breaker | Low — additive, fully tested |
| 2. Retry + CB | `mcp` | Retry loop + CB check in McpTool::execute() | Medium — changes tool execution flow |
| 3. Auto-Reconnect | `mcp`, `agent` | Background health check loop | Medium — new background task |
| 4. Session Isolation | `klyntbot-server` | Per-connection UUID chat_id | Low — single field change |
| 5. HTTP Transport | `klyntbot-server`, `desktop` | Wire rmcp HTTP transport | High — depends on rmcp API |
| 6. MCP Resources | `klyntbot-server` | Expose status/tasks/memory as resources | Medium — new handler methods |
| 7. Progress Streaming | `klyntbot-server` | MCP progress notifications during agent calls | Medium — peer handle plumbing |
| 8. Dead Code | `mcp` | Delete unused handler | Low — deletion only |
