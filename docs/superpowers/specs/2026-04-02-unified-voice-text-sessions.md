# Unified Voice+Text Sessions — Design Spec

## Goal

Voice becomes an input/output layer on top of the existing text chat system — not a separate conversation system. All sessions use `chat:<uuid>` format. Users can click the mic icon in any chat session to speak, and voice transcripts appear as regular messages in the thread.

## Current State

- Text chat creates `chat:<uuid>` sessions, sends messages through `chat_send` IPC
- Voice creates separate `desktop:<uuid>` sessions via `VoiceConversationManager`
- Both call `agent.process_direct_streaming(text, session_key)` — same agent pipeline
- Voice sessions appear as separate "New voice session" entries in sidebar
- Chat page doesn't update in real-time when voice adds messages to a session
- No mic-button dictation in the chat input

## Design

### 1. Session Unification

Remove the `desktop:` session concept. All sessions use `chat:<uuid>`.

**`VoiceConversationManager::start()`** accepts an optional `session_key: Option<String>`:
- If provided (from frontend — the active chat thread key), use it directly. Skip `resolve_session()` entirely.
- If `None` (hotkey with no active thread visible), `resolve_session()` queries recent `chat:*` sessions within the warm window (5 min default). If one exists, reuse it. Otherwise create a new `chat:<uuid>`.

**`resolve_session()` changes:**
- Step 3 currently filters to `desktop:` prefix → change to `chat:` prefix (or remove the prefix filter entirely since all sessions are now `chat:`).
- `create_voice_session_key()` → delete. Replace with inline `SessionKey::new(&ChannelName::new("chat"), &ChatId::new(Uuid::new_v4().to_string()))`.

**Session titles:** Voice sessions auto-update their title from the first transcript content (first 60 chars), matching text chat behavior. Remove the hardcoded `"New voice session"` string.

**Legacy cleanup:** Delete `voice_stop_capture` and `voice_start_capture` handlers in `handlers/voice.rs`. Remove their Tauri command registrations. Everything routes through `VoiceConversationManager`.

### 2. Real-Time Chat Updates

When hotkey-mode voice adds messages to a session, the chat page must update live.

**Backend events:**
- `VoiceConversationManager::run_reflecting_phase()` already emits `AGENT_CONTENT_CHUNK` and `AGENT_DONE` events with the session key. These events need to trigger chat message persistence and frontend notification.
- After voice transcription, emit a `chat:message_added` Tauri event with `{ session_key, message }` so the frontend can append the user's voice message to the thread.
- After agent response completes, the existing `AGENT_DONE` event flow should persist the assistant message and emit another `chat:message_added`.

**Frontend subscription:**
- `useChatSession` subscribes to `chat:message_added` events filtered by the active `sessionKey`.
- When a new message arrives (from voice or another source), it's appended to the local message list without requiring a full refetch.
- This also fixes the general problem of chat not updating when background processes (cron, voice) add messages.

**Message metadata:**
- Voice-originated user messages include `source: "voice"` in their metadata, enabling the UI to show a small mic icon on the message bubble.

### 3. Two Voice Modes

#### Hotkey Mode (Alt+Shift+V)

Full autonomous conversation loop: capture → ASR → agent → TTS → auto-resume.

- The orb appears for visual feedback (WebGL shader).
- Transcript is persisted as a user `ChatMessage` with `source: "voice"`.
- Agent response is persisted as an assistant `ChatMessage`.
- Both appear in the chat thread in real-time via `chat:message_added` events.
- Uses the active session (if provided) or resolves via warm-window logic.

#### Mic-Button Mode (Chat Input)

Dictation only: capture → ASR → insert transcript into the chat input field.

- User clicks the mic icon in `ChatInput` → starts capture via `voice_service.start_capture()`.
- Visual feedback: mic icon turns red/pulsing, waveform or audio level indicator in the input area.
- On silence detection or user clicks stop → `voice_service.stop_capture()` → transcript returned.
- Transcript text is inserted into the input field (`<textarea>`). User can edit before sending.
- No agent call. No TTS. User sends manually via the existing Send button.
- This is a thin frontend feature — calls `voice_start_dictation` / `voice_stop_dictation` IPC commands that delegate to `VoiceService::start_capture()` and `stop_capture()` without the conversation manager loop.

### 4. Data Flow

```
Hotkey Mode:
  Alt+Shift+V → VoiceConversationManager::start(session_key: Option)
    → resolve or use provided chat:<uuid>
    → Listening → capture → ASR transcript
    → persist as ChatMessage(source: "voice") → emit chat:message_added
    → Reflecting → agent.process_direct_streaming(text, chat:<uuid>)
    → persist assistant ChatMessage → emit chat:message_added
    → Speaking → TTS → AudioPlayer
    → auto-resume → Listening

Mic-Button Mode:
  Click mic → voice_start_dictation IPC
    → VoiceService::start_capture()
    → user speaks → silence or click stop
    → voice_stop_dictation IPC → VoiceService::stop_capture()
    → return transcript text to frontend
    → insert into input field
    → user edits + sends via chat_send (normal text flow)
```

### 5. Files Changed

#### Backend (Rust)

| File | Change |
|------|--------|
| `handlers/voice_conversation.rs` | `start()` accepts `session_key: Option<String>`. `resolve_session()` uses `chat:` prefix. Auto-title from first transcript. Remove `create_voice_session_key()`. Persist voice messages as `ChatMessage`. Emit `chat:message_added`. |
| `handlers/voice_conversation_commands.rs` | `voice_conversation_start` IPC accepts optional `sessionKey` param, passes to manager. |
| `handlers/voice.rs` | Delete `voice_start_capture`, `voice_stop_capture`, `voice_dismiss` handlers. Add `voice_start_dictation`, `voice_stop_dictation` (thin wrappers around VoiceService capture). |
| `handlers/chat/streaming.rs` | Emit `chat:message_added` event after persisting messages (enables real-time updates from any source). |
| `desktop/commands/voice.rs` | Remove legacy `voice_start_capture`, `voice_stop_capture`, `voice_get_status`, `voice_simulate_event` Tauri commands. Add `voice_start_dictation`, `voice_stop_dictation`. Update `DEV_COMMANDS`. |
| `desktop/commands/voice_conversation.rs` | Add `session_key: Option<String>` param to `voice_conversation_start`. |
| `desktop/src/main.rs` | Update Tauri command registrations. |

#### Frontend (TypeScript)

| File | Change |
|------|--------|
| `hooks/useVoiceConversation.ts` | `start()` accepts optional `sessionKey`, passes to IPC. |
| `hooks/useChatSession.ts` | Subscribe to `chat:message_added` events for active session. Append messages without refetch. |
| `components/ChatInput.tsx` (or equivalent) | Mic button: on click → `voice_start_dictation` IPC, show recording UI. On stop → `voice_stop_dictation`, insert transcript into input. |
| `components/VoiceBrainOrb.tsx` | When triggered from chat page, pass active session key to `start()`. |
| `components/ChatMessage.tsx` (or equivalent) | Show mic icon for messages with `source: "voice"` metadata. |

### 6. What's NOT Changing

- `VoiceConversationManager` conversation loop (capture → ASR → reflect → speak → resume) — kept as-is
- Agent pipeline (`process_direct_streaming`) — no changes
- WebGL orb component — no changes
- AudioPlayer, streaming ASR, VAD — all recent work stays
- Session storage schema — `chat:` sessions already work

### 7. Migration

Pre-release, no user data concerns. Existing `desktop:*` sessions in dev DB can be ignored or deleted.
