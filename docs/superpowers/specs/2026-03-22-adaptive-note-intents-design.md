# Adaptive Note Intents: Purpose-Aware Knowledge System

**Date:** 2026-03-22
**Status:** Approved
**Phase:** Phase 1 (foundation + research tabs + instant cross-domain links)

## Problem

The notes system forces a study-first mental model onto every note. Research notes (finance deep-dives, project retrospectives, market research) trigger full atom extraction (LLM call per section on every save), display study-oriented insight tabs (self-assessment, practice history, cloze drills), and waste tokens on features that are actively wrong for analysis workflows. Meanwhile, the knowledge graph has no research-specific surfacing — no evidence maps, source trails, or cross-domain links.

Two compounding failures:
1. **Token waste** — background LLM calls fire on every note save regardless of purpose
2. **Missing research features** — study tabs don't serve analysis use cases; no hypothesis tracking, evidence mapping, or automatic cross-domain connections (e.g., episodic memory explaining a finance anomaly)

## Solution: Adaptive Note Intents

Notes become **purpose-aware** via a notebook-level default with per-note override. Three intents:

- **Study** — full learning pipeline (atoms, flashcards, FSRS-5, 7 study insight tabs). Current behavior, unchanged.
- **Research** — evidence maps, source trails, cross-domain links, hypothesis tracking. Graph-driven instant tabs + opt-in LLM "Deepen" tabs. No atom extraction.
- **Capture** — clean writing surface with lightweight enrichment (entity mentions + embeddings). No insight panel, no background LLM calls.

The system adapts to intent automatically. Users set purpose once at notebook creation; individual notes can override with one click.

## Design

### 1. Data Model & Intent Resolution

#### Schema changes

Pre-release: consolidate directly into existing `001_create_notes.sql` migration.

```sql
-- notebooks table
intent TEXT NOT NULL DEFAULT 'study' CHECK (intent IN ('study', 'research', 'capture'))

-- notes table
intent_override TEXT CHECK (intent_override IN ('study', 'research', 'capture'))
```

#### Schema migration note

Since SQLite `ALTER TABLE ADD COLUMN` silently ignores `CHECK` constraints, the new columns must be added directly to the `CREATE TABLE` statements in `001_create_notes.sql` (drop and recreate). Pre-release status means no user data to preserve.

#### Domain model

```rust
// feature-notes crate — domain enum for the notes subsystem
// Lives in feature-notes (L4), not common (L0), because NoteIntent is
// notes-domain-specific. cognitive (L5) already depends on feature-notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteIntent {
    Study,
    Research,
    Capture,
}

impl NoteIntent {
    /// Resolve effective intent from note override + notebook default.
    /// When notebook_intent is None (orphan note with no notebook), defaults to Study.
    pub fn effective(note_override: Option<&str>, notebook_intent: Option<&str>) -> Self {
        let raw = note_override.or(notebook_intent).unwrap_or("study");
        match raw {
            "research" => Self::Research,
            "capture" => Self::Capture,
            // Deliberate catch-all: any unrecognized value (typo, future intent added
            // to DB before Rust enum is updated) falls back to Study. The CHECK
            // constraint prevents invalid values in normal operation; this is a
            // defensive fallback, not silent error swallowing.
            _ => Self::Study,
        }
    }
}
```

New fields on `NoteRow` and `NotebookRow`:
- `NotebookRow.intent: String` (defaults to `"study"`)
- `NoteRow.intent_override: Option<String>` (nullable)
- `Note` domain model gains `intent: NoteIntent` (resolved at load time)
- `Notebook` domain model gains `intent: NoteIntent`

Orphan notes (no notebook, `notebook_id = NULL`) resolve to `Study` by default. The `LEFT JOIN` in `resolve_note_intent` returns `NULL` for `nb.intent` which maps to `None` in the Rust signature.

#### IPC contract changes (desktop-shared)

Both `NoteResponse` and `NotebookResponse` in `desktop-shared/src/commands/notes.rs` gain new fields:
- `NotebookResponse` + `intent: String`
- `NoteResponse` + `intent_override: Option<String>` + `effective_intent: String` (pre-resolved for frontend convenience)

#### Behavior matrix

| Behavior | Study | Research | Capture |
|----------|-------|----------|---------|
| Atom extraction (background LLM) | Full | Skipped | Skipped |
| Embeddings | Yes | Yes | Yes |
| Entity mention extraction | Yes | Yes | Yes |
| Insight panel | 7 study tabs (all auto) | 7 research tabs (4 instant + 3 on-demand) | Hidden |
| Background LLM calls on save | Atom extraction | None | None |
| FSRS flashcard pipeline | Full (atom-linked) | Manual only (via editor toolbar "Generate Cards") | None |
| Flashcard generation | Yes (editor toolbar + atom-linked) | Yes (editor toolbar only — no atom suggestions) | No (unless intent override to study) |

### 2. Research Tab Manifest & Progressive Disclosure

#### Study tabs (existing, unchanged)

| Tab | Source | Trigger |
|-----|--------|---------|
| Atoms | LLM extraction (background) | Auto on save |
| Synthesis | LLM streaming | Auto on panel open |
| Gap Analysis | LLM | Parallel with synthesis |
| Self-Assessment | LLM | Parallel with synthesis |
| Concept Map | LLM -> Mermaid | Parallel with synthesis |
| Perspectives | LLM x N personas | Parallel with synthesis |
| Practice History | DB query | Instant |

#### Research tabs (new)

**Graph-driven (instant, zero LLM):**

| Tab | Data Source | Description |
|-----|------------|-------------|
| Evidence Map | `note_entity_mentions` + `EntityRepo` relationships + backlinks | Entity graph with relationships, mention counts, and source notes |
| Source Trail | `note_links` (wikilinks) + `get_backlinks_with_context()` | Provenance chain showing cites/cited-by with context snippets |
| Cross-domain Links | Entity overlap + vector similarity + +/-7 day temporal window | Results grouped by domain: Finance, Episodic, Notes, Tasks/Projects |
| Risks, Gaps & Next Steps | Entity graph holes + dead wikilinks + sparse connections | Actionable items with suggested actions: "Create note", "Link transaction", "Create task" |

**LLM-driven (opt-in via "Deepen with AI" button):**

| Tab | Description | Max Tokens |
|-----|------------|------------|
| Executive Summary | Key findings, conclusions, actionable insights. Decision-focused, not study-focused. | 4096 |
| Hypothesis Tracker | Extracts claims/hypotheses with confidence levels (high/medium/low/speculative) | 4096 |
| Counter-arguments | Strongest counter-arguments, alternative explanations, risks. Uses cognitive context for contradicting evidence. | 4096 |

#### Progressive disclosure UX

- Graph-driven tabs load instantly on panel open (zero latency, zero cost)
- LLM tabs show a placeholder with brief description + "Deepen with AI" button
- "Generate Full Analysis" button at top triggers all 3 LLM tabs simultaneously
- All LLM results versioned and cached in `InsightContent` (reuses existing storage)
- Cached per note content hash — repeated opens are free

#### Research prompt templates

Add to the existing `app-core/handlers/notes/insight_prompts.rs` file (new public functions alongside the existing study prompt functions):

- **Executive Summary**: "Synthesize the key findings, conclusions, and actionable insights from this research. Focus on what decisions this research supports, not what the reader should study."
- **Hypothesis Tracker**: "Extract the explicit and implicit claims/hypotheses in this research. For each, assess confidence level (high/medium/low/speculative) based on supporting evidence found in the content and related context."
- **Counter-arguments**: "Identify the strongest counter-arguments, alternative explanations, or risks that challenge the conclusions in this research. Draw on the provided context for contradicting evidence."

### 3. Atom Extraction Gating & Capture Mode

#### Fetching notebook intent in crud.rs

The `note_create` and `note_update` handlers need the notebook's intent to resolve `effective_intent`. Since these handlers already have `notebook_id` from the note row, add a lightweight repo method:

```rust
// NoteRepo or NotebookRepo — single-column fetch
pub async fn get_notebook_intent(&self, notebook_id: &str) -> Result<Option<String>> {
    // Returns None if notebook doesn't exist (orphan note safety)
    sqlx::query_scalar("SELECT intent FROM notebooks WHERE id = ?")
        .bind(notebook_id)
        .fetch_optional(&self.pool)
        .await
}
```

For orphan notes (`notebook_id = None`), skip the fetch and pass `None` to `NoteIntent::effective()`.

#### Primary gating: suppress NoteContentChanged for non-study notes

The key mechanism: `AtomExtractionService` only subscribes to `NoteContentChanged`. For non-study notes, we simply do not publish this event. `NoteUpdated` (already used for UI cache invalidation) continues to be published for all intents — it has no effect on atom extraction.

```rust
// app-core/handlers/notes/crud.rs — after saving the note
let notebook_intent = match note_row.notebook_id.as_deref() {
    Some(nb_id) => self.repos.notebooks.get_notebook_intent(nb_id).await?,
    None => None,
};
let intent = NoteIntent::effective(
    note_row.intent_override.as_deref(),
    notebook_intent.as_deref(),
);

// Entity mentions + links — always (all intents)
self.extract_links_and_mentions(&note_id, &body).await?;

// Embeddings — always (all intents)
if let Some(handler) = &self.embedding_handler {
    tokio::spawn(/* embed_note — unchanged */);
}

// NoteUpdated — always (UI cache invalidation)
self.bus.publish(DomainEvent::NoteUpdated { note_id: note_id.clone(), .. });

// NoteContentChanged — study only (triggers atom extraction)
if intent == NoteIntent::Study {
    self.bus.publish(DomainEvent::NoteContentChanged { note_id, .. });
}
```

This pattern also applies to `note_version_restore`. Note: `note_version_restore` calls `NoteRepo::update_note` directly (not `AppCore::note_update`), so the intent-fetch-and-gate logic must be added to the restore handler itself — it is NOT inherited from the `note_update` handler. The restore handler must: fetch notebook intent, resolve effective intent, and conditionally publish `NoteContentChanged` using the same logic above.

#### Safety-net gating: service-side check

Defense-in-depth for race conditions (e.g., intent changes after `NoteContentChanged` was already queued):

```rust
// cognitive/src/services/atom_extraction.rs — in event handler
DomainEvent::NoteContentChanged { note_id, .. } => {
    let intent = resolve_note_intent(&pool, &note_id).await;
    if intent != NoteIntent::Study {
        debug!(note_id, ?intent, "skipping atom extraction — non-study intent");
        continue;
    }
    // ... existing debounce + hash check + extraction logic unchanged
}
```

`resolve_note_intent` query (returns `Option<String>` for both columns due to `LEFT JOIN`):
```sql
SELECT n.intent_override, nb.intent
FROM notes n
LEFT JOIN notebooks nb ON nb.id = n.notebook_id
WHERE n.id = ?
```

Dual gating: event-level (primary, prevents `NoteContentChanged` from being published) + service-level (safety net for race conditions).

#### Capture mode behavior

What Capture gets (all zero-LLM, already wired):
- Entity mention extraction via `extract_links_and_mentions()` — unchanged
- Embeddings via `NoteEmbeddingHandler` — unchanged
- Temporal markers via existing `created_at` / `updated_at`

What Capture skips:
- Atom extraction (gated above)
- Insight panel (frontend hides panel)
- All background LLM calls
- Flashcard generation suggestions

#### Intent change handling

- **Any -> Study**: show prompt: "Switching to Study mode. Want to run a full analysis and generate flashcards now?" with [Yes - Analyze now] / [Later]. "Yes" publishes `NoteContentChanged` via a dedicated `trigger_study_analysis(note_id)` handler that bypasses the normal debounce window (uses a separate event or resets the debounce map entry). "Later" defers — atom extraction triggers on the next normal save.
- **Study -> Any**: existing atoms remain (not deleted), stop receiving FSRS updates, archive naturally via salience decay.
- **Any direction**: insight panel swaps tab set immediately. Cached insights for old intent stay versioned in `InsightContent` — no data loss.

Note: the existing 5-second debounce in `AtomExtractionService` would suppress a `NoteContentChanged` event if the user switches intent within 5 seconds of the last save. The `trigger_study_analysis` handler addresses this by either resetting the debounce entry or using a force flag on the event.

#### Token impact estimate

For a user with 50% research/capture notes:
- Atom extraction: ~50% fewer background LLM calls (1024 max_tokens per section, eliminated for non-study)
- Insight pipeline: ~50% fewer parallel LLM batches (4-10 calls x 4096 tokens, eliminated for non-study; research only fires on explicit Deepen click)
- Net effect: background token spend drops roughly in half

### 4. Agent Tool Integration

#### NotesTool changes

Add `intent` parameter to `create` and `search` actions. Add `intent` filter to `list_by_entity`.

```json
{
    "intent": {
        "type": "string",
        "enum": ["study", "research", "capture", "auto"],
        "description": "Note purpose. auto (default) inherits from notebook or infers from calling skill context."
    }
}
```

Schema cost: ~45 additional tokens.

**Smart defaults**: when `intent` is `"auto"` (or omitted):
- Called from finance/project skills -> defaults to `research`
- Called from general/task-management skills -> inherits notebook default
- On `create`: sets `intent_override` only if explicitly passed (non-auto)
- On `search`/`list_by_entity`: filters by intent when specified, returns all intents when omitted

Add a new `NoteRepo::list_notes_by_entity_with_intent` method (or add an optional `intent` parameter to the existing `list_notes_by_entity`) to support intent-filtered entity queries.

No changes to: `get`, `update`, `delete`, `list`, notebook/link/tag actions (remain intent-agnostic).

### 5. Frontend UX

#### Purpose badge (editor header)

Subtle pill next to note title showing current effective intent with icon:
- `Study` — soft indigo, icon: book
- `Research` — warm amber/teal, icon: chart
- `Capture` — cool slate, icon: notebook

Click opens `glass-panel` popover with 3 options + brief descriptions.
Keyboard shortcut: `Cmd/Ctrl + Shift + P`.

Hover tooltip micro-hints:
- Study: "Atoms - Flashcards - Spaced repetition"
- Research: "Evidence maps - Cross-domain links - Source trails"
- Capture: "Clean writing - Silently connects finance & tasks"

#### Notebook creation

Purpose selection as a step in notebook creation flow:

```
What's the purpose of this notebook?

  Book  Study / Learning
     Full learning pipeline — atoms, flashcards, spaced repetition

  Chart  Research / Analysis
     Evidence maps, source trails, cross-domain links

  Notebook  Capture / Journal
     Clean writing surface — silently connects the dots
```

Default: Study. Light inference from title: titles containing "analysis", "review", "Q2", "project", "research" pre-select Research with "Recommended for analysis" badge.

#### Sidebar hints

Notebooks show purpose icon next to title. Notes with `intent_override` different from notebook show a small override indicator dot.

#### InsightReviewPanel branching

```typescript
// Use the server-resolved effective_intent (canonical source of truth).
// Do NOT resolve locally via note.intentOverride ?? notebook.intent — the
// server handles orphan notes, fallback logic, and future intent additions.
const { effectiveIntent } = note;

if (effectiveIntent === 'capture') return null;

const tabs = effectiveIntent === 'research' ? RESEARCH_TABS : STUDY_TABS;
```

Tab manifests are static config objects defining: tab id, label, icon, source (`'graph'` | `'llm'`), and IPC command. Graph tabs go to `done` on load. LLM tabs start `idle` with Deepen button.

### 6. IPC Commands (new)

```rust
// Graph-driven (instant, zero LLM)
"research_evidence_map"        // note_id -> EvidenceMapResponse
"research_source_trail"        // note_id -> SourceTrailResponse
"research_cross_domain_links"  // note_id -> Vec<CrossDomainLink>
"research_risks_gaps"          // note_id -> Vec<RiskGapItem>

// LLM-driven (on-demand)
"research_deepen_tab"          // note_id, tab_id -> streaming insight
```

Per CLAUDE.md, every new command module with `#[tauri::command]` functions must export `pub const DEV_COMMANDS: &[&str]` listing all command names, and be registered in `dev_server/mod.rs`. The 5 new research IPC commands must be included. The `dev_server_covers_all_tauri_commands` test enforces this at compile time.

#### Response types (desktop-shared)

```rust
pub struct EvidenceMapResponse {
    pub entities: Vec<EvidenceEntity>,
    pub relationships: Vec<EntityRelation>,
    pub source_notes: Vec<NoteReference>,
}

pub struct CrossDomainLink {
    pub domain: String,          // "finance", "episodic", "notes", "tasks"
    pub title: String,
    pub relevance: f64,          // 0-1 combined score
    pub connection_type: String, // "entity_overlap", "temporal", "semantic"
    pub snippet: Option<String>,
}

pub struct RiskGapItem {
    pub gap_type: String,        // "missing_source", "sparse_connections", "dead_link"
    pub subject: String,
    pub suggested_action: Option<String>,
}
```

## Phase 2 (deferred)

- Cross-domain Links "Deepen connections" button (LLM-decomposed `InsightForge` query for non-obvious patterns)
- Finance skill auto-query integration (automatically surface research notes during spending analysis)
- Smart notebook creation inference (heuristic pre-selection + "Recommended" badge refinement)
- Visual treatment polish (mode-specific accent colors on sidebar, editor chrome)
- Configurable Capture depth per notebook (Minimal vs Smart)
- Work context integration (research notes auto-linked to active work sessions)

## Non-goals

- Structured observability — single-user app, existing tracing is sufficient
- Breaking changes to study flow — existing behavior is completely preserved
- Automatic intent detection from note content — intent is set at notebook level with explicit per-note override, not inferred from content
- Migration scripts — pre-release, schema changes are direct alterations

## Testing Strategy

- Unit tests for `NoteIntent::effective()` resolution logic
- Integration tests: create notes with each intent, verify atom extraction fires only for study
- Integration tests: research tab IPC commands return correct data shapes
- Frontend: verify panel hides for capture, swaps tabs for research vs study
- Agent tool: verify `intent` parameter on create/search/list_by_entity filters correctly
- Token regression: subscribe to `DomainEventBus` in test, save a research/capture note, assert `NoteContentChanged` is never published (only `NoteUpdated`). This validates the primary gate without needing an LLM test double.
