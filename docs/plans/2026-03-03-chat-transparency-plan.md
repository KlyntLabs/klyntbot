# Chat Transparency UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add per-message transparency display to /chat showing token usage, tool calls, memory accesses, skills, execution steps, cache usage, agents, and learning — toggled via a header button.

**Architecture:** Enrich existing `AgentEvent` stream with 5 new variants (UsageReport, MemoryAccess, SkillLoaded, LearningEvent, SubagentSpawned). Wire dropped events through the relay. Accumulate transparency data per-request in `chat.rs`, persist to `SessionMessage.metadata.transparency`. Frontend accumulates incrementally in `useAgentStream`, renders via 6 new components.

**Tech Stack:** Rust (agent events, desktop-shared payloads, chat relay), TypeScript/React (hooks, components), Tailwind v4 tokens, lucide-react icons.

**Design doc:** `docs/plans/2026-03-03-chat-transparency-ui-design.md`

---

### Task 1: Add New AgentEvent Variants

**Files:**
- Modify: `crates/agent/src/events.rs:L1-L70`

**Step 1: Add 5 new variants to AgentEvent enum**

In `crates/agent/src/events.rs`, add these variants after `EntityCreated` (line 69):

```rust
    /// Token usage report after cost tracking.
    UsageReport {
        #[serde(rename = "promptTokens")]
        prompt_tokens: u32,
        #[serde(rename = "completionTokens")]
        completion_tokens: u32,
        #[serde(rename = "cacheReadTokens")]
        cache_read_tokens: u32,
        #[serde(rename = "cacheWriteTokens")]
        cache_write_tokens: u32,
        #[serde(rename = "estimatedCostUsd")]
        estimated_cost_usd: f64,
        model: String,
        #[serde(rename = "responseTimeMs")]
        response_time_ms: u64,
    },

    /// A memory search or operation was performed.
    MemoryAccess {
        action: String,
        query: Option<String>,
        #[serde(rename = "resultsCount")]
        results_count: u32,
    },

    /// A skill was loaded into the system prompt.
    SkillLoaded {
        name: String,
        trigger: String,
    },

    /// A learning event occurred (threshold adjustment, pattern detection).
    LearningEvent {
        #[serde(rename = "eventType")]
        event_type: String,
        detail: String,
    },

    /// A subagent was spawned.
    SubagentSpawned {
        label: String,
        profile: String,
    },
```

**Step 2: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -20`
Expected: Success (new variants are additive, no existing match arms break due to `_ => {}` catch-all)

**Step 3: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add transparency AgentEvent variants

UsageReport, MemoryAccess, SkillLoaded, LearningEvent, SubagentSpawned"
```

---

### Task 2: Add Tauri Event Payloads

**Files:**
- Modify: `crates/desktop-shared/src/events.rs:L1-L113`

**Step 1: Add event name constants**

After `AGENT_EXECUTION_STARTED` (line 32), add:

```rust
pub const AGENT_ITERATION_START: &str = "agent:iteration_start";
pub const AGENT_CONFIDENCE_ASSESSED: &str = "agent:confidence_assessed";
pub const AGENT_USAGE_REPORT: &str = "agent:usage_report";
pub const AGENT_MEMORY_ACCESS: &str = "agent:memory_access";
pub const AGENT_SKILL_LOADED: &str = "agent:skill_loaded";
pub const AGENT_LEARNING_EVENT: &str = "agent:learning_event";
pub const AGENT_SUBAGENT_SPAWNED: &str = "agent:subagent_spawned";
```

**Step 2: Add payload structs**

After `InteractionRequestPayload` (line 112), add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IterationStartPayload {
    pub session_key: String,
    pub iteration: usize,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceAssessedPayload {
    pub session_key: String,
    pub score: f32,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReportPayload {
    pub session_key: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub estimated_cost_usd: f64,
    pub model: String,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAccessPayload {
    pub session_key: String,
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadedPayload {
    pub session_key: String,
    pub name: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEventPayload {
    pub session_key: String,
    pub event_type: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSpawnedPayload {
    pub session_key: String,
    pub label: String,
    pub profile: String,
}

/// Accumulated transparency data for an assistant message.
/// Serialized into `SessionMessage.metadata.transparency`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TransparencyUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<TransparencyCost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TransparencyTiming>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<TransparencyTool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memory_accesses: Vec<TransparencyMemoryAccess>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<TransparencySkill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<TransparencyExecution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<TransparencyClassification>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<TransparencySubagent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub learning: Vec<TransparencyLearning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyCost {
    pub estimated_usd: f64,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTiming {
    pub total_ms: u64,
    pub classification_ms: Option<u64>,
    pub context_assembly_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTool {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyMemoryAccess {
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySkill {
    pub name: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyExecution {
    pub engine: String,
    pub iterations: u32,
    pub max_iterations: u32,
    pub escalations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyClassification {
    pub strategy: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySubagent {
    pub label: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyLearning {
    pub event_type: String,
    pub detail: String,
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared 2>&1 | head -20`
Expected: Success

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/events.rs
git commit -m "feat(desktop-shared): add transparency event payloads and data types"
```

---

### Task 3: Emit UsageReport from Pipeline

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:L294-L324`
- Modify: `crates/agent/src/output/cost_tracker.rs:L230-L260`

**Step 1: Make `estimate_cost` public**

In `crates/agent/src/output/cost_tracker.rs:L214`, change:

```rust
fn estimate_cost(usage: &Usage, model: &str) -> f64 {
```

to:

```rust
pub fn estimate_cost(usage: &Usage, model: &str) -> f64 {
```

**Step 2: Update `record_usage` to emit UsageReport**

In `crates/agent/src/intent_pipeline/pipeline.rs`, replace the `record_usage` method (lines 310-324) with:

```rust
    async fn record_usage(
        &self,
        result: &RouterResult,
        mode_name: &str,
        ctx: &RoutingContext,
        event_tx: &Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        pipeline_elapsed_ms: u64,
    ) {
        let cost = crate::output::cost_tracker::estimate_cost(
            &result.usage,
            &self.config.execution_model,
        );

        if let Err(e) = self
            .cost_tracker
            .record(
                &result.usage,
                &self.config.execution_model,
                &self.config.provider_name,
                mode_name,
                ctx.channel.as_str(),
            )
            .await
        {
            warn!("IntentPipeline: failed to record usage: {}", e);
        }

        // Emit usage report to the streaming relay
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::UsageReport {
                    prompt_tokens: result.usage.prompt_tokens,
                    completion_tokens: result.usage.completion_tokens,
                    cache_read_tokens: result.usage.cache_read_tokens,
                    cache_write_tokens: result.usage.cache_write_tokens,
                    estimated_cost_usd: cost,
                    model: self.config.execution_model.clone(),
                    response_time_ms: pipeline_elapsed_ms,
                })
                .await;
        }
    }
```

**Step 3: Update the `process_message` call site**

In `crates/agent/src/intent_pipeline/pipeline.rs`, update the `record_usage` call at line 295. Replace:

```rust
        // Step 6: Record usage (best-effort)
        self.record_usage(&router_result, &mode_name, ctx).await;
```

with:

```rust
        // Step 6: Record usage (best-effort) + emit usage report
        let pipeline_elapsed_ms = pipeline_start.elapsed().as_millis() as u64;
        self.record_usage(&router_result, &mode_name, ctx, &event_tx, pipeline_elapsed_ms)
            .await;
```

**Step 4: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -30`
Expected: Success

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/output/cost_tracker.rs
git commit -m "feat(agent): emit UsageReport event after cost tracking"
```

---

### Task 4: Wire All Events Through the Chat Relay

**Files:**
- Modify: `crates/desktop/src/commands/chat.rs:L190-L370`

**Step 1: Add TransparencyData accumulator**

In `crates/desktop/src/commands/chat.rs`, after `let mut current_text = String::new();` (line 212), add:

```rust
        // Transparency data accumulation
        let mut transparency = desktop_shared::events::TransparencyData::default();
```

**Step 2: Replace the `_ => {}` catch-all with full handlers**

Replace lines 336-356 (from `AgentEvent::ClassificationComplete` through `_ => {}`) with:

```rust
                        AgentEvent::ClassificationComplete { strategy, confidence, source, duration_ms } => {
                            transparency.classification = Some(desktop_shared::events::TransparencyClassification {
                                strategy: strategy.clone(),
                                confidence,
                                source: source.clone(),
                            });
                            if let Some(ref mut timing) = transparency.timing {
                                timing.classification_ms = Some(duration_ms);
                            } else {
                                transparency.timing = Some(desktop_shared::events::TransparencyTiming {
                                    total_ms: 0,
                                    classification_ms: Some(duration_ms),
                                    context_assembly_ms: None,
                                });
                            }
                            let _ = app.emit(
                                AGENT_CLASSIFICATION_COMPLETE,
                                ClassificationCompletePayload {
                                    session_key: sk.clone(),
                                    strategy,
                                    confidence,
                                },
                            );
                        }
                        AgentEvent::ExecutionStarted { engine, max_iterations } => {
                            transparency.execution = Some(desktop_shared::events::TransparencyExecution {
                                engine: engine.clone(),
                                iterations: 0,
                                max_iterations: max_iterations as u32,
                                escalations: 0,
                            });
                            let _ = app.emit(
                                AGENT_EXECUTION_STARTED,
                                ExecutionStartedPayload {
                                    session_key: sk.clone(),
                                    engine,
                                    max_iterations,
                                },
                            );
                        }
                        AgentEvent::ContextAssembled { duration_ms, .. } => {
                            if let Some(ref mut timing) = transparency.timing {
                                timing.context_assembly_ms = Some(duration_ms);
                            } else {
                                transparency.timing = Some(desktop_shared::events::TransparencyTiming {
                                    total_ms: 0,
                                    classification_ms: None,
                                    context_assembly_ms: Some(duration_ms),
                                });
                            }
                        }
                        AgentEvent::IterationStart { iteration, max } => {
                            if let Some(ref mut exec) = transparency.execution {
                                exec.iterations = iteration as u32;
                            }
                            let _ = app.emit(
                                events::AGENT_ITERATION_START,
                                events::IterationStartPayload {
                                    session_key: sk.clone(),
                                    iteration,
                                    max_iterations: max,
                                },
                            );
                        }
                        AgentEvent::ConfidenceAssessed { score, action } => {
                            let _ = app.emit(
                                events::AGENT_CONFIDENCE_ASSESSED,
                                events::ConfidenceAssessedPayload {
                                    session_key: sk.clone(),
                                    score,
                                    action,
                                },
                            );
                        }
                        AgentEvent::UsageReport {
                            prompt_tokens, completion_tokens,
                            cache_read_tokens, cache_write_tokens,
                            estimated_cost_usd, model, response_time_ms,
                        } => {
                            transparency.usage = Some(desktop_shared::events::TransparencyUsage {
                                prompt_tokens,
                                completion_tokens,
                                cache_read_tokens,
                                cache_write_tokens,
                            });
                            transparency.cost = Some(desktop_shared::events::TransparencyCost {
                                estimated_usd: estimated_cost_usd,
                                model: model.clone(),
                            });
                            if let Some(ref mut timing) = transparency.timing {
                                timing.total_ms = response_time_ms;
                            } else {
                                transparency.timing = Some(desktop_shared::events::TransparencyTiming {
                                    total_ms: response_time_ms,
                                    classification_ms: None,
                                    context_assembly_ms: None,
                                });
                            }
                            let _ = app.emit(
                                events::AGENT_USAGE_REPORT,
                                events::UsageReportPayload {
                                    session_key: sk.clone(),
                                    prompt_tokens,
                                    completion_tokens,
                                    cache_read_tokens,
                                    cache_write_tokens,
                                    estimated_cost_usd,
                                    model,
                                    response_time_ms,
                                },
                            );
                        }
                        AgentEvent::MemoryAccess { action, query, results_count } => {
                            transparency.memory_accesses.push(desktop_shared::events::TransparencyMemoryAccess {
                                action: action.clone(),
                                query: query.clone(),
                                results_count,
                            });
                            let _ = app.emit(
                                events::AGENT_MEMORY_ACCESS,
                                events::MemoryAccessPayload {
                                    session_key: sk.clone(),
                                    action,
                                    query,
                                    results_count,
                                },
                            );
                        }
                        AgentEvent::SkillLoaded { name, trigger } => {
                            transparency.skills.push(desktop_shared::events::TransparencySkill {
                                name: name.clone(),
                                trigger: trigger.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_SKILL_LOADED,
                                events::SkillLoadedPayload {
                                    session_key: sk.clone(),
                                    name,
                                    trigger,
                                },
                            );
                        }
                        AgentEvent::LearningEvent { event_type, detail } => {
                            transparency.learning.push(desktop_shared::events::TransparencyLearning {
                                event_type: event_type.clone(),
                                detail: detail.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_LEARNING_EVENT,
                                events::LearningEventPayload {
                                    session_key: sk.clone(),
                                    event_type,
                                    detail,
                                },
                            );
                        }
                        AgentEvent::SubagentSpawned { label, profile } => {
                            transparency.subagents.push(desktop_shared::events::TransparencySubagent {
                                label: label.clone(),
                                profile: profile.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_SUBAGENT_SPAWNED,
                                events::SubagentSpawnedPayload {
                                    session_key: sk.clone(),
                                    label,
                                    profile,
                                },
                            );
                        }
```

**Step 3: Also accumulate tool data in transparency**

In the existing `AgentEvent::ToolEnd` handler (around line 261), add after the segment push (before the emit):

```rust
                            transparency.tools.push(desktop_shared::events::TransparencyTool {
                                name: name.clone(),
                                success,
                                duration_ms,
                            });
```

**Step 4: Persist transparency alongside segments**

Replace the `AgentEvent::Done` handler's metadata persistence section (lines 294-316) with:

```rust
                        AgentEvent::Done { content } => {
                            flush_text(&mut current_text, &mut segments);
                            // Persist segments + transparency to the assistant message metadata
                            let mut meta = serde_json::Map::new();
                            if !segments.is_empty() {
                                meta.insert(
                                    "segments".to_string(),
                                    serde_json::to_value(&segments).unwrap_or_default(),
                                );
                            }
                            // Always persist transparency (even if mostly empty — the frontend
                            // checks for its presence to know the message has transparency data)
                            meta.insert(
                                "transparency".to_string(),
                                serde_json::to_value(&transparency).unwrap_or_default(),
                            );
                            let meta_value = serde_json::Value::Object(meta);
                            match repos.sessions.update_last_assistant_metadata(
                                &sk, None, Some(&meta_value),
                            ).await {
                                Ok(true) => {}
                                Ok(false) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    match repos.sessions.update_last_assistant_metadata(
                                        &sk, None, Some(&meta_value),
                                    ).await {
                                        Ok(false) => tracing::warn!("metadata persist: no assistant message found for {sk}"),
                                        Err(e) => tracing::warn!("metadata persist retry failed for {sk}: {e}"),
                                        _ => {}
                                    }
                                }
                                Err(e) => tracing::warn!("metadata persist failed for {sk}: {e}"),
                            }
                            let _ = app.emit(
                                AGENT_DONE,
                                DonePayload {
                                    session_key: sk.clone(),
                                    content,
                                },
                            );
                            break;
                        }
```

**Step 5: Deserialize transparency in chat_messages**

In `chat_messages` (line 106-124), update the message mapping to also extract `transparency`. Replace:

```rust
    Ok(rows
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant" || m.role == "interaction")
        .map(|m| {
            let segments: Option<Vec<events::MessageSegment>> = m
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("segments"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            ChatMessageResponse {
                id: m.id.to_string(),
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: m.timestamp,
                segments,
            }
        })
        .collect())
```

with:

```rust
    Ok(rows
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant" || m.role == "interaction")
        .map(|m| {
            let segments: Option<Vec<events::MessageSegment>> = m
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("segments"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let transparency: Option<events::TransparencyData> = m
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("transparency"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            ChatMessageResponse {
                id: m.id.to_string(),
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: m.timestamp,
                segments,
                transparency,
            }
        })
        .collect())
```

**Step 6: Add transparency field to ChatMessageResponse**

In `crates/desktop-shared/src/commands.rs:L134-L141`, add the field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<MessageSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<TransparencyData>,
}
```

Add the import at the top of `commands.rs`:

```rust
use crate::events::TransparencyData;
```

Update the `chat_send` return at line 372-378 to include `transparency: None`.

**Step 7: Verify full workspace compiles**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Success (or warnings only)

**Step 8: Commit**

```bash
git add crates/desktop/src/commands/chat.rs crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop): wire transparency events through chat relay

Accumulate TransparencyData per-request, persist to metadata,
deserialize on chat_messages fetch. Replace _ => {} catch-all
with full event handlers."
```

---

### Task 5: Add Frontend TypeScript Types

**Files:**
- Modify: `desktop-ui/src/lib/types.ts:L67-L141`

**Step 1: Add TransparencyData types**

After the `ExecutionStartedPayload` interface (line 140), add:

```typescript
// ── Transparency Events ───────────────────────────────────────────────

export interface IterationStartPayload {
  sessionKey: string;
  iteration: number;
  maxIterations: number;
}

export interface UsageReportPayload {
  sessionKey: string;
  promptTokens: number;
  completionTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  estimatedCostUsd: number;
  model: string;
  responseTimeMs: number;
}

export interface MemoryAccessPayload {
  sessionKey: string;
  action: string;
  query?: string;
  resultsCount: number;
}

export interface SkillLoadedPayload {
  sessionKey: string;
  name: string;
  trigger: string;
}

export interface LearningEventPayload {
  sessionKey: string;
  eventType: string;
  detail: string;
}

export interface SubagentSpawnedPayload {
  sessionKey: string;
  label: string;
  profile: string;
}

// ── Transparency Data (per-message) ───────────────────────────────────

export interface TransparencyData {
  usage?: { promptTokens: number; completionTokens: number; cacheReadTokens: number; cacheWriteTokens: number };
  cost?: { estimatedUsd: number; model: string };
  timing?: { totalMs: number; classificationMs?: number; contextAssemblyMs?: number };
  tools?: { name: string; success: boolean; durationMs: number }[];
  memoryAccesses?: { action: string; query?: string; resultsCount: number }[];
  skills?: { name: string; trigger: string }[];
  execution?: { engine: string; iterations: number; maxIterations: number; escalations: number };
  classification?: { strategy: string; confidence: number; source: string };
  subagents?: { label: string; profile: string }[];
  learning?: { eventType: string; detail: string }[];
}
```

**Step 2: Add transparency to ChatMessage**

Update the `ChatMessage` interface (line 71-77) to:

```typescript
export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'interaction';
  content: string;
  timestamp?: string;
  segments?: MessageSegment[];
  transparency?: TransparencyData;
}
```

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/types.ts
git commit -m "feat(desktop-ui): add transparency TypeScript types"
```

---

### Task 6: Update useAgentStream Hook

**Files:**
- Modify: `desktop-ui/src/hooks/useAgentStream.ts:L1-L180`

**Step 1: Add transparency state and event listeners**

Update imports to include new payload types:

```typescript
import type {
  ActiveInteraction,
  ContentChunkPayload,
  ToolStartPayload,
  ToolEndPayload,
  AgentDonePayload,
  AgentErrorPayload,
  InteractionRequestPayload,
  MessageSegment,
  TransparencyData,
  ClassificationCompletePayload,
  ExecutionStartedPayload,
  IterationStartPayload,
  UsageReportPayload,
  MemoryAccessPayload,
  SkillLoadedPayload,
  LearningEventPayload,
  SubagentSpawnedPayload,
} from '../lib/types';
```

Add to the `AgentStream` interface:

```typescript
  transparency: TransparencyData | null;
  clearTransparency: () => void;
```

Add state after `activeInteraction` state (line 42):

```typescript
  const [transparency, setTransparency] = useState<TransparencyData | null>(null);
```

Add to `resetStream` callback (inside the function body, after `setActiveInteraction(null)`):

```typescript
    setTransparency(null);
```

Add these event listeners after the `agent:error` listener (after line 167):

```typescript
  useEvent<ClassificationCompletePayload>('agent:classification_complete', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      classification: { strategy: payload.strategy, confidence: payload.confidence, source: 'pipeline' },
    }));
  });

  useEvent<ExecutionStartedPayload>('agent:execution_started', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      execution: { engine: payload.engine, iterations: 0, maxIterations: payload.maxIterations, escalations: 0 },
    }));
  });

  useEvent<IterationStartPayload>('agent:iteration_start', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      execution: prev?.execution
        ? { ...prev.execution, iterations: payload.iteration }
        : { engine: 'unknown', iterations: payload.iteration, maxIterations: payload.maxIterations, escalations: 0 },
    }));
  });

  useEvent<UsageReportPayload>('agent:usage_report', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      usage: {
        promptTokens: payload.promptTokens,
        completionTokens: payload.completionTokens,
        cacheReadTokens: payload.cacheReadTokens,
        cacheWriteTokens: payload.cacheWriteTokens,
      },
      cost: { estimatedUsd: payload.estimatedCostUsd, model: payload.model },
      timing: { ...prev?.timing, totalMs: payload.responseTimeMs },
    }));
  });

  useEvent<MemoryAccessPayload>('agent:memory_access', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      memoryAccesses: [...(prev?.memoryAccesses ?? []), { action: payload.action, query: payload.query, resultsCount: payload.resultsCount }],
    }));
  });

  useEvent<SkillLoadedPayload>('agent:skill_loaded', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      skills: [...(prev?.skills ?? []), { name: payload.name, trigger: payload.trigger }],
    }));
  });

  useEvent<LearningEventPayload>('agent:learning_event', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      learning: [...(prev?.learning ?? []), { eventType: payload.eventType, detail: payload.detail }],
    }));
  });

  useEvent<SubagentSpawnedPayload>('agent:subagent_spawned', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setTransparency((prev) => ({
      ...prev,
      subagents: [...(prev?.subagents ?? []), { label: payload.label, profile: payload.profile }],
    }));
  });
```

Also update the `agent:tool_end` handler to accumulate into transparency too. After the existing `setSegments(...)` call:

```typescript
    setTransparency((prev) => ({
      ...prev,
      tools: [...(prev?.tools ?? []), { name: payload.name, success: payload.success, durationMs: payload.durationMs }],
    }));
```

Add the clearTransparency callback:

```typescript
  const clearTransparency = useCallback(() => setTransparency(null), []);
```

Update the return object to include `transparency` and `clearTransparency`.

**Step 2: Commit**

```bash
git add desktop-ui/src/hooks/useAgentStream.ts
git commit -m "feat(desktop-ui): accumulate transparency data in useAgentStream"
```

---

### Task 7: Update useChatSession Hook

**Files:**
- Modify: `desktop-ui/src/hooks/useChatSession.ts:L1-L97`

**Step 1: Pass transparency through**

Add `TransparencyData` to the import from types:

```typescript
import type { ActiveInteraction, ChatMessage, MessageSegment, TransparencyData } from '../lib/types';
```

Add to the `ChatSession` interface:

```typescript
  transparency: TransparencyData | null;
```

Destructure `transparency` and `clearTransparency` from `stream` (line 46):

```typescript
  const { isStreaming, clearSegments, clearTransparency, segments, transparency } = stream;
```

In the useEffect that clears segments (line 51-57), also clear transparency:

```typescript
  useEffect(() => {
    const count = messages.filter(m => m.role === 'assistant').length;
    if (!isStreaming && hasSegmentsRef.current && count > assistantCountRef.current) {
      clearSegments();
      clearTransparency();
    }
    assistantCountRef.current = count;
  }, [messages, isStreaming, clearSegments, clearTransparency]);
```

Add `transparency` to the return object:

```typescript
  return {
    messages: displayMessages,
    segments: stream.segments,
    transparency: stream.transparency,
    isStreaming: stream.isStreaming,
    activeTools: stream.activeTools,
    error: stream.error,
    activeInteraction: stream.activeInteraction,
    input,
    setInput,
    send,
    clearInteraction: stream.clearInteraction,
  };
```

**Step 2: Commit**

```bash
git add desktop-ui/src/hooks/useChatSession.ts
git commit -m "feat(desktop-ui): pass transparency through useChatSession"
```

---

### Task 8: Create UI Components

**Files:**
- Create: `desktop-ui/src/components/chat/TokenBadge.tsx`
- Create: `desktop-ui/src/components/chat/TransparencyPanel.tsx`
- Create: `desktop-ui/src/components/chat/TransparencyToggle.tsx`

**Step 1: Create TokenBadge component**

Create `desktop-ui/src/components/chat/TokenBadge.tsx`:

```tsx
import { useState } from 'react';
import { ChevronDown, Loader2 } from 'lucide-react';
import { formatDuration } from '../../lib/utils';
import type { TransparencyData } from '../../lib/types';

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  return `$${usd.toFixed(3)}`;
}

interface TokenBadgeProps {
  transparency: TransparencyData;
  isStreaming?: boolean;
}

export function TokenBadge({ transparency, isStreaming }: TokenBadgeProps) {
  const [expanded, setExpanded] = useState(false);
  const { usage, cost, timing } = transparency;

  // During streaming, show spinner until usage arrives
  if (!usage) {
    if (!isStreaming) return null;
    return (
      <div className="flex justify-end mt-1">
        <div className="flex items-center gap-1 text-[10px] font-light text-dim">
          <Loader2 className="w-2.5 h-2.5 animate-spin" strokeWidth={1.5} />
        </div>
      </div>
    );
  }

  return (
    <div className="mt-1.5">
      <button
        onClick={() => setExpanded(!expanded)}
        className="ml-auto flex items-center gap-1.5 text-[10px] font-light text-dim hover:text-muted transition-colors"
      >
        <span>{'\u2191'}{formatTokens(usage.promptTokens)}</span>
        <span>{'\u2193'}{formatTokens(usage.completionTokens)}</span>
        {cost && <span>{'\u00b7'} {formatCost(cost.estimatedUsd)}</span>}
        <ChevronDown
          className={`w-2.5 h-2.5 transition-transform ${expanded ? 'rotate-180' : ''}`}
          strokeWidth={1.5}
        />
      </button>

      {expanded && (
        <div className="mt-1.5 p-2.5 rounded-lg bg-surface-base border border-border text-[10px] font-light space-y-1">
          <div className="flex justify-between text-muted">
            <span>Input tokens</span>
            <span className="text-secondary">{usage.promptTokens.toLocaleString()}</span>
          </div>
          <div className="flex justify-between text-muted">
            <span>Output tokens</span>
            <span className="text-secondary">{usage.completionTokens.toLocaleString()}</span>
          </div>
          {usage.cacheReadTokens > 0 && (
            <div className="flex justify-between text-muted">
              <span>Cache read</span>
              <span className="text-secondary">{usage.cacheReadTokens.toLocaleString()}</span>
            </div>
          )}
          {usage.cacheWriteTokens > 0 && (
            <div className="flex justify-between text-muted">
              <span>Cache write</span>
              <span className="text-secondary">{usage.cacheWriteTokens.toLocaleString()}</span>
            </div>
          )}
          {cost && (
            <>
              <div className="border-t border-border my-1" />
              <div className="flex justify-between text-muted">
                <span>Model</span>
                <span className="text-secondary">{cost.model}</span>
              </div>
              <div className="flex justify-between text-muted">
                <span>Cost</span>
                <span className="text-secondary">{formatCost(cost.estimatedUsd)}</span>
              </div>
            </>
          )}
          {timing?.totalMs && timing.totalMs > 0 && (
            <div className="flex justify-between text-muted">
              <span>Latency</span>
              <span className="text-secondary">{formatDuration(timing.totalMs)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

**Step 2: Create TransparencyPanel component**

Create `desktop-ui/src/components/chat/TransparencyPanel.tsx`:

```tsx
import { useState } from 'react';
import {
  ChevronDown, FileText, Package, Cpu, Brain, Database, Bot, BookOpen,
} from 'lucide-react';
import { formatDuration } from '../../lib/utils';
import type { TransparencyData } from '../../lib/types';

interface CollapsibleBoxProps {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
  defaultOpen?: boolean;
}

function CollapsibleBox({ title, icon: Icon, children, defaultOpen = true }: CollapsibleBoxProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 bg-surface-raised hover:bg-surface-highest transition-colors"
      >
        <Icon className="w-3 h-3 text-muted" strokeWidth={1.5} />
        <span className="flex-1 text-left text-[11px] font-medium text-secondary">{title}</span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-transform ${open ? 'rotate-0' : '-rotate-90'}`}
          strokeWidth={1.5}
        />
      </button>
      {open && <div className="px-3 py-2 space-y-1 text-[10px] font-light">{children}</div>}
    </div>
  );
}

function Row({ icon: Icon, label, detail }: { icon: React.ElementType; label: string; detail?: string }) {
  return (
    <div className="flex items-center gap-1.5 text-muted">
      <Icon className="w-3 h-3 shrink-0" strokeWidth={1.5} />
      <span className="text-secondary">{label}</span>
      {detail && <span className="text-dim ml-auto">{detail}</span>}
    </div>
  );
}

interface TransparencyPanelProps {
  transparency: TransparencyData;
}

export function TransparencyPanel({ transparency }: TransparencyPanelProps) {
  const { memoryAccesses, skills, execution, classification, subagents, learning, tools } = transparency;
  const hasKlyntbot = (memoryAccesses && memoryAccesses.length > 0) || (tools && tools.length > 0);
  const hasContext = skills && skills.length > 0;
  const hasExecution = execution || classification || (subagents && subagents.length > 0) || (learning && learning.length > 0);

  if (!hasKlyntbot && !hasContext && !hasExecution) return null;

  return (
    <div className="mt-2 space-y-1.5">
      {/* Klyntbot Box */}
      {hasKlyntbot && (
        <CollapsibleBox title="klyntbot" icon={Brain}>
          {memoryAccesses?.map((ma, i) => (
            <Row
              key={`mem-${i}`}
              icon={FileText}
              label={`memory: ${ma.query ?? ma.action}`}
              detail={ma.resultsCount > 0 ? `${ma.resultsCount} hits` : undefined}
            />
          ))}
          {tools?.map((tool, i) => (
            <Row
              key={`tool-${i}`}
              icon={Cpu}
              label={tool.name}
              detail={formatDuration(tool.durationMs)}
            />
          ))}
        </CollapsibleBox>
      )}

      {/* Context Box */}
      {hasContext && (
        <CollapsibleBox title="Context" icon={BookOpen}>
          {skills?.map((skill, i) => (
            <Row
              key={`skill-${i}`}
              icon={Package}
              label={`skill: ${skill.name}`}
              detail={skill.trigger}
            />
          ))}
        </CollapsibleBox>
      )}

      {/* Execution Detail */}
      {hasExecution && (
        <CollapsibleBox title="Execution" icon={Cpu} defaultOpen={false}>
          {execution && (
            <Row
              icon={Cpu}
              label={`Engine: ${execution.engine}`}
              detail={`${execution.iterations}/${execution.maxIterations} iterations`}
            />
          )}
          {classification && (
            <Row
              icon={Brain}
              label={`Strategy: ${classification.strategy}`}
              detail={`${Math.round(classification.confidence * 100)}%`}
            />
          )}
          {subagents?.map((sa, i) => (
            <Row key={`sa-${i}`} icon={Bot} label={`Agent: ${sa.label}`} detail={sa.profile} />
          ))}
          {learning?.map((le, i) => (
            <Row key={`le-${i}`} icon={Database} label={le.eventType} detail={le.detail} />
          ))}
          {(!subagents || subagents.length === 0) && (
            <Row icon={Bot} label="Agents: none" />
          )}
          {(!learning || learning.length === 0) && (
            <Row icon={Database} label="Learning: none" />
          )}
        </CollapsibleBox>
      )}
    </div>
  );
}
```

**Step 3: Create TransparencyToggle component**

Create `desktop-ui/src/components/chat/TransparencyToggle.tsx`:

```tsx
import { Eye, EyeOff } from 'lucide-react';

interface TransparencyToggleProps {
  enabled: boolean;
  onToggle: () => void;
}

export function TransparencyToggle({ enabled, onToggle }: TransparencyToggleProps) {
  const Icon = enabled ? Eye : EyeOff;

  return (
    <button
      onClick={onToggle}
      className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors ${
        enabled
          ? 'bg-brand/10 text-brand hover:bg-brand/20'
          : 'text-muted hover:bg-surface-base hover:text-secondary'
      }`}
      title={enabled ? 'Hide transparency data' : 'Show transparency data'}
    >
      <Icon className="w-4 h-4" strokeWidth={1.5} />
    </button>
  );
}
```

**Step 4: Commit**

```bash
git add desktop-ui/src/components/chat/TokenBadge.tsx desktop-ui/src/components/chat/TransparencyPanel.tsx desktop-ui/src/components/chat/TransparencyToggle.tsx
git commit -m "feat(desktop-ui): add TokenBadge, TransparencyPanel, and TransparencyToggle components"
```

---

### Task 9: Integrate Into MessageList and Chat

**Files:**
- Modify: `desktop-ui/src/components/chat/MessageList.tsx:L1-L117`
- Modify: `desktop-ui/src/components/views/Chat.tsx:L1-L471`

**Step 1: Update MessageList to render transparency**

Add to MessageList imports:

```typescript
import { TokenBadge } from './TokenBadge';
import { TransparencyPanel } from './TransparencyPanel';
import type { ActiveInteraction, ChatMessage, MessageSegment, TransparencyData } from '../../lib/types';
```

Add new props to `MessageListProps`:

```typescript
  showTransparency: boolean;
  /** Live transparency data for the current streaming message. */
  liveTransparency: TransparencyData | null;
```

In the assistant message rendering (line 51-58), wrap the content to add transparency below it:

```tsx
          ) : (
            <div className="max-w-[85%]">
              {msg.segments && msg.segments.length > 0 ? (
                <SegmentedMessage segments={msg.segments} />
              ) : (
                <MarkdownContent content={msg.content} />
              )}
              {showTransparency && msg.transparency && (
                <>
                  <TokenBadge transparency={msg.transparency} />
                  <TransparencyPanel transparency={msg.transparency} />
                </>
              )}
            </div>
          )}
```

For the live streaming section (line 62-73), add transparency below:

```tsx
      {(segments.length > 0 || activeTools.length > 0) && (
        <div className="flex justify-start">
          <div className="max-w-[85%]">
            <SegmentedMessage
              segments={segments}
              activeTools={activeTools}
              isStreaming={isStreaming}
            />
            {showTransparency && liveTransparency && (
              <>
                <TokenBadge transparency={liveTransparency} isStreaming={isStreaming} />
                <TransparencyPanel transparency={liveTransparency} />
              </>
            )}
          </div>
        </div>
      )}
```

**Step 2: Update Chat.tsx to add toggle and pass props**

Add imports to Chat.tsx:

```typescript
import { TransparencyToggle } from '../chat/TransparencyToggle';
import { Eye } from 'lucide-react';
```

Add toggle state (after the `useChatSession` call, around line 160):

```typescript
  const [showTransparency, setShowTransparency] = useState(() => {
    try { return localStorage.getItem('chat:transparency') === 'true'; } catch { return false; }
  });
  const toggleTransparency = useCallback(() => {
    setShowTransparency((prev) => {
      const next = !prev;
      try { localStorage.setItem('chat:transparency', String(next)); } catch {}
      return next;
    });
  }, []);
```

Add the TransparencyToggle button in the conversation header area. Find the right panel (`{/* Right Panel — Conversation */}` at line 385-386) and add a header bar:

```tsx
      <div className="flex-1 flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-end px-4 py-2 border-b border-border">
          <TransparencyToggle enabled={showTransparency} onToggle={toggleTransparency} />
        </div>

        {/* Messages */}
```

Update the `MessageList` usage (line 398-410) to pass the new props:

```tsx
              <MessageList
                messages={chat.messages}
                segments={chat.segments}
                isStreaming={chat.isStreaming}
                activeTools={chat.activeTools}
                error={chat.error}
                activeInteraction={chat.activeInteraction}
                sessionKey={selectedThread}
                onInteractionSubmitted={() => {
                  chat.clearInteraction();
                  refetchThreads();
                }}
                showTransparency={showTransparency}
                liveTransparency={chat.transparency}
              />
```

**Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Success

**Step 4: Commit**

```bash
git add desktop-ui/src/components/chat/MessageList.tsx desktop-ui/src/components/views/Chat.tsx
git commit -m "feat(desktop-ui): integrate transparency into MessageList and Chat view

Toggle via header button, persisted to localStorage.
Shows TokenBadge + TransparencyPanel per assistant message."
```

---

### Task 10: Full Workspace Verification

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: 0 warnings (or only pre-existing)

**Step 2: Run tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All tests pass

**Step 3: Run frontend build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Success

**Step 4: Fix any issues found and commit**

```bash
git add -A
git commit -m "fix: resolve clippy warnings and test failures from transparency feature"
```

---

### Task 11: Interactive Playground Demo

**Step 1: Invoke the playground skill**

Use the `playground` skill to create an interactive HTML playground that demonstrates the transparency UI. The playground should:

- Show a mock chat conversation with 3 messages
- Each assistant message has a TokenBadge with realistic mock data
- Collapsible klyntbot/Context/Execution boxes with mock data
- A toggle button in the header to show/hide all transparency
- Use the same color tokens and styling from the design
- Allow users to toggle transparency on/off and expand/collapse sections

This demonstrates the before/after visual change for the user.
