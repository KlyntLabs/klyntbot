# Language Learning Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the Translation split mode into an AI-powered language learning workspace with sentence breakdown, vocabulary extraction, translation practice with evaluation, grammar analysis, and confusable word detection.

**Architecture:** Smart Hybrid — upgrade the existing Translation split mode right pane from a plain TipTap editor to a `LanguageLearningPanel` with 5 stacked collapsible sections. Backend uses the existing `cognitive_provider.chat()` pattern for LLM calls with structured JSON responses. Vocabulary persistence reuses `FlashcardRepo` (Vocabulary card type) and `SemanticFactRepo` (domain: "learning"). Config adds a new `language: LanguageConfig` section to `config.json`.

**Tech Stack:** Rust (app-core handlers, config schema, Tauri IPC), React/TypeScript (LanguageLearningPanel, hooks, SplitEditor integration), TipTap (editor integration), SQLite (existing tables — no new migrations).

**Spec:** `docs/superpowers/specs/2026-03-19-language-learning-surface-design.md`

---

## File Structure

### Backend — New Files

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/config/src/schema/language.rs` | Create | `LanguageConfig` struct (source_lang, target_lang, auto_detect, proficiency_level) |
| `crates/app-core/src/handlers/notes/language.rs` | Create | 6 handler methods on `AppCore` for translation, evaluation, vocabulary, confusables, enrichment |
| `crates/app-core/src/handlers/notes/language_prompts.rs` | Create | LLM prompt builder functions (system prompts for each handler) |
| `crates/desktop-shared/src/commands/language.rs` | Create | Shared IPC types: params + response structs |
| `crates/desktop/src/commands/language.rs` | Create | Tauri IPC commands + DEV_COMMANDS + dispatch_dev |

### Backend — Modified Files

| File | Action | Changes |
|------|--------|---------|
| `crates/config/src/schema/mod.rs` | Modify | Add `mod language;` + `pub use self::language::*;` |
| `crates/config/src/schema/core.rs` | Modify | Add `pub language: LanguageConfig` field to `Config` struct |
| `crates/desktop-shared/src/commands/mod.rs` | Modify | Add `mod language;` + `pub use language::*;` |
| `crates/app-core/src/handlers/notes/mod.rs` | Modify | Add `mod language;` + `mod language_prompts;` |
| `crates/desktop/src/commands/mod.rs` | Modify | Add `pub mod language;` |
| `crates/desktop/src/dev_server/dispatch.rs` | Modify | Add language dispatch chain entry |
| `crates/desktop/src/dev_server/mod.rs` | Modify | Add `commands::language::DEV_COMMANDS` to test array |
| `crates/desktop/src/main.rs` | Modify | Register 6 language commands in invoke_handler |
| `crates/cognitive/src/repos/semantic_fact.rs` | Modify | Add `find_vocabulary_by_subject()` method for CJK-safe lookup |

### Frontend — New Files

| File | Responsibility |
|------|----------------|
| `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx` | Main container — 5 stacked sections |
| `desktop-ui/src/features/notes/components/editor/language/TranslationSection.tsx` | Always-expanded translation display |
| `desktop-ui/src/features/notes/components/editor/language/WordsSection.tsx` | Word breakdown list with chips + save |
| `desktop-ui/src/features/notes/components/editor/language/GrammarSection.tsx` | Collapsible grammar pattern display |
| `desktop-ui/src/features/notes/components/editor/language/PracticeSection.tsx` | Translation input + Hybrid C evaluation |
| `desktop-ui/src/features/notes/components/editor/language/ConfusableSection.tsx` | Conditional similar-word alerts |
| `desktop-ui/src/features/notes/components/editor/language/CollapsibleSection.tsx` | Shared collapsible section wrapper |
| `desktop-ui/src/features/notes/hooks/useLanguageBreakdown.ts` | Calls language_translate_breakdown IPC |
| `desktop-ui/src/features/notes/hooks/useTranslationPractice.ts` | Manages practice input + evaluation |
| `desktop-ui/src/features/notes/hooks/useLanguageConfig.ts` | Reads global config + per-note override |
| `desktop-ui/src/features/notes/hooks/useVocabularySave.ts` | Batch-saves vocabulary to flashcards + facts |

### Frontend — Modified Files

| File | Changes |
|------|---------|
| `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx` | Replace right pane in translation mode with LanguageLearningPanel |
| `desktop-ui/src/features/notes/components/editor/AnnotationSidebar.tsx` | Add language enrichment to annotation cards (P2) |

---

## Phase 0: Backend Foundation

### Task 1: Config — LanguageConfig Schema

**Files:**
- Create: `crates/config/src/schema/language.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Create language.rs config module**

Create `crates/config/src/schema/language.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfig {
    /// Source language for translation (e.g., "zh", "ja", "en")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_lang: Option<String>,

    /// Target language for translation (e.g., "en", "vi")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lang: Option<String>,

    /// Auto-detect source language when not configured
    #[serde(default = "super::core::default_true")]
    pub auto_detect: bool,

    /// User's proficiency level (e.g., "HSK 3", "CEFR B1")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proficiency_level: Option<String>,
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

- [ ] **Step 2: Register in mod.rs**

In `crates/config/src/schema/mod.rs`, add `mod language;` to the private module block (after `mod launcher;`) and `pub use self::language::*;` to the re-export block.

- [ ] **Step 3: Add field to Config struct**

In `crates/config/src/schema/core.rs`, add to the `Config` struct (after the `launcher` field):

```rust
#[serde(default)]
pub language: LanguageConfig,
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p config`
Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/language.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add LanguageConfig for language learning pair settings"
```

---

### Task 2: Shared IPC Types — Language Commands

**Files:**
- Create: `crates/desktop-shared/src/commands/language.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

- [ ] **Step 1: Create language shared types**

Create `crates/desktop-shared/src/commands/language.rs`:

```rust
use serde::{Deserialize, Serialize};

// ── Translation Breakdown ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateBreakdownParams {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslateBreakdownResponse {
    pub translation: String,
    pub words: Vec<WordBreakdown>,
    pub grammar_patterns: Vec<GrammarPattern>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WordBreakdown {
    pub word: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub part_of_speech: String,
    pub proficiency_level: Option<String>,
    pub example_sentence: Option<String>,
    pub is_new: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrammarPattern {
    pub pattern: String,
    pub explanation: String,
    pub pattern_type: Option<String>,
}

// ── Translation Evaluation ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateTranslationParams {
    pub source_text: String,
    pub user_translation: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEvalResponse {
    pub grades: EvalGrades,
    pub corrections: Vec<Correction>,
    pub model_translation: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvalGrades {
    pub meaning: String,
    pub grammar: String,
    pub naturalness: String,
    pub word_choice: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Correction {
    pub original: String,
    pub suggested: String,
    pub explanation: String,
    pub category: String,
}

// ── Vocabulary Save ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularySaveParams {
    pub words: Vec<VocabItem>,
    pub note_id: Option<String>,
    pub deck: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VocabItem {
    pub word: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub part_of_speech: String,
    pub example_sentence: Option<String>,
}

// ── Confusable Detection ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectConfusablesParams {
    pub word: String,
    pub source_lang: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfusableResponse {
    pub has_confusable: bool,
    pub confusable_word: Option<String>,
    pub confusable_meaning: Option<String>,
    pub explanation: Option<String>,
}

// ── Annotation Enrichment ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichAnnotationParams {
    pub annotation_id: String,
    pub quoted_text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationEnrichmentResponse {
    pub translation: String,
    pub words: Vec<WordBreakdown>,
}
```

- [ ] **Step 2: Register module in mod.rs**

In `crates/desktop-shared/src/commands/mod.rs`, add `mod language;` and `pub use language::*;`.

- [ ] **Step 3: Build to verify**

Run: `cargo build -p desktop-shared`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/language.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(desktop-shared): add language learning IPC types"
```

---

### Task 3: Backend — Vocabulary Lookup (CJK-Safe)

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`

- [ ] **Step 1: Add find_vocabulary_by_subject method**

In `crates/cognitive/src/repos/semantic_fact.rs`, add a new method to `SemanticFactRepo`:

```rust
/// Find vocabulary facts by exact subject match (CJK-safe, does NOT use FTS5).
/// Used for confusable word detection and "is_new" vocabulary checks.
pub async fn find_vocabulary_by_subject(
    &self,
    word: &str,
) -> Result<Vec<SemanticFact>, sqlx::Error> {
    sqlx::query_as::<_, SemanticFact>(
        "SELECT * FROM semantic_facts WHERE domain = 'learning' AND memory_type = 'vocabulary' AND subject = ?1 AND superseded_at IS NULL",
    )
    .bind(word)
    .fetch_all(&self.pool)
    .await
}

/// Find vocabulary facts with subjects similar to the given word (prefix match).
/// Used for confusable detection: finds words that share characters.
pub async fn find_similar_vocabulary(
    &self,
    word: &str,
    limit: i64,
) -> Result<Vec<SemanticFact>, sqlx::Error> {
    // For CJK: match any fact that shares at least one character with the word
    // For Latin: use LIKE prefix match
    let pattern = if word.chars().any(|c| c > '\u{2E80}') {
        // CJK: search for any fact containing any character from the word
        // Use individual character OR matching
        let chars: Vec<String> = word.chars().map(|c| format!("%{c}%")).collect();
        if let Some(first) = chars.first() {
            first.clone()
        } else {
            return Ok(vec![]);
        }
    } else {
        format!("{}%", &word[..word.len().min(3)])
    };

    sqlx::query_as::<_, SemanticFact>(
        "SELECT * FROM semantic_facts WHERE domain = 'learning' AND memory_type = 'vocabulary' AND subject LIKE ?1 AND subject != ?2 AND superseded_at IS NULL LIMIT ?3",
    )
    .bind(&pattern)
    .bind(word)
    .bind(limit)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): add CJK-safe vocabulary lookup methods to SemanticFactRepo"
```

---

### Task 4: Backend — LLM Prompt Builders

**Files:**
- Create: `crates/app-core/src/handlers/notes/language_prompts.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

- [ ] **Step 1: Create language_prompts.rs**

Create `crates/app-core/src/handlers/notes/language_prompts.rs`. This contains pure functions returning system prompt strings. The LLM is instructed to return structured JSON:

```rust
/// Build system prompt for translation + sentence breakdown.
pub fn translate_breakdown_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning assistant. Translate the given text from {source_lang} to {target_lang} and provide a detailed breakdown.

Respond ONLY with a JSON object (no markdown fences, no explanation). The format:
{{
  "translation": "full translation",
  "words": [
    {{
      "word": "original word",
      "reading": "pronunciation (pinyin for Chinese, IPA for English, null if same script)",
      "meaning": "translation of this word",
      "part_of_speech": "noun/verb/adj/adv/etc",
      "proficiency_level": "HSK 1-6 for Chinese, CEFR A1-C2 for English, null if unknown",
      "example_sentence": "a short example sentence using this word"
    }}
  ],
  "grammar_patterns": [
    {{
      "pattern": "[Subject] + verb + [Object] + 来 + [Purpose]",
      "explanation": "plain language explanation of this grammar pattern",
      "pattern_type": "purpose clause / passive / conditional / etc"
    }}
  ]
}}

Rules:
- Extract ALL meaningful words (skip particles/punctuation unless pedagogically important)
- For Chinese: always include pinyin with tone marks
- For English: include IPA only for words with non-obvious pronunciation
- Identify 1-3 grammar patterns (0 if none are notable)
- proficiency_level: use HSK 1-6 for Chinese, CEFR A1-C2 for European languages
- Keep explanations concise (1-2 sentences)"#
    )
}

/// Build system prompt for evaluating a user's translation attempt.
pub fn evaluate_translation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning evaluator. A student is translating from {source_lang} to {target_lang}. Evaluate their translation across 4 dimensions.

You will receive the source text and the student's translation attempt.

Respond ONLY with a JSON object:
{{
  "grades": {{
    "meaning": "A+/A/A-/B+/B/B-/C+/C/C-/D+/D/F",
    "grammar": "same scale",
    "naturalness": "same scale",
    "word_choice": "same scale"
  }},
  "corrections": [
    {{
      "original": "what the student wrote",
      "suggested": "better version",
      "explanation": "why this is better (1-2 sentences, include linguistic reason)",
      "category": "grammar/vocabulary/register/naturalness"
    }}
  ],
  "model_translation": "your ideal translation of the source text"
}}

Grading guide:
- A: native-level quality
- B: clearly understood, minor issues
- C: meaning conveyed but notable errors
- D: significant errors affecting comprehension
- F: incomprehensible or wrong meaning

Rules:
- Be encouraging but honest
- Focus on the most impactful corrections (max 5)
- Explain WHY each correction matters for learning
- model_translation should be natural, not literal"#
    )
}

/// Build system prompt for detecting confusable words.
pub fn detect_confusables_prompt(source_lang: &str) -> String {
    format!(
        r#"You are a vocabulary specialist for {source_lang}. Given two similar words, explain the key difference between them for a language learner.

Respond ONLY with a JSON object:
{{
  "explanation": "clear explanation of the difference (2-3 sentences)",
  "word1_usage": "when to use word 1",
  "word2_usage": "when to use word 2",
  "example_word1": "example sentence using word 1",
  "example_word2": "example sentence using word 2"
}}"#
    )
}

/// Build system prompt for annotation language enrichment.
pub fn enrich_annotation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning assistant. Translate the given text from {source_lang} to {target_lang} and extract key vocabulary.

Respond ONLY with a JSON object:
{{
  "translation": "full translation",
  "words": [
    {{
      "word": "original word",
      "reading": "pronunciation (pinyin/IPA/null)",
      "meaning": "translation",
      "part_of_speech": "noun/verb/adj/etc",
      "proficiency_level": "HSK 1-6 / CEFR A1-C2 / null"
    }}
  ]
}}

Keep it concise — this is for a small annotation card, not a full breakdown."#
    )
}
```

- [ ] **Step 2: Register in mod.rs**

In `crates/app-core/src/handlers/notes/mod.rs`, add:

```rust
mod language;
mod language_prompts;
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p app-core`
Expected: May fail if language.rs doesn't exist yet — that's fine, the prompts module will compile on its own once language.rs is added in Task 5.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/language_prompts.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(app-core): add language learning LLM prompt builders"
```

---

### Task 5: Backend — App-Core Language Handlers

**Files:**
- Create: `crates/app-core/src/handlers/notes/language.rs`

- [ ] **Step 1: Create language.rs handler**

Create `crates/app-core/src/handlers/notes/language.rs` with all 6 handler methods on `AppCore`. Follow the `card_generation.rs` pattern:

```rust
use cognitive::repos::SemanticFactRepo;
use cognitive::types::SemanticFact;
use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvalGrades, EvaluateTranslationParams, TranslateBreakdownParams,
    TranslateBreakdownResponse, TranslationEvalResponse, VocabItem, VocabularySaveParams,
    WordBreakdown,
};
use desktop_shared::errors::ApiError;

use super::language_prompts;
use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    /// Translate text and return sentence breakdown with word-by-word analysis.
    pub async fn language_translate_breakdown(
        &self,
        params: TranslateBreakdownParams,
    ) -> Result<TranslateBreakdownResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let system = language_prompts::translate_breakdown_prompt(
            &params.source_lang,
            &params.target_lang,
        );
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(params.text.clone()),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let cleaned = common::strip_llm_fences(&text);
        let mut result: TranslateBreakdownResponse = serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse LLM response: {e}")))?;

        // Mark words as new/known by checking SemanticFact store
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
        for word in &mut result.words {
            let existing = sf_repo.find_vocabulary_by_subject(&word.word).await.unwrap_or_default();
            word.is_new = existing.is_empty();
        }

        Ok(result)
    }

    /// Evaluate a user's translation attempt across 4 dimensions.
    pub async fn language_evaluate_translation(
        &self,
        params: EvaluateTranslationParams,
    ) -> Result<TranslationEvalResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let system = language_prompts::evaluate_translation_prompt(
            &params.source_lang,
            &params.target_lang,
        );
        let user_prompt = format!(
            "Source text ({}):\n{}\n\nStudent's translation ({}):\n{}",
            params.source_lang, params.source_text, params.target_lang, params.user_translation
        );
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let cleaned = common::strip_llm_fences(&text);
        serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse: {e}")))
    }

    /// Save vocabulary words as flashcards + semantic facts.
    pub async fn language_save_vocabulary(
        &self,
        params: VocabularySaveParams,
    ) -> Result<Vec<desktop_shared::commands::notes::FlashcardResponse>, ApiError> {
        let flashcard_repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());

        let now = chrono::Utc::now().to_rfc3339();
        let mut new_cards = Vec::new();

        for item in &params.words {
            let vocab_data = serde_json::json!({
                "word": item.word,
                "reading": item.reading,
                "meaning": item.meaning,
                "example_sentence": item.example_sentence,
                "part_of_speech": item.part_of_speech,
            });

            new_cards.push(cognitive::types::NewFlashcard {
                source_note_id: params.note_id.clone(),
                source_context: item.example_sentence.clone(),
                deck: params.deck.clone(),
                front: item.word.clone(),
                back: item.meaning.clone(),
                card_type: "vocabulary".to_string(),
                cloze_data: None,
                vocab_data: Some(vocab_data),
                image_data: None,
                tags: vec!["vocabulary".to_string(), "language-learning".to_string()],
            });

            // Also save as SemanticFact
            let fact_id = uuid::Uuid::new_v4().to_string();
            let fact = SemanticFact {
                id: fact_id,
                domain: "learning".to_string(),
                subject: item.word.clone(),
                predicate: "meaning".to_string(),
                object: item.meaning.clone(),
                confidence: 1.0,
                source: format!("note:{}", params.note_id.as_deref().unwrap_or("manual")),
                valid_from: now.clone(),
                valid_until: None,
                recorded_at: now.clone(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                memory_type: "vocabulary".to_string(),
                scope_type: "system".to_string(),
                scope_id: None,
            };
            sf_repo.upsert(&fact).await.map_err(map_cognitive_err)?;
        }

        flashcard_repo
            .create_batch(new_cards)
            .await
            .map_err(map_cognitive_err)?;

        // Return the created flashcards as responses
        let cards = flashcard_repo
            .list_all_in_deck(&params.deck, params.words.len() as i64, 0)
            .await
            .map_err(map_cognitive_err)?;

        Ok(cards
            .into_iter()
            .map(|c| super::flashcard::flashcard_to_response(&c))
            .collect())
    }

    /// Detect confusable words by checking existing vocabulary.
    pub async fn language_detect_confusables(
        &self,
        params: DetectConfusablesParams,
    ) -> Result<ConfusableResponse, ApiError> {
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
        let similar = sf_repo
            .find_similar_vocabulary(&params.word, 5)
            .await
            .map_err(map_cognitive_err)?;

        if similar.is_empty() {
            return Ok(ConfusableResponse {
                has_confusable: false,
                confusable_word: None,
                confusable_meaning: None,
                explanation: None,
            });
        }

        let confusable = &similar[0];

        // Use LLM to explain the difference
        let provider = match self.cognitive_provider.as_ref() {
            Some(p) => p,
            None => {
                return Ok(ConfusableResponse {
                    has_confusable: true,
                    confusable_word: Some(confusable.subject.clone()),
                    confusable_meaning: Some(confusable.object.clone()),
                    explanation: None,
                });
            }
        };

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let system = language_prompts::detect_confusables_prompt(&params.source_lang);
        let user_prompt = format!(
            "Word 1: {} ({})\nWord 2: {} ({})",
            params.word, "new word", confusable.subject, confusable.object
        );
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response.content.unwrap_or_default();
        let cleaned = common::strip_llm_fences(&text);

        // Parse the explanation from LLM
        let explanation: Option<String> = serde_json::from_str::<serde_json::Value>(cleaned)
            .ok()
            .and_then(|v| v.get("explanation").and_then(|e| e.as_str().map(String::from)));

        Ok(ConfusableResponse {
            has_confusable: true,
            confusable_word: Some(confusable.subject.clone()),
            confusable_meaning: Some(confusable.object.clone()),
            explanation,
        })
    }

    /// Enrich an annotation with language data (translation + word breakdown).
    pub async fn language_enrich_annotation(
        &self,
        params: EnrichAnnotationParams,
    ) -> Result<AnnotationEnrichmentResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let system = language_prompts::enrich_annotation_prompt(
            &params.source_lang,
            &params.target_lang,
        );
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(params.quoted_text),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response"))?;

        let cleaned = common::strip_llm_fences(&text);
        serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse: {e}")))
    }
}
```

**Note:** The `flashcard_to_response` function is referenced from `super::flashcard` — check that it's `pub(crate)` accessible. If not, make it public within the notes handlers module.

- [ ] **Step 2: Build to verify**

Run: `cargo build -p app-core`
Expected: Compiles (may need adjustments for imports — `providers` and `common` need to be in scope via `use` statements at the top of the file).

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/notes/language.rs
git commit -m "feat(app-core): add language learning handlers (translate, evaluate, save, confusables, enrich)"
```

---

### Task 6: Backend — Tauri IPC Commands + Dev Server Wiring

**Files:**
- Create: `crates/desktop/src/commands/language.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Create language.rs Tauri commands**

Create `crates/desktop/src/commands/language.rs`:

```rust
use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, TranslateBreakdownParams,
    TranslateBreakdownResponse, TranslationEvalResponse, VocabularySaveParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn language_translate_breakdown(
    state: State<'_, Arc<AppCore>>,
    params: TranslateBreakdownParams,
) -> Result<TranslateBreakdownResponse, ApiError> {
    state.language_translate_breakdown(params).await
}

#[tauri::command]
pub async fn language_evaluate_translation(
    state: State<'_, Arc<AppCore>>,
    params: EvaluateTranslationParams,
) -> Result<TranslationEvalResponse, ApiError> {
    state.language_evaluate_translation(params).await
}

#[tauri::command]
pub async fn language_save_vocabulary(
    state: State<'_, Arc<AppCore>>,
    params: VocabularySaveParams,
) -> Result<serde_json::Value, ApiError> {
    let cards = state.language_save_vocabulary(params).await?;
    Ok(serde_json::to_value(cards).unwrap_or_default())
}

#[tauri::command]
pub async fn language_detect_confusables(
    state: State<'_, Arc<AppCore>>,
    params: DetectConfusablesParams,
) -> Result<ConfusableResponse, ApiError> {
    state.language_detect_confusables(params).await
}

#[tauri::command]
pub async fn language_enrich_annotation(
    state: State<'_, Arc<AppCore>>,
    params: EnrichAnnotationParams,
) -> Result<AnnotationEnrichmentResponse, ApiError> {
    state.language_enrich_annotation(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "language_translate_breakdown",
    "language_evaluate_translation",
    "language_save_vocabulary",
    "language_detect_confusables",
    "language_enrich_annotation",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "language_translate_breakdown" => dev::val(
            core.language_translate_breakdown(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_evaluate_translation" => dev::val(
            core.language_evaluate_translation(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_save_vocabulary" => dev::val(
            core.language_save_vocabulary(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_detect_confusables" => dev::val(
            core.language_detect_confusables(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_enrich_annotation" => dev::val(
            core.language_enrich_annotation(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register module in commands/mod.rs**

In `crates/desktop/src/commands/mod.rs`, add `pub mod language;`

- [ ] **Step 3: Add dispatch chain in dispatch.rs**

In `crates/desktop/src/dev_server/dispatch.rs`, add after the `annotations` entry:

```rust
if let Some(r) = commands::language::dispatch_dev(cmd, core, &body).await {
    return into_api_result(r);
}
```

- [ ] **Step 4: Add DEV_COMMANDS to test in mod.rs**

In `crates/desktop/src/dev_server/mod.rs`, add to the modules array (after `commands::annotations::DEV_COMMANDS,`):

```rust
commands::language::DEV_COMMANDS,
```

- [ ] **Step 5: Register commands in main.rs**

In `crates/desktop/src/main.rs`, add after the Annotations section:

```rust
// Language Learning
commands::language::language_translate_breakdown,
commands::language::language_evaluate_translation,
commands::language::language_save_vocabulary,
commands::language::language_detect_confusables,
commands::language::language_enrich_annotation,
```

- [ ] **Step 6: Build and test**

Run: `cargo build -p desktop`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: All pass — the DEV_COMMANDS test confirms parity.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/language.rs crates/desktop/src/commands/mod.rs crates/desktop/src/dev_server/dispatch.rs crates/desktop/src/dev_server/mod.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): wire language learning IPC commands + dev server"
```

---

## Phase 0: Frontend Foundation

### Task 7: Frontend — useLanguageBreakdown Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useLanguageBreakdown.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface WordBreakdown {
  word: string;
  reading: string | null;
  meaning: string;
  partOfSpeech: string;
  proficiencyLevel: string | null;
  exampleSentence: string | null;
  isNew: boolean;
}

export interface GrammarPattern {
  pattern: string;
  explanation: string;
  patternType: string | null;
}

export interface TranslateBreakdownResponse {
  translation: string;
  words: WordBreakdown[];
  grammarPatterns: GrammarPattern[];
}

export function useLanguageBreakdown() {
  const [result, setResult] = useState<TranslateBreakdownResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const translate = useCallback(
    async (text: string, sourceLang: string, targetLang: string) => {
      setLoading(true);
      setError(null);
      try {
        const response = await ipc<TranslateBreakdownResponse>(
          "language_translate_breakdown",
          { params: { text, sourceLang, targetLang } },
        );
        setResult(response);
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Translation failed";
        setError(msg);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
  }, []);

  return { result, loading, error, translate, reset };
}
```

- [ ] **Step 2: Build to verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles (hook is not yet used).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useLanguageBreakdown.ts
git commit -m "feat(ui): add useLanguageBreakdown hook for AI translation"
```

---

### Task 8: Frontend — useVocabularySave Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useVocabularySave.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";
import type { WordBreakdown } from "./useLanguageBreakdown";

export function useVocabularySave() {
  const [saving, setSaving] = useState(false);
  const [savedCount, setSavedCount] = useState<number | null>(null);

  const saveWords = useCallback(
    async (words: WordBreakdown[], noteId: string | null, deck: string) => {
      setSaving(true);
      try {
        const vocabItems = words.map((w) => ({
          word: w.word,
          reading: w.reading,
          meaning: w.meaning,
          partOfSpeech: w.partOfSpeech,
          exampleSentence: w.exampleSentence,
        }));
        await ipc("language_save_vocabulary", {
          params: { words: vocabItems, noteId, deck },
        });
        setSavedCount(words.length);
        invalidateQueries("flashcard_");
        // Auto-clear saved count after 5 seconds
        setTimeout(() => setSavedCount(null), 5000);
      } catch {
        // Silently fail — vocab save is non-critical
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  const dismissSaved = useCallback(() => setSavedCount(null), []);

  return { saving, savedCount, saveWords, dismissSaved };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useVocabularySave.ts
git commit -m "feat(ui): add useVocabularySave hook for batch vocabulary persistence"
```

---

### Task 9: Frontend — CollapsibleSection + LanguageLearningPanel Shell

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/language/CollapsibleSection.tsx`
- Create: `desktop-ui/src/features/notes/components/editor/language/TranslationSection.tsx`
- Create: `desktop-ui/src/features/notes/components/editor/language/WordsSection.tsx`
- Create: `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx`

- [ ] **Step 1: Create language/ directory**

Run: `mkdir -p desktop-ui/src/features/notes/components/editor/language`

- [ ] **Step 2: Create CollapsibleSection wrapper**

A shared component for collapsible sections with consistent styling:

```typescript
// desktop-ui/src/features/notes/components/editor/language/CollapsibleSection.tsx
import { type ReactNode, useCallback, useState } from "react";

interface CollapsibleSectionProps {
  title: string;
  defaultExpanded?: boolean;
  badge?: ReactNode;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  defaultExpanded = false,
  badge,
  children,
}: CollapsibleSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  const toggle = useCallback(() => setExpanded((prev) => !prev), []);

  return (
    <div className="border-b border-border">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={expanded}
        className="flex w-full items-center justify-between px-3 py-2 text-[10px] text-muted-foreground uppercase tracking-wider hover:bg-surface-hover transition-colors"
      >
        <span className="flex items-center gap-2">
          {title}
          {badge}
        </span>
        <span className="text-[10px]">{expanded ? "▾" : "▸"}</span>
      </button>
      {expanded && <div className="px-3 pb-3">{children}</div>}
    </div>
  );
}
```

- [ ] **Step 3: Create TranslationSection**

```typescript
// desktop-ui/src/features/notes/components/editor/language/TranslationSection.tsx
interface TranslationSectionProps {
  translation: string | null;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}

export function TranslationSection({ translation, loading, error, onRetry }: TranslationSectionProps) {
  return (
    <div className="border-b border-border px-3 py-3">
      <div className="text-[10px] text-muted-foreground uppercase tracking-wider mb-2">Translation</div>
      {loading && (
        <div className="space-y-2">
          <div className="h-4 bg-surface-hover rounded animate-pulse" />
          <div className="h-4 bg-surface-hover rounded animate-pulse w-3/4" />
        </div>
      )}
      {error && (
        <div className="text-xs text-red-400">
          {error}{" "}
          <button type="button" onClick={onRetry} className="text-brand underline">Retry</button>
        </div>
      )}
      {translation && !loading && (
        <div className="rounded-md border-l-2 border-brand bg-surface-hover/50 px-3 py-2">
          <p className="text-sm text-primary leading-relaxed">{translation}</p>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create WordsSection**

```typescript
// desktop-ui/src/features/notes/components/editor/language/WordsSection.tsx
import type { WordBreakdown } from "../../../hooks/useLanguageBreakdown";

interface WordsSectionProps {
  words: WordBreakdown[];
  onSaveWords: (words: WordBreakdown[]) => void;
  saving: boolean;
}

export function WordsSection({ words, onSaveWords, saving }: WordsSectionProps) {
  const newWords = words.filter((w) => w.isNew);

  return (
    <div className="border-b border-border px-3 py-3">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Words ({words.length})
        </span>
        {newWords.length > 0 && (
          <button
            type="button"
            onClick={() => onSaveWords(newWords)}
            disabled={saving}
            className="rounded-md bg-brand px-2.5 py-1 text-[10px] font-semibold text-black hover:bg-brand/90 disabled:opacity-50"
          >
            {saving ? "Saving..." : `Save ${newWords.length} new word${newWords.length !== 1 ? "s" : ""}`}
          </button>
        )}
      </div>
      <div className="space-y-1">
        {words.map((word) => (
          <WordRow key={word.word} word={word} />
        ))}
      </div>
    </div>
  );
}

function WordRow({ word }: { word: WordBreakdown }) {
  return (
    <div className="flex items-center justify-between py-1.5 border-b border-border/50 last:border-0">
      <div className="flex items-center gap-2">
        <span className="text-xs text-primary font-medium">{word.word}</span>
        {word.reading && (
          <span className="text-[10px] text-muted">{word.reading}</span>
        )}
        {word.isNew && (
          <span className="text-[9px] text-brand">★ new</span>
        )}
      </div>
      <div className="flex items-center gap-2">
        {word.proficiencyLevel && (
          <span className="rounded-full bg-purple-500/15 px-1.5 py-0.5 text-[9px] text-purple-400">
            {word.proficiencyLevel}
          </span>
        )}
        <span className="text-xs text-muted">{word.meaning}</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Create LanguageLearningPanel (main container)**

```typescript
// desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx
import { useEffect } from "react";
import { useLanguageBreakdown } from "../../hooks/useLanguageBreakdown";
import { useVocabularySave } from "../../hooks/useVocabularySave";
import { TranslationSection } from "./language/TranslationSection";
import { WordsSection } from "./language/WordsSection";
import { CollapsibleSection } from "./language/CollapsibleSection";

interface LanguageLearningPanelProps {
  noteId: string;
  noteTitle: string;
  sourceText: string;
  sourceLang: string;
  targetLang: string;
}

export function LanguageLearningPanel({
  noteId,
  noteTitle,
  sourceText,
  sourceLang,
  targetLang,
}: LanguageLearningPanelProps) {
  const { result, loading, error, translate } = useLanguageBreakdown();
  const { saving, savedCount, saveWords, dismissSaved } = useVocabularySave();

  // Auto-translate on mount or when source text changes
  useEffect(() => {
    if (sourceText.trim().length > 5) {
      translate(sourceText, sourceLang, targetLang);
    }
  }, [sourceText, sourceLang, targetLang, translate]);

  const handleSaveWords = (words: typeof result.words) => {
    saveWords(words, noteId, noteTitle);
  };

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      {/* Save feedback snackbar */}
      {savedCount !== null && (
        <div className="mx-3 mt-2 flex items-center justify-between rounded-md bg-green-500/10 px-3 py-2 text-xs text-green-400">
          <span>Saved {savedCount} word{savedCount !== 1 ? "s" : ""} to "{noteTitle}"</span>
          <button type="button" onClick={dismissSaved} className="text-green-300 hover:text-green-200">✕</button>
        </div>
      )}

      {/* Section 1: Translation (always expanded) */}
      <TranslationSection
        translation={result?.translation ?? null}
        loading={loading}
        error={error}
        onRetry={() => translate(sourceText, sourceLang, targetLang)}
      />

      {/* Section 2: Words (always expanded) */}
      {result && (
        <WordsSection
          words={result.words}
          onSaveWords={handleSaveWords}
          saving={saving}
        />
      )}

      {/* Section 3: Grammar (collapsed by default) */}
      {result && result.grammarPatterns.length > 0 && (
        <CollapsibleSection title="Grammar Patterns">
          <div className="space-y-2">
            {result.grammarPatterns.map((gp, i) => (
              <div key={i} className="rounded-md border border-blue-500/20 bg-blue-500/5 p-2">
                <p className="text-xs font-mono text-blue-300">{gp.pattern}</p>
                <p className="mt-1 text-xs text-muted">{gp.explanation}</p>
                {gp.patternType && (
                  <span className="mt-1 inline-block rounded-full bg-blue-500/15 px-1.5 py-0.5 text-[9px] text-blue-400">
                    {gp.patternType}
                  </span>
                )}
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}

      {/* Sections 4 & 5 (Practice + Confusables) — P1, placeholder for now */}
    </div>
  );
}
```

- [ ] **Step 6: Build and verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/language/ desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx
git commit -m "feat(ui): add LanguageLearningPanel with Translation + Words + Grammar sections"
```

---

### Task 10: Frontend — SplitEditor Integration

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

- [ ] **Step 1: Import LanguageLearningPanel**

Add import at the top of SplitEditor.tsx:

```typescript
import { LanguageLearningPanel } from "./LanguageLearningPanel";
```

- [ ] **Step 2: Replace right pane in translation mode**

In the right pane conditional rendering (where `splitMode === "annotation"` renders AnnotationSidebar), add a case for `splitMode === "translation"`:

```typescript
{splitMode === "translation" ? (
  <LanguageLearningPanel
    noteId={note.id}
    noteTitle={note.title}
    sourceText={leftContentRef.current.markdown || leftContentRef.current.html}
    sourceLang="zh"
    targetLang="en"
  />
) : splitMode === "annotation" ? (
  <AnnotationSidebar ... />
) : (
  <>
    <div className="px-3 py-1.5 ...">
      {rightLabel}
    </div>
    <EditorContentWrapper editor={rightEditor} ... />
  </>
)}
```

**Note:** For now, hardcode `sourceLang="zh"` and `targetLang="en"` — Task 11 (useLanguageConfig) will make this dynamic. The key is that the right pane now shows the AI panel instead of a plain editor in translation mode.

- [ ] **Step 3: Build and manually test**

Run: `cd desktop-ui && bun run dev`
Open a note → click "Translate" in SplitToolbar → right pane should show the LanguageLearningPanel with AI translation + word breakdown.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/SplitEditor.tsx
git commit -m "feat(ui): wire LanguageLearningPanel into SplitEditor translation mode"
```

---

### Task 11: Frontend — useLanguageConfig Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useLanguageConfig.ts`
- Modify: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

- [ ] **Step 1: Create useLanguageConfig**

```typescript
import { useQuery } from "@shared/hooks/useQuery";
import { useCallback, useMemo } from "react";

interface LanguageConfigResponse {
  sourceLang: string | null;
  targetLang: string | null;
  autoDetect: boolean;
  proficiencyLevel: string | null;
}

const DEFAULT_CONFIG: LanguageConfigResponse = {
  sourceLang: null,
  targetLang: null,
  autoDetect: true,
  proficiencyLevel: null,
};

/** Detect likely source language from text using Unicode script detection. */
function detectLanguage(text: string): string {
  const cjkPattern = /[\u2E80-\u9FFF\uF900-\uFAFF]/;
  const japanesePattern = /[\u3040-\u309F\u30A0-\u30FF]/;
  if (japanesePattern.test(text)) return "ja";
  if (cjkPattern.test(text)) return "zh";
  return "en";
}

export function useLanguageConfig(
  noteId: string | null,
  perspectiveConfig: string | null | undefined,
  sourceText?: string,
) {
  const { data: settings } = useQuery<LanguageConfigResponse>(
    "config_get_section",
    { section: "language" },
    DEFAULT_CONFIG,
  );

  // Check for per-note override in perspectiveConfig
  const noteOverride = useMemo(() => {
    if (!perspectiveConfig) return null;
    try {
      const config = JSON.parse(perspectiveConfig);
      return config.languagePair ?? null;
    } catch {
      return null;
    }
  }, [perspectiveConfig]);

  // Resolution order: per-note → global → auto-detect
  const sourceLang = useMemo(() => {
    if (noteOverride?.sourceLang) return noteOverride.sourceLang;
    if (settings?.sourceLang) return settings.sourceLang;
    if (sourceText) return detectLanguage(sourceText);
    return "zh";
  }, [noteOverride, settings, sourceText]);

  const targetLang = useMemo(() => {
    if (noteOverride?.targetLang) return noteOverride.targetLang;
    if (settings?.targetLang) return settings.targetLang;
    return "en";
  }, [noteOverride, settings]);

  return { sourceLang, targetLang, proficiencyLevel: settings?.proficiencyLevel ?? null };
}
```

- [ ] **Step 2: Wire into SplitEditor**

Replace the hardcoded `sourceLang="zh"` / `targetLang="en"` in the LanguageLearningPanel props with values from `useLanguageConfig`. Import the hook and call it in SplitEditor, passing the result to LanguageLearningPanel.

- [ ] **Step 3: Build and verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useLanguageConfig.ts desktop-ui/src/features/notes/components/editor/SplitEditor.tsx
git commit -m "feat(ui): add useLanguageConfig hook with global pair + per-note override + auto-detect"
```

---

## Phase 1: Practice + Grammar + Confusables

### Task 12: Frontend — PracticeSection Component

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useTranslationPractice.ts`
- Create: `desktop-ui/src/features/notes/components/editor/language/PracticeSection.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx`

- [ ] **Step 1: Create useTranslationPractice hook**

Hook that manages practice input state, calls `language_evaluate_translation`, and stores grades:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface TranslationEvalResponse {
  grades: { meaning: string; grammar: string; naturalness: string; wordChoice: string };
  corrections: Array<{ original: string; suggested: string; explanation: string; category: string }>;
  modelTranslation: string;
}

export function useTranslationPractice() {
  const [evaluation, setEvaluation] = useState<TranslationEvalResponse | null>(null);
  const [evaluating, setEvaluating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const evaluate = useCallback(
    async (sourceText: string, userTranslation: string, sourceLang: string, targetLang: string) => {
      setEvaluating(true);
      setError(null);
      try {
        const response = await ipc<TranslationEvalResponse>("language_evaluate_translation", {
          params: { sourceText, userTranslation, sourceLang, targetLang },
        });
        setEvaluation(response);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : "Evaluation failed");
      } finally {
        setEvaluating(false);
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setEvaluation(null);
    setError(null);
  }, []);

  return { evaluation, evaluating, error, evaluate, reset };
}
```

- [ ] **Step 2: Create PracticeSection component**

The Hybrid C format with letter grades + expandable corrections. See spec for exact layout.

- [ ] **Step 3: Add PracticeSection to LanguageLearningPanel**

Add below the Grammar section as a `CollapsibleSection` with title "Practice" and the grade summary bar as badge when collapsed.

- [ ] **Step 4: Build and verify**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useTranslationPractice.ts desktop-ui/src/features/notes/components/editor/language/PracticeSection.tsx desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx
git commit -m "feat(ui): add Practice section with Hybrid C evaluation (grades + expandable corrections)"
```

---

### Task 13: Frontend — ConfusableSection Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/language/ConfusableSection.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx`

- [ ] **Step 1: Create ConfusableSection**

Conditional component that queries `language_detect_confusables` for each new word and displays alerts.

- [ ] **Step 2: Wire into LanguageLearningPanel**

Add at the bottom of the panel, only rendered when confusable pairs are detected.

- [ ] **Step 3: Build and verify**

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/language/ConfusableSection.tsx desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx
git commit -m "feat(ui): add ConfusableSection for similar-word detection alerts"
```

---

## Phase 2: Annotation Enrichment

### Task 14: Frontend — Smart Annotation Enrichment

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/AnnotationSidebar.tsx`

- [ ] **Step 1: Add language enrichment to AnnotationCard**

When `quotedText` contains foreign-language text (detected via Unicode script), call `language_enrich_annotation` IPC. Cache result in `ai_suggestion` field. Display: translation + word chips with HSK levels + "Save N new words" button.

- [ ] **Step 2: Add enrichment cache check**

Only call LLM if `ai_suggestion` is null or doesn't contain an `enrichment` key. Use a sequential queue (max 1 concurrent enrichment call) to avoid thundering herd.

- [ ] **Step 3: Build and manually test**

Open a note with Chinese annotations → Annotate mode → annotation cards should show language enrichment.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/AnnotationSidebar.tsx
git commit -m "feat(ui): add smart language enrichment to annotation cards"
```

---

### Task 15: Final — Lint, Format, Test

- [ ] **Step 1: Rust checks**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

Fix any warnings or formatting issues.

- [ ] **Step 2: Frontend checks**

```bash
cd desktop-ui && bun run lint:fix
```

- [ ] **Step 3: Run test suite**

```bash
cargo nextest run -p config -p cognitive -p app-core -p desktop-shared
cargo nextest run -p desktop -E 'test(dev_server_covers)'
cd desktop-ui && bun run build
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: lint and format for language learning surface"
```
