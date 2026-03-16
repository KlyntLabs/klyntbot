# Insight Review — Design Specification (v2)

**Date:** 2026-03-16
**Status:** Approved
**Feature:** Transform the disabled "Synthesize" button in the AI Suggestions panel into a full "Insight Review" system — a deep AI-powered research companion that synthesizes knowledge, finds gaps, generates quizzes, maps concepts, and offers multi-perspective analysis through pluggable personas.

**Inspired by:** MiroFish's multi-agent simulation pipeline — adapted from "simulate thousands of agents on social media" to "simulate expert perspectives on your personal knowledge network."

---

## 1. Overview

### Trigger
- **Button name:** "Insight Review"
- **Icon:** `Brain` (lucide-react)
- **Tooltip:** "Deep analysis of your knowledge network"
- **Keyboard shortcut:** `Cmd+Shift+I` (toggle)
- **Location:** Replaces the current disabled "Synthesize" button in `AISuggestionsPanel.tsx`

### Behavior
When activated, the existing right-side context panel (~280px) smoothly expands to **640px** and replaces its content with the Insight Review interface. The editor area shrinks proportionally. On close (X, Escape, or `Cmd+Shift+I`), the panel contracts back to ~280px and restores normal context sections.

### Loading Strategy: Hybrid (Option C)
1. **On open:** Immediately start a streaming LLM call for Tab 1 (Synthesis). User sees first tokens in ~2-3 seconds.
2. **In parallel:** Fire off Tabs 2-5 (Gap Analysis, Self-Assessment, Concept Map, Perspectives) as structured JSON calls in the background.
3. **Tab switching:** Instant for any tab that has finished loading. Skeleton loaders shown for in-progress tabs.

### Edge Cases
- **Empty note / no body:** Show a friendly message: "Add some content to your note to generate insights." Disable the Insight Review button when `note.body` is empty or whitespace-only.
- **No related notes:** Still generate insights from the current note alone + cognitive memory. The LLM prompts handle this gracefully (context block will just have fewer entries). Synthesis tab notes "limited connections found" if < 2 related notes.
- **LLM failure:** Per-tab error state with retry button. Other tabs continue independently. Global "Regenerate All" in header retries all failed tabs.
- **Very long notes:** Context assembly caps total input at ~12,000 tokens. Current note gets up to 4,000 tokens, related notes split the remaining budget (truncate longest first).

---

## 2. The 5 Tabs

### Tab 1: Synthesis
- Deep, well-written synthesis connecting the current note + all related notes + relevant semantic facts from cognitive memory.
- Output: clean Markdown with key themes, connections, and non-obvious insights.
- **Streamed** — first tab visible, first tokens in ~2-3s.

### Tab 2: Gap Analysis
- Detects knowledge gaps, missing concepts, contradictions, shallow coverage.
- Suggests specific next steps (papers, topics, new notes to create).
- Leverages cognitive memory + vector search for context.
- Output: structured Markdown with clear sections.
- **Deep Dive actions:** Each identified gap includes a "Create Note" button that auto-generates a starter note with: title, outline, key questions to answer, and backlinks to source notes. See §6 for details.

### Tab 3: Self-Assessment
- Generates 8 quiz questions: 4 multiple choice + 4 short answer.
- Each question includes: type, question text, choices (if MC), correct answer, explanation, source notes, difficulty ("easy"/"medium"/"hard"), difficulty_score (0.0-1.0), and a unique id.
- Interactive quiz UI with reveal-on-check, running score, and "Reveal All Answers" button (appears after the user has answered 3+ individual questions).
- After completing quiz: prominent "Save as Flashcard Deck" option.
- **Scenario Challenge:** After 50%+ quiz completion, a bonus scenario section appears — a realistic problem requiring cross-note concept application. See §7 for details.

### Tab 4: Concept Map
- Mermaid `mindmap` diagram showing concept connections across the note cluster.
- Max 4 levels deep, max 5-6 branches per node, max 6 words per label.
- Fallback: if Mermaid parsing fails, show clean indented text outline.
- "Copy Mermaid code" button for power users.
- Dark theme matching glassmorphism design system.

### Tab 5: Perspectives (NEW — Pluggable Persona System)
- 3-4 expert personas analyze the note cluster from different viewpoints.
- Each persona writes 2-3 paragraphs: agreements, challenges, extensions, and recommendations.
- Personas can **disagree with each other** — this is a feature, not a bug.
- Personas are selected from the **Persona Registry** (see §3).
- User can swap personas and regenerate individual perspectives.
- Output: structured Markdown with a section per persona (name, role, then analysis).

---

## 3. Pluggable Persona System

### Design Principle
Personas are **data, not code**. They live in a registry (SQLite), can be added/removed/modified at any time, and are auto-selected based on domain relevance. The system ships with builtins but encourages user customization.

### Persona Template

```rust
struct PersonaTemplate {
    id: String,              // nanoid
    name: String,            // "Dr. Sarah Chen"
    role: String,            // "ML Research Scientist"
    expertise: String,       // "Deep learning architectures, NLP, evaluation methods"
    perspective: String,     // System prompt fragment describing their viewpoint
    tone: String,            // "analytical", "provocative", "encouraging", "skeptical"
    icon: String,            // Emoji or lucide icon name: "🔬", "⚖️", "🎯"
    source: PersonaSource,   // Builtin, AutoGenerated, UserCreated
    domains: Vec<String>,    // Tags/topics this persona is relevant for: ["ml", "ai", "research"]
    is_active: bool,         // User can deactivate without deleting
    created_at: String,
    updated_at: String,
}

enum PersonaSource {
    Builtin,        // Shipped with app, cannot be deleted (but can be deactivated)
    AutoGenerated,  // Created by LLM based on note domain
    UserCreated,    // Created manually by the user
}
```

### Built-in Personas (shipped with app)

| Name | Role | Tone | Perspective |
|------|------|------|-------------|
| The Skeptic | Critical Analyst | skeptical | Challenges assumptions, demands evidence, identifies logical fallacies and unsupported claims. |
| The Practitioner | Industry Professional | pragmatic | Focuses on real-world application — what works in practice vs. theory? What are the implementation pitfalls? |
| The Connector | Interdisciplinary Researcher | curious | Draws parallels to other fields, identifies transferable patterns, suggests unexpected connections. |
| The Student | Curious Learner | inquisitive | Asks the "dumb" questions — tests whether explanations are clear, identifies jargon, and flags concepts that need unpacking. |
| The Strategist | Systems Thinker | analytical | Looks at the bigger picture — second-order effects, trade-offs, long-term implications, and strategic considerations. |
| The Devil's Advocate | Contrarian Thinker | provocative | Deliberately argues the opposite position. Steelmans counterarguments the user hasn't considered. |

All six ship as builtins. Default selection: 3-4 per review (see selection algorithm below).

### Persona Selection Algorithm

When Insight Review opens for a note:

1. **Check pinned personas**: If the user has pinned specific personas to this note → use those.
2. **Check domain match**: Match note tags + cognitive memory topics against persona `domains` array.
3. **Auto-generate if needed**: If < 2 domain-relevant personas exist, fire an LLM call to generate 1-2 domain-specific personas and save them to the registry for future use.
4. **Select 3-4**: Always include 1 builtin (rotate: Skeptic → Connector → Strategist) + 2-3 best domain matches.
5. **Diversity rule**: Never select two personas with the same `tone`.

### Persona Auto-Generation

When the LLM creates a new persona (step 3 above), it receives:
- The note's title, tags, and first 500 chars
- The list of existing personas (to avoid duplicates)

It returns a `PersonaTemplate` JSON blob which is inserted into the registry with `source: AutoGenerated`.

**Rate limit**: Max 3 auto-generated personas per domain per week (prevents unbounded growth).

### User Persona Management

Users can:
- **Create** custom personas via a simple form (name, role, expertise, perspective, tone, domains)
- **Edit** any non-builtin persona
- **Deactivate** any persona (including builtins) — hidden from selection but not deleted
- **Delete** auto-generated and user-created personas
- **Pin** specific personas to a note — overrides auto-selection
- **Import/Export** personas as JSON (for sharing)

UI: Accessible via a small "Manage Personas" gear icon in the Perspectives tab header.

### Persona Interaction in Perspectives Tab

Each persona's response section includes:
- **Avatar + name + role** header
- **Analysis content** (2-3 paragraphs, markdown)
- **"Ask follow-up"** button → opens a mini-chat with that persona (1-3 exchanges, same context)
- **"Swap persona"** button → pick a different persona, regenerate just that slot
- **Rating** (thumbs up/down) → adjusts persona's relevance score for future auto-selection

---

## 4. Database Schema

### `flashcards` table (in `cognitive` crate)

```sql
CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_session_id TEXT,
    insight_review_id TEXT,
    deck TEXT NOT NULL DEFAULT 'general',
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'short_answer',
    choices JSON,

    -- FSRS scheduling (defaults are for cards inserted outside of insight_save_flashcards)
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 0.5,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX idx_flashcards_due ON flashcards(due_at);
CREATE INDEX idx_flashcards_deck ON flashcards(deck);
CREATE INDEX idx_flashcards_insight ON flashcards(insight_review_id);
```

### `insight_review_cache` table

```sql
CREATE TABLE IF NOT EXISTS insight_review_cache (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    synthesis TEXT,
    gap_analysis TEXT,
    self_assessment TEXT,
    concept_map TEXT,
    perspectives TEXT,
    persona_ids JSON,
    updated_at TEXT NOT NULL,

    UNIQUE(note_id, content_hash)
);

CREATE INDEX idx_insight_cache_note ON insight_review_cache(note_id);
```

**Changes from v1:** Added `version` (for cross-session tracking), `perspectives`, and `persona_ids` columns.

Per-tab caching: both combined response and individual tab outputs are stored so "Regenerate Tab X" works without re-running everything. The `updated_at` column tracks when any tab was last regenerated.

### `insight_personas` table (NEW)

```sql
CREATE TABLE IF NOT EXISTS insight_personas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    expertise TEXT NOT NULL,
    perspective TEXT NOT NULL,
    tone TEXT NOT NULL DEFAULT 'analytical',
    icon TEXT NOT NULL DEFAULT '🧠',
    source TEXT NOT NULL DEFAULT 'builtin',
    domains JSON NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    relevance_score REAL NOT NULL DEFAULT 0.5,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_personas_source ON insight_personas(source);
CREATE INDEX idx_personas_active ON insight_personas(is_active);
```

### `insight_persona_pins` table (NEW)

```sql
CREATE TABLE IF NOT EXISTS insight_persona_pins (
    note_id TEXT NOT NULL,
    persona_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (note_id, persona_id)
);
```

### Cache Invalidation Strategy
The `content_hash` is computed from: `SHA-256(note.body + sorted(related_note_ids))`. This means the cache invalidates when:
- The current note's body changes
- The set of related notes changes (e.g., new notes become semantically similar)

**Accepted trade-off:** Changes to a related note's *body* do not invalidate the cache (only the related note set matters). This is acceptable because: (1) insight reviews are a point-in-time analysis, (2) the user can always click "Regenerate" to refresh, and (3) computing hashes of all related note bodies on every open would be expensive.

### Cross-Session Tracking

Previous cache entries are **not deleted** on cache miss — only a new row with an incremented `version` is inserted. This enables the "What's Changed" banner (§8). Cleanup: entries older than 30 days or beyond the 5 most recent versions per note are pruned on app startup.

### Migration
All tables (`flashcards`, `insight_review_cache`, `insight_personas`, `insight_persona_pins`) are added to the existing `cognitive` crate migration. Per CLAUDE.md pre-release policy: bump the `FeatureMigration` version in `crates/cognitive/src/repos/mod.rs` and append the new DDL to the existing SQL in-place (no incremental migration files).

### FSRS Init on Save
- easy difficulty → stability 4.0 days
- medium difficulty → stability 2.0 days
- hard difficulty → stability 0.8 days

### FlashcardRepo API (in `crates/cognitive/src/repos/`)

```rust
impl FlashcardRepo {
    pub async fn create_batch(&self, cards: Vec<NewFlashcard>) -> Result<Vec<FlashcardRow>>;
    pub async fn get_due_cards(&self, deck: Option<&str>, limit: usize) -> Result<Vec<FlashcardRow>>;
    pub async fn record_review(&self, id: &str, quality: ReviewQuality) -> Result<FlashcardRow>;
    pub async fn list_by_note(&self, note_id: &str) -> Result<Vec<FlashcardRow>>;
    pub async fn list_decks(&self) -> Result<Vec<DeckSummary>>;
    pub async fn delete_deck(&self, deck: &str) -> Result<()>;
}
```

`ReviewQuality` enum: `Again`, `Hard`, `Good`, `Easy`.

Deck naming: auto-generated as `Review: [Note Title]` or `[Domain] Knowledge Review`.

### PersonaRepo API (NEW)

```rust
impl PersonaRepo {
    pub async fn list_active(&self) -> Result<Vec<PersonaRow>>;
    pub async fn list_all(&self) -> Result<Vec<PersonaRow>>;
    pub async fn get(&self, id: &str) -> Result<Option<PersonaRow>>;
    pub async fn create(&self, persona: NewPersona) -> Result<PersonaRow>;
    pub async fn update(&self, id: &str, updates: PersonaUpdate) -> Result<PersonaRow>;
    pub async fn set_active(&self, id: &str, active: bool) -> Result<()>;
    pub async fn delete(&self, id: &str) -> Result<()>; // Errors on builtin
    pub async fn select_for_note(&self, note_tags: &[String], pinned: &[String]) -> Result<Vec<PersonaRow>>;
    pub async fn update_relevance(&self, id: &str, delta: f64) -> Result<()>;

    // Pins
    pub async fn get_pins(&self, note_id: &str) -> Result<Vec<String>>;
    pub async fn set_pins(&self, note_id: &str, persona_ids: &[String]) -> Result<()>;
}
```

---

## 5. InsightForge Context Assembly (Upgraded)

Before calling the LLM, the handler assembles context using a **multi-dimensional retrieval strategy** inspired by MiroFish's InsightForge search pattern.

### Step 1: Query Decomposition (NEW)

A fast LLM call (~0.5s, structured JSON, small model) decomposes the note's topic into 3-5 sub-queries:

```
Given this note title and first 300 chars, generate 3-5 semantic search queries
that would retrieve different dimensions of relevant knowledge.

Note: "{title}" — "{body_preview}"

Return JSON: { "queries": ["query1", "query2", ...] }
```

### Step 2: Parallel Multi-Source Retrieval

For EACH sub-query (in parallel):
1. **Cognitive memory:** top 5 semantic facts via `MemoryRetriever` (scored by FSRS retrievability + relevance)
2. **Note search:** top 3 related notes via embedding similarity

Plus the base retrieval (always runs):
3. **Current note:** title + full body
4. **Direct related notes:** top 8 from `note_suggestions` scoring (title + first 500 chars each)
5. **Link graph:** backlinks + outlinks for the current note
6. **Tags:** all tags across the note cluster

### Step 3: Merge & Deduplicate

- Deduplicate notes/facts by ID
- Rank by frequency across sub-queries (a fact found by 3/5 sub-queries ranks higher)
- Apply token budget: ~12,000 total. Current note: up to 4,000. Related notes + facts split remaining budget (truncate longest first).

### Step 4: Context Block Assembly

The final context block is injected into each tab's prompt with clear sections:

```
--- BEGIN KNOWLEDGE CONTEXT ---

## Current Note
Title: {title}
Tags: {tags}
{body}

## Related Notes (ranked by relevance)
### {related_note_1_title}
{content_preview}
...

## Semantic Facts from Memory
- {fact_1} (relevance: 0.92)
- {fact_2} (relevance: 0.87)
...

## Link Graph
Backlinks: {note_titles}
Outlinks: {note_titles}

--- END KNOWLEDGE CONTEXT ---
```

### Performance Note
The query decomposition adds ~0.5-1s latency but significantly enriches context — sub-queries catch connections that single-query search misses. For a note about "distributed systems", the decomposition might generate: "consensus algorithms", "CAP theorem tradeoffs", "real-world distributed failure modes", "microservice patterns" — each pulling different related notes.

---

## 6. Deep Dive Note Generation (Gap Analysis Extension)

### Behavior
Each gap identified in Tab 2 includes a **"Create Note"** action button. When clicked:

1. LLM generates a starter note from the gap description:
   - **Title:** derived from the gap (e.g., "Consensus Algorithms — Deep Dive")
   - **Outline:** 3-5 sections with key questions to answer
   - **Seed content:** 1-2 paragraphs summarizing what's known from existing notes
   - **Backlinks:** auto-links to the source notes that surfaced this gap
   - **Tags:** inherited from the parent note cluster

2. The note is created via the existing `note_create` IPC command.
3. A toast notification confirms creation with a "Go to note" link.

### LLM Prompt for Note Generation

```
Given this knowledge gap identified in the user's notes, generate a starter note
that helps them explore this topic.

Gap: {gap_description}
Source notes: {source_note_titles}
Existing knowledge: {relevant_context_from_parent_notes}

Return JSON:
{
  "title": "...",
  "body": "## Overview\n...\n\n## Key Questions\n- ...\n\n## What We Know So Far\n...\n\n## Next Steps\n- ...",
  "tags": ["tag1", "tag2"]
}
```

### Gap Analysis Prompt Update
The Gap Analysis prompt (Tab 2) is updated to include a structured `gaps` array in its output for machine-parseable gap extraction:

```json
{
  "markdown": "... human-readable markdown ...",
  "gaps": [
    {
      "id": "gap-1",
      "title": "Consensus Algorithms",
      "type": "missing_concept",
      "description": "Referenced in distributed systems notes but never explored",
      "source_notes": ["Distributed Systems Overview", "CAP Theorem Notes"],
      "priority": "high"
    }
  ]
}
```

---

## 7. Scenario Challenges (Self-Assessment Extension)

### Behavior
After the user has answered 50%+ of quiz questions, a **"Scenario Challenge"** section appears below the quiz:

- A realistic, multi-step problem that requires applying concepts from 2+ notes
- Presents a situation, asks the user to reason through it
- Includes decision points with consequences
- After the user writes their response, they can reveal the model answer

### Example
> **Scenario:** You're designing a new microservice that needs to handle 10K requests/sec with strong consistency guarantees. Your team suggests using eventual consistency for performance. Based on your notes on CAP theorem and distributed consensus...
>
> 1. What trade-off is your team proposing?
> 2. Under what conditions would you agree? Disagree?
> 3. What alternative architecture would you suggest?

### Flashcard Integration
Scenarios can be saved as a special flashcard type (`card_type: 'scenario'`) with higher initial difficulty (stability: 0.5 days).

---

## 8. Cross-Session Insight Tracking

### "What's Changed" Banner
When the user re-opens Insight Review and the cache misses (content has changed), the system checks for a previous cache entry for the same note. If found:

1. A brief LLM call compares the previous synthesis vs. the new context (not the new synthesis — that hasn't been generated yet). It returns a 1-2 sentence summary of what changed.

2. A banner appears at the top of the panel:
   > "Since your last review: 2 new related notes found, 1 previous gap addressed."

3. After the new review completes, the banner updates with a diff summary:
   > "New insight: connection between X and Y not seen in previous review."

### Implementation
- `insight_review_cache` retains previous versions (column: `version INTEGER`)
- On cache miss: query `SELECT * FROM insight_review_cache WHERE note_id = ? ORDER BY version DESC LIMIT 1`
- Compare `content_hash` to confirm it's actually stale (not just a re-open of the same content)
- LLM diff call is fire-and-forget — the banner updates async, doesn't block tab generation

### Cleanup
On app startup, prune entries older than 30 days or beyond the 5 most recent versions per note.

---

## 9. LLM Prompts

### Tab 1: Synthesis

```
You are a research synthesis assistant. Given the user's note and its related notes
from their knowledge base, write a deep synthesis that:

1. Identifies the 3-5 key themes across these notes
2. Draws non-obvious connections between concepts
3. Highlights where ideas reinforce or build on each other
4. Surfaces insights the user may not have explicitly written

Format as clean Markdown with ## headings for each theme.
Keep it focused and insightful — not a summary, but a synthesis.
Do not repeat content verbatim from the notes.

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 2: Gap Analysis (Updated — dual output)

```
You are a knowledge gap analyst. Given the user's note cluster, identify:

1. **Missing concepts** — important topics referenced but never explored in depth
2. **Contradictions** — places where notes disagree or present conflicting info
3. **Shallow coverage** — topics mentioned briefly that deserve deeper treatment
4. **Research suggestions** — specific papers, books, or topics to explore next
5. **Notes to create** — suggest 2-3 new note titles that would strengthen the network

Format as Markdown with clear sections. Be specific and actionable.
For each gap, reference which note(s) it relates to.

ALSO return a machine-readable JSON block at the end, wrapped in ```json fences:
[{
  "id": "gap-1",
  "title": "short title",
  "type": "missing_concept" | "contradiction" | "shallow_coverage",
  "description": "1-2 sentence description",
  "source_notes": ["note title 1"],
  "priority": "high" | "medium" | "low"
}]

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 3: Self-Assessment

```
You are an educational assessment designer. Generate a self-assessment quiz based
on the user's knowledge network.

Generate exactly 8 questions:
- 4 multiple choice (4 options each, one correct)
- 4 short answer (expecting 1-2 sentence responses)

For each question, include:
- A unique short id (e.g. "q1", "q2")
- The question text
- The correct answer
- A brief explanation of why
- Which note(s) the question draws from
- Difficulty: "easy", "medium", or "hard"
- Difficulty score: 0.0-1.0 (for FSRS initialization)

Questions should test understanding, not memorization. Include questions that
require connecting ideas across multiple notes.

Respond as JSON array:
[{
  "id": "q1",
  "type": "multiple_choice" | "short_answer",
  "question": "...",
  "choices": ["A", "B", "C", "D"] | null,
  "correct_answer": "...",
  "explanation": "...",
  "source_notes": ["note title 1", "note title 2"],
  "difficulty": "easy" | "medium" | "hard",
  "difficulty_score": 0.35
}]

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 3b: Scenario Challenge (fires after quiz generation)

```
You are a scenario designer for applied learning. Based on the user's knowledge
network, create ONE realistic scenario that requires applying concepts from
multiple notes to solve.

The scenario should:
- Present a concrete, realistic situation
- Require reasoning across 2-3 different notes/topics
- Include 2-3 decision points
- Have a non-obvious best approach

Return JSON:
{
  "title": "Short scenario title",
  "situation": "2-3 paragraph setup describing the scenario",
  "questions": [
    "What trade-off is being proposed?",
    "Under what conditions would you agree?",
    "What alternative would you suggest?"
  ],
  "model_answer": "3-4 paragraph ideal response with reasoning",
  "source_notes": ["note title 1", "note title 2"],
  "difficulty_score": 0.7
}

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 4: Concept Map

```
You are a concept mapping specialist. Create a Mermaid mindmap diagram showing
how concepts connect across the user's note cluster.

Rules:
- Use Mermaid `mindmap` syntax exactly
- Root node = the current note's title wrapped in double parens: root((Title))
- Branch into major themes/concepts
- Show connections to related notes by name
- Max 4 levels deep, max 5-6 branches per node
- Max 6 words per node label
- Use clean, short labels (no full sentences)

If you cannot generate valid Mermaid syntax, return a clean indented text outline
instead, prefixed with "FALLBACK:" on the first line.

Example format:
mindmap
  root((Machine Learning Notes))
    Supervised Learning
      Regression
      Classification
    Neural Networks
      Deep Learning Architectures
      Transformer Models
    Applications
      NLP
      Computer Vision

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

### Tab 5: Perspectives (NEW)

```
You are simulating {persona_count} expert perspectives analyzing a user's
knowledge network. Each persona has a distinct viewpoint and expertise.

For each persona below, write a 2-3 paragraph analysis from their perspective.
They should:
- Engage directly with the content (not just summarize)
- Identify what's strong and what's weak from their viewpoint
- Offer specific recommendations or challenges
- Disagree with each other where appropriate

{persona_blocks}

Format as Markdown:
## {Persona Name} — {Role}
*{One-line perspective summary}*

{2-3 paragraphs of analysis}

**Key recommendation:** {one actionable suggestion}

---

Repeat for each persona.

--- BEGIN KNOWLEDGE CONTEXT ---
{assembled_context}
--- END KNOWLEDGE CONTEXT ---
```

Where `{persona_blocks}` is dynamically assembled from the selected personas:

```
### Persona 1: {name}
Role: {role}
Expertise: {expertise}
Perspective: {perspective}
Tone: {tone}
```

### Query Decomposition (InsightForge)

```
Given this note title and the first 300 characters of its body, generate 3-5
semantic search queries that would retrieve different dimensions of relevant
knowledge from a personal knowledge base.

Each query should target a different angle — e.g., foundational concepts,
practical applications, related theories, common misconceptions, or historical context.

Note title: "{title}"
Content preview: "{body_preview}"

Return JSON: { "queries": ["query1", "query2", "query3"] }
```

---

## 10. Backend Handler

### Streaming IPC Contract

Tauri `#[tauri::command]` functions cannot stream and return a final value in one call. Following the existing `chat_send` pattern:

1. The `note_insight_review` command returns an initial `InsightReviewStarted { insightReviewId, contentHash, cached: bool, previousVersion: Option<u32> }` immediately.
2. If `cached: true`, the frontend calls `note_insight_cache_get` to load all tabs instantly.
3. If `cached: false`, the backend spawns a background task that emits Tauri events:
   - **`insight:synthesis-chunk`** `{ insightReviewId, chunk: string }` — streaming tokens for Tab 1
   - **`insight:synthesis-done`** `{ insightReviewId, content: string }` — Tab 1 complete
   - **`insight:tab-done`** `{ insightReviewId, tab: string, content: string }` — Tabs 2-5 complete (one event per tab)
   - **`insight:error`** `{ insightReviewId, tab: string, error: string }` — per-tab error
   - **`insight:changes-summary`** `{ insightReviewId, summary: string }` — cross-session diff (async, arrives after tabs start)
4. Frontend listens for these events via `listen('insight:*')` and updates per-tab state.

### Layer Boundary: FlashcardRow → FlashcardResponse DTO

`FlashcardRow` lives in `crates/cognitive/src/repos/flashcard.rs` (L5). It cannot appear in `desktop-shared` (L7) IPC types. Instead, define a `FlashcardResponse` DTO in `desktop-shared` and map `FlashcardRow → FlashcardResponse` in the `app-core` handler layer (same pattern as `NoteRow → NoteResponse`).

Same boundary applies for `PersonaRow → PersonaResponse`.

### File: `crates/app-core/src/handlers/notes/insight.rs`

```rust
impl AppCore {
    /// Start insight review: check cache, spawn LLM tasks, return initial response
    pub async fn note_insight_review(&self, note_id: &str) -> Result<InsightReviewStarted, ApiError> {
        // 1. Load note + compute content_hash (SHA-256 of note.body + sorted related note IDs)
        // 2. Check insight_review_cache by (note_id, content_hash)
        // 3. If cache hit → return InsightReviewStarted { cached: true, ... }
        // 4. Else:
        //    a. Run InsightForge query decomposition (fast LLM call)
        //    b. Parallel multi-source retrieval for each sub-query
        //    c. Merge + deduplicate + apply token budget
        //    d. Select personas (check pins → domain match → auto-generate if needed)
        //    e. Generate insight_review_id (nanoid)
        //    f. Check for previous version (for cross-session tracking)
        //    g. Spawn background task:
        //       - Stream Tab 1 (Synthesis) via Tauri events (insight:synthesis-chunk)
        //       - Fire Tabs 2-5 in parallel (structured JSON mode)
        //       - Emit insight:tab-done for each completed tab
        //       - Fire cross-session diff (if previous version exists)
        //       - Cache all results with version++
        //    h. Return InsightReviewStarted { cached: false, insightReviewId, contentHash, previousVersion }
    }

    /// Regenerate a single tab (uses cached context, only re-runs one prompt)
    pub async fn note_insight_regenerate_tab(
        &self,
        note_id: &str,
        tab: InsightTab,
    ) -> Result<TabContent, ApiError> {
        // Re-run single LLM call, update cache for that tab only
    }

    /// Save quiz questions as flashcards with FSRS init
    pub async fn insight_save_flashcards(
        &self,
        note_id: &str,
        insight_review_id: &str,
        deck_name: &str,
        questions: Vec<QuizQuestion>,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        // Map difficulty_score to initial FSRS stability:
        //   easy (< 0.33) → 4.0 days
        //   medium (0.33-0.66) → 2.0 days
        //   hard (> 0.66) → 0.8 days
        // Batch insert via FlashcardRepo
        // Convert FlashcardRow → FlashcardResponse for IPC
    }

    /// Get cached insight review (for instant re-open)
    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>, ApiError> {
        // Load most recent cache entry for this note
    }

    /// Create a starter note from a knowledge gap
    pub async fn insight_create_deep_dive_note(
        &self,
        note_id: &str,
        gap: GapDescription,
    ) -> Result<NoteResponse, ApiError> {
        // 1. LLM generates title + body + tags from gap description
        // 2. Create note via existing note_create handler
        // 3. Auto-link to source notes
        // 4. Return created note
    }

    /// Persona management
    pub async fn insight_list_personas(&self) -> Result<Vec<PersonaResponse>, ApiError>;
    pub async fn insight_create_persona(&self, persona: NewPersonaParams) -> Result<PersonaResponse, ApiError>;
    pub async fn insight_update_persona(&self, id: &str, updates: PersonaUpdateParams) -> Result<PersonaResponse, ApiError>;
    pub async fn insight_delete_persona(&self, id: &str) -> Result<(), ApiError>;
    pub async fn insight_toggle_persona(&self, id: &str, active: bool) -> Result<(), ApiError>;
    pub async fn insight_set_persona_pins(&self, note_id: &str, persona_ids: Vec<String>) -> Result<(), ApiError>;
    pub async fn insight_rate_persona(&self, id: &str, helpful: bool) -> Result<(), ApiError>;
}
```

### IPC Commands

All commands must be added to `tauri::generate_handler!` in `crates/desktop/src/main.rs` and to `DEV_COMMANDS` + `dispatch_dev` in `crates/desktop/src/commands/notes.rs` (enforced by the `dev_server_covers_all_tauri_commands` test).

| Command | Args | Returns |
|---------|------|---------|
| `note_insight_review` | `{ noteId }` | `InsightReviewStarted` (then streams via Tauri events) |
| `note_insight_regenerate_tab` | `{ noteId, tab }` | `TabContent` |
| `note_insight_save_flashcards` | `{ noteId, insightReviewId, deckName, questions }` | `Vec<FlashcardResponse>` |
| `note_insight_cache_get` | `{ noteId }` | `Option<InsightReviewResponse>` |
| `note_insight_deep_dive` | `{ noteId, gap }` | `NoteResponse` |
| `note_insight_list_personas` | `{}` | `Vec<PersonaResponse>` |
| `note_insight_create_persona` | `NewPersonaParams` | `PersonaResponse` |
| `note_insight_update_persona` | `{ id, ...updates }` | `PersonaResponse` |
| `note_insight_delete_persona` | `{ id }` | `()` |
| `note_insight_toggle_persona` | `{ id, active }` | `()` |
| `note_insight_set_pins` | `{ noteId, personaIds }` | `()` |
| `note_insight_rate_persona` | `{ id, helpful }` | `()` |

### Tauri Event Names
| Event | Payload | Direction |
|-------|---------|-----------|
| `insight:synthesis-chunk` | `{ insightReviewId, chunk }` | Backend → Frontend |
| `insight:synthesis-done` | `{ insightReviewId, content }` | Backend → Frontend |
| `insight:tab-done` | `{ insightReviewId, tab, content }` | Backend → Frontend |
| `insight:error` | `{ insightReviewId, tab, error }` | Backend → Frontend |
| `insight:changes-summary` | `{ insightReviewId, summary }` | Backend → Frontend |

---

## 11. Frontend Components

### Component Tree

```
KnowledgeBasePage
  └── ContextPanel (width transitions: ~280px ↔ 640px)
        ├── [Normal mode] AISuggestionsPanel, BacklinksPanel, GraphMinimap, MoreSection
        └── [Insight mode] InsightReviewPanel
              ├── InsightHeader (title, close X, global regenerate, "What's Changed" banner)
              ├── InsightTabs (5 tab buttons + per-tab regenerate icons + status dots)
              ├── InsightContent (scrollable, switches by active tab)
              │     ├── SynthesisTab → MarkdownContent (streaming)
              │     ├── GapAnalysisTab → MarkdownContent + DeepDiveButtons
              │     │     └── DeepDiveButton (per gap, creates starter note)
              │     ├── SelfAssessmentTab → QuizRenderer + ScenarioChallenge
              │     │     ├── QuizCard (per question, with reveal)
              │     │     ├── QuizScore (running tally)
              │     │     ├── RevealAllButton (after 3+ attempts)
              │     │     └── ScenarioChallenge (after 50%+ answered)
              │     ├── ConceptMapTab → MermaidRenderer (with text fallback)
              │     │     └── CopyMermaidButton
              │     └── PerspectivesTab (NEW)
              │           ├── PersonaSelector (swap/manage personas)
              │           ├── PersonaCard (per persona: avatar, analysis, actions)
              │           │     ├── FollowUpChat (mini 1-3 exchange chat)
              │           │     ├── SwapPersonaButton
              │           │     └── RateButton (thumbs up/down)
              │           └── ManagePersonasModal
              │                 ├── PersonaList (all personas with toggle/edit/delete)
              │                 └── CreatePersonaForm
              └── InsightFooter (action buttons, always visible)
```

### Files to Create

| File | Purpose |
|------|---------|
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Main panel: header, tabs, content router, footer |
| `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx` | Streaming markdown renderer |
| `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx` | Gap analysis markdown + deep dive buttons |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx` | Interactive quiz with cards + scenario |
| `desktop-ui/src/features/notes/components/insight/ScenarioChallenge.tsx` | Applied scenario problem |
| `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx` | Mermaid renderer + fallback |
| `desktop-ui/src/features/notes/components/insight/MermaidRenderer.tsx` | Mermaid.js wrapper (dark theme) |
| `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx` | Persona-based analysis panel |
| `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx` | Individual persona analysis + actions |
| `desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx` | Swap personas, open manage modal |
| `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx` | CRUD for personas |
| `desktop-ui/src/features/notes/components/insight/ChangesBanner.tsx` | Cross-session "What's Changed" |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | State management, IPC calls, streaming |
| `desktop-ui/src/features/notes/hooks/usePersonas.ts` | Persona CRUD, selection, pinning |

### State Shape (`useInsightReview.ts`)

```typescript
interface InsightReviewState {
  isOpen: boolean;
  noteId: string | null;
  insightReviewId: string | null;
  contentHash: string | null;
  activeTab: 'synthesis' | 'gaps' | 'assessment' | 'concept-map' | 'perspectives';
  tabs: {
    synthesis: { status: TabStatus; content: string };
    gaps: { status: TabStatus; content: string; gaps: GapItem[] };
    assessment: { status: TabStatus; questions: QuizQuestion[]; scenario: ScenarioChallenge | null };
    conceptMap: { status: TabStatus; mermaid: string; fallbackText: string };
    perspectives: { status: TabStatus; personas: PersonaAnalysis[]; selectedPersonaIds: string[] };
  };
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
  changesSummary: string | null; // Cross-session diff banner
}

type TabStatus = 'idle' | 'streaming' | 'loading' | 'done' | 'error';

interface GapItem {
  id: string;
  title: string;
  type: 'missing_concept' | 'contradiction' | 'shallow_coverage';
  description: string;
  sourceNotes: string[];
  priority: 'high' | 'medium' | 'low';
}

interface PersonaAnalysis {
  personaId: string;
  name: string;
  role: string;
  icon: string;
  content: string; // Markdown analysis
  followUpMessages: { role: 'user' | 'assistant'; content: string }[];
}

interface ScenarioChallenge {
  title: string;
  situation: string;
  questions: string[];
  modelAnswer: string;
  sourceNotes: string[];
  userResponse: string;
  revealed: boolean;
}
```

### Panel Transition

In `ContextPanel.tsx`, the width is driven by a prop from `KnowledgeBasePage`:
- `insightOpen ? 640 : defaultContextWidth`
- CSS: `transition-[width] duration-250 ease-out`
- Content swaps between normal context sections and `InsightReviewPanel`

### Tab Status Indicators
- **Idle:** dim dot
- **Loading/Streaming:** pulsing purple dot (`animate-pulse` + `--purple`)
- **Done:** solid green dot (`--success`)
- **Error:** red dot (`--destructive`) with retry

### Footer Actions

| Button | Icon | Condition | Behavior |
|--------|------|-----------|----------|
| Insert into note | `FileInput` | Always enabled | Appends `## Insight Review — {date}` + active tab content |
| Create note | `FilePlus` | Always enabled | Creates "Insight: {Note Title}", auto-links source notes |
| Save as Deck | `BookOpen` | Enabled after 50%+ quiz answered. Brand pulse when ready. | Saves flashcards with FSRS init |
| Copy | `Copy` | Always enabled | Copies active tab content (markdown or mermaid source) |

### Keyboard Shortcuts
- `Cmd+Shift+I` — toggle open/close (local keyboard shortcut in KnowledgeBasePage, NOT a global Tauri shortcut — does not conflict with existing global shortcuts like `Cmd+Shift+C`)
- `1/2/3/4/5` — switch tabs (when panel focused)
- `Escape` — close panel
- `Cmd+Shift+R` — regenerate active tab

---

## 12. Styling

All components use the existing glassmorphism design system:
- Panel background: `glass-panel` class
- Inner sections: `bg-white/[0.04]` with `border-border` separators
- Tab bar: `bg-white/[0.06]` active state with `border-b border-border` separator
- Quiz cards: `glass-card` styling
- Persona cards: `glass-card` with `border-l-2` colored by persona tone
- Footer: `border-t border-border` with `glass-button` styled actions
- Status dots: CSS custom properties (`--purple`, `--success`, `--destructive`)
- Mermaid: dark theme with `--brand` accent color integration
- Changes banner: `bg-brand/10` with `border-brand/20` and `text-brand` text
- All text follows hierarchy: `text-primary`, `text-secondary`, `text-muted`, `text-dim`

### Persona Tone Colors
| Tone | Left border color | Icon background |
|------|-------------------|-----------------|
| skeptical | `--destructive` (red) | `bg-destructive/10` |
| pragmatic | `--warning` (amber) | `bg-warning/10` |
| curious | `--brand` (purple) | `bg-brand/10` |
| inquisitive | `--info` (blue) | `bg-info/10` |
| analytical | `--success` (green) | `bg-success/10` |
| provocative | `--orange` | `bg-orange/10` |

---

## 13. Phased Implementation

### Phase 1: Core (Build First)
- Tabs 1-4 (Synthesis, Gap Analysis, Self-Assessment, Concept Map) — original design
- InsightForge context assembly (query decomposition + multi-source retrieval)
- Deep Dive note generation (Gap Analysis buttons)
- Flashcard saving with FSRS
- Caching + cache invalidation

### Phase 2: Perspectives + Polish
- Tab 5 (Perspectives) with pluggable persona system
- Persona registry (builtin + auto-generated + user-created)
- Persona management UI
- Cross-session insight tracking ("What's Changed" banner)
- Scenario challenges (Self-Assessment extension)

### Phase 3: Evolution (Future)
- Temporal knowledge tracking (how understanding evolves over time)
- Persona auto-generation improvements (learn from user ratings)
- Knowledge growth metrics (dashboard showing notes created from gaps, quiz performance trends)
- Persona follow-up chat (mini conversations with individual personas)

---

## 14. New & Modified Files

### Backend (new files)

| File | Crate | Purpose |
|------|-------|---------|
| `crates/cognitive/src/repos/flashcard.rs` | cognitive | FlashcardRepo + FlashcardRow + NewFlashcard + ReviewQuality types |
| `crates/cognitive/src/repos/insight_cache.rs` | cognitive | InsightCacheRepo with versioned caching |
| `crates/cognitive/src/repos/persona.rs` | cognitive | PersonaRepo + PersonaRow + persona selection logic |
| `crates/app-core/src/handlers/notes/insight.rs` | app-core | Insight Review handler (InsightForge context, LLM calls, caching, streaming, personas) |

### Backend (modified files)

| File | Crate | Change |
|------|-------|--------|
| `crates/cognitive/src/repos/mod.rs` | cognitive | Register `flashcard`, `insight_cache`, `persona` modules |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | cognitive | Append `flashcards` + `insight_review_cache` + `insight_personas` + `insight_persona_pins` DDL, bump version |
| `crates/desktop-shared/src/commands/notes.rs` | desktop-shared | Add all DTOs (InsightReviewStarted, InsightReviewResponse, TabContent, QuizQuestion, FlashcardResponse, PersonaResponse, GapDescription, ScenarioChallenge) |
| `crates/desktop/src/commands/notes.rs` | desktop | Add IPC commands + update `DEV_COMMANDS` + `dispatch_dev` |
| `crates/desktop/src/main.rs` | desktop | Register all new commands in `tauri::generate_handler!` |
| `crates/app-core/src/handlers/notes/mod.rs` | app-core | Register `insight` module |

---

## 15. Dependencies

### New (frontend)
- `mermaid` — Mermaid.js for concept map rendering (~200KB, note: not effectively tree-shakeable, full bundle includes all diagram parsers)

### Existing (no changes needed)
- `react-markdown` + `remark-gfm` + `rehype-highlight` — already in chat feature
- `lucide-react` — already used throughout
- `nanoid` — already available for ID generation
- `sha256` — already available (used by note suggestions)
