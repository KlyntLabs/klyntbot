# Phase 3: Card Generation Pipeline + Source Linking Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build AI-powered flashcard generation from notes and free text with a preview/approval flow, source linking, and Quick Generate on the Learn dashboard.

**Architecture:** New `feature-learning` crate (L4) provides prompt templates and LLM response parsing. `app-core` handlers orchestrate: fetch note via `note_repo` → build prompt via `feature-learning` → call LLM via `cognitive_provider` → parse response → return previews to frontend. A second handler saves approved cards via `flashcard_repo.create_batch()` with `source_note_id` + `source_context` populated. Frontend adds a `CardGenerationModal` triggered from the editor toolbar and Learn dashboard's Quick Generate section.

**Tech Stack:** Rust (`feature-learning` crate, `app-core` handlers, `desktop-shared` IPC types, `desktop` Tauri commands), React + TypeScript (modal, hooks, toolbar integration), Tailwind v4 glass-card styling.

**Key patterns to follow:**
- LLM calls: `providers::cognitive_chat_params(&config, max_tokens)` → `cognitive_provider.chat(messages, None, &params)` (see `insight.rs:996`)
- IPC types: `#[serde(rename_all = "camelCase")]` in `desktop-shared/src/commands/notes.rs`
- Tauri commands: thin adapters in `desktop/src/commands/notes.rs` delegating to `AppCore` methods
- Frontend SWR: `useQuery(cmd, args, fallback)` + `invalidateQueries(prefix)`
- CSS: glass-card, glass-button, glass-panel classes. Never hardcode hex/rgba.

**Depends on:** Phase 1 (FSRS-5 engine) and Phase 2 (/learn page) — both completed.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/feature-learning/Cargo.toml` | Crate manifest — minimal deps (common, serde, serde_json, tracing) |
| `crates/feature-learning/src/lib.rs` | Module declarations + re-exports |
| `crates/feature-learning/src/types.rs` | `GeneratedCard`, `CardGenerationContext` structs |
| `crates/feature-learning/src/card_generator.rs` | Prompt template + JSON response parser |
| `crates/app-core/src/handlers/notes/card_generation.rs` | `flashcard_generate` + `flashcard_save_generated` handlers |
| `desktop-ui/src/features/notes/hooks/useCardGeneration.ts` | Generation flow state management |
| `desktop-ui/src/features/notes/components/CardGenerationModal.tsx` | Preview/approve/save modal |
| `desktop-ui/src/features/learn/components/QuickGenerate.tsx` | Note picker + clipboard generation UI |
| `desktop-ui/src/features/learn/components/NotePicker.tsx` | Searchable note selection dropdown |

### Modified files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Add `feature-learning` to `members` + `[workspace.dependencies]` |
| `crates/app-core/Cargo.toml` | Add `feature-learning` dependency |
| `crates/app-core/src/handlers/notes/mod.rs` | Add `mod card_generation;` |
| `crates/desktop-shared/src/commands/notes.rs` | Add `FlashcardGenerateParams`, `GeneratedCardPreview`, `FlashcardGenerateResponse`, `FlashcardSaveGeneratedParams` |
| `crates/desktop/src/commands/notes.rs` | Add `flashcard_generate`, `flashcard_save_generated` Tauri commands + DEV_COMMANDS |
| `crates/desktop/src/main.rs` | Add 2 commands to `generate_handler![]` |
| `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx` | Add "Generate Cards" button in mode buttons area |
| `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx` | Accept + forward `onGenerateCards` prop |
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Wire CardGenerationModal + pass generate handler to editor |
| `desktop-ui/src/features/learn/components/DashboardHome.tsx` | Replace placeholder Quick Generate card with real component |
| `desktop-ui/src/features/learn/components/ImmersiveReview.tsx` | Wire "Source" button to navigate to source note |
| `desktop-ui/src/features/learn/pages/LearnPage.tsx` | Add Quick Generate modal state |
| `crates/app-core/src/handlers/notes/insight.rs` | Fix `insight_save_flashcards` to populate `source_context` |

---

### Task 1: Create `feature-learning` crate with types and card generation logic

**Files:**
- Create: `crates/feature-learning/Cargo.toml`
- Create: `crates/feature-learning/src/lib.rs`
- Create: `crates/feature-learning/src/types.rs`
- Create: `crates/feature-learning/src/card_generator.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `crates/feature-learning/Cargo.toml`**

```toml
[package]
name = "feature-learning"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Create `crates/feature-learning/src/types.rs`**

These types represent the output of card generation (before saving to DB). They are transport-agnostic — `app-core` converts them to `FlashcardResponse` after persisting.

```rust
use serde::{Deserialize, Serialize};

/// A single card produced by the LLM card generator.
/// Deserialized from the LLM JSON response, then mapped to `NewFlashcard` for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCard {
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Vec<String>,
    pub source_context: Option<String>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
}

/// Context assembled for card generation — passed to the prompt builder.
pub struct CardGenerationContext {
    pub note_content: String,
    pub note_title: String,
    pub existing_cards_summary: Option<String>,
}
```

- [ ] **Step 3: Create `crates/feature-learning/src/card_generator.rs`**

Contains the prompt template and JSON response parser. Does NOT call the LLM — that stays in `app-core`.

```rust
use crate::types::{CardGenerationContext, GeneratedCard};
use tracing::warn;

/// Build the system + user messages for card generation.
///
/// Returns `(system_prompt, user_prompt)` — caller sends both to the LLM.
pub fn build_generation_prompt(ctx: &CardGenerationContext) -> (String, String) {
    let system = r#"You are a flashcard generation assistant for spaced repetition learning. Generate high-quality, self-contained flashcards from the provided content.

Rules:
1. Generate 5-15 cards depending on content density
2. Use varied card types:
   - "basic" for concept questions and understanding checks (most common)
   - "cloze" for definitions, key facts, and fill-in-the-blank — use {{c1::hidden}} syntax in the front field
   - "vocabulary" for foreign language words — populate vocab_data with word, reading, meaning, example_sentence, part_of_speech
3. Each card MUST be self-contained (understandable without the source note)
4. Prefer testing understanding and application over rote memorization
5. source_context is a 1-2 sentence excerpt from the note that the card tests — include it for every card
6. Tags: 1-3 lowercase hyphenated concepts (e.g., "machine-learning", "te-form", "photosynthesis")
7. For cloze cards: front contains the text with {{c1::hidden}} markers, back is the full revealed text
8. For vocabulary cards: front is the word (with reading if applicable), back is the meaning + example

Respond ONLY with a JSON array. No markdown fences, no explanation, no preamble."#.to_string();

    let mut user = String::new();

    if let Some(ref existing) = ctx.existing_cards_summary {
        user.push_str("The user already has these flashcards from this note. Do NOT generate duplicates:\n");
        user.push_str(existing);
        user.push_str("\n\n");
    }

    user.push_str(&format!("--- BEGIN NOTE: {} ---\n", ctx.note_title));
    user.push_str(&ctx.note_content);
    user.push_str("\n--- END NOTE ---");

    (system, user)
}

/// Parse the LLM response JSON into a list of generated cards.
///
/// Handles common LLM quirks: markdown fences around JSON, trailing commas (via serde),
/// and individual card validation (skips malformed cards rather than failing entirely).
pub fn parse_generated_cards(response: &str) -> Result<Vec<GeneratedCard>, String> {
    let cleaned = strip_json_fences(response);

    let cards: Vec<GeneratedCard> = serde_json::from_str(&cleaned)
        .map_err(|e| format!("Failed to parse card generation response: {e}"))?;

    // Filter out cards with empty front/back
    let valid: Vec<GeneratedCard> = cards
        .into_iter()
        .filter(|c| {
            let ok = !c.front.trim().is_empty() && !c.back.trim().is_empty();
            if !ok {
                warn!("Skipping generated card with empty front or back");
            }
            ok
        })
        .collect();

    if valid.is_empty() {
        return Err("No valid cards generated".to_string());
    }

    Ok(valid)
}

/// Build a summary of existing cards for duplicate avoidance.
///
/// Returns None if there are no existing cards.
pub fn summarize_existing_cards(cards: &[(String, String)]) -> Option<String> {
    if cards.is_empty() {
        return None;
    }

    let summary: Vec<String> = cards
        .iter()
        .take(30) // Cap at 30 to avoid prompt overflow
        .map(|(front, back)| {
            let front_truncated = truncate(front, 80);
            let back_truncated = truncate(back, 80);
            format!("- Q: {} → A: {}", front_truncated, back_truncated)
        })
        .collect();

    Some(summary.join("\n"))
}

fn strip_json_fences(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string()
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_cards() {
        let json = r#"[
            {"front": "What is photosynthesis?", "back": "The process by which plants convert light to energy", "card_type": "basic", "tags": ["biology"], "source_context": "Photosynthesis is the process...", "cloze_data": null, "vocab_data": null},
            {"front": "{{c1::Mitochondria}} is the powerhouse of the cell", "back": "Mitochondria is the powerhouse of the cell", "card_type": "cloze", "tags": ["biology", "cell"], "source_context": "The mitochondria...", "cloze_data": {"clozes": [{"index": 1, "hint": "organelle"}]}, "vocab_data": null}
        ]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].card_type, "basic");
        assert_eq!(cards[1].card_type, "cloze");
    }

    #[test]
    fn parse_with_markdown_fences() {
        let json = "```json\n[{\"front\": \"Q\", \"back\": \"A\", \"card_type\": \"basic\", \"tags\": [], \"source_context\": null, \"cloze_data\": null, \"vocab_data\": null}]\n```";
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn skips_empty_cards() {
        let json = r#"[
            {"front": "Good card", "back": "Good answer", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null},
            {"front": "", "back": "No front", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null}
        ]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn error_on_invalid_json() {
        let result = parse_generated_cards("not json");
        assert!(result.is_err());
    }

    #[test]
    fn error_on_all_empty() {
        let json = r#"[{"front": "", "back": "", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null}]"#;
        let result = parse_generated_cards(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No valid cards"));
    }

    #[test]
    fn parse_vocabulary_card() {
        let json = r#"[{"front": "食べる", "back": "to eat", "card_type": "vocabulary", "tags": ["japanese", "n5"], "source_context": "食べる means to eat", "cloze_data": null, "vocab_data": {"word": "食べる", "reading": "たべる", "meaning": "to eat", "example_sentence": "寿司を食べる", "part_of_speech": "verb"}}]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards[0].card_type, "vocabulary");
        assert!(cards[0].vocab_data.is_some());
    }

    #[test]
    fn build_prompt_without_existing() {
        let ctx = CardGenerationContext {
            note_content: "Test content".to_string(),
            note_title: "Test Note".to_string(),
            existing_cards_summary: None,
        };
        let (system, user) = build_generation_prompt(&ctx);
        assert!(system.contains("flashcard generation"));
        assert!(user.contains("Test content"));
        assert!(!user.contains("duplicates"));
    }

    #[test]
    fn build_prompt_with_existing() {
        let ctx = CardGenerationContext {
            note_content: "Test content".to_string(),
            note_title: "Test Note".to_string(),
            existing_cards_summary: Some("- Q: What is X? → A: Y".to_string()),
        };
        let (_, user) = build_generation_prompt(&ctx);
        assert!(user.contains("duplicates"));
        assert!(user.contains("What is X?"));
    }

    #[test]
    fn summarize_existing_empty() {
        assert!(summarize_existing_cards(&[]).is_none());
    }

    #[test]
    fn summarize_existing_truncates() {
        let cards = vec![("Q1".to_string(), "A1".to_string())];
        let summary = summarize_existing_cards(&cards).unwrap();
        assert!(summary.contains("Q1"));
        assert!(summary.contains("A1"));
    }
}
```

- [ ] **Step 4: Create `crates/feature-learning/src/lib.rs`**

```rust
pub mod card_generator;
pub mod types;

pub use card_generator::{build_generation_prompt, parse_generated_cards, summarize_existing_cards};
pub use types::{CardGenerationContext, GeneratedCard};
```

- [ ] **Step 5: Add `feature-learning` to workspace**

In `Cargo.toml` (workspace root), add to `members`:
```toml
    "crates/feature-learning",
```

And to `[workspace.dependencies]`:
```toml
feature-learning = { path = "crates/feature-learning" }
```

- [ ] **Step 6: Verify the crate compiles and tests pass**

Run:
```bash
cargo nextest run -p feature-learning
```
Expected: All 9 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-learning/ Cargo.toml
git commit -m "feat(learning): create feature-learning crate with card generation prompt + parser"
```

---

### Task 2: Add IPC types for card generation

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Add IPC request/response types**

Add after the existing `FlashcardListParams` struct (around line 319):

```rust
// ── Card Generation ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardGenerateParams {
    /// Generate from a specific note (fetches note content)
    pub note_id: Option<String>,
    /// Generate from raw text (clipboard, selection)
    pub text_content: Option<String>,
    /// Suggested deck name (optional)
    pub deck_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCardPreview {
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Vec<String>,
    pub source_context: Option<String>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardGenerateResponse {
    pub cards: Vec<GeneratedCardPreview>,
    pub deck_suggestion: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardSaveGeneratedParams {
    pub note_id: Option<String>,
    pub deck: String,
    pub cards: Vec<GeneratedCardPreview>,
}
```

- [ ] **Step 2: Verify compilation**

Run:
```bash
cargo build -p desktop-shared
```
Expected: Compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(learning): add card generation IPC types"
```

---

### Task 3: Backend card generation handlers

**Files:**
- Create: `crates/app-core/src/handlers/notes/card_generation.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/app-core/Cargo.toml`

- [ ] **Step 1: Add `feature-learning` dependency to `app-core`**

In `crates/app-core/Cargo.toml`, add to `[dependencies]`:
```toml
feature-learning = { workspace = true }
```

- [ ] **Step 2: Create `crates/app-core/src/handlers/notes/card_generation.rs`**

```rust
use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use providers::{Message, UserContent};

use crate::state::AppCore;

impl AppCore {
    /// Generate flashcard previews from a note or raw text.
    ///
    /// Calls the LLM with the note content + existing cards context.
    /// Returns preview cards for the user to approve/edit before saving.
    pub async fn flashcard_generate(
        &self,
        params: FlashcardGenerateParams,
    ) -> Result<FlashcardGenerateResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        // Resolve content: from note or raw text
        let (note_title, note_content, note_id) = match (&params.note_id, &params.text_content) {
            (Some(nid), _) => {
                let note = self
                    .note_repo
                    .get_note(nid)
                    .await
                    .map_err(|e| ApiError::new("NOT_FOUND", e.to_string()))?
                    .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;
                (note.title.clone(), note.body.clone(), Some(nid.clone()))
            }
            (_, Some(text)) => {
                if text.trim().is_empty() {
                    return Err(ApiError::new("INVALID_INPUT", "Text content is empty"));
                }
                ("Pasted Text".to_string(), text.clone(), None)
            }
            _ => {
                return Err(ApiError::new(
                    "INVALID_INPUT",
                    "Either note_id or text_content is required",
                ));
            }
        };

        // Fetch existing cards for duplicate avoidance
        let existing_summary = if let Some(ref nid) = note_id {
            let repo = self.flashcard_repo()?;
            let existing = repo
                .list_by_note(nid)
                .await
                .unwrap_or_default();
            let pairs: Vec<(String, String)> = existing
                .iter()
                .map(|c| (c.front.clone(), c.back.clone()))
                .collect();
            feature_learning::summarize_existing_cards(&pairs)
        } else {
            None
        };

        // Build prompt
        let ctx = feature_learning::CardGenerationContext {
            note_content,
            note_title: note_title.clone(),
            existing_cards_summary: existing_summary,
        };
        let (system_prompt, user_prompt) = feature_learning::build_generation_prompt(&ctx);

        // Call LLM
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let messages = vec![
            Message::System {
                content: system_prompt,
            },
            Message::User {
                content: UserContent::Text(user_prompt),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", format!("Card generation failed: {e}")))?;

        let response_text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        // Parse response
        let generated = feature_learning::parse_generated_cards(&response_text)
            .map_err(|e| ApiError::new("PARSE_ERROR", e))?;

        // Convert to preview type
        let cards: Vec<GeneratedCardPreview> = generated
            .into_iter()
            .map(|g| GeneratedCardPreview {
                front: g.front,
                back: g.back,
                card_type: g.card_type,
                tags: g.tags,
                source_context: g.source_context,
                cloze_data: g.cloze_data,
                vocab_data: g.vocab_data,
            })
            .collect();

        // Suggest deck name from note title or hint
        let deck_suggestion = params
            .deck_hint
            .unwrap_or_else(|| note_title.chars().take(40).collect());

        Ok(FlashcardGenerateResponse {
            cards,
            deck_suggestion,
        })
    }

    /// Save user-approved generated cards as real flashcards.
    pub async fn flashcard_save_generated(
        &self,
        params: FlashcardSaveGeneratedParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;

        let cards: Vec<cognitive::NewFlashcard> = params
            .cards
            .iter()
            .map(|c| {
                let card_type = cognitive::CardType::parse(&c.card_type);
                cognitive::NewFlashcard {
                    source_note_id: params.note_id.clone(),
                    source_context: c.source_context.clone(),
                    deck: params.deck.clone(),
                    front: c.front.clone(),
                    back: c.back.clone(),
                    card_type,
                    cloze_data: c.cloze_data.clone(),
                    vocab_data: c.vocab_data.clone(),
                    image_data: None,
                    tags: c.tags.clone(),
                    stability: 1.0,
                    difficulty: 5.0,
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(super::flashcard::flashcard_to_response)
            .collect())
    }
}
```

- [ ] **Step 3: Register the module in `mod.rs`**

Add to `crates/app-core/src/handlers/notes/mod.rs`:
```rust
mod card_generation;
```

- [ ] **Step 4: Verify compilation**

Run:
```bash
cargo build -p app-core
```
Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/
git commit -m "feat(learning): add card generation + save handlers in app-core"
```

---

### Task 4: Tauri commands + dev server registration

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Add Tauri commands**

Add to `crates/desktop/src/commands/notes.rs` (after the existing flashcard commands):

```rust
#[tauri::command]
pub async fn flashcard_generate(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardGenerateParams,
) -> Result<FlashcardGenerateResponse, ApiError> {
    state.flashcard_generate(params).await
}

#[tauri::command]
pub async fn flashcard_save_generated(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardSaveGeneratedParams,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_save_generated(params).await
}
```

- [ ] **Step 2: Add to `DEV_COMMANDS`**

Add the new command names to the `DEV_COMMANDS` constant in the same file:

```rust
"flashcard_generate",
"flashcard_save_generated",
```

- [ ] **Step 3: Add dispatch_dev match arms**

In the `dispatch_dev` function (same file), add:

```rust
"flashcard_generate" => {
    dev::val(core.flashcard_generate(try_field!(dev::parse_params(body))).await)
}
"flashcard_save_generated" => {
    dev::val(core.flashcard_save_generated(try_field!(dev::parse_params(body))).await)
}
```

- [ ] **Step 4: Register in `main.rs`**

Add to `generate_handler![]` in `crates/desktop/src/main.rs`:

```rust
commands::notes::flashcard_generate,
commands::notes::flashcard_save_generated,
```

- [ ] **Step 5: Verify compilation**

Run:
```bash
cargo build -p desktop
```
Expected: Compiles without errors. The `dev_server_covers_all_tauri_commands` test should pass since both commands are in `DEV_COMMANDS`.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/
git commit -m "feat(learning): register card generation Tauri commands"
```

---

### Task 5: Frontend — useCardGeneration hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useCardGeneration.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";

export interface GeneratedCardPreview {
  front: string;
  back: string;
  cardType: string;
  tags: string[];
  sourceContext: string | null;
  clozeData: unknown | null;
  vocabData: unknown | null;
}

interface GenerateResponse {
  cards: GeneratedCardPreview[];
  deckSuggestion: string;
}

interface UseCardGenerationReturn {
  /** Whether the LLM is currently generating cards */
  generating: boolean;
  /** Preview cards returned by the LLM (before approval) */
  previews: GeneratedCardPreview[];
  /** Suggested deck name from the generator */
  deckSuggestion: string;
  /** Error message if generation failed */
  error: string | null;
  /** Generate cards from a note */
  generateFromNote: (noteId: string, deckHint?: string) => Promise<void>;
  /** Generate cards from raw text (clipboard) */
  generateFromText: (text: string, deckHint?: string) => Promise<void>;
  /** Toggle approval state for a card by index */
  toggleCard: (index: number) => void;
  /** Update a card's front/back by index */
  editCard: (index: number, field: "front" | "back", value: string) => void;
  /** Which card indices are approved */
  approved: Set<number>;
  /** Save all approved cards */
  saveApproved: (noteId: string | null, deck: string) => Promise<void>;
  /** Whether cards are being saved */
  saving: boolean;
  /** Reset state (close modal) */
  reset: () => void;
}

export function useCardGeneration(): UseCardGenerationReturn {
  const [generating, setGenerating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [previews, setPreviews] = useState<GeneratedCardPreview[]>([]);
  const [deckSuggestion, setDeckSuggestion] = useState("");
  const [approved, setApproved] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const generate = useCallback(
    async (noteId?: string, textContent?: string, deckHint?: string) => {
      setGenerating(true);
      setError(null);
      setPreviews([]);
      setApproved(new Set());

      try {
        const response = await ipc<GenerateResponse>("flashcard_generate", {
          noteId: noteId ?? null,
          textContent: textContent ?? null,
          deckHint: deckHint ?? null,
        });
        setPreviews(response.cards);
        setDeckSuggestion(response.deckSuggestion);
        // Auto-approve all cards initially
        setApproved(new Set(response.cards.map((_, i) => i)));
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setGenerating(false);
      }
    },
    [],
  );

  const generateFromNote = useCallback(
    (noteId: string, deckHint?: string) => generate(noteId, undefined, deckHint),
    [generate],
  );

  const generateFromText = useCallback(
    (text: string, deckHint?: string) => generate(undefined, text, deckHint),
    [generate],
  );

  const toggleCard = useCallback((index: number) => {
    setApproved((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }, []);

  const editCard = useCallback(
    (index: number, field: "front" | "back", value: string) => {
      setPreviews((prev) =>
        prev.map((card, i) => (i === index ? { ...card, [field]: value } : card)),
      );
    },
    [],
  );

  const saveApproved = useCallback(
    async (noteId: string | null, deck: string) => {
      const approvedCards = previews.filter((_, i) => approved.has(i));
      if (approvedCards.length === 0) return;

      setSaving(true);
      try {
        await ipc("flashcard_save_generated", {
          noteId,
          deck,
          cards: approvedCards,
        });
        invalidateQueries("flashcard_");
        setPreviews([]);
        setApproved(new Set());
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setSaving(false);
      }
    },
    [previews, approved],
  );

  const reset = useCallback(() => {
    setPreviews([]);
    setApproved(new Set());
    setError(null);
    setDeckSuggestion("");
    setGenerating(false);
    setSaving(false);
  }, []);

  return {
    generating,
    previews,
    deckSuggestion,
    error,
    generateFromNote,
    generateFromText,
    toggleCard,
    editCard,
    approved,
    saveApproved,
    saving,
    reset,
  };
}
```

- [ ] **Step 2: Verify with Biome**

Run:
```bash
cd desktop-ui && bun run lint:fix
```
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useCardGeneration.ts
git commit -m "feat(learning): add useCardGeneration hook"
```

---

### Task 6: Frontend — CardGenerationModal component

**Files:**
- Create: `desktop-ui/src/features/notes/components/CardGenerationModal.tsx`

- [ ] **Step 1: Create the modal component**

This is a portal-based modal (same pattern as `QuickAdd.tsx` in the learn feature). Shows generated cards with approve/dismiss toggles, inline editing, deck picker, and save button.

```tsx
import { Check, ChevronDown, ChevronUp, Loader2, Sparkles, X } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { GeneratedCardPreview } from "../hooks/useCardGeneration";

interface CardGenerationModalProps {
  open: boolean;
  generating: boolean;
  previews: GeneratedCardPreview[];
  deckSuggestion: string;
  approved: Set<number>;
  error: string | null;
  saving: boolean;
  onToggleCard: (index: number) => void;
  onEditCard: (index: number, field: "front" | "back", value: string) => void;
  onSave: (noteId: string | null, deck: string) => void;
  onClose: () => void;
  noteId: string | null;
}

function CardPreviewRow({
  card,
  index,
  isApproved,
  onToggle,
  onEdit,
}: {
  card: GeneratedCardPreview;
  index: number;
  isApproved: boolean;
  onToggle: () => void;
  onEdit: (field: "front" | "back", value: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);

  const typeLabel =
    card.cardType === "cloze"
      ? "Cloze"
      : card.cardType === "vocabulary"
        ? "Vocab"
        : "Basic";

  const typeBg =
    card.cardType === "cloze"
      ? "bg-purple/10 text-purple"
      : card.cardType === "vocabulary"
        ? "bg-blue-400/10 text-blue-400"
        : "bg-muted text-muted-foreground";

  return (
    <div
      className={`glass-card p-3 transition-all ${isApproved ? "opacity-100" : "opacity-40"}`}
    >
      <div className="flex items-start gap-2">
        {/* Approve toggle */}
        <button
          type="button"
          onClick={onToggle}
          className={`mt-0.5 w-5 h-5 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
            isApproved
              ? "bg-brand text-white"
              : "bg-muted text-muted-foreground hover:bg-accent"
          }`}
        >
          {isApproved && <Check size={12} />}
        </button>

        {/* Card content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className={`text-[10px] px-1.5 py-0.5 rounded-md ${typeBg}`}>
              {typeLabel}
            </span>
            {card.tags.length > 0 && (
              <span className="text-[10px] text-muted-foreground">
                {card.tags.join(", ")}
              </span>
            )}
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="ml-auto text-muted-foreground hover:text-foreground"
            >
              {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
          </div>

          <p className="text-sm text-foreground leading-snug">{card.front}</p>

          {expanded && (
            <div className="mt-2 space-y-2">
              <div>
                <label className="text-[10px] text-muted-foreground block mb-0.5">
                  Front
                </label>
                <textarea
                  value={card.front}
                  onChange={(e) => onEdit("front", e.target.value)}
                  className="w-full bg-muted/50 rounded-md px-2 py-1.5 text-sm text-foreground resize-none"
                  rows={2}
                />
              </div>
              <div>
                <label className="text-[10px] text-muted-foreground block mb-0.5">
                  Back
                </label>
                <textarea
                  value={card.back}
                  onChange={(e) => onEdit("back", e.target.value)}
                  className="w-full bg-muted/50 rounded-md px-2 py-1.5 text-sm text-foreground resize-none"
                  rows={2}
                />
              </div>
              {card.sourceContext && (
                <p className="text-[11px] text-muted-foreground italic">
                  Source: {card.sourceContext}
                </p>
              )}
            </div>
          )}

          {!expanded && (
            <p className="text-[12px] text-muted-foreground mt-0.5 truncate">
              {card.back}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

export function CardGenerationModal({
  open,
  generating,
  previews,
  deckSuggestion,
  approved,
  error,
  saving,
  onToggleCard,
  onEditCard,
  onSave,
  onClose,
  noteId,
}: CardGenerationModalProps) {
  const [deck, setDeck] = useState(deckSuggestion);

  // Update deck when suggestion changes
  useEffect(() => {
    if (deckSuggestion) setDeck(deckSuggestion);
  }, [deckSuggestion]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  const approvedCount = approved.size;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={() => {}}
        role="presentation"
      />

      {/* Modal */}
      <div className="relative glass-panel rounded-2xl w-full max-w-lg max-h-[80vh] flex flex-col mx-4">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-border">
          <div className="flex items-center gap-2">
            <Sparkles size={18} className="text-brand" strokeWidth={1.5} />
            <h2 className="text-sm font-semibold text-foreground">Generate Flashcards</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-2">
          {generating && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <Loader2 size={24} className="text-brand animate-spin" strokeWidth={1.5} />
              <p className="text-sm text-muted-foreground">Generating cards from your note...</p>
            </div>
          )}

          {error && (
            <div className="glass-card p-3 border border-red-500/20">
              <p className="text-sm text-red-400">{error}</p>
            </div>
          )}

          {!generating &&
            previews.map((card, i) => (
              <CardPreviewRow
                key={`${card.front.slice(0, 20)}-${i}`}
                card={card}
                index={i}
                isApproved={approved.has(i)}
                onToggle={() => onToggleCard(i)}
                onEdit={(field, value) => onEditCard(i, field, value)}
              />
            ))}
        </div>

        {/* Footer */}
        {!generating && previews.length > 0 && (
          <div className="px-5 py-4 border-t border-border space-y-3">
            {/* Deck picker */}
            <div className="flex items-center gap-2">
              <label className="text-[12px] text-muted-foreground whitespace-nowrap">
                Deck:
              </label>
              <input
                type="text"
                value={deck}
                onChange={(e) => setDeck(e.target.value)}
                placeholder="Enter deck name..."
                className="flex-1 bg-muted/50 rounded-lg px-3 py-1.5 text-sm text-foreground placeholder:text-dim"
              />
            </div>

            {/* Save button */}
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-muted-foreground">
                {approvedCount} of {previews.length} cards selected
              </span>
              <button
                type="button"
                onClick={() => onSave(noteId, deck)}
                disabled={approvedCount === 0 || !deck.trim() || saving}
                className="glass-button px-4 py-2 text-sm text-foreground disabled:opacity-40 disabled:cursor-not-allowed inline-flex items-center gap-1.5"
              >
                {saving ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Check size={14} />
                )}
                Save {approvedCount} Card{approvedCount !== 1 ? "s" : ""}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
```

- [ ] **Step 2: Verify with Biome**

Run:
```bash
cd desktop-ui && bun run lint:fix
```
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/CardGenerationModal.tsx
git commit -m "feat(learning): add CardGenerationModal preview component"
```

---

### Task 7: Frontend — Editor toolbar + KnowledgeBasePage integration

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditorPanel.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Add "Generate Cards" button to EditorToolbar**

In `EditorToolbar.tsx`:

1. Add `Sparkles` to the lucide-react import.
2. Add `onGenerateCards?: (selectedText?: string) => void` to `EditorToolbarProps`.
3. In the `modeButtons` JSX, add the generate button **before** the focus mode button:

```tsx
{onGenerateCards && (
  <button
    type="button"
    onClick={() => {
      // If text is selected in the editor, generate from selection
      const { from, to } = editor.state.selection;
      const selectedText = from !== to ? editor.state.doc.textBetween(from, to, "\n") : undefined;
      onGenerateCards(selectedText);
    }}
    title="Generate flashcards from note (or selection)"
    className="p-1.5 rounded-lg transition-all text-dim hover:text-muted-foreground hover:bg-card"
  >
    <Sparkles className="w-3.5 h-3.5" strokeWidth={1.5} />
  </button>
)}
```

This handles **both** spec flow 1 ("Generate from note") and flow 2 ("Generate from selection"):
- No selection → generates from full note content
- Text selected → generates from selection only (passes selection as `textContent` to the handler)

- [ ] **Step 2: Thread `onGenerateCards` through NoteEditorPanel**

In `NoteEditorPanel.tsx`:

1. Add `onGenerateCards?: (selectedText?: string) => void` to `NoteEditorPanelProps`.
2. Pass it to the `<NoteEditor>` component (or wherever `EditorToolbar` is rendered).

Check where `EditorToolbar` is rendered — it's inside `NoteEditor.tsx`. Thread the prop through:
- `NoteEditorPanel` → `NoteEditor` → `EditorToolbar`

In each component, add `onGenerateCards?: (selectedText?: string) => void` to props and forward it.

- [ ] **Step 3: Wire CardGenerationModal in KnowledgeBasePage**

In `KnowledgeBasePage.tsx`:

1. Import `useCardGeneration` and `CardGenerationModal`.
2. Add the hook call inside the component:
```tsx
const cardGen = useCardGeneration();
const [cardGenOpen, setCardGenOpen] = useState(false);
```

3. Create handler that supports both full-note and selection-based generation:
```tsx
const handleGenerateCards = useCallback((selectedText?: string) => {
  if (!selectedNote) return;
  setCardGenOpen(true);
  if (selectedText) {
    // Generate from selection — pass text directly, but keep noteId for source linking
    cardGen.generateFromText(selectedText, selectedNote.title);
  } else {
    // Generate from full note
    cardGen.generateFromNote(selectedNote.id);
  }
}, [selectedNote, cardGen.generateFromNote, cardGen.generateFromText]);
```

4. Pass `onGenerateCards={handleGenerateCards}` to `NoteEditorPanel`.

5. Add `CardGenerationModal` at the end of the component return:
```tsx
<CardGenerationModal
  open={cardGenOpen}
  generating={cardGen.generating}
  previews={cardGen.previews}
  deckSuggestion={cardGen.deckSuggestion}
  approved={cardGen.approved}
  error={cardGen.error}
  saving={cardGen.saving}
  onToggleCard={cardGen.toggleCard}
  onEditCard={cardGen.editCard}
  onSave={(noteId, deck) => {
    cardGen.saveApproved(noteId, deck).then(() => setCardGenOpen(false));
  }}
  onClose={() => {
    cardGen.reset();
    setCardGenOpen(false);
  }}
  noteId={selectedNote?.id ?? null}
/>
```

**Note for implementer:** Find the correct variable name for the selected note in KnowledgeBasePage — it may be accessed via a different state variable. Look for the note that's currently being edited.

- [ ] **Step 4: Verify the toolbar renders and modal opens**

Run:
```bash
cd desktop-ui && bun run build
```
Expected: No build errors. The Sparkles icon should appear in the editor toolbar.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(learning): add Generate Cards button to editor toolbar"
```

---

### Task 8: Frontend — Quick Generate on Learn dashboard

**Files:**
- Create: `desktop-ui/src/features/learn/components/NotePicker.tsx`
- Create: `desktop-ui/src/features/learn/components/QuickGenerate.tsx`
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx`
- Modify: `desktop-ui/src/features/learn/pages/LearnPage.tsx`

- [ ] **Step 1: Create NotePicker component**

A minimal search-and-select dropdown for notes.

```tsx
import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";
import { FileText, Search } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface NotePickerProps {
  onSelect: (note: { id: string; title: string }) => void;
  onCancel: () => void;
}

export function NotePicker({ onSelect, onCancel }: NotePickerProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Note[]>([]);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Escape closes
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onCancel]);

  const search = useCallback(async (q: string) => {
    if (q.trim().length < 2) {
      setResults([]);
      return;
    }
    setLoading(true);
    try {
      const notes = await ipc<Note[]>("note_search", { query: q });
      setResults(notes.slice(0, 10));
    } catch {
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  // Debounced search
  useEffect(() => {
    const timer = setTimeout(() => search(query), 200);
    return () => clearTimeout(timer);
  }, [query, search]);

  return (
    <div className="space-y-2">
      <div className="relative">
        <Search
          size={14}
          className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search notes..."
          className="w-full bg-muted/50 rounded-lg pl-8 pr-3 py-2 text-sm text-foreground placeholder:text-dim"
        />
      </div>

      {results.length > 0 && (
        <div className="max-h-48 overflow-y-auto space-y-0.5">
          {results.map((note) => (
            <button
              key={note.id}
              type="button"
              onClick={() => onSelect({ id: note.id, title: note.title })}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left text-sm text-foreground hover:bg-accent transition-colors"
            >
              <FileText size={14} className="text-muted-foreground flex-shrink-0" />
              <span className="truncate">{note.title}</span>
            </button>
          ))}
        </div>
      )}

      {query.length >= 2 && !loading && results.length === 0 && (
        <p className="text-[12px] text-muted-foreground text-center py-2">No notes found</p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create QuickGenerate component**

```tsx
import { Clipboard, FileText, Loader2, MessageSquare } from "lucide-react";
import { useState } from "react";
import { NotePicker } from "./NotePicker";

type QuickGenMode = null | "note" | "clipboard";

interface QuickGenerateProps {
  onGenerateFromNote: (noteId: string) => void;
  onGenerateFromText: (text: string) => void;
  generating: boolean;
}

export function QuickGenerate({
  onGenerateFromNote,
  onGenerateFromText,
  generating,
}: QuickGenerateProps) {
  const [mode, setMode] = useState<QuickGenMode>(null);
  const [clipboardText, setClipboardText] = useState("");

  if (generating) {
    return (
      <div className="glass-card p-4 flex items-center justify-center gap-2">
        <Loader2 size={16} className="text-brand animate-spin" strokeWidth={1.5} />
        <span className="text-sm text-muted-foreground">Generating cards...</span>
      </div>
    );
  }

  if (mode === "note") {
    return (
      <div className="glass-card p-4">
        <p className="text-[12px] text-muted-foreground mb-2">Select a note to generate from:</p>
        <NotePicker
          onSelect={(note) => {
            setMode(null);
            onGenerateFromNote(note.id);
          }}
          onCancel={() => setMode(null)}
        />
      </div>
    );
  }

  if (mode === "clipboard") {
    return (
      <div className="glass-card p-4 space-y-2">
        <p className="text-[12px] text-muted-foreground">
          Paste text to generate flashcards:
        </p>
        <textarea
          value={clipboardText}
          onChange={(e) => setClipboardText(e.target.value)}
          placeholder="Paste content here..."
          className="w-full bg-muted/50 rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-dim resize-none"
          rows={4}
          autoFocus
        />
        <div className="flex items-center justify-between">
          <button
            type="button"
            onClick={() => setMode(null)}
            className="text-[12px] text-muted-foreground hover:text-foreground"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => {
              if (clipboardText.trim()) {
                setMode(null);
                onGenerateFromText(clipboardText);
              }
            }}
            disabled={!clipboardText.trim()}
            className="glass-button px-3 py-1.5 text-[12px] text-foreground disabled:opacity-40"
          >
            Generate
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 text-left">
      <p className="text-sm font-medium text-foreground mb-3">Quick Generate</p>
      <div className="space-y-1.5">
        <button
          type="button"
          onClick={() => setMode("note")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-left"
        >
          <FileText size={14} strokeWidth={1.5} />
          From note...
        </button>
        <button
          type="button"
          onClick={() => setMode("clipboard")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-left"
        >
          <Clipboard size={14} strokeWidth={1.5} />
          From clipboard...
        </button>
        <button
          type="button"
          disabled
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground opacity-50 cursor-not-allowed text-left"
        >
          <MessageSquare size={14} strokeWidth={1.5} />
          From last chat...
          <span className="ml-auto text-[10px]">Soon</span>
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Update DashboardHome to use QuickGenerate**

In `DashboardHome.tsx`, replace the static placeholder card (the `"AI Study Session"` / `"Coming in Phase 5"` block at ~line 100-108) with a second action card layout. Add the `QuickGenerate` component alongside it.

Change the `DashboardHomeProps` interface:
```tsx
interface DashboardHomeProps {
  onStartReview: (deck?: string) => void;
  onQuickAdd: () => void;
  onGenerateFromNote: (noteId: string) => void;
  onGenerateFromText: (text: string) => void;
  generating: boolean;
}
```

Replace the "AI Study Session" placeholder div (~line 100-108) with:
```tsx
<QuickGenerate
  onGenerateFromNote={onGenerateFromNote}
  onGenerateFromText={onGenerateFromText}
  generating={generating}
/>
```

Import `QuickGenerate` at the top.

- [ ] **Step 4: Wire Quick Generate in LearnPage**

In `LearnPage.tsx`:

1. Import `useCardGeneration` from `../../notes/hooks/useCardGeneration` and `CardGenerationModal` from `../../notes/components/CardGenerationModal`.

2. Add hook + state:
```tsx
const cardGen = useCardGeneration();
const [cardGenOpen, setCardGenOpen] = useState(false);
```

3. Pass to `DashboardHome`:
```tsx
<DashboardHome
  onStartReview={...}
  onQuickAdd={...}
  onGenerateFromNote={(noteId) => {
    setCardGenOpen(true);
    cardGen.generateFromNote(noteId);
  }}
  onGenerateFromText={(text) => {
    setCardGenOpen(true);
    cardGen.generateFromText(text);
  }}
  generating={cardGen.generating}
/>
```

4. Add `CardGenerationModal` at the end:
```tsx
<CardGenerationModal
  open={cardGenOpen}
  generating={cardGen.generating}
  previews={cardGen.previews}
  deckSuggestion={cardGen.deckSuggestion}
  approved={cardGen.approved}
  error={cardGen.error}
  saving={cardGen.saving}
  onToggleCard={cardGen.toggleCard}
  onEditCard={cardGen.editCard}
  onSave={(noteId, deck) => {
    cardGen.saveApproved(noteId, deck).then(() => setCardGenOpen(false));
  }}
  onClose={() => {
    cardGen.reset();
    setCardGenOpen(false);
  }}
  noteId={null}
/>
```

- [ ] **Step 5: Verify build**

Run:
```bash
cd desktop-ui && bun run build
```
Expected: No build errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/learn/ desktop-ui/src/features/notes/
git commit -m "feat(learning): add Quick Generate on Learn dashboard with note picker"
```

---

### Task 9: Source linking — review source button + insight fix

**Files:**
- Modify: `desktop-ui/src/features/learn/components/ImmersiveReview.tsx`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Wire "Source" button in ImmersiveReview**

In `ImmersiveReview.tsx`, find the disabled "Source" button (~line 170-177). Replace it with a functional button:

```tsx
<button
  type="button"
  onClick={() => {
    if (current?.sourceNoteId) {
      onExit();
      // Navigate to the source note in the notes page
      window.location.hash = `/notes?id=${current.sourceNoteId}`;
    }
  }}
  disabled={!current?.sourceNoteId}
  className={`flex items-center gap-1 text-[11px] transition-colors ${
    current?.sourceNoteId
      ? "text-muted-foreground hover:text-foreground cursor-pointer"
      : "text-muted-foreground opacity-50 cursor-not-allowed"
  }`}
>
  <ExternalLink size={12} strokeWidth={1.5} />
  Source
</button>
```

**Note for implementer:** Check how `KnowledgeBasePage` handles the `?id=` search param — it reads `searchParams.get("id")` in a `useSearchParams()` hook and selects the note. Use `useNavigate` from react-router instead of `window.location.hash` if the hash router requires it. The correct approach is:

```tsx
import { useNavigate } from "react-router";
// ...
const navigate = useNavigate();
// In the handler:
navigate(`/notes?id=${current.sourceNoteId}`);
```

But `useNavigate` is only available inside the router context, and `ImmersiveReview` is already inside it (rendered via `LearnPage` which is inside `AppShell`). So `navigate()` should work.

However, the `onExit()` call will switch back to dashboard mode. The navigate needs to happen **instead of** exit, not before it. Adjust the flow:

```tsx
onClick={() => {
  if (current?.sourceNoteId) {
    navigate(`/notes?id=${current.sourceNoteId}`);
  }
}}
```

The navigation away from `/learn` will unmount the component, so `onExit()` is not needed.

- [ ] **Step 2: Add keyboard shortcut for source navigation**

In the keyboard handler `useEffect`, add an 'S' key binding (when card is revealed):

```tsx
if (e.key === "s" || e.key === "S") {
  if (current?.sourceNoteId) {
    e.preventDefault();
    navigate(`/notes?id=${current.sourceNoteId}`);
  }
  return;
}
```

Add this inside the `if (revealed && current)` block, after the rating map.

- [ ] **Step 3: Fix `insight_save_flashcards` to populate source_context**

In `crates/app-core/src/handlers/notes/insight.rs`, update the `insight_save_flashcards` method (~line 393-437).

Currently `source_context` is `None`. Populate it with the question's source note title (from `q.source_notes`) and the question text as context:

```rust
// Inside the .map(|q| { ... }) closure, replace:
//   source_context: None,
// with:
source_context: if q.source_notes.is_empty() {
    None
} else {
    Some(format!("From: {}", q.source_notes.join(", ")))
},
```

This is a minimal fix. A more sophisticated approach (extracting the actual paragraph from the note body that the question relates to) can be done in Phase 5 when the tutor has deeper content analysis.

- [ ] **Step 4: Verify compilation and build**

Run:
```bash
cargo build -p app-core && cd desktop-ui && bun run build
```
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs desktop-ui/src/features/learn/components/ImmersiveReview.tsx
git commit -m "feat(learning): wire source note navigation + populate source_context in insight cards"
```

---

## Testing Checklist

After all tasks are complete, verify the full flow:

- [ ] **Backend generation**: Call `flashcard_generate` with a note ID via dev server HTTP. Verify JSON response contains 5+ cards with varied types.
- [ ] **Backend save**: Call `flashcard_save_generated` with the previews. Verify cards appear in `flashcard_list_decks`.
- [ ] **Editor toolbar**: Click Sparkles icon in note editor. Modal opens, shows loading, then card previews.
- [ ] **Card approval flow**: Toggle cards on/off, edit front/back, change deck name, save. Verify cards created in correct deck.
- [ ] **Quick Generate — note**: In Learn dashboard, click "From note...", search for a note, select it. Modal opens with generated cards.
- [ ] **Quick Generate — clipboard**: Click "From clipboard...", paste text, generate. Modal opens with cards (no source note link).
- [ ] **Source navigation**: During review, click "Source" button on a card that has `sourceNoteId`. Navigates to `/notes?id=...`.
- [ ] **Keyboard shortcut**: Press 'S' during review on a source-linked card. Navigates to source note.
- [ ] **Sidebar badge**: After saving generated cards, the sidebar Learn badge updates with new due count.
- [ ] **Insight flashcards**: Save flashcards from Insight Review → cards now have `sourceContext` populated.

---

## Architecture Decisions

1. **`feature-learning` stays lightweight** — only prompt templates and parsing for Phase 3. The LLM call orchestration stays in `app-core` where `cognitive_provider` lives. This follows the `feature-insights` pattern.

2. **Two-step generation flow** (generate → preview → save) — lets the user review AI output before committing to the database. The generate endpoint is stateless; previews are held in frontend state.

3. **`GeneratedCardPreview` in `desktop-shared`** — since it's an IPC response type, it belongs with other IPC types. The internal `GeneratedCard` type in `feature-learning` is separate (no `camelCase` serde).

4. **Source linking is snapshot-based** — `source_context` captures the excerpt at generation time. Phase 9 (Feedback Arrow) will add staleness detection when notes change.

5. **No streaming for generation** — card generation uses `chat()` (non-streaming) since the response is JSON that must be parsed atomically. The loading state is sufficient UX for a 3-5 second LLM call.

6. **Unified toolbar button for note + selection** — the Sparkles button checks if text is selected in the TipTap editor. If so, it passes the selection as `textContent` for focused generation. If not, it generates from the full note. This covers spec flows 1 and 2 with a single button. Flows 3/4 (sentence pairs, Cornell Q&A) depend on Phase 4 split-pane modes.
