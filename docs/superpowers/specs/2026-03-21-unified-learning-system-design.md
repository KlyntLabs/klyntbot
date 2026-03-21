# Unified Learning System: Knowledge Atoms

**Date:** 2026-03-21
**Status:** Approved
**Scope:** Connect notes, flashcards, coaching, autotuner, and activity tracking into one unified learning system via a new first-class "Knowledge Atom" entity.

## Problem

Learning happens in silos. Vocabulary saves write to SemanticFact + Flashcard stores but emit no DomainEvents. Coaching watches focus sessions and tasks but has zero visibility into learning activity. Autotuner optimizes for speed/cost but never sees retention data. Activity Log ignores learning entirely. The result: the system remembers isolated words but not concepts, can't nudge about declining retention, and can't personalize responses based on what the user actually knows.

## Solution: Knowledge Atoms

A **Knowledge Atom** is the central node that represents a single piece of personal knowledge — a vocabulary word, a concept from a note, a learned skill, or a behavioral pattern. Every atom links to its source note, has FSRS-5 retention scheduling, emits DomainEvents, and is visible to coaching/autotuner/activity tracking.

## 1. Data Model

### knowledge_atoms table (new)

| Column | Type | Description |
|---|---|---|
| `id` | TEXT PK | UUID |
| `subject` | TEXT NOT NULL | What this atom represents ("PR review checklist", "验证") |
| `atom_type` | TEXT NOT NULL | "vocabulary" / "concept" / "skill" / "fact" |
| `domain` | TEXT NOT NULL | "language:zh", "software-engineering", "finance" |
| `source_note_id` | TEXT | FK to notes.id |
| `source_range` | TEXT | JSON: {startOffset, endOffset} in note body |
| `source_context` | TEXT | Snippet of original text for flashcard display |
| `secondary_sources` | TEXT | JSON array of {noteId, range} for cross-note reinforcement |
| `semantic_fact_id` | TEXT | FK to semantic_facts.id |
| `retention_pct` | REAL DEFAULT 1.0 | Current FSRS retrievability [0.0-1.0] |
| `stability` | REAL DEFAULT 1.0 | FSRS stability |
| `difficulty` | REAL DEFAULT 5.0 | FSRS difficulty |
| `personal_importance` | REAL DEFAULT 0.7 | User-set importance [0.0-1.0] |
| `status` | TEXT DEFAULT 'suggested' | "suggested" / "active" / "archived" |
| `salience` | REAL DEFAULT 1.0 | Decays without interaction (tiered multiplicative decay, NOT FSRS) |
| `last_interaction_ts` | TEXT | Any click/review/reference |
| `archived_at` | TEXT | When status changed to "archived" (drives 7-day restore window) |
| `metadata` | TEXT | JSON: vocab data (pinyin, examples), concept links, etc. |
| `topic_id` | TEXT | FK to knowledge_topics.id |
| `created_at` | TEXT NOT NULL | |
| `updated_at` | TEXT NOT NULL | |

### knowledge_topics table (new)

| Column | Type | Description |
|---|---|---|
| `id` | TEXT PK | UUID |
| `name` | TEXT NOT NULL | "PR Workflows", "Chinese Vocabulary" |
| `domain` | TEXT NOT NULL | |
| `atom_count` | INTEGER DEFAULT 0 | |
| `avg_retention` | REAL DEFAULT 1.0 | |
| `created_at` | TEXT NOT NULL | |

### Indexes

```sql
CREATE INDEX idx_atoms_note ON knowledge_atoms(source_note_id) WHERE status != 'archived';
CREATE INDEX idx_atoms_last_interaction ON knowledge_atoms(last_interaction_ts) WHERE status = 'active';
CREATE INDEX idx_atoms_topic ON knowledge_atoms(topic_id);
CREATE INDEX idx_atoms_status ON knowledge_atoms(status, salience);
```

### Existing table modifications

- `flashcards`: Add `atom_id TEXT REFERENCES knowledge_atoms(id)` — multiple cards per atom.
- No changes to `semantic_facts` — atoms reference them via `semantic_fact_id`.

### Schema location

All new tables and the `flashcards.atom_id` ALTER go into the consolidated `crates/cognitive/migrations/001_cognitive_tables.sql` (pre-release, no incremental migrations needed per CLAUDE.md). The `knowledge_atoms` CREATE TABLE must appear before the `flashcards` ALTER TABLE to satisfy the FK reference. Update the `FeatureMigration` version in `cognitive` crate.

### Key design decisions

- Atoms wrap SemanticFacts, they don't replace them. The triple store remains the lookup index; atoms add retention scheduling, anchoring, and lifecycle management.
- `source_range` anchors the atom to an exact position in the note. Flashcards show this snippet as context during review.
- **Two separate decay systems on each atom:**
  - `retention_pct` uses the **FSRS-5 formula** (`R = 1/(1 + t/(9*S))`) from `fsrs5.rs`. Updated on every flashcard review and recomputed daily by the decay cron. This is the user-facing "how well do I remember this?" metric.
  - `salience` uses **tiered multiplicative decay** (0.85/0.92/0.97 per day depending on importance). This is the system-internal "should I still show this?" metric. NOT the cognitive decay formula from `decay.rs`.
- `personal_importance` multiplies salience decay rate and prioritizes coaching nudges. Set on bulk-accept via importance picker. Mapping: low (0.3) / medium (0.7, default) / high (1.0). The `atom_accept` IPC command accepts `personal_importance: f64`.
- `secondary_sources` tracks cross-note reinforcement. When an atom is referenced in a new note, the system auto-links it and boosts salience.
- `metadata` is a flexible JSON field for type-specific data (vocab: pinyin/examples, concepts: related tasks, skills: coaching logs).
- `knowledge_topics.atom_count` and `avg_retention` are denormalized aggregates, updated by the nightly decay cron AND on atom accept/archive operations. Staleness of up to 24h between cron runs is acceptable for non-critical display.

## 2. DomainEvents + Bus Integration

### New events

All 11 variants are added to the existing `DomainEvent` enum in `crates/bus/src/domain_events.rs` (NOT a separate enum). This ensures all existing subscribers (CoachingService, ActivityLogSubscriber, BackgroundConsolidation) see them via their existing `broadcast::Receiver<DomainEvent>`. Existing `match` arms that use `_ => {}` catchalls will silently ignore unhandled variants; consumers that need to react get explicit match arms added.

```rust
// Added to DomainEvent enum in bus/src/domain_events.rs:
KnowledgeAtomCreated { atom_id: String, atom_type: String, domain: String, source_note_id: Option<String>, personal_importance: f64 },
KnowledgeAtomAccepted { atom_id: String, atom_type: String },
KnowledgeAtomArchived { atom_id: String, reason: String },
AtomFlashcardReviewed { atom_id: String, card_id: String, quality: u8, recall_speed_ms: u64, new_retention_pct: f64, source_note_id: Option<String> },
AtomReinforced { atom_id: String, referencing_note_id: String, new_salience: f64 },
AtomInteracted { atom_id: String, interaction_type: String, note_id: Option<String> },
RetentionMilestoneReached { atom_id: String, topic_id: Option<String>, new_retention_pct: f64, milestone: String, previous_pct: f64 },
TranslationCompleted { note_id: String, source_lang: String, target_lang: String, word_count: usize, is_selection: bool },
NoteStudied { note_id: String, duration_secs: u64, atoms_reviewed: usize, mode: String },
KnowledgeTransferDetected { atom_id: String, from_domain: String, to_domain: String, confidence: f64 },
CoachingLearningDigest { fading_count: usize, archived_count: usize, streak_days: usize, strongest_topic: Option<String>, weakest_topic: Option<String> },
```

### Emission points

| Handler | Events emitted |
|---|---|
| `language_save_vocabulary` | `KnowledgeAtomCreated` + `KnowledgeAtomAccepted` |
| `flashcard_record_review` (if card has atom_id) | `AtomFlashcardReviewed` + optional `RetentionMilestoneReached` |
| `language_translate_breakdown` | `TranslationCompleted` |
| New: `atom_accept` | `KnowledgeAtomAccepted` |
| New: `atom_dismiss` / decay cron | `KnowledgeAtomArchived` |
| New: note save (semantic match) | `AtomReinforced` + `AtomInteracted` |
| New: right panel click/review | `AtomInteracted` |
| New: cross-domain reference | `KnowledgeTransferDetected` |

### Salience classification

| Event | Verdict | Reason |
|---|---|---|
| `KnowledgeAtomAccepted` | Extract | User explicitly valued — immediate LLM enrichment |
| `AtomFlashcardReviewed` | Accumulate | High volume, promote patterns after 5+ reviews |
| `TranslationCompleted` | Accumulate | Batch with other learning activity |
| `AtomReinforced` | Accumulate | Track cross-note patterns |
| `RetentionMilestoneReached` | Extract | Coaching celebration trigger |
| `KnowledgeAtomArchived` | Discard | Cleanup noise |
| `AtomInteracted` | Discard | Too frequent for extraction |

### Consumers

| Consumer | Events watched | Action |
|---|---|---|
| ActivityLogSubscriber | All learning events | Logs to activity_log; enables "23 min mastering concept X" summaries |
| BackgroundConsolidation | Extract/Accumulate gated | Enriches atoms with LLM context, detects cross-domain patterns |
| CoachingPatternDetector | Reviews, archives, milestones | Detects streaks, decay, momentum, domain gaps, knowledge transfer |
| Autotuner | Reviews (retention data) | Feeds retention curves into prompt optimization |
| ContextInferenceEngine | NoteStudied, TranslationCompleted | "Learning" becomes a recognized work context |

## 3. Auto-Extraction + Right Panel UX

### Extraction pipeline (runs on note save, background)

1. Debounce: skip if content hash unchanged since last extraction. The hash is stored as a KV entry keyed by `atom_extraction:{note_id}` (same pattern as `insight_reviews.input_hash`).
2. Background task (tokio::spawn, non-blocking):
   - Split note into semantic sections (by headings or ~200-word chunks).
   - Skip sections that already have active atoms (check by source_range overlap).
   - Call cognitive_provider with extraction prompt (1024 token budget — lightweight, fast).
   - Deduplicate against existing atoms (same subject + same note).
   - Check for cross-note semantic matches → emit `AtomReinforced` instead of creating duplicates.
   - Insert as `status="suggested"`, `salience=0.8`.
   - Emit `KnowledgeAtomCreated` for each.
3. Frontend: useQuery invalidation triggers right panel refresh.

### Vocabulary atom upgrade path

Existing `language_save_vocabulary` flow creates SemanticFact + Flashcard. Upgrade:
1. Create KnowledgeAtom (atom_type="vocabulary", source_note_id, source_range).
2. Link SemanticFact via `semantic_fact_id`.
3. Link Flashcard via `flashcard.atom_id`.
4. Status = "active" (vocab saves are already user-intentional).
5. Emit `KnowledgeAtomCreated` + `KnowledgeAtomAccepted`.

### Right panel layout

**Translation mode:**
- Existing: Translation, Words (with selection checkboxes), Grammar Patterns, Practice
- New section inserted between Translation and Words: **Knowledge Atoms**
  - Active atoms: full cards with subject, source snippet, retention % (color-coded: green >80%, amber 50-80%, red <50%), "Review" button
  - Separator: "Suggested"
  - Suggested atoms: dimmed cards with "Suggested" badge, Accept/Dismiss buttons, "Why this?" hover popover
  - Top of section: "Accept all (N)" button → opens modal with checkboxes + importance emoji picker
  - Collapsed expander: "Related from your knowledge (N)"

**Single/non-translate mode:**
- Knowledge Atoms section appears at the top of the right panel (above AI Suggestions)
- Same layout: active atoms, suggested atoms, related expander

### Inline quick review

User taps "Review" on an atom card → card expands in-place to show next due flashcard:
- New IPC command: `atom_next_card(atom_id)` → returns the next due flashcard for this atom (bypasses deck selection). Falls back to the most recently created card if none are due.
- Reuses existing `CardRenderer` component from Learn feature, adapted to accept an explicit `card` prop.
- Shows source context snippet below the card.
- Rating buttons (Again/Hard/Good/Easy) → calls existing `flashcard_record_review` → updates retention live → card collapses.
- No navigation to separate review screen for micro-reviews.

## 4. Coaching Integration + Decay System

### Decay engine (daily cron)

Registered as a `CronSchedule::Cron` with `tz` set to the user's local timezone (read from `chrono::Local` at registration time). Fires at 3 AM local.

1. Query active atoms where `last_interaction_ts < now - 7 days` (uses `idx_atoms_last_interaction` index).
2. Apply tiered decay weighted by personal_importance:
   - importance > 0.85: `salience *= 0.97` (slow decay, protected)
   - importance 0.5-0.85: `salience *= 0.92` (standard)
   - importance < 0.5: `salience *= 0.85` (fast decay)
3. Update `retention_pct` from FSRS retrievability: `R = 1 / (1 + elapsed / (9 * stability))`.
4. If `salience < 0.1` AND `retention_pct < 0.3` AND `last_interaction > 30 days`:
   - Set `status = "archived"`, emit `KnowledgeAtomArchived { reason: "decay" }`.
5. If `retention_pct < 0.6` AND `personal_importance > 0.7`:
   - Flag for coaching nudge (high-importance atom fading).
6. Compute topic-level aggregates (avg_retention, atom_count) → update knowledge_topics.
7. Emit `CoachingLearningDigest { fading_atoms, archived_atoms, streak_status, strongest_topic, weakest_topic }`.

### Coaching patterns (new in feature-coaching)

| Pattern | Trigger | Coaching action |
|---|---|---|
| StudyStreakPattern | Consecutive days with AtomFlashcardReviewed | Celebrate at 3/7/14/30 days; nudge within 24h on break |
| RetentionDecayPattern | High-importance atom drops below 60% | Contextual nudge with deep link; max 1/atom/week |
| LearningMomentumPattern | Creating >> reviewing (or vice versa) over 7 days | "Absorbing fast — want a catch-up session?" or "Strong retention week!" |
| DomainGapPattern | Retention declining across all atoms in a domain | "Chinese vocab dropped 12% — 15-min session would fix it" |
| KnowledgeTransferPattern | Atom from one domain referenced in another | "Your 验证 atom just helped in a PR note — second brain working!" |

### Coaching delivery

| Trigger | Delivery | Tone |
|---|---|---|
| Streak milestone | Morning briefing | Celebratory |
| Streak break | Chat, next session start | Gentle nudge |
| High-importance fading | Chat + note panel badge | Protective ("core to how you work") |
| Momentum imbalance | Morning briefing | Coaching insight |
| Domain-wide decay | Morning briefing (weekly) | Actionable suggestion |
| Retention personal best | Inline in chat | Pure celebration |
| Knowledge transfer | Inline in chat | Delight ("second brain working!") |
| Weekly Knowledge Health | Morning briefing | Summary digest |

Tone bias: 70% celebration, 30% gentle correction. Nudges lead with positive framing.

### Focus session micro-review

When a high-importance atom is flagged for decay AND the user starts a focus session in the same domain, inject a 45-second contextual micro-review at session start: "Before you dive in — 45s review on PR checklist to keep your streak alive?" with Yes/Skip. Presented inline in the productivity timer UI.

### Autotuner retention feedback loop

Nightly autotuner cycle gains a new metric: `knowledge_retention_score` — weighted average of `retention_pct` across active atoms, weighted by `personal_importance`. This is a **long-term metric** (changes over weeks), so it does NOT participate in per-trial evaluation (which operates per-request). Instead, it feeds a new `PromptContextSource` that adjusts the system prompt: topics with declining retention get reinforcement examples injected into agent responses. The autotuner's `MetricSnapshot` struct gains a `retention_context: Option<RetentionContext>` field for this data, but the existing `correction_rate`/`token_cost`/`response_time` evaluation logic is unchanged.

### Morning briefing integration

New "Knowledge Health" section in existing morning briefing:
- Atoms strengthened this week
- Atoms fading (with names)
- Strongest domain + retention %
- Study streak status
- [Start 5-min review] [See details] action buttons

## 5. Migration + Phase Delivery Plan

### One-time migration (first launch after upgrade)

1. Check migration flag in KV store.
2. Query all active vocabulary SemanticFacts (domain=learning, memory_type=vocabulary).
3. Create KnowledgeAtom for each, linking semantic_fact_id and flashcard.atom_id.
4. Auto-create topics from distinct domains.
5. Set migration flag.
6. Show toast: "Your knowledge graph is live — N atoms created. [Open a note] [Show my strongest topics]"

### Phase 1: "The Atom Core" (~3-4 days)

Foundation: atoms exist, vocab migrated, events flow, right panel shows atoms.

- knowledge_atoms + knowledge_topics tables + KnowledgeAtomRepo
- KnowledgeAtomService (create, accept, dismiss, archive)
- Migration job (vocab SemanticFacts → atoms)
- Upgrade language_save_vocabulary to create atoms
- Add atom_id FK to flashcards table
- 10 new DomainEvent variants + emission from handlers
- Salience classification for new events
- New command module: `crates/desktop/src/commands/atoms.rs` with `DEV_COMMANDS` export. Commands: `atoms_for_note`, `atom_accept`, `atom_dismiss`, `atom_next_card`, `atom_restore`. Add to `dev_server/mod.rs` dispatch and test coverage list.
- KnowledgeAtomsPanel component (right panel section)
- Inline quick review (embed CardRenderer)
- Launcher command: knowledge (basic explorer list view)
- Migration toast with "Show me my atoms" action

### Phase 2: "The Living Brain" (~3-4 days)

Auto-extraction, decay, activity tracking, simple topic heatmap.

- Extraction prompt + pipeline (on note save, debounced by content hash)
- Suggested atoms UX (dimmed cards, Accept/Dismiss, bulk-accept with importance)
- "Why this?" popover on suggested atoms
- Cross-note reinforcement detection (semantic match on note save)
- secondary_sources auto-linking
- Daily decay cron job (3 AM, tiered by personal_importance)
- Retention recomputation from FSRS retrievability
- Auto-archive at threshold
- CoachingLearningDigest event + consumption
- ActivityLogSubscriber handles all learning events
- ContextInferenceEngine: "learning" as work context
- Workspace toggle: "Auto-extract from new notes"
- Knowledge Health page with simple topic heatmap (colored bars + avg_retention + drill-down)

### Phase 3: "The Mentor" (~4-5 days)

Coaching patterns, autotuner retention, micro-reviews, cross-feature injection.

- 5 coaching pattern detectors (streak, decay, momentum, domain gap, knowledge transfer)
- Coaching message templates (70% celebration tone)
- Deep links: chat nudge → note with atom highlighted
- Morning briefing: Knowledge Health section
- Weekly Knowledge Health Snapshot digest
- Focus session micro-review injection (pre-session, 45s)
- Inline editor retention badge (fading atom margin pill)
- Autotuner: knowledge_retention_score metric
- Autotuner: per-topic prompt reinforcement
- Cross-feature atom injection (finance page, task list — surface relevant atoms)
- Undo/restore for archived atoms (7-day window)

### Phase 4: "The Dashboard" (~2-3 days)

Full visual knowledge graph and advanced analytics.

- Enhanced Knowledge Health page (full visual graph view)
- Cross-domain connections view
- Retention history charts (30/90 day sparklines)
- "Start focused review" from dashboard (topic-scoped)
- IPC: knowledge_health_summary, knowledge_topic_detail

## Testing Strategy

- Unit tests: KnowledgeAtomRepo CRUD, decay computation, salience classification
- Integration tests: vocab save → atom creation → flashcard linking → event emission
- E2E: migration of existing vocab, right-panel rendering, inline review flow
- All tests use ephemeral SQLite (StoragePool::connect_in_memory)

## Non-goals

- Multi-user knowledge sharing (single-user app)
- Real-time collaboration on atoms
- FSRS weight optimization from review history (future enhancement)
- Plugin/extension API for atoms (future enhancement)
