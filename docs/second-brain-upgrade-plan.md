# Second Brain Upgrade Plan

> Generated 2026-04-07 from deep analysis of Claude Code's memory architecture vs Klynt's cognitive system.
> **Last updated:** 2026-04-07 — reflects all Phase A, Phase B, and Episodic improvements completed.

## Executive Summary

Klynt's cognitive architecture (structured SPO triples, vector+BM25 hybrid retrieval, FSRS-5 decay, 10-factor relevance scoring, mirror self-reflection, autotuner) is **fundamentally more sophisticated** than Claude Code's flat-file memory system. The upstream pipeline was disconnected — `ChatTurnCompleted` events had been stripped of message content, blocking all chat-based fact extraction. **This has been fixed.** The system now extracts facts from every conversation turn with full 3-turn context, maintains per-session scratchpads, and surfaces episodic memories in per-turn retrieval.

Claude Code's key advantage was its **multi-layered extraction strategy**: (1) main agent proactive saves, (2) per-turn forked LLM extraction subagent, (3) periodic cross-session consolidation ("autoDream"). Klynt now matches or exceeds this with: (1) `record_fact` tool for proactive saves, (2) enriched `BackgroundConsolidationService` with session history, (3) `SessionMemoryService` for per-session scratchpads.

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
| UnifiedMemoryService | ✅ Working | RRF merge: facts + recalls + **episodic memories** |
| CognitiveContextSource | ✅ Working | Static user model + procedural rules in system prompt |
| BM25/FTS5 search | ✅ Working | Porter stemming, superseded facts excluded |
| Weekly reflection | ✅ Working | **8-episode guard** (lowered from 20), processes last 7 days |
| Memory compaction | ✅ Working | 90-day archive, 10K active-fact budget |
| Mirror (4 subscribers) | ✅ Working | Routing drift, meta-rules, brain versioning, trial preview |
| Autotuner | ✅ Working | Always-on, nightly cycle, shadow scoring |
| All feature crates | ✅ Working | tasks, finance, notes, productivity, coaching, insights, launcher |
| Atom extraction | ✅ Working | Debounce + content-hash dedup on note edits |
| **Chat-based fact extraction** | ✅ **Fixed** | `ChatTurnCompleted` carries `user_message`, enriched with 3-turn session history |
| **MidLoopCompressor** | ✅ **Wired** | Compresses old tool results at 70% context threshold |
| **LiveContextRefresher** | ✅ **Wired** | Injects promoted memories mid-execution |
| **MemoryTool `record_fact`** | ✅ **New** | LLM can explicitly save facts via `UserStatedFact` events |
| **SessionMemoryService** | ✅ **New** | Per-session scratchpad updated every 3 turns via LLM |
| **SessionMemoryContextSource** | ✅ **New** | Priority 88, injects scratchpad into system prompt |
| **Episodic memory summaries** | ✅ **New** | Concise one-line summaries at creation time |
| **Episodic FTS in retrieval** | ✅ **New** | BM25 search over episodic memories merged into per-turn retrieval |

### What Remains

| Component | Status | Notes |
|-----------|--------|-------|
| Per-skill execution budgets | ⚠️ Dead code | `ExecutionBudget::new(depth, "general")` hardcodes "general" |
| Mirror TrialPreview evaluator | ⚠️ Phase 5 stub | `EarlyTrialEvaluator` wired as `None` |
| Louvain community scoring | ⚠️ Partial | Algorithm exists, score passed as 0.0 everywhere |

---

## Implementation Status

### Phase A: Critical Fixes — ✅ COMPLETE

- [x] **A1: Restore chat-based fact extraction** — `e89d5393`
  - Re-added `user_message: Option<String>` with `#[serde(default)]` to `ChatTurnCompleted`
  - Updated all 4 publish sites, all match/destructure sites, all test sites
  - `event_to_observation()` now creates `Observation` from message content
- [x] **A2: Wire MidLoopCompressor** — `3af7bc34`
  - Constructed before loop, invoked after `ToolsExecuted`, emits `ContextCompressed` events
- [x] **A3: Wire LiveContextRefresher** — `64b16b47`
  - Constructed conditionally, called after compression, respects `pause_context_updates`
- [x] **A4: MemoryTool `record_fact` action** — `c35261b8`
  - Publishes `UserStatedFact` domain events at importance 1.0
  - `DomainEventBus` injected via builder pattern

### Phase B: CC-Inspired High-Value Features — ✅ COMPLETE

- [x] **B1: Enriched post-turn extraction** — `96b5ba64` + `a64731c4`
  - `BackgroundServiceConfig` gets `session_repo`
  - `event_to_observation` loads last 6 messages (3 turns) for full context
  - LLM extractor sees `[user]: ... [assistant]: ... [user]: ...` instead of raw text
- [x] **B2: Session memory scratchpad** — `c1a43a56` + `181a63ee` + `4bfe3bb5` + `f80d3003`
  - `session_memory` table + `SessionMemoryRepo` (upsert/get/delete)
  - `SessionMemoryService` subscribes to `ChatTurnCompleted`, updates every 3 turns
  - `SessionMemoryContextSource` at priority 88 injects into system prompt
  - LLM-based summarization with heuristic fallback
- [ ] **B3: Cross-session consolidation upgrade** — NOT STARTED
  - Extend weekly reflection to synthesize facts across sessions
- [x] **B4: Memory freshness warnings** — PARTIALLY ADDRESSED
  - Episodic memory summaries provide temporal context; full freshness annotations not yet added

### Episodic Memory Improvements — ✅ COMPLETE

- [x] **Lower reflection threshold** — `1a16bbab`
  - `MIN_EPISODE_COUNT` reduced from 20 to 8 for faster first reflection
- [x] **Generate summaries at creation** — `ca930164`
  - `summarize_observation()` extracts last user message or truncates at 120 chars
- [x] **Wire FTS into UnifiedMemoryService** — `57ae79d8` + `00cae81c`
  - `MemorySource::EpisodicMemory` variant added
  - `fetch_episodes()` via BM25 FTS5, capped at 5 results, merged via RRF
  - Prefers `summary` over raw `content` for concise injection
- [x] **Fix bloated episodic memories** — `45f899ad`
  - `ChatTurnCompleted` importance kept at 0.5 (below 0.7 episodic threshold)

### Phase C: Polish & Enhancement — NOT STARTED

- [ ] **C1: Away/resume summary**
- [ ] **C2: Magic notes (auto-updating notes)**
- [ ] **C3: Proactive memory surfacing (per-query relevance)**
- [ ] **C4: Wire Mirror TrialPreview evaluator (Phase 5)**
- [ ] **C5: Activate community scoring (Louvain)**

---

## What Klynt Has That's Superior to Claude Code

- **Structured SPO triples** with confidence, stability, supersession chains (vs CC's flat markdown)
- **FSRS-5 forgetting curve** for fact decay and flashcard scheduling (vs CC's no decay)
- **10-factor relevance scoring** with configurable weights and autotuner optimization (vs CC's simple Sonnet selection)
- **Vector + BM25 hybrid retrieval** with RRF merge across 3 sources: facts + recalls + episodic (vs CC's filename+description matching)
- **Mirror self-reflection** with routing drift detection, meta-rule proposals, brain versioning (nothing in CC)
- **Autotuner** with shadow scoring, nightly promotion, live param injection (nothing in CC)
- **Multi-persona debate** system with blackboard pattern and scope-based memory (vs CC's basic forked agents)
- **Coaching pipeline** with event-driven pattern detection and intervention routing (nothing in CC)
- **Atom extraction** from notes with cross-note reinforcement detection (nothing in CC)
- **Session memory scratchpad** with LLM-powered summarization (matches CC's SessionMemory)
- **Episodic memory system** with FTS search, summaries, and per-turn injection (nothing in CC)

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

Klynt Memory Flow (current, after all upgrades):
  User msg → AgentRuntime → ChatTurnCompleted(session_key, user_message):
    → BackgroundConsolidationService:
        → evaluate_salience() → Extract
        → event_to_observation(event, session_repo) → load 3 turns of history
        → ExtractionHandler.extract_facts_batch() → ExtractedFact[]
        → ConsolidationHandler.decide_batch() → MemoryOp[]
        → execute_memory_ops() → SemanticFactRepo + LanceDB
        → EpisodicMemory created (if importance >= 0.7) with summary
    → SessionMemoryService:
        → Every 3 turns → LLM summarization → session_memory table
  Next query:
    → CognitiveContextSource (priority 60) → static user model + procedural rules
    → SessionMemoryContextSource (priority 88) → per-session scratchpad
    → UnifiedMemoryService → RRF merge of:
        ├─ SemanticFact vector + BM25 retrieval
        ├─ ConversationRecall (LanceDB time-decay)
        └─ EpisodicMemory FTS5 search (capped at 5)
    → LiveContextRefresher → inject promoted memories mid-execution
```

---

## Commit History (2026-04-07)

### Phase A
| Hash | Message |
|------|---------|
| `e89d5393` | fix(cognitive): restore user_message on ChatTurnCompleted for fact extraction |
| `3af7bc34` | fix(agent): wire MidLoopCompressor into execute_loop |
| `64b16b47` | fix(agent): wire LiveContextRefresher into execute_loop |
| `c35261b8` | feat(memory): add record_fact action to MemoryTool |

### Phase B
| Hash | Message |
|------|---------|
| `96b5ba64` | refactor(cognitive): add session_repo to BackgroundServiceConfig |
| `a64731c4` | feat(cognitive): enrich ChatTurnCompleted extraction with session history |
| `c1a43a56` | feat(storage): add session_memory table and SessionMemoryRepo |
| `181a63ee` | feat(cognitive): add SessionMemoryService for per-session scratchpads |
| `4bfe3bb5` | feat(agent): add SessionMemoryContextSource (priority 88) |
| `f80d3003` | feat(agent): wire SessionMemoryService + SessionMemoryContextSource |

### Episodic Improvements
| Hash | Message |
|------|---------|
| `1a16bbab` | fix(cognitive): lower reflection MIN_EPISODE_COUNT from 20 to 8 |
| `ca930164` | feat(cognitive): generate summary at episodic memory creation |
| `57ae79d8` | feat(context_engine): add EpisodicMemory variant to MemorySource |
| `00cae81c` | feat(cognitive): wire episodic memory FTS search into UnifiedMemoryService |

### Fixes
| Hash | Message |
|------|---------|
| `45f899ad` | fix(cognitive): prevent ChatTurnCompleted from creating bloated episodic memories |
| `6871295a` | fix(ui): enable scroll on System page tab content |
| `9bbec134` | fix(test): update memory_tool_registration for record_fact action |
