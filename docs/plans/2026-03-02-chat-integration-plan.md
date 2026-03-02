# Chat Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the mocked desktop chat with a fully functional AI agent — embedding AgentLoop in Tauri, adding session categorization, persona system, and sidebar context chat.

**Architecture:** Single Tauri process hosts AgentLoop + Channels + CronService. Desktop chat uses `process_direct_streaming()` for in-process streaming. Sessions are categorized by generic `entity_kind` + cached PARA ancestry. Personas are `.md` files with cascading inheritance. Sidebar auto-detects page context.

**Tech Stack:** Rust (Tauri v2, sqlx, tokio), React/TypeScript (Vite), SQLite, LanceDB

---

## Task 1: Add `session_context` Table

**Files:**
- Create: `crates/storage/migrations/002_session_context.sql`
- Modify: `crates/storage/src/repos/mod.rs` (add SessionContextRepo to Repos)
- Create: `crates/storage/src/repos/session_context.rs`
- Create: `crates/storage/src/rows/session_context.rs`
- Modify: `crates/storage/src/rows/mod.rs` (add module)
- Test: inline `#[cfg(test)] mod tests` in `session_context.rs`

**Step 1: Write the migration**

```sql
-- crates/storage/migrations/002_session_context.sql
CREATE TABLE IF NOT EXISTS session_context (
    session_key  TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    context_type TEXT NOT NULL DEFAULT 'general',
    entity_kind  TEXT,
    entity_id    TEXT,
    area_id      TEXT,
    project_id   TEXT,
    is_ephemeral INTEGER NOT NULL DEFAULT 0,
    is_pinned    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_session_context_area ON session_context(area_id);
CREATE INDEX IF NOT EXISTS idx_session_context_entity ON session_context(entity_kind, entity_id);
CREATE INDEX IF NOT EXISTS idx_session_context_ephemeral ON session_context(is_ephemeral, is_pinned);
```

**Step 2: Write the row type**

```rust
// crates/storage/src/rows/session_context.rs
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct SessionContextRow {
    pub session_key: String,
    pub context_type: String,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub is_ephemeral: bool,
    pub is_pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}
```

**Step 3: Write failing tests for SessionContextRepo**

Test: upsert, get, update, delete, list_by_area, list_visible (non-ephemeral OR pinned), cleanup_ephemeral.

**Step 4: Implement SessionContextRepo**

Methods: `upsert`, `get`, `update`, `delete`, `list_by_area`, `list_visible`, `cleanup_old_ephemeral(days: i64)`, `pin(session_key)`.

**Step 5: Run tests**

```bash
cargo nextest run -p storage -E 'test(session_context)'
```

**Step 6: Wire into Repos aggregate**

Add `pub session_context: SessionContextRepo` to `Repos` struct in `crates/storage/src/repos/mod.rs`. Initialize in `from_pool()`.

**Step 7: Run full storage tests + commit**

```bash
cargo nextest run -p storage
git add crates/storage/ && git commit -m "feat(storage): add session_context table and repo"
```

---

## Task 2: PersonaManager — Load and Parse `.md` Personas

**Files:**
- Create: `crates/agent/src/persona.rs`
- Modify: `crates/agent/src/lib.rs` (add `pub mod persona`)
- Test: inline `#[cfg(test)] mod tests` in `persona.rs`

**Step 1: Write failing tests**

Test cases:
- Parse YAML frontmatter from persona markdown (scope, skills, tone, name)
- `load_all()` reads `~/.klyntbot/personas/*.md`
- `for_session()` returns cascaded chain: global → area → project/feature
- Scope resolution: `scope.project: "CryptoGuard"` matches by name via DB lookup
- Empty personas dir returns empty manager

**Step 2: Implement PersonaManager**

```rust
// crates/agent/src/persona.rs

pub struct Persona {
    pub name: String,
    pub instructions: String,       // markdown body after frontmatter
    pub scope: PersonaScope,
    pub skills: Vec<String>,        // skill names to activate
    pub tone: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub enum PersonaScope {
    Global,                         // no scope in frontmatter
    Area { name: String },
    Project { name: String },
    Feature { kind: String },       // "finance", "finance.budgets", "habit"
}

pub struct PersonaChain {
    pub personas: Vec<Persona>,     // ordered: global first, most specific last
    pub merged_skills: HashSet<String>,
    pub merged_instructions: String,
    pub tone: Option<String>,
}

pub struct PersonaManager {
    personas: Vec<Persona>,
    resolved_scopes: HashMap<String, ResolvedScope>,  // persona name → entity IDs
}

impl PersonaManager {
    pub async fn load(personas_dir: &Path) -> Self
    pub async fn resolve_scopes(&mut self, repos: &Repos)
    pub fn for_session(&self, context: &SessionContextRow) -> PersonaChain
    pub fn reload(&mut self, personas_dir: &Path)  // hot-reload
}
```

Frontmatter parsing: reuse the pattern from `skills.rs` (lines 166-249) — split on `---`, parse YAML.

**Step 3: Run tests**

```bash
cargo nextest run -p agent -E 'test(persona)'
```

**Step 4: Commit**

```bash
git add crates/agent/src/persona.rs crates/agent/src/lib.rs
git commit -m "feat(agent): add PersonaManager for .md file personas"
```

---

## Task 3: PersonaContextSource and PageContextSource

**Files:**
- Create: `crates/agent/src/context_sources/persona.rs`
- Create: `crates/agent/src/context_sources/page_context.rs`
- Modify: `crates/agent/src/context_sources/mod.rs` (register new sources)
- Modify: `crates/agent/src/context_sources/skills.rs` (add persona skill filtering)
- Test: inline tests in each new file

**Step 1: Write PersonaContextSource**

```rust
// crates/agent/src/context_sources/persona.rs
// Priority: 95

pub struct PersonaContextSource {
    persona_manager: Arc<PersonaManager>,
    session_context_repo: SessionContextRepo,
}

impl ContextSource for PersonaContextSource {
    fn name(&self) -> &str { "persona" }
    fn priority(&self) -> u8 { 95 }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // 1. Look up session_context for ctx.chat_id (session_key = channel:chat_id)
        // 2. Call persona_manager.for_session(&context)
        // 3. Return merged_instructions if non-empty
    }
}
```

**Step 2: Write PageContextSource**

```rust
// crates/agent/src/context_sources/page_context.rs
// Priority: 90

pub struct PageContextSource {
    repos: Repos,
}

impl ContextSource for PageContextSource {
    fn name(&self) -> &str { "page_context" }
    fn priority(&self) -> u8 { 90 }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // 1. Look up session_context for this session
        // 2. If entity_kind is set, load entity details:
        //    - project: name, status, tasks (with IDs), OKRs
        //    - task: full task details, subtasks, dependencies
        //    - objective: KRs with progress, linked tasks
        //    - finance.*: relevant finance data for sub-category
        //    - area: projects list, task count
        // 3. Format as structured context string
        // 4. Return None for general/unscoped sessions
    }
}
```

**Step 3: Modify SkillContentSource to accept persona filter**

In `crates/agent/src/context_sources/skills.rs`, add optional persona skill filter. When a persona is active, only inject skills listed in the persona's `skills` array. Fallback to current behavior (all always-loaded skills) when no persona.

This requires `SkillContentSource` to access `PersonaManager` + `SessionContextRepo` to resolve the current session's persona skills.

**Step 4: Run tests + commit**

```bash
cargo nextest run -p agent -E 'test(persona)' -E 'test(page_context)'
git add crates/agent/src/context_sources/
git commit -m "feat(agent): add PersonaContextSource and PageContextSource"
```

---

## Task 4: Add Skill Scoping

**Files:**
- Modify: `crates/agent/src/skills.rs` (add scope parsing + `skills_for_persona()`)
- Modify: `skills/finance/SKILL.md` (add scope frontmatter)
- Test: inline tests in `skills.rs`

**Step 1: Extend skill metadata parsing**

Add optional `scope` field to skill YAML frontmatter. Same format as personas:

```yaml
scope:
  feature: finance
```

**Step 2: Add `skills_for_persona()` method**

```rust
impl SkillManager {
    /// Returns skills that match the given persona skill names,
    /// plus any globally-scoped skills without a scope field.
    pub fn skills_for_context(
        &self,
        persona_skills: &HashSet<String>,
        session_context: Option<&SessionContextRow>,
    ) -> Vec<&Skill> {
        // 1. If no persona skills and no session context → return all (current behavior)
        // 2. Filter: include skill if:
        //    a. skill.name is in persona_skills, OR
        //    b. skill has no scope (global), OR
        //    c. skill.scope matches session_context.entity_kind
    }
}
```

**Step 3: Add scope to finance skill**

```yaml
# skills/finance/SKILL.md — add to frontmatter:
scope:
  feature: finance
```

**Step 4: Bundle missing skills**

Add `browser`, `finance`, `weekly-report` to `BUILTIN_SKILLS` array in `skills.rs` (currently missing from compile-time bundling).

**Step 5: Run tests + commit**

```bash
cargo nextest run -p agent -E 'test(skill)'
git add crates/agent/src/skills.rs skills/
git commit -m "feat(agent): add skill scoping and bundle missing skills"
```

---

## Task 5: Merge `serve` into Desktop AppCore

**Files:**
- Modify: `crates/desktop/src/app_core.rs` (major rewrite — add AgentLoop, Bus, Channels, Cron)
- Modify: `crates/desktop/src/main.rs` (update setup hook for async init)
- Modify: `crates/desktop/Cargo.toml` (add dependencies: agent, bus, channels, scheduling, providers, config, context_engine)
- Delete: `crates/cli/src/serve.rs` (merge logic into app_core)
- Modify: `crates/cli/src/commands.rs` (remove serve subcommand)
- Modify: `crates/cli/src/main.rs` (remove serve match arm)

**Step 1: Update desktop Cargo.toml dependencies**

Add workspace dependencies that `serve.rs` uses: `agent`, `bus`, `channels`, `scheduling`, `providers`, `config`, `context_engine`, `session`, `tools`, `tools-core`.

**Step 2: Rewrite AppCore**

```rust
// crates/desktop/src/app_core.rs

pub struct AppCore {
    pub repos: Repos,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub persona_manager: Arc<PersonaManager>,
    // Private — managed internally
    channel_manager: Arc<TokioMutex<ChannelManager>>,
    cron_service: Arc<CronService>,
    shutdown_token: CancellationToken,
}

impl AppCore {
    pub async fn init() -> Result<Self, String> {
        // 1. Load config (same as serve.rs lines 20-32)
        // 2. Connect StoragePool + VectorStore
        // 3. Create Repos
        // 4. Create MessageBus (capacity 100)
        // 5. Initialize provider
        // 6. Start CronService + register jobs (port from serve.rs lines 48-264)
        // 7. Load PersonaManager from ~/.klyntbot/personas/
        // 8. Build AgentLoop via builder (port from serve.rs lines 496-501)
        //    - Add PersonaContextSource and PageContextSource to context sources
        // 9. Create ChannelManager
        // 10. Return Self
    }

    pub async fn start_background_services(&self) {
        // Spawn: agent loop, channel manager, heartbeat
        // (port from serve.rs lines 549-566)
    }

    pub async fn shutdown(&self) {
        // Graceful shutdown (port from serve.rs shutdown logic)
    }
}
```

**Step 3: Update main.rs setup hook**

The Tauri setup hook calls `AppCore::init()` and `start_background_services()`. Store in Tauri managed state.

**Step 4: Delete serve.rs and remove CLI subcommand**

Remove `serve` from `crates/cli/src/commands.rs` enum and `crates/cli/src/main.rs` match. Delete `crates/cli/src/serve.rs`.

**Step 5: Build and verify**

```bash
cargo build --workspace
cargo nextest run --workspace
```

**Step 6: Commit**

```bash
git add crates/desktop/ crates/cli/
git commit -m "feat(desktop): embed AgentLoop in Tauri, remove serve command"
```

---

## Task 6: Wire `chat_send` to AgentLoop with Streaming

**Files:**
- Modify: `crates/desktop/src/commands/chat.rs` (rewrite chat_send, implement chat_cancel)
- Modify: `crates/desktop-shared/src/events.rs` (add new event types)
- Modify: `crates/desktop-shared/src/commands.rs` (add ChatSendRequest with context)
- Modify: `crates/desktop/src/main.rs` (pass AppHandle to chat commands for emit)

**Step 1: Add context to ChatSendRequest**

```rust
// crates/desktop-shared/src/commands.rs

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendRequest {
    pub content: String,
    pub session_key: String,
    pub context: Option<SessionContextInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextInput {
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub context_type: Option<String>,
    pub is_ephemeral: Option<bool>,
}
```

**Step 2: Add streaming event types**

```rust
// crates/desktop-shared/src/events.rs — add:

pub const AGENT_TOOL_START: &str = "agent:tool_start";
pub const AGENT_TOOL_END: &str = "agent:tool_end";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_ENTITY_CREATED: &str = "agent:entity_created";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStartPayload {
    pub session_key: String,
    pub name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEndPayload {
    pub session_key: String,
    pub name: String,
    pub success: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorPayload {
    pub session_key: String,
    pub message: String,
}
```

**Step 3: Rewrite chat_send**

```rust
// crates/desktop/src/commands/chat.rs

#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppCore>,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> Result<ChatMessageResponse, String> {
    // 1. Upsert session + session_context (if context provided)
    // 2. Store user message in DB
    // 3. Resolve PARA ancestry for session_context (area_id, project_id)
    // 4. Call agent.process_direct_streaming(content, session_key)
    // 5. Spawn background task to read StreamingHandle.event_rx:
    //    - AgentEvent::ContentChunk → app.emit(AGENT_CONTENT_CHUNK, payload)
    //    - AgentEvent::ToolStart → app.emit(AGENT_TOOL_START, payload)
    //    - AgentEvent::ToolEnd → app.emit(AGENT_TOOL_END, payload)
    //    - AgentEvent::EntityCreated → app.emit(AGENT_ENTITY_CREATED + entity:updated)
    //    - AgentEvent::Done → app.emit(AGENT_DONE, payload)
    //    - AgentEvent::Error → app.emit(AGENT_ERROR, payload)
    // 6. Auto-detect session_context if not provided (from tool usage in response)
    // 7. Return the user message immediately (streaming handles the AI response)
}
```

**Step 4: Implement chat_cancel**

```rust
#[tauri::command]
pub async fn chat_cancel(
    state: tauri::State<'_, AppCore>,
    session_key: String,
) -> Result<(), String> {
    // Look up active StreamingHandle for session_key
    // Call cancel_token.cancel()
    // Store active handles in AppCore via DashMap<String, CancellationToken>
}
```

**Step 5: Build + test manually**

```bash
cargo build -p desktop
cd desktop-ui && bun run dev  # test in browser (mock mode)
```

**Step 6: Commit**

```bash
git add crates/desktop/ crates/desktop-shared/
git commit -m "feat(desktop): wire chat_send to AgentLoop with streaming events"
```

---

## Task 7: Frontend — Real Streaming Chat

**Files:**
- Modify: `desktop-ui/src/components/views/Chat.tsx` (real streaming, remove mocks)
- Modify: `desktop-ui/src/components/chat/ChatPanel.tsx` (same streaming integration)
- Modify: `desktop-ui/src/lib/types.ts` (add streaming types, session context types)
- Create: `desktop-ui/src/hooks/useAgentStream.ts` (shared streaming hook)
- Create: `desktop-ui/src/components/chat/MessageList.tsx` (extracted shared component)

**Step 1: Add types**

```typescript
// desktop-ui/src/lib/types.ts — add:

export interface SessionContext {
  entityKind?: string;
  entityId?: string;
  contextType?: string;
  isEphemeral?: boolean;
}

export interface ContentChunkPayload {
  sessionKey: string;
  data: string;
}

export interface ToolStartPayload {
  sessionKey: string;
  name: string;
}

export interface ToolEndPayload {
  sessionKey: string;
  name: string;
  success: boolean;
}

export interface AgentDonePayload {
  sessionKey: string;
  content: string;
}

export interface AgentErrorPayload {
  sessionKey: string;
  message: string;
}

// Update ChatThread to include context
export interface ChatThread {
  sessionKey: string;
  title: string;
  messageCount: number;
  updatedAt: string;
  projectId?: string;
  // New fields:
  contextType?: string;
  entityKind?: string;
  entityId?: string;
  areaId?: string;
  areaName?: string;
  projectName?: string;
}
```

**Step 2: Create useAgentStream hook**

```typescript
// desktop-ui/src/hooks/useAgentStream.ts

export function useAgentStream(sessionKey: string) {
  const [streamingContent, setStreamingContent] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [activeTools, setActiveTools] = useState<string[]>([]);

  // Listen to agent:content_chunk — append data to streamingContent
  useEvent<ContentChunkPayload>('agent:content_chunk', (payload) => {
    if (payload.sessionKey === sessionKey) {
      setStreamingContent(prev => prev + payload.data);
    }
  });

  // Listen to agent:done — finalize message, reset streaming state, refetch messages
  useEvent<AgentDonePayload>('agent:done', (payload) => {
    if (payload.sessionKey === sessionKey) {
      setStreamingContent('');
      setIsStreaming(false);
    }
  });

  // Listen to agent:tool_start/end — track active tools for UI indicators
  useEvent<ToolStartPayload>('agent:tool_start', (payload) => { ... });
  useEvent<ToolEndPayload>('agent:tool_end', (payload) => { ... });

  // Listen to agent:error
  useEvent<AgentErrorPayload>('agent:error', (payload) => { ... });

  return { streamingContent, isStreaming, setIsStreaming, activeTools };
}
```

**Step 3: Extract MessageList component**

Extract the message rendering loop from `Chat.tsx` (lines 287-315) into a shared `MessageList` component used by both Chat and SidebarChat.

**Step 4: Update Chat.tsx**

- Remove `mockChatProjects`, `mockMessages`, `localMessages` state
- Remove the `setTimeout` mock AI response (lines 150-159)
- Use `useAgentStream(selectedThread)` for streaming
- Show `streamingContent` as a partial assistant message during streaming
- Show active tool indicators during tool execution
- `handleSend` calls `chat_send` with context from scope selector (Task 9)
- Refetch messages after `agent:done`

**Step 5: Update ChatPanel.tsx similarly**

Same streaming integration, using `useAgentStream('desktop-panel')`.

**Step 6: Test in dev mode + commit**

```bash
cd desktop-ui && bun run dev  # verify no regressions in browser mock mode
git add desktop-ui/src/
git commit -m "feat(desktop-ui): real-time streaming chat with agent events"
```

---

## Task 8: Frontend — Thread List with PARA Grouping

**Files:**
- Modify: `desktop-ui/src/components/views/Chat.tsx` (thread sidebar redesign)
- Modify: `crates/desktop/src/commands/chat.rs` (join session_context in chat_threads)
- Modify: `crates/desktop-shared/src/commands.rs` (extend ChatThreadResponse)

**Step 1: Extend ChatThreadResponse with context fields**

```rust
// crates/desktop-shared/src/commands.rs — update ChatThreadResponse:

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    pub updated_at: String,
    // New context fields:
    pub context_type: Option<String>,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
}
```

**Step 2: Update chat_threads query**

```rust
// crates/desktop/src/commands/chat.rs — update chat_threads:
// LEFT JOIN session_context ON sessions.key = session_context.session_key
// LEFT JOIN areas ON session_context.area_id = areas.id
// LEFT JOIN projects ON session_context.project_id = projects.id
// WHERE session_context.is_ephemeral = 0 OR session_context.is_pinned = 1 OR session_context IS NULL
```

**Step 3: Frontend grouping logic**

```typescript
// In Chat.tsx — replace threadsByProject with threadsByHierarchy:

const FEATURE_GROUPS: Record<string, { label: string, icon: any }> = {
  'finance': { label: 'Finance', icon: Wallet },
  // future features add a line here
};

const groupedThreads = useMemo(() => {
  // Group 1: PARA hierarchy (area_id set)
  //   → Map<areaName, Map<projectName | null, ChatThread[]>>
  // Group 2: Feature threads (entity_kind prefix, no area_id)
  //   → Map<featurePrefix, Map<subCategory, ChatThread[]>>
  // Group 3: General (no context)
  //   → ChatThread[]
}, [threads]);
```

**Step 4: Render grouped thread sidebar**

Collapsible area groups → project sub-groups → thread items. Feature groups with sub-category nesting. General at the bottom.

**Step 5: Add "New thread" with optional scope selector**

Dropdown or modal when creating a new thread: choose Area, Project, or leave as General. Sets the `session_context` on creation.

**Step 6: Test + commit**

```bash
cargo build -p desktop
cd desktop-ui && bun run dev
git add crates/desktop/ crates/desktop-shared/ desktop-ui/src/
git commit -m "feat(desktop): PARA-grouped thread list with session context"
```

---

## Task 9: Sidebar Context Chat

**Files:**
- Create: `desktop-ui/src/components/chat/SidebarChat.tsx`
- Create: `desktop-ui/src/hooks/usePageContext.ts`
- Modify: `desktop-ui/src/components/views/MainApp.tsx` (replace ChatPanel with SidebarChat)
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx` (add sidebar toggle)
- Modify: `desktop-ui/src/components/views/TaskDetail.tsx` (add sidebar toggle)
- Modify: `desktop-ui/src/components/views/ObjectiveDetail.tsx` (add sidebar toggle)
- Modify: `desktop-ui/src/App.tsx` (add sidebar layout wrapper)

**Step 1: Create usePageContext hook**

```typescript
// desktop-ui/src/hooks/usePageContext.ts

import { useLocation, useParams } from 'react-router-dom';
import type { SessionContext } from '../lib/types';

export function usePageContext(): SessionContext | null {
  const location = useLocation();
  const params = useParams();

  // /project/:id → { entityKind: 'project', entityId: params.id }
  // /task/:id → { entityKind: 'task', entityId: params.id }
  // /objective/:id → { entityKind: 'objective', entityId: params.id }
  // /finance/budgets → { entityKind: 'finance.budgets' }
  // /finance/investments → { entityKind: 'finance.investments' }
  // /finance/... → { entityKind: 'finance.*' }
  // / → null (dashboard, general)
  // /chat → null (sidebar hidden)
}
```

**Step 2: Create SidebarChat component**

```typescript
// desktop-ui/src/components/chat/SidebarChat.tsx

export function SidebarChat({ isOpen, onClose }: Props) {
  const pageContext = usePageContext();
  const sessionKey = pageContext
    ? `sidebar:${pageContext.entityKind}:${pageContext.entityId || 'all'}`
    : 'desktop-panel';

  const { streamingContent, isStreaming, setIsStreaming, activeTools } =
    useAgentStream(sessionKey);
  const messages = useQuery<ChatMessage[]>('chat_messages', { session_key: sessionKey });
  const sendMessage = useMutation('chat_send');

  const handleSend = async (content: string) => {
    setIsStreaming(true);
    await sendMessage.mutate({
      content,
      session_key: sessionKey,
      context: pageContext ? { ...pageContext, isEphemeral: true } : undefined,
    });
  };

  const handlePin = async () => {
    // Call new IPC command: chat_pin_thread({ session_key: sessionKey })
    // Shows toast: "Thread pinned — visible in Chat"
  };

  return (
    // Header: entity name + icon + pin button
    // MessageList (shared component)
    // Streaming indicator + active tools
    // Input area
  );
}
```

**Step 3: Add sidebar toggle to all detail pages**

Add a chat icon button (bottom-right FAB or header icon) that toggles the sidebar. Use a shared layout wrapper or context provider so the sidebar persists across page navigation.

**Step 4: Add `chat_pin_thread` IPC command**

```rust
// crates/desktop/src/commands/chat.rs
#[tauri::command]
pub async fn chat_pin_thread(
    state: tauri::State<'_, AppCore>,
    session_key: String,
) -> Result<(), String> {
    state.repos.session_context.pin(&session_key).await.map_err(|e| e.to_string())
}
```

**Step 5: Add ephemeral cleanup to session cleanup service**

Add `cleanup_old_ephemeral(7)` call to the existing `SessionCleanupService` in `crates/agent/src/agent_loop/builder.rs`.

**Step 6: Test + commit**

```bash
cd desktop-ui && bun run dev
git add desktop-ui/src/ crates/desktop/ crates/desktop-shared/
git commit -m "feat(desktop-ui): add Cursor-style sidebar chat with page context"
```

---

## Task 10: Register Finance IPC Commands

**Files:**
- Modify: `crates/desktop/src/main.rs` (register finance commands)
- Create: `crates/desktop/src/commands/finance.rs` (finance IPC handlers)
- Modify: `crates/desktop/src/commands/mod.rs` (add finance module)
- Remove: `desktop-ui/src/data/mockFinanceData.ts` (no longer needed)

**Step 1: Implement finance commands**

Map the frontend's expected calls (`finance_accounts`, `finance_transactions`, `finance_budget_usage`, `finance_portfolios`, `finance_investments`, `finance_goals`, `finance_liabilities`, `finance_net_worth`) to queries against `FinanceStorage` repos.

**Step 2: Register in main.rs**

Add all finance commands to the `invoke_handler!` macro call.

**Step 3: Remove mock data**

Delete `desktop-ui/src/data/mockFinanceData.ts` and update all finance views to remove fallback mock references.

**Step 4: Test + commit**

```bash
cargo build -p desktop
cd desktop-ui && bun run dev
git add crates/desktop/ desktop-ui/
git commit -m "feat(desktop): wire finance IPC commands, remove mock data"
```

---

## Task 11: Auto-Detect Session Context from Tool Usage

**Files:**
- Modify: `crates/desktop/src/commands/chat.rs` (post-response context detection)
- Modify: `crates/agent/src/events.rs` (add EntityCreated data for context)

**Step 1: Implement auto-detection logic**

After the agent finishes processing (on `AgentEvent::Done`), check which tools were called:
- If `task`/`project`/`okr`/`area` tools used → extract entity IDs from tool results
- If single entity domain → auto-set `session_context`
- If multiple domains → leave as `general`
- If session already has context → don't override

**Step 2: Use EntityCreated events**

The `AgentEvent::EntityCreated(EntityCard)` already carries entity kind + ID. Collect these during streaming and use them to infer context.

**Step 3: Resolve PARA ancestry**

When auto-detecting a project, look up its `area_id`. When detecting a task, look up its `area_id` and `project_id`. Cache in `session_context`.

**Step 4: Test + commit**

```bash
cargo nextest run -p desktop
git add crates/desktop/ crates/agent/
git commit -m "feat(desktop): auto-detect session context from tool usage"
```

---

## Task 12: Integration Testing and Cleanup

**Files:**
- Modify: `desktop-ui/src/components/views/Chat.tsx` (remove all remaining mock data)
- Modify: `desktop-ui/src/components/chat/ChatPanel.tsx` (remove or replace with SidebarChat)
- Modify: `CLAUDE.md` (update architecture docs)

**Step 1: Remove all mock data from frontend**

- Delete `mockChatProjects` from `Chat.tsx`
- Delete `mockMessages` from `Chat.tsx`
- Delete hardcoded AI response string from `Chat.tsx`
- Delete `mockFinanceData.ts` (if not done in Task 10)
- Remove `isTauri` guards that fall back to mock data in chat components

**Step 2: End-to-end manual test**

1. Run `cargo tauri dev` (starts desktop app with real agent)
2. Open `/chat`, create a new thread
3. Send a message → verify streaming response appears
4. Send "list my tasks" → verify tool execution + response
5. Open `/project/:id`, toggle sidebar → verify page context pre-loaded
6. Verify thread appears in correct PARA group
7. Pin a sidebar thread → verify it appears in `/chat` thread list

**Step 3: Update CLAUDE.md**

Update architecture section:
- Remove `klyntbot serve` references
- Document AppCore as the single entry point
- Add persona system documentation
- Add sidebar chat documentation
- Update extension traits table

**Step 4: Run full test suite**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
cd desktop-ui && bun run build
```

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat(desktop): complete chat integration with agent, personas, and sidebar"
```

---

## Dependency Graph

```
Task 1 (session_context table)
  ├── Task 2 (PersonaManager) ──→ Task 3 (Context Sources)
  │                                  ├── Task 4 (Skill Scoping)
  │                                  └── Task 5 (Merge serve → AppCore) ──→ Task 6 (Wire chat_send)
  │                                                                            ├── Task 7 (Frontend streaming)
  │                                                                            │     └── Task 8 (Thread grouping)
  │                                                                            │           └── Task 9 (Sidebar chat)
  │                                                                            └── Task 10 (Finance IPC)
  └── Task 11 (Auto-detect context) — depends on Task 6 + Task 1

Task 12 (Integration + cleanup) — depends on all above
```

Tasks 2, 3, 4 can be parallelized after Task 1.
Tasks 7 and 10 can be parallelized after Task 6.
Task 9 depends on Task 7 (needs MessageList + useAgentStream).
