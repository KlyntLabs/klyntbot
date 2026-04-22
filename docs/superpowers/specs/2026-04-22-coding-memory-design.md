# Coding Memory — Design

**Date:** 2026-04-22
**Status:** Draft (pre-implementation)
**Scope:** Single design. Implementation plan will be derived via `writing-plans`.
**Pre-release policy:** Per CLAUDE.md — no user data to migrate, no backward-compat shims, no feature-flag gating. Schema changes consolidated into Phase 1.

---

## 1. Problem statement

Agentic coding tools (Claude Code, Codex, kimi-cli, opencode) ship powerful per-session reasoning but share a critical limitation: **no cross-session memory**. Each session starts blank. The agent re-derives project context, re-learns style preferences, re-attempts fixes it already abandoned, and re-reads files it already understood. Existing memory layers (claude-mem, Mem0, Zep, Letta, Supermemory, ByteRover) are either locked to a single CLI, require SDK integration that CLI tools don't expose, or ship as cloud-backed services unsuitable for local-first use.

klyntbot already owns most of the memory infrastructure the field is converging on — bi-temporal semantic facts, FSRS5 decay, Hebbian co-activation, RRF retrieval, knowledge-graph entities with Louvain communities, a Mirror self-observation subsystem, a Reforge nightly consolidation cycle, and MCP-exposed temporal query tools (`facts_as_of`, `change_history`, `competing_truths`, `decision_points`). None of it is currently pointed at coding.

**What we want:** coding CLIs accumulate memory in klyntbot passively via hooks, Distiller converts raw events into structured memory online, Reforge optimizes and externalizes memory offline, Mirror observes effectiveness in real time, and all of this happens without the user or the coding CLI knowing anything beyond "install klyntbot and flip a toggle."

**The leverage:** the architectural pieces already exist. This design is ~70% *wiring existing capability* and ~30% adding targeted upgrades (counterfactual memory, causal edges, symbol-grounded facts with git invalidation, project-evolving skills).

---

## 2. Goals and non-goals

### Goals

- One local-first memory + cognition layer that works transparently across Claude Code, Codex, kimi-cli, and opencode.
- Dual ingestion: passive (hooks observe agent activity) + active (MCP tools for agent-initiated recall).
- Memory quality sufficient to meet or exceed Supermemory's 88.46% on LongMemEval knowledge-update category.
- Stale facts auto-invalidate on git commits (symbol-grounded memory).
- Recurring bug patterns surface as user-visible alerts before the user re-attempts them.
- Per-project skills evolve automatically from usage — the agent becomes a "harness engineer."
- Zero regression on existing klyntbot personal-AI functionality.

### Non-goals

- Building a new coding CLI (klynt-cli is a future project that will consume this layer).
- Cloud sync or team sharing infrastructure (local-first only for this design).
- Replacing klyntbot's cognitive crate — we extend it, never fork it.
- Observability infrastructure (OpenTelemetry, Prometheus) — CLAUDE.md excludes this explicitly.
- Supporting coding CLIs outside the four above in Phase 1-8 (extensible via the `IngestAdapter` trait; specific CLIs come later).

---

## 3. Key decisions (locked during brainstorming)

| Axis | Decision |
|---|---|
| Project shape | Combined memory + cognition + multi-CLI ingestion. All in one system. |
| Ingestion priority | External CLIs first; native `klynt-cli` integration deferred. |
| Data topology | **Shared store with klyntbot** (Approach A). Same SQLite + LanceDB. Personal and coding memories co-exist with `scope_repo_id` partitioning. |
| Crate placement | Two new crates in klyntbot workspace: `coding-memory` (L5) and `coding-ingest` (L5). No new standalone binary. |
| Distiller timing | Per-turn batched. One LLM call per user turn (not per tool call). |
| Distiller role | **Online writer only.** Emits ADD or SUPERSEDE. Never DELETE. |
| Reforge role | **Offline optimizer only.** Six responsibilities (see §9). Never writes raw memory; never duplicates Distiller's write path. |
| Mirror role | **Real-time observer.** `PatternEffectivenessSubscriber` updates `effectiveness_score` within seconds. |
| Integration surface | Hooks (passive) + MCP tools (active). No LLM proxy, no ACP in Phase 1-7. |
| Daemon lifecycle | klyntbot desktop owns the ingest socket. Hook falls back to file buffer when desktop is off; user sees warning. |
| User install path | Desktop UI settings page (no separate CLI install command). |
| Rule artifacts | Reforge writes managed-block sections of `CLAUDE.md`, `AGENTS.md`, `.cursorrules`. User hand-edits preserved outside the markers. |
| Schema approach | Consolidated Phase-1 migration for every column/table across all 8 phases. Pre-release authorizes direct schema changes. |

### The invariants

1. **Provenance-always.** Every fact carries `metadata.provenance.source_events` — no unknowable writes.
2. **Distiller-never-deletes.** Count of rows monotone non-decreasing after any Distiller cycle.
3. **Reforge-never-deletes-raw.** All prior episodic memories survive any Reforge cycle.
4. **Bi-temporal monotone.** `valid_until >= valid_from` always holds.
5. **SUPERSEDE chain.** Predecessor's `valid_until == successor's valid_from`.
6. **Scope isolation.** Repo-scoped retrieval never leaks cross-repo facts (except global `scope_repo_id IS NULL` facts).
7. **Hook round-trip identity.** `parse(serialize(AgentEvent)) == AgentEvent` for all CLI formats.
8. **Causal edge validity.** No dangling `from_id` or `to_id` references.
9. **Budget enforcement.** SessionStart injection ≤ 800 tokens; UserPromptSubmit ≤ 1500 tokens.

All nine enforced via `proptest!` in `tests/coding_memory_property.rs`.

---

## 4. Architecture and topology

### Component diagram

```
┌──────────────────────────────────────────────────────────────┐
│                    External Coding CLIs                       │
│  Claude Code   Codex   kimi-cli   opencode   (klynt-cli — future) │
└───────┬─────────┬────────┬──────────┬─────────────────────────┘
        │ hooks   │ hooks  │ hooks/wire │ poll
        ▼         ▼        ▼            ▼
     ┌────────────────────────────────────────┐
     │      klyntbot-hook (shell binary)      │
     │  CLI-specific adapters → AgentEvent    │
     └──────────────┬─────────────────────────┘
                    │ Unix socket ~/.klyntbot/ingest.sock
                    │ (fallback: ~/.klyntbot/ingest-buffer.jsonl)
                    ▼
     ┌────────────────────────────────────────┐
     │  klyntbot desktop — ingestion daemon   │
     │  ┌─────────────────────────────────┐   │
     │  │ ingest_event_log (SQLite)       │   │
     │  └──────────────┬──────────────────┘   │
     │                 │ turn-boundary trigger │
     │                 ▼                      │
     │  ┌─────────────────────────────────┐   │
     │  │ Distiller                       │   │
     │  │  Phase A: extractive            │   │
     │  │  Phase B: LLM via Provider Mgr  │   │
     │  │  Phase C: reconciliation        │   │
     │  └──────────────┬──────────────────┘   │
     │                 ▼                      │
     │  ┌─────────────────────────────────┐   │
     │  │  klyntbot cognitive store       │   │
     │  │  semantic_facts (+scope_repo_id │   │
     │  │    +metadata)                   │   │
     │  │  episodic_memories (+kind)      │   │
     │  │  procedural_rules               │   │
     │  │  memory_causal_edges (NEW)      │   │
     │  │  LanceDB vectors (unchanged)    │   │
     │  └────────┬───────────────┬────────┘   │
     │           │               │            │
     │   ┌───────▼─────┐  ┌──────▼──────┐     │
     │   │ Recall API  │  │   Reforge   │     │
     │   │ MCP + hooks │  │ 7 phases    │     │
     │   └───────┬─────┘  └──────┬──────┘     │
     │           │               │            │
     │           └───────┬───────┘            │
     │                   ▼                    │
     │         ┌─────────────────┐            │
     │         │  Mirror         │            │
     │         │  subscribers    │            │
     │         └─────────────────┘            │
     └────────────────────────────────────────┘
```

### Crate layout

Two new crates in the klyntbot workspace at L5 (alongside `agent`, `cognitive`, `channels`):

```
bot/crates/
├── coding-memory/            NEW (L5)
│   ├── src/
│   │   ├── facts.rs          # RepoContext, FixAttempt, StylePreference, WorkflowPattern, FailurePattern, DeadEndAttempt
│   │   ├── scope.rs          # RepoScope, ProvenanceMetadata, AnchoredSymbol, CausalEdge
│   │   ├── distiller/        # per-turn LLM extraction
│   │   ├── recall/           # CodingRecallService, progressive disclosure renderers
│   │   ├── reforge_phase.rs  # Phase 2.5 (Coding Synthesis), Phase 3.5 (Rule Artifact Generation)
│   │   └── skills.rs         # scope-aware SkillStore extension, project skill evolution
│   └── migrations/           # Phase-1 consolidated schema delta
│
└── coding-ingest/            NEW (L5)
    ├── src/
    │   ├── event.rs          # AgentEvent, EventKind (single source of truth)
    │   ├── adapters/
    │   │   ├── claude_code.rs
    │   │   ├── codex.rs
    │   │   ├── kimi_wire.rs
    │   │   └── opencode.rs
    │   ├── transport.rs      # Unix socket + file-buffer fallback
    │   └── daemon.rs         # desktop-embedded ingestion server
    └── bin/klyntbot-hook.rs  # tiny stdin reader/dispatcher
```

**Dependencies (upward only):** `coding-memory` depends on `cognitive`, `context-engine`, `storage`, `bus`, `providers`. `coding-ingest` depends on `coding-memory`, `bus`, `storage`, `common`. Neither reaches into desktop/tauri.

### Schema deltas (all Phase 1, consolidated)

| Target | Change |
|---|---|
| `semantic_facts` | `ADD COLUMN scope_repo_id TEXT NULL` |
| `semantic_facts` | `ADD COLUMN metadata TEXT NULL` (JSON: provenance, anchored_symbols, sensitivity, etc.) |
| `semantic_facts` | `ADD COLUMN actor_id TEXT DEFAULT 'local_user'` (forward-compat for future multi-user; implementation out of scope) |
| `episodic_memories` | `ADD COLUMN kind TEXT DEFAULT 'general'` (values: `general \| fix_attempt \| test_run \| refactor \| review \| turn_trace`) |
| `episodic_memories` | `ADD COLUMN actor_id TEXT DEFAULT 'local_user'` |
| `ingest_event_log` | `CREATE TABLE` — append-only AgentEvent buffer with `processed BOOLEAN DEFAULT FALSE`, `actor_id TEXT DEFAULT 'local_user'` |
| `memory_causal_edges` | `CREATE TABLE (id, from_id, to_id, edge_kind, confidence, inferred_at)` |
| `memory_utilization` | `CREATE TABLE (memory_id, retrieved_at, cited_in_response BOOLEAN)` |
| `skill_versions` | `ADD COLUMN scope TEXT DEFAULT 'global'` + `ADD COLUMN scope_repo_id TEXT NULL` |

All additions. No renames, no drops, no breaking changes.

### Binaries

- **`klyntbot-hook`** *(new, tiny)* — shell binary users' coding CLIs spawn per hook. Reads stdin JSON, normalizes to `AgentEvent`, writes to Unix socket (or file buffer), exits <5ms.
- **`klyntbot-mcp`** *(extended)* — existing MCP server gains coding-specific recall tools (see §8).
- **`klyntbot`** desktop *(extended internally, surface unchanged)* — owns the ingest socket, runs the per-turn Distiller, hosts the full Reforge cycle, writes rule artifacts, surfaces Mirror alerts.

### Installation story

```
1. User installs klyntbot desktop (existing flow).
2. Desktop UI — Settings → "Coding CLI Integration" page.
3. Toggle per supported CLI (Claude Code / Codex / kimi-cli / opencode).
4. Flipping toggle ON:
   - writes hook config to CLI's settings file (~/.claude/settings.json, etc.)
   - backs up original first
   - starts monitoring ingest socket
5. User starts using the coding CLI — memory accumulates silently.
6. Next session: SessionStart injection surfaces relevant prior memory.
```

If the desktop app is not running, `klyntbot-hook` falls back to file buffering and emits a rate-limited stderr warning visible in the coding CLI's notification surface.

---

## 5. Ingestion layer

### AgentEvent (the core contract — versioned from day one)

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "v", rename_all = "camelCase")]
pub enum AgentEvent {
    V1(AgentEventV1),
    // future V2 never breaks V1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventV1 {
    pub id: Uuid,
    pub source: AgentSource,          // ClaudeCode | Codex | KimiCli | OpenCode | KlyntCli
    pub session_id: String,
    pub turn_id: Option<String>,
    pub cwd: PathBuf,
    pub repo: Option<RepoScope>,
    pub occurred_at: Timestamp,
    pub kind: EventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    SessionStart { model: Option<String>, source_reason: String },
    SessionEnd   { reason: String },
    UserPrompt   { text: String, attachments: Vec<PathBuf> },
    AssistantMsg { text: String, truncated: bool, token_usage: Option<TokenUsage> },
    ToolCall     { tool: String, args_preview: String, ok: bool, duration_ms: u32, result_preview: String },
    FileEdit     { path: PathBuf, op: FileOp, bytes: u64, diff_preview: Option<String> },
    TestRun      { command: String, framework: Option<String>, passed: u32, failed: u32, duration_ms: u32 },
    CompactEvent { trigger: String, token_count: u32 },
    Error        { tool: Option<String>, message: String },
}
```

### CLI adapter mappings

| CLI | Transport | Captured events | Notes |
|---|---|---|---|
| Claude Code | shell hook (`type: command`) → `klyntbot-hook claude-code` | 7 hook events filtered from 27 | `Bash` with `pytest\|cargo test\|npm test\|go test` pattern → `TestRun`, else `ToolCall` |
| Codex | shell hook (TOML config) → `klyntbot-hook codex` | 5 hook events | `tool_kind` provides richer semantic hints |
| kimi-cli | shell hook (tier 1) or Wire client (tier 2) | 13 hook events; Wire adds streaming text | Wire path enables richer `AssistantMsg` capture |
| opencode | SQLite WAL polling (500ms) | messages table diff | best-effort only; opt-in |

### Transport

**Hot path:**
```
hook fires → klyntbot-hook parses → connect to ~/.klyntbot/ingest.sock
  → send 4-byte LE length + JSON → exit(0) (< 5ms, fire-and-forget)
```

**Cold path (desktop off):**
```
klyntbot-hook → socket connect fails → open(O_APPEND) ~/.klyntbot/ingest-buffer.jsonl
  → append one line → rate-limited stderr warning (once per session via touch-file)
  → exit(0)

[Desktop next startup]
  → drain ingest-buffer.jsonl → insert rows into ingest_event_log
  → archive + truncate buffer (50MB rotation, 7-day TTL, 500MB hard cap)
  → run pending Distiller turns on drained events
```

### Three-tier user-facing warning

1. Per-session stderr message from `klyntbot-hook` on buffer-fallback (rate-limited via touch-file).
2. `klyntbot status` CLI command shows socket state + buffer size + last distillation time.
3. Install-time copy in the desktop UI settings page: explicit note that desktop must run in background.

### Install flow (desktop UI driven)

- Toggle writes hook stanza to the CLI's settings file (atomic write; pre-install backup).
- Toggle OFF reverses cleanly.
- UI "Diagnose" button invokes `klyntbot-hook` with a synthetic event to verify the install works.

### Path-based exclusion (privacy — credentials must never reach the store)

Sensitive files are filtered at the **hook level**, before an event enters `ingest_event_log`. A Distiller that reads `.env` or `secrets/*` would permanently memoize credentials — unacceptable.

**Default `excludePaths`** (shipped, user can extend):
```json
{
  "codingMemory": {
    "ingest": {
      "excludePaths": [
        "**/.env", "**/.env.*",
        "**/secrets/**", "**/private/**",
        "**/*.key", "**/*.pem", "**/*.p12", "**/*.pfx",
        "**/id_rsa", "**/id_ed25519", "**/known_hosts",
        "**/.aws/credentials", "**/.gcloud/**", "**/.kube/config",
        "**/node_modules/**", "**/target/**", "**/.git/**"
      ]
    }
  }
}
```

**Filtering behavior:**
- `klyntbot-hook` evaluates `FileEdit.path` and `ToolCall.args_preview` against each glob; matches cause the event to be dropped **before** socket write (not even buffered on disk).
- Compressed events (where `AssistantMsg.text` or `result_preview` contains a match against path patterns) are truncated with `"[redacted: sensitive path]"` marker.
- The Distiller also applies the same globs as a defense-in-depth layer: even if an event somehow reaches `ingest_event_log`, facts derived from excluded paths are rejected.
- User can add project-specific exclusions via per-repo config `<repo_root>/.klyntbot/ignore.toml`.

**Not a content scanner.** We do not regex-grep file contents for secrets — that's out of scope (false-positive-heavy, infinite maintenance). Path-based exclusion is a coarse but reliable filter; users with stricter needs should set their editor to not open secret files.

---

## 6. Distiller (online writer)

### Timing

- **Trigger:** `EventKind::SessionEnd` OR in-memory turn buffer reaches a turn boundary (receiving an `AssistantMsg` with token usage, or 2 minutes of silence after the last event).
- Buffer holds all events between consecutive `UserPrompt`s.
- On trigger: read `ingest_event_log` rows for `(session_id, turn_id)`, mark `processing = true` atomically.

### Two-phase processing

**Phase A — Extractive (always runs, no LLM, <50ms):**

Deterministic pass producing a `TurnTrace`:
- `files_read: Vec<PathBuf>` from `FileEdit { op: Read }`
- `files_modified: Vec<PathBuf>` with byte deltas
- `commands_run: Vec<String>` from `Bash` tool calls
- `test_outcomes: Vec<TestRun>` directly from `TestRun` events
- `errors_encountered: Vec<(tool, message)>`
- `token_usage` from `AssistantMsg`

Always written to `episodic_memories` with `kind = 'turn_trace'`. This is the baseline durable record — never lost even if Phase B fails.

**Phase B — LLM synthesis (via existing `ProviderManager`):**

Input to the LLM:
- User prompt text
- Assistant final message
- `TurnTrace` from Phase A
- System prompt: *"You are a memory distiller. From this coding-agent turn, emit zero or more structured observations via the `record_observation` tool. Emit nothing if nothing significant happened."*

The LLM responds via a tool call:

```rust
record_observation(
    kind: "fix_attempt" | "style_preference" | "workflow_pattern"
        | "repo_context" | "failure_pattern",
    subject: String,
    predicate: String,
    object: String,
    confidence: f32,          // 0.0-1.0
    scope: "global" | "repo",
    reasoning: String,
)
```

**Model:** whatever the user has configured in `ProviderManager`. No hardcoded Claude. New provider role declared: `ProviderRole::Distiller` (defaults to user's small-tier model).

### Phase C — Reconciliation (Mem0-style, no DELETE)

For each emitted observation:
1. Vector-retrieve top-5 similar existing facts, filtered by `scope_repo_id` + `domain`.
2. If max similarity > 0.9 AND `(subject, predicate)` exact match → skip; bump `access_count` and update stability via existing FSRS5 path.
3. If max similarity > 0.75 → write new fact with `supersedes: <predecessor_id>`; predecessor's `valid_until` left NULL (Reforge decides).
4. Else → write fresh fact with `valid_from = now`, `valid_until = NULL`.

All writes route through existing `SemanticFactRepo::upsert` / `EpisodicMemoryRepo::insert`. This inherits Hebbian co-activation, FSRS5 init, vector embedding automatically.

### Provenance-always (invariant enforcement)

Every write includes in `metadata`:
```json
{
  "provenance": {
    "source_events": ["<ingest_event_log.id>", ...],
    "session_id": "...",
    "turn_id": "...",
    "distilled_at": "2026-04-22T...",
    "distiller_model": "<model_id>",
    "source_kind": "distiller_extractive" | "distiller_llm" | "user_corrected"
  }
}
```

In dev builds, writes without valid provenance panic. In release, they are logged and rejected. Enforced by `ReforgeWriter` wrapper.

### Failure modes

| Failure | Behavior |
|---|---|
| LLM provider down | Phase A writes complete; Phase B retries 1m / 5m / 30m; fact marked `distillation: pending` in metadata |
| LLM returns malformed tool call | Log + drop observation; Phase A turn trace preserved |
| LLM times out (>30s) | Cancel; retry on next Reforge cycle |
| Reconciliation similarity lookup fails | Fall back to fresh write (Reforge dedupes later) |
| Cost ceiling breached | Throttle; queue remainder |

Typical cost at Haiku-tier prices: ~$0.0003/turn; heavy day of 500 turns < $0.15.

---

## 7. Coding fact taxonomy

### Principle

**No new memory types. No new tables (beyond §4's deltas).** Coding facts reuse `SemanticFact`, `EpisodicMemory`, `ProceduralRule` with a subject/predicate vocabulary layered on top.

### Distiller-written kinds (8 — 5 via LLM, 3 via extractive or derivation)

| Kind | Memory type | Scope | Emitted by | Notes |
|---|---|---|---|---|
| `FixAttempt` | `EpisodicMemory { kind: 'fix_attempt' }` | repo | LLM (`fix_attempt`) | Structured JSON content: `{problem_hash, problem, files, approach, outcome, insight, duration_ms, test_before, test_after}` |
| `DeadEndAttempt` | `EpisodicMemory` + `SemanticFact { memory_type: 'counterfactual' }` | repo | LLM (`fix_attempt` with `outcome: failure \| abandoned`) + derived counterfactual | Episode PLUS a derived counterfactual fact linking the problem to its failed approach |
| `TestRun` | `EpisodicMemory { kind: 'test_run' }` | repo | Phase A extractive (no LLM) | Fast, deterministic, always captured from `EventKind::TestRun` |
| `RefactorEpisode` | `EpisodicMemory { kind: 'refactor' }` | repo | Phase A extractive (detected from file-edit patterns) | Tagged with `anchored_symbols` |
| `StylePreference` | `SemanticFact { domain: 'preferences' }` | global (default) / repo | LLM (`style_preference`) | Subject `user`, predicate `prefers \| avoids \| uses \| dislikes` |
| `RepoContext` | `SemanticFact { domain: 'work' }` | repo | LLM (`repo_context`) + Phase A extractive for deterministic facts (test_command, package_manager) | Subject `repo:<canonical_id>`, predicate `framework \| language \| package_manager \| test_command \| lint_command \| deployment \| convention \| architecture_layer \| depends_on \| has_gotcha` |
| `WorkflowPattern` | `ProceduralRule { source: 'observed' }` | repo / global | LLM (`workflow_pattern`) | Low confidence at Distiller time; effectiveness tracked by Mirror |
| `FailurePattern` | `ProceduralRule { source: 'observed' }` | repo / global | LLM (`failure_pattern`) | Recurring failure + remediation |

**LLM tool schema — `kind` enum exactly 5 values:** `fix_attempt | style_preference | workflow_pattern | repo_context | failure_pattern`.

### Reforge-only kinds (3)

The Distiller MUST NOT emit these. The `record_observation` tool schema enforces a 5-value enum.

| Kind | Memory type | Source |
|---|---|---|
| `ProblemSolutionPattern` | `ProceduralRule` + supporting `memory_causal_edges` | Reforge Phase 2.5 — promoted from ≥3 causal chains sharing `problem_hash` prefix |
| `ProjectUnderstanding` | `SemanticFact` with high `convergence_score` | Reforge Phase 2.5 — synthesized from convergent `RepoContext` facts |
| `UserHabit` | `ProceduralRule { source: 'reflected' }` | Reforge Phase 2.5 — abstracted from `WorkflowPattern`s observed across ≥3 repos |

### Memory-type taxonomy (free-form string on `SemanticFact`)

| Value | Semantics |
|---|---|
| `fact` *(default)* | Observed, high-confidence, low-revision |
| `counterfactual` | "Tried X, didn't work" — `predicate: failed_because \| avoided_due_to` |
| `opinion` | Agent belief with confidence that can be revised on new evidence (Hindsight-style) |
| `observation` | Raw observed behavior, pre-distillation |

### Sensitivity tagging (privacy tier)

Every memory carries `metadata.sensitivity: "normal" | "high" | "excluded"`:

| Value | Behavior |
|---|---|
| `normal` *(default)* | Standard retrieval, standard externalization to rule artifacts |
| `high` | Retrieved normally but **never** externalized to `CLAUDE.md` / `AGENTS.md` / `.cursorrules` or any on-disk artifact outside klyntbot's own SQLite |
| `excluded` | Hidden from retrieval unless explicit `include_excluded: true` flag passed; used for "I tried a password but don't want this remembered" cases |

The Distiller auto-tags `high` for facts derived from tool calls with paths matching an extended sensitivity set (auth-related paths, billing/payment paths). The user can promote/demote sensitivity via the workbench UI (see §11.5).

Reforge's Rule Artifact Generation phase filters out `high` and `excluded` before calling the LLM — sensitive content never reaches the externalized markdown.

### Scope semantics and retrieval

`semantic_facts.scope_repo_id` is the partitioning key:

| Query context | Filter |
|---|---|
| Repo-scoped (agent working in repo X) | `scope_repo_id = X OR scope_repo_id IS NULL`; +0.15 relevance boost on repo matches |
| Cross-repo (user explicitly asks) | `scope_repo_id IS NOT NULL AND scope_repo_id != X` |
| Personal (no repo context) | `scope_type = 'user' AND scope_repo_id IS NULL` |
| Mixed (default) | All; RRF + repo boost sorts |

Hebbian co-activation works across scopes — cross-domain insights emerge naturally.

### Tier A — activate existing capabilities (zero new code)

These are already in klyntbot, waiting for coding data to flow in:

| Capability | Where | Use for coding |
|---|---|---|
| Bi-temporal query surface | `TemporalTool` at `crates/tools/src/domain/temporal.rs` | `facts_as_of`, `change_history`, `competing_truths`, `decision_points`, `knowledge_diff` already MCP-exposed |
| Value-density tiering | `score_turn()` at `crates/cognitive/src/services/value_density.rs:115` | Already contains coding verbs (deployed/fixed/broke/shipped/migrated/refactored). Auto-tiers coding turns |
| Contradiction events | `DomainEvent::ContradictionDetected` at `crates/bus/src/domain_events.rs:317` | Fires automatically when new fact conflicts with existing |
| User-correction ingestion | `DomainEvent::UserCorrectedAI` with `CorrectionKind::MemoryMiss` | Highest-signal negative training data |
| Knowledge atoms | 7 types including `procedure` and `pattern` | Coding-ready |
| Autotuner trial surface | 12 relevance weights + 27 params in `TrialParams` | Add `session_type` discriminator; self-tune coding vs personal separately |

### Tier B — small additive upgrades (Phase 3)

| # | Upgrade | Cost | Addresses |
|---|---|---|---|
| B1 | Counterfactual memory (`memory_type = 'counterfactual'`) + `DeadEndAttempt` kind | Zero schema change (free-form memory_type) | Counterfactual blindness, error propagation |
| B2 | Always-on provenance metadata | Uses new `metadata` column | Provenance loss, summarization drift |
| B3 | `code_state` axis on `UserSituationSnapshot` | ~40 lines in `crates/context_engine/src/rewriter.rs` | Context-insensitive retrieval |
| B4 | `CodeDomainSearcher` registered in `InsightForge` | ~50 lines implementing `DomainSearcher` trait | Retrieval precision |
| B5 | Autotuner `session_type` tag in `ShadowContext` | ~10 lines | One-size-fits-all tuning |

### Tier C — game-changers (Phases 1 schema, 6 behavior)

**C1 — Causal failure edges (MAGMA-style).** Schema: `memory_causal_edges` table (Phase 1). Behavior (Phase 6): session-end pass counts causal chains by `(edge_kind, problem_hash)`; Phase 2.5 promotes clusters of ≥3 to `ProblemSolutionPattern`. Auto-populated from: test pass→fail transitions; `FixAttempt` + `TestRun` correlations within a turn. New MCP tool `trace_causes(subject, repo?, depth)` walks the graph.

**C2 — Symbol-grounded memory with git invalidation.** Schema: `anchored_symbols` in fact `metadata` JSON (Phase 1). Behavior (Phase 6): tree-sitter extraction at Distiller time adds `SymbolRef { file_path, symbol, git_hash }` to each coding fact. A git post-commit hook (a `klyntbot-hook` subcommand) fires on every commit, diffs changed files, queries fact store for anchored facts, invalidates bi-temporally if symbol deleted, marks `stale_candidate` if file changed but symbol survives. Reforge's deep validation runs a full tree-sitter pass against current codebase state.

**C3 — Failure-state-aware retrieval (Skill-RAG-inspired).** Behavior (Phase 4): after the 12-factor scoring, a `RetrievalQualityProbe` computes `coverage_score = mean(top_k.sim) - min(top_k.sim)`. Below threshold → dispatch to a **formalized retrieval-skill registry**:

```rust
pub trait RetrievalSkill: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn apply(&self, ctx: &EscalationContext) -> Result<EscalationOutcome>;
    fn effectiveness_score(&self) -> f32;   // EMA-updated from outcomes
}
```

Phase-4 registrations (closed set):

| Skill | Tier | Behavior |
|---|---|---|
| `QueryRewriter` | `deep_think` | Uses existing `QueryPipeline` PRF + multi-query expansion to generate 3 rewrites; re-retrieves and RRF-merges |
| `QueryDecomposer` | `deep_think` | Splits compound queries into 2-4 sub-queries; retrieves per-sub; merges |
| `EvidenceFocuser` | `deep_think` | Uses cross-encoder reranker on top-20 candidates to identify most relevant 5 |
| `RawEventEscalator` | `ultra` | Bypasses summaries; uses provenance pointers (Tier B2) to retrieve raw `ingest_event_log` rows matching query |
| `CausalContextExpander` | `ultra` | Walks `memory_causal_edges` from top-k results; surfaces causal chains as additional context |

**Skill selection logic:** the probe passes `(coverage_score, query_shape, active_budget_tier)` to a selector; selector tries skills in order of `effectiveness_score`; each skill may succeed (coverage now acceptable), fail (try next), or escalate (increase budget tier).

**Effectiveness tracking:** each `apply()` publishes `DomainEvent::RetrievalSkillApplied { skill, before_score, after_score, budget_used, session_id }`. `PatternEffectivenessSubscriber` (§10) updates per-skill EMA over outcomes measured via downstream `UserCorrectedAI` rate. Low-effectiveness skills get deprioritized in the selector.

Reforge Phase 6 trains the coverage-score threshold itself as part of its autotuner surface.

---

## 8. Recall API

### Two paths, one engine

```
Coding CLI
    │
    ├─ PASSIVE (hook-injected additionalContext, no agent call)
    │  → klyntbot-hook context subcmd
    │
    └─ ACTIVE (agent-initiated MCP tool call)
       → klyntbot-mcp recall_* tools

Both → CodingRecallService (in coding-memory crate):
         1. scope resolve (cwd → git → repo_id)
         2. QueryPipeline enrichment (PRF + multi-query)
         3. UnifiedMemoryService.retrieve()
         4. failure-state probe (C3)
         5. escalate if coverage low
         6. dead-end check (B1)
         7. render (JSON for MCP / markdown for injection)
```

### MCP tool surface (progressive disclosure)

```rust
// Layer 1 — compact index
recall_index(query, repo?, kinds?, days?, limit=10)
    -> { results: [IndexEntry{id, kind, title, when, scope, confidence, token_cost}],
         coverage_score: f32, escalation_available: bool }

// Layer 2 — chronological framing
recall_timeline(ids | query, repo?, days=30)
    -> { entries: [TimelineEntry{id, kind, when, snippet, related_ids}] }

// Layer 3 — full structured content + provenance
recall_fetch(ids, include_provenance?, include_causal_graph?)
    -> { entries: [FullEntry{id, kind, content, metadata, causal_edges, supersedes, superseded_by}] }

// Specialized
trace_causes(subject, repo?, depth=3)      -> CausalTraceResponse
check_dead_ends(approach, repo?)           -> DeadEndResponse
recall_facts_as_of(subject, predicate, as_of)  -> FactsAsOfResponse
recall_change_history(subject, predicate, repo?) -> ChangeHistoryResponse
recall_decision_points(domain: "code", repo?)    -> DecisionPointsResponse
```

All tools added to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`. All prefixed `recall_` / `trace_` / `check_` to avoid collision with existing `memory` tool.

### Passive injection — SessionStart (800-token budget)

Markdown structure:

```markdown
## Project memory — <repo_id>

### What you need to know about this repo
- <RepoContext facts, up to 6>

### Your preferences (relevant)
- <StylePreference facts filtered by semantic relevance>

### Recent activity (last 7 days)
| when | what | id |
|---|---|---|
| 2d ago | <fix_attempt title> | ep_7f2... |
| 5d ago | ❌ tried <approach> — abandoned (<reason>) | ep_9c4... |

### Open threads
- <unfinished turn traces from last SessionEnd>

*Call `recall_fetch(ids=[...])` for details.*
```

### Passive injection — UserPromptSubmit (1500-token budget)

Steps:
1. Enrich query via existing `QueryPipeline`.
2. Retrieve via `UnifiedMemoryService` with coding-tuned weights (Tier B5).
3. **Failure-state probe (C3):** compute `coverage_score`. Below threshold → escalate.
4. **Dead-end check (B1):** if prompt matches a counterfactual with confidence > 0.7, inject warning block.
5. Render to markdown.

Sample output:

```markdown
## Relevant memory for this turn

### ⚠️ Heads-up
You previously tried **<approach>** (<date>) — abandoned because <reason>.

### Likely relevant
- [ep_7f2] 2d ago: <title> (<location>)
- [rc_01] RepoContext: <fact>

### Causal context
- commit bf2a (today) → broke → test X → similar past failure was ep_5e1 (fixed in <location>)

*Fetch details: `recall_fetch(ids=["ep_7f2", "ep_5e1"])`*
```

If `code_state = StackTraceActive { error_type }`, the error type is prepended to the query bundle before retrieval.

### Scope resolution

- Default: `cwd → git rev-parse → remote origin URL → canonical repo_id`. Cached per-session.
- Explicit override: all MCP tools accept `repo: Option<String>`.
- Cross-repo: `repo: "*"` removes repo filter.
- No repo detected: falls back to `local:<sanitized-path>` with warning.

---

## 9. Reforge — cognitive optimizer cycle

### The invariant (verbatim from user spec)

> **Distiller writes memory. Reforge improves memory.**
> Reforge never overwrites or replaces the Distiller's raw memory. It creates *derived* knowledge, adjusts *how* memory is retrieved, and externalizes memory into artifacts. Raw episodic memory and original semantic facts are always preserved in the bi-temporal store.

Enforced at code level: `ReforgeWriter` wrapper rejects DELETE operations with runtime panic (dev) / log+skip (release).

### Two-tier timing

| Tier | Trigger | Duration | LLM calls | Scope |
|---|---|---|---|---|
| Session-end light | `SessionEnd` event, per-repo | < 2 seconds | 0 | Just this session |
| Nightly heavy | Cron at 3 AM local | 2-10 minutes | 3-6 | All memory |

### Session-end light pass (no LLM)

1. **Hebbian bump** — increment `co_activation` for every co-retrieved pair this session.
2. **Within-session exact-dedup** — merge same-`problem_hash` FixAttempts; append `attempts_count += 1`.
3. **Stale-candidate resolution** — re-validate facts marked `stale_candidate` by C2 git hook if their anchored file was touched this session.
4. **Session summary cache** — 200-token markdown summary to `session_summaries` table for next SessionStart.

### Nightly heavy cycle — mapping the 6 responsibilities

| User spec responsibility | Phase | What happens |
|---|---|---|
| **1. Pattern Extraction** | 2.5 — Coding Synthesis (NEW) | LLM consumes sessions + new FixAttempts + causal edges + active WorkflowPatterns. Emits: ExtractPattern, ExtractFailurePattern, PromoteToProblemClass |
| **2. Memory Consolidation** | 6.5 — Graph Consolidation (EXTENDED) | Existing Louvain + community merge. Extended: cross-session fact-dedup via vector similarity > 0.92 → supersede older with newer, preserve both in bi-temporal history |
| **3. Abstraction & Promotion** | 2.5 (same LLM call) | Within same synthesis: PromoteToProjectUnderstanding, PromoteToUserHabit, PromoteToProblemSolutionPattern (from causal chains) |
| **4. Knowledge Graph Refinement** | 6.5 (EXTENDED) | Existing Hebbian + communities. Extended: tree-sitter pass produces SymbolRef entities; `depends_on`/`called_by`/`modifies` edges in `entity_relationships` |
| **5. Retrieval Optimization** | 6 — Optimize (EXTENDED) | Existing autotuner. Extended: partition champion sets by `session_type`. Selective-delete signal: memories retrieved ≥N times, never cited → `stability *= 0.5` (demote, don't delete) |
| **6. Rule Generation** | 3.5 — Rule Artifact Generation (NEW) | LLM reads active patterns/preferences/understanding with `confidence ≥ 0.7` and `stability ≥ 0.5`. Writes managed-block sections of per-repo CLAUDE.md / AGENTS.md / .cursorrules |

### Game-changer integration into Reforge

- **C1 causal edges:** session-end counts chains by `(edge_kind, problem_hash)`; Phase 2.5 promotes ≥3 to `ProblemSolutionPattern` with source edges in `metadata.supporting_chains`.
- **C2 git invalidation:** post-commit hook handles single-commit immediate invalidation; Phase 6.5 extension runs deep tree-sitter validation against current codebase state. Facts with surviving symbols but drifted semantics → `needs_review`.
- **C3 escalation thresholds:** Phase 6 trains thresholds based on escalations that led to reduced `UserCorrectedAI` rates.

### Project-scoped evolving skills (Phase 3.5 sub-phase)

New paradigm: skills adapt per project based on observed patterns. The agent becomes a "harness engineer."

**Storage (configurable per repo):**
- Default (private): `~/.klyntbot/project-skills/<sanitized-repo-id>/<skill-name>/SKILL.md`
- Team-shared (opt-in): `<repo_root>/.klyntbot/skills/<skill-name>/SKILL.md`

**Schema:** `skill_versions.scope` + `skill_versions.scope_repo_id` added in Phase 1.

**Evolution process:**
1. Detect: `WorkflowPattern`s with `scope: repo` and `confidence ≥ 0.7` not yet expressed as skills.
2. Synthesize (LLM): draft `SKILL.md` with `name`, `description`, `whenToUse`, procedure steps, references to supporting `FixAttempt` episodes + `RepoContext` facts. Start with `effectiveness_score: 0.5`.
3. Write via existing `SkillFileManager` with managed-block markers.
4. Journal the version via existing `SkillVersionRow`.
5. Evaluate continuously (Mirror's `PatternEffectivenessSubscriber` updates `effectiveness_score` in real time).
6. Supersede: when a newer WorkflowPattern with overlapping scope achieves higher effectiveness, old skill's managed block gets `status: superseded`, retained for audit.

**Skill-level autotuner surface:** `SkillListingSource` threshold becomes tunable per scope; low-effectiveness skills naturally stop injecting into context over time.

### Managed-block pattern for CLAUDE.md / AGENTS.md / .cursorrules

```markdown
<!-- klyntbot:managed:start | generated: 2026-04-22T03:00Z | cycle: 247 -->
# Repo notes (klyntbot-managed)

## Architecture
- <synthesized facts>

## Style preferences observed
- <synthesized preferences>

## Known failure patterns
- <synthesized failure remediations>
<!-- klyntbot:managed:end -->

<!-- your hand-written content below this marker is preserved -->
```

Reforge rule writer:
1. Read existing file if present.
2. Locate managed block by markers; if absent, insert at top.
3. Rewrite managed block; hash-check user content outside the range.
4. Write atomically. Journal diff in `skill_versions`.
5. Skip write if user modified managed range (surface as `mirror_snippet` for user attention).

### Failure isolation

Each phase runs with independent `Result<()>`. A phase failure logs to `mirror_snippets` and records telemetry but never cascades. Phase 2.5 LLM failure → skip synthesis this cycle, continue to Phase 3. Phase 3.5 LLM failure → don't regenerate rule artifacts, but don't break retrieval optimization. System gracefully degrades to "raw memory only" during LLM outages.

---

## 10. Mirror integration

### Subscribers (extend existing + add two new)

| Subscriber | Status | Coding-specific behavior |
|---|---|---|
| `RoutingMirrorSubscriber` | Extend | Subscribes to `SkillRouted { scope: Project }`. Accumulates per-project-skill activation. Drift detection: skill activation drops >50% over 7 days while repo active → `ProjectSkillObsolete` alert |
| `MetaRuleDetector` | Extend | Subscribes to `FixAttemptFailed`. Triggers meta-rule when same `problem_hash` fails ≥3 times across sessions |
| `PatternEffectivenessSubscriber` | NEW | Real-time. Subscribes to `PatternApplied`, `PatternOutcome`. Updates `effectiveness_score` via EMA `score = 0.9 * score + 0.1 * outcome_value` |
| `StaleMemorySubscriber` | NEW | Subscribes to `MemoryRetrieved` + `AssistantMsgCompleted`. Computes cited vs. ignored. Flags consistently-ignored memories for Reforge's selective-delete signal |

### New DomainEvent variants (added to existing 63)

```rust
DomainEvent::PatternApplied { pattern_id, session_id, repo, source: "skill_listing"|"recall_injection"|"explicit_call" }
DomainEvent::PatternOutcome { pattern_id, outcome: "success"|"partial"|"failure", evidence, measured_at }
DomainEvent::FixAttemptFailed { problem_hash, repo, attempt_count }
DomainEvent::MemoryRetrieved { memory_ids, query, session_id, turn_id }
DomainEvent::AssistantMsgCompleted { session_id, turn_id, cited_memory_ids }
```

### Coding meta-rules

| Trigger | Action |
|---|---|
| `FixAttempt.problem_hash == X` fails ≥3 times across sessions | `ProblemClassRefactor { suggestion: "recurring — consider architectural change" }` |
| User overrides ≥3 dead-end warnings in same repo | `LowerDeadEndThreshold { repo, new_threshold: 0.8 }` |
| Distiller emits same `StylePreference` ≥3 times across repos with low refutation | `PromoteToGlobal { fact, from_scope: repo, to: global }` |
| Same counterfactual retrieved ≥5 times, always ignored | `PromoteCounterfactualVisibility { fact_id, new_strategy: prepend_block }` |

All route through existing `MetaRule` table with status `Pending`. User sees in Mirror UI; can approve/reject/snooze.

### Alert severity and kind — closed enums

```rust
pub enum MirrorAlertSeverity { Low, Medium, High, Critical }

pub enum MirrorAlertKind {
    // Routing / skill alerts
    ProjectSkillObsolete,        // skill activation dropped >50% in 7 days
    UncapturedPattern,           // problem_hash recurred >=3x without matching WorkflowPattern
    ScopeMisclassified,          // global fact retrieved heavily in repo sessions, irrelevant
    SkillFileConflict,           // user edited managed block; auto-rewrite skipped
    // Pattern / learning alerts
    ProblemClassRefactor,        // same problem_hash fails >=3x across sessions
    LowerDeadEndThreshold,       // >=3 dead-end overrides in same repo
    PromoteToGlobal,             // preference observed across >=3 repos, low refutation
    PromoteCounterfactualVisibility, // counterfactual retrieved >=5x, ignored
    // Integrity alerts
    StaleFactDetected,           // C2 git invalidation produced a stale_candidate
    ProvenanceMissing,           // safety-net: fact somehow lacks valid provenance (should panic)
    DistillerQueueBacklog,       // >50 pending distillations — LLM provider probably down
}
```

### Alert surfacing

1. klyntbot desktop notification center — existing `MirrorState.pending_snippets` rendering.
2. SessionStart context injection (opt-in, default on for `severity: High`): high-severity alerts render into the §8 Recall-API markdown block.
3. Mirror MCP tool — coding-specific action filters: `mirror get_coding_alerts(repo?, severity?, kind?)`.
4. Workbench **Mirror Alerts Feed** panel (§11.5) — full alert browser with approve/dismiss/snooze actions.

### Pattern effectiveness feedback loop (the learning signal)

Per pattern/skill, `PatternEffectivenessSubscriber` executes:
1. `PatternApplied { pattern_id: X, session_id: S }` arrives.
2. Wait for same session's `SessionEnd` or next `UserCorrectedAI` targeting pattern output.
3. Compute outcome:
   - `UserCorrectedAI { CorrectionKind::MemoryMiss }` or negative sentiment → `failure`
   - `TestRun { passed: true }` following pattern application → `success`
   - No signal within 2h → `inconclusive`
4. Update `effectiveness_score` via EMA (success=1.0, partial=0.5, failure=0.0).
5. Publish `PatternOutcome` for Reforge nightly consumption.

---

## 11. Phased buildout

### Guiding principles

| Principle | Why |
|---|---|
| Phases ≠ versions | No partial v1 that lives with half the features. Phases are dev milestones; versions are release labels applied to final states. |
| Architecture skeleton first | Phase 1 defines every trait, type, schema, MCP tool stub. Phases 2+ only implement. |
| Production-quality every phase | Zero clippy warnings, full doc comments, property tests for invariants. No `// TODO` in shipped paths. |
| All schema consolidated into Phase 1 | Pre-release authorizes direct schema changes. No mid-project migrations. |
| Provenance always | Every write from Phase 3 onward includes full provenance. No exceptions. |
| Single source of truth for types | `AgentEvent`, `EventKind`, `CodingKind`, `CausalEdgeKind` defined once, imported everywhere. |

### The 8 phases

**Phase 1 — Architecture skeleton (1 week)**

*One big PR establishing the final shape. No runtime behavior.*

- `coding-memory` crate: all public types, all traits. Methods return `unimplemented!()` with explicit panic messages.
- `coding-ingest` crate: `AgentEvent`, `IngestSocket` trait, `IngestAdapter` trait, module stubs for all 4 CLI adapters.
- Consolidated schema migration — every column and table from §4.
- All 5 new `DomainEvent` variants added.
- All MCP tool stubs registered — return typed `NotImplementedInPhase { required_phase }` error.
- `klyntbot-hook` binary with arg parsing for all 4 CLIs; writes to stderr only.
- Architecture diagram + decision records in `docs/coding-memory/`.

**Exit gates:** workspace builds clean; zero clippy warnings; fmt passes; all public items documented.

**Phase 2 — Ingestion transport + Claude Code end-to-end (1 week)**

- Unix socket + file-buffer fallback implementations.
- Three-tier warning in klyntbot-hook.
- Desktop-owned daemon lifecycle.
- Full Claude Code adapter (7 hook events → `AgentEvent`).
- Desktop UI settings page with Claude Code toggle writing `~/.claude/settings.json`.
- Repo detection via `git rev-parse`.
- Integration tests: round-trip + desktop-off recovery.

**Exit gates:** synthetic Claude Code session round-trips; desktop-off recovery test passes; zero regressions on personal-AI.

**Phase 3 — Write path (Distiller + Tier A/B activation) (2 weeks)**

- Distiller Phase A (extractive, zero LLM).
- Distiller Phase B (LLM via `ProviderManager`).
- Mem0-style reconciliation (ADD/SUPERSEDE/NOOP).
- All 5 coding fact kinds with provenance-always.
- Counterfactual memory (B1).
- `code_state` on `UserSituationSnapshot` (B3).
- `CodeDomainSearcher` registered in InsightForge (B4).
- Autotuner session-type tag (B5).
- Tier A activation audit — synthetic tests prove `score_turn()`, `ContradictionDetected`, `UserCorrectedAI` fire for coding.

**Exit gates:** property test proves provenance-always; property test proves bi-temporal invariants; cost < $0.01/test-session.

**Phase 4 — Read path (recall API) (1 week)**

- All MCP recall tools functional.
- SessionStart passive injection (800 tok budget).
- UserPromptSubmit passive injection (1500 tok budget).
- Failure-state-aware retrieval (C3).
- Dead-end warning block.
- Unified `CodingRecallService`.

**Exit gates:** synthetic scenario — agent sees prior memory on next session; dead-end warning triggers on repeat attempt; C3 escalation measurable.

**Phase 5 — Reflection (Reforge + Mirror) (2 weeks)**

- Session-end light Reforge pass.
- Nightly Reforge Phase 2.5 (Coding Synthesis) + Phase 3.5 (Rule Artifact Generation).
- Managed-block writers for CLAUDE.md / AGENTS.md / .cursorrules.
- Scope-aware `SkillStore`.
- Project-scoped evolving skills (Phase 3.5 sub-phase).
- Mirror `RoutingMirrorSubscriber` + `MetaRuleDetector` coding extensions.
- `PatternEffectivenessSubscriber` (real-time).
- `StaleMemorySubscriber`.
- Selective-delete signal.

**Exit gates:** nightly cycle clean on seeded data; CLAUDE.md has ≥5 useful project-specific statements; project skill `effectiveness_score` drift demonstrable.

**Phase 6 — Game-changers wired live (2 weeks)**

*Schema already exists from Phase 1; this phase only adds behavior.*

- C1 causal edge auto-detection (test pass→fail, FixAttempt↔TestRun correlation).
- C2 tree-sitter symbol extraction at Distiller time.
- C2 git post-commit hook via desktop UI install.
- C2 Reforge deep symbol validation pass.
- `trace_causes` becomes non-stub.
- `anchored_symbols` populated on every write.
- ≥3-causal-chain promotion to `ProblemSolutionPattern`.

**Exit gates:** commit deleting a function invalidates anchored facts within 1 minute; `trace_causes` returns meaningful chain on seeded data.

**Phase 7 — Multi-CLI (2 weeks)**

*Remaining adapters against already-stable traits.*

- Codex adapter (5 hook events).
- kimi-cli adapter (13 events + Wire client tier 2 path).
- opencode SQLite polling adapter.
- Desktop UI toggles for all 4 CLIs.
- Cross-CLI event normalization tests proving `AgentEvent` identity.

**Exit gates:** full per-CLI test matrix green; no adapter required modifying any Phase-1 trait.

**Phase 8 — Hardening + benchmarks (2 weeks)**

- Benchmark suite (LongMemEval-like scenarios).
- Stress tests (10K events/session; multi-week Reforge; concurrent CLIs).
- Performance audit (retrieval p95 <200ms; Distiller p99 <30s).
- Security audit (provenance integrity, scope isolation, memory privacy).
- User guide + developer guide + contributor guide.

**Exit gates:** benchmark results committed; all invariants proved via proptest; production-readiness review passes.

### Quality gates (enforced at every phase)

| Gate | Check |
|---|---|
| Compilation | `cargo build --workspace` |
| Lint | `cargo clippy --workspace --all-targets --all-features` — zero warnings |
| Format | `cargo fmt --all --check` |
| Tests | `cargo nextest run --workspace` |
| Doc coverage | `cargo rustdoc -- -D missing-docs` on new public items |
| Provenance invariant | proptest |
| Bi-temporal invariant | proptest |
| Scope invariant | proptest |
| No TODO regression | CI grep in shipped paths |
| No panic paths | clippy `unwrap_used` / `expect_used` deny |
| Workbench panel live | Every phase that lands a new data shape ships the matching workbench panel (see §11.5) |

---

## 11.5. Coding Memory Workbench (desktop UI)

### Purpose

A user-facing section of the klyntbot desktop app that gives visibility into coding-agent activity, accumulated memory, learning signals, and Reforge-driven changes. Helps users **trust, audit, and tune** the system. Precedent: moonshot ships a similar surface for kimi-cli — this is product UX, not ops infra.

### Scope boundary

**Is:** a window into data already persisted in klyntbot's stores. All reads, no writes-as-side-effect.

**Is not:** ops observability. No Prometheus / OpenTelemetry exporters, no cross-machine aggregation, no admin/SRE audience, no external network endpoints. CLAUDE.md non-goal on observability is respected — this is product surface, not infrastructure.

### Technology stack

Extends existing `desktop-ui/` per CLAUDE.md conventions:
- Tauri 2 + Vite + React + Tailwind v4 + Biome 2.0 + React Compiler
- `useQuery(cmd, args)` for reads; `useMutation(cmd)` for alert actions
- All IPC through `ipc()` wrapper; never direct `invoke`
- `glass-panel` class for popovers/dialogs
- Path aliases: `@features/coding-memory/*` → `desktop-ui/src/features/coding-memory/*`
- New Tauri commands in `crates/desktop/src/commands/coding_memory.rs` — thin adapters over `app-core`; all registered in `DEV_COMMANDS` per CLAUDE.md

New top-level route: `/coding-memory` with nested routes per panel.

### Panels

| Panel | Data source | Purpose |
|---|---|---|
| **Session Replay** | `ingest_event_log` ordered by `(session_id, occurred_at)` | Scrubbable timeline of every `AgentEvent`: tool calls, file edits, test runs, assistant messages. Per-turn cost breakdown. Click an event → show raw JSON + distillation outcome |
| **Memory Browser** | `semantic_facts` + `episodic_memories` | Search + filter by repo, kind, date range, `memory_type`, `sensitivity`. Click a memory → full `metadata.provenance` chain with source event drill-down |
| **Activity Timeline** | `episodic_memories` by `occurred_at` | Calendar heatmap of activity density per day; per-repo filter; click a day → day's episodes |
| **Causal Graph Viewer** | `memory_causal_edges` + anchor memories | Force-directed graph: fix→broke→fixed-by chains; cluster by `problem_hash`; color by `edge_kind` |
| **Mirror Alerts Feed** | `mirror_snippets` | Pending / dismissed alerts grouped by severity + kind. One-click approve / reject / snooze actions. Shows meta-rule proposals with before/after config diffs |
| **Pattern Effectiveness Trends** | `workflow_patterns` + `skill_versions` effectiveness over time | Line charts of per-skill `effectiveness_score`; auto-highlight skills approaching decay or promotion thresholds |
| **Reforge Cycle Diff** | `skill_versions` + rule-artifact history | Side-by-side diff of previous vs. current `CLAUDE.md`, AGENTS.md, project skills. Per-cycle rollup of what changed |
| **Cost Tracker** | Distiller + Reforge telemetry | LLM spend per day / week / repo / provider. Breakdown: Distiller Phase B vs. Reforge Phase 2.5 vs. Phase 3.5. Alert when over user-configured ceiling |
| **Stale Candidates** | facts with `metadata.status = 'stale_candidate'` | Facts flagged by C2 git invalidation awaiting review. One-click invalidate-now / keep / review later |
| **CLI Health** | ingest socket state + toggle status + per-CLI event volumes | Per-CLI row: enabled/disabled, last event received, buffered event count, daemon liveness. Red-flag surface when desktop is off during user sessions |
| **Sensitivity Inspector** | `semantic_facts.metadata.sensitivity` | Browse memories by sensitivity tier; demote/promote with explicit confirmation; review `excluded` memories that would otherwise be hidden |

### Per-phase panel delivery

Each phase that lands new data shape also ships its matching panel. No panel waits in a pile for Phase 8.

| Phase | Panels added | Rationale |
|---|---|---|
| 2 | CLI Health (basic — Claude Code only). Session Replay (raw AgentEvent stream) | From the moment ingestion works, the user can see it working |
| 3 | Memory Browser (semantic + episodic with provenance). Activity Timeline. Cost Tracker. Sensitivity Inspector | Every fact that lands is immediately browsable |
| 4 | Session Replay gains recall-injection overlay + recall-tool invocation log | Shows the "why does the agent know X" chain |
| 5 | Mirror Alerts Feed. Pattern Effectiveness Trends. Reforge Cycle Diff | Reflection subsystems are invisible without these panels |
| 6 | Causal Graph Viewer. Stale Candidates panel | Game-changer tier becomes visible |
| 7 | CLI Health gains multi-CLI rows + per-CLI ingest stats | Multi-CLI state is confusing without this |
| 8 | Polish: accessibility audit; performance audit for large data (>100k facts); keyboard navigation; dark-mode-only → light-mode parity | Workbench graduates to production-quality |

### Data access pattern

All panels use `useQuery("coding_memory.<panel_name>", args)`. No panel ever reads the filesystem directly; Tauri commands are the only path. This keeps the web dev server (`bun run dev`) functional in browser-only mode (via the existing `dev_server/` HTTP bridge per CLAUDE.md).

### Performance bounds

- Session Replay pagination: 500 events per page with lazy-load scroll.
- Memory Browser: SQL-level pagination via `LIMIT / OFFSET`; FTS5 search for text queries.
- Causal Graph Viewer: max 200 nodes rendered; larger clusters auto-collapse to community summary.
- Cost Tracker: pre-aggregated rollups in a materialized view refreshed on Reforge cycle.

### Non-goals (workbench-scoped)

- No external HTTP endpoints (not a local web server; all access through Tauri IPC).
- No cross-machine data aggregation.
- No admin/team/multi-user views (matches spec-level non-goal).
- No log forwarding / SIEM integration.
- Data never leaves the device; no "share this session" button.
- No ML / ranking tuning UI (that's the autotuner's job, not the user's).

### Accessibility + UX conventions

Per CLAUDE.md desktop-ui rules:
- All interactive elements keyboard-navigable.
- Tokens from `src/styles/theme.css` only — no hardcoded hex.
- Glassmorphism via `glass-panel` class for overlays.
- No `overflow-x-auto` on containers with absolute-positioned dropdown children (CSS gotcha from CLAUDE.md).
- Vitest coverage for panel logic; Biome 2.0 lint.

---

## 12. Testing, error handling, quality

### Philosophy

- Every phase ships its own test suite; phase complete only when all green.
- In-memory SQLite for all tests (`StoragePool::connect_in_memory()`).
- Test pyramid: unit → integration → property → scenario → benchmark.
- Fixtures version-controlled in `tests/fixtures/coding/`.
- Property tests for every architectural invariant (9 total).

### Per-phase test deliverables

| Phase | Tests added | Rough count |
|---|---|---|
| 1 | Compile + doc tests | ~50 doc tests |
| 2 | Adapter unit tests, hook→socket→daemon integration, desktop-off recovery | ~20 unit + 5 integration + 1 scenario |
| 3 | Distiller unit, reconciliation, full-cycle integration, 5 property tests | ~30 unit + 8 integration + 5 property + 2 scenario |
| 4 | Recall MCP tool units, injection integration, C3 escalation, cross-session scenario | ~25 unit + 6 integration + 2 scenario |
| 5 | Session-end unit, nightly integration, PatternEffectiveness/StaleMemory subscriber tests, project-skill evolution scenario | ~35 unit + 10 integration + 3 scenario |
| 6 | Causal edge heuristics, tree-sitter extraction, git hook round-trip, 2 new properties, causal-chain promotion scenario | ~20 unit + 5 integration + 2 property + 1 scenario |
| 7 | Each CLI adapter, cross-CLI event identity | ~30 unit + 4 integration |
| 8 | 4 criterion benchmarks + 3 stress + 3 security properties | — |

Total: ~300 new tests.

### The 9 architectural invariants (all proptests)

| # | Invariant |
|---|---|
| 1 | ∀ fact: `metadata.provenance.source_events` is non-empty |
| 2 | ∀ fact: `valid_until.map_or(true, \|end\| end >= valid_from)` |
| 3 | If `fact_b.supersedes = fact_a.id`, then `fact_a.valid_until == fact_b.valid_from` |
| 4 | Retrieval with `scope_repo_id = X` never returns rows with `scope_repo_id = Y ≠ X` except NULL |
| 5 | After any Distiller cycle, `count(facts) + count(episodic)` is non-decreasing |
| 6 | After any Reforge cycle, all prior `episodic_memories` rows still exist |
| 7 | `parse(stdin_json(serialize(AgentEvent))) == AgentEvent` for all 4 CLI formats |
| 8 | Every `memory_causal_edges` row references existing `from_id` and `to_id` |
| 9 | SessionStart injection ≤ 800 tokens; UserPromptSubmit ≤ 1500 tokens |

### Error handling matrix

| Subsystem | Failure | Behavior | Rationale |
|---|---|---|---|
| Hook binary | Socket unreachable | File buffer fallback + stderr warning | Never block the CLI |
| Hook binary | File buffer write fails | Exit non-zero, loud stderr | Impossible-state; user must see |
| Daemon | DB locked | Retry 3× + backoff; dead-letter to `ingest_event_log_failed` | Soft degrade; no data loss |
| Distiller | LLM provider timeout | Phase A writes complete; Phase B retries 1m/5m/30m; fact `distillation: pending` | No LLM ≠ no memory |
| Distiller | LLM malformed tool call | Log + drop observation; Phase A preserved | Bad output never reaches store |
| Distiller | Provenance unconstructable | **Panic in dev; log+reject in release** | Write-side integrity is sacred |
| Reforge | Phase 2.5/3.5 LLM failure | Log to `mirror_snippets`; subsequent phases continue | Existing isolation pattern |
| Reforge | Managed-block conflict | Skip write; log `SkillFileConflict` | User content never overwritten |
| Retrieval | Escalation fails | Tier 1 results + `escalation_failed: true` | Graceful partial |
| Retrieval | Query timeout (>5s) | Partial results + marker | Never block agent |
| Git hook | Tree-sitter parse error | Skip symbol extraction; log degraded mode | One bad file doesn't break commit |
| Git hook | Desktop daemon down | Queue in `pending_invalidations`; apply on next startup | Never block user commit |

### Meta-rule: fail closed at write, degrade gracefully at read

Write integrity is sacred — a write with invalid provenance poisons the store forever. Reads can be elastic — empty results + clear error marker are recoverable next turn.

### Fixtures

Under `tests/fixtures/coding/`:
- `synthetic_session_claude_code.jsonl`, `synthetic_session_codex.jsonl`, `synthetic_session_kimi.jsonl` — 10-turn bug-fix session in each CLI's hook format.
- `repo_evolution.git.tar` — small test repo with known commit history (git invalidation tests).
- `distillation_mocks/*.json` — canned LLM responses.
- `reforge_seeds/*.sql` — seeded memory states.
- `longmem_eval_subset.jsonl` — coding-relevant LongMemEval subset for Phase 8 benchmarking.

### Benchmark targets (Phase 8, `criterion`)

| Benchmark | Target |
|---|---|
| Hook round-trip (stdin → socket → daemon ack) | p99 < 5ms |
| Distiller Phase A (extractive only) | p99 < 50ms |
| Distiller Phase B (with LLM call) | p99 < 30s |
| Unified retrieval | p95 < 200ms (50K facts, 10K episodes) |
| Nightly Reforge cycle | < 10 min (100K facts, 30d history) |
| Tree-sitter symbol extraction | < 200ms / 1KB Rust file |

Scenario benchmarks:
- Bug recurrence: seed 3 FixAttempts same `problem_hash`; verify 4th session surfaces warning < 200ms.
- Stale fact invalidation: seed fact anchored to symbol; commit deletes; time-to-invalidate < 1 min.
- LongMemEval-style: recall precision target ≥ 88.46% on knowledge-update category.

### Continuous integration

- On every PR: compile + lint + fmt + all phases' unit tests.
- On merge to main: integration + scenario tests.
- Nightly: property tests (longer fuzz duration) + benchmark deltas vs. baseline.
- Phase-completion PR: all tests from that phase + regression check against all prior phase suites.

### "Done" definition

After Phase 8:
- 300+ tests green.
- All 9 invariants proved via proptest.
- Benchmarks committed, all targets met.
- Zero clippy warnings.
- Full documentation.
- Security audit passed.
- User installs klyntbot desktop, toggles coding CLI integration, and within 5 minutes accumulates coding memory that evolves across sessions, survives commits, generates useful CLAUDE.md, and surfaces pattern alerts.

---

## 13. Appendix

### A. DomainEvent variants added

- `PatternApplied { pattern_id, session_id, repo, source }`
- `PatternOutcome { pattern_id, outcome, evidence, measured_at }`
- `FixAttemptFailed { problem_hash, repo, attempt_count }`
- `MemoryRetrieved { memory_ids, query, session_id, turn_id }`
- `AssistantMsgCompleted { session_id, turn_id, cited_memory_ids }`
- `RetrievalSkillApplied { skill, before_score, after_score, budget_used, session_id }` (§8 C3 formalization)

### B. MCP tool catalog (added to `default_exposed_tools()`)

| Tool | Purpose | Phase |
|---|---|---|
| `recall_index` | Compact index of relevant memories | 4 |
| `recall_timeline` | Chronological framing | 4 |
| `recall_fetch` | Full content + provenance | 4 |
| `trace_causes` | Walk causal graph | 6 |
| `check_dead_ends` | Counterfactual check | 4 |
| `recall_facts_as_of` | Point-in-time fact query | 4 (re-scoped TemporalTool) |
| `recall_change_history` | Decision history | 4 |
| `recall_decision_points` | Facts changed ≥2 times | 4 |

### C. Provider roles

- `ProviderRole::Distiller` — used by per-turn Distiller (defaults to user's small-tier model).
- `ProviderRole::ReforgeSynth` — used by Reforge Phase 2.5 (can be a larger model).
- `ProviderRole::ReforgeRules` — used by Reforge Phase 3.5.

All configurable in `config.json` → `codingMemory.distiller.model`, etc.

### D. Config surface (additions to existing config.json)

```json
{
  "codingMemory": {
    "enabled": true,
    "distiller": {
      "model": "claude-haiku-4-5-20251001",
      "maxInputTokens": 8000,
      "timeout": "30s"
    },
    "ingest": {
      "excludePaths": [
        "**/.env", "**/.env.*",
        "**/secrets/**", "**/private/**",
        "**/*.key", "**/*.pem", "**/*.p12", "**/*.pfx",
        "**/id_rsa", "**/id_ed25519", "**/known_hosts",
        "**/.aws/credentials", "**/.gcloud/**", "**/.kube/config",
        "**/node_modules/**", "**/target/**", "**/.git/**"
      ]
    },
    "privacy": {
      "defaultSensitivity": "normal",
      "autoPromoteHighPaths": ["**/auth/**", "**/billing/**", "**/payment/**"]
    },
    "recall": {
      "sessionStartBudget": 800,
      "userPromptBudget": 1500,
      "deadEndWarnings": true,
      "escalationEnabled": true,
      "coverageThreshold": 0.25
    },
    "reforge": {
      "nightlyCron": "0 3 * * *",
      "ruleArtifacts": {
        "claudeMd": true,
        "agentsMd": true,
        "cursorrules": true,
        "continueRules": true
      }
    },
    "skills": {
      "projectSkills": true,
      "location": "private"
    },
    "workbench": {
      "enabled": true,
      "sessionReplayPageSize": 500,
      "causalGraphMaxNodes": 200
    },
    "cli": {
      "claudeCode": { "enabled": false },
      "codex": { "enabled": false },
      "kimiCli": { "enabled": false },
      "opencode": { "enabled": false }
    }
  }
}
```

All flags default to off; user enables per-CLI via desktop UI.

### E. Key file paths

- Socket: `~/.klyntbot/ingest.sock`
- Buffer: `~/.klyntbot/ingest-buffer.jsonl` (rotated at 50MB, 500MB hard cap, 7-day TTL)
- Project skills (private): `~/.klyntbot/project-skills/<sanitized-repo-id>/<skill>/SKILL.md`
- Project skills (repo): `<repo_root>/.klyntbot/skills/<skill>/SKILL.md`
- Rule artifacts: `<repo_root>/CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.continue/rules/klyntbot.md`
- Fixtures: `tests/fixtures/coding/`
- Docs: `docs/coding-memory/`

### F. Research foundations referenced during brainstorming

- claude-mem architecture (LLM-as-distiller pattern; hooks-based passive observation)
- MAGMA multi-graph memory (arxiv 2601.03236) — causal edges
- Zep / Graphiti (arxiv 2501.13956) — bi-temporal triples
- TierMem (arxiv 2602.17913) — provenance pointers
- Supermemory relational versioning — 88.46% on LongMemEval knowledge-update
- Mem0 ADD/UPDATE/DELETE/NOOP reconciliation (we use ADD/SUPERSEDE/NOOP only)
- Skill-RAG (arxiv 2604.15771) — failure-state-aware retrieval
- Hindsight (arxiv 2512.12818) — epistemic separation (facts vs. opinions)
- empirical study arxiv 2505.16067 — selective add + selective delete = +10pp

### G. Out of scope / Future integrations

Explicit non-goals for this spec, with rationale — so future readers understand what was deliberately deferred vs. accidentally omitted.

| Topic | Status | Rationale |
|---|---|---|
| Cloud sync / team sharing | Out of scope | Spec is local-first single-user. `actor_id` column added in Phase 1 as forward-compat so future multi-user design doesn't need migration. |
| Full multi-user visibility model (private/team/public tiers) | Out of scope | Product design decision deferred until local-first is shipped and validated. Schema-level `actor_id` is the only hook. |
| Ops observability (Prometheus / OpenTelemetry / admin dashboards) | Out of scope | CLAUDE.md non-goal: "single-user local app. Existing tracing logs and PipelineEvent SSE stream are sufficient." User-facing Workbench (§11.5) is a product feature, not ops infra. |
| In-app encryption at rest | Out of scope | Wrong layer. OS-level full-disk encryption (FileVault on macOS, LUKS on Linux) is the correct defense. In-app encryption for a local SQLite is security theater. |
| Claude-mem ecosystem bridge | Deferred | Interesting adjacent project — MCP tool translation layer that lets claude-mem users point at klyntbot as backend. Future design will be tracked separately under `docs/coding-memory/future-integrations.md`. |
| Content-scanning secret detector | Rejected | Path-based `excludePaths` is coarse but reliable. Full content scanning for secrets is false-positive-heavy and impossible to maintain; stricter users set their editor to not open secret files. |
| Per-fact manual merge UI (full "conflict resolution" flows) | Out of scope | Workbench provides promote/demote sensitivity and approve/reject alerts; deeper manual memory editing creates integrity risks not worth the UX trade. |
| Log forwarding / SIEM integration | Out of scope | Data never leaves the device. Enterprises with SIEM requirements should reach klyntbot through the MCP server's audit-tool surface, not bulk log export. |
| `klynt-cli` native coding CLI | Separate future project | This spec makes klynt-cli's future integration cheap — it will embed `coding-memory` as a Rust library + implement an `IngestAdapter`. The CLI itself is a separate brainstorm → spec → plan cycle. |

### H. Amendment log

| Date | Change |
|---|---|
| 2026-04-22 | Initial design committed (commit 59277459e) |
| 2026-04-22 | Amendment 1: `actor_id` forward-compat, path-based `excludePaths`, `sensitivity` tagging, formalized retrieval-skill registry, closed Mirror alert enums, §11.5 Coding Memory Workbench, per-phase panel delivery, Out-of-scope appendix |

---

*End of design.*
