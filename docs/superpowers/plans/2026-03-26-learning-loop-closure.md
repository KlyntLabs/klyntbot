# Learning Loop Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all 5 gaps in the Notes ↔ Learn lifecycle loop so that review feedback persists in the knowledge graph, flashcards are linked to atoms, insight reviews are retention-aware, weak cards surface for regeneration, and note-level retention health is visible.

**Architecture:** Backend-first — modify Rust handlers to persist atoms and link flashcards, enrich the cognitive accessor trait for retention-aware prompts, add repo methods for weak card detection and note retention aggregation, then wire frontend indicators. Final task: end-to-end Chrome testing of the full loop.

**Tech Stack:** Rust (sqlx, tokio, Tauri 2), TypeScript/React (Tailwind v4, SWR useQuery), LLM (cognitive_provider for grading/insight)

**Spec:** Gaps identified from lifecycle audit — no separate spec document (inline).

---

### Task 1: Persist `socratic_exchange` + `flashcard_weak_spot` atoms in grading.rs

**Files:**
- Modify: `crates/app-core/src/handlers/notes/grading.rs` (~L222-L235, ~L340-L357)

Currently both paths publish `KnowledgeAtomCreated` with a fabricated UUID but never call `atom_repo.create()`. The bus event is discarded by the salience filter (`SalienceVerdict::Discard`). Fix: persist the atom inline before publishing the event, so the UUID corresponds to a real DB row.

- [ ] **Step 1: Read the grading.rs file to get current state**

Read `crates/app-core/src/handlers/notes/grading.rs` fully. Note the `flashcard_submit_answer` method (~L108-L237) and `flashcard_explain_answer` method (~L285-L357). Both need the same fix pattern.

- [ ] **Step 2: Add atom persistence to `flashcard_submit_answer` (weak spot path)**

In `flashcard_submit_answer`, find the block at ~L222-L235 where `flashcard_weak_spot` is published. Replace the bus-only pattern with: create the atom via repo first, then publish with the real ID.

**IMPORTANT:** `NewKnowledgeAtom` does NOT have an `id` field — `KnowledgeAtomRepo::create()` generates the UUID internally and returns it in the `KnowledgeAtomRow`. Use the returned row's `.id` for the bus event.

Also check how other handlers access the atom repo — `language.rs` uses `if let Some(atom_repo) = &self.knowledge_atom_repo`. The field is an `Option<Arc<KnowledgeAtomRepo>>` on `AppCore`. Read `crates/app-core/src/state.rs` to confirm the exact field name.

```rust
// Replace the existing bus-only block with:
if let Some(score) = response.score {
    if score < 0.6 {
        if let Some(atom_repo) = &self.knowledge_atom_repo {
            let topic_id = atom_repo
                .get_or_create_topic(&card.deck, &card.deck)
                .await
                .ok()
                .map(|t| t.id);
            let new_atom = cognitive::NewKnowledgeAtom {
                subject: format!("Weak: {}", card.front.chars().take(80).collect::<String>()),
                atom_type: "flashcard_weak_spot".to_string(),
                domain: card.deck.clone(),
                source_note_id: card.source_note_id.clone(),
                source_context: Some(format!("Score: {:.0}% on: {}", score * 100.0, card.front)),
                topic_id,
                personal_importance: 0.7,
                status: "active".to_string(),
                ..Default::default()
            };
            if let Ok(created) = atom_repo.create(&new_atom).await {
                if let Some(bus) = &self.domain_event_bus {
                    bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                        atom_id: created.id,
                        atom_type: "flashcard_weak_spot".to_string(),
                        domain: card.deck.clone(),
                        source_note_id: card.source_note_id.clone(),
                        personal_importance: 0.7,
                    });
                }
            }
        }
    }
}
```

**Note:** `NewKnowledgeAtom` likely derives `Default` (check by reading `knowledge_atom.rs`). The `..Default::default()` fills any remaining fields. The returned `KnowledgeAtomRow` from `create()` contains the real `id` which is then used in the bus event.

- [ ] **Step 3: Add atom persistence to `flashcard_explain_answer` (socratic exchange path)**

Same pattern at ~L340-L357. Replace the bus-only block:

**IMPORTANT:** Same `NewKnowledgeAtom` pattern as Step 2 — no `id` field, use returned `created.id`. Also: keep the bus publish OUTSIDE the atom_repo guard so events still fire when atom_repo is None (preserving existing behavior for coaching subscribers).

```rust
// First try to persist the atom
let mut atom_id_for_event = uuid::Uuid::new_v4().to_string();
let mut persisted = false;
if let Some(atom_repo) = &self.knowledge_atom_repo {
    let topic_id = atom_repo
        .get_or_create_topic(&card.deck, &card.deck)
        .await
        .ok()
        .map(|t| t.id);
    let new_atom = cognitive::NewKnowledgeAtom {
        subject: format!("Socratic: {}", card.front.chars().take(80).collect::<String>()),
        atom_type: "socratic_exchange".to_string(),
        domain: card.deck.clone(),
        source_note_id: card.source_note_id.clone(),
        source_context: Some(explanation.chars().take(200).collect()),
        topic_id,
        personal_importance: 0.6,
        status: "active".to_string(),
        ..Default::default()
    };
    if let Ok(created) = atom_repo.create(&new_atom).await {
        atom_id_for_event = created.id;
        persisted = true;
    }
}
// Always publish bus event (preserves existing behavior for coaching subscribers)
let saved = if let Some(bus) = &self.domain_event_bus {
    bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
        atom_id: atom_id_for_event,
        atom_type: "socratic_exchange".to_string(),
        domain: card.deck.clone(),
        source_note_id: card.source_note_id.clone(),
        personal_importance: 0.6,
    });
    persisted
} else {
    false
};
```

- [ ] **Step 4: Build and clippy**

Run: `cargo build -p app-core && cargo clippy -p app-core --all-targets`
Expected: Compiles, zero new warnings.

- [ ] **Step 5: Write test for weak spot atom persistence**

Add test in `crates/app-core/` test suite (or as an integration test in `tests/`) that:
1. Creates a flashcard via the test harness
2. Calls `flashcard_submit_answer` with a deliberately wrong answer
3. Verifies a `knowledge_atoms` row exists with `atom_type = 'flashcard_weak_spot'` and `source_note_id` matching the card's note

If the full `flashcard_submit_answer` requires an LLM (it does for borderline cases), test the inline atom creation logic directly instead — extract the atom creation into a helper method and unit test that.

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p app-core`
Expected: All pass.

- [ ] **Step 7: Commit**

```
git commit -m "fix(learn): persist socratic_exchange and flashcard_weak_spot atoms to DB

Previously these atoms were published as bus events with fabricated UUIDs
but never written to knowledge_atoms. Now persisted inline before publishing,
so review weak spots appear in Knowledge Health and feed into Insight Review.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Link `atom_id` during AI flashcard generation

**Files:**
- Modify: `crates/app-core/src/handlers/notes/card_generation.rs` (~L201-L255)
- Modify: `crates/cognitive/src/repos/knowledge_atom.rs` (add `find_by_subjects_in_domain`)
- Modify: `crates/cognitive/src/repos/flashcard.rs` (add `update_atom_id` method)

Currently `flashcard_save_generated` sets `atom_id: None` on all cards (L216). Follow the `language_save_vocabulary` pattern: after creating flashcards, match each card's front text against existing atoms by subject similarity in the same domain, and link them.

- [ ] **Step 1: Add `list_active_for_note` to KnowledgeAtomRepo (if not already present)**

Check if `list_for_note` already returns active atoms. If so, reuse it. The atom-to-card matching will use the atom's `subject` field matched against the card's front text.

**The approach:** Instead of fuzzy SQL LIKE (which won't match "What is FSRS-5?" against atom subject "FSRS-5"), load all active atoms for the note and do a Rust-side substring match: check if the card front text contains the atom subject or vice versa.

No new repo method needed if `list_for_note` already exists (confirmed at `knowledge_atom.rs:173`). If it only returns for a specific note, also add `list_active_by_domain`:

```rust
/// List active atoms in a given domain.
pub async fn list_active_by_domain(
    &self,
    domain: &str,
) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
    sqlx::query_as::<_, KnowledgeAtomRow>(
        "SELECT * FROM knowledge_atoms WHERE status = 'active' AND domain = ?1 LIMIT 100",
    )
    .bind(domain)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 2: Add `update_atom_id` to FlashcardRepo**

In `crates/cognitive/src/repos/flashcard.rs`, add:

```rust
/// Link a flashcard to a knowledge atom.
pub async fn update_atom_id(&self, card_id: &str, atom_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE flashcards SET atom_id = ?1 WHERE id = ?2")
        .bind(atom_id)
        .bind(card_id)
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 3: Wire atom linking in `flashcard_save_generated`**

In `crates/app-core/src/handlers/notes/card_generation.rs`, after the `repo.create_batch(cards).await` call (~L236) and before the embed block, add atom linking:

```rust
// Link flashcards to existing atoms by subject match.
// Load all active atoms for this note (or by domain), then do Rust-side matching:
// check if card front contains atom subject (case-insensitive).
// This handles "What is FSRS-5?" matching atom subject "FSRS-5".
if let Some(atom_repo) = &self.knowledge_atom_repo {
    // Try note-specific atoms first, fall back to domain-wide
    let atoms = if let Some(nid) = &params.note_id {
        atom_repo.list_for_note(nid).await.unwrap_or_default()
    } else {
        atom_repo.list_active_by_domain(&params.deck).await.unwrap_or_default()
    };
    let active_atoms: Vec<_> = atoms.into_iter().filter(|a| a.status == "active").collect();

    for row in &rows {
        let front_lower = row.front.to_lowercase();
        if let Some(atom) = active_atoms.iter().find(|a| {
            front_lower.contains(&a.subject.to_lowercase())
        }) {
            let _ = repo.update_atom_id(&row.id, &atom.id).await;
        }
    }
}
```

The matching checks if the card's front text contains the atom's subject string (case-insensitive). Atom subjects are concise ("FSRS-5", "ownership", "useEffect") so they naturally substring-match against question sentences.

Check the exact field name for atom_repo on `AppCore` — `language.rs` uses `&self.knowledge_atom_repo`. Read `crates/app-core/src/state.rs` to confirm.

- [ ] **Step 4: Build and clippy**

Run: `cargo build -p app-core -p cognitive && cargo clippy -p app-core -p cognitive --all-targets`

- [ ] **Step 5: Test**

Run: `cargo nextest run -p cognitive -E 'test(flashcard)' && cargo nextest run -p app-core`

- [ ] **Step 6: Commit**

```
git commit -m "feat(learn): link atom_id during AI flashcard generation

After creating flashcards from a note, match each card's front text against
existing atoms in the same domain and set atom_id. This enables atom retention
updates on review and FIRe domain-based propagation.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Enrich gap analysis with atom retention data

**Files:**
- Modify: `crates/feature-insights/src/prompt_builder.rs` (~L94-L100)
- Modify: `crates/feature-insights/src/lib.rs` or wherever `CognitiveAccessor` trait is defined
- Modify: `crates/app-core/src/adapters/cognitive_accessor.rs` (~L156-L165)

Currently `search_atoms()` returns `Vec<String>` (subjects only). Change it to return subject + retention so the prompt can distinguish fading vs strong knowledge.

- [ ] **Step 1: Find and modify the CognitiveAccessor trait**

Search for the `CognitiveAccessor` trait definition — it's likely in `crates/feature-insights/src/lib.rs` or a `traits.rs` file. Find the `search_atoms` method signature.

Change the return type from `Vec<String>` to `Vec<AtomWithRetention>`:

```rust
pub struct AtomWithRetention {
    pub subject: String,
    pub retention_pct: f64,
}

// In the trait:
async fn search_atoms(&self, note_id: &str) -> Vec<AtomWithRetention>;
```

- [ ] **Step 2: Update the implementation in cognitive_accessor.rs**

In `crates/app-core/src/adapters/cognitive_accessor.rs`, modify `search_atoms`:

```rust
async fn search_atoms(&self, note_id: &str) -> Vec<AtomWithRetention> {
    match self.atom_repo.list_for_note(note_id).await {
        Ok(atoms) => atoms
            .into_iter()
            .filter(|a| a.status == "active")
            .map(|a| AtomWithRetention {
                subject: a.subject,
                retention_pct: a.retention_pct,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
```

- [ ] **Step 3: Update the prompt builder to use retention data**

In `crates/feature-insights/src/prompt_builder.rs`, modify the "Already Learned" section (~L94-L100):

```rust
if !atoms.is_empty() {
    let mut strong = Vec::new();
    let mut fading = Vec::new();
    for a in &atoms {
        if a.retention_pct >= 0.6 {
            strong.push(a.subject.as_str());
        } else {
            fading.push(format!("{} ({}% retention)", a.subject, (a.retention_pct * 100.0) as u32));
        }
    }
    let mut section = String::from("## Knowledge State\n");
    if !strong.is_empty() {
        section.push_str(&format!(
            "Strong knowledge (don't re-explain): {}.\n",
            strong.join(", ")
        ));
    }
    if !fading.is_empty() {
        section.push_str(&format!(
            "Fading knowledge (may need reinforcement): {}.\n\
             Consider these as priority areas for gap analysis.\n",
            fading.join(", ")
        ));
    }
    sections.push(section);
}
```

- [ ] **Step 4: Update NoopCognitiveAccessor + all callers**

**CRITICAL:** The `CognitiveAccessor` trait has a `NoopCognitiveAccessor` impl (in `crates/feature-insights/src/traits.rs` ~L67) that returns `Vec::new()` for `search_atoms`. This must be updated to return `Vec<AtomWithRetention>` or it won't compile.

Also search for ALL call sites of `search_atoms` across `crates/`:

```bash
grep -rn "search_atoms" crates/
```

Update each to work with `Vec<AtomWithRetention>`. Callers that only need subjects can map: `.iter().map(|a| a.subject.clone()).collect()`.

Known locations:
1. `NoopCognitiveAccessor` in `traits.rs` — change return to `vec![]` (same, but type changes)
2. The concrete impl in `cognitive_accessor.rs` — already updated in Step 2
3. `prompt_builder.rs` — already updated in Step 3
4. Any other callers found by grep

- [ ] **Step 5: Build and clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets`

- [ ] **Step 6: Commit**

```
git commit -m "feat(insights): enrich gap analysis with atom retention data

search_atoms now returns AtomWithRetention (subject + retention_pct).
The prompt builder categorizes atoms as 'strong' (≥60%) or 'fading'
with explicit retention percentages, enabling smarter gap analysis.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Surface weak cards for regeneration

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs` (add `list_struggling_cards`)
- Create: `crates/app-core/src/handlers/notes/struggling_cards.rs` (new handler)
- Modify: `crates/app-core/src/handlers/notes/mod.rs` (register module)
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add IPC type)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri command + DEV_COMMANDS + dispatch_dev)
- Modify: `crates/desktop/src/main.rs` (register command)
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx` (add "Needs Attention" section)

- [ ] **Step 1: Add `list_struggling_cards` to FlashcardRepo**

In `crates/cognitive/src/repos/flashcard.rs`, add:

```rust
/// Cards with 3+ lapses that haven't been suspended — candidates for regeneration.
pub async fn list_struggling_cards(&self, limit: i64) -> Result<Vec<FlashcardRow>, sqlx::Error> {
    sqlx::query_as::<_, FlashcardRow>(
        r#"SELECT * FROM flashcards
           WHERE lapses >= 3 AND suspended = 0
           ORDER BY lapses DESC, review_count DESC
           LIMIT ?1"#,
    )
    .bind(limit)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] **Step 2: Create handler + IPC type + Tauri command**

Add `StrugglingCardResponse` to `desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrugglingCardResponse {
    pub id: String,
    pub front: String,
    pub back: String,
    pub deck: String,
    pub lapses: i64,
    pub review_count: i64,
    pub source_note_id: Option<String>,
}
```

Create `crates/app-core/src/handlers/notes/struggling_cards.rs`:

```rust
use desktop_shared::commands::StrugglingCardResponse;
use desktop_shared::errors::ApiError;
use crate::state::AppCore;

impl AppCore {
    pub async fn flashcard_list_struggling(
        &self,
        limit: i64,
    ) -> Result<Vec<StrugglingCardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let cards = repo
            .list_struggling_cards(limit)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(|c| StrugglingCardResponse {
            id: c.id,
            front: c.front,
            back: c.back,
            deck: c.deck,
            lapses: c.lapses,
            review_count: c.review_count,
            source_note_id: c.source_note_id,
        }).collect())
    }
}
```

Register module in `notes/mod.rs`. Add Tauri command `flashcard_list_struggling` in `commands/notes.rs` with `DEV_COMMANDS` + `dispatch_dev`. Register in `main.rs`.

- [ ] **Step 3: Add "Needs Attention" section to DashboardHome**

In `desktop-ui/src/features/learn/components/DashboardHome.tsx`, add a hook call:

```ts
const { data: struggling } = useQuery<StrugglingCard[]>("flashcard_list_struggling", { limit: 5 }, []);
```

Before the DeckList, render a "Needs Attention" section if struggling cards exist:

```tsx
{struggling.length > 0 && (
  <div className="space-y-2">
    <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider px-1">
      Needs Attention
    </h3>
    {struggling.map((card) => (
      <div key={card.id} className="glass-card px-3 py-2.5 flex items-center justify-between">
        <div className="min-w-0 flex-1">
          <p className="text-sm text-foreground truncate">{card.front}</p>
          <p className="text-[11px] text-red-400">{card.lapses} lapses</p>
        </div>
        <button
          type="button"
          onClick={() => onGenerateFromText(card.front + "\n\nExpected answer: " + card.back)}
          className="glass-button px-2.5 py-1 text-xs text-foreground"
        >
          Regenerate
        </button>
      </div>
    ))}
  </div>
)}
```

The "Regenerate" button feeds the card's content into the existing `generateFromText` pipeline, which will create better replacement cards via AI.

- [ ] **Step 4: Build, clippy, lint**

Run: `cargo build -p desktop && cargo clippy -p desktop --all-targets`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```
git commit -m "feat(learn): surface struggling cards (3+ lapses) for regeneration

Adds flashcard_list_struggling endpoint and 'Needs Attention' section
on the dashboard showing cards with 3+ lapses and a Regenerate button.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Note-level retention health signal

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs` (add `note_retention_health`)
- Modify: `crates/app-core/src/handlers/notes/crud.rs` (add `note_retention_health` handler)
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add response type)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri command)
- Modify: `crates/desktop/src/main.rs` (register)

This provides an API for the notes feature to query "how healthy is the knowledge from this note?" — average retention of all flashcards + atoms linked to this note.

- [ ] **Step 1: Add `note_retention_health` to FlashcardRepo**

In `crates/cognitive/src/repos/flashcard.rs`:

**IMPORTANT:** SQLite aggregate queries always return exactly one row — even when no rows match, you get `(NULL, 0, NULL)`. Using `fetch_optional` won't help because it returns `Some(row)` with NULL values that fail sqlx deserialization for non-Option types. Use `COALESCE` to handle NULLs and `fetch_one`:

```rust
/// Average retention metrics for all flashcards linked to a note.
pub async fn note_retention_health(
    &self,
    note_id: &str,
) -> Result<Option<(f64, i64, i64)>, sqlx::Error> {
    let row: (f64, i64, i64) = sqlx::query_as(
        r#"SELECT
            COALESCE(AVG(stability), 0.0) as avg_stability,
            COUNT(*) as total_cards,
            COALESCE(SUM(lapses), 0) as total_lapses
           FROM flashcards
           WHERE source_note_id = ?1 AND suspended = 0"#,
    )
    .bind(note_id)
    .fetch_one(&self.pool)
    .await?;
    // COUNT(*) = 0 means no cards for this note
    if row.1 == 0 {
        Ok(None)
    } else {
        Ok(Some(row))
    }
}
```

- [ ] **Step 2: Add handler + IPC type + Tauri command**

Add `NoteRetentionHealthResponse` to `desktop-shared/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteRetentionHealthResponse {
    pub avg_stability: f64,
    pub total_cards: i64,
    pub total_lapses: i64,
    pub health_score: f64, // 0.0–1.0, derived from stability + lapses
}
```

Handler in `crud.rs` (or a new `notes/retention.rs`):

```rust
pub async fn note_retention_health(
    &self,
    note_id: String,
) -> Result<Option<NoteRetentionHealthResponse>, ApiError> {
    let repo = self.flashcard_repo()?;
    let result = repo
        .note_retention_health(&note_id)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
    Ok(result.map(|(avg_stab, total_cards, total_lapses)| {
        let lapse_penalty = (total_lapses as f64 / total_cards.max(1) as f64).min(1.0) * 0.3;
        let health = (avg_stab / 30.0).min(1.0) * (1.0 - lapse_penalty);
        NoteRetentionHealthResponse {
            avg_stability: avg_stab,
            total_cards,
            total_lapses,
            health_score: health.clamp(0.0, 1.0),
        }
    }))
}
```

Add Tauri command `note_retention_health` in `commands/notes.rs` with `DEV_COMMANDS` + `dispatch_dev`. Register in `main.rs`.

- [ ] **Step 3: Build, clippy, test registration**

Run: `cargo build -p desktop && cargo clippy -p desktop --all-targets`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`

- [ ] **Step 4: Commit**

```
git commit -m "feat(notes): add note_retention_health endpoint

Returns average stability, total cards, total lapses, and a derived
health score for all flashcards linked to a note. Enables the notes
editor to show retention health indicators.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Final — Full Build + Lint + Tests

**Files:** None (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: No new warnings from our changes.

- [ ] **Step 3: All Rust tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Format check**

Run: `cargo fmt --all --check`
Fix any formatting issues in our files only.

- [ ] **Step 6: Commit any fixes**

---

### Task 7: End-to-End Chrome Testing — Full Notes ↔ Learn Loop

**Files:** None (testing only, uses Chrome browser automation)

This task tests the complete lifecycle loop through the frontend to verify all 5 gaps are closed. Run with the Tauri dev server active (`cargo tauri dev` or `cd desktop-ui && bun run dev`).

- [ ] **Step 1: Create a note with content**

Navigate to `/#/notes`. Create a new note titled "Test: Rust Ownership" with body content about ownership, borrowing, and lifetimes. Wait for atom extraction (5s debounce + LLM call).

Verify via API: `POST /api/atoms_for_note { noteId: "<id>" }` returns suggested atoms.

- [ ] **Step 2: Accept atoms and generate flashcards**

Accept 2-3 suggested atoms via `POST /api/atom_accept { id: "<atom_id>" }`.

Click the flashcard generation button on the note (or use API: `POST /api/flashcard_generate { noteId: "<id>" }`). Approve and save the generated cards.

Verify: `POST /api/flashcard_list_cards { deck: "..." }` — cards have `sourceNoteId` set.

**Verify Gap 2 fix:** Check that at least some saved cards have `atomId` set (not null) — this confirms the atom linking worked.

- [ ] **Step 3: Review cards — test AI grading + Socratic**

Navigate to `/#/learn`. Switch to Type mode. Answer a card incorrectly (score < 0.6).

Verify:
- AI grading shows score, explanation, key concepts
- Socratic "Let's understand why" chip appears (on flip mode with Hard rating)

**Verify Gap 1 fix:** `POST /api/knowledge_health_summary` — should show `totalAtoms` increased (new `flashcard_weak_spot` atom was persisted).

- [ ] **Step 4: Check Knowledge Health page**

Navigate to `/#/learn/knowledge`.

Verify:
- Topics tab shows the domain from the review with atom counts
- The weak spot atom from the poor review is visible
- Trends tab shows retention chart data

- [ ] **Step 5: Check Insight Review reflects retention**

Navigate back to the note. Open Insight Review.

**Verify Gap 3 fix:** The gap analysis prompt should reference fading atoms differently from strong ones. Check if the gap analysis mentions concepts you scored poorly on as "fading" or "needs reinforcement."

This is hard to verify directly from the UI — alternatively, check via console/network that the insight API call is made and the response references atom retention.

- [ ] **Step 6: Test struggling cards dashboard**

**Verify Gap 4:** If any cards have 3+ lapses, they should appear in the "Needs Attention" section on `/#/learn`. Since we may not have enough reviews for 3 lapses, verify via API: `POST /api/flashcard_list_struggling { limit: 5 }` returns the expected shape.

- [ ] **Step 7: Test note retention health**

**Verify Gap 5:** `POST /api/note_retention_health { noteId: "<id>" }` returns `{ avgStability, totalCards, totalLapses, healthScore }` for the note we created.

- [ ] **Step 8: Test the Source button loop closure**

During review, click the "Source" button (or press S). Verify it navigates to the original note. Edit the note slightly → wait for atom re-extraction → verify new atoms appear. This confirms the full loop: note → atoms → flashcards → review → back to note → new atoms.

- [ ] **Step 9: Verify coaching integration**

Check `POST /api/review_stats_summary` returns updated streak and weekly data reflecting all the reviews done during testing.

Check `POST /api/morning_briefing_summary` — if the user has fading atoms, they should appear in the briefing's learning section.
