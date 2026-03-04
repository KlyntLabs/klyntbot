# Launcher Chat Integration Design

## Summary

Integrate the existing chat system into the Klynt Launcher as a dual-mode experience. The launcher gains a Chat mode alongside its existing Command mode, allowing users to get quick AI answers without leaving the floating window. Conversations can be expanded to the full main chat at any time.

## Approach: Dual-Mode Launcher

Single 700x600 launcher window with two modes sharing the same space. No new windows, no resizing.

### State Machine

```
                    ┌─────────────────┐
     Alt+Space ───▶ │  COMMAND MODE   │ ◀── Esc (from chat)
                    │                 │ ◀── Back button
                    │ • Search bar    │ ◀── New command selected
                    │ • Command list  │
                    │ • "Ask AI" item │
                    └────────┬────────┘
                             │
                   Select "Ask Klynt AI"
                   (or Tab from search)
                             │
                             ▼
                    ┌─────────────────┐
                    │   CHAT MODE     │
                    │                 │──── ⌘/  ──▶ Expand to main
                    │ • Message list  │──── Esc ──▶ Command mode
                    │ • Tool spinner  │              (clears session)
                    │ • Interactions  │
                    │ • Input bar     │
                    └─────────────────┘
```

### Session Lifecycle

- Entering Chat mode creates an ephemeral session key (`launcher-{timestamp}`)
- Session persists across launcher hide/show (Alt+Space toggles)
- Session cleared when: Esc from Chat mode, Back button, command selected, or expanded to main
- Messages go through the same `chat_send` IPC command and are stored in SQLite

## Chat Mode Layout

```
┌─ Launcher (Chat Mode) ─────────────────────────────────┐
│ ┌─ Header ────────────────────────────────────────────┐ │
│ │ [←] Back          Klynt AI          [⌘/ Expand]     │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌─ Messages (scrollable, ~400px) ─────────────────────┐ │
│ │                                                      │ │
│ │  You: What meetings do I have today?                 │ │
│ │                                                      │ │
│ │  ● calendar_list...        (spinner while running)   │ │
│ │                                                      │ │
│ │  Klynt: You have 3 meetings today:                   │ │
│ │    • 10am - Team standup                             │ │
│ │    • 2pm  - Design review                            │ │
│ │    • 4pm  - 1:1 with Alex                            │ │
│ │                                                      │ │
│ │  ┌─ Interaction Card ─────────────────────────────┐  │ │
│ │  │ What type of budget?                            │  │ │
│ │  │ [● Monthly] [ Annual] [ One-time] [ Rolling]   │  │ │
│ │  └────────────────────────────────────────────────┘  │ │
│ │                                                      │ │
│ └──────────────────────────────────────────────────────┘ │
│ ┌─ Input ─────────────────────────────────────────────┐ │
│ │ [✨] Follow up...                           [Send]  │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌─ Footer ────────────────────────────────────────────┐ │
│ │ [Esc] Back to commands    [⌘/] Open full chat       │ │
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### Visual Decisions

- **User messages**: Compact, left-aligned, no bubbles (space is tight)
- **Assistant messages**: Markdown rendered via existing `MarkdownContent`
- **Tool calls**: Spinner with dot + tool name while running, disappears when done
- **Interaction cards**: Full `InteractionCard` component inline
- **No transparency panel**: Lightweight feel — expand to main for full details

## Expand-to-Main Flow

```
Launcher (Chat Mode)                    Main App
─────────────────────                   ────────────────────
1. User clicks ⌘/ Expand
2. Emit Tauri event:
   'open-chat' {
     sessionKey: "launcher-1709571234"
   }
3. Hide launcher window
                                        4. Receive 'open-chat' event
                                        5. setActiveSidebar('Chat')
                                        6. Navigate to sessionKey
                                        7. Fetch messages for that session
                                        8. Full chat continues
```

Messages are already persisted in SQLite via the same `chat_send` IPC — no transfer needed.

**Edge case — streaming mid-expand**: Main chat's `useAgentStream` picks up subsequent events for the same session key. Small gap possible during handoff — acceptable for MVP.

**After expand**: Launcher clears local chat state, resets to Command mode. The thread appears in main chat's thread list.

## Component Architecture

```
Launcher.tsx (modified)
├── mode: 'command' | 'chat'
├── sessionKey: string | null
│
├── [mode === 'command']
│   ├── LauncherHeader          (existing)
│   ├── LauncherSearch          (existing)
│   ├── LauncherResults         (existing)
│   └── LauncherFooter          (existing)
│
└── [mode === 'chat']
    ├── LauncherChatHeader      (NEW — back + title + expand)
    ├── LauncherChatMessages    (NEW — compact messages + tool spinners)
    │   ├── MarkdownContent     (reused from chat/)
    │   ├── InteractionCard     (reused from chat/)
    │   └── ToolSpinner         (NEW — inline dot + name)
    ├── LauncherChatInput       (NEW — textarea + send)
    └── LauncherChatFooter      (NEW — keyboard hints)
```

### Hooks

No new hooks. Reuses:
- `useChatSession(sessionKey)` — message fetching, streaming, sending, interactions
- `useAgentStream(sessionKey)` — event listener (inside useChatSession)
- `useIpc` — for `chat_send`

### New Components (4)

| Component | Lines (est.) | Purpose |
|-----------|-------------|---------|
| `LauncherChatHeader` | ~30 | Back button, "Klynt AI" title, ⌘/ expand button |
| `LauncherChatMessages` | ~80 | Maps messages + streaming segments to compact view |
| `LauncherChatInput` | ~40 | Sparkles icon, textarea, send button |
| `ToolSpinner` | ~15 | Animated dot + tool name during execution |

`LauncherChatFooter` is just the existing `LauncherFooter` with different hint text.

## Keyboard Navigation

```
COMMAND MODE (existing)              CHAT MODE (new)
─────────────────────               ─────────────────────
↑/↓     Navigate items              Enter     Send message
Enter   Select item / Ask AI        ⌘/        Expand to main
Esc     Hide launcher               Esc       Back to Command mode
```

### Transitions

| From | Trigger | To | Side Effect |
|------|---------|-----|-------------|
| Command | Enter on "Ask Klynt AI" | Chat | Creates session, sends initial query |
| Command | Tab from search | Chat | Creates session, sends query |
| Chat | Esc | Command | Clears session |
| Chat | Back button | Command | Clears session |
| Chat | ⌘/ | Main app chat | Emits `open-chat`, hides launcher |
| Chat | Blur (click outside) | Hidden (chat preserved) | Session persists, next Alt+Space resumes |

### Focus Management

- Enter Chat mode → auto-focus chat input
- Return to Command mode → auto-focus search input
- After send → re-focus chat input
- Interaction card appears → focus moves to card (existing keyboard nav)

## Non-Goals (MVP)

- No transparency panel in launcher
- No thread list / history in launcher
- No file attachments in launcher
- No window resizing or animation between modes
- No launcher-specific agent profile
