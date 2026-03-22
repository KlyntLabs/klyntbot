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

#### Domain model

```rust
// common crate — shared enum used across all layers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteIntent {
    Study,
    Research,
    Capture,
}

impl NoteIntent {
    /// Resolve effective intent from note override + notebook default.
    pub fn effective(note_override: Option<&str>, notebook_intent: &str) -> Self {
        let raw = note_override.unwrap_or(notebook_intent);
        match raw {
            "research" => Self::Research,
            "capture" => Self::Capture,
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

#### Behavior matrix

| Behavior | Study | Research | Capture |
|----------|-------|----------|---------|
| Atom extraction (background LLM) | Full | Skipped | Skipped |
| Embeddings | Yes | Yes | Yes |
| Entity mention extraction | Yes | Yes | Yes |
| Insight panel | 7 study tabs | 7 research tabs | Hidden |
| Background LLM calls on save | Atom extraction | None | None |
| FSRS flashcard pipeline | Full | Available on manual override | None |
| Flashcard generation (manual) | Yes | Yes | No (unless override) |

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

New prompts in `app-core/handlers/notes/insight_prompts.rs`:

- **Executive Summary**: "Synthesize the key findings, conclusions, and actionable insights from this research. Focus on what decisions this research supports, not what the reader should study."
- **Hypothesis Tracker**: "Extract the explicit and implicit claims/hypotheses in this research. For each, assess confidence level (high/medium/low/speculative) based on supporting evidence found in the content and related context."
- **Counter-arguments**: "Identify the strongest counter-arguments, alternative explanations, or risks that challenge the conclusions in this research. Draw on the provided context for contradicting evidence."

### 3. Atom Extraction Gating & Capture Mode

#### Primary gating: event publishing

```rust
// app-core/handlers/notes/crud.rs — after saving the note
let intent = NoteIntent::effective(
    note_row.intent_override.as_deref(),
    notebook_intent.as_str(),
);

// Entity mentions + links — always (all intents)
self.extract_links_and_mentions(&note_id, &body).await?;

// Embeddings — always (all intents)
if let Some(handler) = &self.embedding_handler {
    tokio::spawn(/* embed_note — unchanged */);
}

// Domain events — conditional on intent
match intent {
    NoteIntent::Study => {
        // NoteContentChanged triggers atom extraction
        self.bus.publish(DomainEvent::NoteContentChanged { note_id, .. });
    }
    NoteIntent::Research | NoteIntent::Capture => {
        // NoteUpdated for UI cache invalidation only — no atom extraction
        self.bus.publish(DomainEvent::NoteUpdated { note_id, .. });
    }
}
```

#### Safety-net gating: service-side check

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

`resolve_note_intent` query:
```sql
SELECT n.intent_override, nb.intent
FROM notes n
LEFT JOIN notebooks nb ON nb.id = n.notebook_id
WHERE n.id = ?
```

Dual gating: event-level (primary, prevents service from waking up) + service-level (safety net for race conditions during intent changes).

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

- **Any -> Study**: publish `NoteContentChanged` to trigger atom extraction. Show prompt: "Switching to Study mode. Want to run a full analysis and generate flashcards now?" with [Yes - Analyze now] / [Later] options.
- **Study -> Any**: existing atoms remain (not deleted), stop receiving FSRS updates, archive naturally via salience decay.
- **Any direction**: insight panel swaps tab set immediately. Cached insights for old intent stay versioned in `InsightContent` — no data loss.

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
const effectiveIntent = note.intentOverride ?? notebook.intent;

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
- Token regression: verify no background LLM calls for research/capture note saves
