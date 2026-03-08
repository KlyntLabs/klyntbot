# Klyntbot System Test Scenarios

> **Purpose:** Real-world test scenarios to verify all system layers connect properly.
> Generated: 2026-03-08 | Based on: SYSTEM_ANALYSIS.md + ARCHITECTURE_DIAGRAMS.md + codebase audit

---

## Pre-Test Verification

Before running scenarios, verify system status via Debug > System tab:

- [ ] Cognitive pipeline: LlmExtractionHandler active
- [ ] Embedding model: loaded (paraphrase-multilingual-MiniLM-L12-v2)
- [ ] Background consolidation: running
- [ ] Session manager: connected
- [ ] Cron service: running with N registered jobs

---

## Layer Map

Each scenario lists which architectural layers it exercises:

```
L0: common          L4: tools, features    L7: app-core, desktop
L1: config, bus     L5: agent, cognitive   L8: klyntbot
L2: storage         L6: mcp
L3: providers, session, context_engine
```

---

## T1: Cognitive Memory — Multi-Fact Extraction

**Layers:** L1 (bus) → L3 (providers, session) → L5 (agent, cognitive) → L2 (storage)

**Tests:** LLM extraction handler, consolidation, FSRS scoring, debug page display

### Steps

1. Start fresh session (New Thread)
2. Send: `My name is Jayden. I'm 25 years old, born in Ho Chi Minh City. I work as a software engineer at a startup. I love Rust and TypeScript, and I use Neovim with dark themes. I'm currently learning Go and Japanese.`
3. Wait 10 seconds for cognitive pipeline
4. Check Debug > Memory page

### Expected

- **7+ distinct facts** in Semantic Facts table:
  - `identity | user | name | Jayden`
  - `identity | user | age | 25`
  - `identity | user | birthplace | Ho Chi Minh City`
  - `work | user | occupation | software engineer`
  - `preferences | user | favorite_language | Rust` (or similar)
  - `preferences | user | favorite_editor | Neovim`
  - `learning | user | learning_language | Go`
- All with `confidence: 1.0`, `source: user_stated`
- User Model cards populated (identity, work, preferences, learning)

### Failure Indicates

- LLM extraction falling back to heuristic (check for single blob fact)
- DeepSeek JSON parsing issue (ResponseFormat compatibility)
- DomainEventBus not delivering ChatTurnCompleted events

---

## T2: Cross-Session Memory Recall

**Layers:** L3 (session, context_engine) → L5 (cognitive) → L2 (storage, vectors)

**Tests:** CognitiveContextSource, static/dynamic fact retrieval, FSRS access tracking

### Steps (after T1)

1. Start a NEW thread (different session)
2. Send: `What is my name?`
3. Verify response contains "Jayden"
4. Start another NEW thread
5. Send: `What programming languages do I use?`
6. Verify response mentions Rust, TypeScript, and possibly Go

### Expected

- Bot correctly recalls facts from T1 session
- Debug page shows `access_count` incrementing on recalled facts
- `stability` values increase for frequently accessed facts

### Failure Indicates

- Context engine skipping memory retrieval for Direct mode
- CognitiveContextSource not including "general" domain in retrieval
- Vector search not finding relevant facts

---

## T3: Fact Update / Consolidation

**Layers:** L5 (cognitive — consolidation handler) → L2 (storage)

**Tests:** LLM consolidation (UPDATE vs ADD vs NOOP), supersession logic

### Steps (after T1)

1. New thread, send: `Actually, I'm 26 now. And I switched from Neovim to VS Code recently.`
2. Wait 10 seconds
3. Check Debug > Memory

### Expected

- `age` fact updated from "25" → "26" (superseded_at set on old fact)
- `favorite_editor` updated from "Neovim" → "VS Code"
- Other facts (name, occupation, etc.) remain unchanged (NOOP)
- No duplicate facts created for unchanged information

### Failure Indicates

- Consolidation handler always choosing ADD instead of UPDATE
- Consolidation falling back to heuristic (which always adds)
- `find_similar()` not matching existing subject+predicate pairs

---

## T4: Question Filtering (No Fact Extraction from Questions)

**Layers:** L5 (cognitive — extraction handler)

**Tests:** Question detection in extraction prompt, empty array return

### Steps

1. New thread, clear facts first (Debug > delete all)
2. Send: `What's the weather like today?`
3. Wait 5 seconds
4. Send: `How do I install Rust?`
5. Check Debug > Memory

### Expected

- **Zero new facts** created from questions
- No "chat_stated" heuristic facts either

### Failure Indicates

- Extraction prompt not filtering questions
- Heuristic fallback running instead of LLM extraction

---

## T5: Task Management (ReAct Loop + Tool Execution)

**Layers:** L3 (providers) → L4 (feature-todo) → L5 (agent — reactive engine) → L2 (storage)

**Tests:** Intent classification → Reactive mode, TaskTool multi-action dispatch, tool execution transparency

### Steps

1. New thread, send: `Create a task called "Review pull requests" with priority high, due tomorrow`
2. Observe execution panel (Execution section in sidebar)
3. Send: `List all my tasks`
4. Send: `Mark "Review pull requests" as completed`

### Expected

- Step 1: Agent selects **task** agent, uses **Reactive** strategy
  - Tool call: `task.add` with appropriate params
  - Task created successfully
- Step 2: Execution panel shows `Engine: Reactive`, tool start/end events
- Step 3: `task.list` returns the created task
- Step 4: `task.update` marks it complete

### Failure Indicates

- Intent classification routing to Direct mode (no tools)
- Task agent not matching on keywords
- Tool parameter parsing errors
- Direct → Reactive escalation needed but not triggered

---

## T6: Direct Mode + Escalation

**Layers:** L5 (agent — direct engine, execution router)

**Tests:** Direct mode for simple queries, escalation to Reactive when tools needed

### Steps

1. New thread, send: `Hello, how are you?`
2. Check execution panel — should show `Engine: Direct`
3. Send: `What is 2 + 2?`
4. Should answer directly without tools
5. Send: `Search the web for latest Rust news`
6. Check execution panel — should show escalation or Reactive

### Expected

- Greetings → Direct mode (single LLM call)
- Simple math → Direct mode
- Web search → Reactive mode (WebSearchTool needed)
  - If initially classified as Direct, escalation triggers retry with tools

### Failure Indicates

- Intent classifier over-triggering Reactive for simple messages
- Escalation path not working (empty response from Direct)

---

## T7: Multi-Agent Delegation

**Layers:** L5 (agent — agent manager, delegation tool, agent runtime)

**Tests:** Orchestrator routing, DelegateTool, sub-agent execution, depth limiting

### Steps

1. New thread, send: `I need to plan my week: create tasks for Monday standup, Wednesday review, and Friday retro. Also check my budget for dining this month.`
2. Observe agent switching in execution panel

### Expected

- Initial routing to **general** (orchestrator) agent
- Delegation to **task** agent for task creation
- Delegation to **finance** agent for budget check
- Both delegations complete successfully
- `DelegationStarted` and `DelegationCompleted` events visible

### Failure Indicates

- DelegateTool not injected into general agent's tools
- Agent matching not finding specialized agents
- Delegation depth exceeded (shouldn't happen with depth=0→1)

---

## T8: Chain-of-Thought Planning (Complex Tasks)

**Layers:** L5 (agent — reactive engine, planning)

**Tests:** Complexity scoring ≥5, plan generation, step tracking

### Steps

1. New thread, send: `Research the latest developments in WebAssembly, find 3 key trends, create a task for each trend to investigate further, and summarize your findings`
2. Watch for plan generation in execution panel

### Expected

- Complexity score ≥5 triggers planning prompt
- `PlanGenerated` event with structured steps
- Multiple tool calls (web search + task creation)
- `PlanStepCompleted` events as steps finish
- Final synthesis includes plan progress

### Failure Indicates

- Complexity scoring too low for this query
- Planning prompt not injected at iteration 1
- Plan parsing failure (falls through to normal ReAct)

---

## T9: Scheduling / Reminders

**Layers:** L3 (scheduling) → L1 (bus) → L5 (agent)

**Tests:** CronTool, job creation, timer execution

### Steps

1. New thread, send: `Remind me in 2 minutes to check my email`
2. Wait for the reminder to fire (2+ minutes)
3. Send: `What reminders do I have?`

### Expected

- CronTool creates a one-shot (`At`) schedule
- After 2 minutes, reminder triggers → message appears
- List shows active/completed reminders

### Failure Indicates

- CronTool not registered in tool registry
- CronService timer loop not waking for new jobs
- Callback not publishing to message bus

---

## T10: Productivity Context

**Layers:** L4 (feature-productivity) → L1 (bus) → L5 (cognitive)

**Tests:** Productivity data in context, activity tracking integration

### Steps

1. Use the desktop app actively for a few minutes
2. New thread, send: `How productive have I been today?`
3. Check if response includes productivity metrics

### Expected

- Response includes active hours, focus time, productivity score
- Activity categories mentioned (if activity tracker is running)
- Context shows productivity data was retrieved

### Failure Indicates

- ProductivityTool not registered
- Activity tracker not running (macOS accessibility permissions needed)
- Productivity data not being injected into context

---

## T11: Finance Operations

**Layers:** L4 (feature-finance) → L2 (storage)

**Tests:** FinanceTool multi-action, account CRUD, transaction recording

### Steps

1. New thread, send: `Create a checking account called "Main Account" with a balance of $5000`
2. Send: `Add an expense of $45.50 for groceries from Main Account`
3. Send: `What's my current balance?`
4. Send: `Show me my spending summary`

### Expected

- Account created with initial balance
- Transaction recorded, balance updated
- Summary shows categorized spending

### Failure Indicates

- FinanceTool not enabled in config
- Finance migrations not applied
- PriceService initialization failure

---

## T12: Conversation History Persistence

**Layers:** L3 (session) → L2 (storage)

**Tests:** Session save/load, history in context across messages

### Steps

1. New thread, send: `Let's talk about Rust. What are its main advantages?`
2. Wait for response
3. Send: `What about its disadvantages?`
4. Verify response references "Rust" from previous context
5. Send: `Summarize our conversation so far`

### Expected

- Response 3 uses correct pronoun reference ("its" = Rust)
- Summary accurately recaps the multi-turn conversation
- Session persists all messages to SQLite

### Failure Indicates

- Session not saving messages between turns
- History not included in context assembly
- Token budget cutting off recent history

---

## T13: Adaptive Learning Feedback Loop

**Layers:** L5 (agent — learning, confidence evaluator)

**Tests:** Outcome recording, threshold adaptation, confidence gating

### Steps

1. Send several messages that trigger tool calls (task creation, web search)
2. Check Debug > System or Pipeline tab for learning stats
3. Look for `learning_outcomes` entries in database

### Expected

- Each tool execution records an outcome (success/failure, duration)
- `learning_state` table has threshold data
- Confidence threshold adjusts over time (±0.05 max per cycle)

### Failure Indicates

- OutcomeRecorder not wired
- LearningService hourly tick not running
- AdaptiveThresholds not persisting to database

---

## T14: Embedding-Based Semantic Search

**Layers:** L2 (storage — vector store) → L4 (tools) → L5 (cognitive)

**Tests:** Vector search for tasks and memory, cosine similarity, FSRS scoring

### Steps (after T5 — tasks exist)

1. New thread, send: `Find tasks related to code reviews`
2. Check if semantic search is used (not just keyword matching)
3. Send: `What do I know about my work projects?` (tests cognitive vector search)

### Expected

- Task search uses embedding similarity (not just SQL LIKE)
- Memory retrieval uses vector path (not fallback 0.5 similarity)
- Debug panel shows vector search was executed

### Failure Indicates

- Embedding model not loaded (fastembed initialization failed)
- VectorStore tables not created
- Search falling back to keyword-only mode

---

## T15: Error Recovery & Resilience

**Layers:** L3 (providers — circuit breaker, retry) → L5 (agent)

**Tests:** Provider failover, graceful degradation, heuristic fallbacks

### Steps

1. (Simulated) If possible, temporarily break the API key
2. Send a message and observe behavior
3. Restore the key and send another message

### Expected

- Circuit breaker opens after failures
- Cognitive handlers fall back to heuristic extraction
- System recovers after circuit breaker resets (60s)
- No crash or hang — graceful error messages

### Failure Indicates

- Circuit breaker not configured
- No fallback provider set
- Heuristic fallback not wired

---

## T16: Desktop UI Transparency

**Layers:** L7 (desktop, desktop-shared) → L5 (agent events)

**Tests:** Real-time event streaming, execution metadata display

### Steps

1. Send any message that triggers tools
2. Observe the execution sidebar panel

### Expected

- **Agents** section shows matched agent name
- **Execution** section shows:
  - Engine type (Direct/Reactive)
  - Strategy and confidence percentage
  - Iteration count (for Reactive)
- **Learning** section shows confidence threshold
- Tool start/end events appear in real-time
- Token usage and cost displayed after completion

### Failure Indicates

- AgentEvent channel not connected to Tauri emitter
- Event serialization mismatch
- Desktop-shared event types out of sync

---

## T17: Concurrent Multi-Turn Stress

**Layers:** All layers simultaneously

**Tests:** Session isolation, no cross-session contamination

### Steps

1. Open two chat threads side by side (if UI supports tabs)
2. In Thread A: `My favorite color is blue`
3. In Thread B: `My favorite color is red`
4. In Thread A: `What's my favorite color?`
5. In Thread B: `What's my favorite color?`

### Expected

- Thread A answers "blue" (from its own session context)
- Thread B answers "red" (from its own session context)
- Cognitive memory may show both facts (different sessions contribute to same user model)
- No cross-contamination within session history

### Failure Indicates

- Session manager not isolating sessions properly
- DashMap locking issues
- Shared mutable state between sessions

---

## T18: Notes Management (CRUD + Notebooks + Linking)

**Layers:** L4 (feature-notes) → L5 (agent — reactive engine) → L2 (storage)

**Tests:** NotesTool registration, note CRUD, notebooks, tagging, search, linking

### Steps

1. New thread, send: `Create a notebook called "Work Projects"`
2. Send: `Create a note called "Sprint Goals" in the Work Projects notebook with body "Ship v2.0 by March 15"`
3. Send: `Create another note called "API Design Notes" with tags "api", "architecture"`
4. Send: `Search my notes for "Sprint"`
5. Send: `Link "Sprint Goals" to "API Design Notes"`
6. Send: `List all my notebooks`
7. Send: `List notes in Work Projects`

### Expected

- Step 1: Notebook created with confirmation
- Step 2: Note created inside the notebook
- Step 3: Note created with tags
- Step 4: Search returns "Sprint Goals"
- Step 5: Notes linked successfully
- Step 6: Shows "Work Projects" with note count
- Step 7: Shows only "Sprint Goals" (filtered by notebook)

### Failure Indicates

- NotesTool not registered in tool registry
- Notes migrations not applied
- Agent not routing to Reactive mode for tool calls

---

## Known Gaps (Not Testable Yet)

| Gap | Impact | Status |
|-----|--------|--------|
| **No web chat channel** | Browser chat requires Tauri desktop or dev server | R14 in progress |
| **Weekly reflection untested** | Requires 7+ days of episodic data | Can test manually via API |

---

## Quick Smoke Test (5-Minute Verification)

Run these in order for a fast end-to-end check:

1. **T1** (multi-fact extraction) — verifies cognitive pipeline
2. **T2** (cross-session recall) — verifies memory retrieval
3. **T5** (task creation) — verifies tool execution
4. **T6 steps 1-2** (greeting) — verifies direct mode
5. **T12** (conversation continuity) — verifies session persistence

If all 5 pass, the core system is healthy.
