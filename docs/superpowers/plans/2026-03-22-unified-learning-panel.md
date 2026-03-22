# Unified Learning Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge Knowledge Atoms and Insight Review into a single "Learn" panel with atoms as the default tab, gap-to-atom pipeline, flashcard-atom linking, and quiz-atom interaction signals.

**Architecture:** Move the `KnowledgeAtomsPanel` from below the editor into a new `AtomsTab` component inside the existing `InsightReviewPanel` (renamed to "Learn"). Add a gap→atom backend pipeline after insight generation. Link insight flashcards to matching atoms. Emit `AtomInteracted` events from quiz submissions. Enrich insight context with accepted atom subjects.

**Tech Stack:** Rust (SQLite, tokio), React (TypeScript, Tailwind CSS), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-03-22-unified-learning-panel-design.md`

---

## File Map

### New files
| File | Responsibility |
|---|---|
| `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx` | Atoms tab component — active atoms, suggested atoms, bulk accept, inline review. Migrated from KnowledgeAtomsPanel with adaptations for panel context. |

### Modified files
| File | Changes |
|---|---|
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Add "atoms" to tab definitions, render AtomsTab, rename header "Insight Review" → "Learn", default tab to "atoms" |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add "atoms" to `TabId` union type |
| `desktop-ui/src/features/notes/components/NoteEditor.tsx` | Remove `KnowledgeAtomsPanel` import and render |
| `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx` | Rename button label "Insight Review" → "Learn" |
| `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx` | Replace SemanticFacts query with note-level atom count |
| `desktop-ui/src/features/notes/components/AtomCard.tsx` | Add "From gaps" badge when metadata.source === "gap_analysis" |
| `crates/app-core/src/handlers/notes/insight.rs` | Add `create_atoms_from_gaps()` after gap tab generation; update `insight_save_flashcards()` to match flashcards→atoms; update `note_insight_submit_quiz()` to emit AtomInteracted |
| `crates/feature-insights/src/traits.rs` | Add `search_atoms()` to CognitiveAccessor trait + NoopCognitiveAccessor |
| `crates/app-core/src/adapters/cognitive_accessor.rs` | Implement `search_atoms()` via KnowledgeAtomRepo |
| `crates/feature-insights/src/prompt_builder.rs` | Add "Already Learned" section from atoms in `build_context()` |

### Deleted files
| File | Reason |
|---|---|
| `crates/feature-insights/src/prompts.rs` | Dead duplicate — live prompts in `insight_prompts.rs` |

---

### Task 1: Create AtomsTab component + wire into InsightReviewPanel

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`

- [ ] **Step 1: Add "atoms" to TabId type**

In `desktop-ui/src/features/notes/hooks/useInsightReview.ts`, find the `TabId` type definition and add `"atoms"`:

```typescript
export type TabId = "atoms" | "synthesis" | "gaps" | "assessment" | "concept-map" | "perspectives";
```

Also update the initial state `activeTab` default from `"synthesis"` to `"atoms"`.

- [ ] **Step 2: Create AtomsTab component**

Create `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx`. This is essentially the existing `KnowledgeAtomsPanel` adapted to work inside the insight panel (no `border-b border-border` wrapper since the panel handles chrome):

```tsx
import { useSearchParams } from "react-router";
import { useEffect, useRef, useState } from "react";
import { useAtomActions, useKnowledgeAtoms } from "../../hooks/useKnowledgeAtoms";
import { AtomCard } from "../AtomCard";
import { BulkAcceptModal } from "../BulkAcceptModal";

interface AtomsTabProps {
  noteId: string | null;
}

export function AtomsTab({ noteId }: AtomsTabProps) {
  const { activeAtoms, suggestedAtoms, refetch } = useKnowledgeAtoms(noteId);
  const { accept, dismiss } = useAtomActions(noteId);
  const [bulkModalOpen, setBulkModalOpen] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();
  const highlightAtomId = searchParams.get("atomId");
  const scrolledRef = useRef(false);

  useEffect(() => {
    if (!highlightAtomId || scrolledRef.current) return;
    const el = document.querySelector(`[data-atom-id="${highlightAtomId}"]`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      scrolledRef.current = true;
      setSearchParams({}, { replace: true });
    }
  }, [highlightAtomId, setSearchParams]);

  const totalCount = activeAtoms.length + suggestedAtoms.length;

  if (totalCount === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <p className="text-[11px] text-muted-foreground text-center">
          No knowledge atoms yet. Atoms are auto-extracted when you write — check back after saving some content.
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-3 py-2">
      {/* Header */}
      <div className="flex items-center justify-between mb-1">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Knowledge Atoms ({totalCount})
        </span>
        {suggestedAtoms.length > 0 && (
          <button
            type="button"
            onClick={() => setBulkModalOpen(true)}
            className="text-[10px] text-brand hover:text-brand/80 transition-colors"
          >
            Accept all ({suggestedAtoms.length})
          </button>
        )}
      </div>

      {/* Active atoms */}
      {activeAtoms.length > 0 && (
        <div className="-mx-1">
          {activeAtoms.map((atom) => (
            <div key={atom.id} data-atom-id={atom.id}>
              <AtomCard atom={atom} onReviewDone={refetch} />
            </div>
          ))}
        </div>
      )}

      {/* Suggested atoms */}
      {suggestedAtoms.length > 0 && (
        <>
          {activeAtoms.length > 0 && <div className="my-1 border-t border-border/30" />}
          <span className="text-[9px] text-muted-foreground uppercase tracking-wider mb-0.5 block px-1">
            Suggested
          </span>
          <div className="-mx-1">
            {suggestedAtoms.map((atom) => (
              <AtomCard
                key={atom.id}
                atom={atom}
                onAccept={accept}
                onDismiss={dismiss}
                onReviewDone={refetch}
              />
            ))}
          </div>
        </>
      )}

      {/* Bulk accept modal */}
      {bulkModalOpen && noteId && (
        <BulkAcceptModal
          atoms={suggestedAtoms}
          onClose={() => {
            setBulkModalOpen(false);
            refetch();
          }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Wire AtomsTab into InsightReviewPanel**

In `InsightReviewPanel.tsx`:

1. Import `AtomsTab`:
```tsx
import { AtomsTab } from "./insight/AtomsTab";
```

2. Update TABS array to add atoms as first tab:
```tsx
const TABS: { id: TabId; label: string }[] = [
  { id: "atoms", label: "Atoms" },
  { id: "synthesis", label: "Synthesis" },
  { id: "gaps", label: "Gap Analysis" },
  { id: "assessment", label: "Self-Assessment" },
  { id: "concept-map", label: "Concept Map" },
  { id: "perspectives", label: "Perspectives" },
];
```

3. Rename header from "Insight Review" to "Learn":
```tsx
<span className="text-[12px] font-medium text-foreground flex-1">Learn</span>
```

4. Update `tabStatus` function to handle "atoms" tab (always return "done"):
```tsx
case "atoms":
  return "done";
```

5. Add atoms tab rendering in the tab content area:
```tsx
{state.activeTab === "atoms" && <AtomsTab noteId={state.noteId} />}
```

6. Update `getActiveContent()` to handle "atoms":
```tsx
case "atoms":
  return ""; // atoms tab has no copyable text content
```

- [ ] **Step 4: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`

---

### Task 2: Remove KnowledgeAtomsPanel from editor + rename button

**Files:**
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`
- Modify: `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx`

- [ ] **Step 1: Remove KnowledgeAtomsPanel from NoteEditor**

In `NoteEditor.tsx`:
1. Remove import: `import { KnowledgeAtomsPanel } from "./KnowledgeAtomsPanel";`
2. Remove render at line ~432: `<KnowledgeAtomsPanel noteId={note.id} />`

- [ ] **Step 2: Rename button in AISuggestionsPanel**

In `AISuggestionsPanel.tsx`, change the button label from "Insight Review" to "Learn":
```tsx
<Brain size={10} />
Learn
```

- [ ] **Step 3: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`

---

### Task 3: Update KnowledgeGrowthMetrics + AtomCard badge

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx`
- Modify: `desktop-ui/src/features/notes/components/AtomCard.tsx`

- [ ] **Step 1: Replace KnowledgeGrowthMetrics**

Read the current file first. Replace the `note_insight_knowledge_growth` IPC call with data from the atoms tab. Since the metrics component is inside the InsightReviewPanel which has access to `state.noteId`, pass the noteId as a prop and use `useKnowledgeAtoms(noteId)` to show "N atoms · M suggested":

```tsx
import { useKnowledgeAtoms } from "../../hooks/useKnowledgeAtoms";

interface KnowledgeGrowthMetricsProps {
  noteId: string | null;
}

export function KnowledgeGrowthMetrics({ noteId }: KnowledgeGrowthMetricsProps) {
  const { activeAtoms, suggestedAtoms } = useKnowledgeAtoms(noteId);
  const active = activeAtoms.length;
  const suggested = suggestedAtoms.length;

  if (active === 0 && suggested === 0) return null;

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-[10px] text-muted-foreground bg-accent/30 rounded-md mx-3">
      <span className="font-medium text-foreground">{active}</span> atoms
      {suggested > 0 && (
        <>
          <span>·</span>
          <span className="font-medium text-amber-400">{suggested}</span> suggested
        </>
      )}
    </div>
  );
}
```

Update the render in `InsightReviewPanel.tsx` to pass `noteId`:
```tsx
<KnowledgeGrowthMetrics noteId={state.noteId} />
```

- [ ] **Step 2: Add "From gaps" badge to AtomCard**

In `AtomCard.tsx`, read the current file first. Find where the atom subject is rendered. Parse `atom.metadata` (JSON string) and if `source === "gap_analysis"`, render a small badge:

```tsx
const metadata = atom.metadata ? JSON.parse(atom.metadata) : null;
const isFromGaps = metadata?.source === "gap_analysis";
```

Render next to the subject:
```tsx
{isFromGaps && (
  <span className="text-[8px] px-1 py-0.5 rounded bg-amber-500/15 text-amber-400 shrink-0">
    From gaps
  </span>
)}
```

- [ ] **Step 3: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`

---

### Task 4: Backend — Gap Analysis → Atom creation pipeline

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Add gap→atom helper function**

After the `run_insight_pipeline` function, add a new helper:

```rust
/// Parse gap analysis JSON and create suggested atoms for unmatched gaps.
async fn create_atoms_from_gaps(
    pool: &sqlx::SqlitePool,
    note_id: &str,
    gap_content: &str,
    insight_review_id: &str,
    domain_event_bus: &Option<Arc<bus::DomainEventBus>>,
) {
    // Parse JSON block from gap content (same format as frontend: trailing ```json block)
    let Some(json_start) = gap_content.find("```json") else { return };
    let json_body = &gap_content[json_start + 7..];
    let Some(json_end) = json_body.find("```") else { return };
    let json_str = json_body[..json_end].trim();

    let gaps: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return,
    };

    let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());

    for gap in &gaps {
        let Some(topic) = gap.get("topic").and_then(|v| v.as_str()) else { continue };
        let description = gap.get("description").and_then(|v| v.as_str()).unwrap_or("");

        // Check if atom already exists for this subject on this note
        let existing = atom_repo.find_by_subject_across_notes(topic).await.unwrap_or_default();
        if existing.iter().any(|a| a.source_note_id.as_deref() == Some(note_id)) {
            continue;
        }

        let metadata = serde_json::json!({
            "source": "gap_analysis",
            "insight_review_id": insight_review_id,
        });

        let new_atom = cognitive::NewKnowledgeAtom {
            subject: topic.to_string(),
            atom_type: "concept".to_string(),
            domain: "general".to_string(),
            source_note_id: Some(note_id.to_string()),
            source_context: Some(description.to_string()),
            metadata: Some(metadata.to_string()),
            ..Default::default()
        };

        match atom_repo.create(&new_atom).await {
            Ok(atom) => {
                // Assign topic
                if let Ok(t) = atom_repo.get_or_create_topic(&atom.domain, &atom.domain).await {
                    let _ = sqlx::query("UPDATE knowledge_atoms SET topic_id = ?1 WHERE id = ?2")
                        .bind(&t.id)
                        .bind(&atom.id)
                        .execute(pool)
                        .await;
                }
                if let Some(bus) = domain_event_bus {
                    bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                        atom_id: atom.id,
                        atom_type: "concept".to_string(),
                        domain: "general".to_string(),
                        source_note_id: Some(note_id.to_string()),
                        personal_importance: 0.7,
                    });
                }
            }
            Err(e) => {
                tracing::debug!("Failed to create gap atom: {e}");
            }
        }
    }
}
```

- [ ] **Step 2: Call create_atoms_from_gaps after gap tab generation**

In `run_insight_pipeline`, find where the gap tab content is stored (after `insight:tab-done` with tab="gaps"). After the content is stored, spawn the gap→atom creation:

```rust
// After gap content is available and stored:
if let Some(gap_content) = &content.gap_analysis {
    let pool = pool.clone();
    let note_id = note_id.clone();
    let insight_id = insight_review_id.clone();
    let bus = domain_event_bus.clone();
    tokio::spawn(async move {
        create_atoms_from_gaps(&pool, &note_id, gap_content, &insight_id, &bus).await;
    });
}
```

Read the `run_insight_pipeline` function carefully to find the right insertion point — it's after all tabs are generated and `store_insight` is called.

- [ ] **Step 3: Build and test**

Run: `cargo build -p app-core 2>&1 | tail -5`

---

### Task 5: Backend — Flashcard → Atom linking

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Update insight_save_flashcards to match atoms**

In `insight_save_flashcards()`, before creating the flashcards batch, query active atoms for the note and try to match each question to an atom:

```rust
// Before the cards Vec creation:
let atom_repo = cognitive::KnowledgeAtomRepo::new(self.storage_pool.inner().clone());
let active_atoms = atom_repo
    .list_for_note(&params.note_id, "active")
    .await
    .unwrap_or_default();
```

Then in the map closure that creates `NewFlashcard`, replace `atom_id: None` with a match attempt:

```rust
atom_id: active_atoms.iter()
    .find(|a| q.question.to_lowercase().contains(&a.subject.to_lowercase())
        || a.subject.to_lowercase().contains(&q.question.split_whitespace().take(3).collect::<Vec<_>>().join(" ").to_lowercase()))
    .map(|a| a.id.clone()),
```

This is a best-effort keyword match — if no atom matches, it falls back to `None` (no regression).

- [ ] **Step 2: Build and test**

Run: `cargo build -p app-core 2>&1 | tail -5`

---

### Task 6: Backend — Quiz score → AtomInteracted events

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Update note_insight_submit_quiz to emit atom events**

In `note_insight_submit_quiz()`, after computing progress, add atom interaction:

```rust
// After service.compute_progress(...):

// Emit AtomInteracted for quiz questions that match atoms
if let Some(bus) = &self.domain_event_bus {
    let atom_repo = cognitive::KnowledgeAtomRepo::new(self.storage_pool.inner().clone());
    if let Ok(atoms) = atom_repo.list_for_note(&insight.note_id, "active").await {
        // Update last_interaction_ts for all active atoms on this note
        // (the user demonstrated engagement by taking the quiz)
        for atom in &atoms {
            let _ = atom_repo.touch_interaction(&atom.id).await;
            bus.publish(bus::DomainEvent::AtomInteracted {
                atom_id: atom.id.clone(),
                interaction_type: "quiz_answer".to_string(),
                note_id: Some(insight.note_id.clone()),
            });
        }
    }
}
```

Note: `touch_interaction` updates `last_interaction_ts = now()`. Check if this method exists on `KnowledgeAtomRepo`; if not, add it.

- [ ] **Step 2: Add touch_interaction if missing**

Check `crates/cognitive/src/repos/knowledge_atom.rs` for a `touch_interaction` method. If it doesn't exist, add:

```rust
pub async fn touch_interaction(&self, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE knowledge_atoms SET last_interaction_ts = ?1 WHERE id = ?2")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build --workspace 2>&1 | grep "^error" | head -5`

---

### Task 7: Backend — Context enrichment with atoms

**Files:**
- Modify: `crates/feature-insights/src/traits.rs`
- Modify: `crates/app-core/src/adapters/cognitive_accessor.rs`
- Modify: `crates/feature-insights/src/prompt_builder.rs`

- [ ] **Step 1: Add search_atoms to CognitiveAccessor trait**

In `traits.rs`, add to the `CognitiveAccessor` trait:
```rust
/// Get accepted knowledge atom subjects for a note.
async fn search_atoms(&self, note_id: &str) -> Vec<String>;
```

Add to `NoopCognitiveAccessor`:
```rust
async fn search_atoms(&self, _: &str) -> Vec<String> {
    Vec::new()
}
```

- [ ] **Step 2: Implement in CognitiveAccessorImpl**

In `cognitive_accessor.rs`, read the file to find the struct fields. Add `KnowledgeAtomRepo` if not present, or construct it from the pool. Implement:

```rust
async fn search_atoms(&self, note_id: &str) -> Vec<String> {
    let atom_repo = cognitive::KnowledgeAtomRepo::new(self.pool.clone());
    atom_repo
        .list_for_note(note_id, "active")
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.subject)
        .collect()
}
```

- [ ] **Step 3: Add "Already Learned" section to prompt builder**

In `prompt_builder.rs`, inside `build_context()`, after the cognitive data section (after the rules section, before the parent insight section), add:

```rust
// Section 3c: Already-learned atoms (prevents redundant synthesis)
let atom_subjects = self.cognitive.search_atoms(&note.id).await;
if !atom_subjects.is_empty() {
    sections.push(format!(
        "## Already Learned\nThe user has accepted these concepts as known: {}.\n\
         Consider these as established knowledge — don't re-explain them in the synthesis.\n\
         Focus gap analysis on what's NOT yet covered.",
        atom_subjects.join(", ")
    ));
}
```

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace 2>&1 | grep "^error" | head -5`

---

### Task 8: Cleanup — delete dead files

**Files:**
- Delete: `crates/feature-insights/src/prompts.rs`
- Modify: `crates/feature-insights/src/lib.rs` (remove `pub mod prompts;` if present)

- [ ] **Step 1: Check if prompts.rs is referenced anywhere**

Run: `grep -r "prompts::" crates/feature-insights/src/ --include="*.rs"` to verify no active code references the dead module.

- [ ] **Step 2: Delete the file and remove module declaration**

Delete `crates/feature-insights/src/prompts.rs`. In `crates/feature-insights/src/lib.rs`, remove `pub mod prompts;` if it exists.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p feature-insights 2>&1 | tail -5`

---

### Task 9: Final integration — build + lint + test

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -5`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -10`

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Run related tests**

Run: `cargo nextest run --workspace -E 'test(dev_server_covers) | test(knowledge_atom) | test(insight)' 2>&1 | tail -5`

---

## Verification Checklist

- [ ] "Learn" button appears in AI Suggestions panel (was "Insight Review")
- [ ] Opening the panel shows Atoms tab by default with active + suggested atoms
- [ ] All 5 insight tabs still work (Synthesis, Gaps, Quiz, Map, Perspectives)
- [ ] KnowledgeAtomsPanel no longer appears below editor content
- [ ] Generating Gap Analysis creates suggested atoms with "From gaps" badge
- [ ] Saving flashcards from quiz links them to matching atoms (atom_id set)
- [ ] Submitting quiz emits AtomInteracted events + updates last_interaction_ts
- [ ] Insight context includes "Already Learned" section with accepted atom subjects
- [ ] "N atoms · M suggested" replaces "N new facts" in panel header
- [ ] Dead `prompts.rs` file deleted
- [ ] `cargo build --workspace` clean
- [ ] `cargo clippy` zero warnings
- [ ] `cd desktop-ui && bun run lint` no new errors
