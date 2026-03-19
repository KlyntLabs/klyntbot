# Language Learning Surface — Design Spec

> Transform the Translation split mode into an AI-powered language learning workspace. Serves three personas: students reading foreign-language content, dedicated language learners (HSK/IELTS prep), and bilingual knowledge workers who need fast translation.

## Personas

| Persona | Primary Need | Usage Pattern |
|---------|-------------|---------------|
| **A — Student** | Understand foreign content + build vocabulary | Read textbooks/papers in Chinese/English, needs meaning + vocab extraction |
| **B — Language Learner** | Deep drill-down on grammar, practice, contrastive pairs | Actively studying (HSK/IELTS), takes grammar notes, does translation practice |
| **C — Knowledge Worker** | Zero-friction translation | Works across languages daily, translate fast and move on, save vocab only when needed |

## Architecture Overview

The Language Learning Surface is NOT a new mode — it **upgrades the existing Translation split mode** with AI-powered features. The right pane transforms from a plain TipTap editor into a stacked-sections panel with progressive disclosure.

### Entry Points

1. **Right-click context menu → "Translate"** — opens Translation split mode with AI panel
2. **SplitToolbar → "Translate" button** — same as above
3. **Keyboard shortcut** — configurable (default: none, user can bind)
4. **Annotation enrichment** — automatic language enrichment on annotation cards when foreign text is detected

### System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ NoteEditor                                                   │
│  ┌──────────────────────┬──────────────────────────────────┐ │
│  │ Left Pane (Editor)   │ Right Pane (Language Panel)      │ │
│  │                      │  ┌─────────────────────────────┐ │ │
│  │ Source text with      │  │ § Translation (expanded)    │ │ │
│  │ annotation highlights │  │ § Words (expanded)          │ │ │
│  │                      │  │ § Grammar (collapsed)       │ │ │
│  │                      │  │ § Practice (collapsed)      │ │ │
│  │                      │  │ § Confusable Alerts (cond.) │ │ │
│  │                      │  └─────────────────────────────┘ │ │
│  └──────────────────────┴──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## Language Configuration

### Global Default (Settings > Language Learning)

```
Primary Learning Pair
  Source: [Chinese     ▾]
  Target: [English     ▾]
```

- Stored in `config.json` under `language.sourceLang` / `language.targetLang` (note: the key is `language`, NOT `learning` — the existing `learning` key is taken by `LearningConfig` for adaptive confidence thresholds)
- Applies to all notes by default
- First-time behavior: auto-detect on first Translation use → show banner: "Detected Chinese → English. [Set as default]"

### Per-Note Override

- Small picker in the Translation pane header: `中文 → English ▾`
- Click opens quick menu: top 3 recent pairs + full list
- Override stored in `note.perspectiveConfig` JSON (reuse existing field)
- Label: "Using global pair • Override for this note"

### Detection Logic

1. If per-note override exists → use it
2. If global pair is set → use it
3. If neither → auto-detect source language via a lightweight heuristic first (Unicode script detection: CJK = Chinese, Latin = English/etc.), falling back to LLM only for ambiguous scripts. Show a loading skeleton in the Translation section while detecting. If detection fails (LLM error/timeout), default to English → user's system locale and show a banner "Could not detect language. [Set manually]"
4. Short text (<20 chars): use global pair, show hint "Detected: Chinese → English"

---

## Right Pane: Stacked Sections Layout

When Translation split mode is active, the right pane renders a `LanguageLearningPanel` instead of a plain editor. Five sections in fixed order, each collapsible.

### Section 1: Translation (Always Expanded)

**Purpose:** AI translation of the full left-pane content or selected text.

**Content:**
- Full translation in a bordered block (brand orange left border)
- "Manual mode" toggle in top-right corner (collapses AI panel, shows plain editor — for Persona C)

**Behavior:**
- Translates on mode entry (full left pane content)
- Re-translates when left pane content changes (debounced 2s)
- If text is selected in the left pane when entering mode → translate selection only
- Streaming: use `cognitive_provider.chat_stream()` with an event emitter loop (same pattern as `insight.rs`). The handler spawns a `tokio::spawn` task, reads stream chunks, and emits `language:translation-chunk` events via `AppEventEmitter`. Frontend listens via `useEvent("language:translation-chunk")` in Tauri mode, or `EventSource` bridging in browser dev mode (add to `useInsightSSE.ts` bridge list). The command returns a `{ requestId: String }` immediately — streaming happens asynchronously. A `language:translation-done` event signals completion with the full `TranslateBreakdownResponse` JSON. On error, emit `language:translation-error`.

### Section 2: Words (Always Expanded)

**Purpose:** Word-by-word breakdown with readings, meanings, and proficiency levels.

**Content:**
- Vertical list of extracted words, each row:
  ```
  使用 (shǐyòng)              uses           [HSK 3]
  逐步 (zhúbù)                progressively  [HSK 5]  ★ new
  提取 (tíqǔ)                 extract        [HSK 6]  ★ new
  ```
- "★ new" badge on words not yet in the user's SemanticFact vocabulary (the backend checks this via `SemanticFactRepo` query with `scope_type = "system"` + `domain = "learning"` + `memory_type = "vocabulary"` + exact `subject` match — single-user app, no user_id needed)
- Each word row is clickable → expands to show: example sentence from the note, additional example from LLM, part of speech

**Actions:**
- **"Save N new words"** button (brand orange) — batch-creates:
  - `Flashcard` entries (type: Vocabulary, with `vocab_data: { word, reading, meaning, example_sentence, part_of_speech }`)
  - `SemanticFact` entries (domain: "learning", memory_type: "vocabulary", subject: word, predicate: "meaning", object: translation)
- Individual word save via click on the word's "+" icon

**Chinese-specific:**
- Pinyin annotation with tone coloring (tone 1 red, 2 orange, 3 green, 4 blue, neutral gray)
- HSK level badges (1-6, color-coded by difficulty band)

**English-specific:**
- IPA pronunciation
- CEFR level badges (A1-C2)
- Collocation hints on hover

### Section 3: Grammar Pattern (Collapsed by Default)

**Purpose:** Identify and explain grammar structures in the source text.

**Content:**
- Pattern template: `[Subject] + 使用 + [Object] + 来 + [Purpose verb phrase]`
- Plain-language explanation: "This is a purpose clause pattern: 'X uses Y to Z'"
- Related patterns from user's ProceduralRule memory (if any)

**Chinese-specific:**
- 把-construction, 被-construction, 是...的 pattern identification
- Measure word suggestions when nouns are detected

**English-specific:**
- Phrasal verb detection and explanation
- Conditional/subjunctive pattern identification

**Auto-expand rule:** If the source text is a single sentence (short text), auto-expand this section.

### Section 4: Practice (Collapsed by Default)

**Purpose:** User writes their own translation and receives AI evaluation.

**Collapsed state:** Shows only a grade summary bar if a previous attempt exists: `B+ C D+ B`

**Expanded state:**

1. **Input area** — textarea where user writes their translation
2. **"Check My Translation" button** — triggers LLM evaluation
3. **Results** — Hybrid format (Letter Grades + Expandable):

```
┌─────────────────────────────────────┐
│  [A-]      [C+]      [D+]     [B]  │
│ Meaning  Grammar  Natural  Choice   │
├─────────────────────────────────────┤
│ ✗ "do extraction" → "extract"      │
│   ▸ Why?                           │
│ ✗ "step by step" → "progressively" │
│   ▾ Why?                           │
│   │ 逐步 carries a formal register.│
│   │ "progressively" matches the    │
│   │ academic tone better.          │
│ ✓ "multiple layers" — accurate     │
├─────────────────────────────────────┤
│ [Try again (with hint)]            │
│ [Show model translation]           │
│ [Save corrections as flashcards]   │
└─────────────────────────────────────┘
```

**4 evaluation dimensions:**
- **Meaning** — semantic accuracy of the translation
- **Grammar** — structural correctness in the target language
- **Naturalness** — how native it sounds (collocations, phrasing)
- **Word Choice** — vocabulary precision + register/formality match

**Letter grade scale:** A+ through F, color-coded (green A-B, orange C, red D-F)

**"Why?" expanders:**
- Click any correction → shows explanation with:
  - Specific linguistic reason
  - Alternative suggestion
  - Example from user's SemanticFact vocabulary (if available)
  - Link to grammar pattern (if applicable)
- Expanders can stack (open multiple simultaneously)

**Action buttons (tiered):**
1. "Try again (with hint)" — shows one keyword clue, user rewrites
2. "Show model translation" — reveals AI's full translation (only after at least one attempt)
3. "Save corrections as flashcards" — generates Vocabulary flashcards from each correction, with the original sentence as `source_context`

**Edge cases:**
- Perfect translation: All A+ → message "Perfect match!" + "Save as model sentence" button
- Very short text: auto-expand Practice section
- No clear mistakes: hide expanders, show only grades

### Section 5: Confusable Alerts (Conditional)

**Purpose:** Flag semantically similar words already in the user's vocabulary.

**Only shown when:** a word in the current breakdown has a near-synonym in the user's SemanticFact store.

**Content:**
```
⚠ Confusable Pair
  特征 (tèzhēng) = features/characteristics
  特点 (tèdiǎn) = characteristics/traits [in your vocabulary]

  Key difference: 特征 emphasizes distinguishing markers;
  特点 emphasizes notable qualities.

  [Create contrastive flashcard pair]
```

**Detection:** Query `SemanticFactRepo` using exact `subject` field match (NOT `search_fts` — FTS5's default tokenizer does not handle CJK word boundaries correctly). Use a dedicated query: `SELECT * FROM semantic_facts WHERE domain = 'learning' AND memory_type = 'vocabulary' AND subject LIKE ?1` with prefix matching, or add a new repo method `find_by_subject_prefix(word, domain)`. If a semantically similar word is found, trigger an LLM call to explain the difference.

**Action:** "Create contrastive flashcard pair" generates two linked Vocabulary flashcards with the difference as `source_context`.

---

## Annotation Sidebar: Smart Language Enrichment

When in Annotate split mode, annotation cards are automatically enriched with language data **if the quoted text contains foreign-language content** (Smart Detection).

### Detection Logic

1. Compare annotation's `quotedText` against the user's target language (from global pair or per-note override)
2. If the text is in a different language than the user's target → enrich
3. If the text matches the user's native/target language → plain card (no enrichment)
4. Code-switching (mixed languages): enrich only the foreign-language portions

### Enriched Annotation Card Layout

When enrichment is triggered, the annotation card in `AnnotationSidebar` shows additional sections:

1. **Quoted text with inline pinyin/IPA** (superscript above each word)
2. **Quick translation** (one-line, gray background)
3. **Word chips** with proficiency levels (HSK/CEFR badges)
4. **"Save N new words"** action button (brand orange)
5. **Grammar badge** (if a pattern is detected) → click opens mini-expander inline
6. **User's comment/note area** (unchanged from current design)
7. **Delete button** (unchanged)

### Enrichment Data Flow

1. On annotation creation with foreign text:
   - Detect language of `quotedText`
   - If foreign → make one LLM call for translation + word breakdown
   - Cache the enrichment data in the annotation's `ai_suggestion` field as JSON
2. On subsequent loads: check `ai_suggestion` field first. If non-null and valid JSON with an `enrichment` key → use cached data, NO LLM call. This prevents thundering-herd on notes with 10+ annotations.
3. Cache miss = `ai_suggestion` is null OR doesn't contain `enrichment` key → trigger enrichment
4. Frontend loads annotations, then enriches only those with cache miss in a **sequential queue** (not parallel) — max 1 concurrent LLM call for enrichment
5. User can force-refresh enrichment via a small "↻" button (clears `ai_suggestion` and re-triggers)
6. Error handling: if LLM call fails, show the annotation card without enrichment (plain mode). Don't retry automatically.

---

## Backend Architecture

### New Handler File

`crates/app-core/src/handlers/notes/language.rs`

All methods on `AppCore`, using `cognitive_provider.chat()` pattern (same as `card_generation.rs`).

#### Methods

| Method | Input | Output | LLM? |
|--------|-------|--------|------|
| `language_translate_breakdown` | text, source_lang, target_lang | `TranslateBreakdownResponse` | Yes |
| `language_evaluate_translation` | source_text, user_translation, source_lang, target_lang | `TranslationEvalResponse` | Yes |
| `language_extract_vocabulary` | text, source_lang, target_lang, user_level? | `VocabularyExtractionResponse` | Yes |
| `language_detect_confusables` | word, source_lang, note_id | `ConfusableResponse` | Yes (if match found) |
| `language_enrich_annotation` | annotation_id, quoted_text, source_lang, target_lang | `AnnotationEnrichmentResponse` | Yes |
| `language_save_vocabulary` | words: Vec<VocabItem>, note_id, deck | `Vec<FlashcardResponse>` (full card data for UI) | No |

#### Response Types

All shared IPC types live in `crates/desktop-shared/src/commands/language.rs` (following the existing pattern — NOT in `app-core`). All types derive `Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`.

```rust
struct TranslateBreakdownResponse {
    translation: String,
    words: Vec<WordBreakdown>,
    grammar_patterns: Vec<GrammarPattern>,
}

struct WordBreakdown {
    word: String,
    reading: Option<String>,       // pinyin, IPA, etc.
    meaning: String,
    part_of_speech: String,
    proficiency_level: Option<String>,  // "HSK 3", "CEFR B1"
    example_sentence: Option<String>,
    is_new: bool,                  // not in user's SemanticFacts
}

struct GrammarPattern {
    pattern: String,               // "[S] + 使用 + [O] + 来 + [Purpose]"
    explanation: String,
    pattern_type: Option<String>,  // "purpose clause", "把-construction"
}

struct TranslationEvalResponse {
    grades: EvalGrades,
    corrections: Vec<Correction>,
    model_translation: String,
}

struct EvalGrades {
    meaning: String,      // "A-", "B+", "C", etc.
    grammar: String,
    naturalness: String,
    word_choice: String,
}

struct Correction {
    original: String,
    suggested: String,
    explanation: String,
    category: String,     // "grammar", "vocabulary", "register", "naturalness"
}
```

### Prompt Files

`crates/app-core/src/handlers/notes/language_prompts.rs`

Following the `insight_prompts.rs` pattern: pure functions returning `(system_prompt, user_prompt)` pairs.

Prompts:
- `build_translate_breakdown_prompt(text, source_lang, target_lang, user_level)`
- `build_evaluate_translation_prompt(source, user_translation, source_lang, target_lang)`
- `build_detect_confusables_prompt(word, existing_similar_words, source_lang)`
- `build_enrich_annotation_prompt(quoted_text, source_lang, target_lang)`

Each function returns a system prompt `String`. The user prompt is constructed inline in the handler (same pattern as `card_generation.rs` — the system prompt is static, the user prompt varies per call). All prompts instruct the LLM to return structured JSON (matching the response types above). The parser uses `common::strip_llm_fences()` for robustness.

**Correction flashcard format** (from Practice section "Save corrections"):
- Front: `"How do you say '逐步' in academic English?"` (source word + register hint)
- Back: `"progressively / incrementally (formal register, not 'step by step')"` (correct form + explanation)
- `source_context`: the full original sentence from the note
- `card_type`: "vocabulary"

### Vocabulary Persistence

When "Save N new words" is clicked:

1. **Flashcards** — created via `FlashcardRepo::create_batch()`:
   - `card_type: "vocabulary"`
   - `vocab_data: { word, reading, meaning, example_sentence, part_of_speech }`
   - `source_note_id: note.id`
   - `source_context: original_sentence`
   - `deck: note.title` (or user-selected deck)
   - FSRS-5 scheduling starts immediately (due_at = now)

2. **SemanticFacts** — created via `SemanticFactRepo::upsert()`:
   - `domain: "learning"`
   - `memory_type: "vocabulary"`
   - `subject: word` (e.g., "逐步")
   - `predicate: "meaning"`
   - `object: translation` (e.g., "progressively")
   - `scope_type: "system"`
   - `source: "note:{note_id}"`

3. **Confusable pairs** — if detected, create two linked flashcards with:
   - Front: "特征 vs 特点 — what's the difference?"
   - Back: explanation of the distinction
   - Tags: `["confusable", "chinese", source_lang]`

### Tauri IPC Commands

New commands in `crates/desktop/src/commands/language.rs`:

- `language_translate_breakdown`
- `language_evaluate_translation`
- `language_extract_vocabulary`
- `language_detect_confusables`
- `language_enrich_annotation`
- `language_save_vocabulary`

Following the existing pattern: `State<'_, Arc<AppCore>>` + `ApiError`.

**Mandatory dev server integration (per CLAUDE.md):**
- Export `pub const DEV_COMMANDS: &[&str]` listing all 6 command names
- Implement `pub(crate) async fn dispatch_dev(cmd, core, body)` for browser dev mode
- Register in `crates/desktop/src/dev_server/mod.rs` → add `commands::language::DEV_COMMANDS` to the modules array
- Register in `crates/desktop/src/dev_server/dispatch.rs` → add `commands::language::dispatch_dev` chain
- Register all 6 commands in `crates/desktop/src/main.rs` invoke handler
- The `dev_server_covers_all_tauri_commands` test will fail if any of these steps is missed

**Note:** `language.rs` in `desktop/src/commands/` contains ONLY thin `#[tauri::command]` shims delegating to `AppCore` methods — no business logic. This is consistent with all other command files (e.g., `entity_links.rs`).

---

## Frontend Architecture

### New Components

| File | Purpose |
|------|---------|
| `LanguageLearningPanel.tsx` | Main container for the right pane in Translation mode. Renders 5 stacked sections. |
| `TranslationSection.tsx` | Always-expanded translation display with streaming support |
| `WordsSection.tsx` | Word breakdown list with chips, HSK/CEFR badges, save actions |
| `GrammarSection.tsx` | Collapsible grammar pattern display |
| `PracticeSection.tsx` | Translation input + evaluation results (Hybrid C format) |
| `ConfusableSection.tsx` | Conditional alert cards for similar words |
| `LanguagePicker.tsx` | Flag + text combo language pair selector |

### New Hooks

| Hook | Purpose |
|------|---------|
| `useLanguageBreakdown.ts` | Calls `language_translate_breakdown` IPC, manages loading/result state |
| `useTranslationPractice.ts` | Manages practice input, calls evaluation, tracks grades |
| `useLanguageConfig.ts` | Reads global pair from settings, manages per-note override |
| `useVocabularySave.ts` | Batch-saves vocabulary to flashcards + semantic facts |

### SplitEditor Modification

In `SplitEditor.tsx`, when `splitMode === "translation"`:
- Replace the right-pane `EditorContentWrapper` with `LanguageLearningPanel`
- Add "Manual mode" toggle that switches back to the plain editor
- Pass the left editor's content to the panel for translation

### Annotation Sidebar Enhancement

In `AnnotationSidebar.tsx` / `AnnotationCard`:
- After annotation data loads, check if `quotedText` contains foreign text
- If yes, call `language_enrich_annotation` IPC (cached in `ai_suggestion` field)
- Render enrichment data (pinyin, translation, word chips) below the quoted text
- Add "Save N new words" button to enriched cards

---

## Data Model Changes

### Config Schema Addition

In `crates/config/src/schema/`:

```rust
/// Config key: `language` (NOT `learning` — that's taken by `LearningConfig`).
/// Added to the root `Config` struct as: `#[serde(default)] pub language: LanguageConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfig {
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    #[serde(default = "default_true")]
    pub auto_detect: bool,
    pub proficiency_level: Option<String>, // "HSK 3", "CEFR B1", etc.
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            source_lang: None,
            target_lang: None,
            auto_detect: true,
            proficiency_level: None,
        }
    }
}
```

### No New Tables

All data fits existing structures:
- Vocabulary → `flashcards` table (card_type = "vocabulary")
- Facts → `semantic_facts` table (domain = "learning", memory_type = "vocabulary")
- Confusable pairs → `flashcards` table (tagged "confusable")
- Enrichment cache → `annotations.ai_suggestion` field (already exists, JSON)
- Language config → `config.json` (global) + `notes.perspective_config` (per-note override)

---

## UX Flows by Persona

### Persona A — Student Reading Chinese Paper

1. Opens note with Chinese content
2. Clicks "Translate" in SplitToolbar (or right-click → Translate)
3. Right pane shows AI breakdown: translation + word list with HSK levels
4. Scans words, sees 3 new ones (★ badge)
5. Clicks "Save 3 new words" → flashcards created, due for review today
6. Scrolls down, sees grammar pattern → reads explanation
7. Returns to reading, vocabulary saved for FSRS review later

### Persona B — HSK 5 Prep Student

1. Opens note with Chinese study content
2. Enters Translation mode
3. Reads the breakdown, notes the grammar pattern
4. Expands Practice section → writes their own translation
5. Clicks "Check My Translation" → sees B+ C D+ B grades
6. Expands "Why?" on the D+ (Naturalness) → reads about register
7. Clicks "Try again (with hint)" → gets keyword "progressively"
8. Rewrites → checks again → A- B+ B A- → improvement
9. Clicks "Save corrections as flashcards" → mistakes become FSRS cards
10. Confusable alert appears for 特征/特点 → creates contrastive pair

### Persona C — Bilingual Knowledge Worker

1. Opens note with mixed Chinese content
2. Clicks "Translate" → sees translation instantly
3. Scans the translation, gets the meaning
4. Clicks "Manual mode" toggle → right pane becomes plain editor
5. Types their own working notes alongside the source
6. OR: stays in AI mode, sees word list, ignores Grammar/Practice sections
7. Occasionally clicks "Save 1 new word" on an interesting term

---

## Cross-Cutting Concerns

### Error States

All LLM-powered sections follow a consistent error pattern:
- **Loading:** Skeleton placeholder (animated pulse bars, same pattern as InsightReviewPanel)
- **LLM failure** (rate limit, timeout, network): Show skeleton + "Translation failed. [Retry]" button. Don't block the entire panel — other sections that don't need LLM (e.g., cached data) remain functional.
- **Empty response:** Treat as error, show retry button.

### Save Feedback

After "Save N new words" or "Save corrections as flashcards":
- Show a snackbar/toast: "Saved X words to [deck name]" with an **Undo** button (5-second window)
- Undo deletes the recently created flashcards + semantic facts
- Snackbar auto-dismisses after 5 seconds

### Responsive Behavior

When the right pane is narrow (<280px):
- Grammar and Practice sections auto-collapse (only Translation + Words visible)
- Word chips switch to a compact single-line format
- "Save" buttons remain full-width for easy tap targets

### Accessibility

- HSK/CEFR level chips include `aria-label` (e.g., `aria-label="HSK level 3"`)
- All collapsible sections use `aria-expanded` + keyboard Enter/Space to toggle
- Word list rows are keyboard-focusable with arrow navigation
- Letter grades in Practice have screen-reader-friendly labels (e.g., "Meaning: B plus")

### Learning Telemetry (Lightweight)

Track counters in the cognitive memory system (not a new table — use SemanticFacts with `domain = "learning"`, `memory_type = "telemetry"`):
- `translation_checks` — how many times user triggered translation
- `practice_attempts` — how many translation evaluations completed
- `words_saved` — total vocabulary items saved

These enable future coaching suggestions (e.g., "You've translated 50 sentences this week — try Practice mode to deepen retention").

---

## Non-Goals (Explicitly Out of Scope)

- **Speech/pronunciation practice** — no audio recording or speech recognition
- **Real-time grammar correction while typing** — focus is on translation evaluation, not inline writing correction
- **OCR/image translation** — text-only input
- **Offline translation** — requires LLM API access
- **Custom FSRS weights per language** — single global weight set (existing behavior)
- **Character stroke order animation** — future enhancement, not in this phase
- **Spaced repetition review UI** — existing flashcard review system is sufficient

---

## Implementation Priority

| Phase | Features | Effort |
|-------|----------|--------|
| **P0** | Translation section + Words section + language config + vocabulary save pipeline | Medium |
| **P1** | Practice section (evaluation + grades + corrections) | Medium |
| **P1** | Grammar section + Confusable alerts | Low-Medium |
| **P2** | Annotation sidebar enrichment (Smart Detection) | Medium |
| **P2** | "Try again with hint" + "Show model translation" tiered practice | Low |

P0 delivers immediate value for all 3 personas. P1 adds the deep learning features Persona B needs. P2 polishes the annotation integration.
