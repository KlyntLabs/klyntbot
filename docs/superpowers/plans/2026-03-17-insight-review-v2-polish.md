# Insight Review V2 — Polish Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close out the Insight Review Design Spec by implementing the two remaining features: a "What's Changed" banner showing cross-session diffs, and scenario challenges in the Self-Assessment quiz.

**Architecture:** The "What's Changed" banner fires a lightweight LLM diff call comparing previous synthesis with new context when a cache miss occurs. The scenario challenge generates an applied "what-if" scenario after 50%+ quiz completion, triggered on-demand from the Self-Assessment tab. Both features follow the existing event-driven tab pipeline pattern.

**Tech Stack:** Rust (serde, providers), TypeScript/React, Tauri SSE events

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/components/insight/ChangesBanner.tsx` | Renders the "What's Changed" summary between old and new insight |
| `desktop-ui/src/features/notes/components/insight/ScenarioChallenge.tsx` | Renders a scenario challenge card with situation, questions, and model answer |

### Modified files

| File | Change |
|------|--------|
| `crates/app-core/src/handlers/notes/insight_prompts.rs` | Add `changes_summary_prompt()` and `scenario_challenge_prompt()` |
| `crates/app-core/src/handlers/notes/insight.rs` | Emit `insight:changes-summary` event on cache miss; add `note_insight_generate_scenario` handler |
| `crates/desktop-shared/src/commands/notes.rs` | Add `ScenarioChallengeResponse` DTO |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_generate_scenario` Tauri command + DEV_COMMANDS + dispatch |
| `crates/desktop/src/main.rs` | Register `note_insight_generate_scenario` |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add `changesSummary` to state; listen for `insight:changes-summary` event |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Render `ChangesBanner` above tabs when present |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx` | Add "Generate Scenario" button + `ScenarioChallenge` rendering after 50%+ quiz |

---

## Chunk 1: "What's Changed" Banner

### Task 1: Add changes summary prompt

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs`

- [ ] **Step 1: Add the prompt function**

After the existing `format_persona_blocks` function, add:

```rust
/// Generate a brief "What's Changed" summary comparing previous insight with current context.
pub fn changes_summary_prompt(previous_synthesis: &str, current_context: &str) -> String {
    format!(
        r#"You are comparing a previous insight analysis with updated knowledge context to identify what's new or different.

Previous synthesis:
{previous_synthesis}

Current knowledge context (may have new notes, updated content, or addressed gaps):
{current_context}

Respond with a single concise sentence (max 80 words) summarizing what has changed. Focus on:
- New related notes discovered
- Previous gaps that have been addressed
- Significant content changes
- New connections not seen before

If nothing meaningful has changed, respond with exactly: "No significant changes since last review."

Do NOT repeat the synthesis. Just describe what's different."#
    )
}
```

- [ ] **Step 2: Build**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_prompts.rs
git commit -m "feat(app-core): add changes_summary_prompt for cross-session diff"
```

---

### Task 2: Emit `insight:changes-summary` on cache miss

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

When `note_insight_review` detects a cache miss AND a previous version exists, fire a background LLM call to generate the changes summary. This runs concurrently with the main insight pipeline (doesn't block it).

- [ ] **Step 1: Find the cache miss path in `note_insight_review`**

In `insight.rs`, after the cache check fails and before the `tokio::spawn(run_insight_pipeline(...))`, check for a previous version.

- [ ] **Step 2: Add the changes summary generation**

After the line `let emitter = emitter_override.unwrap_or_else(|| Arc::clone(&self.event_emitter));` (~line 174) and before `let insight_service = self.insight_service.clone();` (~line 175), add:

```rust
// Fire "What's Changed" summary if a previous version exists
if let Some(ref service) = self.insight_service {
    if let Ok(Some(prev)) = service.get_latest(note_id).await {
        let prev_content: feature_insights::InsightContent =
            serde_json::from_str(&prev.content).unwrap_or_default();
        if let Some(prev_synthesis) = prev_content.synthesis {
            let changes_emitter = emitter.clone();
            let changes_context = context_text.clone();
            let changes_provider = provider.clone();
            let changes_config = self.config.read().await;
            let changes_params = providers::cognitive_chat_params(&changes_config, 512);
            drop(changes_config);

            tokio::spawn(async move {
                let prompt =
                    insight_prompts::changes_summary_prompt(&prev_synthesis, &changes_context);
                let messages = vec![
                    providers::Message::System { content: prompt },
                    providers::Message::User {
                        content: providers::UserContent::Text(
                            "Generate the changes summary now.".to_string(),
                        ),
                    },
                ];
                if let Ok(response) = changes_provider.chat(&messages, None, &changes_params).await
                {
                    if let Some(summary) = response.content {
                        changes_emitter.emit_event(
                            "insight:changes-summary",
                            serde_json::json!({ "summary": summary }),
                        );
                    }
                }
            });
        }
    }
}
```

Note: This block must come AFTER context_text is built but BEFORE the main pipeline spawn (so we can clone `context_text`). The `emitter` variable must already be assigned (from the `emitter_override` line).

- [ ] **Step 3: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(app-core): emit insight:changes-summary on cache miss with previous version"
```

---

### Task 3: Add `changesSummary` to frontend state + event listener

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`

- [ ] **Step 1: Add `changesSummary` to `InsightReviewState`**

```typescript
export interface InsightReviewState {
  // ... existing fields ...
  changesSummary: string | null;  // add this
}
```

Update `INITIAL_STATE` to include `changesSummary: null`.

- [ ] **Step 2: Add event listener**

After the existing `useEvent("insight:perspectives-meta", ...)` listener, add:

```typescript
useEvent<{ summary: string }>("insight:changes-summary", ({ summary }) => {
  setState((prev) => ({ ...prev, changesSummary: summary }));
});
```

- [ ] **Step 3: Clear changesSummary on close and open**

In the `open` callback, when resetting state via `{ ...INITIAL_STATE, ... }`, `changesSummary` will be reset to `null` since it's part of `INITIAL_STATE`.

- [ ] **Step 4: Build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightReview.ts
git commit -m "feat(desktop-ui): add changesSummary state and event listener to useInsightReview"
```

---

### Task 4: Create `ChangesBanner` component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/ChangesBanner.tsx`

- [ ] **Step 1: Create the component**

```typescript
import { ArrowRight, X } from "lucide-react";
import { useState } from "react";

interface Props {
  summary: string;
}

export function ChangesBanner({ summary }: Props) {
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;

  // Don't show if nothing meaningful changed
  if (summary.includes("No significant changes")) return null;

  return (
    <div className="px-3 py-2 border-b border-border bg-brand/5 flex items-start gap-2">
      <ArrowRight size={12} className="text-brand shrink-0 mt-0.5" />
      <p className="text-[11px] text-muted-foreground leading-relaxed flex-1">
        {summary}
      </p>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="text-dim hover:text-muted-foreground transition-colors shrink-0"
      >
        <X size={10} />
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/ChangesBanner.tsx
git commit -m "feat(desktop-ui): add ChangesBanner component for cross-session diff display"
```

---

### Task 5: Wire `ChangesBanner` into `InsightReviewPanel`

**Files:**
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

- [ ] **Step 1: Import and render**

Add import:
```typescript
import { ChangesBanner } from "./insight/ChangesBanner";
```

Render between the scope hint bar and the tab bar:
```tsx
{/* What's Changed banner */}
{state.changesSummary && (
  <ChangesBanner summary={state.changesSummary} />
)}
```

- [ ] **Step 2: Build + test**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(desktop-ui): wire ChangesBanner into InsightReviewPanel"
```

---

## Chunk 2: Scenario Challenges

### Task 6: Add scenario challenge prompt

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs`

- [ ] **Step 1: Add the prompt function**

```rust
/// Generate an applied scenario challenge based on the knowledge context.
pub fn scenario_challenge_prompt(context: &str) -> String {
    format!(
        r#"You are a scenario designer for applied learning. Create a realistic scenario that tests the user's ability to apply concepts from their knowledge network.

Requirements:
- Concrete situation (not abstract)
- Requires applying concepts from 2+ notes
- 2-3 decision points where knowledge matters
- The best approach should NOT be immediately obvious

Respond ONLY with JSON (no markdown, no explanation):
{{
  "title": "Short scenario title (5-10 words)",
  "situation": "2-3 paragraph setup describing the scenario in detail",
  "questions": ["Decision question 1", "Decision question 2", "Decision question 3"],
  "modelAnswer": "3-4 paragraph ideal response explaining the reasoning and how concepts from the notes apply",
  "sourceNotes": ["note title 1", "note title 2"],
  "difficultyScore": 0.7
}}

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}
```

- [ ] **Step 2: Build**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_prompts.rs
git commit -m "feat(app-core): add scenario_challenge_prompt for applied learning scenarios"
```

---

### Task 7: Add `note_insight_generate_scenario` backend handler

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add DTO)
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (add handler)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri command + DEV_COMMANDS + dispatch)
- Modify: `crates/desktop/src/main.rs` (register command)

This is an on-demand endpoint — the frontend calls it when the user clicks "Generate Scenario" after completing 50%+ of the quiz.

- [ ] **Step 1: Add DTO in desktop-shared**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioChallengeResponse {
    pub title: String,
    pub situation: String,
    pub questions: Vec<String>,
    pub model_answer: String,
    pub source_notes: Vec<String>,
    pub difficulty_score: f64,
}
```

- [ ] **Step 2: Add handler in app-core**

In `insight.rs`, add after `note_insight_submit_quiz`:

```rust
pub async fn note_insight_generate_scenario(
    &self,
    note_id: &str,
) -> Result<ScenarioChallengeResponse, ApiError> {
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

    // Reuse the same context building as regenerate_tab
    let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
    let note_domains = insight_context::extract_note_domains(&tags);

    let ctx_text = if let Some(ref service) = self.insight_service {
        let scope: feature_insights::ScopeConfig = service
            .get_latest(note_id)
            .await
            .ok()
            .flatten()
            .and_then(|l| serde_json::from_str(&l.scope_config).ok())
            .unwrap_or_default();

        let scope_ids = service.resolve_scope(note_id, &scope).await;
        let mut related_notes = Vec::new();
        for id in &scope_ids {
            if let Ok(Some(n)) = self.note_repo.get_note(id).await {
                related_notes.push(n);
            }
        }

        match service
            .prepare_context(&note, &related_notes, &scope_ids, &scope, &note_domains)
            .await
        {
            Ok(prepared) => prepared.context.text,
            Err(_) => {
                let related = self.fetch_related_notes(note_id).await;
                insight_context::assemble_context(&note, &related, None).text
            }
        }
    } else {
        let related = self.fetch_related_notes(note_id).await;
        insight_context::assemble_context(&note, &related, None).text
    };

    let config = self.config.read().await;
    let params = providers::cognitive_chat_params(&config, 4096);
    drop(config);

    let prompt = insight_prompts::scenario_challenge_prompt(&ctx_text);
    let messages = vec![
        providers::Message::System { content: prompt },
        providers::Message::User {
            content: providers::UserContent::Text(
                "Generate the scenario challenge now.".to_string(),
            ),
        },
    ];

    let response = provider
        .chat(&messages, None, &params)
        .await
        .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

    let content = response.content.unwrap_or_default();

    serde_json::from_str::<ScenarioChallengeResponse>(&content).map_err(|e| {
        ApiError::new(
            "PARSE_ERROR",
            format!("Failed to parse scenario response: {e}"),
        )
    })
}
```

- [ ] **Step 3: Add Tauri command + DEV_COMMANDS + dispatch + main.rs**

Tauri command:
```rust
#[tauri::command]
pub async fn note_insight_generate_scenario(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<ScenarioChallengeResponse, ApiError> {
    state.note_insight_generate_scenario(&note_id).await
}
```

Add `ScenarioChallengeResponse` to the import list from `desktop_shared::commands` at the top of the file.

Add `"note_insight_generate_scenario"` to `DEV_COMMANDS`.

Add dispatch_dev arm:
```rust
"note_insight_generate_scenario" => {
    let id = try_field!(dev::get_str(body, "noteId"));
    dev::val(core.note_insight_generate_scenario(&id).await)
}
```

Register in `main.rs`:
```rust
commands::notes::note_insight_generate_scenario,
```

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/ crates/app-core/ crates/desktop/
git commit -m "feat(desktop): add note_insight_generate_scenario endpoint"
```

---

### Task 8: Create `ScenarioChallenge` component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/ScenarioChallenge.tsx`

- [ ] **Step 1: Create the component**

```typescript
import { BookOpen, ChevronDown } from "lucide-react";
import { useState } from "react";

export interface ScenarioData {
  title: string;
  situation: string;
  questions: string[];
  modelAnswer: string;
  sourceNotes: string[];
  difficultyScore: number;
}

interface Props {
  scenario: ScenarioData;
}

export function ScenarioChallenge({ scenario }: Props) {
  const [showAnswer, setShowAnswer] = useState(false);

  return (
    <div className="rounded-lg bg-card border border-border-subtle p-4 space-y-3">
      <div className="flex items-center gap-2">
        <BookOpen size={14} className="text-brand" />
        <span className="text-[13px] font-medium text-foreground">
          {scenario.title}
        </span>
        <span className="text-[9px] px-1.5 py-0.5 rounded bg-brand/10 text-brand ml-auto">
          Scenario
        </span>
      </div>

      <p className="text-[12px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
        {scenario.situation}
      </p>

      <div className="space-y-2">
        <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Decision Points
        </span>
        {scenario.questions.map((q, i) => (
          <div
            key={q}
            className="flex items-start gap-2 text-[11px] text-foreground"
          >
            <span className="text-brand font-medium shrink-0">
              {i + 1}.
            </span>
            <span>{q}</span>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={() => setShowAnswer((p) => !p)}
        className="flex items-center gap-1 text-[10px] text-brand hover:text-brand/80 transition-colors"
      >
        <ChevronDown
          size={10}
          className={`transition-transform ${showAnswer ? "rotate-180" : ""}`}
        />
        {showAnswer ? "Hide" : "Show"} Model Answer
      </button>

      {showAnswer && (
        <div className="p-3 rounded-md bg-accent border border-border-subtle">
          <p className="text-[11px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
            {scenario.modelAnswer}
          </p>
          {scenario.sourceNotes.length > 0 && (
            <div className="mt-2 flex items-center gap-1 flex-wrap">
              <span className="text-[9px] text-dim">Sources:</span>
              {scenario.sourceNotes.map((note) => (
                <span
                  key={note}
                  className="text-[9px] px-1.5 py-0.5 rounded bg-card text-muted-foreground border border-border-subtle"
                >
                  {note}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/ScenarioChallenge.tsx
git commit -m "feat(desktop-ui): add ScenarioChallenge component for applied learning scenarios"
```

---

### Task 9: Wire scenario into `SelfAssessmentTab`

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx`

The scenario "Generate Scenario" button appears after the user has answered 50%+ of quiz questions. Clicking it fires an IPC call and renders the `ScenarioChallenge` component.

- [ ] **Step 1: Read current SelfAssessmentTab**

Understand the current structure — it takes `status`, `questions`, `quizState`, `onAnswer`, `onReveal`, `onRevealAll`, `onSaveFlashcards` as props.

- [ ] **Step 2: Add scenario state + generation logic**

Add imports to the component (merge with existing imports as needed):

```typescript
import { useState, useCallback } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { BookOpen } from "lucide-react";
import { ScenarioChallenge, type ScenarioData } from "./ScenarioChallenge";

// Inside the component:
const [scenario, setScenario] = useState<ScenarioData | null>(null);
const [scenarioLoading, setScenarioLoading] = useState(false);

const answeredCount = Object.keys(quizState.answers).length;
const showScenarioButton = answeredCount >= questions.length * 0.5 && questions.length > 0;

const handleGenerateScenario = useCallback(async () => {
  if (!noteId) return;
  setScenarioLoading(true);
  try {
    const data = await ipc<ScenarioData>("note_insight_generate_scenario", { noteId });
    setScenario(data);
  } catch {
    // Silently fail
  } finally {
    setScenarioLoading(false);
  }
}, [noteId]);
```

Note: `SelfAssessmentTab` needs `noteId` as a new prop. Add it to the Props interface.

- [ ] **Step 3: Add the "Generate Scenario" button and scenario rendering**

After the quiz questions list and before the footer/save area, add:

```tsx
{/* Scenario Challenge */}
{showScenarioButton && !scenario && (
  <button
    type="button"
    onClick={handleGenerateScenario}
    disabled={scenarioLoading}
    className="flex items-center gap-1.5 text-[11px] text-brand hover:text-brand/80 transition-colors disabled:text-dim"
  >
    {scenarioLoading ? (
      <>
        <span className="w-3 h-3 border border-brand/40 border-t-brand rounded-full animate-spin" />
        Generating scenario...
      </>
    ) : (
      <>
        <BookOpen size={12} />
        Generate Applied Scenario
      </>
    )}
  </button>
)}

{scenario && <ScenarioChallenge scenario={scenario} />}
```

- [ ] **Step 4: Pass `noteId` from InsightReviewPanel**

In `InsightReviewPanel.tsx`, update the `SelfAssessmentTab` rendering to pass `noteId`:

```tsx
<SelfAssessmentTab
  status={state.tabs.assessment.status}
  questions={state.tabs.assessment.questions}
  quizState={state.quizState}
  noteId={state.noteId}
  onAnswer={actions.answerQuestion}
  onReveal={actions.revealAnswer}
  onRevealAll={actions.revealAll}
  onSaveFlashcards={actions.saveFlashcards}
/>
```

- [ ] **Step 5: Lint + build**

Run: `cd desktop-ui && bun run lint:fix`
Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(desktop-ui): add scenario challenge generation to SelfAssessmentTab"
```

---

## Chunk 3: Verification

### Task 10: Full verification

- [ ] **Step 1: Backend tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`
Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Frontend build + test**

Run: `cd desktop-ui && bun run build`
Run: `cd desktop-ui && bun run test`

- [ ] **Step 5: Manual smoke test**

Start: `cargo tauri dev`

1. Open a note → click Insight Review → wait for generation to complete
2. Close panel → edit the note (add new content) → reopen Insight Review
3. Verify "What's Changed" banner appears above the tabs with a meaningful diff summary
4. Verify the banner has a dismiss (X) button that hides it
5. Switch to Self-Assessment tab → answer 50%+ of questions
6. Verify "Generate Applied Scenario" button appears
7. Click it → verify scenario loads with title, situation, decision points
8. Click "Show Model Answer" → verify answer and source notes display
9. Test on a note with no previous insight → verify no banner appears (fresh generation)
10. Test on a note where content hasn't changed → verify cached path (no banner needed)
