# Unified Voice+Text Sessions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify voice and text chat into a single session system — voice becomes an I/O layer on top of existing chat, not a separate conversation system.

**Architecture:** `VoiceConversationManager::start()` accepts an optional `session_key` from the frontend (the active chat thread). When provided, voice messages go into that thread. When absent (hotkey mode), it resolves the most recent `chat:*` session within a 5-minute warm window. Voice transcripts are persisted as `ChatMessage` rows and emit `chat:message_added` for real-time UI updates. A new mic-button dictation mode captures speech and returns the transcript to the input field for editing.

**Tech Stack:** Rust, TypeScript/React, Tauri 2 IPC.

---

## File Structure

### Backend
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` — Accept session key, use `chat:` prefix, persist messages, auto-title
- Modify: `crates/app-core/src/handlers/voice_conversation_commands.rs` — Pass session_key param
- Modify: `crates/app-core/src/handlers/voice.rs` — Replace legacy handlers with dictation commands
- Modify: `crates/desktop/src/commands/voice_conversation.rs` — Add session_key to Tauri command
- Modify: `crates/desktop/src/commands/voice.rs` — Replace legacy with dictation commands
- Modify: `crates/desktop/src/main.rs` — Update command registrations

### Frontend
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts` — Accept session_key in start()
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` — Pass active session key
- Modify: `desktop-ui/src/features/chat/components/ChatInput.tsx` — Mic button dictation mode

---

## Task 1: Session Key Unification — Use `chat:` Prefix

Change `VoiceConversationManager` to use `chat:<uuid>` sessions instead of `desktop:<uuid>`. Accept an optional explicit session key.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Replace `create_voice_session_key()` with `chat:` prefix**

In `crates/app-core/src/handlers/voice_conversation.rs`, replace `create_voice_session_key()` (lines 143-148):

```rust
pub fn create_voice_session_key() -> SessionKey {
    SessionKey::new(
        &ChannelName::new("desktop"),
        &ChatId::new(Uuid::new_v4().to_string()),
    )
}
```

with:

```rust
fn create_chat_session_key() -> SessionKey {
    SessionKey::new(
        &ChannelName::new("chat"),
        &ChatId::new(Uuid::new_v4().to_string()),
    )
}
```

Note: changed from `pub` to private `fn` — external callers should use `start(session_key)` instead.

- [ ] **Step 2: Update `resolve_session()` to accept optional override and use `chat:` prefix**

Replace `resolve_session()` (lines 249-290) with:

```rust
    pub async fn resolve_session(&self, explicit_key: Option<&str>) -> SessionKey {
        // 0. Explicit key provided by the frontend (e.g., active chat thread)
        if let Some(key) = explicit_key {
            return SessionKey::from_raw(key);
        }

        let config = self.config.read().await;
        let warm_session_min = config.conversation.warm_session_minutes;
        let warm_chat_min = config.conversation.warm_chat_minutes;
        drop(config);

        // 1. If manager already has an active session (phase != Idle), reuse it
        {
            let state = self.state.lock().await;
            if state.phase != ConversationPhase::Idle {
                if let Some(ref key) = state.session_key {
                    return key.clone();
                }
            }

            // 2. If previous session_key exists and last_activity is warm, reattach
            if let Some(ref key) = state.session_key {
                if is_warm_session(state.last_activity, warm_session_min) {
                    return key.clone();
                }
            }
        }

        // 3. Query DB for most recent session — reuse if within warm window
        if let Ok(sessions) = self
            .repos
            .sessions
            .list_sessions_since(Utc::now() - Duration::minutes(warm_chat_min as i64))
            .await
        {
            if let Some(session) = sessions.first() {
                return SessionKey::from_raw(&session.key);
            }
        }

        // 4. Otherwise create new chat session
        create_chat_session_key()
    }
```

Key changes:
- New `explicit_key: Option<&str>` parameter — step 0 bypass
- Step 3 removes the `starts_with("desktop:")` filter — uses ANY recent session
- Step 4 uses `create_chat_session_key()` instead of `create_voice_session_key()`

Check if `SessionKey::from_raw` exists. If not, use `SessionKey::from_parts` by parsing the key, or add a simple constructor. Search for existing `SessionKey` constructors in `crates/common/src/types.rs`.

- [ ] **Step 3: Update `start()` to accept optional session_key**

In `start()` (lines 295-375), change the signature and pass the key to `resolve_session`:

```rust
    pub async fn start(&self, session_key: Option<String>) -> common::Result<StartResponse> {
```

Change line ~312 from:
```rust
        let session_key = self.resolve_session().await;
```
to:
```rust
        let session_key = self.resolve_session(session_key.as_deref()).await;
```

- [ ] **Step 4: Auto-title from first transcript instead of "New voice session"**

In `start()` (around line 335), replace:
```rust
    } else {
        "New voice session".to_string()
    };
```
with:
```rust
    } else {
        String::new() // Will be set from first transcript
    };
```

Then in `run_reflecting_phase()`, after getting the transcript, update the session title if it's empty. Find where `pending_transcript` is set (around line 620) and add:

```rust
    // Auto-title from first transcript (same as text chat)
    {
        let mut state = self.state.lock().await;
        if state.session_title.is_empty() {
            let title = transcript_text.chars().take(60).collect::<String>();
            state.session_title = title.clone();
            // Update in DB
            let _ = self.repos.sessions.update_session_title(
                state.session_key.as_ref().unwrap().as_str(),
                &title,
            ).await;
        }
    }
```

Check if `update_session_title` exists on `SessionRepo`. If not, use `upsert_voice_session` with updated metadata.

- [ ] **Step 5: Update `reset_for_new_session()`**

In `reset_for_new_session()` (lines 98-109), change:
```rust
        self.session_title = "New voice session".to_string();
```
to:
```rust
        self.session_title = String::new();
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p app-core`

Expected: May have errors from callers of `start()` and `resolve_session()` that need the new parameter — fix those in the next task.

---

## Task 2: Update IPC — Pass session_key to voice_conversation_start

Wire the `session_key` parameter through the IPC chain: Tauri command → AppCore → VoiceConversationManager.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation_commands.rs`
- Modify: `crates/desktop/src/commands/voice_conversation.rs`

- [ ] **Step 1: Update AppCore command handler**

In `crates/app-core/src/handlers/voice_conversation_commands.rs`, change `voice_conversation_start` (lines 21-30):

```rust
    pub async fn voice_conversation_start(
        &self,
        session_key: Option<String>,
    ) -> Result<VoiceConversationStartResponse, ApiError> {
        let manager = self.voice_conversation_manager()?;
        let result = manager
            .start(session_key)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))?;
        Ok(result.into())
    }
```

- [ ] **Step 2: Update Tauri command**

In `crates/desktop/src/commands/voice_conversation.rs`, change `voice_conversation_start` (lines 16-23):

```rust
#[tauri::command]
pub async fn voice_conversation_start(
    state: State<'_, Arc<AppCore>>,
    session_key: Option<String>,
) -> Result<VoiceConversationStartResponse, ApiError> {
    let result = state.voice_conversation_start(session_key).await?;
    set_tray_voice(true, 1);
    Ok(result)
}
```

- [ ] **Step 3: Update dev server dispatch**

Search for `voice_conversation_start` in `crates/desktop/src/dev_server/` and update the dispatch to pass the session_key from the request body:

```rust
"voice_conversation_start" => {
    let session_key = body.get("sessionKey").and_then(|v| v.as_str()).map(String::from);
    dev::val(core.voice_conversation_start(session_key).await)
}
```

- [ ] **Step 4: Update main.rs hotkey handler**

In `crates/desktop/src/main.rs`, find where `voice_conversation_start` is called from the hotkey handler (the `manager.start()` call). Update to pass `None` (hotkey has no active session context):

Find: `manager.start().await`
Replace: `manager.start(None).await`

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p desktop`

Expected: Clean compile.

---

## Task 3: Persist Voice Messages as ChatMessages

When voice produces a transcript, persist it as a `ChatMessage` and emit `chat:message_added` so the chat UI updates in real-time. The existing `useChatSession.ts` already subscribes to this event.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Persist user transcript before agent call**

In `run_reflecting_phase()` (around line 620, after getting `transcript_text`), persist the voice transcript as a user ChatMessage before calling the agent:

```rust
        // Persist voice transcript as a user message in the chat session
        let session_key_str = {
            let state = self.state.lock().await;
            state.session_key.as_ref().unwrap().to_string()
        };

        // Store user message
        let user_msg_id = Uuid::new_v4().to_string();
        let _ = self.repos.sessions.store_message(
            &session_key_str,
            &user_msg_id,
            "user",
            &transcript_text,
            Some(&serde_json::json!({ "source": "voice" })),
        ).await;

        // Emit real-time update for chat UI
        self.emitter.emit_event(
            "chat:message_added",
            &serde_json::json!({
                "sessionKey": session_key_str,
                "source": "voice",
            }),
        );
```

Check `SessionRepo::store_message` — verify it exists and its signature. If it doesn't exist, look for alternatives like `insert_message`, `add_message`, or check how `chat_send()` in `streaming.rs` persists user messages (around line 826-835).

- [ ] **Step 2: Emit chat:message_added after agent response**

In the event loop inside `run_reflecting_phase()` (around line 692), when `AgentEvent::Done` fires, emit `chat:message_added` again for the assistant message:

```rust
    agent::AgentEvent::Done { content, .. } => {
        // ... existing code ...

        // Emit real-time update for assistant response
        self.emitter.emit_event(
            "chat:message_added",
            &serde_json::json!({
                "sessionKey": session_key_str,
                "source": "voice",
            }),
        );
    }
```

- [ ] **Step 3: Emit `emit_chat_thread` for sidebar updates**

After the reflecting phase completes (around line 757), emit a thread update so the sidebar shows the voice session with proper title:

```rust
        // Notify sidebar of thread update
        let is_new = {
            let state = self.state.lock().await;
            state.turn_count <= 1
        };
        self.emitter.emit_event(
            if is_new { "chat:thread_created" } else { "chat:thread_updated" },
            &serde_json::json!({ "sessionKey": session_key_str }),
        );
```

Check how `emit_chat_thread` works in `streaming.rs` and use the same pattern/event names.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p app-core`

---

## Task 4: Replace Legacy Voice Handlers with Dictation

Delete the legacy `voice_start_capture`, `voice_stop_capture` handlers and replace with thin dictation commands that capture → transcribe → return text.

**Files:**
- Modify: `crates/app-core/src/handlers/voice.rs`
- Modify: `crates/desktop/src/commands/voice.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Replace voice.rs handlers**

Replace the entire `crates/app-core/src/handlers/voice.rs` with dictation handlers:

```rust
//! Voice dictation handlers — speech-to-text for the chat input field.

use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    /// Start voice dictation — begins capturing audio for speech-to-text.
    /// Returns immediately. Call `voice_stop_dictation` to stop and get transcript.
    pub async fn voice_start_dictation(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        service
            .start_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;
        Ok(())
    }

    /// Stop voice dictation — finalizes capture and returns the transcript text.
    pub async fn voice_stop_dictation(&self) -> Result<String, ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        match result {
            Some((transcript, _metadata)) => Ok(transcript.text),
            None => Ok(String::new()),
        }
    }

    /// Simulate a VoiceEvent for dev/testing.
    pub async fn voice_simulate_event(
        &self,
        event_json: serde_json::Value,
    ) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let event: voice_engine::VoiceEvent = serde_json::from_value(event_json)
            .map_err(|e| ApiError::new("VALIDATION", &format!("Invalid VoiceEvent: {e}")))?;
        service
            .emit_event(event)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))
    }
}
```

- [ ] **Step 2: Update desktop Tauri commands**

Replace `crates/desktop/src/commands/voice.rs` with:

```rust
//! Voice dictation Tauri command handlers.

use std::sync::Arc;

use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn voice_start_dictation(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    state.voice_start_dictation().await
}

#[tauri::command]
pub async fn voice_stop_dictation(
    state: State<'_, Arc<AppCore>>,
) -> Result<String, ApiError> {
    state.voice_stop_dictation().await
}

#[tauri::command]
pub async fn voice_simulate_event(
    state: State<'_, Arc<AppCore>>,
    event: serde_json::Value,
) -> Result<(), ApiError> {
    state.voice_simulate_event(event).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_start_dictation",
    "voice_stop_dictation",
    "voice_simulate_event",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "voice_start_dictation" => dev::val(core.voice_start_dictation().await),
        "voice_stop_dictation" => dev::val(core.voice_stop_dictation().await),
        "voice_simulate_event" => {
            let event = body
                .get("event")
                .cloned()
                .unwrap_or(serde_json::json!(null));
            dev::val(core.voice_simulate_event(event).await)
        }
        _ => return None,
    })
}
```

- [ ] **Step 3: Update main.rs command registrations**

In `crates/desktop/src/main.rs`, find where `voice_start_capture`, `voice_stop_capture`, `voice_dismiss`, `voice_get_status` are registered in the Tauri `invoke_handler` and replace with:

```rust
voice_start_dictation,
voice_stop_dictation,
voice_simulate_event,
```

Remove any references to `voice_start_capture`, `voice_stop_capture`, `voice_dismiss`, `voice_get_status`.

Also search for the focus-session fallback path in the hotkey handler (where `voice_start_capture` was called as a legacy fallback) and update it to use the conversation manager instead.

- [ ] **Step 4: Update dev server dispatch list**

In `crates/desktop/src/dev_server/mod.rs`, update the voice module dispatch entry and the test that checks all commands are covered.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p desktop`

Expected: Clean compile. The `dev_server_covers_all_tauri_commands` test should still pass with updated command names.

---

## Task 5: Frontend — Pass Session Key to Voice Start

Update the frontend to pass the active chat thread's session key when starting voice.

**Files:**
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts`
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`

- [ ] **Step 1: Update `start()` in useVoiceConversation to accept sessionKey**

In `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts`, change the `start` function (lines 154-161):

```typescript
const start = useCallback(async (sessionKey?: string): Promise<SessionInfo> => {
  const result = await ipc<{ sessionKey: string; sessionTitle: string; isContinuing: boolean }>(
    "voice_conversation_start",
    sessionKey ? { sessionKey } : undefined,
  );
  const info: SessionInfo = { key: result.sessionKey, title: result.sessionTitle, turnCount: 0 };
  setSessionInfo(info);
  return info;
}, []);
```

- [ ] **Step 2: Update VoiceBrainOrb to pass session key when available**

In `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`, the auto-start in browser dev mode (around line 39) passes no session key — this is correct for hotkey mode. No change needed here since the hotkey always passes `None`.

For the chat page integration, the session key will be passed from `ChatInput` (Task 7), not from the orb.

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

---

## Task 6: Frontend — Mic Button Dictation in Chat Input

Add dictation functionality to the existing mic button in `ChatInput.tsx`. Click mic → capture → ASR → insert transcript into input field.

**Files:**
- Modify: `desktop-ui/src/features/chat/components/ChatInput.tsx`

- [ ] **Step 1: Add dictation state and handlers**

In `ChatInput.tsx`, add state and IPC handlers for dictation:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useEvent } from "@shared/hooks/useEvent";
import { useState } from "react";

// Inside the component:
const [isDictating, setIsDictating] = useState(false);

const startDictation = async () => {
  try {
    setIsDictating(true);
    await ipc("voice_start_dictation");
  } catch {
    setIsDictating(false);
  }
};

const stopDictation = async () => {
  try {
    const transcript = await ipc<string>("voice_stop_dictation");
    if (transcript) {
      onInputChange(input ? `${input} ${transcript}` : transcript);
    }
  } finally {
    setIsDictating(false);
  }
};

// Auto-stop on silence (voice:event with captureEnded)
useEvent<Record<string, unknown>>("voice:event", (payload) => {
  if (isDictating && payload.type === "captureEnded") {
    stopDictation();
  }
});
```

- [ ] **Step 2: Wire the mic button**

Replace the existing non-functional mic button (lines 91-97) with:

```tsx
<button
  type="button"
  aria-label={isDictating ? "Stop dictation" : "Voice input"}
  onClick={isDictating ? stopDictation : startDictation}
  className={`size-8 flex items-center justify-center transition-colors shrink-0 rounded-lg ${
    isDictating
      ? "text-destructive animate-pulse bg-destructive/10"
      : "text-muted-foreground hover:text-foreground hover:bg-accent"
  }`}
>
  <Mic className="size-4" strokeWidth={1.5} />
</button>
```

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

---

## Task 7: Integration Verification

Verify everything compiles, tests pass, and the feature works end-to-end.

**Files:**
- All modified files from Tasks 1-6

- [ ] **Step 1: Full workspace compilation**

Run: `cargo build --workspace`

Expected: Clean build.

- [ ] **Step 2: Clippy check**

Run: `cargo clippy --workspace --all-targets --all-features`

Expected: No new warnings.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace`

Expected: All pass. The `dev_server_covers_all_tauri_commands` test may need attention if command names changed.

- [ ] **Step 4: Frontend build + lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

- [ ] **Step 5: Manual smoke test — Hotkey mode**

Run: `cargo tauri dev`

1. Open the app, create a text chat session, type a message
2. Press Alt+Shift+V — orb should appear
3. Speak — transcript should appear in the SAME chat thread as a user message
4. Agent responds — response appears in the same chat thread + plays via TTS
5. Session title in sidebar should update from transcript content (not "New voice session")

- [ ] **Step 6: Manual smoke test — Dictation mode**

1. In a chat session, click the mic icon in the input bar
2. Speak a message
3. Mic icon should pulse red during capture
4. On silence, transcript appears in the input field
5. Edit if needed, then click Send — message sends through normal chat flow

- [ ] **Step 7: Manual smoke test — Session continuity**

1. Type a text message: "Remember my name is Jayden"
2. Press Alt+Shift+V and ask via voice: "What's my name?"
3. The agent should know your name from the text message in the same session
