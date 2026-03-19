# Layer 4: Feature Learning (`crates/feature-learning/`)

## Overview

The `feature-learning` crate provides LLM-powered flashcard generation from note content. It is a lightweight crate (3 source files) that handles prompt construction and response parsing for spaced repetition card creation. The actual LLM call and flashcard persistence are handled by higher layers.

## Dependencies

- `common`
- External: `serde`, `serde_json`, `tracing`

Note: This is one of the lightest feature crates -- no `tools-core`, `storage`, or `async-trait` dependencies.

## Module Organization

```
crates/feature-learning/src/
  lib.rs               # Re-exports
  types.rs             # GeneratedCard, CardGenerationContext
  card_generator.rs    # Prompt building + response parsing
```

## Key Types (`types.rs`)

### GeneratedCard
```rust
pub struct GeneratedCard {
    pub front: String,
    pub back: String,
    pub card_type: String,        // "basic", "cloze", "vocabulary"
    pub tags: Vec<String>,
    pub source_context: Option<String>,  // excerpt from source note
    pub cloze_data: Option<Value>,       // cloze-specific data
    pub vocab_data: Option<Value>,       // vocabulary-specific data (word, reading, meaning, example)
}
```

### CardGenerationContext
```rust
pub struct CardGenerationContext {
    pub note_content: String,
    pub note_title: String,
    pub existing_cards_summary: Option<String>,  // prevents duplicate generation
}
```

## Public API

### `build_generation_prompt(ctx) -> (system_prompt, user_prompt)`

Constructs the LLM prompt pair for flashcard generation. The system prompt specifies:
- Generate 5-15 cards depending on content density
- Three card types: basic (concept Q&A), cloze (fill-in-blank with `{{c1::hidden}}` syntax), vocabulary (with vocab_data)
- Cards must be self-contained (understandable without source note)
- Include `source_context` excerpts for every card
- Tags: 1-3 lowercase hyphenated concepts
- JSON-only response (no markdown fences)

If `existing_cards_summary` is provided, it is prepended to prevent duplicate generation.

### `parse_generated_cards(response) -> Result<Vec<GeneratedCard>, String>`

Parses the LLM JSON response with tolerance for common LLM quirks:
- Strips markdown fences via `common::helpers::strip_llm_fences`
- Filters out cards with empty front/back fields
- Returns descriptive error on parse failure

### `summarize_existing_cards(cards) -> String`

Produces a compact summary of existing flashcards for dedup context. Used to populate `CardGenerationContext::existing_cards_summary`.

## Integration

This crate is consumed by the `app-core` layer which:
1. Fetches note content from `feature-notes`
2. Summarizes existing flashcards from the cognitive layer
3. Calls `build_generation_prompt()` to construct the LLM prompt
4. Sends the prompt to an LLM provider
5. Calls `parse_generated_cards()` to parse the response
6. Persists generated cards into the cognitive flashcard system
