# Phase 6: Polish & Cross-Session Tracking Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken "What's Changed" banner, add persona follow-up chat, add knowledge growth metrics, polish scenario challenge error handling, and wire entity merge/dedup into the desktop app.

**Architecture:** Five independent features. (1) Changes banner: new backend IPC `note_insight_changes_summary` replaces the broken SSE event path. (2) Persona chat: new IPC endpoint + inline chat component inside `PersonaCard`. (3) Knowledge growth: new IPC `cognitive_change_summary` exposing `TemporalService::change_summary` + frontend display in insight panel. (4) Scenario polish: fence-stripping + error feedback. (5) Entity dedup: new Tauri commands + entity browser/merge UI.

**Tech Stack:** Rust (`app-core`, `desktop-shared`, `desktop`, `cognitive`), TypeScript/React (`desktop-ui`), existing SQLite tools.

**Spec reference:** `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` §10 Phase 6.

**Critical bug to fix first:** After Phase 5's on-demand change, `open()` no longer calls `note_insight_review`, so the "What's Changed" banner (which fires via `tokio::spawn` inside that function) never appears. Task 1 fixes this.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx` | Inline chat component inside PersonaCard — input field + message history with single persona identity. |
| `crates/app-core/src/handlers/notes/persona_chat.rs` | `AppCore::note_insight_persona_chat` handler — builds LLM prompt with persona identity + note context + chat history. |
| `crates/app-core/src/handlers/entities.rs` | `AppCore` entity handlers: `entity_list`, `entity_search`, `entity_merge`, `entity_get_neighborhood`. |
| `crates/desktop/src/commands/entities.rs` | Thin Tauri command adapters delegating to `AppCore` entity handlers. |
| `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx` | Renders knowledge growth stats (new facts, updated facts, by-domain breakdown) inside insight panel. |

### Modified files

| File | Change |
|------|--------|
| `crates/app-core/src/handlers/notes/insight.rs` | Add `note_insight_changes_summary(note_id)` — extracts the changes-summary logic from `note_insight_review` into a standalone IPC endpoint. Also add `strip_markdown_fences` to `note_insight_generate_scenario` response. |
| `crates/app-core/src/handlers/notes/mod.rs` | Add `pub mod persona_chat;` |
| `crates/app-core/src/handlers/mod.rs` | Add `pub mod entities;` |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | In `open()`: after cache loads, call `note_insight_changes_summary` if a cached insight exists with version > 1. |
| `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx` | Pass `noteId` + `onChat` callback to `PersonaCard`. |
| `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx` | Add expandable chat section with "Ask this persona" button. |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx:L61` | Replace empty `catch` with error state feedback. |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Add `KnowledgeGrowthMetrics` section below the changes banner. |
| `crates/desktop-shared/src/commands/notes.rs` | Add `ChangesSummaryResponse`, `PersonaChatRequest`/`Response` types. |
| `crates/desktop-shared/src/commands/mod.rs` | Add entity command types (or create `entities.rs`). |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_changes_summary` and `note_insight_persona_chat` Tauri commands. |
| `crates/desktop/src/commands/mod.rs` | Add `pub mod entities;` and register entity commands. |

---

## Task 1: Fix "What's Changed" Banner (Critical Bug)

The banner is broken after Phase 5's on-demand change. `open()` no longer calls `note_insight_review`, so the changes-summary `tokio::spawn` never fires. Fix: extract into a standalone IPC endpoint.

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`

- [ ] **Step 1: Add `ChangesSummaryResponse` type to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add after `ScenarioChallengeResponse`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesSummaryResponse {
    pub summary: String,
}
```

- [ ] **Step 2: Extract changes-summary logic into standalone `AppCore` method**

In `crates/app-core/src/handlers/notes/insight.rs`, add a new method to `impl AppCore`:

```rust
    /// Generate a "What's Changed" summary comparing current note context to the last insight.
    /// Called by the frontend after loading cached insight on open.
    pub async fn note_insight_changes_summary(
        &self,
        note_id: &str,
    ) -> Result<Option<ChangesSummaryResponse>, ApiError> {
        let service = match &self.insight_service {
            Some(s) => s,
            None => return Ok(None),
        };

        let latest = match service.get_latest(note_id).await.ok().flatten() {
            Some(r) => r,
            None => return Ok(None),
        };

        // Need at least version 2 for a meaningful diff
        if latest.version < 2 {
            return Ok(None);
        }

        let prev_content: feature_insights::InsightContent =
            serde_json::from_str(&latest.content).unwrap_or_default();
        let prev_synthesis = match prev_content.synthesis {
            Some(s) => s,
            None => return Ok(None),
        };

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = super::insight_context::assemble_context(&note, &related_notes, None);

        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 512);
        drop(config);

        let prompt = super::insight_prompts::changes_summary_prompt(&prev_synthesis, &ctx.text);
        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text(
                    "Generate the changes summary now.".to_string(),
                ),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        match response.content {
            Some(summary) if !summary.contains("No significant changes") => {
                Ok(Some(ChangesSummaryResponse { summary }))
            }
            _ => Ok(None),
        }
    }
```

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/notes.rs`, add:

```rust
#[tauri::command]
pub async fn note_insight_changes_summary(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Option<ChangesSummaryResponse>, ApiError> {
    state.note_insight_changes_summary(&note_id).await
}
```

Register in the Tauri command list (check `tauri.conf.json` or `main.rs` for the `invoke_handler!` macro) and add to `DEV_COMMANDS` in the notes command module.

- [ ] **Step 4: Wire frontend — call after cache load**

In `desktop-ui/src/features/notes/hooks/useInsightReview.ts`, modify the `open()` function. After the `applyCachedContent` call, fire the changes summary:

```typescript
    async (noteId: string) => {
      setState({ ...INITIAL_STATE, isOpen: true, noteId });

      try {
        const cached = await ipc<InsightReviewCachedResponse | null>("note_insight_cache_get", {
          noteId,
        });
        if (cached) {
          applyCachedContent(cached, { insightReviewId: cached.insightReviewId });

          // Fire "What's Changed" summary if we have a cached insight
          ipc<{ summary: string } | null>("note_insight_changes_summary", { noteId })
            .then((result) => {
              if (result?.summary) {
                setState((prev) => ({ ...prev, changesSummary: result.summary }));
              }
            })
            .catch(() => {}); // Best-effort — don't block panel open
        }
      } catch {
        // No cache — all tabs stay "idle"
      }
    },
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p app-core && cargo check -p desktop`

- [ ] **Step 6: Commit**

```bash
git commit -m "fix(insights): restore What's Changed banner via standalone IPC endpoint"
```

---

## Task 2: Scenario Challenge Polish

Fix the silent error swallow and add markdown fence stripping to the backend.

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (`note_insight_generate_scenario`)
- Modify: `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx`

- [ ] **Step 1: Add fence stripping to scenario response parsing**

In `crates/app-core/src/handlers/notes/insight.rs`, in `note_insight_generate_scenario` (around the `serde_json::from_str` call), add `strip_markdown_fences` before parsing:

```rust
        let content = response.content.unwrap_or_default();
        let cleaned = strip_markdown_fences(&content);

        serde_json::from_str::<ScenarioChallengeResponse>(&cleaned).map_err(|e| {
            ApiError::new(
                "PARSE_ERROR",
                format!("Failed to parse scenario response: {e}"),
            )
        })
```

- [ ] **Step 2: Add error feedback in SelfAssessmentTab**

In `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx`, replace the empty `catch` block at line 61:

```typescript
    } catch {
      setScenario(null);
      setScenarioError(true);
    } finally {
```

Add the `scenarioError` state at the top of the component:

```typescript
  const [scenarioError, setScenarioError] = useState(false);
```

And render the error near the scenario button:

```tsx
{scenarioError && !scenario && (
  <p className="text-[10px] text-destructive mt-1">
    Failed to generate scenario. Try again.
  </p>
)}
```

Reset `scenarioError` to `false` at the start of `handleGenerateScenario`.

- [ ] **Step 3: Verify**

Run: `cargo check -p app-core`
Run: `cd desktop-ui && npx biome check src/features/notes/components/insight/SelfAssessmentTab.tsx`

- [ ] **Step 4: Commit**

```bash
git commit -m "fix(insights): add fence stripping to scenario response and show error feedback"
```

---

## Task 3: Persona Follow-up Chat

Add an inline chat capability to PersonaCard so users can ask follow-up questions to individual personas.

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx`
- Create: `crates/app-core/src/handlers/notes/persona_chat.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx`

- [ ] **Step 1: Add IPC types to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaChatParams {
    pub note_id: String,
    pub persona_id: String,
    pub persona_name: String,
    pub persona_role: String,
    pub persona_tone: String,
    pub user_message: String,
    /// Previous messages for context (alternating user/assistant).
    pub history: Vec<PersonaChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaChatResponse {
    pub reply: String,
}
```

- [ ] **Step 2: Create backend handler**

Create `crates/app-core/src/handlers/notes/persona_chat.rs`:

```rust
use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

use super::insight_context;

impl AppCore {
    pub async fn note_insight_persona_chat(
        &self,
        params: &PersonaChatParams,
    ) -> Result<PersonaChatResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(&params.note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        let system_prompt = format!(
            "You are {name}, a {role}. Your communication style is {tone}.\n\
             You are analyzing a note and answering follow-up questions from the user's perspective.\n\
             Stay in character. Be concise (2-4 sentences per response).\n\n\
             --- NOTE CONTEXT ---\n{context}\n--- END CONTEXT ---",
            name = params.persona_name,
            role = params.persona_role,
            tone = params.persona_tone,
            context = ctx.text,
        );

        let mut messages = vec![providers::Message::System {
            content: system_prompt,
        }];

        // Add chat history
        for msg in &params.history {
            match msg.role.as_str() {
                "user" => messages.push(providers::Message::User {
                    content: providers::UserContent::Text(msg.content.clone()),
                }),
                "assistant" => messages.push(providers::Message::Assistant {
                    content: msg.content.clone(),
                    tool_calls: None,
                }),
                _ => {}
            }
        }

        // Add current message
        messages.push(providers::Message::User {
            content: providers::UserContent::Text(params.user_message.clone()),
        });

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 512);
        drop(config);

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        Ok(PersonaChatResponse {
            reply: response.content.unwrap_or_default(),
        })
    }
}
```

- [ ] **Step 3: Register module and Tauri command**

In `crates/app-core/src/handlers/notes/mod.rs`, add `pub mod persona_chat;`.

In `crates/desktop/src/commands/notes.rs`, add the Tauri command:

```rust
#[tauri::command]
pub async fn note_insight_persona_chat(
    state: State<'_, Arc<AppCore>>,
    params: PersonaChatParams,
) -> Result<PersonaChatResponse, ApiError> {
    state.note_insight_persona_chat(&params).await
}
```

Add to the Tauri invoke handler registration and `DEV_COMMANDS`.

- [ ] **Step 4: Create PersonaChat frontend component**

Create `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx`:

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { Send } from "lucide-react";
import { useCallback, useState } from "react";

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

interface PersonaChatProps {
  noteId: string;
  personaId: string;
  personaName: string;
  personaRole: string;
  personaTone: string;
}

export function PersonaChat({
  noteId,
  personaId,
  personaName,
  personaRole,
  personaTone,
}: PersonaChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);

  const send = useCallback(async () => {
    const text = input.trim();
    if (!text || loading) return;

    const userMsg: ChatMessage = { role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const result = await ipc<{ reply: string }>("note_insight_persona_chat", {
        noteId,
        personaId,
        personaName,
        personaRole,
        personaTone,
        userMessage: text,
        history: messages,
      });
      setMessages((prev) => [...prev, { role: "assistant", content: result.reply }]);
    } catch {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "Failed to get response. Try again." },
      ]);
    } finally {
      setLoading(false);
    }
  }, [input, loading, messages, noteId, personaId, personaName, personaRole, personaTone]);

  return (
    <div className="mt-2 space-y-2 border-t border-border pt-2">
      {messages.map((msg, i) => (
        <div
          key={`${msg.role}-${i}`}
          className={`text-[11px] leading-relaxed ${
            msg.role === "user" ? "text-foreground" : "text-muted-foreground italic"
          }`}
        >
          <span className="text-[9px] text-dim mr-1">
            {msg.role === "user" ? "You:" : `${personaName}:`}
          </span>
          {msg.content}
        </div>
      ))}
      {loading && (
        <div className="text-[10px] text-dim italic animate-pulse">
          {personaName} is thinking...
        </div>
      )}
      <div className="flex gap-1">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && send()}
          placeholder={`Ask ${personaName}...`}
          className="flex-1 text-[11px] px-2 py-1 rounded-md bg-input border border-border text-foreground placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-purple/30"
          disabled={loading}
        />
        <button
          type="button"
          onClick={send}
          disabled={loading || !input.trim()}
          className="p-1 rounded-md text-purple hover:bg-purple/10 transition-colors disabled:text-dim"
        >
          <Send size={12} />
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Modify PersonaCard to accept chat props**

In `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx`, add chat toggle:

```tsx
import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { MessageCircle } from "lucide-react";
import { useState } from "react";
import { PersonaChat } from "./PersonaChat";

interface PersonaCardProps {
  name: string;
  role: string;
  icon: string;
  tone: string;
  content: string;
  noteId?: string;
  personaId?: string;
}
```

Add state and chat section inside the component:

```tsx
  const [showChat, setShowChat] = useState(false);

  // After the MarkdownContent div:
  {noteId && personaId && (
    <>
      <button
        type="button"
        onClick={() => setShowChat((p) => !p)}
        className="flex items-center gap-1 text-[10px] text-purple hover:text-purple/80 transition-colors"
      >
        <MessageCircle size={10} />
        {showChat ? "Hide chat" : "Ask this persona"}
      </button>
      {showChat && (
        <PersonaChat
          noteId={noteId}
          personaId={personaId}
          personaName={name}
          personaRole={role}
          personaTone={tone}
        />
      )}
    </>
  )}
```

- [ ] **Step 6: Pass noteId and personaId from PerspectivesTab**

In `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx`, add `noteId` prop and pass through to `PersonaCard`:

```tsx
interface PerspectivesTabProps {
  status: TabStatus;
  content: string;
  personas: PersonaMeta[];
  noteId?: string | null;
}
```

And in the card rendering:

```tsx
<PersonaCard
  key={persona.id}
  name={persona.name}
  role={persona.role}
  icon={persona.icon}
  tone={persona.tone}
  content={section}
  noteId={noteId ?? undefined}
  personaId={persona.id}
/>
```

Update `InsightReviewPanel.tsx` to pass `noteId` to `PerspectivesTab`.

- [ ] **Step 7: Verify**

Run: `cargo check -p app-core && cargo check -p desktop`
Run: `cd desktop-ui && npx biome check src/features/notes/components/insight/PersonaChat.tsx src/features/notes/components/insight/PersonaCard.tsx src/features/notes/components/insight/PerspectivesTab.tsx`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(insights): add persona follow-up chat in Perspectives tab"
```

---

## Task 4: Knowledge Growth Metrics

Expose `TemporalService::change_summary` to the frontend and display metrics in the insight panel.

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx`
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add response types)
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (add handler)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri command)
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

- [ ] **Step 1: Add response types to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGrowthResponse {
    pub new_facts_count: usize,
    pub updated_facts_count: usize,
    pub superseded_facts_count: usize,
    pub by_domain: Vec<DomainCount>,
    pub period_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCount {
    pub domain: String,
    pub count: usize,
}
```

- [ ] **Step 2: Add AppCore handler**

In `crates/app-core/src/handlers/notes/insight.rs`, add:

```rust
    pub async fn note_insight_knowledge_growth(
        &self,
        days: u32,
    ) -> Result<KnowledgeGrowthResponse, ApiError> {
        let temporal = self
            .temporal_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Temporal service not available"))?;

        let since = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let summary = temporal
            .change_summary(since, None)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let by_domain = summary
            .by_domain
            .into_iter()
            .map(|(domain, count)| DomainCount { domain, count })
            .collect();

        Ok(KnowledgeGrowthResponse {
            new_facts_count: summary.new_facts.len(),
            updated_facts_count: summary.updated_facts.len(),
            superseded_facts_count: summary.superseded_facts.len(),
            by_domain,
            period_days: days,
        })
    }
```

**Note:** Check if `self.temporal_service` exists on `AppCore`. If not, it may be accessed differently — search for `TemporalService` usage in `app-core` to find the field name. If it's not wired yet, you'll need to add it to `AppCore` and initialize it in the builder from `StoragePool`.

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/notes.rs`:

```rust
#[tauri::command]
pub async fn note_insight_knowledge_growth(
    state: State<'_, Arc<AppCore>>,
    days: Option<u32>,
) -> Result<KnowledgeGrowthResponse, ApiError> {
    state.note_insight_knowledge_growth(days.unwrap_or(7)).await
}
```

Add to invoke handler and `DEV_COMMANDS`.

- [ ] **Step 4: Create KnowledgeGrowthMetrics component**

Create `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx`:

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { Brain, TrendingUp } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

interface KnowledgeGrowth {
  newFactsCount: number;
  updatedFactsCount: number;
  supersededFactsCount: number;
  byDomain: { domain: string; count: number }[];
  periodDays: number;
}

interface Props {
  isOpen: boolean;
}

export function KnowledgeGrowthMetrics({ isOpen }: Props) {
  const [data, setData] = useState<KnowledgeGrowth | null>(null);

  const fetch = useCallback(async () => {
    try {
      const result = await ipc<KnowledgeGrowth>("note_insight_knowledge_growth", { days: 7 });
      if (result.newFactsCount > 0 || result.updatedFactsCount > 0) {
        setData(result);
      }
    } catch {
      // Silently fail — metric is supplementary
    }
  }, []);

  useEffect(() => {
    if (isOpen) fetch();
  }, [isOpen, fetch]);

  if (!data) return null;

  return (
    <div className="px-3 py-1.5 border-b border-border flex items-center gap-2 text-[10px] text-dim">
      <TrendingUp size={10} className="text-success shrink-0" />
      <span>
        <span className="text-muted-foreground font-medium">{data.newFactsCount}</span> new facts
      </span>
      {data.updatedFactsCount > 0 && (
        <span>
          · <span className="text-muted-foreground font-medium">{data.updatedFactsCount}</span>{" "}
          updated
        </span>
      )}
      {data.byDomain.length > 0 && (
        <span className="ml-auto text-[9px]">
          {data.byDomain
            .slice(0, 3)
            .map((d) => d.domain)
            .join(", ")}
        </span>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Wire into InsightReviewPanel**

In `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`, import and render after the ChangesBanner:

```tsx
import { KnowledgeGrowthMetrics } from "./insight/KnowledgeGrowthMetrics";

// After {state.changesSummary && <ChangesBanner ... />}:
<KnowledgeGrowthMetrics isOpen={state.isOpen} />
```

- [ ] **Step 6: Verify**

Run: `cargo check -p app-core && cargo check -p desktop`
Run: `cd desktop-ui && npx biome check src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx`

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(insights): add knowledge growth metrics to insight panel"
```

---

## Task 5: Entity Merge/Dedup Tools

Wire `EntityRepo` operations to IPC commands and create a basic entity browser/merge UI.

**Files:**
- Create: `crates/app-core/src/handlers/entities.rs`
- Create: `crates/desktop/src/commands/entities.rs`
- Create: `crates/desktop-shared/src/commands/entities.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

- [ ] **Step 1: Add entity command types to desktop-shared**

Create `crates/desktop-shared/src/commands/entities.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityResponse {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub mention_count: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitySearchParams {
    pub query: String,
    pub entity_type: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityMergeParams {
    /// The entity to keep (target).
    pub keep_id: String,
    /// The entity to merge into the target (will be deleted).
    pub merge_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityNeighborhoodResponse {
    pub center: EntityResponse,
    pub neighbors: Vec<EntityRelationshipResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRelationshipResponse {
    pub entity: EntityResponse,
    pub relationship_type: String,
    pub strength: f64,
    pub direction: String, // "outgoing" or "incoming"
}
```

In `crates/desktop-shared/src/commands/mod.rs`, add `pub mod entities;`.

- [ ] **Step 2: Create AppCore entity handlers**

Create `crates/app-core/src/handlers/entities.rs`:

```rust
use desktop_shared::commands::entities::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    pub async fn entity_search(
        &self,
        params: &EntitySearchParams,
    ) -> Result<Vec<EntityResponse>, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let rows = repo
            .find_by_name(&params.query)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let limit = params.limit.unwrap_or(20) as usize;
        Ok(rows
            .into_iter()
            .filter(|r| {
                params
                    .entity_type
                    .as_ref()
                    .map_or(true, |t| r.entity_type == *t)
            })
            .take(limit)
            .map(entity_row_to_response)
            .collect())
    }

    pub async fn entity_merge(
        &self,
        params: &EntityMergeParams,
    ) -> Result<EntityResponse, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let merged = repo
            .merge_entities(&params.merge_id, &params.keep_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(entity_row_to_response(merged))
    }

    pub async fn entity_get_neighborhood(
        &self,
        entity_id: &str,
        depth: u32,
    ) -> Result<EntityNeighborhoodResponse, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let neighborhood = repo
            .get_neighborhood(entity_id, depth)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let center = repo
            .get(entity_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Entity not found"))?;

        Ok(EntityNeighborhoodResponse {
            center: entity_row_to_response(center),
            neighbors: neighborhood
                .edges
                .into_iter()
                .map(|edge| EntityRelationshipResponse {
                    entity: entity_row_to_response(edge.target),
                    relationship_type: edge.relationship_type,
                    strength: edge.strength,
                    direction: if edge.outgoing {
                        "outgoing".to_string()
                    } else {
                        "incoming".to_string()
                    },
                })
                .collect(),
        })
    }
}

fn entity_row_to_response(row: cognitive::repos::EntityRow) -> EntityResponse {
    EntityResponse {
        id: row.id,
        name: row.name,
        entity_type: row.entity_type,
        description: row.description,
        mention_count: row.mention_count,
        last_seen_at: row.last_seen_at,
    }
}
```

**Important:** The exact field names on `EntityRow`, `GraphNeighborhood`, and edge types may differ. Read `crates/cognitive/src/repos/entity.rs` to confirm the actual struct fields and adapt accordingly. The `merge_entities` signature may be `(source_id, target_id)` — check which is "keep" vs "delete".

In `crates/app-core/src/handlers/mod.rs`, add `pub mod entities;`.

- [ ] **Step 3: Create Tauri commands**

Create `crates/desktop/src/commands/entities.rs`:

```rust
use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::commands::entities::*;
use desktop_shared::errors::ApiError;
use tauri::State;

#[tauri::command]
pub async fn entity_search(
    state: State<'_, Arc<AppCore>>,
    params: EntitySearchParams,
) -> Result<Vec<EntityResponse>, ApiError> {
    state.entity_search(&params).await
}

#[tauri::command]
pub async fn entity_merge(
    state: State<'_, Arc<AppCore>>,
    params: EntityMergeParams,
) -> Result<EntityResponse, ApiError> {
    state.entity_merge(&params).await
}

#[tauri::command]
pub async fn entity_get_neighborhood(
    state: State<'_, Arc<AppCore>>,
    entity_id: String,
    depth: Option<u32>,
) -> Result<EntityNeighborhoodResponse, ApiError> {
    state
        .entity_get_neighborhood(&entity_id, depth.unwrap_or(1))
        .await
}
```

In `crates/desktop/src/commands/mod.rs`, add `pub mod entities;` and register the commands.

Add `DEV_COMMANDS` to the entities module.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p desktop`

The entity handler code depends on exact `EntityRow` / `GraphNeighborhood` field names from `cognitive::repos::entity`. If compilation fails, read the actual types and adjust the conversion function.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(entities): wire EntityRepo operations to IPC commands"
```

---

## Task 6: Final Verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero new warnings.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: Clean.

- [ ] **Step 5: Frontend lint**

Run: `cd desktop-ui && npx biome check src/features/notes/components/insight/`
Expected: Only pre-existing issues (no new errors from our files).

---

## What's NOT in This Plan (Deferred)

- **Entity browser/merge UI component** — The IPC commands are wired (Task 5), but the full entity browser UI (search, visual graph, merge confirmation dialog) is substantial frontend work that deserves its own plan. The concept map tab (`ConceptMapTab`) already renders entities visually via Mermaid — the entity merge UI could extend that, or live as a separate settings panel.
- **Cross-note aggregate knowledge growth dashboard** — Task 4 surfaces per-session metrics, but a full cross-note trend dashboard (showing learning curves across all notes) is a separate feature.
- **Persona chat history persistence** — Task 3's chat is ephemeral (resets on panel close). Persisting chat history to SQLite is deferred.
