# Reforge Cycle — Unified Nightly Consolidation Architecture

## Summary

Replace 5 separate cron jobs (Weekly Reflection, Mirror Narrative, Autotuner Nightly, Compaction Daily, Mirror Cleanup) with a single nightly **Reforge Cycle** that improves Memory, Skills, and Parameters in one coordinated pass.

The Reforge Cycle runs nightly (adaptive — skips when no new data), collects session scratchpads + episodic memories + correction patterns + routing data, processes them through 3 focused LLM calls, and applies the results: fact updates, rule updates, skill file edits, narrative generation, autotuner evaluation, and compaction.

**Key innovations over current architecture:**
- **Nightly, not weekly** — faster feedback loop, gated by new data
- **Session-aware synthesis** — groups episodics by session, feeds session scratchpads
- **Skill auto-improvement** — edits skill files (frontmatter, body, references) based on correction patterns
- **Single coordinated cycle** — eliminates race conditions between 5 independent systems
- **Incremental processing** — processes since last run, not always 7-day window

---

## Architecture Overview

```
Nightly Reforge Cycle (__klyntbot_reforge_nightly)
Trigger: 3am daily (user timezone), UserIdle 300s, 4h tolerance

  Phase 1: COLLECT
    ├─ Session scratchpads since last Reforge
    ├─ Episodic memories since last Reforge
    ├─ Current user model (all active facts)
    ├─ Active procedural rules
    ├─ Routing snapshots since last Reforge
    ├─ Correction events + pending MetaRules
    ├─ Current skill file contents (from ~/.klyntbot/skills/)
    └─ Retrieval feedback metrics

  Phase 2: SYNTHESIZE (LLM call #1 — Knowledge)
    Input:  sessions + episodics + user model + rules
    Output: fact_updates, rule_updates, stale_facts, cross_session_patterns

  Phase 3: REVIEW (LLM call #2 — Skills & Behavior)
    Input:  corrections + MetaRules + routing data + skill contents + Phase 2 output
    Output: skill_edits[], routing_insights, context_priority_suggestions

  Phase 4: NARRATE (LLM call #3 — Summary)
    Input:  Phase 2 + Phase 3 outputs + routing snapshots
    Output: human-readable narrative for Brain page

  Phase 5: APPLY
    ├─ Consolidate facts (add/update/supersede)
    ├─ Apply rules (dedup, reinforce, upsert)
    ├─ Write skill edits to ~/.klyntbot/skills/ + version in DB
    ├─ Store narrative as episodic memory
    └─ Record Reforge run metadata

  Phase 6: OPTIMIZE
    ├─ Autotuner trial evaluation (metric-based, no LLM)
    ├─ Autotuner trial generation (existing LLM call)
    └─ Champion promotion if winner found

  Phase 7: COMPACT
    ├─ Archive superseded facts (90d)
    ├─ Delete old episodics (90d, low access)
    ├─ Deactivate stale rules (90d, <2 signals)
    ├─ Prune accumulated observations (7d)
    ├─ Delete failed observations (30d)
    ├─ Delete old session memories (90d)
    ├─ Co-activation decay (weekly on Sundays)
    ├─ Clean old routing snapshots + narrative snippets (90d)
    └─ Reindex poor embeddings (high importance, low retrieval score)
```

---

## Cron Registration

### New Job
```
Job:      __klyntbot_reforge_nightly
Schedule: "0 3 * * *" (3am daily, user's timezone)
Intent:   UserIdle 300s, 4h tolerance, WhenIdle catch-up
```

### Removed Jobs
| Old Job | Replaced By |
|---------|-------------|
| `__klyntbot_cognitive_weekly_reflection` (Mon 9am) | Reforge Phase 2+5 |
| `__klyntbot_mirror_weekly_narrative` (Sun 10am) | Reforge Phase 4 |
| `__klyntbot_autotuner_nightly` (nightly) | Reforge Phase 6 |
| `__klyntbot_cognitive_compaction_daily` (daily 3am) | Reforge Phase 7 |
| `__klyntbot_mirror_cleanup` (Sun 4am) | Reforge Phase 7 |

### Skip Gate
1. Load `reforge_state.last_run_at`
2. Count new episodic memories since last run
3. Count new session scratchpads since last run
4. If both zero → skip, log "Reforge skipped: no new data"
5. If first run (`last_run_at` is NULL) → bootstrap with 7-day window

---

## Data Collection (Phase 1) — Incremental

### Reforge State Table
```sql
CREATE TABLE reforge_state (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    last_run_at TEXT,
    last_run_stats TEXT,         -- JSON: { facts_added, rules_updated, skills_edited, ... }
    run_count INTEGER DEFAULT 0
);
```

### Collection Strategy
| Data | Query | Notes |
|------|-------|-------|
| Session scratchpads | `WHERE updated_at > last_run_at` | Grouped by session_key |
| Episodic memories | `WHERE occurred_at > last_run_at` | Grouped by nearest session |
| User model | All active facts (full) | Current state, not incremental |
| Procedural rules | All active (full) | Current state |
| Routing snapshots | `WHERE created_at > last_run_at` | Aggregated per skill |
| MetaRules | `WHERE status = 'pending'` | Pending correction proposals |
| Skill files | Full read from `~/.klyntbot/skills/` | Need complete content for edit context |
| Retrieval feedback | `avg_precision_since(last_run_at)` | Aggregate metric |

### First Run Bootstrap
When `last_run_at` is NULL, load the last 7 days of data. After first successful run, switch to incremental.

### Failure Recovery
If any phase fails, `last_run_at` is NOT updated. Next night retries with the accumulated data window.

---

## LLM Call #1 — Knowledge Synthesis (Phase 2)

### Purpose
Synthesize facts and rules from recent session activity, detect cross-session patterns and contradictions.

### Input
- Session scratchpads grouped by session (with metadata: duration, project, timestamps)
- Episodic memories since last Reforge
- Current user model (all active facts, grouped by domain)
- Active procedural rules
- Retrieval feedback metrics

### Prompt Structure
```
You are a knowledge consolidation engine for a personal AI assistant.
Analyze the user's recent sessions and memories against their existing knowledge base.

## Sessions Since Last Reforge

[Session A — 2026-04-08 9:00am, 45min, project: Klynt]
Scratchpad:
  Current Task: Implementing brain view overhaul
  Key Decisions: Used Louvain for community detection
  Progress: 7/8 tasks complete

Episodic memories:
  - "User prefers cognitive-first graph over notes-only"
  - "Co-activation edges reveal transitive fact relationships"

[Session B — 2026-04-08 2:15pm, 20min]
Scratchpad:
  Current Task: Debugging tiptap crashes
  Key Decisions: try/catch on all editor.view.dom access sites
  Progress: Fixed 7 crash sites

Episodic memories:
  - "Tiptap view.dom getter throws when editor not mounted"

## Current User Model
[productivity] User: prefers = morning deep work sessions [strong]
[preferences] User: uses = dark mode IDE [moderate]
...

## Active Rules
- "Always check calendar before scheduling deep work" (signal_count: 5)
- ...

## Retrieval Feedback
Average precision since last Reforge: 0.72 (based on 34 queries)

Produce JSON:
{
  "fact_updates": [{
    "action": "add" | "update" | "remove",
    "subject": "...",
    "predicate": "...",
    "object": "...",
    "domain": "...",
    "confidence": 0.0-1.0,
    "reason": "why this fact should change"
  }],
  "rule_updates": [{
    "action": "add" | "update" | "reinforce",
    "rule_text": "...",
    "domain": "...",
    "reason": "..."
  }],
  "stale_facts": [{
    "fact_id": "...",
    "reason": "Not accessed in 90 days, no reinforcing signals"
  }],
  "cross_session_patterns": [{
    "pattern": "User is more productive in morning sessions",
    "sessions_involved": ["Session A", "Session B"],
    "confidence": 0.85
  }],
  "extraction_quality_flag": null | "diagnosis string if low quality detected"
}
```

### Key Difference from Current Reflection
Sessions are grouped and labeled with metadata. The LLM can reason about per-session patterns and cross-session contradictions, not just a flat episodic list.

---

## LLM Call #2 — Skills & Behavior Review (Phase 3)

### Purpose
Propose skill edits and behavioral insights from correction patterns, routing data, and MetaRules.

### Input
- Pending MetaRules (correction-derived proposals)
- Routing snapshots (per-skill: message count, avg confidence, fallback rate)
- Current skill file contents (SKILL.md frontmatter + body, references/)
- Phase 2 output (new facts/rules feed forward)
- Retrieval feedback metrics

### Prompt Structure
```
You are a skill improvement engine for a personal AI assistant.
Analyze correction patterns and routing data to propose targeted skill edits.

## Pending MetaRules (correction patterns detected)
- "User corrected summarization behavior 3 times across 2 sessions"
- "Low-confidence routing to finance skill (avg 0.42, 12 messages)"

## Routing Summary (since last Reforge)
general: 45% of messages (avg confidence 0.78)
task-management: 30% (avg confidence 0.85)
finance-management: 15% (avg confidence 0.42) ← low
automation: 10% (avg confidence 0.71)

## Current Skills

### automation (SKILL.md)
---
name: automation
description: Reminders, cron jobs, and recurring automations
whenToUse: When the user mentions remind, schedule, every day, recurring, cron, or automate
---
[body content...]

### finance-management (SKILL.md)
---
name: finance-management
description: Personal finance tracking with multi-currency support
whenToUse: When the user mentions expenses, budget, accounts, transactions, spending, savings, or investments
---
[body content...]

## New Knowledge from Synthesis Phase
- Added fact: "User tracks investments in THB and USD"
- Added rule: "Show currency code alongside symbol"

## Context Priority Metrics
Session memory (priority 88): referenced in 60% of LLM responses
Semantic facts: referenced in 40% of LLM responses
Episodic memories: referenced in 15% of LLM responses

Produce JSON:
{
  "skill_edits": [{
    "skill_name": "finance-management",
    "file_path": "SKILL.md",
    "edit_type": "frontmatter",
    "field": "whenToUse",
    "new_value": "When the user mentions expenses, budget, accounts, transactions, spending, savings, investments, or portfolio",
    "reason": "8 messages about 'portfolio' routed to general with low confidence"
  }, {
    "skill_name": "automation",
    "file_path": "SKILL.md",
    "edit_type": "body_replace",
    "old_text": "exact text to replace",
    "new_text": "replacement text",
    "reason": "User corrected this behavior 3 times"
  }, {
    "skill_name": "task-management",
    "file_path": "references/daily-planner.md",
    "edit_type": "body_insert",
    "section": "## Morning Routine",
    "new_text": "- Check calendar conflicts before scheduling deep work",
    "reason": "Pattern from 5 sessions: user always checks calendar first"
  }],
  "routing_insights": [
    "Finance skill needs broader trigger coverage for investment-related terms"
  ],
  "context_priority_suggestions": [{
    "source": "episodic_memory",
    "current_priority": "low (15% reference rate)",
    "suggestion": "Consider reducing episodic memory injection limit from 5 to 3",
    "reason": "Low utilization suggests noise outweighs signal"
  }]
}
```

### Skill Edit Types
| Type | What It Edits | Fields |
|------|---------------|--------|
| `frontmatter` | YAML frontmatter field | `field`, `new_value` |
| `body_replace` | Replace text in body | `old_text`, `new_text` |
| `body_insert` | Insert text at section | `section`, `new_text` |
| `body_remove` | Remove text from body | `old_text` |
| `reference_edit` | Edit a reference file | `file_path`, same sub-types |

---

## LLM Call #3 — Narrative (Phase 4)

### Purpose
Generate a human-readable summary for the Brain page.

### Input
Phase 2 + Phase 3 outputs, routing snapshot summary.

### Prompt
```
Summarize tonight's Reforge cycle for the user. Be concise (2-3 paragraphs).
Include: what was learned, what changed, any notable patterns or recommendations.

Knowledge synthesis: {{ phase_2_output }}
Skill review: {{ phase_3_output }}
Routing: {{ routing_summary }}
```

### Output
Free-text narrative stored as episodic memory (domain="reforge", importance=0.9, stability=5.0).

---

## Skill Versioning & File Management

### Skill Lifecycle
1. **First run** — Seed `~/.klyntbot/skills/` from compiled defaults. DB records each file as version 1, source `[Seed]`.
2. **Reforge edits** — Phase 3 proposes edits. Phase 5 writes new file to disk + stores previous content and diff in DB as new version with source `[Reforge]`.
3. **User edits** — Filesystem watcher detects changes to `~/.klyntbot/skills/**`. On change, reads new content, diffs against last known version, stores new version with source `[User]`.
4. **Reset** — Brain page shows version timeline with diffs. "Reset to v3" writes v3 content to disk, creates new version with source `[User]` and reason "Reset to v3".

### Skill Versions Table
```sql
CREATE TABLE skill_versions (
    id TEXT PRIMARY KEY,
    skill_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    file_path TEXT NOT NULL,       -- relative: "SKILL.md", "references/cron.md"
    content TEXT NOT NULL,          -- full file content at this version
    diff TEXT,                      -- unified diff from previous version
    source TEXT NOT NULL,           -- 'Seed', 'Reforge', 'User'
    reason TEXT,                    -- why the change was made
    created_at TEXT NOT NULL
);
CREATE INDEX idx_skill_versions_name ON skill_versions(skill_name, version);
```

### Version Labels
```
v1  [Seed]    — Initial skill (compiled default)
v2  [Reforge] — Removed summarization step based on 3 correction signals
v3  [User]    — Jayden manually edited the routing section
v4  [Reforge] — Added project-scoping guideline from 5 session patterns
v5  [User]    — Jayden rewrote the opening instructions
```

### User Edit Detection
On startup, compute content hashes for all files in `~/.klyntbot/skills/`. On each Reforge Phase 1, re-hash and compare against DB's last known version. If mismatch detected → store new version with source `[User]` before proceeding. No filesystem watcher needed — the Reforge cycle itself detects changes.

### Conflict Resolution
If user edits a skill file between Reforge Phase 1 (collect) and Phase 5 (apply), the Reforge detects the content hash mismatch at apply time and **skips that skill's edit**. User version is preserved. Logged as "Skipped edit to {skill} — user modified since collection."

### Skill Edit Scope
The Reforge can edit any part of a skill directory:
- **Frontmatter fields**: `name`, `description`, `whenToUse`, `triggers`
- **Body content**: instructions, tables, flowcharts, red flags
- **Reference files**: `references/*.md` — add, edit, or update
- **Scripts/assets**: `scripts/*.md`, `assets/*.md`

---

## Enhancement Areas (Built Into Existing Phases)

### 1. Context Source Prioritization
**Phase:** 2 (input) + 5 (apply)

Retrieval feedback metrics show reference rates per source type. When a source is consistently unused, the Reforge suggests priority adjustments. Applied by updating `CognitiveRetrievalConfig`.

### 2. Extraction Prompt Evolution
**Phase:** 2 (analysis) + 4 (narrative)

The Synthesize call detects patterns like "7 facts superseded within 24h" or low average convergence. Surfaces as `extraction_quality_flag` in the narrative with diagnosis.

### 3. Routing Calibration
**Phase:** 3 (Skills & Behavior Review)

Routing improvements happen through skill edits: better `whenToUse`, new `triggers`, body changes to handle misrouted cases. No separate routing calibration system needed.

### 4. Embedding Reindexing
**Phase:** 7 (Compact)

During compaction, identify facts where `access_count > 5 AND last_retrieval_score < 0.3 AND importance > 0.6`. Flag for re-embedding via `embedder.embed_and_store_fact()`.

### 5. User Model Staleness
**Phase:** 2 (output)

The Synthesize call sees full user model with `last_accessed` timestamps. Facts not accessed in 60+ days with no reinforcing signals get flagged as `stale_facts`. Phase 5 reduces their confidence. Narrative mentions them.

---

## Error Handling

### Phase-Level Independence
Each phase runs independently. Failures don't cascade:

| Phase Fails | Impact |
|-------------|--------|
| Phase 1 (Collect) | Entire cycle aborts — no data to process |
| Phase 2 (Synthesize) | Skip fact/rule updates; Phase 3 still runs with corrections/routing only |
| Phase 3 (Review) | Skip skill edits; Phase 2 results still applied |
| Phase 4 (Narrate) | No narrative stored; everything else applies |
| Phase 5 (Apply) | Partial apply — each sub-step independent; `last_run_at` updated for succeeded items |
| Phase 6 (Optimize) | Autotuner skips this cycle; no impact on memory/skills |
| Phase 7 (Compact) | Cleanup deferred to next cycle |

### `last_run_at` Update
Updated after Phase 5 completes (even partially). This ensures failed data is not re-processed, while Phase 6/7 failures don't block the incremental window.

---

## Code Changes

### Removed
| Current | Action |
|---------|--------|
| `reflection.rs` (`run_weekly_reflection`) | **Delete** |
| `MirrorFacade::generate_weekly_narrative` | **Delete** |
| `AutoTunerOrchestrator::register_nightly_cycle` | **Refactor** — extract evaluation logic for Phase 6 |
| 5 cron job registrations in `cron.rs` | **Delete** |

### Kept (Unchanged)
| Current | Why |
|---------|-----|
| `background.rs` (per-event consolidation) | Continuous extraction pipeline unchanged |
| `session_memory.rs` (scratchpad generation) | Produces data Reforge consumes |
| Mirror subscribers (routing, MetaRule, ConfigArchiver, TrialPreview) | Detect patterns; Reforge consumes their output |
| `compaction.rs` (`run_compaction`) | Called by Phase 7 (add embedding reindex) |

### New
| Component | Location |
|-----------|----------|
| `ReforgeService` | `crates/cognitive/src/services/reforge.rs` |
| `ReforgeCollector` | `crates/cognitive/src/services/reforge/collector.rs` |
| `ReforgeHandler` trait | `crates/cognitive/src/services/reforge.rs` (3 methods: synthesize, review, narrate) |
| `LlmReforgeHandler` | `crates/agent/src/adapters/cognitive_handlers.rs` |
| `ReforgeStateRepo` | `crates/storage/src/repos/reforge_state.rs` |
| `SkillVersionRepo` | `crates/storage/src/repos/skill_version.rs` |
| `SkillFileManager` | `crates/cognitive/src/services/reforge/skill_files.rs` |
| Cron registration | `crates/app-core/src/init/cron.rs` — single `__klyntbot_reforge_nightly` |
| DB migrations | `skill_versions` + `reforge_state` tables |

### New Tables
```sql
CREATE TABLE skill_versions (
    id TEXT PRIMARY KEY,
    skill_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    content TEXT NOT NULL,
    diff TEXT,
    source TEXT NOT NULL,
    reason TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_skill_versions_name ON skill_versions(skill_name, version);

CREATE TABLE reforge_state (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    last_run_at TEXT,
    last_run_stats TEXT,
    run_count INTEGER DEFAULT 0
);
```

---

## Success Criteria

1. **Single nightly cycle** replaces 5 cron jobs — zero race conditions between systems
2. **Session-aware synthesis** — LLM receives grouped session scratchpads, not flat episodic list
3. **Skill auto-improvement** — Reforge edits skill files (frontmatter, body, references) with version history
4. **User always wins** — filesystem watcher detects user edits, Reforge skips conflicting changes
5. **Incremental processing** — only processes data since last successful run
6. **Adaptive frequency** — skips when no new data, runs nightly when active
7. **Brain page shows** — Reforge history, skill version diffs, reset capability
8. **Phase isolation** — individual phase failures don't cascade
9. **All existing tests pass** — background consolidation, autotuner metrics, mirror subscribers unchanged
10. **Narrative generated** — human-readable summary available on Brain page after each Reforge
