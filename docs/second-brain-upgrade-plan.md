# Second Brain Upgrade Plan

> Generated 2026-04-07 from deep analysis of Claude Code's memory architecture vs Klynt's cognitive system.

## Executive Summary

Klynt's cognitive architecture (structured SPO triples, vector+BM25 hybrid retrieval, FSRS-5 decay, 10-factor relevance scoring, mirror self-reflection, autotuner) is **fundamentally more sophisticated** than Claude Code's flat-file memory system. However, the **upstream pipeline is disconnected** — `ChatTurnCompleted` events were stripped of message content, blocking all chat-based fact extraction. The downstream systems are excellent but starved of input.

Claude Code's key advantage is its **multi-layered extraction strategy**: (1) main agent proactive saves, (2) per-turn forked LLM extraction subagent, (3) periodic cross-session consolidation ("autoDream"). If any layer misses something, another catches it.

---

## Claude Code Architecture Reference

### Memory System (3 Tiers)

| Tier | Trigger | What It Does | Key Files |
|------|---------|-------------|-----------|
| **Proactive saves** | During conversation | Main agent writes markdown files with YAML frontmatter to `~/.claude/projects/*/memory/` | System prompt instructions in `memdir.ts` |
| **Extract memories** | Post-turn (fire-and-forget) | Forked LLM subagent reads conversation tail, writes/updates memory files. Cursor-tracked (only new messages). Max 5 turns. Skipped if main agent already wrote. | `extractMemories.ts`, `prompts.ts` |
| **autoDream** | Post-turn, gated: ≥24h + ≥5 sessions + file lock | Consolidation subagent reads past session transcripts + existing memory files. 4-phase: orient → gather → merge → prune. | `autoDream.ts`, `consolidationPrompt.ts`, `consolidationLock.ts` |

### Other Intelligence Systems

| System | What It Does | Trigger |
|--------|-------------|---------|
| **Session memory** | Per-session markdown scratchpad (task state, files, errors, learnings). Capped at 12K tokens. | Post-sampling hook, token/tool-call thresholds |
| **Relevant memory prefetch** | Sonnet side-query selects ≤5 relevant memory files per user query from file manifest. | Pre-query, parallel with context assembly |
| **Prompt suggestion** | Predicts next user message (2-12 words), shown as ghost text. | Post-sampling hook |
| **Speculation** | Pre-executes predicted suggestion in background with CoW filesystem overlay. | After prompt suggestion generated |
| **Auto-compaction** | Session-memory-based compaction when near context limit. No LLM call needed. | `shouldAutoCompact()` in query loop |
| **Magic docs** | Auto-updates markdown files tagged `# MAGIC DOC:` based on conversation. | Post-sampling hook on file read |
| **Skill improvement** | Monitors corrections every 5 turns → suggests skill file rewrites. | Post-sampling hook |
| **Away summary** | 1-3 sentence recap on return from idle. | Terminal regains focus after idle |

### KAIROS (Assistant Mode)

Persistent, always-on agent mode with: cron scheduler, MCP push channels, GitHub webhooks, perpetual bridge (never exits), daily log files instead of direct topic writes, and its own dream cycle. Gated by `settings.json` `assistant: true` + GrowthBook `tengu_kairos`.

### Key Patterns

- **Forked agent**: `runForkedAgent()` shares parent's prompt cache (byte-identical system prompt). Near-zero incremental cost.
- **Post-sampling hooks**: In-process registry for background intelligence (session memory, magic docs, skill improvement).
- **Stop hooks**: Turn-end coordinator for fire-and-forget tasks (extract memories, autoDream, prompt suggestion).
- **Side queries**: Cheap Sonnet calls for classification/selection tasks (memory relevance, away summary, skill improvement).
- **Gate chains**: Cheapest checks first (feature flag → time → session count → file lock) to avoid expensive work.

---

## Klynt Current State Assessment

### What's Working

| Component | Status | Notes |
|-----------|--------|-------|
| BackgroundConsolidationService | ✅ Working | Event-driven, 3s batch window, salience filter |
| Salience filter | ✅ Working | Extract/Accumulate/Discard triage for all domain events |
| ExtractionHandler (heuristic) | ✅ Working | Pattern-matches source_event, generates SPO facts |
| ExtractionHandler (LLM) | ✅ Working | Structured JSON output, falls back to heuristic |
| ConsolidationHandler | ✅ Working | LLM-backed dedup decisions (Add/Update/Delete/Noop) |
| retrieve_relevant_facts | ✅ Working | Vector + BM25 + FSRS hybrid, 10-factor scoring |
| UnifiedMemoryService | ✅ Working | RRF merge + autotuner live param overrides |
| CognitiveContextSource | ✅ Working | Static user model + procedural rules in system prompt |
| BM25/FTS5 search | ✅ Working | Porter stemming, superseded facts excluded |
| Weekly reflection | ✅ Working | 20-episode guard, processes last 7 days |
| Memory compaction | ✅ Working | 90-day archive, 10K active-fact budget |
| Mirror (4 subscribers) | ✅ Working | Routing drift, meta-rules, brain versioning, trial preview |
| Autotuner | ✅ Working | Always-on, nightly cycle, shadow scoring |
| All feature crates | ✅ Working | tasks, finance, notes, productivity, coaching, insights, launcher |
| Atom extraction | ✅ Working | Debounce + content-hash dedup on note edits |

### What's Broken / Not Wired

| Component | Status | Root Cause |
|-----------|--------|------------|
| Chat-based fact extraction | ❌ Broken | `ChatTurnCompleted` stripped of `user_message` → `event_to_observation()` returns `None` |
| `UserStatedFact` events | ❌ Never published | No code in production emits this event — only used in tests |
| MidLoopCompressor | ⚠️ Not wired | Exists and tested, but never instantiated in `execute_loop` |
| LiveContextRefresher | ⚠️ Not wired | `ContextUpdateQueue` passes through but `inject_pending` never called |
| Per-skill execution budgets | ⚠️ Dead code | `ExecutionBudget::new(depth, "general")` hardcodes "general" |
| Mirror TrialPreview evaluator | ⚠️ Phase 5 stub | `EarlyTrialEvaluator` wired as `None` |
| Louvain community scoring | ⚠️ Partial | Algorithm exists, score passed as 0.0 everywhere |

---

## Implementation Plan

### Phase A: Critical Fixes (Broken Pipelines)

- [ ] **A1: Restore chat-based fact extraction**
  - Re-add `user_message: Option<String>` to `ChatTurnCompleted` event in `bus/src/domain_events.rs`
  - Populate from agent loop in `agent/src/agent_loop/mod.rs` (line ~683)
  - Update `event_to_observation()` in `cognitive/src/services/background.rs` (line 741) to create `Observation` from the message content
  - Update all match arms that destructure `ChatTurnCompleted`
  - Files: `bus/domain_events.rs`, `agent/agent_loop/mod.rs`, `cognitive/services/background.rs`, `activity-log/normalizers.rs`

- [ ] **A2: Wire MidLoopCompressor into execute_loop**
  - Instantiate `MidLoopCompressor` in `execution/execute_loop.rs`
  - Call `check_and_compress()` at each iteration boundary (after tool results, before next LLM call)
  - Needs `TokenCounter` + context window size from `ExecutionParams`
  - Files: `agent/execution/execute_loop.rs`

- [ ] **A3: Wire LiveContextRefresher into execute_loop**
  - Create `LiveContextRefresher` when `ContextUpdateQueue` is present in `ExecutionParams`
  - Call `inject_pending()` at iteration boundary (after MidLoopCompressor, before next LLM call)
  - Files: `agent/execution/execute_loop.rs`

- [ ] **A4: Publish UserStatedFact events from agent pipeline**
  - Option A (heuristic): Detect "I am/I'm/my name is/I work/I prefer" patterns in user messages within `process_message` and publish `UserStatedFact`
  - Option B (LLM): Add a `memory` tool action that the agent can call to explicitly record facts (the model decides what's worth saving via system prompt guidance)
  - Recommended: Option B (matches Claude Code's approach — delegate judgment to the LLM)
  - Files: `agent/agent_loop/mod.rs` or new tool action in `tools/`

### Phase B: CC-Inspired High-Value Features

- [ ] **B1: Post-turn LLM memory extraction**
  - After `ChatTurnCompleted`, spawn a lightweight LLM call with last N messages + current user model
  - LLM outputs structured `Vec<ExtractedFact>` (subject, predicate, object, domain, confidence)
  - Feed into existing `SemanticFactRepo::upsert()` + `SemanticFactEmbedder::embed_and_store_fact()`
  - Use existing `cognitive_provider` (separate from chat provider) to avoid contention
  - Gate: skip if message is a question, < 10 chars, or purely a tool command
  - Dedup: skip if main agent already wrote facts via memory tool (check `last_memory_write_timestamp`)
  - Files: new service in `cognitive/services/` or extend `background.rs`

- [ ] **B2: Session memory scratchpad**
  - Per-session structured summary: current task, context, decisions, errors, key results
  - Updated every N turns (or when token count grows by threshold) via cheap LLM call
  - Stored in `session_context` or new `session_memory` table
  - Injected into context assembly via new `SessionMemoryContextSource`
  - Enables smart auto-compaction: when near context limit, replace old messages with session memory summary (no LLM call)
  - Files: new `cognitive/services/session_memory.rs`, new context source

- [ ] **B3: Cross-session consolidation upgrade**
  - Extend weekly reflection to also synthesize semantic facts across sessions
  - Gate chain (CC-inspired): ≥N hours since last consolidation + ≥M sessions
  - Merge: find facts with overlapping subjects across sessions → consolidate, increase stability
  - Prune: archive low-confidence facts that haven't been accessed in 30+ days
  - Update: convert relative temporal references to absolute dates
  - Files: extend `cognitive/services/reflection.rs` or new `consolidation_pass.rs`

- [ ] **B4: Memory freshness warnings**
  - When `CognitiveContextSource` or `UnifiedMemoryService` injects facts into context, annotate facts older than 7 days with `[stale — verify before acting]`
  - Simple check: `Utc::now() - fact.valid_from > Duration::days(7)`
  - Files: `cognitive/services/context_source.rs`, `cognitive/services/memory_retriever.rs`

### Phase C: Polish & Enhancement

- [ ] **C1: Away/resume summary**
  - On session resume after idle (detect via `SessionCreated` event + check last session's `last_active_at`), generate 1-3 sentence recap
  - Use last session's messages + session memory (if B2 is done)
  - Inject as system message at session start
  - Files: `agent/agent_loop/mod.rs` or `app-core/handlers/chat/`

- [ ] **C2: Magic notes (auto-updating notes)**
  - Notes tagged with `#auto-update` or a special frontmatter field get refreshed based on conversation context
  - Leverage existing `NoteEditingFinished` → `AtomExtractionService` pipeline
  - Add a post-turn check: if conversation references a magic note's topic, queue an update
  - Files: `feature-notes/`, new subscriber on `DomainEventBus`

- [ ] **C3: Proactive memory surfacing (per-query relevance)**
  - Before each LLM call, run a cheap side-query (or vector search) to select the N most relevant facts for the current message
  - Different from static `CognitiveContextSource` (which injects the same user model every time)
  - Similar to CC's `findRelevantMemories` but using Klynt's superior vector+BM25 retrieval
  - Files: extend `context_engine/` or `cognitive/services/retrieval.rs`

- [ ] **C4: Wire Mirror TrialPreview evaluator (Phase 5)**
  - Implement `EarlyTrialEvaluator` trait in `app-core` or `agent`
  - Compute: correction_rate_delta, confidence_trend, dominant_skill_shift over 4-hour window
  - Wire into `TrialPreviewSubscriber::new(..., Some(evaluator))`
  - Files: `cognitive/mirror/subscribers/trial.rs`, new impl in `app-core/`

- [ ] **C5: Activate community scoring (Louvain)**
  - Wire `community_score` in retrieval scoring (currently hardcoded to 0.0)
  - Ensure `CommunityTreeBuilder` is populating community graph tables
  - Files: `cognitive/services/retrieval.rs`, `cognitive/louvain.rs`

---

## What Klynt Already Has That's Superior to Claude Code

These should be preserved and not regressed during upgrades:

- **Structured SPO triples** with confidence, stability, supersession chains (vs CC's flat markdown)
- **FSRS-5 forgetting curve** for fact decay and flashcard scheduling (vs CC's no decay)
- **10-factor relevance scoring** with configurable weights and autotuner optimization (vs CC's simple Sonnet selection)
- **Vector + BM25 hybrid retrieval** with RRF merge (vs CC's filename+description matching)
- **Mirror self-reflection** with routing drift detection, meta-rule proposals, brain versioning (nothing in CC)
- **Autotuner** with shadow scoring, nightly promotion, live param injection (nothing in CC)
- **Multi-persona debate** system with blackboard pattern and scope-based memory (vs CC's basic forked agents)
- **Coaching pipeline** with event-driven pattern detection and intervention routing (nothing in CC)
- **Atom extraction** from notes with cross-note reinforcement detection (nothing in CC)

---

## Architecture Comparison Quick Reference

```
Claude Code Memory Flow:
  User msg → LLM response → stopHooks:
    ├─ extractMemories (forked LLM, per-turn) → .md files
    ├─ autoDream (forked LLM, periodic) → consolidate .md files
    └─ sessionMemory (forked LLM, per-turn) → session scratchpad
  Next query:
    └─ findRelevantMemories (Sonnet side-query) → inject ≤5 files

Klynt Memory Flow (after Phase A fixes):
  User msg → AgentRuntime → ChatTurnCompleted(user_message):
    → BackgroundConsolidationService:
        → evaluate_salience() → Extract
        → event_to_observation() → Observation
        → ExtractionHandler.extract_facts_batch() → ExtractedFact[]
        → ConsolidationHandler.decide_batch() → MemoryOp[]
        → execute_memory_ops() → SemanticFactRepo + LanceDB
  Next query:
    → CognitiveContextSource → static user model + rules
    → UnifiedMemoryService → vector + BM25 + FSRS retrieval
```
