# Insight Tab Chat Sessions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform insight tabs (Synthesis, Gap Analysis, Self-Assessment, Concept Map) from static AI output into interactive chat sessions with streaming responses and persistent history, and upgrade Persona Chat with the same capabilities.

**Architecture:** New lightweight `note_insight_tab_chat` backend endpoint that does direct LLM streaming (no agent loop) with session persistence via the existing `sessions`/`session_messages` tables. Emits `agent:content_chunk` and `agent:done` events so the frontend `chatStreamStore` works unchanged. Frontend reuses `useAgentStream` for streaming and `useQuery("chat_messages")` for history — only the send path calls the new IPC instead of `chat_send`. Persona chat gets the same streaming + persistence upgrade.

**Tech Stack:** Rust (providers::chat_stream, storage::SessionRepo), TypeScript/React (chatStreamStore, useAgentStream), Tauri IPC, SSE (browser dev mode)

---

## File Structure

### Backend — New Files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/handlers/notes/insight_chat.rs` | Streaming relay helper + tab chat handler + upgraded persona chat handler |

### Backend — Modified Files

| File | Change |
|------|--------|
| `crates/app-core/src/handlers/notes/mod.rs` | Add `pub mod insight_chat;` |
| `crates/desktop-shared/src/commands/notes.rs` | Add `InsightChatParams`, `InsightChatStarted` types |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_tab_chat` Tauri command |
| `crates/desktop/src/dev_server/streaming.rs` | Add `dispatch_insight_tab_chat` for browser dev mode SSE |
| `crates/desktop/src/dev_server/dispatch.rs` | Route `note_insight_tab_chat` to streaming dispatch |
| `crates/app-core/src/handlers/notes/persona_chat.rs` | Keep old handler, add `note_insight_persona_chat_stream` |

### Frontend — New Files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/hooks/useInsightChat.ts` | Thin hook: session key, history fetch via `useQuery("chat_messages")`, streaming via `useAgentStream`, send via `ipc("note_insight_tab_chat")` |
| `desktop-ui/src/features/notes/components/insight/InsightChatInput.tsx` | Reusable chat input + message list UI component for insight tab chats |

### Frontend — Modified Files

| File | Change |
|------|--------|
| `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx` | Add `InsightChatInput` below content |
| `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx` | Add `InsightChatInput` below content |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx` | Add `InsightChatInput` below quiz |
| `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx` | Add `InsightChatInput` below diagram |
| `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx` | Rewrite with streaming + persistence |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Pass `noteId` + `insightReviewId` to tab components; clear sessions on regenerate |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add `clearTabChatSessions` action called on regeneration |

---

## Conventions Reference

- **IPC calls**: `useQuery(cmd, args)` for reads, `ipc(cmd, payload)` for writes. Args use `camelCase`. Tauri commands use `snake_case`.
- **Event names**: `agent:content_chunk`, `agent:done`, `agent:error` — already handled by `chatStreamStore` keyed by `sessionKey`.
- **Session key format**: Free-form string. We use `insight:{noteId}:{tabName}` for tabs, `insight:{noteId}:persona:{personaId}` for personas.
- **Streaming flow**: Backend emits events via `AppEventEmitter`. Tauri mode: `tauri::AppHandle::emit`. Browser mode: `broadcast::channel` → SSE endpoint.
- **Provider streaming**: `provider.chat_stream(&messages, None, &params) -> Result<LlmStream>` where `LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>>>>`. Each chunk has `content: Option<String>` and `is_final: bool`.

---

### Task 1: Backend — IPC Types and Streaming Relay

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs:292` (after NoteRetentionHealth section)
- Create: `crates/app-core/src/handlers/notes/insight_chat.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

- [ ] **Step 1: Add IPC types in desktop-shared**

Add after line 316 in `crates/desktop-shared/src/commands/notes.rs`:

```rust
// ── Insight Tab Chat ────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightChatParams {
    pub note_id: String,
    pub tab_name: String,
    pub user_message: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightChatStarted {
    pub session_key: String,
    pub message_id: String,
}
```

- [ ] **Step 2: Create the insight_chat module with streaming relay**

Create `crates/app-core/src/handlers/notes/insight_chat.rs`:

```rust
//! Insight tab chat — streaming LLM conversations grounded in insight analysis.
//!
//! Direct provider.chat_stream() (no agent loop). Persists messages via
//! the existing sessions/session_messages tables. Emits agent:content_chunk
//! and agent:done events so the frontend chatStreamStore works unchanged.

use std::sync::Arc;

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use desktop_shared::events;
use futures_util::StreamExt;

use crate::events::AppEventEmitter;
use crate::state::AppCore;

/// Relay a streaming LLM response to the frontend via AppEventEmitter.
///
/// Stores user + assistant messages in the session. Emits `agent:content_chunk`
/// per token and `agent:done` on completion, keyed by `session_key`.
async fn relay_insight_stream(
    repos: storage::Repos,
    provider: Arc<dyn providers::DynProvider>,
    messages: Vec<providers::Message>,
    params: providers::ChatParams,
    session_key: String,
    emitter: Arc<dyn AppEventEmitter>,
) {
    let stream_result = provider.chat_stream(&messages, None, &params).await;

    match stream_result {
        Ok(mut stream) => {
            let mut full_content = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(c) => {
                        if let Some(text) = &c.content {
                            full_content.push_str(text);
                            emitter.emit_event(
                                events::AGENT_CONTENT_CHUNK,
                                serde_json::json!({
                                    "sessionKey": session_key,
                                    "data": text,
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        emitter.emit_event(
                            events::AGENT_ERROR,
                            serde_json::json!({
                                "sessionKey": session_key,
                                "message": e.to_string(),
                            }),
                        );
                        return;
                    }
                }
            }

            // Store assistant message
            let msg_id = uuid::Uuid::new_v4();
            let _ = repos
                .sessions
                .add_message(
                    &session_key,
                    msg_id,
                    "assistant",
                    &full_content,
                    None,
                    None,
                    None,
                    None,
                )
                .await;

            // Emit done
            emitter.emit_event(
                events::AGENT_DONE,
                serde_json::json!({
                    "sessionKey": session_key,
                    "content": full_content,
                }),
            );
        }
        Err(e) => {
            emitter.emit_event(
                events::AGENT_ERROR,
                serde_json::json!({
                    "sessionKey": session_key,
                    "message": e.to_string(),
                }),
            );
        }
    }
}

/// Build the system prompt for a tab chat session.
fn tab_chat_system_prompt(note_title: &str, note_body: &str, tab_name: &str, tab_content: &str) -> String {
    let tab_label = match tab_name {
        "synthesis" => "Synthesis",
        "gaps" | "gap_analysis" => "Gap Analysis",
        "assessment" | "self_assessment" => "Self-Assessment",
        "concept-map" | "concept_map" => "Concept Map",
        _ => "Analysis",
    };

    let body_preview = common::truncate_at_boundary(note_body, 3000);

    format!(
        "You are an AI tutor helping a student deepen their understanding.\n\
         You previously generated a {tab_label} analysis of their note. \
         The student is now asking follow-up questions about it.\n\n\
         Stay focused on the analysis and the note content. Be concise (2-5 sentences). \
         Use examples from the note when relevant. If asked about something outside \
         the analysis scope, briefly redirect.\n\n\
         --- NOTE: {note_title} ---\n{body_preview}\n--- END NOTE ---\n\n\
         --- {tab_label} ANALYSIS ---\n{tab_content}\n--- END ANALYSIS ---"
    )
}

impl AppCore {
    /// Handle a tab chat message: stream a grounded response.
    ///
    /// Returns immediately after spawning the streaming relay. The frontend
    /// receives events via `agent:content_chunk` / `agent:done`.
    pub async fn note_insight_tab_chat(
        &self,
        params: &InsightChatParams,
        emitter: Arc<dyn AppEventEmitter>,
    ) -> Result<InsightChatStarted, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        // 1. Load note
        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        // 2. Load tab content from latest insight
        let tab_content = if let Some(insight_service) = &self.insight_service {
            if let Ok(Some(latest)) = insight_service.get_latest(&params.note_id).await {
                let content: feature_insights::types::InsightContent =
                    serde_json::from_str(&latest.content).unwrap_or_default();
                match params.tab_name.as_str() {
                    "synthesis" => content.synthesis.unwrap_or_default(),
                    "gaps" | "gap_analysis" => content.gap_analysis.unwrap_or_default(),
                    "assessment" | "self_assessment" => {
                        content.self_assessment.unwrap_or_default()
                    }
                    "concept-map" | "concept_map" => content.concept_map.unwrap_or_default(),
                    _ => String::new(),
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if tab_content.is_empty() {
            return Err(ApiError::new(
                "NO_CONTENT",
                "No insight content found for this tab. Generate an insight first.",
            ));
        }

        // 3. Ensure session exists
        let session_key = &params.session_key;
        let title = format!("Insight chat: {}", note.title);
        let metadata = serde_json::json!({ "title": title });
        self.repos
            .sessions
            .upsert_session(session_key, &metadata, None)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // 4. Store user message
        let user_msg_id = uuid::Uuid::new_v4();
        self.repos
            .sessions
            .add_message(
                session_key,
                user_msg_id,
                "user",
                &params.user_message,
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // 5. Load history from session (last 20 messages for context window)
        let history = self
            .repos
            .sessions
            .get_recent_messages(session_key, 20)
            .await
            .unwrap_or_default();

        // 6. Build LLM messages
        let system_prompt =
            tab_chat_system_prompt(&note.title, &note.body, &params.tab_name, &tab_content);

        let mut messages = vec![providers::Message::system(system_prompt)];
        for msg in &history {
            match msg.role.as_str() {
                "user" => messages.push(providers::Message::user(msg.content.clone())),
                "assistant" => messages.push(providers::Message::Assistant {
                    content: Some(msg.content.clone()),
                    tool_calls: None,
                    reasoning_content: None,
                }),
                _ => {}
            }
        }

        // 7. Spawn streaming relay
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let repos = self.repos.clone();
        let provider = Arc::clone(provider);
        let sk = session_key.clone();

        tokio::spawn(relay_insight_stream(
            repos,
            provider,
            messages,
            chat_params,
            sk,
            emitter,
        ));

        Ok(InsightChatStarted {
            session_key: session_key.clone(),
            message_id: user_msg_id.to_string(),
        })
    }

    /// Clear all chat sessions for a note's insight tabs.
    /// Called when insight is regenerated to start fresh conversations.
    pub async fn note_insight_clear_tab_chats(
        &self,
        note_id: &str,
    ) -> Result<(), ApiError> {
        let tab_names = ["synthesis", "gaps", "assessment", "concept-map"];
        for tab in &tab_names {
            let session_key = format!("insight:{note_id}:{tab}");
            let _ = self.repos.sessions.delete_session(&session_key).await;
        }
        Ok(())
    }
}
```

- [ ] **Step 3: Register the module**

Add to `crates/app-core/src/handlers/notes/mod.rs`:

```rust
pub mod insight_chat;
```

- [ ] **Step 4: Run clippy to verify**

Run: `cargo clippy -p app-core --all-targets`
Expected: 0 warnings (may show unused import warnings until Tauri commands are wired)

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs crates/app-core/src/handlers/notes/insight_chat.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(insights): add insight tab chat backend with streaming relay"
```

---

### Task 2: Backend — Tauri Command + Dev Server Wiring

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/dev_server/streaming.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`

- [ ] **Step 1: Add Tauri command**

Add to `crates/desktop/src/commands/notes.rs` (alongside the existing insight commands):

```rust
#[tauri::command]
pub async fn note_insight_tab_chat(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppCore>>,
    params: desktop_shared::commands::InsightChatParams,
) -> Result<desktop_shared::commands::InsightChatStarted, ApiError> {
    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> =
        Arc::new(super::TauriEmitter(app));
    state.note_insight_tab_chat(&params, emitter).await
}

#[tauri::command]
pub async fn note_insight_clear_tab_chats(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<(), ApiError> {
    state.note_insight_clear_tab_chats(&note_id).await
}
```

Also add `note_insight_tab_chat` and `note_insight_clear_tab_chats` to `DEV_COMMANDS` in the same file.

- [ ] **Step 2: Add dev server streaming dispatch**

Add to `crates/desktop/src/dev_server/streaming.rs`:

```rust
/// Handle `note_insight_tab_chat` — needs SSE channel for streaming events.
pub(super) async fn dispatch_insight_tab_chat(
    core: &AppCore,
    body: &Value,
    sse_channels: &SseChannels,
) -> ApiResult {
    let params: desktop_shared::commands::InsightChatParams =
        match serde_json::from_value(body.get("params").cloned().unwrap_or_default()) {
            Ok(p) => p,
            Err(e) => return err(ApiError::new("INVALID_PARAMS", e.to_string())),
        };

    let session_key = params.session_key.clone();
    let tx = sse_channels
        .entry(session_key)
        .or_insert_with(|| broadcast::channel(256).0)
        .clone();
    let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter { tx });

    match core.note_insight_tab_chat(&params, emitter).await {
        Ok(result) => ok(result),
        Err(e) => err(e),
    }
}
```

- [ ] **Step 3: Route in dispatch.rs**

In the dispatch match block in `crates/desktop/src/dev_server/dispatch.rs`, add:

```rust
"note_insight_tab_chat" => {
    return streaming::dispatch_insight_tab_chat(&core, &body, &state.sse_channels).await;
}
```

This must go BEFORE the catch-all dispatch to `core` methods, alongside the existing `chat_send` routing.

- [ ] **Step 4: Register Tauri commands in the app builder**

In `crates/desktop/src/lib.rs` (or wherever Tauri commands are registered), add `note_insight_tab_chat` and `note_insight_clear_tab_chats` to the `invoke_handler` list.

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p desktop`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/notes.rs crates/desktop/src/dev_server/streaming.rs crates/desktop/src/dev_server/dispatch.rs crates/desktop/src/lib.rs
git commit -m "feat(insights): wire insight tab chat Tauri command and dev server SSE"
```

---

### Task 3: Frontend — useInsightChat Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useInsightChat.ts`

- [ ] **Step 1: Create the hook**

Create `desktop-ui/src/features/notes/hooks/useInsightChat.ts`:

```typescript
import { useQuery } from "@shared/hooks/useQuery";
import type { ChatMessage } from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAgentStream } from "@features/chat/hooks/useAgentStream";
import { ipc } from "@shared/hooks/useIpc";

interface InsightChat {
  messages: ChatMessage[];
  isStreaming: boolean;
  streamingContent: string;
  error: string | null;
  input: string;
  setInput: (value: string) => void;
  send: () => Promise<void>;
  hasMessages: boolean;
}

/**
 * Chat session for an insight tab, grounded in the tab's generated analysis.
 *
 * Reuses the existing streaming infrastructure: `useAgentStream` for live
 * token-by-token rendering, `useQuery("chat_messages")` for persisted history.
 * Send path calls `note_insight_tab_chat` instead of `chat_send`.
 */
export function useInsightChat(
  noteId: string | null,
  tabName: string,
  enabled: boolean,
): InsightChat {
  const sessionKey = noteId ? `insight:${noteId}:${tabName}` : "";

  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    "chat_messages",
    sessionKey && enabled ? { sessionKey } : null,
    [],
  );

  const [input, setInput] = useState("");
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  const stream = useAgentStream(sessionKey, () => {
    setPendingUserMsg(null);
    refetch();
  });

  // Clear streaming segments when new assistant message arrives from refetch
  const { isStreaming, clearSegments, clearTransparency, segments } = stream;
  const hasSegmentsRef = useRef(false);
  hasSegmentsRef.current = segments.length > 0;
  const assistantCountRef = useRef(0);

  useEffect(() => {
    const count = messages.filter((m) => m.role === "assistant").length;
    if (!isStreaming && hasSegmentsRef.current && count > assistantCountRef.current) {
      clearSegments();
      clearTransparency();
    }
    assistantCountRef.current = count;
  }, [messages, isStreaming, clearSegments, clearTransparency]);

  const displayMessages = useMemo(() => {
    if (!pendingUserMsg) return messages;
    return [...messages, { id: "pending", role: "user" as const, content: pendingUserMsg }];
  }, [messages, pendingUserMsg]);

  // Combine streaming text segments into a single content string
  const streamingContent = useMemo(() => {
    return segments
      .filter((s): s is { type: "text"; content: string } => s.type === "text")
      .map((s) => s.content)
      .join("");
  }, [segments]);

  const send = useCallback(async () => {
    if (!input.trim() || stream.isStreaming || !noteId) return;
    const text = input;
    setInput("");
    setPendingUserMsg(text);
    stream.startStreaming();

    try {
      await ipc("note_insight_tab_chat", {
        params: {
          noteId,
          tabName,
          userMessage: text,
          sessionKey,
        },
      });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Failed to send message";
      stream.failStreaming(msg);
      setPendingUserMsg(null);
    }
  }, [input, noteId, tabName, sessionKey, stream]);

  return {
    messages: displayMessages,
    isStreaming: stream.isStreaming,
    streamingContent,
    error: stream.error,
    input,
    setInput,
    send,
    hasMessages: messages.length > 0,
  };
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightChat.ts
git commit -m "feat(insights): add useInsightChat hook for tab chat sessions"
```

---

### Task 4: Frontend — InsightChatInput Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/InsightChatInput.tsx`

- [ ] **Step 1: Create the component**

Create `desktop-ui/src/features/notes/components/insight/InsightChatInput.tsx`:

```tsx
import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { MessageCircle, Send } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ChatMessage } from "@shared/types";

interface InsightChatInputProps {
  messages: ChatMessage[];
  isStreaming: boolean;
  streamingContent: string;
  error: string | null;
  input: string;
  setInput: (value: string) => void;
  send: () => Promise<void>;
  placeholder?: string;
}

export function InsightChatInput({
  messages,
  isStreaming,
  streamingContent,
  error,
  input,
  setInput,
  send,
  placeholder = "Ask about this analysis...",
}: InsightChatInputProps) {
  const [expanded, setExpanded] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const hasMessages = messages.length > 0;

  // Auto-expand when there are messages
  useEffect(() => {
    if (hasMessages) setExpanded(true);
  }, [hasMessages]);

  // Scroll to bottom on new messages or streaming content
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages.length, streamingContent]);

  if (!expanded && !hasMessages) {
    return (
      <div className="mt-4 pt-3 border-t border-border">
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="flex items-center gap-1.5 text-2xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <MessageCircle size={11} />
          Ask a follow-up question
        </button>
      </div>
    );
  }

  return (
    <div className="mt-4 pt-3 border-t border-border space-y-2">
      {/* Messages */}
      {(hasMessages || isStreaming) && (
        <div ref={scrollRef} className="max-h-48 overflow-y-auto space-y-2">
          {messages.map((msg) => (
            <div
              key={msg.id}
              className={`text-[11px] leading-relaxed ${
                msg.role === "user"
                  ? "text-foreground"
                  : "text-muted-foreground"
              }`}
            >
              <span className="text-[9px] text-dim mr-1 font-medium">
                {msg.role === "user" ? "You:" : "AI:"}
              </span>
              {msg.role === "assistant" ? (
                <MarkdownContent content={msg.content} />
              ) : (
                msg.content
              )}
            </div>
          ))}

          {/* Streaming response */}
          {isStreaming && streamingContent && (
            <div className="text-[11px] leading-relaxed text-muted-foreground">
              <span className="text-[9px] text-dim mr-1 font-medium">AI:</span>
              <MarkdownContent content={streamingContent} />
              <span className="inline-block w-1.5 h-3 bg-purple animate-pulse ml-0.5 align-text-bottom rounded-sm" />
            </div>
          )}

          {/* Streaming with no content yet */}
          {isStreaming && !streamingContent && (
            <div className="text-2xs text-dim italic animate-pulse">Thinking...</div>
          )}

          {/* Error */}
          {error && (
            <div className="text-2xs text-destructive">{error}</div>
          )}
        </div>
      )}

      {/* Input */}
      <div className="flex gap-1">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          placeholder={placeholder}
          className="flex-1 text-[11px] px-2 py-1.5 rounded-md bg-input border border-border text-foreground placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-purple/30"
          disabled={isStreaming}
        />
        <button
          type="button"
          onClick={send}
          disabled={isStreaming || !input.trim()}
          className="p-1.5 rounded-md text-purple hover:bg-purple/10 transition-colors disabled:text-dim"
        >
          <Send size={12} />
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/InsightChatInput.tsx
git commit -m "feat(insights): add InsightChatInput component for tab chat UI"
```

---

### Task 5: Frontend — Wire Chat into Synthesis, Gap Analysis, Concept Map Tabs

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

- [ ] **Step 1: Update SynthesisTab to include chat**

Replace `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx`:

```tsx
import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { useInsightChat } from "../../hooks/useInsightChat";
import type { TabStatus } from "../../hooks/useInsightReview";
import { InsightChatInput } from "./InsightChatInput";

interface SynthesisTabProps {
  status: TabStatus;
  content: string;
  noteId: string | null;
}

function SkeletonLoader() {
  return (
    <div className="space-y-3 animate-pulse">
      <div className="h-3 bg-card rounded w-3/4" />
      <div className="h-3 bg-card rounded w-full" />
      <div className="h-3 bg-card rounded w-5/6" />
    </div>
  );
}

export function SynthesisTab({ status, content, noteId }: SynthesisTabProps) {
  const chat = useInsightChat(noteId, "synthesis", status === "done");

  if (status === "idle") {
    return <p className="text-[11px] text-dim italic">Start an insight review to see synthesis</p>;
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate synthesis. Try regenerating.
      </p>
    );
  }

  if (status === "streaming" && !content) {
    return (
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <span className="inline-block w-1.5 h-3.5 bg-purple animate-pulse rounded-sm" />
          <span className="text-[11px] text-muted-foreground animate-pulse">
            Generating synthesis...
          </span>
        </div>
        <SkeletonLoader />
      </div>
    );
  }

  return (
    <div>
      <div className="text-xs text-muted-foreground leading-relaxed">
        <MarkdownContent content={content} />
        {status === "streaming" && (
          <span className="inline-block w-1.5 h-3.5 bg-purple animate-pulse ml-0.5 align-text-bottom rounded-sm" />
        )}
      </div>

      {status === "done" && (
        <InsightChatInput
          messages={chat.messages}
          isStreaming={chat.isStreaming}
          streamingContent={chat.streamingContent}
          error={chat.error}
          input={chat.input}
          setInput={chat.setInput}
          send={chat.send}
          placeholder="Ask about this synthesis..."
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Update GapAnalysisTab to include chat**

Add to `GapAnalysisTab` props interface:

```typescript
interface GapAnalysisTabProps {
  status: TabStatus;
  content: string;
  noteId: string | null;
  onCreateNote?: (title: string, body: string) => void;
}
```

Add inside the component body (before the return):

```typescript
const chat = useInsightChat(noteId, "gaps", status === "done");
```

Add the `useInsightChat` import and `InsightChatInput` import at the top:

```typescript
import { useInsightChat } from "../../hooks/useInsightChat";
import { InsightChatInput } from "./InsightChatInput";
```

Wrap the existing return JSX (the `<div className="space-y-4">` block for the done/streaming case) in a parent `<div>` and add the chat below:

```tsx
return (
  <div>
    <div className="space-y-4">
      <div className="text-xs text-muted-foreground leading-relaxed">
        <MarkdownContent content={markdown} />
      </div>
      {/* ... existing gaps buttons ... */}
    </div>

    {status === "done" && (
      <InsightChatInput
        messages={chat.messages}
        isStreaming={chat.isStreaming}
        streamingContent={chat.streamingContent}
        error={chat.error}
        input={chat.input}
        setInput={chat.setInput}
        send={chat.send}
        placeholder="Ask about these knowledge gaps..."
      />
    )}
  </div>
);
```

- [ ] **Step 3: Update ConceptMapTab to include chat**

Add to `ConceptMapTab` props interface:

```typescript
interface ConceptMapTabProps {
  status: TabStatus;
  mermaid: string;
  fallbackText: string;
  noteId: string | null;
}
```

Add `useInsightChat` and `InsightChatInput` imports, add chat hook call:

```typescript
const chat = useInsightChat(noteId, "concept-map", status === "done");
```

In the mermaid diagram return block and the fallback text return block, wrap in parent `<div>` and add:

```tsx
{status === "done" && (
  <InsightChatInput
    messages={chat.messages}
    isStreaming={chat.isStreaming}
    streamingContent={chat.streamingContent}
    error={chat.error}
    input={chat.input}
    setInput={chat.setInput}
    send={chat.send}
    placeholder="Ask about this concept map..."
  />
)}
```

- [ ] **Step 4: Update InsightReviewPanel to pass noteId to tabs**

In `InsightReviewPanel.tsx`, update tab component renders to pass `noteId`:

```tsx
{state.activeTab === "synthesis" && (
  <SynthesisTab
    status={state.tabs.synthesis.status}
    content={state.tabs.synthesis.content}
    noteId={state.noteId}
  />
)}
{state.activeTab === "gaps" && (
  <GapAnalysisTab
    status={state.tabs.gaps.status}
    content={state.tabs.gaps.content}
    noteId={state.noteId}
    onCreateNote={handleCreateDeepDiveNote}
  />
)}
{state.activeTab === "concept-map" && (
  <ConceptMapTab
    status={state.tabs.conceptMap.status}
    mermaid={state.tabs.conceptMap.mermaid}
    fallbackText={state.tabs.conceptMap.fallbackText}
    noteId={state.noteId}
  />
)}
```

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(insights): wire tab chat into Synthesis, Gap Analysis, and Concept Map tabs"
```

---

### Task 6: Frontend — Wire Chat into Self-Assessment Tab

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx`

The Self-Assessment tab is special: it's a quiz, not markdown. The chat is grounded in quiz questions + answers. Users can ask "Why is the answer B?" or "Explain this concept more."

- [ ] **Step 1: Add chat to SelfAssessmentTab**

Add to `SelfAssessmentTabProps`:

```typescript
interface SelfAssessmentTabProps {
  status: TabStatus;
  questions: QuizQuestion[];
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
  noteId: string | null;
  onAnswer: (questionId: string, answer: string) => void;
  onReveal: (questionId: string) => void;
  onRevealAll: () => void;
  onSaveFlashcards: (deckName: string) => void;
}
```

Note: `noteId` is already in the props (used for scenario challenge). No change needed to the interface.

Add imports and hook at the top of the component:

```typescript
import { useInsightChat } from "../../hooks/useInsightChat";
import { InsightChatInput } from "./InsightChatInput";
```

Inside the component:

```typescript
const chat = useInsightChat(noteId, "assessment", status === "done");
```

At the bottom of the "done" render path (after the save flashcards section, before the closing fragment), add:

```tsx
<InsightChatInput
  messages={chat.messages}
  isStreaming={chat.isStreaming}
  streamingContent={chat.streamingContent}
  error={chat.error}
  input={chat.input}
  setInput={chat.setInput}
  send={chat.send}
  placeholder="Ask about this assessment..."
/>
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx
git commit -m "feat(insights): wire tab chat into Self-Assessment tab"
```

---

### Task 7: Frontend — Upgrade PersonaChat with Streaming + Persistence

**Files:**
- Modify: `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/PersonaCard.tsx`

The existing `PersonaChat` is stateless (history in React state, lost on unmount, no streaming). We upgrade it to use the same infrastructure as tab chat: session persistence + streaming.

- [ ] **Step 1: Rewrite PersonaChat with persistence and streaming**

Replace `desktop-ui/src/features/notes/components/insight/PersonaChat.tsx`:

```tsx
import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { useInsightChat } from "../../hooks/useInsightChat";
import { Send } from "lucide-react";
import { useEffect, useRef } from "react";

interface PersonaChatProps {
  noteId: string;
  personaId: string;
  personaName: string;
}

export function PersonaChat({ noteId, personaId, personaName }: PersonaChatProps) {
  // Persona chats use the same tab chat infrastructure with a persona-namespaced key.
  // The tab name "persona:{id}" tells the backend to load the persona's perspective
  // instead of a regular tab's content.
  const chat = useInsightChat(noteId, `persona:${personaId}`, true);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [chat.messages.length, chat.streamingContent]);

  return (
    <div className="mt-2 space-y-2 border-t border-border pt-2">
      {/* Messages */}
      {(chat.hasMessages || chat.isStreaming) && (
        <div ref={scrollRef} className="max-h-36 overflow-y-auto space-y-2">
          {chat.messages.map((msg) => (
            <div
              key={msg.id}
              className={`text-[11px] leading-relaxed ${
                msg.role === "user" ? "text-foreground" : "text-muted-foreground"
              }`}
            >
              <span className="text-[9px] text-dim mr-1 font-medium">
                {msg.role === "user" ? "You:" : `${personaName}:`}
              </span>
              {msg.role === "assistant" ? (
                <MarkdownContent content={msg.content} />
              ) : (
                msg.content
              )}
            </div>
          ))}

          {chat.isStreaming && chat.streamingContent && (
            <div className="text-[11px] leading-relaxed text-muted-foreground">
              <span className="text-[9px] text-dim mr-1 font-medium">{personaName}:</span>
              <MarkdownContent content={chat.streamingContent} />
              <span className="inline-block w-1.5 h-3 bg-purple animate-pulse ml-0.5 align-text-bottom rounded-sm" />
            </div>
          )}

          {chat.isStreaming && !chat.streamingContent && (
            <div className="text-2xs text-dim italic animate-pulse">
              {personaName} is thinking...
            </div>
          )}

          {chat.error && <div className="text-2xs text-destructive">{chat.error}</div>}
        </div>
      )}

      {/* Input */}
      <div className="flex gap-1">
        <input
          type="text"
          value={chat.input}
          onChange={(e) => chat.setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              chat.send();
            }
          }}
          placeholder={`Ask ${personaName}...`}
          className="flex-1 text-[11px] px-2 py-1 rounded-md bg-input border border-border text-foreground placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-purple/30"
          disabled={chat.isStreaming}
        />
        <button
          type="button"
          onClick={chat.send}
          disabled={chat.isStreaming || !chat.input.trim()}
          className="p-1 rounded-md text-purple hover:bg-purple/10 transition-colors disabled:text-dim"
        >
          <Send size={12} />
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Update PersonaCard props**

In `PersonaCard.tsx`, the `PersonaChat` component no longer needs `personaRole` and `personaTone` props (the backend loads persona attributes from DB). Update the `PersonaChat` mount:

```tsx
{showChat && (
  <PersonaChat
    noteId={noteId}
    personaId={persona.id}
    personaName={persona.name}
  />
)}
```

Remove `personaRole` and `personaTone` from the props passed to `PersonaChat`.

- [ ] **Step 3: Backend — Handle persona tab name in insight_chat.rs**

In `crates/app-core/src/handlers/notes/insight_chat.rs`, update the `note_insight_tab_chat` method to handle persona tab names. The tab_name format for personas is `persona:{personaId}`.

In the tab content loading section, add a case for persona:

```rust
// 2. Load tab content from latest insight
let tab_content = if let Some(insight_service) = &self.insight_service {
    if let Ok(Some(latest)) = insight_service.get_latest(&params.note_id).await {
        let content: feature_insights::types::InsightContent =
            serde_json::from_str(&latest.content).unwrap_or_default();

        if let Some(persona_id) = params.tab_name.strip_prefix("persona:") {
            // Load this persona's perspective from the stored perspectives
            self.load_persona_perspective(&latest, persona_id)
                .unwrap_or_default()
        } else {
            match params.tab_name.as_str() {
                "synthesis" => content.synthesis.unwrap_or_default(),
                "gaps" | "gap_analysis" => content.gap_analysis.unwrap_or_default(),
                "assessment" | "self_assessment" => content.self_assessment.unwrap_or_default(),
                "concept-map" | "concept_map" => content.concept_map.unwrap_or_default(),
                _ => String::new(),
            }
        }
    } else {
        String::new()
    }
} else {
    String::new()
};
```

Add the helper method:

```rust
/// Load a specific persona's perspective from the stored insight content.
fn load_persona_perspective(
    &self,
    insight_row: &feature_insights::types::InsightReviewRow,
    persona_id: &str,
) -> Option<String> {
    let content: feature_insights::types::InsightContent =
        serde_json::from_str(&insight_row.content).ok()?;
    let perspectives = content.perspectives?;
    // Perspectives are stored as a combined markdown with persona sections
    // For now, include the full perspectives text as context
    Some(perspectives)
}
```

Also update `tab_chat_system_prompt` to handle persona:

```rust
fn tab_chat_system_prompt(
    note_title: &str,
    note_body: &str,
    tab_name: &str,
    tab_content: &str,
    persona_info: Option<(&str, &str)>, // (name, role)
) -> String {
    let body_preview = common::truncate_at_boundary(note_body, 3000);

    if let Some((name, role)) = persona_info {
        return format!(
            "You are {name}, a {role}.\n\
             You previously analyzed a student's note and gave your perspective. \
             The student is now asking follow-up questions.\n\
             Stay in character. Be concise (2-4 sentences).\n\n\
             --- NOTE: {note_title} ---\n{body_preview}\n--- END NOTE ---\n\n\
             --- YOUR PERSPECTIVE ---\n{tab_content}\n--- END PERSPECTIVE ---"
        );
    }

    // ... existing tab prompt logic
}
```

For persona chat, resolve the persona from DB:

```rust
if let Some(persona_id) = params.tab_name.strip_prefix("persona:") {
    if let Some(persona_repo) = &self.persona_repo {
        if let Ok(Some(persona)) = persona_repo.get(persona_id).await {
            persona_info = Some((persona.name.clone(), persona.role.clone()));
        }
    }
}
```

- [ ] **Step 4: Update `note_insight_clear_tab_chats` to also clear persona sessions**

In `insight_chat.rs`, the `note_insight_clear_tab_chats` method should also clear persona sessions. Since we don't know all persona IDs upfront, we clear sessions matching the prefix:

```rust
pub async fn note_insight_clear_tab_chats(&self, note_id: &str) -> Result<(), ApiError> {
    // Clear standard tab chats
    let tab_names = ["synthesis", "gaps", "assessment", "concept-map"];
    for tab in &tab_names {
        let session_key = format!("insight:{note_id}:{tab}");
        let _ = self.repos.sessions.delete_session(&session_key).await;
    }
    // Clear persona chats — delete all sessions matching prefix
    let _ = self
        .repos
        .sessions
        .delete_sessions_by_prefix(&format!("insight:{note_id}:persona:"))
        .await;
    Ok(())
}
```

This requires adding a `delete_sessions_by_prefix` method to `SessionRepo`:

```rust
pub async fn delete_sessions_by_prefix(&self, prefix: &str) -> Result<u64, StorageError> {
    let pattern = format!("{prefix}%");
    let result = sqlx::query("DELETE FROM sessions WHERE key LIKE ?1")
        .bind(&pattern)
        .execute(&self.pool)
        .await?;
    // Also delete orphaned messages
    sqlx::query(
        "DELETE FROM session_messages WHERE session_key LIKE ?1"
    )
    .bind(&pattern)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected())
}
```

Add this to `crates/storage/src/repos/session.rs`.

- [ ] **Step 5: Lint and check compilation**

Run: `cd desktop-ui && bun run lint:fix`
Run: `cargo clippy -p app-core -p storage --all-targets`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/PersonaChat.tsx desktop-ui/src/features/notes/components/insight/PersonaCard.tsx crates/app-core/src/handlers/notes/insight_chat.rs crates/storage/src/repos/session.rs
git commit -m "feat(insights): upgrade PersonaChat with streaming and persistence"
```

---

### Task 8: Session Lifecycle — Clear on Regenerate

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

When the user regenerates an insight (full or per-tab), the chat sessions should be cleared so conversations stay grounded in the latest analysis.

- [ ] **Step 1: Add clear action to useInsightReview**

In `useInsightReview.ts`, add a `clearTabChats` action that calls the backend:

```typescript
const clearTabChats = useCallback(async () => {
  if (!state.noteId) return;
  try {
    await ipc("note_insight_clear_tab_chats", { noteId: state.noteId });
  } catch {
    // Best effort — don't block regeneration
  }
}, [state.noteId]);
```

Expose it in the returned `actions` object.

- [ ] **Step 2: Call clear on regeneration**

In the existing `regenerateTab` action (or in the `InsightReviewPanel`'s regenerate button handler), call `clearTabChats()` before triggering regeneration:

```typescript
const handleRegenerate = async () => {
  await actions.clearTabChats();
  actions.regenerateTab(state.activeTab);
};
```

Also call `clearTabChats()` when a full insight review is started (in the `open` flow or when the user manually triggers a full regeneration).

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightReview.ts desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(insights): clear tab chat sessions on insight regeneration"
```

---

### Task 9: Backend Tests

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_chat.rs` (add `#[cfg(test)] mod tests`)

- [ ] **Step 1: Test system prompt builder**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_chat_system_prompt_synthesis() {
        let prompt = tab_chat_system_prompt(
            "Rust Ownership",
            "Ownership is a set of rules...",
            "synthesis",
            "The note covers ownership basics...",
            None,
        );
        assert!(prompt.contains("Synthesis"));
        assert!(prompt.contains("Rust Ownership"));
        assert!(prompt.contains("Ownership is a set of rules"));
        assert!(prompt.contains("The note covers ownership basics"));
    }

    #[test]
    fn test_tab_chat_system_prompt_persona() {
        let prompt = tab_chat_system_prompt(
            "Rust Ownership",
            "Content here",
            "persona:abc",
            "From a skeptic's perspective...",
            Some(("The Skeptic", "Devil's advocate")),
        );
        assert!(prompt.contains("You are The Skeptic"));
        assert!(prompt.contains("Devil's advocate"));
        assert!(prompt.contains("YOUR PERSPECTIVE"));
    }

    #[test]
    fn test_tab_label_mapping() {
        // Verify all tab names produce valid labels
        for tab in &["synthesis", "gaps", "gap_analysis", "assessment", "self_assessment", "concept-map", "concept_map"] {
            let prompt = tab_chat_system_prompt("T", "B", tab, "C", None);
            assert!(!prompt.contains("Analysis Analysis"), "Double 'Analysis' for tab {tab}");
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p app-core -E 'test(tab_chat)'`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_chat.rs
git commit -m "test(insights): add unit tests for tab chat system prompt builder"
```

---

## Testing Checklist

After all tasks are complete, verify end-to-end:

1. **Tab chat — happy path**: Open a note with existing insight → switch to Synthesis tab → type a question → verify streaming response appears → verify message persists after tab switch and back
2. **Tab chat — all tabs**: Repeat for Gap Analysis, Self-Assessment, Concept Map
3. **Persona chat — streaming**: Open Perspectives tab → click "Ask this persona" → type question → verify streaming response (not blocking)
4. **Persona chat — persistence**: Close persona card → reopen → verify messages are still there
5. **Session lifecycle**: Regenerate insight → verify chat messages are cleared → start fresh conversation
6. **Browser dev mode**: Test with `bun run dev` + embedded HTTP server → verify SSE streaming works via EventSource
7. **Note switching**: Switch to a different note → switch back → verify each note has its own chat sessions
