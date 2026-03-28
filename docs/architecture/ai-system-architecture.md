# AI System Architecture

## Overview

Klyntbot's AI system is a multi-layered intelligence pipeline that transforms user messages into contextual, tool-augmented responses while maintaining long-term cognitive memory and self-optimizing its behavior.

```
User Message
     |
     v
+----+----+    +----------+    +----------+    +-----------+
| Skill   |--->| Intent   |--->| Context  |--->| Execution |---> Response
| Router  |    | Analyzer |    | Engine   |    | Router    |
+----+----+    +----+-----+    +----+-----+    +-----+-----+
     |              |               |                |
     v              v               v                v
 SkillCatalog   Heuristic +     Memory          Direct or
 (5 orchestr.)  LLM class.     Retrieval        Reactive
                               Token Budget      (ReAct loop)
                                                      |
                                                      v
                                               +------+------+
                                               | Tool        |
                                               | Registry    |
                                               | (20+ tools) |
                                               +------+------+
                                                      |
                               +----------------------+
                               |                      |
                               v                      v
                          +----+-----+          +-----+------+
                          | Cognitive |          | Autotuner  |
                          | Memory   |          | (shadow    |
                          | System   |          |  trials)   |
                          +----------+          +------------+
```

## LLM Provider Abstraction

### Provider Interface

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: Option<&[Value]>,
                  params: &ChatParams) -> Result<LlmResponse>;
    async fn chat_stream(...) -> Result<LlmStream>;
    fn supports_streaming(&self) -> bool;
    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
    async fn count_tokens(...) -> Result<usize>;
    fn capabilities(&self) -> ProviderCapabilities;
    fn context_window(&self) -> usize;
    async fn health_check(&self) -> Result<ProviderHealth>;
    fn classifier_provider(&self) -> Option<DynProvider>;
}
```

**`DynProvider = Arc<dyn LlmProvider>`** — trait object for runtime polymorphism.

### Supported Providers

| Provider | Adapter | Key Features |
|----------|---------|-------------|
| Anthropic | `AnthropicNativeProvider` | Prompt caching, extended thinking, native token counting, 200K context |
| OpenAI | `OpenAICompatProvider` | GPT models via OpenAI API |
| OpenRouter | `OpenAICompatProvider` | Multi-model routing |
| DeepSeek | `OpenAICompatProvider` | Reasoning content (R1) |
| Gemini | `OpenAICompatProvider` | Via compatibility endpoint |
| Groq | `OpenAICompatProvider` | Fast inference + Whisper transcription |
| vLLM | `OpenAICompatProvider` | Self-hosted models |
| ZhipuAI, DashScope, Moonshot, MiniMax, AiHubMix | `OpenAICompatProvider` | China-region providers |

### Provider Capabilities

`ProviderCapabilities` flags: `extended_thinking`, `structured_outputs`, `prompt_caching`, `native_token_counting`, `vision`, `streaming`, `tool_choice_required`, `parallel_tool_calls`.

### Circuit Breaker & Failover

`ProviderManager` wraps primary + fallback provider pair:

```
Request --> Primary Provider
                |
           [Success] --> Return
                |
           [Failure] --> Increment counter
                |
           [threshold=5 consecutive failures]
                |
           Circuit OPEN --> Failback Provider
                |
           [reset_timeout=60s]
                |
           Circuit HALF-OPEN --> Retry primary
```

Circuit state persisted to SQLite (`circuit_breaker_state` table) for survival across restarts.

### Classifier Provider Separation

Providers can expose a separate cheaper/faster model for intent classification via `classifier_provider()`. This allows using Claude Haiku for classification while Claude Sonnet handles execution.

## Message Types

The `Message` enum represents all conversation roles:

| Variant | Fields | Purpose |
|---------|--------|---------|
| `System` | `content: String` | System prompt (assembled by ContextEngine) |
| `User` | `content: UserContent` | Text or MultiPart (images) |
| `Assistant` | `content, tool_calls, reasoning_content` | LLM response (reasoning for thinking models) |
| `Tool` | `tool_call_id, name, content` | Tool execution result |
| `ContextUpdate` | `reason, content` | Mid-execution injection (live memory refresh) |

`ContextUpdate` is serialized as a System-role message with `<context_update reason="...">` XML wrapper.

## Skill System

### Architecture

```
+---------------+     +---------------+     +------------------+
| SkillCatalog  |---->| SkillRouter   |---->| SkillContextSrc  |
| (discovery)   |     | (selection)   |     | (injection)      |
+-------+-------+     +-------+-------+     +--------+---------+
        |                      |                      |
   +----+----+          +------+------+        +------+------+
   | Built-in|          | Keyword     |        | Tier 1: Full|
   | include_|          | + Semantic  |        | body (once) |
   | str!()  |          | blended     |        +-------------+
   +---------+          | scoring     |        | Tier 2: Refs|
   | User    |          +-------------+        | (always)    |
   | skills  |                                 +-------------+
   +---------+                                 | Tier 3:     |
   | Persona |                                 | Summaries   |
   | skills  |                                 +-------------+
   +---------+
```

### Built-in Orchestrator Skills

| Skill | Triggers (sample) | Allowed Tools | MCP Access | Max Iterations |
|-------|--------------------|---------------|-----------|----------------|
| `general` | hello, hi, look up, search, remember | ask_user, memory, web_search, web_fetch, grep, glob, read_file, list_dir, spawn, learning | All (`["*"]`) | 15 |
| `task-management` | create task, plan my day, weekly review, decompose | task, tasks, area, project, okr, notes, productivity, ask_user, memory | google-calendar | 12 |
| `finance-management` | budget, expenses, spending, investment | finance, ask_user, memory | None | 10 |
| `automation` | remind me, schedule, every day, cron | cron, ask_user | None | 10 |
| `communication` | send message, notify, email | message, ask_user | None | 10 |

### Skill Routing Algorithm

**Dual scoring (keyword + semantic):**

1. **Keyword score:** Description token overlap + trigger phrase exact match (0.3 per hit, capped at 1.0)
2. **Semantic score:** Cosine similarity between query embedding and pre-computed skill description embedding
3. **Blended:** `score = kw_score * 0.7 + sem_score * 0.3` (weights tunable by autotuner)
4. **Candidacy gate:** Must have `kw_score > 0` OR `sem_score >= 0.5`
5. **Ambiguity tiebreaker:** Within 0.05 of top score → prefer skill with fewer triggers (more specific)
6. **Fallback:** Always returns `general` if no candidates pass gate

**Non-orchestrator activation:** Threshold 0.4, max 3 per message.

### Progressive Loading (Token Optimization)

```
First message:    [Full orchestrator body injected]        ~2000 tokens
                  [Activated skill summaries]              ~200 tokens each
                  [Always-loaded references]               ~500 tokens each

Second message:   [Orchestrator body NOT re-injected]      0 tokens (dedup)
(same skill)      [Activated skill summaries]              ~200 tokens each
                  [References filtered by relevance]       0-500 tokens each

On-demand:        LLM calls skill_reference tool           Full body loaded
```

Multi-token reference names (e.g., `daily-planner`) only load when at least one token appears in the message. Single-token refs (e.g., `todo`) always load.

## Intent Analysis Pipeline

4-layer cascade with increasing cost:

```
Message --> Layer 1: Aho-Corasick Heuristic (microseconds)
              |
         [Unambiguous] --> Direct or Reactive (with signals)
              |
         [Ambiguous] --> Layer 2: Embedding Heuristic
                            |
                       [Confident] --> Classified
                            |
                       [Low confidence] --> Layer 3: LLM Classifier
                                              |
                                         [Classified] --> Mode determined
                                              |
                                         [Fallback] --> Reactive(15 iterations)
```

### Layer 1 — Aho-Corasick Heuristic

Compiled once via `OnceLock`. Patterns:
- **Greeting detection:** Exact set + start patterns → `Direct` mode
- **Domain matchers:** task_mgmt, finance, notes, automation substring patterns
- **Complexity signals:** Tool indicators, sequential language, risk patterns, state/retry indicators
- **Negation + hypothetical detection** (flags `has_hypothetical = true`)

### ComplexitySignals

```
Score Component              Points    Condition
estimated_tool_calls >= 3     +2       Many tool calls expected
estimated_tool_calls >= 2     +1       Some tool calls
has_sequential_deps           +2       Steps depend on each other
failure_risk >= Medium        +1       Risk of failure
requires_state_tracking       +1       Must track state across calls
requires_retries              +1       May need retries
                              ----
Maximum score                  7
```

**Iteration budget:** `min(max(estimated_tool_calls * 3, 10) + 5, 30)` — floor 15, ceiling 30.

### Execution Mode Decision

| Mode | When Used | Max Iterations | Tools |
|------|-----------|----------------|-------|
| `Direct` | Greetings, factual Q&A, conversational | 1 | None (no tools passed) |
| `Reactive` | Tool-requiring tasks, complex queries | 10-30 (dynamic) | Full tool set |

**Automatic escalation:** `DirectEngine` detecting tool calls in LLM response → `EngineResult::Escalate` → re-run with `ReactiveEngine`.

**Confidence downgrade:** If `confidence < evaluator.threshold()` → downgrade Reactive to Direct.

**CoT planning:** If `complexity_score >= 4` → chain-of-thought planning prompt prepended to iteration 1.

## Context Engine

### Token Budget Allocation

Waterfall allocator with priority ordering:

| Priority | Section | Allocation Strategy |
|----------|---------|-------------------|
| 0 (highest) | System identity | Always allocated first |
| 1 | Active task | Allocated second |
| 2 | Tool definitions | Only for Reactive mode (zero for Direct) |
| 3 | Recent history | Uncompressed recent messages |
| 4 | Retrieved memory | Semantic memory from vector store |
| 5 | Compressed history | Older messages, summarized |
| 6 | Persona | Persona instructions |
| 7 (lowest) | Skills | Skill content (protected from mid-loop compression) |

**Reserve:** 15% of context window reserved for LLM response.

**Token counting:** `CharTokenCounter` (4 chars = 1 token) as default. Provider-specific counters available.

### Context Sources (Pluggable)

| Source | Data Provided |
|--------|--------------|
| `IdentityContextSource` | User name, role, timezone, workspace persona |
| `SkillContextSource` | Active orchestrator body, activated skill summaries, references |
| `PersonaContextSource` | Active persona instructions |
| `ProjectContextSource` | Project instructions, role, memories |
| `AreaContextSource` | PARA area context |
| `TodoContextSource` | Pending tasks |
| `SessionContextSource` | PARA scope for current session |
| `ProductivityContextSource` | Daily productivity summary |
| `AnnotationContextSource` | Critical annotations |
| `ConfidenceContextSource` | Confidence guidance |
| `PageContextSource` | Current desktop page context |

All sources implement `ContextSource` trait. Called in parallel via `join_all()`. Non-empty sections joined with `\n\n---\n\n`.

### Memory Prefetch

Memory retrieval is expensive (vector similarity search). The runtime optimizes by running it concurrently with intent classification:

```
process_message()
     |
     +--spawn--> prefetch_memory(profile)     <-- runs concurrently
     |
     +---------> IntentAnalyzer::analyze()    <-- runs concurrently
     |
     +--await--> both complete
     |
     +---------> ContextEngine::assemble_with_prefetched()
```

~95% of cases: prefetch profile matches final profile. On mismatch (orchestration override), the in-flight prefetch handle is aborted (`prefetch_handle.abort()`) and fresh retrieval is performed with the correct profile.

### Query Rewriting

`QueryRewriter` enriches retrieval queries using `RetrievalContext`:
- Active task context
- User situation
- Active desktop view
- Correction context (excludes rejected topic terms)

## Cognitive Memory System

### Three Memory Types

```
+-------------------+     +-------------------+     +-------------------+
| Episodic Memory   |     | Semantic Memory   |     | Procedural Rules  |
| (events/episodes) |     | (facts/knowledge) |     | (behavior rules)  |
+--------+----------+     +--------+----------+     +--------+----------+
         |                         |                         |
    SQLite table             SQLite table +             SQLite table
    (narrative               LanceDB vectors            (if/then rules)
     records)                (384-dim embeddings)
```

### Semantic Facts (Long-Term Knowledge)

Triple-store inspired structure:

| Field | Type | Purpose |
|-------|------|---------|
| `subject` | String | Entity (e.g., "user") |
| `predicate` | String | Relationship (e.g., "prefers") |
| `object` | String | Value (e.g., "dark mode") |
| `confidence` | f64 | Extraction confidence |
| `stability` | f64 | FSRS-like stability for decay |
| `memory_type` | Enum | decision, milestone, pattern, insight, fact |
| `scope_type` | Enum | system, persona, squad |

**Memory type classification** (keyword-based on object text):
- `["decided to", "let's go with"]` → decision
- `["completed", "shipped"]` → milestone
- `["noticed that", "pattern"]` → pattern
- `["realized", "learned"]` → insight

### Extraction Pipeline

```
DomainEvent published
     |
     v
BackgroundConsolidationService subscribes
     |
     v
evaluate_salience(event)
     |
     +--Extract------> collect_batch(5s window)
     |                      |
     +--Accumulate---> AccumulatedObservationRepo
     |                 (promote when count >= threshold
     |                  AND days_seen >= min_days)
     +--Discard-----> dropped
                            |
                            v
                  ExtractionHandler::extract()
                  (LLM batch extraction OR heuristic)
                            |
                            v
                  ConsolidationHandler::consolidate()
                  (LLM: ADD/UPDATE/DELETE/NOOP per candidate)
                            |
                            v
                  SemanticFactRepo::upsert()
                  SemanticFactEmbedder::embed() --> LanceDB
                            |
                            v
                  ContextUpdateQueue::push(MemoryPromoted)
                  (injected at next ReAct iteration boundary)
```

### Salience Evaluation

~50 match arms categorizing events into Extract/Accumulate/Discard:

| Verdict | Example Events |
|---------|---------------|
| **Extract** (immediate) | `UserStatedFact`, `UserCorrectedAI`, `ChatTurnCompleted`, `BudgetAlert`, session quality >= 80, task estimation deviation > 50% |
| **Accumulate** (buffer) | Routine productivity events, standard task/finance/note operations |
| **Discard** | Session created, rule evolved, low-value events |

### Relevance Scoring (10-Factor Formula)

Evolved from 6-factor (base) → 8-factor (Phase 1: hierarchy + path_coherence) → 10-factor (Phase 2: community + cross_note):

```
relevance = w_semantic       * semantic_similarity           // 0.20
          + w_retrievability * retrievability(elapsed, stab)  // 0.10
          + w_importance     * importance                     // 0.08
          + w_frequency      * frequency_score                // 0.05
          + w_situation      * situational_boost               // 0.15
          + w_temporal       * temporal_recency                // 0.02
          + w_hierarchy      * hierarchy_score                 // 0.10 (Phase 1)
          + w_path_coherence * path_coherence                  // 0.05 (Phase 1)
          + w_community      * community_membership            // 0.15 (Phase 2)
          + w_cross_note     * cross_note_boost                // 0.10 (Phase 2)
```

**Phase 1 factors:** `hierarchy_score` biases retrieval toward the correct tree depth (roots for summaries, leaves for details). `path_coherence` rewards nodes whose siblings also scored well.

**Phase 2 factors:** `community_membership` is `membership_score × community_stability` for tree nodes in matched communities. `cross_note_boost` is `log2(source_note_count)` clamped [0, 1] — communities spanning 4+ notes get maximum boost.

**Backward compatibility:** For non-note results (cognitive facts, conversation recall), hierarchy/path_coherence/community/cross_note factors are neutral (0 or 0.5), degrading to original 6-factor behavior.

### FSRS-5 Spaced Repetition

Full FSRS-5 implementation with 19 weight parameters for flashcard/knowledge atom scheduling:

- `retrievability(t, S) = 1 / (1 + t / (9 * S))` — at `t=S`, R ~= 0.9
  > **Note:** This formula governs flashcard scheduling (`fsrs5.rs`) only. The memory relevance decay for semantic facts uses a separate exponential formula: `retrievability(t, S) = exp(ln(0.9) * t / S)` (in `decay.rs`).
- `initial_stability(rating)` = `w[rating-1]` — defaults: 0.40255, 1.18385, 3.173, 15.69105
- `initial_difficulty(rating)` = `w4 - exp(w5 * (rating-1)) + 1` — clamped [1.0, 10.0]
- `next_stability_success(S, D, R, rating)` — hard penalty `w15`, easy bonus `w16`
- `next_stability_failure(S, D, R)` — never increases stability

### Mirror Self-Reflection Layer

Event-driven self-awareness system with 4 subscribers:

```
DomainEventBus
     |
     +-------> RoutingMirrorSubscriber
     |         (hourly routing snapshots, drift detection)
     |
     +-------> MetaRuleDetector
     |         (correction streaks --> pending rule proposals)
     |
     +-------> ConfigArchiver
     |         (autotuner promotions --> brain version timeline)
     |
     +-------> TrialPreviewSubscriber
               (4h early trial evaluation, kill/continue)

     All --> MirrorFacade (public API)
             - get_state(), get_narratives()
             - approve/dismiss meta-rules
             - kill trials, revert brain versions
             - generate weekly narrative (LLM)
```

**Storage:** 6 tables in `003_mirror_tables.sql`.

**Cron:** Weekly narrative (Sunday 10am UTC), cleanup of >90 day data (Sunday 4am UTC).

## Autotuner (Self-Optimization)

### 28 Tunable Parameters

Expanded from 19D (base) → 24D (Phase 1) → 28D (Phase 2):

| Category | Parameters |
|----------|-----------|
| Routing | `skill_keyword_weight`, `skill_semantic_weight`, `skill_activation_threshold` |
| Classification | `heuristic_confidence_threshold`, `llm_classifier_timeout_ms` |
| Memory retrieval | `relevance_weight_semantic/retrievability/situation/importance/frequency/temporal`, `vector_top_k`, `min_similarity` |
| FSRS | `fsrs_desired_retention`, `accumulate_promote_threshold`, `accumulate_min_days` |
| Query rewriting | `rewrite_confidence_threshold`, `rewrite_max_signals`, `rewrite_min_enrichment_length` |
| Hierarchical notes (Phase 1) | `w_hierarchy`, `w_path_coherence`, `tree_top_k`, `tree_min_similarity`, `hybrid_bias` |
| Community graph (Phase 2) | `w_community`, `w_cross_note`, `community_top_k`, `community_min_similarity` |

### Shadow Experiment Flow

```
Message received
     |
     v
AutoTunerHook::on_message_received()
     |
     +--shadow--> ShadowClassifier (re-classify with trial params)
     |            ShadowRetriever (retrieve with trial params)
     |
     +--live----> Normal pipeline execution
     |
     v
AutoTunerHook::on_message_completed()
     |
     +--record--> MetricCollector aggregates TrialResult
                  (correction_rate, accuracy, tokens, latency, memory relevance)
```

### Promotion Constraints (All Must Pass)

1. `correction_rate` must improve by >= 5%
2. `avg_tokens_per_message` must not increase > 8%
3. `avg_response_time_ms` must not increase significantly
4. `routing_stability` must not decrease significantly
5. `memory_relevance` must not decrease significantly
6. Phase 2: `retrieval_precision` must not drop > 5%
7. Phase 2: `promotion_accuracy` must not drop
8. Phase 3: `rewrite_engagement_rate` must not drop

**Diversity bonus:** When multiple candidates pass, prefer candidates farther from current champion in 19-dimensional parameter space (Euclidean distance).

### LLM-Driven Variant Generation

The autotuner uses LLM to generate 3 trial variants per generation cycle, given:
- Current champion params + metrics
- Recent trial history
- 7-day trend summary
- Behavioral context
- Experiment pace (conservative/balanced/bold)

### Brain Versioning

Each autotuner promotion creates a `BrainVersion` record. `MirrorFacade::revert_to_version()` allows rolling back to any previous brain configuration.

## Coaching System

Event-driven behavioral coaching pipeline in `feature-coaching`:

```
DomainEventBus
     |
     v
SignalAccumulator
(buffers domain events into coaching signals)
     |
     v
Trigger Evaluation
(threshold-based: repeated patterns, focus breaks, etc.)
     |
     +--[Learning template match]--> Fast-path response (no LLM call)
     |
     +--[Complex/novel pattern]---> CoachingReasonerHandler (LLM call)
     |
     v
InterventionRouter
(rate limiter: prevents intervention fatigue)
     |
     v
FeedbackTracker
(tracks user response to coaching for adaptation)
     |
     v
coaching:intervention Tauri event → UI overlay
```

**Focus-mode behavior:** When a focus session is active, coaching triggers are queued rather than delivered immediately. On focus session end, accumulated triggers are consolidated into a post-session debrief (a single comprehensive coaching intervention).

## Insight Generation System

The `feature-insights` crate generates multi-tab LLM-powered reviews of notes, distinct from `InsightForge` (which is a RAG retrieval component in `context_engine`).

| Component | Crate | Purpose |
|-----------|-------|---------|
| `InsightForge` | `context_engine` | Multi-domain RAG retrieval (decompose → search → rank) |
| `InsightService` | `feature-insights` | LLM note review generation (5-tab output, versioned) |

**InsightService output (5 tabs):**
- `synthesis` — unified summary
- `gap_analysis` — knowledge gaps identified (Phase 2: community-level gap detection across notebooks)
- `self_assessment` — understanding evaluation
- `concept_map` — key concept relationships
- `perspectives` — alternative viewpoints (generated via unified debate engine)

**Features:** Versioned insights with evolution timeline, smart merge deduplication, scope resolution (note → notebook → project), persona/squad-based generation via `run_debate()` (unified debate engine), streaming SSE during generation (via `/api/insight/events` endpoint), progress tracking, debate transcript storage.

## Distraction Detection Pipeline

In `feature-productivity`, monitors user activity and detects distractions:

```
macOS Activity APIs (platform-macos)
     |
     v
DistractionMonitor
(watches app switches, idle time, patterns)
     |
     v
DistractionAnalyzer + HeuristicVerdict
(rule-based: known distraction apps, time-of-day patterns)
     |
     +--[High confidence]--> DistractionInterceptor (immediate)
     |
     +--[Uncertain]--------> DistractionClassifierHandler (LLM call)
                                  |
                                  v
                             DistractionInterceptor
                                  |
                                  v
                             Position overlay on cursor's monitor
                             Emit distraction:intervention event
                             Emit distraction:detected event
```

The distraction overlay window (`340x300`, transparent, always-on-top) is positioned on the monitor where the user's cursor is located.

## Focus Timer & Tray Countdown

Two coordinating components in the `desktop` crate:

**FocusTimer:** Runs a `tokio::time::interval(1s)` loop. Supports three modes:
- `Focus` — deep work session
- `Pomodoro` — timed work intervals
- `Break` — rest period

Command channel (`mpsc::Sender<TimerCommand>`) for Extend/Pause/Resume. Emits `focus:tick` and `focus:completed` events.

**Tray Countdown:** Shows next calendar event or task deadline in the macOS menu bar (e.g., "24:57 - Standup"). Polls DB every 30s, ticks every 1s.

**Coordination via `FOCUS_ACTIVE` AtomicBool:**
- When `FocusTimer` starts a session → sets `FOCUS_ACTIVE = true` → tray countdown yields (focus timer owns the tray title)
- When `FocusTimer` ends → `notify_focus_ended()` sets `FOCUS_ACTIVE = false` → countdown resumes
- Both use `tauri::async_runtime::spawn` (not `tokio::spawn`) because they start during Tauri's setup hook

> **Gotcha:** If the focus timer panics without calling `notify_focus_ended()`, the countdown is permanently silenced until app restart.

## Learning System (feature-learning + cognitive)

The learning system spans two crates:

| Crate | Responsibility |
|-------|---------------|
| `feature-learning` | `LearningTool` (user-facing), `CardGenerator` (AI-driven flashcard/knowledge atom generation from notes) |
| `cognitive` | `FlashcardRepo`, `KnowledgeAtomRepo`, `ReviewSessionRepo`, `RetentionHistoryRepo`, `DeckPreferenceRepo` — FSRS-5 scheduling and persistence |

The `LearningEventBus` (separate from `DomainEventBus`) carries `LearningEvent::AnalysisCompleted` for adaptive confidence threshold changes managed by the learning background analysis loop.
