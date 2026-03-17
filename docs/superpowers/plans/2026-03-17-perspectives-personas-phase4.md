# Phase 4: Perspectives + Personas — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a 5th "Perspectives" tab to Insight Review that generates multi-persona analysis, build persona management UI, wire DB personas into the agent runtime for explicit analysis, and add persona auto-generation.

**Architecture:** The existing `PersonaRepo` (cognitive L5) stores 6 builtins + user personas in SQLite. The insight pipeline (`insight.rs`) spawns perspectives as a parallel LLM call alongside tabs 2-4, using a dynamic prompt built from selected personas. Persona CRUD flows through app-core handlers → Tauri commands → frontend hooks. A new `AnalysisPersonaContextSource` injects DB personas into the agent's system prompt when analysis keywords are detected.

**Tech Stack:** Rust (tokio, sqlx, serde_json, providers), React (TypeScript, Tailwind v4, lucide-react)

**Spec:** `docs/superpowers/specs/2026-03-16-insight-review-design.md` (§2 Tab 5, §3 Personas, §9 Prompts, §11 Components, §12 Styling)

**Depends on:** Phases 0-3 (complete — PersonaRepo, InsightReview tabs 1-4, LLM pipeline, knowledge graph, temporal intelligence)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/handlers/notes/insight_personas.rs` | Persona CRUD handlers + auto-generation + domain extraction |
| `crates/agent/src/context_sources/analysis_persona.rs` | AnalysisPersonaContextSource — injects DB personas for analysis queries |
| `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx` | Perspectives tab — renders persona cards |
| `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx` | Single persona card with tone color + analysis content |
| `desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx` | Swap/select persona dropdown |
| `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx` | Full persona CRUD modal (list, create, edit, toggle, delete) |
| `desktop-ui/src/features/notes/hooks/usePersonas.ts` | Persona management hook (list, create, update, delete, toggle, pins) |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/repos/persona.rs` | Add `list_all()`, `update()`, `update_relevance()` methods + `PersonaUpdate` struct |
| `crates/cognitive/src/lib.rs` | Re-export `PersonaRepo`, `PersonaRow`, `NewPersona` |
| `crates/cognitive/src/repos/insight_cache.rs` | Extend `upsert()` with `perspectives` + `persona_ids` params |
| `crates/app-core/src/state.rs` | Add `persona_repo: Option<cognitive::PersonaRepo>` field |
| `crates/app-core/src/init/mod.rs` | Init `persona_repo` during AppCore assembly |
| `crates/app-core/src/handlers/notes/mod.rs` | Register `insight_personas` module |
| `crates/app-core/src/handlers/notes/insight_prompts.rs` | Add `perspectives_prompt()` |
| `crates/app-core/src/handlers/notes/insight.rs` | Wire perspectives into pipeline, update `cache_get`, update `regenerate_tab` |
| `crates/desktop-shared/src/commands/notes.rs` | Add `PersonaResponse`, persona param DTOs, extend `InsightReviewResponse` |
| `crates/desktop/src/commands/notes.rs` | Add 7 persona Tauri commands + DEV_COMMANDS + dispatch_dev arms |
| `crates/desktop/src/main.rs` | Register persona commands in `generate_handler!` |
| `crates/agent/src/context_sources/mod.rs` | Register `analysis_persona` module |
| `crates/agent/src/agent_loop/builder.rs` | Wire `AnalysisPersonaContextSource` into context engine |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add `"perspectives"` to `TabId`, add perspectives tab state + events |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Add 5th tab + gear icon for persona management |

---

## Chunk 1: Backend — PersonaRepo Completion + Perspectives Pipeline

### Task 1: PersonaRepo Additions + Re-export + AppCore State

**Files:**
- Modify: `crates/cognitive/src/repos/persona.rs`
- Modify: `crates/cognitive/src/lib.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add PersonaUpdate struct and new methods to PersonaRepo**

In `crates/cognitive/src/repos/persona.rs`, add after the `NewPersona` struct (line 36):

```rust
#[derive(Debug, Clone)]
pub struct PersonaUpdate {
    pub name: Option<String>,
    pub role: Option<String>,
    pub expertise: Option<String>,
    pub perspective: Option<String>,
    pub tone: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
}
```

Then add these methods inside the `impl PersonaRepo` block, after `list_active()` (line 192):

```rust
    /// List all personas (including inactive), for the management UI.
    pub async fn list_all(&self) -> Result<Vec<PersonaRow>, sqlx::Error> {
        sqlx::query_as::<_, PersonaRow>(
            "SELECT * FROM insight_personas ORDER BY source ASC, name ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Update a non-builtin persona. Returns error for builtins.
    pub async fn update(
        &self,
        id: &str,
        updates: &PersonaUpdate,
    ) -> Result<Option<PersonaRow>, sqlx::Error> {
        let persona = self.get(id).await?;
        let Some(existing) = persona else {
            return Ok(None);
        };
        if existing.source == "builtin" {
            return Err(sqlx::Error::Protocol(
                "Cannot edit builtin persona".into(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let name = updates.name.as_deref().unwrap_or(&existing.name);
        let role = updates.role.as_deref().unwrap_or(&existing.role);
        let expertise = updates.expertise.as_deref().unwrap_or(&existing.expertise);
        let perspective = updates.perspective.as_deref().unwrap_or(&existing.perspective);
        let tone = updates.tone.as_deref().unwrap_or(&existing.tone);
        let icon = updates.icon.as_deref().unwrap_or(&existing.icon);
        let domains_json = if let Some(ref domains) = updates.domains {
            serde_json::to_string(domains).unwrap_or_else(|_| "[]".into())
        } else {
            existing.domains.clone()
        };

        sqlx::query(
            r#"
            UPDATE insight_personas
            SET name = ?1, role = ?2, expertise = ?3, perspective = ?4,
                tone = ?5, icon = ?6, domains = ?7, updated_at = ?8
            WHERE id = ?9
            "#,
        )
        .bind(name)
        .bind(role)
        .bind(expertise)
        .bind(perspective)
        .bind(tone)
        .bind(icon)
        .bind(&domains_json)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get(id).await
    }

    /// Adjust a persona's relevance score (for thumbs up/down feedback).
    pub async fn update_relevance(&self, id: &str, delta: f64) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE insight_personas
            SET relevance_score = MAX(0.0, MIN(1.0, relevance_score + ?1)),
                updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(delta)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
```

- [ ] **Step 2: Add tests for new PersonaRepo methods**

Append to the `#[cfg(test)] mod tests` block in `crates/cognitive/src/repos/persona.rs`:

```rust
    #[tokio::test]
    async fn test_list_all_includes_inactive() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        // Deactivate one
        repo.set_active("builtin-skeptic", false).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 6); // All 6 still returned
        let active = repo.list_active().await.unwrap();
        assert_eq!(active.len(), 5); // Only 5 active
    }

    #[tokio::test]
    async fn test_update_user_persona() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let custom = repo
            .create(&NewPersona {
                name: "Original".into(),
                role: "Tester".into(),
                expertise: "Testing".into(),
                perspective: "Test perspective".into(),
                tone: "neutral".into(),
                icon: "🧪".into(),
                domains: vec!["testing".into()],
            })
            .await
            .unwrap();

        let updated = repo
            .update(
                &custom.id,
                &PersonaUpdate {
                    name: Some("Updated Name".into()),
                    role: None,
                    expertise: None,
                    perspective: None,
                    tone: Some("analytical".into()),
                    icon: None,
                    domains: None,
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.role, "Tester"); // unchanged
        assert_eq!(updated.tone, "analytical"); // changed
    }

    #[tokio::test]
    async fn test_cannot_update_builtin() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let result = repo
            .update(
                "builtin-skeptic",
                &PersonaUpdate {
                    name: Some("Hacked".into()),
                    role: None,
                    expertise: None,
                    perspective: None,
                    tone: None,
                    icon: None,
                    domains: None,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_relevance() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        // Default relevance is 0.5
        let before = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((before.relevance_score - 0.5).abs() < f64::EPSILON);

        // Thumbs up (+0.1)
        repo.update_relevance("builtin-skeptic", 0.1).await.unwrap();
        let after = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((after.relevance_score - 0.6).abs() < f64::EPSILON);

        // Clamped at 1.0
        repo.update_relevance("builtin-skeptic", 0.5).await.unwrap();
        let capped = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((capped.relevance_score - 1.0).abs() < f64::EPSILON);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -- persona`
Expected: all persona tests pass (existing + new).

- [ ] **Step 4: Re-export PersonaRepo types from cognitive crate root**

In `crates/cognitive/src/repos/mod.rs`, update the persona re-export (line 25) to include `PersonaUpdate`:

```rust
pub use persona::{NewPersona, PersonaRepo, PersonaRow, PersonaUpdate};
```

Then in `crates/cognitive/src/lib.rs`, add to the re-export block (after line 34):

```rust
pub use repos::{NewPersona, PersonaRepo, PersonaRow, PersonaUpdate};
```

- [ ] **Step 5: Add persona_repo field to AppCore**

In `crates/app-core/src/state.rs`, add after line 100 (`flashcard_repo` field):

```rust
    /// Persona repo for Insight Review personas (None when cognitive feature unavailable).
    pub persona_repo: Option<cognitive::PersonaRepo>,
```

- [ ] **Step 6: Initialize persona_repo in AppCore assembly**

In `crates/app-core/src/init/mod.rs`, in the `AppCore { ... }` struct literal (around line 242), add after the `flashcard_repo` field:

```rust
            persona_repo: Some(cognitive::PersonaRepo::new(
                storage_pool.inner().clone(),
            )),
```

- [ ] **Step 7: Build**

Run: `cargo build -p app-core`
Expected: compiles.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/repos/persona.rs crates/cognitive/src/lib.rs crates/app-core/src/state.rs crates/app-core/src/init/mod.rs
git commit -m "feat(cognitive): complete PersonaRepo API, re-export, and add to AppCore state"
```

---

### Task 2: Perspectives Prompt + Note Domain Extraction

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs`
- Modify: `crates/app-core/src/handlers/notes/insight_context.rs`

- [ ] **Step 1: Add perspectives_prompt function**

In `crates/app-core/src/handlers/notes/insight_prompts.rs`, add after the `concept_map_prompt` function:

```rust
/// Tab 5: Perspectives — multi-persona analysis.
///
/// `persona_blocks` is pre-formatted from selected PersonaRow entries.
pub fn perspectives_prompt(context: &str, persona_blocks: &str) -> String {
    format!(
        r#"You are simulating expert perspectives analyzing a user's knowledge network.
Each persona has a distinct viewpoint and expertise.

For each persona below, write a 2-3 paragraph analysis from their perspective.
They should:
- Engage directly with the content (not just summarize)
- Identify what's strong and what's weak from their viewpoint
- Offer specific recommendations or challenges
- Disagree with each other where appropriate

{persona_blocks}

Format as Markdown. For EACH persona, use exactly this structure:

## {{Persona Name}} — {{Role}}
*{{One-line perspective summary}}*

{{2-3 paragraphs of analysis}}

**Key recommendation:** {{one actionable suggestion}}

---

Repeat for each persona. Separate each persona section with `---`.

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Format a list of PersonaRow entries into prompt blocks for the perspectives prompt.
pub fn format_persona_blocks(personas: &[(String, String, String, String, String)]) -> String {
    personas
        .iter()
        .enumerate()
        .map(|(i, (name, role, expertise, perspective, tone))| {
            format!(
                "### Persona {}: {name}\nRole: {role}\nExpertise: {expertise}\nPerspective: {perspective}\nTone: {tone}",
                i + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
```

- [ ] **Step 2: Add domain extraction helper to insight_context**

In `crates/app-core/src/handlers/notes/insight_context.rs`, add after the `assemble_context` function:

```rust
/// Extract domain hints from a note's tags for persona selection.
///
/// Tags are lowercased and returned as-is — the `PersonaRepo::select_for_note`
/// method matches them against persona `domains` JSON arrays.
pub fn extract_note_domains(tags: &[String]) -> Vec<String> {
    tags.iter().map(|t| t.to_lowercase()).collect()
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_prompts.rs crates/app-core/src/handlers/notes/insight_context.rs
git commit -m "feat(app-core): add perspectives prompt template and note domain extraction"
```

---

### Task 3: Wire Perspectives into Insight Pipeline + Extend Cache Upsert

**Files:**
- Modify: `crates/cognitive/src/repos/insight_cache.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Extend cache upsert to include perspectives and persona_ids**

In `crates/cognitive/src/repos/insight_cache.rs`, replace the `upsert` method (lines 69-113) with:

```rust
    /// Insert or update a cache entry.
    ///
    /// On conflict, existing non-null values are preserved via `COALESCE` when
    /// the incoming value is `None`.  Returns the stored row after the upsert.
    pub async fn upsert(
        &self,
        note_id: &str,
        content_hash: &str,
        synthesis: Option<&str>,
        gap_analysis: Option<&str>,
        self_assessment: Option<&str>,
        concept_map: Option<&str>,
        perspectives: Option<&str>,
        persona_ids: Option<&str>,
    ) -> Result<InsightCacheRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO insight_review_cache
                (id, note_id, content_hash, version, synthesis, gap_analysis,
                 self_assessment, concept_map, perspectives, persona_ids, created_at, updated_at)
            VALUES
                (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ON CONFLICT(note_id, content_hash) DO UPDATE SET
                synthesis        = COALESCE(excluded.synthesis,        insight_review_cache.synthesis),
                gap_analysis     = COALESCE(excluded.gap_analysis,     insight_review_cache.gap_analysis),
                self_assessment  = COALESCE(excluded.self_assessment,  insight_review_cache.self_assessment),
                concept_map      = COALESCE(excluded.concept_map,      insight_review_cache.concept_map),
                perspectives     = COALESCE(excluded.perspectives,     insight_review_cache.perspectives),
                persona_ids      = COALESCE(excluded.persona_ids,      insight_review_cache.persona_ids),
                version          = insight_review_cache.version + 1,
                updated_at       = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(note_id)
        .bind(content_hash)
        .bind(synthesis)
        .bind(gap_analysis)
        .bind(self_assessment)
        .bind(concept_map)
        .bind(perspectives)
        .bind(persona_ids)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Fetch and return the stored row.
        let row = self
            .get_if_fresh(note_id, content_hash)
            .await?
            .expect("row must exist after upsert");
        Ok(row)
    }
```

- [ ] **Step 2: Fix existing upsert call sites in tests**

In the `#[cfg(test)] mod tests` block of `insight_cache.rs`, update the `upsert` calls to include the two new params:

```rust
    // In test_upsert_and_get — the upsert call:
    let row = repo
        .upsert(note_id, hash, Some("synthesis text"), None, None, None, None, None)
        .await
        .unwrap();

    // In test_update_tab — the upsert call:
    repo.upsert(note_id, hash, Some("initial synthesis"), None, None, None, None, None)
        .await
        .unwrap();
```

- [ ] **Step 3: Rewrite insight.rs to wire perspectives into the pipeline**

Replace `crates/app-core/src/handlers/notes/insight.rs` with:

```rust
use std::sync::Arc;

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::events::AppEventEmitter;
use crate::state::AppCore;

use super::{insight_context, insight_prompts};

impl AppCore {
    /// Start insight review: check cache, return initial response.
    pub async fn note_insight_review(
        &self,
        note_id: &str,
    ) -> Result<InsightReviewStarted, ApiError> {
        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        if note.body.trim().is_empty() {
            return Err(ApiError::new("VALIDATION", "Note has no content"));
        }

        // Compute content hash: SHA-256(title + body + sorted related note IDs)
        let related_ids = self.get_related_note_ids(note_id).await;
        let hash_input = format!("{}{}{}", note.title, note.body, related_ids.join(","));
        let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

        // Check cache
        if let Some(ref repo) = self.insight_cache_repo {
            if let Ok(Some(_cached)) = repo.get_if_fresh(note_id, &content_hash).await {
                return Ok(InsightReviewStarted {
                    insight_review_id: format!(
                        "ir-{}",
                        uuid::Uuid::new_v4()
                            .to_string()
                            .split('-')
                            .next()
                            .unwrap_or("0000")
                    ),
                    content_hash,
                    cached: true,
                });
            }
        }

        let insight_review_id = format!(
            "ir-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0000")
        );

        // Get the cognitive LLM provider
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?
            .clone();

        // Assemble context from note + related notes
        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        // Extract domains from note tags for persona selection
        // NoteRow doesn't carry tags — fetch them separately
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let note_domains = insight_context::extract_note_domains(&tags);

        // Select personas for perspectives tab
        let selected_personas = if let Some(ref persona_repo) = self.persona_repo {
            persona_repo
                .select_for_note(note_id, &note_domains, 4)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Read chat params from config before spawning
        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        // Clone everything needed for the background task
        let emitter = Arc::clone(&self.event_emitter);
        let cache_repo = self.insight_cache_repo.clone();
        let note_id_owned = note_id.to_string();
        let content_hash_clone = content_hash.clone();
        let context_text = ctx.text;
        let note_title = ctx.note_title;

        // Spawn background task for LLM calls + streaming events
        tokio::spawn(async move {
            run_insight_pipeline(InsightPipelineArgs {
                provider,
                emitter,
                cache_repo,
                note_id: note_id_owned,
                content_hash: content_hash_clone,
                context: context_text,
                note_title,
                params,
                personas: selected_personas,
            })
            .await;
        });

        Ok(InsightReviewStarted {
            insight_review_id,
            content_hash,
            cached: false,
        })
    }

    /// Get cached insight review for instant re-open.
    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>, ApiError> {
        let repo = match &self.insight_cache_repo {
            Some(r) => r,
            None => return Ok(None),
        };

        let cached = match repo
            .get(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        {
            Some(c) => c,
            None => return Ok(None),
        };

        let self_assessment: Option<Vec<QuizQuestion>> = cached
            .self_assessment
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        let persona_ids: Option<Vec<String>> = cached
            .persona_ids
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(Some(InsightReviewResponse {
            insight_review_id: cached.id,
            note_id: cached.note_id,
            synthesis: cached.synthesis,
            gap_analysis: cached.gap_analysis,
            self_assessment,
            concept_map: cached.concept_map,
            perspectives: cached.perspectives,
            persona_ids,
        }))
    }

    /// Save quiz questions as flashcards with FSRS init.
    pub async fn insight_save_flashcards(
        &self,
        params: InsightSaveFlashcardsParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;

        let cards: Vec<cognitive::NewFlashcard> = params
            .questions
            .iter()
            .map(|q| {
                let (stability, difficulty) = match q.difficulty.as_str() {
                    "easy" => (4.0, 0.3),
                    "hard" => (0.8, 0.7),
                    _ => (2.0, 0.5), // medium
                };
                cognitive::NewFlashcard {
                    source_note_id: Some(params.note_id.clone()),
                    insight_review_id: Some(params.insight_review_id.clone()),
                    deck: params.deck_name.clone(),
                    question: q.question.clone(),
                    answer: q.correct_answer.clone(),
                    card_type: if q.question_type == "multiple_choice" {
                        cognitive::CardType::MultipleChoice
                    } else {
                        cognitive::CardType::ShortAnswer
                    },
                    choices: q.choices.as_ref().map(|c| serde_json::json!(c)),
                    stability,
                    difficulty,
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| FlashcardResponse {
                id: r.id,
                deck: r.deck,
                question: r.question,
                answer: r.answer,
                card_type: r.card_type,
                choices: r
                    .choices
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
                stability: r.stability,
                difficulty: r.difficulty,
                due_at: r.due_at,
                state: r.state,
                review_count: r.review_count,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Regenerate a single tab.
    pub async fn note_insight_regenerate_tab(
        &self,
        note_id: &str,
        tab: &str,
    ) -> Result<TabContent, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let prompt = match tab {
            "synthesis" => insight_prompts::synthesis_prompt(&ctx.text),
            "gaps" => insight_prompts::gap_analysis_prompt(&ctx.text),
            "assessment" => insight_prompts::self_assessment_prompt(&ctx.text),
            "concept-map" => insight_prompts::concept_map_prompt(&ctx.text, &ctx.note_title),
            "perspectives" => {
                // Re-select personas for fresh regeneration
                let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
                let note_domains = insight_context::extract_note_domains(&tags);
                let personas = if let Some(ref persona_repo) = self.persona_repo {
                    persona_repo
                        .select_for_note(note_id, &note_domains, 4)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let blocks: Vec<(String, String, String, String, String)> = personas
                    .iter()
                    .map(|p| {
                        (
                            p.name.clone(),
                            p.role.clone(),
                            p.expertise.clone(),
                            p.perspective.clone(),
                            p.tone.clone(),
                        )
                    })
                    .collect();
                let persona_blocks = insight_prompts::format_persona_blocks(&blocks);
                insight_prompts::perspectives_prompt(&ctx.text, &persona_blocks)
            }
            _ => return Err(ApiError::new("VALIDATION", "Invalid tab name")),
        };

        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text("Generate the analysis now.".to_string()),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let content = response.content.unwrap_or_default();

        // Update cache if available
        if let Some(ref repo) = self.insight_cache_repo {
            let related_ids = self.get_related_note_ids(note_id).await;
            let hash_input = format!("{}{}{}", note.title, note.body, related_ids.join(","));
            let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));
            let _ = repo.update_tab(note_id, &content_hash, tab, &content).await;
        }

        Ok(TabContent {
            tab: tab.to_string(),
            content,
        })
    }

    /// Helper: get sorted related note IDs for cache hash computation.
    async fn get_related_note_ids(&self, note_id: &str) -> Vec<String> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut ids: Vec<String> = backlinks.into_iter().map(|(note, _ctx)| note.id).collect();
        ids.sort();
        ids
    }

    /// Helper: fetch full NoteRow for each backlinked note.
    async fn fetch_related_notes(&self, note_id: &str) -> Vec<feature_notes::models::NoteRow> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut notes = Vec::new();
        for (backlink_note, _ctx) in &backlinks {
            if let Ok(Some(full_note)) = self.note_repo.get_note(&backlink_note.id).await {
                notes.push(full_note);
            }
        }
        notes
    }
}

/// Bundles all data needed by the background insight pipeline task.
struct InsightPipelineArgs {
    provider: providers::DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    cache_repo: Option<cognitive::InsightCacheRepo>,
    note_id: String,
    content_hash: String,
    context: String,
    note_title: String,
    params: providers::ChatParams,
    personas: Vec<cognitive::PersonaRow>,
}

/// Run the full insight pipeline: stream synthesis, then fire tabs 2-5 in parallel.
async fn run_insight_pipeline(args: InsightPipelineArgs) {
    let InsightPipelineArgs {
        provider,
        emitter,
        cache_repo,
        note_id,
        content_hash,
        context,
        note_title,
        params,
        personas,
    } = args;

    // 1. Stream synthesis (Tab 1)
    let synthesis = stream_synthesis(&provider, &emitter, &context, &params).await;

    // 2. Build prompts for tabs 2-5
    let gaps_prompt = insight_prompts::gap_analysis_prompt(&context);
    let assessment_prompt = insight_prompts::self_assessment_prompt(&context);
    let concept_map_prompt = insight_prompts::concept_map_prompt(&context, &note_title);

    // Build perspectives prompt from selected personas
    let persona_blocks: Vec<(String, String, String, String, String)> = personas
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                p.role.clone(),
                p.expertise.clone(),
                p.perspective.clone(),
                p.tone.clone(),
            )
        })
        .collect();
    let formatted_blocks = insight_prompts::format_persona_blocks(&persona_blocks);
    let perspectives_prompt = insight_prompts::perspectives_prompt(&context, &formatted_blocks);

    // Emit persona metadata so the frontend knows which personas are being used
    let personas_meta: Vec<serde_json::Value> = personas
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "role": p.role,
                "icon": p.icon,
                "tone": p.tone,
            })
        })
        .collect();
    emitter.emit_event(
        "insight:perspectives-meta",
        serde_json::json!({ "personas": personas_meta }),
    );

    // 3. Fire tabs 2-5 in parallel
    let (gaps, assessment, concept_map, perspectives) = tokio::join!(
        generate_tab(&provider, &emitter, "gaps", &gaps_prompt, &params),
        generate_tab(
            &provider,
            &emitter,
            "assessment",
            &assessment_prompt,
            &params,
        ),
        generate_tab(
            &provider,
            &emitter,
            "concept-map",
            &concept_map_prompt,
            &params,
        ),
        generate_tab(
            &provider,
            &emitter,
            "perspectives",
            &perspectives_prompt,
            &params,
        ),
    );

    // 4. Cache all results
    if let Some(ref repo) = cache_repo {
        let persona_ids_json = serde_json::to_string(
            &personas.iter().map(|p| &p.id).collect::<Vec<_>>(),
        )
        .ok();
        let _ = repo
            .upsert(
                &note_id,
                &content_hash,
                synthesis.as_deref(),
                gaps.as_deref(),
                assessment.as_deref(),
                concept_map.as_deref(),
                perspectives.as_deref(),
                persona_ids_json.as_deref(),
            )
            .await;
    }
}

/// Stream Tab 1 (Synthesis) token-by-token, emitting chunks via events.
async fn stream_synthesis(
    provider: &providers::DynProvider,
    emitter: &Arc<dyn AppEventEmitter>,
    context: &str,
    params: &providers::ChatParams,
) -> Option<String> {
    let messages = vec![
        providers::Message::System {
            content: insight_prompts::synthesis_prompt(context),
        },
        providers::Message::User {
            content: providers::UserContent::Text("Generate the synthesis now.".to_string()),
        },
    ];

    match provider.chat_stream(&messages, None, params).await {
        Ok(mut stream) => {
            let mut full_content = String::new();
            while let Some(chunk_result) = StreamExt::next(&mut stream).await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(text) = &chunk.content {
                            full_content.push_str(text);
                            emitter.emit_event(
                                "insight:synthesis-chunk",
                                serde_json::json!({ "content": text }),
                            );
                        }
                    }
                    Err(e) => {
                        emitter.emit_event(
                            "insight:error",
                            serde_json::json!({ "tab": "synthesis", "error": e.to_string() }),
                        );
                        return None;
                    }
                }
            }
            emitter.emit_event("insight:synthesis-done", serde_json::json!({}));
            Some(full_content)
        }
        Err(e) => {
            emitter.emit_event(
                "insight:error",
                serde_json::json!({ "tab": "synthesis", "error": e.to_string() }),
            );
            None
        }
    }
}

/// Generate a non-streaming tab (gaps, assessment, concept-map, perspectives) via a single LLM call.
async fn generate_tab(
    provider: &providers::DynProvider,
    emitter: &Arc<dyn AppEventEmitter>,
    tab_name: &str,
    prompt: &str,
    params: &providers::ChatParams,
) -> Option<String> {
    let messages = vec![
        providers::Message::System {
            content: prompt.to_string(),
        },
        providers::Message::User {
            content: providers::UserContent::Text("Generate the analysis now.".to_string()),
        },
    ];

    match provider.chat(&messages, None, params).await {
        Ok(response) => {
            let content = response.content.unwrap_or_default();
            emitter.emit_event(
                "insight:tab-done",
                serde_json::json!({ "tab": tab_name, "content": content }),
            );
            Some(content)
        }
        Err(e) => {
            emitter.emit_event(
                "insight:error",
                serde_json::json!({ "tab": tab_name, "error": e.to_string() }),
            );
            None
        }
    }
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p app-core`
Expected: compiles (may fail until DTOs are updated in Task 4).

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/insight_cache.rs crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(app-core): wire perspectives into insight pipeline with persona selection"
```

---

### Task 4: DTOs — PersonaResponse + Extend InsightReviewResponse

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`

- [ ] **Step 1: Add PersonaResponse and persona param DTOs**

In `crates/desktop-shared/src/commands/notes.rs`, add after the `InsightSaveFlashcardsParams` struct (end of file):

```rust
// ── Persona Management ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub source: String,
    pub domains: Vec<String>,
    pub is_active: bool,
    pub relevance_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonaParams {
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePersonaParams {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub expertise: Option<String>,
    pub perspective: Option<String>,
    pub tone: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPersonaPinsParams {
    pub note_id: String,
    pub persona_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatePersonaParams {
    pub id: String,
    pub helpful: bool,
}
```

- [ ] **Step 2: Add perspectives and persona_ids to InsightReviewResponse**

In the same file, update `InsightReviewResponse` (around line 187) to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewResponse {
    pub insight_review_id: String,
    pub note_id: String,
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<Vec<QuizQuestion>>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
    pub persona_ids: Option<Vec<String>>,
}
```

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace`
Expected: compiles.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(desktop-shared): add persona DTOs and extend InsightReviewResponse with perspectives"
```

---

## Chunk 2: Backend — Persona CRUD Handlers + Agent Integration

### Task 5: Persona CRUD Handlers

**Files:**
- Create: `crates/app-core/src/handlers/notes/insight_personas.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

- [ ] **Step 1: Create insight_personas.rs with all persona handlers**

Create `crates/app-core/src/handlers/notes/insight_personas.rs`:

```rust
//! Persona management handlers for Insight Review.

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    /// List all personas (including inactive) for the management UI.
    pub async fn note_insight_list_personas(&self) -> Result<Vec<PersonaResponse>, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let rows = repo
            .list_all()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows.into_iter().map(persona_row_to_response).collect())
    }

    /// Create a new user-defined persona.
    pub async fn note_insight_create_persona(
        &self,
        params: CreatePersonaParams,
    ) -> Result<PersonaResponse, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let row = repo
            .create(&cognitive::NewPersona {
                name: params.name,
                role: params.role,
                expertise: params.expertise,
                perspective: params.perspective,
                tone: params.tone,
                icon: params.icon,
                domains: params.domains,
            })
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(persona_row_to_response(row))
    }

    /// Update a non-builtin persona.
    pub async fn note_insight_update_persona(
        &self,
        params: UpdatePersonaParams,
    ) -> Result<PersonaResponse, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let row = repo
            .update(
                &params.id,
                &cognitive::PersonaUpdate {
                    name: params.name,
                    role: params.role,
                    expertise: params.expertise,
                    perspective: params.perspective,
                    tone: params.tone,
                    icon: params.icon,
                    domains: params.domains,
                },
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Persona not found"))?;

        Ok(persona_row_to_response(row))
    }

    /// Delete a non-builtin persona.
    pub async fn note_insight_delete_persona(&self, id: &str) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.delete(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Toggle a persona's active state.
    pub async fn note_insight_toggle_persona(
        &self,
        id: &str,
        active: bool,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.set_active(id, active)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Set pinned personas for a note (overrides auto-selection).
    pub async fn note_insight_set_pins(
        &self,
        params: SetPersonaPinsParams,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.set_pins(&params.note_id, &params.persona_ids)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Rate a persona (thumbs up/down) — adjusts relevance score.
    pub async fn note_insight_rate_persona(
        &self,
        params: RatePersonaParams,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let delta = if params.helpful { 0.1 } else { -0.1 };
        repo.update_relevance(&params.id, delta)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }
}

/// Convert a PersonaRow to a PersonaResponse DTO.
fn persona_row_to_response(row: cognitive::PersonaRow) -> PersonaResponse {
    let domains: Vec<String> = serde_json::from_str(&row.domains).unwrap_or_default();
    PersonaResponse {
        id: row.id,
        name: row.name,
        role: row.role,
        expertise: row.expertise,
        perspective: row.perspective,
        tone: row.tone,
        icon: row.icon,
        source: row.source,
        domains,
        is_active: row.is_active == 1,
        relevance_score: row.relevance_score,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
```

- [ ] **Step 2: Register module**

In `crates/app-core/src/handlers/notes/mod.rs`, add:

```rust
mod insight_personas;
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_personas.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(app-core): add persona CRUD handlers for Insight Review"
```

---

### Task 6: Tauri Commands for Persona Management

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Add new DTO imports**

In `crates/desktop/src/commands/notes.rs`, update the `desktop_shared::commands` import block to include the new persona types:

```rust
use desktop_shared::commands::{
    BacklinkResponse, CreatePersonaParams, FlashcardResponse, HybridSearchResponse,
    InboxCreateParams, InboxItemResponse, InsightReviewResponse, InsightReviewStarted,
    InsightSaveFlashcardsParams, NoteCreateParams, NoteLinkResponse, NoteResponse,
    NoteSuggestionsResponse, NoteUpdateParams, NoteVersionResponse, NotebookCreateParams,
    NotebookResponse, NotebookUpdateParams, PersonaResponse, RatePersonaParams,
    SetPersonaPinsParams, TabContent, UpdatePersonaParams,
};
```

- [ ] **Step 2: Add persona Tauri commands**

In the same file, add after the `note_insight_regenerate_tab` command (before the `DEV_COMMANDS` block):

```rust
// ── Persona Management commands ───────────────────────────────────

#[tauri::command]
pub async fn note_insight_list_personas(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<PersonaResponse>, ApiError> {
    state.note_insight_list_personas().await
}

#[tauri::command]
pub async fn note_insight_create_persona(
    state: State<'_, Arc<AppCore>>,
    params: CreatePersonaParams,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_create_persona(params).await
}

#[tauri::command]
pub async fn note_insight_update_persona(
    state: State<'_, Arc<AppCore>>,
    params: UpdatePersonaParams,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_update_persona(params).await
}

#[tauri::command]
pub async fn note_insight_delete_persona(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.note_insight_delete_persona(&id).await
}

#[tauri::command]
pub async fn note_insight_toggle_persona(
    state: State<'_, Arc<AppCore>>,
    id: String,
    active: bool,
) -> Result<(), ApiError> {
    state.note_insight_toggle_persona(&id, active).await
}

#[tauri::command]
pub async fn note_insight_set_pins(
    state: State<'_, Arc<AppCore>>,
    params: SetPersonaPinsParams,
) -> Result<(), ApiError> {
    state.note_insight_set_pins(params).await
}

#[tauri::command]
pub async fn note_insight_rate_persona(
    state: State<'_, Arc<AppCore>>,
    params: RatePersonaParams,
) -> Result<(), ApiError> {
    state.note_insight_rate_persona(params).await
}
```

- [ ] **Step 3: Add to DEV_COMMANDS**

In the same file, add the new command names to the `DEV_COMMANDS` array:

```rust
    "note_insight_list_personas",
    "note_insight_create_persona",
    "note_insight_update_persona",
    "note_insight_delete_persona",
    "note_insight_toggle_persona",
    "note_insight_set_pins",
    "note_insight_rate_persona",
```

- [ ] **Step 4: Add dispatch_dev arms**

In the `dispatch_dev` function in the same file, add match arms for each new command:

```rust
        "note_insight_list_personas" => dev::val(core.note_insight_list_personas().await),
        "note_insight_create_persona" => {
            dev::val(core.note_insight_create_persona(try_field!(dev::parse_params(body))).await)
        }
        "note_insight_update_persona" => {
            dev::val(core.note_insight_update_persona(try_field!(dev::parse_params(body))).await)
        }
        "note_insight_delete_persona" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_insight_delete_persona(&id).await)
        }
        "note_insight_toggle_persona" => {
            let id = try_field!(dev::get_str(body, "id"));
            let active = body.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
            dev::val(core.note_insight_toggle_persona(&id, active).await)
        }
        "note_insight_set_pins" => {
            dev::val(core.note_insight_set_pins(try_field!(dev::parse_params(body))).await)
        }
        "note_insight_rate_persona" => {
            dev::val(core.note_insight_rate_persona(try_field!(dev::parse_params(body))).await)
        }
```

- [ ] **Step 5: Register commands in main.rs**

In `crates/desktop/src/main.rs`, find the `generate_handler!` macro invocation and add the 7 new commands:

```rust
notes::note_insight_list_personas,
notes::note_insight_create_persona,
notes::note_insight_update_persona,
notes::note_insight_delete_persona,
notes::note_insight_toggle_persona,
notes::note_insight_set_pins,
notes::note_insight_rate_persona,
```

- [ ] **Step 6: Build**

Run: `cargo build --workspace`
Expected: compiles.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/notes.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add persona management Tauri commands"
```

---

### Task 7: Agent Integration — AnalysisPersonaContextSource

**Files:**
- Create: `crates/agent/src/context_sources/analysis_persona.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

This context source detects analysis-oriented user messages and injects DB persona perspectives into the agent system prompt. It supplements the existing file-based `PersonaContextSource`.

- [ ] **Step 1: Create AnalysisPersonaContextSource**

Create `crates/agent/src/context_sources/analysis_persona.rs`:

```rust
//! Injects DB-based persona perspectives when the user asks for analysis.
//!
//! Detects analysis keywords in the current message and fetches personas from
//! `PersonaRepo` to provide structured perspective context in the system prompt.

use async_trait::async_trait;
use cognitive::repos::PersonaRepo;
use context_engine::source::{ContextSource, SourceContext};

/// Analysis keywords that trigger persona injection.
const ANALYSIS_KEYWORDS: &[&str] = &[
    "analyze",
    "analysis",
    "trade-off",
    "tradeoff",
    "compare",
    "advice",
    "perspective",
    "opinions",
    "weigh",
    "pros and cons",
    "evaluate",
    "assess",
    "review",
    "critique",
    "should i",
    "what do you think",
];

pub struct AnalysisPersonaContextSource {
    persona_repo: PersonaRepo,
}

impl AnalysisPersonaContextSource {
    pub fn new(persona_repo: PersonaRepo) -> Self {
        Self { persona_repo }
    }

    fn is_analysis_query(message: &str) -> bool {
        let lower = message.to_lowercase();
        ANALYSIS_KEYWORDS.iter().any(|kw| lower.contains(kw))
    }
}

#[async_trait]
impl ContextSource for AnalysisPersonaContextSource {
    fn name(&self) -> &str {
        "analysis_personas"
    }

    fn priority(&self) -> u8 {
        94 // Just below file-based PersonaContextSource (95)
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // Only activate for analysis-oriented queries
        let message = ctx.message.as_deref()?;
        if !Self::is_analysis_query(message) {
            return None;
        }

        // Fetch active personas
        let personas = self.persona_repo.list_active().await.ok()?;
        if personas.is_empty() {
            return None;
        }

        // Select up to 4 personas for diversity
        let selected: Vec<_> = personas.into_iter().take(4).collect();

        let mut output = String::from("# Analysis Perspectives\n\n");
        output.push_str(
            "The user is asking for analysis. Consider these expert perspectives:\n\n",
        );

        for p in &selected {
            output.push_str(&format!(
                "**{} ({}):** {}\n",
                p.name, p.role, p.perspective
            ));
        }

        output.push_str(
            "\nFor each perspective: 2-3 sentences of focused analysis, then synthesize a recommendation.\n",
        );

        Some(output)
    }

    fn estimated_tokens(&self) -> usize {
        300
    }
}
```

- [ ] **Step 2: Register module**

In `crates/agent/src/context_sources/mod.rs`, add:

```rust
pub mod analysis_persona;
```

- [ ] **Step 3: Wire into the agent builder**

In `crates/agent/src/agent_loop/builder.rs`, find where `PersonaContextSource` is created (around line 264) and add the `AnalysisPersonaContextSource` right after it:

```rust
            // Analysis persona context source (DB personas for analysis queries)
            if let Some(ref pool) = self.pool {
                sources.push(Box::new(
                    crate::context_sources::analysis_persona::AnalysisPersonaContextSource::new(
                        cognitive::repos::PersonaRepo::new(pool.clone()),
                    ),
                ));
            }
```

- [ ] **Step 4: Build**

Run: `cargo build -p agent`
Expected: compiles.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/context_sources/analysis_persona.rs crates/agent/src/context_sources/mod.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): add AnalysisPersonaContextSource for DB persona injection on analysis queries"
```

---

### Task 8: Persona Auto-Generation Handler

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_personas.rs`

This adds an LLM-based handler that generates a new persona tailored to the note's content domain. Called from the frontend when fewer than 2 domain-relevant personas are available.

- [ ] **Step 1: Add auto-generation method**

In `crates/app-core/src/handlers/notes/insight_personas.rs`, add this method inside the `impl AppCore` block:

```rust
    /// Auto-generate a persona based on a note's content.
    ///
    /// Uses an LLM call to create a domain-specific persona tailored to the note's
    /// topics. The persona is saved with `source: "auto"`.
    pub async fn note_insight_auto_generate_persona(
        &self,
        note_id: &str,
    ) -> Result<PersonaResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let persona_repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        // Get existing persona names to avoid duplicates
        let existing = persona_repo.list_active().await.unwrap_or_default();
        let existing_names: Vec<&str> = existing.iter().map(|p| p.name.as_str()).collect();

        // NoteRow doesn't carry tags — fetch separately
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let tags_str = tags.join(", ");

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let prompt = format!(
            r#"Generate a unique expert persona for analyzing content about the following topic.

Note title: {}
Note tags: {}
Note content (first 500 chars): {}

Existing personas (DO NOT duplicate these): {}

Return a JSON object with these exact fields:
{{
  "name": "A distinctive persona name (2-3 words, not a real person)",
  "role": "Their professional role (2-4 words)",
  "expertise": "Domain expertise description (1 sentence)",
  "perspective": "How they approach analysis (1 sentence)",
  "tone": "One of: analytical, curious, pragmatic, skeptical, inquisitive, provocative",
  "icon": "A single emoji that represents this persona",
  "domains": ["2-4 lowercase domain keywords relevant to the note"]
}}

Return ONLY the JSON object, no markdown fences, no explanation."#,
            note.title,
            tags_str,
            &note.body[..note.body.len().min(500)],
            existing_names.join(", "),
        );

        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text(
                    "Generate the persona now.".to_string(),
                ),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let content = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        // Parse the JSON response
        let generated: serde_json::Value = serde_json::from_str(content.trim())
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse persona JSON: {e}")))?;

        let new_persona = cognitive::NewPersona {
            name: generated["name"].as_str().unwrap_or("Expert").to_string(),
            role: generated["role"].as_str().unwrap_or("Domain Analyst").to_string(),
            expertise: generated["expertise"].as_str().unwrap_or("General").to_string(),
            perspective: generated["perspective"].as_str().unwrap_or("Balanced analysis").to_string(),
            tone: generated["tone"].as_str().unwrap_or("analytical").to_string(),
            icon: generated["icon"].as_str().unwrap_or("🧠").to_string(),
            domains: generated["domains"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        };

        // Save with source "auto" via PersonaRepo::create_auto
        let row = persona_repo
            .create_auto(&new_persona)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(persona_row_to_response(row))
    }
```

- [ ] **Step 2: Add create_auto() method to PersonaRepo**

In `crates/cognitive/src/repos/persona.rs`, add to the `impl PersonaRepo` block (after the `create` method):

```rust
    /// Create an auto-generated persona (source = "auto").
    pub async fn create_auto(&self, persona: &NewPersona) -> Result<PersonaRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let domains_json =
            serde_json::to_string(&persona.domains).unwrap_or_else(|_| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO insight_personas
                (id, name, role, expertise, perspective, tone, icon, source, domains,
                 is_active, relevance_score, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'auto', ?8, 1, 0.5, ?9, ?9)
            "#,
        )
        .bind(&id)
        .bind(&persona.name)
        .bind(&persona.role)
        .bind(&persona.expertise)
        .bind(&persona.perspective)
        .bind(&persona.tone)
        .bind(&persona.icon)
        .bind(&domains_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }
```

- [ ] **Step 3: Add Tauri command for auto-generation**

In `crates/desktop/src/commands/notes.rs`, add the auto-generate command:

```rust
#[tauri::command]
pub async fn note_insight_auto_generate_persona(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_auto_generate_persona(&note_id).await
}
```

Add `"note_insight_auto_generate_persona"` to `DEV_COMMANDS`, add the dispatch arm:

```rust
        "note_insight_auto_generate_persona" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_auto_generate_persona(&note_id).await)
        }
```

Register in `crates/desktop/src/main.rs` `generate_handler!`:

```rust
notes::note_insight_auto_generate_persona,
```

- [ ] **Step 4: Build**

Run: `cargo build --workspace`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_personas.rs crates/cognitive/src/repos/persona.rs crates/desktop/src/commands/notes.rs crates/desktop/src/main.rs
git commit -m "feat(app-core): add LLM-based persona auto-generation from note content"
```

---

## Chunk 3: Frontend — Perspectives Tab + Persona Management UI

### Task 9: Extend useInsightReview Hook

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`

- [ ] **Step 1: Add perspectives to TabId and state**

Update the `TabId` type (line 9):

```typescript
export type TabId = "synthesis" | "gaps" | "assessment" | "concept-map" | "perspectives";
```

Add `PersonaMeta` interface after `QuizQuestion`:

```typescript
export interface PersonaMeta {
  id: string;
  name: string;
  role: string;
  icon: string;
  tone: string;
}
```

Update `InsightReviewState.tabs` to include perspectives:

```typescript
  tabs: {
    synthesis: { status: TabStatus; content: string };
    gaps: { status: TabStatus; content: string };
    assessment: { status: TabStatus; questions: QuizQuestion[] };
    conceptMap: { status: TabStatus; mermaid: string; fallbackText: string };
    perspectives: { status: TabStatus; content: string; personas: PersonaMeta[] };
  };
```

Add `InsightReviewCachedResponse` fields:

```typescript
interface InsightReviewCachedResponse {
  insightReviewId: string;
  noteId: string;
  synthesis: string | null;
  gapAnalysis: string | null;
  selfAssessment: QuizQuestion[] | null;
  conceptMap: string | null;
  perspectives: string | null;
  personaIds: string[] | null;
}
```

Update `INITIAL_STATE` to include perspectives:

```typescript
    perspectives: { status: "idle", content: "", personas: [] },
```

- [ ] **Step 2: Add perspectives event listeners**

Add after the `insight:error` event listener:

```typescript
  useEvent<{ personas: PersonaMeta[] }>("insight:perspectives-meta", ({ personas }) => {
    setState((prev) => ({
      ...prev,
      tabs: {
        ...prev.tabs,
        perspectives: {
          ...prev.tabs.perspectives,
          personas,
        },
      },
    }));
  });
```

Update the `insight:tab-done` listener to handle perspectives:

```typescript
      } else if (tab === "perspectives") {
        tabs.perspectives = { ...tabs.perspectives, status: "done", content };
      }
```

Update the `insight:error` listener to handle perspectives:

```typescript
      } else if (tab === "perspectives") {
        tabs.perspectives = { ...tabs.perspectives, status: "error" };
      }
```

- [ ] **Step 3: Update open action for perspectives**

In the `open` callback, update the initial state set to include perspectives:

```typescript
        perspectives: { status: "loading", content: "", personas: [] },
```

In the cached path, handle perspectives:

```typescript
        tabs.perspectives = {
          status: cached.perspectives ? "done" : "idle",
          content: cached.perspectives ?? "",
          personas: [], // Persona metadata loaded separately
        };
```

In the streaming path, set perspectives status:

```typescript
        perspectives: { status: "loading", content: "", personas: [] },
```

- [ ] **Step 4: Update regenerateTab for perspectives**

In the `regenerateTab` callback, add the perspectives case:

```typescript
        } else if (tab === "perspectives") {
          tabs.perspectives = { ...tabs.perspectives, status: "loading" };
        }
```

And in the response handling:

```typescript
        } else if (response.tab === "perspectives") {
          tabs.perspectives = { ...tabs.perspectives, status: "done", content: response.content };
        }
```

- [ ] **Step 5: Build**

Run: `cd desktop-ui && bun run build`
Expected: compiles (some components not yet created, may have import errors — verify with `bun run lint:fix`).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightReview.ts
git commit -m "feat(notes-ui): extend useInsightReview hook with perspectives tab state"
```

---

### Task 10: usePersonas Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/usePersonas.ts`

- [ ] **Step 1: Create the persona management hook**

Create `desktop-ui/src/features/notes/hooks/usePersonas.ts`:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useEffect, useState } from "react";

export interface Persona {
  id: string;
  name: string;
  role: string;
  expertise: string;
  perspective: string;
  tone: string;
  icon: string;
  source: string;
  domains: string[];
  isActive: boolean;
  relevanceScore: number;
  createdAt: string;
  updatedAt: string;
}

interface CreatePersonaInput {
  name: string;
  role: string;
  expertise: string;
  perspective: string;
  tone: string;
  icon: string;
  domains: string[];
}

interface UpdatePersonaInput {
  id: string;
  name?: string;
  role?: string;
  expertise?: string;
  perspective?: string;
  tone?: string;
  icon?: string;
  domains?: string[];
}

export interface PersonaActions {
  refresh: () => Promise<void>;
  create: (input: CreatePersonaInput) => Promise<Persona>;
  update: (input: UpdatePersonaInput) => Promise<Persona>;
  remove: (id: string) => Promise<void>;
  toggle: (id: string, active: boolean) => Promise<void>;
  setPins: (noteId: string, personaIds: string[]) => Promise<void>;
  rate: (id: string, helpful: boolean) => Promise<void>;
  autoGenerate: (noteId: string) => Promise<Persona>;
}

export function usePersonas(): [Persona[], PersonaActions] {
  const [personas, setPersonas] = useState<Persona[]>([]);

  const refresh = useCallback(async () => {
    const result = await ipc<Persona[]>("note_insight_list_personas", {});
    setPersonas(result);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const create = useCallback(
    async (input: CreatePersonaInput) => {
      const result = await ipc<Persona>("note_insight_create_persona", { params: input });
      await refresh();
      return result;
    },
    [refresh],
  );

  const update = useCallback(
    async (input: UpdatePersonaInput) => {
      const result = await ipc<Persona>("note_insight_update_persona", { params: input });
      await refresh();
      return result;
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await ipc("note_insight_delete_persona", { id });
      await refresh();
    },
    [refresh],
  );

  const toggle = useCallback(
    async (id: string, active: boolean) => {
      await ipc("note_insight_toggle_persona", { id, active });
      await refresh();
    },
    [refresh],
  );

  const setPins = useCallback(async (noteId: string, personaIds: string[]) => {
    await ipc("note_insight_set_pins", { params: { noteId, personaIds } });
  }, []);

  const rate = useCallback(async (id: string, helpful: boolean) => {
    await ipc("note_insight_rate_persona", { params: { id, helpful } });
  }, []);

  const autoGenerate = useCallback(
    async (noteId: string) => {
      const result = await ipc<Persona>("note_insight_auto_generate_persona", { noteId });
      await refresh();
      return result;
    },
    [refresh],
  );

  return [personas, { refresh, create, update, remove, toggle, setPins, rate, autoGenerate }];
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/usePersonas.ts
git commit -m "feat(notes-ui): add usePersonas hook for persona management"
```

---

### Task 11: PerspectivesTab + PersonaCard Components

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx`
- Create: `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx`

- [ ] **Step 1: Create PersonaCard component**

Create `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx`:

```tsx
import { MarkdownContent } from "@features/chat/components/MarkdownContent";

interface PersonaCardProps {
  name: string;
  role: string;
  icon: string;
  tone: string;
  content: string;
}

const TONE_COLORS: Record<string, { border: string; bg: string }> = {
  direct: { border: "border-l-red-400/60", bg: "bg-red-400/10" },
  skeptical: { border: "border-l-red-400/60", bg: "bg-red-400/10" },
  practical: { border: "border-l-amber-400/60", bg: "bg-amber-400/10" },
  pragmatic: { border: "border-l-amber-400/60", bg: "bg-amber-400/10" },
  curious: { border: "border-l-purple-400/60", bg: "bg-purple-400/10" },
  inquisitive: { border: "border-l-blue-400/60", bg: "bg-blue-400/10" },
  analytical: { border: "border-l-emerald-400/60", bg: "bg-emerald-400/10" },
  provocative: { border: "border-l-orange-400/60", bg: "bg-orange-400/10" },
  formal: { border: "border-l-gray-400/60", bg: "bg-gray-400/10" },
  neutral: { border: "border-l-gray-400/60", bg: "bg-gray-400/10" },
};

function getToneColor(tone: string) {
  return TONE_COLORS[tone] ?? { border: "border-l-gray-400/60", bg: "bg-gray-400/10" };
}

export function PersonaCard({ name, role, icon, tone, content }: PersonaCardProps) {
  const colors = getToneColor(tone);

  return (
    <div
      className={`glass-card border-l-2 ${colors.border} rounded-lg p-3 space-y-2`}
    >
      {/* Header */}
      <div className="flex items-center gap-2">
        <span
          className={`w-7 h-7 rounded-full ${colors.bg} flex items-center justify-center text-sm shrink-0`}
        >
          {icon}
        </span>
        <div className="min-w-0">
          <div className="text-[12px] font-medium text-primary truncate">{name}</div>
          <div className="text-[10px] text-dim">{role}</div>
        </div>
      </div>

      {/* Analysis content */}
      <div className="text-[12px] text-secondary leading-relaxed">
        <MarkdownContent content={content} />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create PerspectivesTab component**

Create `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx`:

```tsx
import type { PersonaMeta, TabStatus } from "../../hooks/useInsightReview";
import { PersonaCard } from "./PersonaCard";

interface PerspectivesTabProps {
  status: TabStatus;
  content: string;
  personas: PersonaMeta[];
}

/** Parse the perspectives markdown into per-persona sections by splitting on `---` and `## ` headings. */
function parsePersonaSections(
  content: string,
  personas: PersonaMeta[],
): { persona: PersonaMeta; section: string }[] {
  if (!content || personas.length === 0) return [];

  // Split by horizontal rules (---) which separate persona sections
  const sections = content.split(/\n---\n/).filter((s) => s.trim().length > 0);

  return personas.map((persona, i) => ({
    persona,
    section: sections[i]?.trim() ?? "",
  }));
}

function SkeletonLoader() {
  return (
    <div className="space-y-4">
      {[1, 2, 3].map((i) => (
        <div key={i} className="glass-card rounded-lg p-3 space-y-2 animate-pulse">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-full bg-surface-low" />
            <div className="space-y-1">
              <div className="h-3 bg-surface-low rounded w-24" />
              <div className="h-2 bg-surface-low rounded w-16" />
            </div>
          </div>
          <div className="h-3 bg-surface-low rounded w-full" />
          <div className="h-3 bg-surface-low rounded w-4/5" />
          <div className="h-3 bg-surface-low rounded w-3/4" />
        </div>
      ))}
    </div>
  );
}

export function PerspectivesTab({ status, content, personas }: PerspectivesTabProps) {
  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">
        Start an insight review to see expert perspectives
      </p>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate perspectives. Try regenerating.
      </p>
    );
  }

  const sections = parsePersonaSections(content, personas);

  // Fallback: if parsing failed or no personas, render full markdown
  if (sections.length === 0 && content) {
    return (
      <div className="space-y-4">
        <div className="text-[10px] text-dim italic">
          Perspectives (persona details unavailable)
        </div>
        <div className="text-[12px] text-secondary leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {sections.map(
        ({ persona, section }) =>
          section && (
            <PersonaCard
              key={persona.id}
              name={persona.name}
              role={persona.role}
              icon={persona.icon}
              tone={persona.tone}
              content={section}
            />
          ),
      )}
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/PersonaCard.tsx desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx
git commit -m "feat(notes-ui): add PerspectivesTab and PersonaCard components"
```

---

### Task 12: ManagePersonasModal + PersonaSelector + Panel Wiring

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx`
- Create: `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

- [ ] **Step 1: Create PersonaSelector component**

Create `desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx`:

```tsx
import type { Persona } from "../../hooks/usePersonas";

interface PersonaSelectorProps {
  personas: Persona[];
  selectedIds: string[];
  onSelect: (personaId: string) => void;
}

export function PersonaSelector({ personas, selectedIds, onSelect }: PersonaSelectorProps) {
  const available = personas.filter((p) => p.isActive && !selectedIds.includes(p.id));

  if (available.length === 0) return null;

  return (
    <select
      onChange={(e) => {
        if (e.target.value) onSelect(e.target.value);
        e.target.value = "";
      }}
      className="text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-muted border border-border"
      defaultValue=""
    >
      <option value="" disabled>
        Add persona...
      </option>
      {available.map((p) => (
        <option key={p.id} value={p.id}>
          {p.icon} {p.name} — {p.role}
        </option>
      ))}
    </select>
  );
}
```

- [ ] **Step 2: Create ManagePersonasModal component**

Create `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx`:

```tsx
import { Plus, Settings2, Trash2, X } from "lucide-react";
import { useCallback, useState } from "react";
import type { Persona, PersonaActions } from "../../hooks/usePersonas";

interface ManagePersonasModalProps {
  personas: Persona[];
  actions: PersonaActions;
  onClose: () => void;
}

const TONE_OPTIONS = [
  "analytical",
  "curious",
  "pragmatic",
  "skeptical",
  "inquisitive",
  "provocative",
  "direct",
  "formal",
];

export function ManagePersonasModal({ personas, actions, onClose }: ManagePersonasModalProps) {
  const [showCreate, setShowCreate] = useState(false);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState({
    name: "",
    role: "",
    expertise: "",
    perspective: "",
    tone: "analytical",
    icon: "🧠",
    domains: "",
  });

  const handleCreate = useCallback(async () => {
    if (!form.name.trim() || !form.role.trim()) return;
    setCreating(true);
    try {
      await actions.create({
        name: form.name,
        role: form.role,
        expertise: form.expertise,
        perspective: form.perspective,
        tone: form.tone,
        icon: form.icon,
        domains: form.domains
          .split(",")
          .map((d) => d.trim().toLowerCase())
          .filter(Boolean),
      });
      setShowCreate(false);
      setForm({
        name: "",
        role: "",
        expertise: "",
        perspective: "",
        tone: "analytical",
        icon: "🧠",
        domains: "",
      });
    } finally {
      setCreating(false);
    }
  }, [form, actions]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="glass-panel w-[480px] max-h-[80vh] rounded-xl flex flex-col">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-3 border-b border-border shrink-0">
          <Settings2 size={14} className="text-purple-400" />
          <span className="text-[13px] font-medium text-primary flex-1">Manage Personas</span>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-muted hover:text-primary hover:bg-white/[0.06]"
          >
            <X size={14} />
          </button>
        </div>

        {/* Persona list */}
        <div className="flex-1 overflow-y-auto p-3 space-y-2 min-h-0">
          {personas.map((p) => (
            <div
              key={p.id}
              className="flex items-center gap-2 p-2 rounded-lg bg-white/[0.03] group"
            >
              <span className="text-sm shrink-0">{p.icon}</span>
              <div className="flex-1 min-w-0">
                <div className="text-[11px] font-medium text-primary truncate">{p.name}</div>
                <div className="text-[10px] text-dim truncate">
                  {p.role} · {p.tone}
                  {p.source === "builtin" && (
                    <span className="ml-1 text-[9px] px-1 py-px rounded bg-white/[0.06]">
                      builtin
                    </span>
                  )}
                  {p.source === "auto" && (
                    <span className="ml-1 text-[9px] px-1 py-px rounded bg-purple-400/20 text-purple-300">
                      auto
                    </span>
                  )}
                </div>
              </div>
              <label className="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={p.isActive}
                  onChange={(e) => actions.toggle(p.id, e.target.checked)}
                  className="w-3 h-3 accent-purple-400"
                />
                <span className="text-[9px] text-dim">Active</span>
              </label>
              {p.source !== "builtin" && (
                <button
                  type="button"
                  onClick={() => actions.remove(p.id)}
                  className="p-1 text-dim hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                  title="Delete persona"
                >
                  <Trash2 size={12} />
                </button>
              )}
            </div>
          ))}
        </div>

        {/* Create form */}
        {showCreate && (
          <div className="border-t border-border p-3 space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <input
                type="text"
                placeholder="Name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
              />
              <input
                type="text"
                placeholder="Role"
                value={form.role}
                onChange={(e) => setForm({ ...form, role: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
              />
            </div>
            <input
              type="text"
              placeholder="Expertise"
              value={form.expertise}
              onChange={(e) => setForm({ ...form, expertise: e.target.value })}
              className="w-full text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
            />
            <input
              type="text"
              placeholder="Perspective (how they analyze)"
              value={form.perspective}
              onChange={(e) => setForm({ ...form, perspective: e.target.value })}
              className="w-full text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
            />
            <div className="grid grid-cols-3 gap-2">
              <select
                value={form.tone}
                onChange={(e) => setForm({ ...form, tone: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
              >
                {TONE_OPTIONS.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
              <input
                type="text"
                placeholder="Icon emoji"
                value={form.icon}
                onChange={(e) => setForm({ ...form, icon: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
                maxLength={4}
              />
              <input
                type="text"
                placeholder="Domains (comma-sep)"
                value={form.domains}
                onChange={(e) => setForm({ ...form, domains: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-primary border border-border"
              />
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="text-[10px] px-3 py-1 rounded-md text-muted hover:text-primary"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleCreate}
                disabled={creating || !form.name.trim() || !form.role.trim()}
                className="text-[10px] px-3 py-1 rounded-md bg-purple-400/20 text-purple-300 hover:bg-purple-400/30 disabled:opacity-50"
              >
                {creating ? "Creating..." : "Create"}
              </button>
            </div>
          </div>
        )}

        {/* Footer */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-t border-border shrink-0">
          {!showCreate && (
            <button
              type="button"
              onClick={() => setShowCreate(true)}
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-muted hover:text-secondary hover:bg-white/[0.06]"
            >
              <Plus size={10} />
              Create Persona
            </button>
          )}
          <div className="flex-1" />
          <button
            type="button"
            onClick={onClose}
            className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-secondary hover:text-primary"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Update InsightReviewPanel with 5th tab + persona management**

Replace `crates/desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` with the updated version that:
1. Adds `"perspectives"` to the TABS array
2. Adds persona management button (gear icon) in the header
3. Renders PerspectivesTab for the perspectives tab
4. Handles getActiveContent for perspectives
5. Shows ManagePersonasModal when triggered

The key changes to the existing file:

**Import additions:**

```typescript
import { Settings2 } from "lucide-react";
import { usePersonas } from "../hooks/usePersonas";
import { ManagePersonasModal } from "./insight/ManagePersonasModal";
import { PerspectivesTab } from "./insight/PerspectivesTab";
```

**TABS array (line 28-33), add:**

```typescript
  { id: "perspectives", label: "Perspectives" },
```

**tabStatus function (line 53-64), add case:**

```typescript
    case "perspectives":
      return state.tabs.perspectives.status;
```

**Inside the component, add state + hook:**

```typescript
  const [showPersonaManager, setShowPersonaManager] = useState(false);
  const [allPersonas, personaActions] = usePersonas();
```

**In the header, add gear icon button** (before the Regenerate All button):

```typescript
        <button
          type="button"
          onClick={() => setShowPersonaManager(true)}
          className="p-1 rounded-md text-muted hover:text-primary hover:bg-white/[0.06] transition-colors"
          title="Manage Personas"
        >
          <Settings2 size={12} />
        </button>
```

**In the content area, add perspectives rendering:**

```typescript
        {state.activeTab === "perspectives" && (
          <PerspectivesTab
            status={state.tabs.perspectives.status}
            content={state.tabs.perspectives.content}
            personas={state.tabs.perspectives.personas}
          />
        )}
```

**Update getActiveContent for perspectives:**

```typescript
      case "perspectives":
        return state.tabs.perspectives.content;
```

**At the end of the component, before the closing `</div>`, add the modal:**

```typescript
      {showPersonaManager && (
        <ManagePersonasModal
          personas={allPersonas}
          actions={personaActions}
          onClose={() => setShowPersonaManager(false)}
        />
      )}
```

- [ ] **Step 4: Lint and format**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors.

- [ ] **Step 5: Build**

Run: `cd desktop-ui && bun run build`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(notes-ui): add ManagePersonasModal, PersonaSelector, and wire 5th tab into panel"
```

---

### Task 13: Final Verification

- [ ] **Step 1: Full Rust tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format Rust**

Run: `cargo fmt --all`

- [ ] **Step 4: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: builds.

- [ ] **Step 5: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: clean.

- [ ] **Step 6: Commit if needed**

```bash
cargo fmt --all
cd desktop-ui && bun run lint:fix
git add -A && git commit -m "style: format Phase 4 implementation"
```
