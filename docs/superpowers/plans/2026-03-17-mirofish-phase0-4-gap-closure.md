# MiroFish Phase 0–4 Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all verified gaps from the Phase 0–4 verification audit so the MiroFish integration architecture spec is fully implemented.

**Architecture:** 8 independent gap fixes across 4 layers: cognitive repos (Rust), consolidation service (Rust), Tauri command layer (Rust), desktop-shared IPC types (Rust), and frontend components (TypeScript/React). Each task is self-contained — no task depends on another task's output.

**Tech Stack:** Rust (sqlx, async-trait, chrono, uuid, serde), TypeScript (React, Tailwind v4 CSS tokens), Tauri IPC

---

## File Structure

| File | Action | Purpose |
|---|---|---|
| `crates/cognitive/src/repos/entity.rs` | Modify | Add `find_path()`, `get_related_entities()`, `backfill_from_note_mentions()` |
| `crates/cognitive/src/services/background.rs` | Modify | Fix contradiction detection guard + add entity type inference + relationship creation |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `backfill_from_note_mentions()` call |
| `crates/app-core/src/handlers/notes/insight_context.rs` | Modify | Enrich `extract_note_domains()` with knowledge graph entity lookup |
| `crates/desktop-shared/src/commands/notes.rs` | Modify | Add `DeckSummaryResponse`, `FlashcardReviewParams` IPC types |
| `crates/desktop/src/commands/notes.rs` | Modify | Add 3 flashcard Tauri commands (`flashcard_list_decks`, `flashcard_get_due`, `flashcard_record_review`) |
| `crates/app-core/src/handlers/notes/flashcard.rs` | Create | AppCore flashcard handler methods |
| `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx` | Modify | Add pin UI section + auto-generate button |
| `desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx` | Create | Flashcard review session component |
| `desktop-ui/src/features/notes/hooks/useFlashcards.ts` | Create | Flashcard deck listing + review session hook |

---

### Task 1: Fix Contradiction Detection Guard (Bug)

**Context:** `background.rs:440` checks `new.confidence < 0.7` — this guards on the *new* fact's confidence. The spec requires guarding on `old.confidence >= 0.7` (high-confidence old facts being contradicted are noteworthy). The current code also only fires for `new.source == "user_stated"`, but the spec says `old.source == "user_stated"`.

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:435-461`

- [ ] **Step 1: Write the failing test**

Add to the existing test module in `background.rs` (or the integration test file that tests consolidation). The test should create a high-confidence user-stated old fact, then consolidate a new fact that contradicts it with `source: "inferred"`, and verify `ContradictionDetected` fires.

```rust
#[tokio::test]
async fn contradiction_fires_on_high_confidence_old_fact() {
    // Setup: insert old fact with confidence=0.9, source="user_stated"
    // Then consolidate a new fact with same subject+predicate but different object
    // with source="inferred", confidence=0.5
    // Assert: ContradictionDetected event was published
    // (This currently fails because the code checks new.source != "user_stated")
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(contradiction_fires_on_high_confidence_old_fact)'`
Expected: FAIL — contradiction not detected because `new.source != "user_stated"` causes `continue`

- [ ] **Step 3: Fix the guard condition**

In `crates/cognitive/src/services/background.rs:438-442`, change:

```rust
// BEFORE (line 440):
if new.confidence < 0.7 || new.source != "user_stated" {
    continue;
}
if let Ok(Some(old_fact)) = repo.get(old_id).await {
    if old_fact.object != new.object
        && !is_same_session(&old_fact.recorded_at, &session_start)

// AFTER:
if let Ok(Some(old_fact)) = repo.get(old_id).await {
    if old_fact.confidence < 0.7
        || old_fact.source != "user_stated"
    {
        continue;
    }
    if old_fact.object != new.object
        && !is_same_session(&old_fact.recorded_at, &session_start)
```

Key changes:
1. Move the `repo.get(old_id)` call **before** the guard check
2. Guard on `old_fact.confidence` and `old_fact.source` instead of `new.*`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(contradiction)'`
Expected: PASS

- [ ] **Step 5: Run full workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "fix(cognitive): guard contradiction detection on old fact confidence/source, not new"
```

---

### Task 2: Add `find_path()` and `get_related_entities()` to EntityRepo

**Context:** The spec requires `find_path(from, to, max_depth)` for shortest-path traversal and `get_related_entities(entity_id, rel_type)` for filtered relationship queries. Neither exists. The existing `get_neighborhood()` at entity.rs:232 and `get_relationships()` at entity.rs:161 establish the pattern.

**Files:**
- Modify: `crates/cognitive/src/repos/entity.rs:294` (insert before `merge_entities`)
- Test: inline `#[cfg(test)] mod tests` in same file

- [ ] **Step 1: Write failing tests for `get_related_entities`**

Use `crate::repos::cognitive_test_pool()` (the existing test helper that runs base + cognitive migrations):

```rust
#[tokio::test]
async fn test_get_related_entities_all() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = EntityRepo::new(pool.clone());
    // Create entities A, B, C
    // Create relationships: A->B (works_on), A->C (uses)
    // get_related_entities(A, None) should return [B, C]
}

#[tokio::test]
async fn test_get_related_entities_filtered() {
    // Same setup with cognitive_test_pool()
    // get_related_entities(A, Some("works_on")) should return [B] only
}
```

- [ ] **Step 2: Run tests — verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(get_related_entities)'`
Expected: FAIL — method does not exist

- [ ] **Step 3: Implement `get_related_entities`**

Add after `get_neighborhood()` (around line 294) in `crates/cognitive/src/repos/entity.rs`:

```rust
/// Get entities related to `entity_id`, optionally filtered by relationship type.
/// Returns the related entities (not the center entity itself).
pub async fn get_related_entities(
    &self,
    entity_id: &str,
    rel_type: Option<&str>,
) -> Result<Vec<EntityRow>, sqlx::Error> {
    let rels = match rel_type {
        Some(rt) => {
            sqlx::query_as::<_, RelationshipRow>(
                "SELECT * FROM entity_relationships WHERE (source_entity_id = ?1 OR target_entity_id = ?1) AND relationship_type = ?2",
            )
            .bind(entity_id)
            .bind(rt)
            .fetch_all(&self.pool)
            .await?
        }
        None => self.get_relationships(entity_id).await?,
    };

    let mut neighbor_ids: Vec<String> = rels
        .iter()
        .map(|r| {
            if r.source_entity_id == entity_id {
                r.target_entity_id.clone()
            } else {
                r.source_entity_id.clone()
            }
        })
        .collect();
    neighbor_ids.sort();
    neighbor_ids.dedup();

    self.get_entities_by_ids(&neighbor_ids).await
}
```

- [ ] **Step 4: Run tests — verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(get_related_entities)'`
Expected: PASS

- [ ] **Step 5: Write failing test for `find_path`**

```rust
#[tokio::test]
async fn test_find_path_direct() {
    // A->B direct edge → path length 1
}

#[tokio::test]
async fn test_find_path_two_hops() {
    // A->B->C, no direct A->C → path length 2
}

#[tokio::test]
async fn test_find_path_no_path() {
    // A and D disconnected → returns empty vec
}
```

- [ ] **Step 6: Run tests — verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(find_path)'`

- [ ] **Step 7: Implement `find_path`**

BFS from `from` to `to` up to `max_depth` hops. Returns the edges along the shortest path:

```rust
/// Find shortest path between two entities via BFS, up to `max_depth` hops.
/// Returns the relationship edges along the path, or empty vec if no path found.
pub async fn find_path(
    &self,
    from: &str,
    to: &str,
    max_depth: u8,
) -> Result<Vec<RelationshipRow>, sqlx::Error> {
    if from == to {
        return Ok(Vec::new());
    }

    // BFS: queue of (current_entity_id, path_of_edges)
    let mut queue: std::collections::VecDeque<(String, Vec<RelationshipRow>)> =
        std::collections::VecDeque::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    visited.insert(from.to_string());
    queue.push_back((from.to_string(), Vec::new()));

    while let Some((current, path)) = queue.pop_front() {
        if path.len() >= max_depth as usize {
            continue;
        }
        let rels = self.get_relationships(&current).await?;
        for rel in rels {
            let neighbor = if rel.source_entity_id == current {
                &rel.target_entity_id
            } else {
                &rel.source_entity_id
            };
            if visited.contains(neighbor.as_str()) {
                continue;
            }
            let mut new_path = path.clone();
            new_path.push(rel);
            if neighbor == to {
                return Ok(new_path);
            }
            visited.insert(neighbor.clone());
            queue.push_back((neighbor.clone(), new_path));
        }
    }

    Ok(Vec::new())
}
```

- [ ] **Step 8: Run tests — verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(find_path)'`
Expected: PASS

- [ ] **Step 9: Clippy check**

Run: `cargo clippy -p cognitive --all-targets`
Expected: 0 warnings

- [ ] **Step 10: Commit**

```bash
git add crates/cognitive/src/repos/entity.rs
git commit -m "feat(cognitive): add EntityRepo::find_path and get_related_entities"
```

---

### Task 3: Add Entity Backfill from `note_entity_mentions`

**Context:** `backfill_from_facts()` at entity.rs:379 backfills from SPO facts. The spec also requires backfilling from `note_entity_mentions`, which stores `(note_id, entity_type, entity_id)` references. The `entity_id` is opaque (task UUID, project slug) — we need to JOIN against `tasks`/`projects` to resolve names. This is a cross-database concern: `note_entity_mentions` is in feature-notes migrations, `entities` is in cognitive migrations — both share the same SQLite DB.

**Files:**
- Modify: `crates/cognitive/src/repos/entity.rs` (add `backfill_from_note_mentions()`)
- Modify: `crates/agent/src/agent_loop/builder.rs:693-700` (wire the call)

- [ ] **Step 1: Write failing test**

**Important:** This test needs the `note_entity_mentions`, `tasks`, and `projects` tables, which come from feature-notes and feature-tasks migrations (not cognitive migrations). The `cognitive_test_pool()` helper only runs base + cognitive migrations. You need to additionally run those feature migrations in the test setup.

```rust
#[tokio::test]
async fn test_backfill_from_note_mentions() {
    let pool = crate::repos::cognitive_test_pool().await;

    // Run feature-notes and feature-tasks migrations manually for the JOINed tables.
    // These feature crates expose their migrations via FeatureMigration.
    // Alternatively, create the needed tables inline:
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, body TEXT,
            notebook_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY, title TEXT NOT NULL,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY, title TEXT NOT NULL,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS note_entity_mentions (
            note_id TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL,
            PRIMARY KEY (note_id, entity_type, entity_id)
        )"
    ).execute(&pool).await.unwrap();

    // Insert test data: a task and a note_entity_mention linking to it
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO tasks (id, title, created_at, updated_at) VALUES ('task-1', 'Fix auth bug', ?1, ?1)")
        .bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO notes (id, title, created_at, updated_at) VALUES ('note-1', 'Dev notes', ?1, ?1)")
        .bind(&now).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO note_entity_mentions (note_id, entity_type, entity_id) VALUES ('note-1', 'task', 'task-1')")
        .execute(&pool).await.unwrap();

    let repo = EntityRepo::new(pool.clone());
    let count = repo.backfill_from_note_mentions().await.unwrap();
    assert_eq!(count, 1);

    // Verify entity was created with name from task title
    let entities = repo.find_by_name("Fix auth bug").await.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].entity_type, "task");
}
```

- [ ] **Step 2: Run test — verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(backfill_from_note_mentions)'`

- [ ] **Step 3: Implement `backfill_from_note_mentions`**

Add after `backfill_from_facts()` in `crates/cognitive/src/repos/entity.rs`:

```rust
/// Backfill entities from `note_entity_mentions` → join against tasks/projects for names.
/// Creates entities for referenced tasks and projects that don't already exist in the graph.
/// Idempotent: uses INSERT OR IGNORE keyed on LOWER(name).
pub async fn backfill_from_note_mentions(&self) -> Result<u32, sqlx::Error> {
    let mut total = 0u32;

    // Tasks: join note_entity_mentions against tasks for title
    let r1 = sqlx::query(
        r#"
        INSERT OR IGNORE INTO entities (id, name, entity_type, description, source, source_id,
            first_seen_at, last_seen_at, mention_count, created_at, updated_at)
        SELECT
            lower(hex(randomblob(16))),
            t.title,
            'task',
            NULL,
            'backfill',
            nem.entity_id,
            COALESCE(t.created_at, datetime('now')),
            COALESCE(t.created_at, datetime('now')),
            COUNT(*),
            COALESCE(t.created_at, datetime('now')),
            COALESCE(t.created_at, datetime('now'))
        FROM note_entity_mentions nem
        JOIN tasks t ON nem.entity_id = t.id
        WHERE nem.entity_type = 'task'
          AND LOWER(TRIM(t.title)) NOT IN (SELECT LOWER(name) FROM entities)
        GROUP BY nem.entity_id
        "#,
    )
    .execute(&self.pool)
    .await;
    if let Ok(r) = r1 {
        total += r.rows_affected() as u32;
    }

    // Projects: join against projects for title
    let r2 = sqlx::query(
        r#"
        INSERT OR IGNORE INTO entities (id, name, entity_type, description, source, source_id,
            first_seen_at, last_seen_at, mention_count, created_at, updated_at)
        SELECT
            lower(hex(randomblob(16))),
            p.title,
            'project',
            NULL,
            'backfill',
            nem.entity_id,
            COALESCE(p.created_at, datetime('now')),
            COALESCE(p.created_at, datetime('now')),
            COUNT(*),
            COALESCE(p.created_at, datetime('now')),
            COALESCE(p.created_at, datetime('now'))
        FROM note_entity_mentions nem
        JOIN projects p ON nem.entity_id = p.id
        WHERE nem.entity_type = 'project'
          AND LOWER(TRIM(p.title)) NOT IN (SELECT LOWER(name) FROM entities)
        GROUP BY nem.entity_id
        "#,
    )
    .execute(&self.pool)
    .await;
    if let Ok(r) = r2 {
        total += r.rows_affected() as u32;
    }

    Ok(total)
}
```

Note: The JOINs silently skip if `tasks`/`projects` tables don't exist (the queries will fail gracefully since we use `if let Ok(r)`). This handles the case where cognitive migrations ran but feature-tasks/feature-notes didn't.

- [ ] **Step 4: Run test — verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(backfill_from_note_mentions)'`

- [ ] **Step 5: Wire into builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, add a second `tokio::spawn` after the existing backfill at line 700:

```rust
// After the existing backfill_from_facts spawn (line 700):
let entity_repo2 = cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
tokio::spawn(async move {
    match entity_repo2.backfill_from_note_mentions().await {
        Ok(0) => {}
        Ok(n) => tracing::info!("Backfilled {n} entities from note mentions"),
        Err(e) => tracing::debug!("Note mention backfill error (non-fatal): {e}"),
    }
});
```

- [ ] **Step 6: Clippy + build check**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/repos/entity.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(cognitive): backfill entities from note_entity_mentions (tasks + projects)"
```

---

### Task 4: Enhance Entity Extraction with Type Inference + Relationships

**Context:** The entity extraction in `background.rs:463-503` always creates entities as `entity_type: "concept"` and never creates relationships between subject/object entities. The spec calls for predicate-based type inference (like `backfill_from_facts` does) and creating a relationship edge.

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:463-503`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_entity_extraction_infers_type_from_predicate() {
    // Setup consolidation with a fact: subject="React", predicate="is_a_technology", object="frontend framework"
    // After consolidation, the entity for "React" should have entity_type="technology", not "concept"
}

#[tokio::test]
async fn test_entity_extraction_creates_relationship() {
    // Setup consolidation with fact: subject="Alice", predicate="works_on", object="Klynt"
    // After consolidation: entities "Alice" and "Klynt" exist AND a relationship edge between them
}
```

- [ ] **Step 2: Run tests — verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(entity_extraction_infers)'`

- [ ] **Step 3: Implement type inference + relationship creation**

Replace the entity extraction block at `background.rs:463-503`:

```rust
// ── Entity extraction from new facts ──────────────────
let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
for (candidate, op) in candidates.iter().zip(ops.iter()) {
    match op {
        crate::types::MemoryOp::Add { .. }
        | crate::types::MemoryOp::Update { .. } => {
            let fact = &candidate.candidate;

            // Infer entity type from predicate
            let subject_type = infer_entity_type(&fact.predicate);

            // Upsert subject as entity
            let subj = entity_repo
                .upsert_entity(&crate::repos::NewEntity {
                    name: fact.subject.clone(),
                    entity_type: subject_type,
                    description: None,
                    source: "extracted".to_string(),
                    source_id: Some(fact.id.clone()),
                    metadata: None,
                })
                .await;

            // Upsert object as entity (skip numeric/short values)
            let obj = if fact.object.len() > 2
                && fact.object.len() < 100
                && !fact.object.chars().all(|c| c.is_ascii_digit() || c == '.')
            {
                let object_type = infer_entity_type(&fact.predicate);
                entity_repo
                    .upsert_entity(&crate::repos::NewEntity {
                        name: fact.object.clone(),
                        entity_type: object_type,
                        description: None,
                        source: "extracted".to_string(),
                        source_id: Some(fact.id.clone()),
                        metadata: None,
                    })
                    .await
                    .ok()
            } else {
                None
            };

            // Create relationship between subject and object entities
            if let (Ok(s), Some(o)) = (subj, obj) {
                let _ = entity_repo
                    .upsert_relationship(&crate::repos::NewRelationship {
                        source_entity_id: s.id,
                        target_entity_id: o.id,
                        relationship_type: fact.predicate.clone(),
                        evidence: Some(format!("{} {} {}", fact.subject, fact.predicate, fact.object)),
                        source: "extracted".to_string(),
                    })
                    .await;
            }
        }
        _ => {}
    }
}
```

Add the helper function (at module level, near `is_same_session`):

```rust
/// Infer entity type from predicate keywords.
/// Applied to both subject and object sides — the predicate determines type regardless of position.
fn infer_entity_type(predicate: &str) -> String {
    let p = predicate.to_lowercase();
    if p.contains("person") || p.contains("manages") || p.contains("hired") || p.contains("reports_to") {
        return "person".to_string();
    }
    if p.contains("project") || p.contains("works_on") || p.contains("contributes_to") {
        return "project".to_string();
    }
    if p.contains("uses") || p.contains("tool") || p.contains("technology") || p.contains("framework") || p.contains("is_a_technology") {
        return "technology".to_string();
    }
    if p.contains("organization") || p.contains("company") || p.contains("employer") {
        return "organization".to_string();
    }
    "concept".to_string()
}
```

- [ ] **Step 4: Run tests — verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(entity_extraction)'`
Expected: PASS

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p cognitive --all-targets`

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): infer entity types from predicates and create relationship edges during extraction"
```

---

### Task 5: Add Flashcard Review Tauri Commands + IPC Types

**Context:** The backend `FlashcardRepo` has `list_decks()`, `get_due_cards()`, and `record_review()` fully implemented, but there are no Tauri commands or IPC types to expose them to the frontend. Currently only `note_insight_save_flashcards` exists.

**Files:**
- Create: `crates/app-core/src/handlers/notes/flashcard.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs` (add module)
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add IPC types)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri commands + DEV_COMMANDS + dispatch_dev)

- [ ] **Step 1: Add IPC types to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add after `InsightSaveFlashcardsParams` (around line 246):

```rust
// ── Flashcard Review ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSummaryResponse {
    pub name: String,
    pub card_count: i64,
    pub due_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardReviewParams {
    pub card_id: String,
    pub quality: String,  // "again" | "hard" | "good" | "easy"
}
```

- [ ] **Step 2: Add AppCore handler methods**

Create `crates/app-core/src/handlers/notes/flashcard.rs`.

**Important:** `ApiError` only has `ApiError::new(code, message)` — no `not_configured` or `bad_request` helpers. `ReviewQuality` is at `cognitive::repos::flashcard::ReviewQuality` (not re-exported from `cognitive` root — import from `cognitive::repos`). There is no `From<FlashcardRow> for FlashcardResponse` — use the same field-by-field mapping as `insight_save_flashcards` at `insight.rs:315-331`.

```rust
use crate::{state::AppCore, ApiError};
use cognitive::repos::flashcard::ReviewQuality;
use desktop_shared::commands::{DeckSummaryResponse, FlashcardReviewParams, FlashcardResponse};

/// Map a FlashcardRow to a FlashcardResponse (same pattern as insight.rs:315-331).
fn flashcard_to_response(r: cognitive::FlashcardRow) -> FlashcardResponse {
    FlashcardResponse {
        id: r.id,
        deck: r.deck,
        question: r.question,
        answer: r.answer,
        card_type: r.card_type,
        choices: r.choices.as_deref().and_then(|s| serde_json::from_str(s).ok()),
        stability: r.stability,
        difficulty: r.difficulty,
        due_at: r.due_at,
        state: r.state,
        review_count: r.review_count,
        created_at: r.created_at,
    }
}

impl AppCore {
    pub async fn flashcard_list_decks(&self) -> Result<Vec<DeckSummaryResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let decks = repo
            .list_decks()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(decks
            .into_iter()
            .map(|d| DeckSummaryResponse {
                name: d.name,
                card_count: d.card_count,
                due_count: d.due_count,
            })
            .collect())
    }

    pub async fn flashcard_get_due(
        &self,
        deck: &str,
        limit: i64,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let cards = repo
            .get_due_cards(deck, limit)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_record_review(
        &self,
        params: FlashcardReviewParams,
    ) -> Result<FlashcardResponse, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let quality = match params.quality.as_str() {
            "again" => ReviewQuality::Again,
            "hard" => ReviewQuality::Hard,
            "good" => ReviewQuality::Good,
            "easy" => ReviewQuality::Easy,
            _ => return Err(ApiError::new("VALIDATION", "Invalid review quality: must be again|hard|good|easy")),
        };
        let card = repo
            .record_review(&params.card_id, quality)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(card))
    }
}
```

Register `mod flashcard;` in `crates/app-core/src/handlers/notes/mod.rs`.

- [ ] **Step 3: Add Tauri commands**

In `crates/desktop/src/commands/notes.rs`:

**First**, add `DeckSummaryResponse` and `FlashcardReviewParams` to the existing import block at the top of the file (lines 3-10) where other `desktop_shared::commands::*` types are imported.

**Then**, add after the persona commands (around line 427):

```rust
// ── Flashcard Review commands ───────────────────────────────────

#[tauri::command]
pub async fn flashcard_list_decks(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<DeckSummaryResponse>, ApiError> {
    state.flashcard_list_decks().await
}

#[tauri::command]
pub async fn flashcard_get_due(
    state: State<'_, Arc<AppCore>>,
    deck: String,
    limit: Option<i64>,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_due(&deck, limit.unwrap_or(10)).await
}

#[tauri::command]
pub async fn flashcard_record_review(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardReviewParams,
) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_record_review(params).await
}
```

Add to `DEV_COMMANDS` array:
```rust
"flashcard_list_decks",
"flashcard_get_due",
"flashcard_record_review",
```

Add to `dispatch_dev` match block (uses `dev_helpers` — `try_field!` is a macro, `dev::parse_params` handles nested params):
```rust
"flashcard_list_decks" => dev::val(core.flashcard_list_decks().await),
"flashcard_get_due" => {
    let deck: String = try_field!(dev::get_str(body, "deck"));
    let limit: Option<i64> = dev::get(body, "limit");
    dev::val(core.flashcard_get_due(&deck, limit.unwrap_or(10)).await)
}
"flashcard_record_review" => {
    let params: FlashcardReviewParams = try_field!(dev::parse_params(body));
    dev::val(core.flashcard_record_review(params).await)
}
```

**Register in `invoke_handler`:** In `crates/desktop/src/main.rs`, add these 3 entries to the `tauri::generate_handler![...]` macro, after the existing `commands::notes::note_insight_auto_generate_persona` line (around line 375):

```rust
commands::notes::flashcard_list_decks,
commands::notes::flashcard_get_due,
commands::notes::flashcard_record_review,
```

- [ ] **Step 4: Build check**

Run: `cargo build -p desktop -p app-core -p desktop-shared`
Expected: compiles

- [ ] **Step 5: Run the dev_server coverage test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`
Expected: PASS — all new commands are in DEV_COMMANDS

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/notes/flashcard.rs crates/app-core/src/handlers/notes/mod.rs \
       crates/desktop-shared/src/commands/notes.rs crates/desktop/src/commands/notes.rs \
       crates/desktop/src/main.rs
git commit -m "feat(flashcards): add list_decks, get_due, record_review Tauri commands"
```

---

### Task 6: Flashcard Review Frontend Component

**Context:** Backend + Tauri commands exist (after Task 5). Need a `FlashcardReview` component and `useFlashcards` hook. This renders as a slide-in panel accessible from the Insight Review "Self-Assessment" tab (where the "Save as Deck" button already lives).

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useFlashcards.ts`
- Create: `desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` (add review button)

- [ ] **Step 1: Create `useFlashcards` hook**

```typescript
// desktop-ui/src/features/notes/hooks/useFlashcards.ts
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface DeckSummary {
  name: string;
  cardCount: number;
  dueCount: number;
}

export interface Flashcard {
  id: string;
  deck: string;
  question: string;
  answer: string;
  cardType: string;
  choices: { label: string; text: string }[] | null;
  stability: number;
  difficulty: number;
  dueAt: string | null;
  state: string;
  reviewCount: number;
  createdAt: string;
}

type ReviewQuality = "again" | "hard" | "good" | "easy";

export function useFlashcards() {
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);

  const fetchDecks = useCallback(async () => {
    const result = await ipc<DeckSummary[]>("flashcard_list_decks", {});
    setDecks(result);
  }, []);

  const startReview = useCallback(async (deck: string) => {
    const due = await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 20 });
    setCards(due);
    setCurrentIndex(0);
    setRevealed(false);
  }, []);

  const reveal = useCallback(() => setRevealed(true), []);

  const review = useCallback(
    async (quality: ReviewQuality) => {
      const card = cards[currentIndex];
      if (!card) return;
      await ipc("flashcard_record_review", { cardId: card.id, quality });
      setRevealed(false);
      setCurrentIndex((i) => i + 1);
    },
    [cards, currentIndex],
  );

  const current = cards[currentIndex] ?? null;
  const remaining = Math.max(0, cards.length - currentIndex);
  const done = currentIndex >= cards.length && cards.length > 0;

  return { decks, cards, current, remaining, done, revealed, fetchDecks, startReview, reveal, review };
}
```

- [ ] **Step 2: Create `FlashcardReview` component**

```typescript
// desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx
import { BookOpen, ChevronRight, RotateCcw, X } from "lucide-react";
import { useEffect } from "react";
import { useFlashcards } from "../../hooks/useFlashcards";

interface FlashcardReviewProps {
  deckName: string;
  onClose: () => void;
}

export function FlashcardReview({ deckName, onClose }: FlashcardReviewProps) {
  const { current, remaining, done, revealed, startReview, reveal, review } = useFlashcards();

  useEffect(() => {
    startReview(deckName);
  }, [deckName, startReview]);

  if (done) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8">
        <BookOpen size={24} className="text-emerald-400" />
        <p className="text-[12px] text-foreground font-medium">Review complete!</p>
        <button
          type="button"
          onClick={onClose}
          className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
        >
          Done
        </button>
      </div>
    );
  }

  if (!current) {
    return (
      <div className="flex items-center justify-center py-8">
        <p className="text-[11px] text-dim">Loading cards...</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 p-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-dim">{remaining} remaining</span>
        <div className="flex-1" />
        <button type="button" onClick={onClose} className="p-1 text-dim hover:text-foreground">
          <X size={12} />
        </button>
      </div>

      {/* Question */}
      <div className="rounded-lg bg-white/[0.03] p-3">
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{current.question}</p>
        {current.choices && (
          <div className="mt-2 space-y-1">
            {current.choices.map((c) => (
              <div key={c.label} className="text-[10px] text-muted-foreground">
                <span className="font-medium">{c.label}.</span> {c.text}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Answer (revealed or button) */}
      {revealed ? (
        <>
          <div className="rounded-lg bg-emerald-400/10 border border-emerald-400/20 p-3">
            <p className="text-[11px] text-emerald-300 whitespace-pre-wrap">{current.answer}</p>
          </div>
          <div className="flex gap-2 justify-center">
            {(["again", "hard", "good", "easy"] as const).map((q) => (
              <button
                key={q}
                type="button"
                onClick={() => review(q)}
                className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.08] capitalize"
              >
                {q}
              </button>
            ))}
          </div>
        </>
      ) : (
        <button
          type="button"
          onClick={reveal}
          className="flex items-center justify-center gap-1 text-[10px] px-3 py-2 rounded-md bg-purple-400/20 text-purple-300 hover:bg-purple-400/30"
        >
          <ChevronRight size={10} />
          Show Answer
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Wire into InsightReviewPanel**

In `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`, add a "Review Due Cards" button next to the existing "Save as Deck" button in the footer. When clicked, open a `FlashcardReview` in-panel:

Add state:
```typescript
const [reviewingDeck, setReviewingDeck] = useState<string | null>(null);
```

Add rendering (below the existing panel content, before the `ManagePersonasModal`):
```tsx
{reviewingDeck && (
  <div className="absolute inset-0 z-30 bg-surface-base/95 rounded-xl overflow-y-auto">
    <FlashcardReview deckName={reviewingDeck} onClose={() => setReviewingDeck(null)} />
  </div>
)}
```

Add a button in the footer (next to "Save as Deck"):
```tsx
<button
  type="button"
  onClick={() => {
    // Use the note title as deck name (same convention as saveFlashcards)
    if (state.noteId) setReviewingDeck(state.noteId);
  }}
  className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.06]"
  title="Review due flashcards"
>
  <RotateCcw size={10} />
  Review
</button>
```

- [ ] **Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: compiles

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: 0 errors

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useFlashcards.ts \
       desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx \
       desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(notes): add flashcard review UI with FSRS-based spaced repetition"
```

---

### Task 7: Enhance ManagePersonasModal with Pin UI + Auto-Generate

**Context:** `ManagePersonasModal` receives `actions` (which includes `setPins` and `autoGenerate`) but never calls them. The modal needs: (1) an "Auto-generate" button that creates a domain-relevant persona from the current note, and (2) a pin section showing which personas are pinned for this specific note.

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` (pass `noteId`)

- [ ] **Step 1: Pass `noteId` to the modal**

In `InsightReviewPanel.tsx`, change the modal rendering (around line 424) to include `noteId`:

```tsx
{showPersonaManager && (
  <ManagePersonasModal
    personas={allPersonas}
    actions={personaActions}
    noteId={state.noteId}
    onClose={() => setShowPersonaManager(false)}
  />
)}
```

- [ ] **Step 2: Update ManagePersonasModal props and add pin + auto-generate UI**

Update `ManagePersonasModalProps`:
```typescript
interface ManagePersonasModalProps {
  personas: Persona[];
  actions: PersonaActions;
  noteId: string | null;
  onClose: () => void;
}
```

Add state for pins and auto-generating:
```typescript
const [pinnedIds, setPinnedIds] = useState<Set<string>>(new Set());
const [autoGenerating, setAutoGenerating] = useState(false);
```

Add pin toggle to each persona row (inside the existing map, after the active checkbox):
```tsx
{noteId && (
  <label className="flex items-center gap-1 cursor-pointer">
    <input
      type="checkbox"
      checked={pinnedIds.has(p.id)}
      onChange={(e) => {
        const next = new Set(pinnedIds);
        if (e.target.checked) next.add(p.id);
        else next.delete(p.id);
        setPinnedIds(next);
        if (noteId) actions.setPins(noteId, [...next]);
      }}
      className="w-3 h-3 accent-amber-400"
    />
    <span className="text-[9px] text-dim">Pin</span>
  </label>
)}
```

Add auto-generate button in the footer (next to "Create Persona"):
```tsx
{noteId && (
  <button
    type="button"
    disabled={autoGenerating}
    onClick={async () => {
      setAutoGenerating(true);
      try {
        await actions.autoGenerate(noteId);
      } finally {
        setAutoGenerating(false);
      }
    }}
    className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-purple-400/10 text-purple-300 hover:bg-purple-400/20 disabled:opacity-50"
  >
    {autoGenerating ? "Generating..." : "Auto-generate"}
  </button>
)}
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx \
       desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(notes): add persona pin UI and auto-generate button to ManagePersonasModal"
```

---

### Task 8: Enrich Domain-Matching with Knowledge Graph Entities

**Context:** `extract_note_domains()` in `insight_context.rs:49-51` just lowercases tags. The spec wants entity graph lookup to enrich domain matching — e.g., if a note mentions "React" (a `technology` entity), the domain `"technology"` should be added so tech-focused personas get selected.

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_context.rs:48-51`
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (pass EntityRepo to domain extraction)

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_extract_note_domains_includes_entity_types() {
    // Setup: entity "React" with entity_type="technology" in the graph
    // Tags: ["React", "tutorial"]
    // extract_note_domains(tags, Some(&entity_repo)) should return ["react", "tutorial", "technology"]
}
```

- [ ] **Step 2: Run test — verify it fails**

- [ ] **Step 3: Implement graph-enriched domain extraction**

Change `extract_note_domains` in `crates/app-core/src/handlers/notes/insight_context.rs`:

```rust
/// Extract domain hints from a note's tags for persona selection.
/// Optionally enriches with entity types from the knowledge graph.
pub async fn extract_note_domains(
    tags: &[String],
    entity_repo: Option<&cognitive::repos::EntityRepo>,
) -> Vec<String> {
    let mut domains: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    // Enrich: look up each tag in the entity graph and add entity_type as a domain
    if let Some(repo) = entity_repo {
        for tag in tags {
            if let Ok(entities) = repo.find_by_name(tag).await {
                for entity in &entities {
                    let et = entity.entity_type.to_lowercase();
                    if !domains.contains(&et) {
                        domains.push(et);
                    }
                }
            }
        }
    }

    domains
}
```

- [ ] **Step 4: Update all 3 call sites**

Changing `extract_note_domains` from sync to `async fn` breaks 3 call sites in `insight.rs`. Each needs `.await` added and an `EntityRepo` constructed. Construct via `cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone())` (check if `self.storage_pool` is accessible; if not, use `self.note_repo.pool().clone()` or the pool available in that handler context).

**Call site 1** — `insight.rs:130` (in `note_insight_review`):
```rust
// BEFORE:
let note_domains = insight_context::extract_note_domains(&tags);
// AFTER:
let entity_repo = cognitive::repos::EntityRepo::new(self.note_repo.pool().clone());
let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;
```

**Call site 2** — `insight.rs:382` (in `note_insight_regenerate_tab`):
```rust
// BEFORE:
let note_domains = insight_context::extract_note_domains(&tags);
// AFTER:
let entity_repo = cognitive::repos::EntityRepo::new(self.note_repo.pool().clone());
let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;
```

**Call site 3** — `insight.rs:465` (in `note_insight_auto_generate_persona`):
```rust
// BEFORE:
let note_domains = insight_context::extract_note_domains(&tags);
// AFTER:
let entity_repo = cognitive::repos::EntityRepo::new(self.note_repo.pool().clone());
let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;
```

If constructing `EntityRepo` is not possible in any context (missing pool access), pass `None` — the function falls back to tag-only matching.

- [ ] **Step 5: Run test — verify it passes**

Run: `cargo nextest run -p app-core -E 'test(extract_note_domains)'`

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p app-core --all-targets`

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_context.rs \
       crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(insights): enrich persona domain matching with knowledge graph entity types"
```

---

## Config Key Naming (Non-Code Change)

The spec says `contextEngine.insightForge.*` and `contextEngine.temporalWeight`, but the implementation uses `cognitive.insightForge*` and `cognitive.relevanceWeightTemporal`. Since we haven't released and `cognitive` is the correct config section for these features (they live in the cognitive crate), **the implementation is correct and the spec should be updated**. No code change needed — update the spec document instead:

- `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` §3 config block: change `contextEngine.insightForge.*` → `cognitive.insightForge*`
- §4 config: change `contextEngine.temporalWeight` → `cognitive.relevanceWeightTemporal`

---

## Execution Order

Tasks 1–4 are Rust-only and independent. Tasks 5–6 are sequential (5 creates the backend, 6 creates the frontend). Task 7 is frontend-only and independent. Task 8 is Rust-only and independent.

**Parallel groups:**
- Group A: Tasks 1, 2, 3, 4, 8 (all independent Rust changes)
- Group B: Task 5 → Task 6 (sequential: backend before frontend)
- Group C: Task 7 (independent frontend)

Groups A, B (step 5 only), and C can run in parallel. Task 6 waits for Task 5.
