# Persona Squads Phase 2: Squad Chat Mode — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add squad-bound chat conversations where users send messages to a squad, the orchestrator handles tool calls, personas respond in parallel, and the user toggles between multi-voice (all personas visible) and synthesized (merged) output.

**Architecture:** Extend `RoutingContext` with `squad_id`. When set, `AgentRuntime` delegates to a new `SquadExecutor` which: (1) runs the orchestrator in Reactive mode for tool calls, (2) fans out to N persona Direct calls in parallel, (3) optionally synthesizes into a single response. Persona outputs are stored as session messages with `persona_id` in metadata. Frontend adds a "New Squad Chat" action, multi-voice message rendering, and a synthesized/multi-voice toggle.

**Tech Stack:** Rust (agent/session/app-core/desktop crates), SQLite, TypeScript/React (desktop-ui), Tauri IPC, SSE streaming.

**Spec:** `docs/superpowers/specs/2026-03-18-persona-squads-design.md` (Phase 2 section)

**Phase 1 reference:** `docs/superpowers/plans/2026-03-18-persona-squads.md` — schema, SquadRepo, parallel persona calls in Insight Review are all complete.

---

## File Map

### New Files
| File | Responsibility |
|------|---------------|
| `crates/agent/src/intent_pipeline/engines/squad.rs` | SquadExecutor — orchestrator pass + parallel persona fan-out + synthesis |
| `desktop-ui/src/features/chat/components/PersonaMessage.tsx` | Renders a single persona's response in multi-voice mode |
| `desktop-ui/src/features/chat/components/SquadChatHeader.tsx` | Shows squad info (icon, name, members) at top of squad chat |
| `desktop-ui/src/features/chat/components/VoiceToggle.tsx` | Toggle between multi-voice and synthesized display modes |

### Modified Files
| File | Changes |
|------|---------|
| `crates/tools-core/src/routing.rs:59-78` | Add `squad_id: Option<String>` field to `RoutingContext`, update constructors |
| `crates/storage/migrations/001_initial.sql:210-218` | Add `squad_id TEXT` column to `sessions` table |
| `crates/storage/src/repos/session.rs` | Add `squad_id` to session CRUD queries, new `list_by_squad()` |
| `crates/session/src/manager.rs:21-38` | Add `squad_id: Option<String>` to `Session` struct |
| `crates/agent/src/agent_runtime/runtime.rs:182-240` | Detect `squad_id` on RoutingContext, delegate to SquadExecutor |
| `crates/agent/src/agent_loop/mod.rs:412-416` | Set `squad_id` on RoutingContext from session metadata |
| `crates/agent/src/intent_pipeline/engines/mod.rs` | Export squad module |
| `crates/storage/src/rows/session.rs` | Add `squad_id` to `SessionRow`/`SessionListRow`, `persona_id` to `SessionMessageRow` |
| `crates/app-core/src/handlers/chat/streaming.rs` | Accept `squad_id` in `chat_send`, store on session, emit persona events |
| `crates/app-core/src/handlers/chat/threads.rs` | Return `squad_id` and `squad_name` in `ChatThreadResponse` |
| `crates/desktop-shared/src/commands/chat.rs` | Add `squad_id` to `ChatThreadResponse`, `SessionContextInput` |
| `crates/desktop/src/commands/chat.rs:37-52` | Pass `squad_id` through `chat_send` |
| `desktop-ui/src/features/chat/hooks/useChatSession.ts:72-92` | Pass `squad_id` in `chat_send` IPC, handle persona events |
| `desktop-ui/src/features/chat/hooks/useAgentStream.ts` | Add `personaMessages` to stream state for multi-voice |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Render SquadChatHeader, VoiceToggle, PersonaMessages |
| `desktop-ui/src/features/chat/components/SidebarChat.tsx` | "New Squad Chat" action with squad picker |
| `desktop-ui/src/features/notes/components/insight/SquadPicker.tsx` | Fix selected squad name display bug |

---

## Task 0: Fix SquadPicker Selected Display Bug (Phase 1 polish)

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/SquadPicker.tsx:22-38`

- [ ] **Step 1: Read the SquadPicker component and identify the bug**

The `selected` variable on line 22 depends on `squads` from `useSquads()` being loaded. When the hook hasn't finished fetching, `squads` is empty and `selected` is undefined, so the button falls back to "Select squad". The fix: show the `selectedSquadId` immediately (optimistic) and update to the full name once squads load.

- [ ] **Step 2: Fix the display logic**

In `SquadPicker.tsx`, change the trigger button render logic. Replace:

```tsx
const selected = squads.find((s) => s.id === selectedSquadId);
```

With a stable display that doesn't depend on the async squad list:

```tsx
const selected = squads.find((s) => s.id === selectedSquadId);
```

The real issue is that `useSquads` returns `loading: true` initially, and during that time `squads` is `[]`. The fix is to **not reset `squads` to `[]` on refresh** — keep the previous value while loading. In `useSquads.ts`, change:

```typescript
const refresh = useCallback(async () => {
  setLoading(true);
  try {
    const result = await ipc<Squad[]>("list_squads", {});
    setSquads(result);
  } finally {
    setLoading(false);
  }
}, []);
```

The `setSquads` only runs on success, so `squads` retains its previous value during loading. The actual bug is that `selectedSquadId` in `InsightReviewState` is set but `useSquads()` inside `SquadPicker` mounts fresh with an empty array. Since `useSquads` calls `refresh()` on mount via `useEffect`, there's a brief window where `squads` is `[]`.

Fix: initialize `squads` from a **synchronous cache** or change `loading` default to not render "Select squad" while loading:

```tsx
// In SquadPicker trigger button:
{loading && selectedSquadId ? (
  <span className="text-[10px] text-muted-foreground">Loading...</span>
) : selected ? (
  <>
    <span>{selected.icon}</span>
    <span className="text-muted-foreground truncate max-w-[120px]">{selected.name}</span>
  </>
) : (
  <span className="text-[10px] text-muted-foreground">Select squad</span>
)}
```

- [ ] **Step 3: Verify fix**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SquadPicker.tsx
git commit -m "fix(ui): show loading state in SquadPicker when squad list is fetching"
```

---

## Task 1: Schema — Add squad_id to sessions + persona_id to session_messages

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:210-233`

- [ ] **Step 1: Add squad_id column to sessions table**

In `crates/storage/migrations/001_initial.sql`, add `squad_id TEXT` to the `sessions` CREATE TABLE statement after `pinned`:

```sql
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    squad_id          TEXT
);
```

- [ ] **Step 2: Add persona_id column to session_messages table**

Add `persona_id TEXT` to `session_messages`:

```sql
CREATE TABLE session_messages (
    id          TEXT PRIMARY KEY,
    session_key TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    request_id  TEXT,
    tool_calls  TEXT,
    metadata    TEXT,
    persona_id  TEXT
);
```

- [ ] **Step 3: Add index for squad_id on sessions**

```sql
CREATE INDEX IF NOT EXISTS idx_sessions_squad ON sessions(squad_id);
```

- [ ] **Step 4: Update SessionRow structs**

In `crates/storage/src/rows/session.rs`, add `squad_id: Option<String>` to `SessionRow` (after `pinned`) and `SessionListRow` (after `pinned`). Add `persona_id: Option<String>` to `SessionMessageRow` (after `metadata`). These are `#[derive(FromRow)]` structs — column order must match the SQL SELECT.

```rust
// In SessionRow:
pub squad_id: Option<String>,

// In SessionListRow:
pub squad_id: Option<String>,

// In SessionMessageRow:
pub persona_id: Option<String>,
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p storage 2>&1 | tail -5`
Note: Pre-release, no migration version bump needed — `001_initial.sql` is modified in-place per CLAUDE.md convention.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/
git commit -m "feat(storage): add squad_id to sessions and persona_id to session_messages"
```

---

## Task 2: Extend RoutingContext with squad_id

**Files:**
- Modify: `crates/tools-core/src/routing.rs:59-110`

- [ ] **Step 1: Write test for RoutingContext squad_id**

Add to the test module in `routing.rs` (or create one if none exists):

```rust
#[test]
fn test_routing_context_squad_id() {
    let ctx = RoutingContext::new("cli".into(), "test-chat".into());
    assert!(ctx.squad_id.is_none());

    let ctx2 = RoutingContext::with_squad("cli".into(), "test-chat".into(), "squad-123".to_string());
    assert_eq!(ctx2.squad_id.as_deref(), Some("squad-123"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools-core -E 'test(squad_id)'`
Expected: FAIL — `squad_id` field doesn't exist.

- [ ] **Step 3: Add squad_id field to RoutingContext**

In `crates/tools-core/src/routing.rs`, add to `RoutingContext` struct:

```rust
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    pub is_direct_mode: bool,
    pub delegation_depth: u32,
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
    /// Squad context — when set, the agent uses SquadExecutor for multi-persona responses.
    pub squad_id: Option<String>,
}
```

Update `new()`:
```rust
pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
    Self {
        channel, chat_id,
        interaction_tx: None,
        is_direct_mode: false,
        delegation_depth: 0,
        entity_tx: None,
        interaction_channel: None,
        squad_id: None,
    }
}
```

Update `with_interaction()` — add `squad_id: None`.

Add new constructor:
```rust
/// Non-interactive mode with a squad context.
pub fn with_squad(channel: ChannelName, chat_id: ChatId, squad_id: String) -> Self {
    Self {
        channel, chat_id,
        interaction_tx: None,
        is_direct_mode: false,
        delegation_depth: 0,
        entity_tx: None,
        interaction_channel: None,
        squad_id: Some(squad_id),
    }
}
```

- [ ] **Step 4: Fix any compilation errors in other crates**

Search for `RoutingContext` construction sites and add `squad_id: None` where needed. Grep: `RoutingContext {` or `RoutingContext::new`. Key sites:
- `crates/agent/src/agent_loop/mod.rs` (line 413)
- `crates/agent/src/` (delegation, subagent manager)
- `crates/mcp/src/` (MCP bridge)
- Any test files

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p tools-core`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/tools-core/src/routing.rs
git commit -m "feat(tools-core): add squad_id field to RoutingContext"
```

---

## Task 3: Extend Session with squad_id + Repo Methods

**Files:**
- Modify: `crates/session/src/manager.rs:21-38`
- Modify: `crates/storage/src/repos/session.rs`

- [ ] **Step 1: Add squad_id to Session struct**

In `crates/session/src/manager.rs`, add to `Session`:

```rust
pub struct Session {
    pub key: String,
    pub messages: Vec<SessionMessage>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: HashMap<String, String>,
    pub squad_id: Option<String>,
}
```

- [ ] **Step 2: Add squad_id to SessionRepo queries**

In `crates/storage/src/repos/session.rs`, update:
- `create_session()` — include `squad_id` in INSERT
- `get_session()` — include `squad_id` in SELECT result
- `list_sessions()` — include `squad_id`
- Add `set_squad_id(key, squad_id)` method

- [ ] **Step 3: Update SessionManager**

In `crates/session/src/manager.rs`, update `get_or_create()` to accept optional `squad_id` and pass to repo. Update `Session::new()` to include `squad_id: None`.

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p session -p storage 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/session/src/manager.rs crates/storage/src/repos/session.rs
git commit -m "feat(session): add squad_id to Session and SessionRepo"
```

---

## Task 4: SquadExecutor — Core Engine

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/squad.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`

- [ ] **Step 1: Write test for SquadExecutor**

In `squad.rs`, add a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_squad_executor_builds_persona_prompts() {
        // Verify that build_persona_prompt() creates correct system prompts
        let prompt = build_persona_prompt(
            "You are an AI assistant.",
            "Skeptic",
            "Critical analyst",
            "Questions claims and looks for weak spots",
            "direct",
        );
        assert!(prompt.contains("Skeptic"));
        assert!(prompt.contains("Critical analyst"));
        assert!(prompt.contains("direct"));
    }

    #[tokio::test]
    async fn test_squad_executor_combines_persona_responses() {
        let responses = vec![
            ("Skeptic".to_string(), "I disagree with this approach.".to_string()),
            ("Connector".to_string(), "I see a link to game theory.".to_string()),
        ];
        let combined = format_multi_voice(&responses);
        assert!(combined.contains("## Skeptic"));
        assert!(combined.contains("## Connector"));
        assert!(combined.contains("I disagree"));
        assert!(combined.contains("game theory"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(squad_executor)'`
Expected: FAIL — functions don't exist.

- [ ] **Step 3: Add PersonaPerspective variant to AgentEvent (BEFORE implementing SquadExecutor)**

In `crates/agent/src/events.rs`, add to the `AgentEvent` enum:

```rust
    /// A persona in a squad completed its analysis.
    PersonaPerspective {
        persona_id: String,
        persona_name: String,
        content: String,
    },
```

This must be done first because `fan_out_personas()` references `AgentEvent::PersonaPerspective`.

- [ ] **Step 4: Implement SquadExecutor**

Create `crates/agent/src/intent_pipeline/engines/squad.rs`:

```rust
//! SquadExecutor — orchestrates parallel persona LLM calls for squad chat mode.
//!
//! Flow:
//! 1. Orchestrator pass (Reactive mode) — handles tool calls if needed
//! 2. Persona fan-out (Direct mode) — N parallel persona calls
//! 3. Optional synthesis — combines persona outputs into one response
//!
//! Reuses DirectEngine for each persona call.

use std::sync::Arc;

use cognitive::{PersonaRow, ResolvedSquad};
use providers::{ChatParams, DynProvider, Message, UserContent};

use super::EngineResult;

/// Build a persona-specific system prompt that layers on top of the orchestrator context.
pub fn build_persona_prompt(
    orchestrator_context: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
) -> String {
    format!(
        r#"{orchestrator_context}

---

You are now responding as **{persona_name}**, a {persona_role}.
Your perspective: {persona_perspective}
Your tone should be: {persona_tone}

Respond to the user's message from your unique perspective. Be direct and specific.
Do NOT repeat other personas' points. Offer your distinct viewpoint."#
    )
}

/// Format multi-voice responses into a combined markdown string.
pub fn format_multi_voice(responses: &[(String, String)]) -> String {
    responses
        .iter()
        .map(|(name, content)| format!("## {name}\n\n{content}"))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// Synthesize multiple persona responses into a single coherent response.
pub fn build_synthesis_prompt(
    user_message: &str,
    persona_responses: &[(String, String)],
) -> String {
    let voices: String = persona_responses
        .iter()
        .map(|(name, content)| format!("**{name}:** {content}"))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"You received the following perspectives from multiple expert personas analyzing a user's message.

User's message: {user_message}

Persona perspectives:
{voices}

Synthesize these perspectives into a single, coherent response that:
1. Integrates the strongest points from each persona
2. Resolves contradictions by explaining trade-offs
3. Provides a clear, actionable recommendation
4. Credits specific personas when their insight is particularly valuable

Respond directly to the user — do not describe the synthesis process."#
    )
}

/// Run parallel persona LLM calls and return (persona_name, response) pairs.
pub async fn fan_out_personas(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(String, String)> {
    let futures: Vec<_> = personas
        .iter()
        .map(|persona| {
            let provider = provider.clone();
            let params = params.clone();
            let system = build_persona_prompt(
                orchestrator_context,
                &persona.name,
                &persona.role,
                &persona.perspective,
                &persona.tone,
            );
            let user_msg = user_message.to_string();
            let persona_name = persona.name.clone();
            let persona_id = persona.id.clone();
            let tx = event_tx.cloned();

            async move {
                let messages = vec![
                    Message::System { content: system },
                    Message::User {
                        content: UserContent::Text(user_msg),
                    },
                ];
                let result = provider.chat(&messages, None, &params).await;
                let text = result.ok().and_then(|r| r.content).unwrap_or_default();

                // Emit per-persona event
                if let Some(tx) = &tx {
                    let _ = tx
                        .send(crate::AgentEvent::PersonaPerspective {
                            persona_id: persona_id.clone(),
                            persona_name: persona_name.clone(),
                            content: text.clone(),
                        })
                        .await;
                }

                (persona_name, text)
            }
        })
        .collect();

    futures_util::future::join_all(futures).await
}
```

- [ ] **Step 5: Export the module**

In `crates/agent/src/intent_pipeline/engines/mod.rs`, add:
```rust
pub mod squad;
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(squad_executor)'`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/squad.rs crates/agent/src/intent_pipeline/engines/mod.rs crates/agent/src/events.rs
git commit -m "feat(agent): add SquadExecutor with parallel persona fan-out and synthesis"
```

---

## Task 5: Integrate SquadExecutor into AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:182-240`
- Modify: `crates/agent/src/agent_loop/mod.rs:400-416`

- [ ] **Step 1: Read current AgentRuntime::process_message**

Read `crates/agent/src/agent_runtime/runtime.rs` fully to understand the flow from skill selection to execution.

- [ ] **Step 2: Add squad detection in AgentRuntime**

After the orchestrator skill is selected (line ~200), check if `ctx.squad_id` is set. If so:

```rust
// After step 2 (active profile set):
if let Some(ref squad_id) = ctx.squad_id {
    // Squad mode: use SquadExecutor instead of normal ExecutionRouter
    return self.run_squad_execution(
        message, history, tool_definitions, tool_names,
        ctx, system_prompt, event_tx, cancel_token,
        squad_id, &profile,
    ).await;
}
```

- [ ] **Step 3: Implement run_squad_execution method**

Add to `impl AgentRuntime`:

```rust
async fn run_squad_execution(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value],
    tool_names: &[&str],
    ctx: &RoutingContext,
    system_prompt: Option<&str>,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    cancel_token: Option<CancellationToken>,
    squad_id: &str,
    profile: &Arc<SkillPackage>,
) -> Result<RuntimeResult> {
    // 1. Resolve squad from SquadRepo
    let squad_repo = self.squad_repo.as_ref()
        .ok_or_else(|| common::KlyntbotError::internal("Squad repo not available"))?;
    let resolved = squad_repo.resolve_squad(squad_id).await
        .map_err(|e| common::KlyntbotError::internal(&e.to_string()))?
        .ok_or_else(|| common::KlyntbotError::internal("Squad not found"))?;

    // 2. Orchestrator pass — use existing ExecutionRouter for tool calls
    //    AgentRuntime.router is ExecutionRouter. Call it with normal context.
    let orchestrator_result = self.router.execute(
        message, history.clone(), tool_definitions, tool_names,
        ctx, system_prompt, profile, &self.config,
        event_tx.clone(), cancel_token.clone(),
    ).await?;
    let orchestrator_context = orchestrator_result.content.clone().unwrap_or_default();

    // 3. Persona fan-out — parallel Direct calls per persona
    //    Get the provider from the router's provider field (access pattern
    //    depends on ExecutionRouter internals — read the struct to confirm).
    //    Alternative: pass the provider as a param from AgentLoop, which
    //    already has access to the LLM provider via the builder.
    let persona_responses = squad::fan_out_personas(
        &self.provider,  // Added in Step 5 below
        &orchestrator_context,
        message,
        &resolved.personas,
        &self.config.chat_params(),  // Use PipelineConfig's params
        event_tx.as_ref(),
    ).await;

    // 4. Synthesis
    let synthesis_prompt = squad::build_synthesis_prompt(message, &persona_responses);
    let synthesis_messages = vec![
        Message::System { content: synthesis_prompt },
        Message::User { content: UserContent::Text("Synthesize now.".to_string()) },
    ];
    let synthesis = self.provider.chat(&synthesis_messages, None, &self.config.chat_params()).await?;

    // 5. Return both: synthesized as main content, multi-voice in metadata
    let multi_voice = squad::format_multi_voice(&persona_responses);
    Ok(RuntimeResult {
        content: synthesis.content,
        multi_voice: Some(multi_voice),
        persona_responses: Some(persona_responses),
        // ... other fields from orchestrator_result
    })
}
```

**Important notes for implementer:**
- `AgentRuntime` does NOT currently have a `self.provider` field. You must add one (see Step 5).
- `self.config.chat_params()` may not exist — check `PipelineConfig` for a method to build `ChatParams`. If it doesn't exist, use `providers::cognitive_chat_params(&config, 4096)` pattern from `insight.rs:177` — but this requires a `config::Config` reference, not `PipelineConfig`. You may need to thread a `DynProvider` + `ChatParams` into `AgentRuntime` from the builder, following the same pattern as `insight.rs` uses.
- `self.router.execute()` — check the actual `ExecutionRouter::execute` signature by reading `crates/agent/src/intent_pipeline/router.rs`. Adapt the call to match.
- **`RuntimeResult`** will need new fields for multi-voice. See Step 4.

- [ ] **Step 4: Extend RuntimeResult with multi-voice fields**

In the RuntimeResult struct (find it in `runtime.rs` or `types.rs`), add:

```rust
pub struct RuntimeResult {
    // ... existing fields ...
    /// Multi-voice formatted output (all personas, markdown).
    pub multi_voice: Option<String>,
    /// Raw per-persona responses for structured display.
    pub persona_responses: Option<Vec<(String, String)>>,
}
```

- [ ] **Step 5: Add SquadRepo + DynProvider to AgentRuntime**

`AgentRuntime` (`crates/agent/src/agent_runtime/runtime.rs:58-81`) currently has NO provider field and NO squad_repo field. Add both:

```rust
pub struct AgentRuntime {
    // ... existing fields (skill_catalog, skill_router, analyzer, context_engine, router, etc.) ...
    /// Squad repo for resolving squads in chat mode.
    squad_repo: Option<cognitive::SquadRepo>,
    /// LLM provider for squad persona fan-out calls (separate from ExecutionRouter's provider).
    squad_provider: Option<providers::DynProvider>,
}
```

Update the builder (`crates/agent/src/agent_loop/builder.rs`) to inject both from the existing `self.cognitive_provider` and `self.pool`. The builder already has `self.pool: Option<SqlitePool>` and `self.cognitive_provider: Option<DynProvider>`.

In `AgentRuntime::new()`, add the new params. In `run_squad_execution`, use `self.squad_provider` instead of `self.provider`.

For `ChatParams`, follow the same pattern as `insight.rs:177`: the implementer should pass a pre-built `ChatParams` from `providers::cognitive_chat_params(&config, 4096)` during AgentRuntime construction, or thread a `config::Config` reference.

- [ ] **Step 6: Set squad_id on RoutingContext in AgentLoop**

In `crates/agent/src/agent_loop/mod.rs` line ~413, look up the session's `squad_id` and set it on the RoutingContext:

```rust
let mut routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());
// Check if this session has a squad context
if let Ok(Some(session)) = self.session_manager.get_session(session_key.as_str()).await {
    if let Some(ref sid) = session.squad_id {
        routing_ctx.squad_id = Some(sid.clone());
    }
}
```

- [ ] **Step 7: Verify compilation**

Run: `cargo build -p agent -p app-core 2>&1 | tail -20`

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/agent_runtime/ crates/agent/src/agent_loop/
git commit -m "feat(agent): integrate SquadExecutor into AgentRuntime for squad chat mode"
```

---

## Task 6: Chat Send — Thread squad_id Through IPC

**Files:**
- Modify: `crates/desktop-shared/src/commands/chat.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`
- Modify: `crates/app-core/src/handlers/chat/threads.rs`
- Modify: `crates/desktop/src/commands/chat.rs:37-52`

- [ ] **Step 1: Add squad_id to chat IPC types**

In `crates/desktop-shared/src/commands/chat.rs` (or wherever `ChatThreadResponse` lives):

Add `squad_id: Option<String>` to `ChatThreadResponse`.
Add `squad_id: Option<String>` to the chat send params or `SessionContextInput`.
Add `persona_id: Option<String>` and `persona_name: Option<String>` to `ChatMessageResponse` — needed for multi-voice rendering when reloading a thread.

- [ ] **Step 2: Update chat_send handler**

In `crates/app-core/src/handlers/chat/streaming.rs`, update `chat_send()` to:
1. Accept `squad_id` parameter
2. If `squad_id` is provided, set it on the session via `SessionRepo::set_squad_id()`
3. Pass through to `AgentLoop::process_message()`

- [ ] **Step 3: Update chat_threads handler**

In `threads.rs`, include `squad_id` in the `ChatThreadResponse` when listing threads. If a session has `squad_id`, resolve the squad name/icon via `SquadRepo` and include in the response.

- [ ] **Step 4: Update Tauri command**

In `crates/desktop/src/commands/chat.rs`, pass `squad_id` from the `chat_send` command to the AppCore handler.

- [ ] **Step 5: Handle PersonaPerspective events in streaming relay**

In the streaming relay (`streaming.rs`), handle the new `AgentEvent::PersonaPerspective` event — emit it as a Tauri/SSE event so the frontend can render multi-voice messages.

- [ ] **Step 6: Verify compilation**

Run: `cargo build -p desktop 2>&1 | tail -10`

- [ ] **Step 7: Commit**

```bash
git add crates/desktop-shared/ crates/app-core/src/handlers/chat/ crates/desktop/src/commands/chat.rs
git commit -m "feat(chat): thread squad_id through chat IPC layer and emit persona events"
```

---

## Task 7: Frontend — PersonaMessage Component

**Files:**
- Create: `desktop-ui/src/features/chat/components/PersonaMessage.tsx`

- [ ] **Step 1: Create PersonaMessage component**

A chat message bubble that shows which persona is speaking:

```tsx
interface PersonaMessageProps {
  personaName: string;
  personaIcon: string;
  personaRole: string;
  content: string;
}

export function PersonaMessage({ personaName, personaIcon, personaRole, content }: PersonaMessageProps) {
  return (
    <div className="flex gap-2 py-2">
      <div className="shrink-0 w-7 h-7 rounded-full bg-purple/10 flex items-center justify-center text-sm">
        {personaIcon}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-1.5 mb-1">
          <span className="text-[11px] font-medium text-foreground">{personaName}</span>
          <span className="text-[9px] text-dim">{personaRole}</span>
        </div>
        <div className="text-[12px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/components/PersonaMessage.tsx
git commit -m "feat(ui): add PersonaMessage component for multi-voice chat rendering"
```

---

## Task 8: Frontend — VoiceToggle + SquadChatHeader

**Files:**
- Create: `desktop-ui/src/features/chat/components/VoiceToggle.tsx`
- Create: `desktop-ui/src/features/chat/components/SquadChatHeader.tsx`

- [ ] **Step 1: Create VoiceToggle**

A simple toggle between "Multi-voice" and "Synthesized" display modes:

```tsx
export type VoiceMode = "multi" | "synthesized";

interface VoiceToggleProps {
  mode: VoiceMode;
  onChange: (mode: VoiceMode) => void;
}
```

Two small buttons/tabs — "Multi" (Users icon) and "Merged" (GitMerge icon). Use `text-[10px]` sizing.

- [ ] **Step 2: Create SquadChatHeader**

Shows the active squad's icon, name, and member list at the top of a squad chat thread:

```tsx
interface SquadChatHeaderProps {
  squadId: string;
}
```

Uses `useSquads()` internally to resolve the squad info. Shows: icon + name + member chips.

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/chat/components/VoiceToggle.tsx desktop-ui/src/features/chat/components/SquadChatHeader.tsx
git commit -m "feat(ui): add VoiceToggle and SquadChatHeader for squad chat mode"
```

---

## Task 9: Frontend — useAgentStream Persona Events + useChatSession squad_id

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useAgentStream.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useChatSession.ts`

- [ ] **Step 1: Add persona message state to useAgentStream**

Extend the stream store to accumulate `PersonaPerspective` events into a `personaMessages` array:

```typescript
interface PersonaSegment {
  personaId: string;
  personaName: string;
  content: string;
}
```

When a `chat:persona-perspective` event arrives, append to `personaMessages`.

- [ ] **Step 2: Expose personaMessages from useAgentStream**

Add `personaMessages: PersonaSegment[]` to the return type.

- [ ] **Step 3: Update useChatSession to pass squad_id**

In `useChatSession.ts`, accept an optional `squadId` parameter and pass it in the `chat_send` IPC call:

```typescript
export function useChatSession(sessionKey: string, squadId?: string, onDone?: () => void): ChatSession {
  // ...
  const send = useCallback(async (extraPayload?: Record<string, unknown>) => {
    // ...
    await ipc<ChatMessage>("chat_send", {
      content: text,
      sessionKey,
      squadId,  // NEW
      ...extraPayload,
    });
  }, [input, sessionKey, squadId, stream]);
}
```

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/
git commit -m "feat(ui): add persona events to agent stream and squad_id to chat session"
```

---

## Task 10: Frontend — SidebarChat "New Squad Chat" + ChatPage Integration

**Files:**
- Modify: `desktop-ui/src/features/chat/components/SidebarChat.tsx`
- Modify: `desktop-ui/src/features/chat/components/ChatPage.tsx`

- [ ] **Step 1: Read current SidebarChat and ChatPage**

Read both files to understand:
- How new threads are created
- How the thread list renders
- How ChatPage renders messages

- [ ] **Step 2: Add "New Squad Chat" to SidebarChat**

Add a dropdown or button next to the existing "New Chat" button. When clicked, opens a squad picker (reuse `SquadPicker` component). On squad selection, creates a new session with `squad_id` set.

Squad threads in the sidebar should show the squad icon and name as a badge.

- [ ] **Step 3: Update ChatPage for multi-voice rendering**

When the current thread has a `squad_id`:
1. Show `SquadChatHeader` at the top
2. Show `VoiceToggle` in the chat toolbar
3. In multi-voice mode, render `PersonaMessage` components for each persona response
4. In synthesized mode, render the normal assistant message

The display mode is local state (`useState<VoiceMode>("multi")`) — defaults to multi-voice.

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/components/
git commit -m "feat(ui): add squad chat creation in sidebar and multi-voice rendering in ChatPage"
```

---

## Task 11: Integration Test + Cleanup

**Files:**
- All modified files from Tasks 0-10

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 2: Run full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: All tests pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: No new warnings.

- [ ] **Step 4: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 5: Smoke test**

Run: `cd desktop-ui && bun run dev` (+ `cargo tauri dev`)
Test flow:
1. Open Chat
2. Click "New Squad Chat" → select Finance Analysis squad
3. Send a message about investment strategy
4. Verify: SquadChatHeader shows squad info
5. Verify: Multi-voice mode shows 3 persona cards (Deep Analyst, Risk Reviewer, Strategist)
6. Toggle to Synthesized mode → single merged response
7. Send follow-up → personas remember context

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(squads): Phase 2 complete — squad chat mode with multi-voice and synthesized responses"
```
