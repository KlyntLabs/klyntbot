# Klyntbot Todo Enhancement Roadmap

**Status**: In Progress (Sprint 5 Complete)
**Version**: 2.0
**Last Updated**: 2026-02-16
**Current Score**: 110/100 (before normalization)
**Target Score**: 99/100 (after normalization)

---

## Executive Summary

Klyntbot has achieved production-grade AI-powered task management with semantic search, smart enrichment, and calendar sync. Current focus: Daily planning automation and multi-device sync.

**Completed (Sprints 1-5):**
- ✅ CLI parity (all 17 todo + 6 project actions)
- ✅ Recurring tasks & dependencies (RRULE, blocked_by/blocks)
- ✅ Bidirectional calendar sync (events ↔ todos)
- ✅ Smart enrichment (auto-infer priority, duration, due date)
- ✅ **Semantic search (local embeddings, 50+ languages) ← Just Shipped!**

**Remaining (Sprints 6-7):**
- Sprint 6: Daily planning skill (proactive AI)
- Sprint 7: Git sync & multi-device (encrypted sync)
- Phase 4: Moonshot features (memory, auto-capture, habits, projects v2)

---

## Table of Contents

- [Completed Sprints (1-5)](#completed-sprints-1-5)
- [Remaining Work](#remaining-work)
  - [Sprint 6: Daily Planning Skill](#sprint-6-daily-planning-skill-week-11-12)
  - [Sprint 7: Git Sync & Multi-Device](#sprint-7-git-sync--multi-device-week-13-14)
- [Phase 4: Moonshot Features](#phase-4-moonshot-features-future)
- [Appendix: Deferred Ideas](#appendix-deferred-ideas)

---

## Completed Sprints (1-5)

### ✅ Sprint 1: CLI Parity (Complete)

**Delivered:** All 17 todo actions + 6 project actions accessible via CLI
- Commands: tree, search, report, attach/detach, add-subtask, move, log-time, update
- Project commands: create, list, show, update, archive, tasks
- Natural language date parsing (--due tomorrow, --due "next Friday")

**Impact:** +11 points (usability)
**Effort:** 25 hours
**Commit:** `feat(cli): expose all 17 todo + 6 project actions to CLI`

---

### ✅ Sprint 2: Recurring Tasks & Dependencies (Complete)

**Delivered:** RRULE-based recurring tasks and task dependencies
- Recurring tasks: templates, automatic spawning, CLI commands
- Dependencies: blocked_by/blocks fields, cycle detection, completion validation
- CLI: `todo recur add/list`, `todo depend --blocks`

**Impact:** +13 points (functionality)
**Effort:** 23 hours
**Commit:** `feat(todo): add recurring tasks (RRULE) and task dependencies`

---

### ✅ Sprint 3: Bidirectional Calendar Sync (Complete)

**Delivered:** Calendar changes automatically sync back to todos
- Reconciliation engine (runs every 5 minutes)
- Sync logic: time changes, completions, cancellations
- CLI: `calendar reconcile`, config: `calendar.bidirectional_sync`
- Notifications on sync

**Impact:** +4 points (integration)
**Effort:** 13.5 hours
**Commit:** `feat(calendar): bidirectional sync — reconcile events back to todos`

---

### ✅ Sprint 4: Smart Enrichment Engine (Complete)

**Delivered:** Auto-infer priority, predict duration, suggest due dates
- Priority inference (keywords: urgent, critical, fix, feature)
- Duration prediction (15min → 120min based on keywords)
- Due date suggestions (urgent → today, important → this week)
- CLI: `todo enrich <id>`, config: `todo.enrichment.enabled`

**Impact:** +7 points (AI intelligence)
**Effort:** 18 hours
**Commit:** `feat(enrichment): smart enrichment engine — auto-infer priority, duration, and due date`

---

### ✅ Sprint 5: Semantic Search (Complete)

**Delivered:** Multilingual semantic search with local embeddings
- Semantic search finds synonyms ("login" → "authentication", "2FA" → "two-factor")
- Hybrid search merges keyword + semantic via RRF (Reciprocal Rank Fusion)
- CLI: `--semantic`, `--hybrid`, `--threshold`, `--limit` flags
- Auto-embedding on add/update (CLI + Agent paths)
- Configurable similarity threshold (default: 0.5)
- Backfill command: `klyntbot todo backfill-embeddings`
- Multilingual model: paraphrase-multilingual-MiniLM-L12-v2 (384 dims, 50+ languages)
- Separate storage: `embeddings.jsonl` (keeps todos.jsonl human-readable)
- 100% local (no API calls, ~420MB model cached)

**Testing:**
- 94 new tests (integration, unit, CLI, benchmarks)
- Performance: 9ms search on 22 tasks (budget: 500ms)
- All 9 acceptance criteria verified
- Manual QA: real user flow tested

**Impact:** +6 points (search quality)
**Effort:** 19.5 hours (team) + 3 hours (QA fixes)
**Commits:**
- `feat(search): semantic search with local embeddings (fastembed)`
- `chore: expand gitignore — exclude user data, temp files, IDE configs`

---

## Remaining Work

### Sprint 6: Daily Planning Skill (Week 11-12)

**Objective**: Every morning, agent auto-plans the day and asks for confirmation.

**Why**: Most powerful use of AI — proactive planning instead of reactive task execution.

#### Implementation

**Core Components:**

1. **Daily planning skill** (`skills/daily-planning/SKILL.md`)
   - Triggers: Cron at daily digest time
   - Analyzes: overdue tasks, priorities, calendar events
   - Suggests: top 3 focus tasks with reasoning

2. **Planning engine** (`crates/agent/src/daily_planning.rs`)
   - Scoring: `(urgency × priority) + (age × 0.1)`
   - Urgency: overdue=10, today=5, tomorrow=3, future=1
   - Returns: `DailyPlan` with suggested tasks + reasoning

3. **User interaction**
   - Notification: "Good morning! Here's your plan: 1. Task A (P5, overdue), 2. Task B..."
   - Commands: "yes" (auto-focus), "swap 1 and 2" (reorder), "skip 3" (partial), "defer all" (archive)

4. **CLI command**: `klyntbot todo plan`
   - Manually trigger planning (don't wait for cron)

**Config:**
```rust
#[serde(default = "default_true")]
pub daily_planning: bool,
```

#### Testing

- Unit test: Scoring algorithm (overdue > today > tomorrow)
- Integration test: Daily plan suggests top 3 tasks
- Integration test: User responses ("yes", "swap 1 and 2", "skip N")
- Integration test: Cron runs at digest time

#### Acceptance Criteria

- ✅ Cron job runs daily at digest time
- ✅ Notification shows suggested focus order with reasoning
- ✅ User can reply "yes", "swap X and Y", "skip N", "defer all"
- ✅ Agent auto-focuses tasks on confirmation
- ✅ `klyntbot todo plan` manually triggers planning
- ✅ Config option `todo.daily_planning: false` disables feature
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(agent): daily planning skill — proactive morning focus suggestions`

**Impact**: +5 points (110 → 115/100 AI proactivity)

**Effort**: 17.5 hours

---

### Sprint 7: Git Sync & Multi-Device (Week 13-14)

**Objective**: All data syncs across machines via encrypted Git. Works offline, syncs on reconnect.

**Why**: Local-first is great, but daily-driver tools need multi-device support (laptop, phone, desktop).

#### Implementation

**Core Components:**

1. **Sync engine** (`crates/sync/src/lib.rs` - new crate)
   - Encryption: `age` crate (modern, simple)
   - Repo: git@github.com:user/klyntbot-data.git
   - Format: *.jsonl → *.jsonl.enc

2. **Operations**
   - `init_sync()`: Clone repo, generate/import keys
   - `encrypt_and_commit()`: Encrypt all JSONL → git commit (debounced 30s)
   - `pull_and_decrypt()`: Git pull → decrypt → merge conflicts
   - `merge_jsonl()`: Merge by timestamp, dedupe, newest wins

3. **CLI commands**
   - `klyntbot sync init <repo_url>`: Setup sync
   - `klyntbot sync push/pull/status`: Manual sync operations

4. **Auto-sync**
   - On mutation: encrypt + commit (debounced 30s)
   - On startup: pull + decrypt + merge

**Config:**
```rust
pub struct SyncConfig {
    pub enabled: bool,
    pub repo_url: Option<String>,
    pub public_key: Option<String>,
    pub secret_key: Option<Secret<String>>,
    pub auto_sync: bool,
}
```

#### Testing

- Integration test: Encrypt → decrypt round-trip
- Integration test: Conflict resolution (concurrent edits)
- Integration test: Auto-sync commits after mutation
- Integration test: Pull on startup syncs state
- Security test: Encrypted files unreadable without key

#### Acceptance Criteria

- ✅ `klyntbot sync init <repo>` sets up sync
- ✅ Mutations auto-commit (debounced 30s)
- ✅ `klyntbot sync push/pull` manual sync
- ✅ Conflict resolution (newest wins)
- ✅ Age encryption (modern, audited)
- ✅ Works offline (queue commits)
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(sync): multi-device encrypted Git sync with age encryption`

**Impact**: +4 points (115 → 97/100 after normalization — daily-driver ready)

**Effort**: 21.5 hours

---

## Phase 4: Moonshot Features (Future)

**Goal**: Build the stuff that makes this the reference implementation.

**Impact**: +20 points (takes it from "best" to "magical")

**Timeline**: 2-3 months (pick based on user feedback after Sprints 6-7)

### 4.1: Memory-Augmented Task Retrieval

**Effort**: 1-2 weeks

**What**: Store conversation history in vector DB. When user says "that thing we talked about yesterday", retrieve relevant tasks + chat snippets.

**How**:
- Add `qdrant` or `chroma` for local vector DB
- Embed every assistant/user message
- On query, search both todos + chat history
- Return unified results

**Impact**: +5 points

---

### 4.2: Auto-Capture from Everywhere

**Effort**: 2-3 weeks

**What**: Browser extension, Telegram/Discord/Slack bots that forward to agent.

**How**:
- Browser extension: highlight text → "Add to klyntbot"
- Platform bots: forward message → parse → create todo
- Unified inbox: all sources → same agent loop

**Impact**: +6 points

---

### 4.3: Habit Tracking

**Effort**: 1 week

**What**: Track streaks, correlate with energy/productivity.

**How**:
- Add `habit` field to Todo (boolean, daily check-in)
- Track streak (days consecutive completion)
- Analyze correlation (high energy after deep work in morning)
- Show in daily digest

**Impact**: +3 points

---

### 4.4: Projects v2 (Kanban, Phases, Custom Fields)

**Effort**: 2 weeks

**What**: Projects become full-featured (like Notion databases).

**How**:
- Add `phases: Vec<Phase>` (e.g., "Backlog", "In Progress", "Review", "Done")
- Add `custom_fields: HashMap<String, Value>` (arbitrary metadata)
- Kanban view in CLI (columns for phases)
- Drag-drop in TUI (using `ratatui`)

**Impact**: +4 points

---

### 4.5: Dependency Graph Visualization

**Effort**: 1 week

**What**: Generate Graphviz/Mermaid diagram of task dependencies.

**How**:
- Traverse `blocked_by`/`blocks` fields
- Output `.dot` or Mermaid syntax
- Render with `graphviz` or show in browser

**Impact**: +2 points

---

## Appendix: Deferred Ideas

These are good ideas but deprioritized for now:

### A.1: Pomodoro Integration

**Why Deferred**: Niche feature, low ROI compared to other Phase 3 work.

**What**: `klyntbot todo focus --pomodoro` → 25/5 cycles with OS notifications.

**Effort**: 1-2 days

**Priority**: P3

---

### A.2: Smart Tag Suggestions

**Why Deferred**: Enrichment engine (Sprint 4) already covers most value.

**What**: Suggest tags based on co-occurrence patterns.

**Effort**: 1 day

**Priority**: P3

---

### A.3: Task Templates

**Why Deferred**: Less urgent than recurring tasks.

**What**: Store pre-defined hierarchies as JSONL templates.

**Effort**: 2 days

**Priority**: P3

---

### A.4: Context-Aware Notifications

**Why Deferred**: Requires OS location APIs, complex setup.

**What**: Suppress notifications when in meetings or away.

**Effort**: 3-4 days

**Priority**: P2

---

### A.5: Focus Session Analytics

**Why Deferred**: Nice-to-have, not critical.

**What**: Track flow state metrics (sessions/day, duration, productivity).

**Effort**: 2 days

**Priority**: P3

---

## Progress Summary

| Phase | Status | Effort | Impact | Deliverables |
|-------|--------|-------:|-------:|--------------|
| **Phase 1** | ✅ Complete | 62 hours | +31 pts | CLI parity, recurring tasks, dependencies, calendar sync |
| **Phase 2** | ⚠️ 2/3 Complete | 37.5/55 hours | +13/+25 pts | Smart enrichment ✓, semantic search ✓, **daily planning pending** |
| **Phase 3** | 🔜 Not Started | 21.5 hours | +4 pts | Git sync, multi-device support |
| **Phase 4** | 🔮 Future | TBD | +20 pts | Memory, auto-capture, habits, projects v2, graph viz |
| **Total** | 71% Complete | 99.5/138.5 hours | +44/+80 pts | 110/115 delivered (97/100 after normalization) |

---

## Next Steps

1. ✅ **Sprint 5 shipped** — semantic search with local embeddings
2. 🔜 **Sprint 6** — Daily planning skill (proactive AI)
3. 🔜 **Sprint 7** — Git sync & multi-device (encrypted)
4. 🔮 **Phase 4** — Moonshot features (post-MVP)

---

**Questions? Feedback?**

Open an issue or PR to discuss this roadmap.
