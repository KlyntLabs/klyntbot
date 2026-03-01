# Klyntbot macOS Desktop Chat App — Design Spec

**Date**: 2026-03-01
**Stack**: Tauri v2 + Leptos (Rust WASM frontend)
**Status**: Approved

## Overview

A macOS desktop application for klyntbot with two interaction modes:

1. **Command Palette (Quick Mode)** — Global hotkey summons a floating chat bar for fast AI interactions
2. **Full Application (Split Pane)** — Full window with chat history sidebar, rich message display, and tool outputs

Both modes share the same input component and connect to klyntbot's agent backend via the `Channel` trait + `MessageBus`.

## Architecture

### Two Tauri Windows

| Window | Type | Size | Behavior |
|--------|------|------|----------|
| Command Palette | Borderless, always-on-top, transparent bg | 640x60 (expands to ~500px height) | `Cmd+K` toggle, dismiss on Escape/blur |
| Full App | Standard macOS chrome | 1200x800 default, resizable | `Cmd+N` new chat, persistent |

### Backend Integration

- Implement `Channel` trait as `MacOSChannel`
- Publish `InboundMessage` to `MessageBus`, subscribe to `OutboundMessage`
- Consume `StreamingHandle::event_rx` for real-time content chunks
- Render `InteractionRequest` (ask_user forms) inline as native UI components
- Read `~/.klyntbot/config.json` for provider/model/channel settings

## Command Palette (Quick Mode)

### Default State
```
┌──────────────────────────────────────────────────┐
│  ○ Ask Klyntbot anything...          [⌘↵ Send]  │
│  [Anthropic ▾] › [claude-opus-4-5 ▾]    [↗ Expand] │
└──────────────────────────────────────────────────┘
```

### Expanded with Response
```
┌──────────────────────────────────────────────────┐
│  You: What's on my calendar today?               │
│                                                  │
│  ◉ Checking calendar...                          │
│  ┌─ Calendar ──────────────────────────────┐     │
│  │ 10:00  Team standup                     │     │
│  │ 14:00  Design review                    │     │
│  │ 16:30  1:1 with Alex                    │     │
│  └─────────────────────────────────────────┘     │
│                                                  │
│  ○ Ask a follow-up...               [⌘↵ Send]   │
│  [Anthropic ▾] › [claude-opus-4-5 ▾]    [↗ Expand] │
└──────────────────────────────────────────────────┘
```

### Behavior
- `Cmd+K` summons/dismisses
- Streams response inline below input
- Max 3-4 exchanges visible, then scrolls
- "Expand" opens full window with complete history
- Tool calls shown as compact chips: `[✓ Created task: Fix auth bug]`
- Conversation persists in session on dismiss

## Full Application Window

### Layout
- **Sidebar** (220px): Search, chat history grouped by date, new chat button
- **Main Area**: Empty state or active conversation
- **Input Bar** (bottom): Shared component with command palette

### Empty State
- "Ask Anything" hero centered (24px semibold)
- "Type @ to use a Klyntbot tool" subtitle
- 6 tool icons: Tasks, Calendar, Goals, Memory, Web, Files — click pre-fills `@tool`
- Vision / Web Search toggles at top

### Active Chat
- **User messages**: Right-aligned, subtle accent background
- **Assistant messages**: Left-aligned, dark card background
- **Streaming**: Character-by-character with cursor blink
- **Tool calls**: Collapsible cards (tool name, params, result)
- **Plans**: Multi-step progress indicator with step status
- **Forms**: Inline buttons/selects/text inputs (from ask_user)
- **Code blocks**: Syntax-highlighted with copy button

### Sidebar
- Search filters by title/content
- Grouped: Today, Yesterday, This Week, Older
- Chat preview: auto-title + 1-line subtitle
- Active chat: accent left border highlight

### Model Selector (Two-Step)
1. Click provider pill → dropdown of configured providers (Anthropic, OpenAI, etc.)
2. Provider selection reveals model dropdown for that provider
3. Selection persists per-session, defaults from `config.agents.defaults`

## Visual Specs

### Color Palette (Dark Theme)
| Token | Hex | Usage |
|-------|-----|-------|
| `bg-primary` | `#1A1B2E` | Main background |
| `bg-sidebar` | `#151623` | Sidebar |
| `bg-card` | `#232438` | Message cards, tool results |
| `bg-input` | `#2A2B40` | Input fields |
| `bg-hover` | `#2E2F4A` | Hover states |
| `accent` | `#6C5CE7` | Active items, buttons |
| `accent-dim` | `#5A4BD1` | Hover on accent |
| `text-primary` | `#E8E8F0` | Main text |
| `text-secondary` | `#8888A0` | Subtitles, timestamps |
| `text-muted` | `#555566` | Placeholders |
| `border` | `#2E2F4A` | Dividers |
| `success` | `#2ECC71` | Completed |
| `warning` | `#F39C12` | In-progress |
| `error` | `#E74C3C` | Failed |

### Typography
- **Font**: SF Pro (system), SF Mono for code
- Hero: 24px semibold
- Sidebar: 13px medium
- Messages: 14px regular
- Code: 13px mono
- Meta: 12px secondary

### Spacing & Radius
- 8px grid system
- Sidebar padding: 12px
- Main content padding: 24px
- Message gap: 16px
- Cards: 12px radius
- Inputs/buttons: 8px radius
- Command palette: 16px radius

## Key Components

| Component | Shared | Notes |
|-----------|--------|-------|
| `InputBar` | Yes | Text input + provider/model selector + submit |
| `MessageBubble` | Full only | User/assistant with role-based styling |
| `ToolCallCard` | Both (compact in palette) | Collapsible tool name/params/result |
| `PlanProgress` | Full only | Step list with status indicators |
| `FormRenderer` | Full only | Renders ask_user interactions inline |
| `ChatSidebar` | Full only | Search + grouped history list |
| `ModelSelector` | Yes | Two-step provider → model picker |
| `ToolIcon` | Full only | Clickable tool shortcut icons |

## Figma Deliverables

1. **Command Palette** — Default state + expanded with response
2. **Full App — Empty State** — Sidebar + hero + tool icons
3. **Full App — Active Chat** — Messages + streaming + tool call cards
4. **Model Selector** — Provider dropdown + model dropdown states
