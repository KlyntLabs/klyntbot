# Chat Page Completion Design

**Date:** 2026-02-26
**Scope:** Complete all Chat page features with real API integration — no mocks, no TODOs.

## Current State

Chat.tsx (~1065 lines) has working WebSocket streaming, message display, and thinking indicators. Six areas are stubbed or incomplete.

## Changes

### 1. Session Switching + History Loading
- Wire `loadSession(key)` → `GET /api/sessions/:key` → populate messages from `SessionMessageRow[]`
- Add `loadSession(key)` method to `useAgent` hook
- New session button: clears messages, generates fresh session key
- Persist `sessionKey` in `sessionStorage` for reload persistence

### 2. Interaction Request UI
- Handle `interaction.request` event in `useAgent` → store in `pendingInteraction` state
- Render `InteractionPanel` inline below last message with question-type-specific inputs:
  - `singleSelect` → radio buttons
  - `multiSelect` → checkboxes
  - `yesNo` → two buttons
  - `freeText` → text input
- Submit → `socket.sendInteractionResponse(requestId, response)`

### 3. Markdown Rendering
- Install `marked` + `DOMPurify` (or equivalent)
- `renderMarkdown()` utility with code block syntax highlighting via CSS
- Apply in MessageBubble via `dangerouslySetInnerHTML`
- Style with `--codex-*` CSS variables

### 4. Sidebar — Quick Tasks
- `useApi('/api/tasks')` → filter pending, show top 5
- Priority badge, status cycle on click, link to detail

### 5. Sidebar — Upcoming Calendar
- `useApi('/api/calendar/events?limit=5')` → show next 5 events
- Event title, relative time, provider indicator

### 6. Sidebar — Memory/Session Context
- Show current session stats: message count, session key, duration
- Aggregate thinking stats from conversation (tool calls, iterations)

### 7. Model Selector Cleanup
- Remove decorative GPT-4 selector from input bar
- Replace with session name display / new chat button

### 8. Session Delete
- Delete button on session list items → `DELETE /api/sessions/:key`
- Optimistic removal from list

## Files Modified

- `crates/dashboard/frontend/src/lib/hooks/useAgent.ts` — add loadSession, interaction handling
- `crates/dashboard/frontend/src/app/pages/Chat.tsx` — all UI changes
- `crates/dashboard/frontend/src/lib/types.ts` — add any missing types
- `crates/dashboard/frontend/package.json` — add marked, dompurify

## Approach

Single-file enhancement following existing dashboard conventions. No component extraction.
