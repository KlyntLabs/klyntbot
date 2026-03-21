# Knowledge Atoms Phase 2: "The Living Brain" Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make atoms alive — auto-extract concepts from notes, decay unused knowledge, detect cross-note reinforcement, and provide a Knowledge Health page with topic heatmap.

**Architecture:** A background `AtomExtractionService` subscribes to `NoteContentChanged` events, debounces by content hash, and uses the cognitive LLM provider to extract concept/fact atoms as `status="suggested"`. A daily decay cron (3 AM local) applies tiered salience decay and FSRS retention recomputation, auto-archiving forgotten atoms. Cross-note reinforcement detects when existing atoms appear in new notes, boosting salience and linking `secondary_sources`. Frontend gets a Knowledge Health page with topic-grouped retention bars.

**Tech Stack:** Rust (SQLite via sqlx, tokio broadcast, sha2), React (TypeScript, Tailwind CSS), Tauri IPC, FSRS-5

**Spec:** `docs/superpowers/specs/2026-03-21-unified-learning-system-design.md` (Section 3: Auto-Extraction, Section 4: Decay System, Phase 2 deliverables)

**Depends on:** Phase 1 complete (knowledge_atoms + knowledge_topics tables, KnowledgeAtomRepo, DomainEvents, KnowledgeAtomsPanel UI)

---

## Already covered by Phase 1

These Phase 2 spec items are already implemented and need no further work:
- **ActivityLogSubscriber handles all learning events** — all 11 event normalizers exist in `activity-log/src/normalizers.rs`
- **Suggested atoms UX (dimmed cards, Accept/Dismiss)** — `KnowledgeAtomsPanel` already splits active/suggested, `AtomCard` renders dimmed suggested cards with Accept (+) and Dismiss (✕) buttons
- **`NoteContentChanged` emission** — already fires in `handlers/notes/crud.rs` on create + update

## File Map

### New files
| File | Responsibility |
|---|---|
| `crates/cognitive/src/services/atom_extraction.rs` | AtomExtractionService: subscribe to NoteContentChanged, debounce, LLM extract, create suggested atoms, detect cross-note reinforcement |
| `crates/cognitive/src/services/atom_decay.rs` | Daily decay logic: tiered salience decay, FSRS retention recompute, auto-archive, emit CoachingLearningDigest |
| `crates/cognitive/src/repos/atom_extraction_cache.rs` | AtomExtractionCache repo: content hash dedup for note extraction |
| `crates/app-core/src/handlers/knowledge_health.rs` | AppCore handlers for knowledge health summary + topic detail |
| `crates/desktop-shared/src/commands/knowledge_health.rs` | IPC param/response types for knowledge health |
| `crates/desktop/src/commands/knowledge_health.rs` | Tauri commands + DEV_COMMANDS + dispatch_dev |
| `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx` | Knowledge Health page with topic heatmap |
| `desktop-ui/src/features/learn/hooks/useKnowledgeHealth.ts` | Query hooks for knowledge health data |
| `desktop-ui/src/features/notes/components/WhyThisPopover.tsx` | "Why this?" hover popover for suggested atoms |
| `desktop-ui/src/features/notes/components/BulkAcceptModal.tsx` | Bulk-accept modal with importance emoji picker |

### Modified files
| File | Changes |
|---|---|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `atom_extraction_cache` table |
| `crates/cognitive/src/repos/mod.rs` | Export new repo, bump migration version |
| `crates/cognitive/src/services/mod.rs` | Export new services |
| `crates/cognitive/src/repos/knowledge_atom.rs` | Add methods: `list_stale_active`, `apply_decay`, `list_topics_with_atoms`, `find_by_subject_across_notes`, `boost_salience` |
| `crates/config/src/schema/cognitive.rs` | Add `atom_extraction` config section (enabled flag, extraction budget). Use existing `super::core::default_true`. |
| `crates/app-core/src/init/cognitive.rs` | Start AtomExtractionService alongside BackgroundConsolidation |
| `crates/app-core/src/init/cron.rs` | Register atom decay daily cron |
| `crates/app-core/src/handlers/mod.rs` | Add knowledge_health handler module |
| `crates/app-core/src/handlers/atoms.rs` | Fill topic_name + linked_card_count (resolve Phase 1 TODOs), add atoms_bulk_accept handler |
| `crates/desktop-shared/src/commands/atoms.rs` | Add AtomBulkAcceptParams |
| `crates/desktop/src/commands/mod.rs` | Add knowledge_health command module |
| `crates/desktop/src/main.rs` | Register knowledge_health + atoms_bulk_accept commands |
| `crates/desktop/src/dev_server/dispatch.rs` | Add knowledge_health dispatch |
| `crates/desktop/src/dev_server/mod.rs` | Add knowledge_health DEV_COMMANDS |
| `crates/desktop-shared/src/commands/mod.rs` | Add knowledge_health types module |
| `crates/activity-log/src/inference.rs` | Recognize learning events as "learning" work context |
| `desktop-ui/src/features/notes/components/AtomCard.tsx` | Add "Why this?" hover trigger for suggested atoms |
| `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx` | Replace "Accept all" with BulkAcceptModal trigger |
| `desktop-ui/src/features/learn/components/DashboardHome.tsx` | Add Knowledge Health card/link |
| `desktop-ui/src/app/router.tsx` | Add knowledge health route |

---

### Task 1: Schema — atom_extraction_cache table

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add atom_extraction_cache table to migration SQL**

Append at the end of `001_cognitive_tables.sql` (after the coaching_intervention_log table):

```sql
-- ── Atom Extraction Cache (content-hash dedup) ────────────────────
CREATE TABLE IF NOT EXISTS atom_extraction_cache (
    note_id       TEXT PRIMARY KEY NOT NULL,
    content_hash  TEXT NOT NULL,
    extracted_at  TEXT NOT NULL
);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, bump the version from `11` to `12` in `cognitive_migrations()`.

- [ ] **Step 3: Verify schema compiles**

Run: `cargo build -p cognitive 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(cognitive): add atom_extraction_cache schema
```

---

### Task 2: AtomExtractionCache repo

**Files:**
- Create: `crates/cognitive/src/repos/atom_extraction_cache.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Create the cache repo**

Create `crates/cognitive/src/repos/atom_extraction_cache.rs`:

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AtomExtractionCache {
    pool: SqlitePool,
}

impl AtomExtractionCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check if note content has already been extracted with this hash.
    pub async fn is_cached(&self, note_id: &str, content_hash: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT note_id FROM atom_extraction_cache WHERE note_id = ?1 AND content_hash = ?2",
        )
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Update or insert the cache entry for a note.
    pub async fn set(&self, note_id: &str, content_hash: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO atom_extraction_cache (note_id, content_hash, extracted_at) VALUES (?1, ?2, ?3) ON CONFLICT(note_id) DO UPDATE SET content_hash = ?2, extracted_at = ?3",
        )
        .bind(note_id)
        .bind(content_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 2: Export from repos/mod.rs**

Add `pub mod atom_extraction_cache;` and `pub use atom_extraction_cache::AtomExtractionCache;` to `crates/cognitive/src/repos/mod.rs`.

- [ ] **Step 3: Write unit test**

Add `#[cfg(test)] mod tests` at the bottom of `atom_extraction_cache.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_cache_miss_then_hit() {
        let pool = cognitive_test_pool().await;
        let cache = AtomExtractionCache::new(pool);

        assert!(!cache.is_cached("note-1", "hash-abc").await.unwrap());
        cache.set("note-1", "hash-abc").await.unwrap();
        assert!(cache.is_cached("note-1", "hash-abc").await.unwrap());
        // Different hash = miss
        assert!(!cache.is_cached("note-1", "hash-def").await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_update_overwrites() {
        let pool = cognitive_test_pool().await;
        let cache = AtomExtractionCache::new(pool);

        cache.set("note-1", "hash-old").await.unwrap();
        cache.set("note-1", "hash-new").await.unwrap();
        assert!(!cache.is_cached("note-1", "hash-old").await.unwrap());
        assert!(cache.is_cached("note-1", "hash-new").await.unwrap());
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(atom_extraction_cache)'`
Expected: all pass

- [ ] **Step 5: Re-export from lib.rs**

Add `pub use repos::AtomExtractionCache;` to `crates/cognitive/src/lib.rs`.

- [ ] **Step 6: Commit**

```
feat(cognitive): add AtomExtractionCache repo with content-hash dedup
```

---

### Task 3: KnowledgeAtomRepo — new methods for extraction + decay

**Files:**
- Modify: `crates/cognitive/src/repos/knowledge_atom.rs`

- [ ] **Step 1: Add `list_stale_active` for decay cron**

```rust
/// List active atoms that haven't been interacted with for N days.
pub async fn list_stale_active(
    &self,
    stale_days: i64,
) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(stale_days)).to_rfc3339();
    sqlx::query_as::<_, KnowledgeAtomRow>(
        r#"
        SELECT * FROM knowledge_atoms
        WHERE status = 'active'
          AND (last_interaction_ts IS NULL OR last_interaction_ts < ?1)
        "#,
    )
    .bind(&cutoff)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 2: Add `batch_update_decay` for efficient decay**

```rust
/// Apply salience decay and retention update to a single atom.
pub async fn apply_decay(
    &self,
    id: &str,
    new_salience: f64,
    new_retention_pct: f64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE knowledge_atoms SET salience = ?2, retention_pct = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(id)
    .bind(new_salience)
    .bind(new_retention_pct)
    .bind(&now)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 3: Add `list_topics_with_atoms` for health page**

```rust
/// List all topics with their atom counts and avg retention (for health dashboard).
pub async fn list_topics_with_atoms(&self) -> Result<Vec<KnowledgeTopicRow>, sqlx::Error> {
    sqlx::query_as::<_, KnowledgeTopicRow>(
        "SELECT * FROM knowledge_topics WHERE atom_count > 0 ORDER BY avg_retention ASC",
    )
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 4: Add `find_by_subject_across_notes` for cross-note reinforcement**

```rust
/// Find existing active atoms matching a subject in other notes (for reinforcement detection).
pub async fn find_by_subject_across_notes(
    &self,
    subject: &str,
    exclude_note_id: &str,
) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
    sqlx::query_as::<_, KnowledgeAtomRow>(
        r#"
        SELECT * FROM knowledge_atoms
        WHERE subject = ?1
          AND source_note_id != ?2
          AND status = 'active'
        "#,
    )
    .bind(subject)
    .bind(exclude_note_id)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 5: Add `boost_salience` for reinforcement**

```rust
/// Boost an atom's salience (capped at 1.0) and record a secondary source.
pub async fn boost_salience(
    &self,
    id: &str,
    boost: f64,
    referencing_note_id: &str,
) -> Result<f64, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    // Read current salience + secondary_sources
    let atom = sqlx::query_as::<_, KnowledgeAtomRow>(
        "SELECT * FROM knowledge_atoms WHERE id = ?1",
    )
    .bind(id)
    .fetch_one(&self.pool)
    .await?;

    let new_salience = (atom.salience + boost).min(1.0);

    // Append to secondary_sources JSON array
    let mut sources: Vec<serde_json::Value> = atom
        .secondary_sources
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    sources.push(serde_json::json!({ "noteId": referencing_note_id, "ts": &now }));
    let sources_json = serde_json::to_string(&sources).unwrap_or_else(|_| "[]".to_string());

    sqlx::query(
        "UPDATE knowledge_atoms SET salience = ?2, secondary_sources = ?3, last_interaction_ts = ?4, updated_at = ?4 WHERE id = ?1",
    )
    .bind(id)
    .bind(new_salience)
    .bind(&sources_json)
    .bind(&now)
    .execute(&self.pool)
    .await?;

    Ok(new_salience)
}
```

- [ ] **Step 6: Write tests for new methods**

```rust
#[tokio::test]
async fn test_list_stale_active() { /* create 2 atoms, one with old interaction_ts, verify count */ }

#[tokio::test]
async fn test_apply_decay() { /* create atom, apply_decay, verify salience + retention changed */ }

#[tokio::test]
async fn test_find_by_subject_across_notes() { /* create same subject in 2 notes, verify find returns the other */ }

#[tokio::test]
async fn test_boost_salience() { /* create atom with salience 0.5, boost 0.3, verify 0.8 + secondary_sources populated */ }
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(knowledge_atom)'`
Expected: all pass

- [ ] **Step 8: Commit**

```
feat(cognitive): add decay + reinforcement methods to KnowledgeAtomRepo
```

---

### Task 4: Atom extraction config

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add atom extraction config section**

In `crates/config/src/schema/cognitive.rs`, add a new struct alongside the existing `CognitiveConfig`. Use the existing `super::core::default_true` helper (do NOT define a local `default_true`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomExtractionConfig {
    #[serde(default = "super::core::default_true")]
    pub enabled: bool,
    #[serde(default = "default_extraction_max_tokens")]
    pub max_tokens: u32,
}

fn default_extraction_max_tokens() -> u32 {
    1024
}

impl Default for AtomExtractionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tokens: default_extraction_max_tokens(),
        }
    }
}
```

Then add `pub atom_extraction: AtomExtractionConfig` (with `#[serde(default)]`) to the parent `CognitiveConfig` struct.

- [ ] **Step 2: Build to verify**

Run: `cargo build -p config 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 3: Commit**

```
feat(config): add atom_extraction config (enabled + max_tokens)
```

---

### Task 5: AtomExtractionService — subscribe to NoteContentChanged, extract concepts

**Files:**
- Create: `crates/cognitive/src/services/atom_extraction.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Create the extraction service**

This is the core of Phase 2. The service:
1. Subscribes to `DomainEventBus`
2. Listens for `NoteContentChanged` events
3. Debounces using a `HashMap<String, Instant>` — stores last-seen timestamp per note_id, only processes if 5 seconds have passed since last event for that note (batch rapid edits)
4. Computes content hash via `sha2::Sha256::digest(content)` → hex string, checks `AtomExtractionCache::is_cached(note_id, hash)` — skips if cached
5. Splits note into sections: split on `## ` headings; if no headings, split into ~200-word chunks. Skip sections shorter than 30 words.
6. For each section, calls the cognitive LLM provider with extraction prompt (max_tokens from config, default 1024)
7. Deduplicates extracted atoms against existing atoms via `find_existing_subjects`
8. For each new concept, checks `find_by_subject_across_notes` — if found, emits `AtomReinforced` and calls `boost_salience` instead of creating a duplicate
9. Creates remaining atoms as `status="suggested"`, `salience=0.8`
10. Emits `KnowledgeAtomCreated` for each new atom
11. Updates cache: `AtomExtractionCache::set(note_id, hash)`

**Extraction prompt:**
```
You are a knowledge extraction assistant. Analyze this text and identify 3-8 key concepts, facts, or skills worth remembering.

For each item, return:
- "subject": short label (2-5 words)
- "atomType": one of "concept", "fact", "skill"
- "domain": category (e.g. "software-engineering", "finance", "language:ja")
- "sourceContext": the relevant sentence or phrase from the text (verbatim)

Return JSON array. Only include genuinely learnable items — skip obvious/trivial facts.

Text:
{section_text}
```

**Debounce mechanism:** `HashMap<String, tokio::time::Instant>` held in the service loop. On receiving `NoteContentChanged`, check if `now - last_seen[note_id] < 5s` — if so, update timestamp but skip processing. If >= 5s or first time, process and update timestamp.

The service follows the same `tokio::spawn` + `CancellationToken` pattern as `BackgroundConsolidationService` at `background.rs:L188-L270`. Use `tokio::select!` on `event_rx.recv()` and `cancel_token.cancelled()`.

- [ ] **Step 2: Export from services/mod.rs**

Add `pub mod atom_extraction;` and the public re-export.

- [ ] **Step 3: Build to verify**

Run: `cargo build -p cognitive 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(cognitive): add AtomExtractionService — auto-extract concepts from notes
```

---

### Task 6: Wire AtomExtractionService into AppCore init

**Files:**
- Modify: `crates/app-core/src/init/cognitive.rs`

- [ ] **Step 1: Start the extraction service during cognitive init**

In `crates/app-core/src/init/cognitive.rs`, add the extraction service start **alongside** the existing `BackgroundConsolidationService::start()` call. Follow the same pattern:

```rust
// ── Atom extraction service (auto-extract from notes) ─────────────
{
    let config = config_snapshot.cognitive.atom_extraction.clone();
    if config.enabled {
        if let (Some(provider), Some(bus)) = (cognitive_provider.clone(), domain_event_bus.clone()) {
            let pool = storage_pool.inner().clone();
            let token = shutdown_token.clone();
            cognitive::services::atom_extraction::AtomExtractionService::start(
                pool, provider, bus, config, token,
            );
        }
    }
}
```

This goes in `init/cognitive.rs` (NOT `init/mod.rs`) because all cognitive services are initialized together there.

- [ ] **Step 2: Build to verify**

Run: `cargo build -p app-core 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 3: Commit**

```
feat(app-core): wire AtomExtractionService into init
```

---

### Task 7: Atom decay cron — daily salience + retention decay

**Files:**
- Create: `crates/cognitive/src/services/atom_decay.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Create the decay engine**

Create `crates/cognitive/src/services/atom_decay.rs`:

```rust
use crate::repos::KnowledgeAtomRepo;
use bus::DomainEventBus;
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Run the daily atom decay cycle.
///
/// 1. Query stale active atoms (no interaction in 7+ days)
/// 2. Apply tiered salience decay based on personal_importance
/// 3. Recompute retention_pct from FSRS: R = 1/(1 + elapsed/(9*S))
/// 4. Auto-archive if salience < 0.1 AND retention < 0.3 AND stale > 30 days
/// 5. Update topic aggregates
/// 6. Emit CoachingLearningDigest
pub async fn run_decay_cycle(pool: &SqlitePool, bus: &DomainEventBus) -> common::Result<()> {
    let repo = KnowledgeAtomRepo::new(pool.clone());
    let stale = repo.list_stale_active(7).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    if stale.is_empty() {
        return Ok(());
    }

    info!("Atom decay: processing {} stale atoms", stale.len());
    let now = chrono::Utc::now();
    let mut fading_count = 0usize;
    let mut archived_count = 0usize;

    for atom in &stale {
        // Tiered salience decay
        let decay_factor = if atom.personal_importance > 0.85 {
            0.97
        } else if atom.personal_importance >= 0.5 {
            0.92
        } else {
            0.85
        };
        let new_salience = atom.salience * decay_factor;

        // FSRS retention: R = 1/(1 + t/(9*S))
        let elapsed_days = atom.last_interaction_ts.as_deref()
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_seconds() as f64 / 86400.0)
            .unwrap_or(30.0);
        let new_retention = 1.0 / (1.0 + elapsed_days / (9.0 * atom.stability));

        // Auto-archive check
        let stale_days = elapsed_days as i64;
        if new_salience < 0.1 && new_retention < 0.3 && stale_days > 30 {
            repo.dismiss(&atom.id).await.ok();
            let _ = bus.publish(bus::DomainEvent::KnowledgeAtomArchived {
                atom_id: atom.id.clone(),
                reason: "decay".to_string(),
            });
            archived_count += 1;
            continue;
        }

        // Apply decay
        repo.apply_decay(&atom.id, new_salience, new_retention).await.ok();

        if new_retention < 0.6 && atom.personal_importance > 0.7 {
            fading_count += 1;
        }
    }

    // Update all topic aggregates
    repo.update_all_topic_aggregates().await.ok();

    // Emit digest
    // (strongest/weakest topic computed from repo)
    let topics = repo.list_topics_with_atoms().await.unwrap_or_default();
    let weakest = topics.first().map(|t| t.name.clone());
    let strongest = topics.last().map(|t| t.name.clone());

    let _ = bus.publish(bus::DomainEvent::CoachingLearningDigest {
        fading_count,
        archived_count,
        streak_days: 0, // TODO: compute from review_log in Phase 3
        strongest_topic: strongest,
        weakest_topic: weakest,
    });

    info!("Atom decay complete: {fading_count} fading, {archived_count} archived");
    Ok(())
}
```

- [ ] **Step 2: Export from services/mod.rs**

Add `pub mod atom_decay;` to `crates/cognitive/src/services/mod.rs`.

- [ ] **Step 3: Register cron job**

In `crates/app-core/src/init/cron.rs`:

1. Add const: `const JOB_ATOM_DECAY: &str = "atom_decay_daily";`

2. In `register_cron_callbacks`, add a handler callback following the weekly reflection pattern. Note: `domain_event_bus` is `Arc<DomainEventBus>` — clone the `Arc`, not the bus itself:

```rust
{
    let pool = pool.clone();
    let bus = Arc::clone(&domain_event_bus);
    cron_service.register_handler(
        JOB_ATOM_DECAY,
        Arc::new(move |_job| {
            let pool = pool.clone();
            let bus = Arc::clone(&bus);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Err(e) = cognitive::services::atom_decay::run_decay_cycle(&pool, &bus).await {
                        tracing::warn!("Atom decay cycle failed: {e}");
                    }
                })
            })
        }),
    );
}
```

3. In `ensure_cron_jobs`, add the job (3 AM local):

```rust
ensure_job!(JOB_ATOM_DECAY, CronSchedule::Cron {
    expr: "0 3 * * *".to_string(),
    tz: Some(config.timezone.clone()),
}, "Daily knowledge atom decay");
```

- [ ] **Step 4: Write tests for decay logic**

Add tests in `atom_decay.rs` with concrete expected values:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_fsrs_retention_formula() {
        // R = 1/(1 + t/(9*S))
        // stability=10, elapsed=30 days → R = 1/(1 + 30/90) = 1/1.333 = 0.75
        let stability = 10.0;
        let elapsed = 30.0;
        let r = 1.0 / (1.0 + elapsed / (9.0 * stability));
        assert!((r - 0.75).abs() < 0.01);

        // stability=1, elapsed=9 days → R = 1/(1 + 9/9) = 0.5
        let r2 = 1.0 / (1.0 + 9.0 / (9.0 * 1.0));
        assert!((r2 - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_tiered_salience_decay() {
        // High importance (>0.85): factor = 0.97
        // salience=0.8, importance=0.9 → new_salience = 0.8 * 0.97 = 0.776
        let new = 0.8 * 0.97;
        assert!((new - 0.776).abs() < 0.001);

        // Low importance (<0.5): factor = 0.85
        // salience=0.5, importance=0.3 → new_salience = 0.5 * 0.85 = 0.425
        let new2 = 0.5 * 0.85;
        assert!((new2 - 0.425).abs() < 0.001);
    }

    #[test]
    fn test_auto_archive_threshold() {
        // Archive when: salience < 0.1 AND retention < 0.3 AND stale > 30 days
        let salience = 0.05;
        let retention = 0.2;
        let stale_days = 45;
        let should_archive = salience < 0.1 && retention < 0.3 && stale_days > 30;
        assert!(should_archive);

        // Don't archive if retention is OK
        let should_not = 0.05 < 0.1 && 0.6 < 0.3 && 45 > 30;
        assert!(!should_not);
    }
}
```

- [ ] **Step 5: Build and run tests**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 6: Commit**

```
feat(cognitive,app-core): add daily atom decay cron — salience + FSRS retention
```

---

### Task 8: IPC types + handlers — knowledge health

**Files:**
- Create: `crates/desktop-shared/src/commands/knowledge_health.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Create: `crates/app-core/src/handlers/knowledge_health.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Create IPC types**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHealthSummary {
    pub total_atoms: usize,
    pub active_atoms: usize,
    pub avg_retention: f64,
    pub topics: Vec<TopicHealthResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicHealthResponse {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub atom_count: i64,
    pub avg_retention: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetailParams {
    pub topic_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetailResponse {
    pub topic: TopicHealthResponse,
    pub atoms: Vec<crate::commands::KnowledgeAtomResponse>,
}
```

- [ ] **Step 2: Create AppCore handlers**

`knowledge_health_summary` → queries all topics + counts active atoms.
`knowledge_topic_detail` → queries topic + its atoms.

- [ ] **Step 3: Export modules**

- [ ] **Step 4: Build to verify**

Run: `cargo build -p app-core 2>&1 | tail -5`

- [ ] **Step 5: Commit**

```
feat(app-core,desktop-shared): add knowledge health IPC types + handlers
```

---

### Task 9: Desktop commands — knowledge health

**Files:**
- Create: `crates/desktop/src/commands/knowledge_health.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create Tauri commands**

Follow the exact pattern from `commands/atoms.rs`: `knowledge_health_summary`, `knowledge_topic_detail`. Include `DEV_COMMANDS` + `dispatch_dev`.

- [ ] **Step 2: Register in main.rs + dev server**

- [ ] **Step 3: Run dev_server parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: pass

- [ ] **Step 4: Commit**

```
feat(desktop): add knowledge health Tauri commands + dev server dispatch
```

---

### Task 10: Fill topic_name and linked_card_count in atom response

**Files:**
- Modify: `crates/app-core/src/handlers/atoms.rs`
- Modify: `crates/cognitive/src/repos/knowledge_atom.rs`

- [ ] **Step 1: Add `list_for_note_with_topic` method**

Add a method that JOINs `knowledge_topics.name` and counts linked flashcards:

```rust
pub async fn list_for_note_enriched(
    &self,
    note_id: &str,
) -> Result<Vec<(KnowledgeAtomRow, Option<String>, i64)>, sqlx::Error> {
    sqlx::query_as::<_, (KnowledgeAtomRow, Option<String>, i64)>(...)
    // or use two queries: list_for_note + batch topic lookup + batch card count
}
```

- [ ] **Step 2: Update `atom_row_to_response` to fill topic_name and linked_card_count**

Remove the `// TODO: join topic name in Phase 2` and `// TODO: count linked cards in Phase 2` comments.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(knowledge_atom)'`

- [ ] **Step 4: Commit**

```
feat(app-core): fill topic_name + linked_card_count in atom responses
```

---

### Task 11: Frontend — Knowledge Health page

**Files:**
- Create: `desktop-ui/src/features/learn/hooks/useKnowledgeHealth.ts`
- Create: `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx`
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Create useKnowledgeHealth hook**

```typescript
import { useQuery } from "@shared/hooks/useQuery";

export interface TopicHealth {
  id: string;
  name: string;
  domain: string;
  atomCount: number;
  avgRetention: number;
}

export interface KnowledgeHealthSummary {
  totalAtoms: number;
  activeAtoms: number;
  avgRetention: number;
  topics: TopicHealth[];
}

export function useKnowledgeHealth() {
  return useQuery<KnowledgeHealthSummary>("knowledge_health_summary", undefined, {
    totalAtoms: 0,
    activeAtoms: 0,
    avgRetention: 0,
    topics: [],
  });
}
```

- [ ] **Step 2: Create KnowledgeHealth page**

Topic heatmap: list of topics as colored retention bars. Color: green >= 80%, amber >= 50%, red < 50%. Each bar shows topic name, atom count, avg retention %. Click to expand to see atoms in that topic.

Follow the `glass-card rounded-xl p-5` pattern from `coaching/pages/OverviewPage.tsx`.

- [ ] **Step 3: Add route + DashboardHome link**

Add lazy route in `router.tsx` for `/learn/knowledge`.
Add a card in `DashboardHome.tsx` that links to the Knowledge Health page.

- [ ] **Step 4: Lint check**

Run: `cd desktop-ui && bun run lint 2>&1 | grep -E "KnowledgeHealth|knowledge_health" | head -10`
Expected: no errors

- [ ] **Step 5: Commit**

```
feat(desktop-ui): add Knowledge Health page with topic heatmap
```

---

### Task 12: "Why this?" popover + Bulk Accept modal

**Files:**
- Create: `desktop-ui/src/features/notes/components/WhyThisPopover.tsx`
- Create: `desktop-ui/src/features/notes/components/BulkAcceptModal.tsx`
- Modify: `desktop-ui/src/features/notes/components/AtomCard.tsx`
- Modify: `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx`
- Modify: `crates/app-core/src/handlers/atoms.rs`
- Modify: `crates/desktop-shared/src/commands/atoms.rs`
- Modify: `crates/desktop/src/commands/atoms.rs`

- [ ] **Step 1: Create WhyThisPopover component**

Small popover shown on hover over suggested atoms. Shows the `sourceContext` snippet and the `domain` as explanation for why this atom was suggested. Use Radix `HoverCard` (already in the project) or a simple `title` tooltip.

- [ ] **Step 2: Create BulkAcceptModal component**

Modal triggered by "Accept all (N)" button. Shows checkboxes for each suggested atom + an importance picker (Low/Medium/High emoji: 🔵/🟡/🔴 mapping to 0.3/0.7/1.0). Calls a new `atoms_bulk_accept` IPC command.

- [ ] **Step 3: Add `atoms_bulk_accept` handler**

In `crates/desktop-shared/src/commands/atoms.rs`, add:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomBulkAcceptParams {
    pub atom_ids: Vec<String>,
    pub personal_importance: f64,
}
```

In `crates/app-core/src/handlers/atoms.rs`, add `atoms_bulk_accept` that loops `atom_accept` for each ID. Register as Tauri command + dev server dispatch.

- [ ] **Step 4: Wire into AtomCard + KnowledgeAtomsPanel**

- AtomCard: wrap suggested atoms in `<WhyThisPopover>` (hover shows source context + domain)
- KnowledgeAtomsPanel: replace inline "Accept all" button with button that opens `<BulkAcceptModal>`

- [ ] **Step 5: Commit**

```
feat(desktop-ui): add "Why this?" popover + bulk accept modal with importance picker
```

---

### Task 13: ContextInferenceEngine — "learning" as work context

**Files:**
- Modify: `crates/activity-log/src/inference.rs`

- [ ] **Step 1: Add learning activity recognition**

In `crates/activity-log/src/inference.rs`, find where activity sources/actions are mapped to work context categories. Add recognition for learning-related activity log entries:
- `action == "flashcard_reviewed"` → context: "learning"
- `action == "translation_completed"` → context: "learning"
- `action == "note_studied"` → context: "learning"
- `action == "atom_created"` → context: "learning"
- `action == "atom_accepted"` → context: "learning"

This makes "learning" a recognized work context that appears in the work context panel and activity summaries.

- [ ] **Step 2: Build to verify**

Run: `cargo build -p activity-log 2>&1 | tail -5`

- [ ] **Step 3: Commit**

```
feat(activity-log): recognize learning activity as "learning" work context
```

---

### Task 14: Workspace toggle — "Auto-extract from new notes"

**Files:**
- Modify: `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx`

- [ ] **Step 1: Add toggle to Personalization settings**

In `PersonalizationSettings.tsx`, add a toggle row for "Auto-extract knowledge atoms from notes" in the AI section. Use the existing `config_update_section` IPC command to update `cognitive.atomExtraction.enabled`. Follow the same toggle pattern used for other boolean settings in this page (switch component with label + description).

Config path: `cognitive.atomExtraction.enabled`
IPC: `config_update_section` with `{ section: "cognitive", key: "atomExtraction.enabled", value: true/false }`

- [ ] **Step 2: Commit**

```
feat(desktop-ui): add workspace toggle for auto-extraction
```

---

### Task 15: Final integration — workspace build + test

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 2: Run all related tests**

Run: `cargo nextest run --workspace -E 'test(knowledge_atom) | test(atom_extraction) | test(atom_decay) | test(flashcard) | test(dev_server_covers)'`
Expected: all pass

- [ ] **Step 3: Clippy check**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep -E "^error|warning:.*atom|warning:.*decay|warning:.*extraction" | head -10`
Expected: zero warnings

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: formatted

- [ ] **Step 5: Frontend lint**

Run: `cd desktop-ui && bun run lint 2>&1 | grep -E "KnowledgeHealth|knowledge_health" | head -10`
Expected: no new errors

- [ ] **Step 6: Commit**

```
feat: Knowledge Atoms Phase 2 complete — extraction, decay, health dashboard
```

---

## Verification Checklist

After all tasks complete:

- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] `cd desktop-ui && bun run lint` — no new errors
- [ ] Dev server parity test passes
- [ ] Manual test: write a note about a concept → suggested atoms appear after a few seconds
- [ ] Manual test: wait for decay cron (or trigger manually) → stale atoms lose salience
- [ ] Manual test: Knowledge Health page shows topics with retention bars
- [ ] Manual test: settings toggle disables/enables extraction
