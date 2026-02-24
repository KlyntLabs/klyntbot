# Web Dashboard UX Specification

**Status:** FINAL — all questions resolved, validated against BA + architect + team lead
**Owner:** UX Designer
**Validated against:** Task #3 BA acceptance criteria (AC-15.x, AC-16.x, AC-17.x)
**Aligned with:** Task #4 Architecture spec (WebSocket events, REST shapes, AppState)
**Informs:** Tasks 15-17 (Frontend Foundation), Tasks 18-27+ (Frontend Pages)

---

## 1. User Flows

### 1.1 Chat: Message Send -> Streaming -> Done

```
User types message in input bar
  -> User presses Enter or clicks Send
  -> Input clears, user message appears in message list
  -> Input bar disabled (shows "Cancel" button)
  -> Thinking indicator appears below user message:
     Phase 1: "Classifying..." (pulsing dot)
       <- classificationComplete event
       -> Badge appears: strategy name + confidence %
     Phase 2: "Building context..." (pulsing dot)
       <- contextAssembled event
       -> Token budget indicator briefly shown
     Phase 3: "Thinking..." with engine badge
       <- executionStarted event
       -> Shows engine name (e.g., "ToolAssisted")
       <- iterationStart events update iteration counter "Step 1/5"
       <- toolStart events: collapsible tool cards appear inline
       <- toolEnd events: tool cards update with duration + success/fail
       <- contentChunk events: text streams character-by-character below tool cards
     Phase 4: Done
       <- done event
       -> Final assistant message replaces streaming content
       -> Thinking indicator disappears
       -> Input bar re-enables
       -> Auto-scroll to bottom
```

**Cancel flow:**
```
User clicks Cancel during streaming
  -> chat.cancel sent via WebSocket
  -> Streaming stops (partial content preserved)
  -> Input bar re-enables
  -> System message: "Response cancelled"
```

**Error flow:**
```
<- error event during streaming
  -> Thinking indicator disappears
  -> Error message appears (red tint, system role)
  -> Input bar re-enables
  -> User can retry
```

**Interaction form flow (ask_user):** (BA-confirmed question types + response protocol)
```
During agent processing:
  <- interaction.request via WebSocket:
     { type: "interaction.request", requestId: "uuid", title: "...", questions: [...] }
     Each question has typed answerType:
  -> Form renders inline in message list:
     - Title text (from request)
     - 1-4 questions rendered by answerType:
       - singleSelect { options } -> radio group or dropdown
       - multiSelect { options } -> checkbox group
       - yesNo { default } -> toggle or Yes/No buttons
       - freeText { placeholder } -> text input
     - Submit button (primary), Cancel button (ghost)
  -> User fills form, clicks Submit
  -> interaction.respond sent via WebSocket:
     { type: "interaction.respond", requestId: "uuid", response: { "Completed": [answers] } }
     (requestId MUST match — backend has a pending oneshot::Sender waiting)
  -> Form collapses to summary (shows what was selected)
  -> Agent continues processing

Cancel flow:
  -> User clicks Cancel
  -> interaction.respond sent with: { response: { "Cancelled": null } }
  -> Agent handles cancellation (may ask again or proceed without)
```

### 1.2 Task CRUD

**Create task:**
```
User clicks "+" button in task list header
  -> Dialog opens (Radix Dialog):
     - Title (required, autofocus)
     - Description (optional, textarea)
     - Priority (optional, Select: Urgent/Important/Normal/Low)
     - Due date (optional, react-day-picker popover)
     - Tags (optional, comma-separated input or tag picker)
     - Project (optional, Select from existing projects)
  -> User fills fields, clicks "Create"
  -> POST /api/tasks -> optimistic add to list
  -> Dialog closes
  -> Sonner toast: "Task created"
  -> New task highlighted briefly in list (green flash)
```

**Edit task (inline):**
```
User clicks task row in list
  -> Navigates to /tasks/:id (TaskDetail page)
  -> Two-column layout loads:
     Left: editable title (contenteditable h1), description (textarea)
     Right: property panel (status, priority, due date, project, tags)
  -> User edits a field
  -> On blur/change: PATCH /api/tasks/:id with changed field
  -> Sonner toast: "Task updated"
```

**Toggle status:**
```
User clicks status icon on task row (checkbox area)
  -> Cycles: todo -> doing -> done
  -> PATCH /api/tasks/:id { status: "next_status" }
  -> Optimistic update: icon changes immediately
  -> If done: row gets strikethrough + fades slightly
  -> Sonner toast: "Task marked as done"
```

**Delete task:**
```
User clicks "..." menu on task row -> "Delete"
  -> Alert Dialog: "Delete task? This cannot be undone."
  -> User confirms
  -> DELETE /api/tasks/:id
  -> Optimistic remove from list
  -> Sonner toast: "Task deleted" with Undo action (5s window)
  -> Undo: POST /api/tasks with original data (re-create)
```

**Filter tasks:** (confirmed query param pattern: matches TodoFilter fields)
```
Task list header shows filter controls:
  - Status tabs: All | Todo | Doing | Done  -> ?status=todo
  - Search input: debounced (300ms)          -> ?q=search_term
  - Priority filter: dropdown (All | 1-5)   -> ?priorityMin=1
  - Project filter: dropdown (All | names)   -> ?projectId=xxx
  - Tags filter: multi-select               -> ?tags=backend,security
  - Limit: implicit (default 100)            -> ?limit=50
  - Sort: dropdown (Created | Due | Priority | Updated)
URL updates with query params for shareable filter state.
API: GET /api/tasks?status=todo&priorityMin=1&tags=backend,security&projectId=xxx&limit=50
```

### 1.3 Settings Save (BA-confirmed read-only fields + restart requirement)

```
User navigates to /settings
  -> GET /api/settings -> loads all sections (secrets show as "••••••")
  -> Left sidebar: config sections (scrollable list)
  -> Right panel: selected section's fields
  -> READ-ONLY fields:
     - dataDir: display-only (requires restart + data migration)
  -> "RESTART REQUIRED" badge fields (editable, but changes need restart):
     - Model changes (agents.defaults.model)
     - Channel configs (channels.*)
     - Provider configs (providers.*)
  -> SECRET fields: show "••••••" (need dedicated "Update" input, not inline edit)
  -> COMPUTED fields (not directly editable):
     - packs.enabledSkills: managed via Skills page toggles
  -> User edits an editable non-secret field
  -> On change: field marked as "unsaved" (subtle dot indicator)
  -> User clicks "Save" button at bottom of section
  -> PATCH /api/settings/:section with changed fields only
  -> Success: Sonner toast "Settings saved"
  -> Warning toast: "Changes require restart to take effect" (config hot-reload not supported)
  -> Error: inline error below field with validation message

Secret editing:
  -> User clicks "Update" button next to secret field
  -> Dedicated input appears (empty, not showing existing value)
  -> User types new value, clicks Save
  -> PATCH includes new value
  -> Input hides, field returns to "••••••" display
  -> Sonner toast: "API key updated"
```

### 1.4 WebSocket Reconnection (BA-confirmed)

```
WebSocket disconnects (server restart, network issue):
  -> Status bar dot turns yellow: "Reconnecting..."
  -> AgentSocket auto-reconnects after 2s (built-in)
  -> During disconnect: chat input disabled, REST pages still functional
  -> On reconnect: status bar returns to green
  -> If mid-streaming: stream is LOST
     -> Show sonner toast: "Connection lost — message may be incomplete"
     -> On reconnect, session history is still available via REST:
        GET /api/sessions/:key (full message history)
  -> No automatic retry of interrupted messages (user must resend)

Blocking behavior (AC-17.3 edge case):
  -> If user calls sendMessage while already streaming:
     -> Block the send (input remains disabled during streaming)
     -> Cancel button is the only way to stop current stream
```

---

## 2. Component Hierarchy: Figma Make Inventory

### 2.1 Components We USE As-Is (from shadcn/ui)

These 35+ shadcn/ui primitives are production-ready and used directly:

| Component | Used In | Notes |
|-----------|---------|-------|
| `button.tsx` | Everywhere | Primary/secondary/ghost/destructive variants |
| `input.tsx` | Forms, search bars | Standard text input |
| `textarea.tsx` | Chat input, task description | Auto-resize variant needed |
| `label.tsx` | All forms | Accessible form labels |
| `badge.tsx` | Tags, status indicators, strategy badges | Color variants per status |
| `card.tsx` | Tool call cards, summary cards | In chat and dashboard |
| `dialog.tsx` | Create task, delete confirm, settings modals | Radix Dialog |
| `alert-dialog.tsx` | Destructive confirmations | Delete task, abandon plan |
| `dropdown-menu.tsx` | Task row "..." menu, sort options | Radix DropdownMenu |
| `select.tsx` | Priority, status, project dropdowns | Radix Select |
| `popover.tsx` | Date picker container, color picker | Radix Popover |
| `tooltip.tsx` | Nav rail icons, toolbar buttons | Radix Tooltip |
| `tabs.tsx` | Finance sub-pages, status filters | Radix Tabs |
| `table.tsx` | Transaction list, cron job list | Sortable columns |
| `checkbox.tsx` | Multi-select forms, task selection | Radix Checkbox |
| `switch.tsx` | Toggle settings (enabled/disabled) | Boolean config fields |
| `separator.tsx` | Section dividers | Visual separation |
| `scroll-area.tsx` | Message list, task list, settings | Radix ScrollArea |
| `skeleton.tsx` | Loading states for all data pages | Consistent loading UI |
| `progress.tsx` | Plan progress, budget usage | Determinate progress bars |
| `slider.tsx` | Confidence threshold, alert threshold | Numeric range inputs |
| `radio-group.tsx` | SingleSelect in interaction forms | Radix RadioGroup |
| `form.tsx` | react-hook-form integration | Form field wrapper |
| `collapsible.tsx` | Tool call details, settings sections | Expand/collapse |
| `sonner.tsx` | Toast notifications | Success/error/info toasts |
| `drawer.tsx` (vaul) | Mobile-style bottom sheets (future) | May use for task quick-edit |
| `command.tsx` (cmdk) | Command palette (Cmd+K) | Quick navigation + actions |
| `toggle.tsx` | Focus mode toggle | Single toggle button |
| `toggle-group.tsx` | View mode switch (list/grid) | Radio-style group |
| `avatar.tsx` | Chat message avatars | User vs Agent indicators |
| `hover-card.tsx` | Task preview on hover | Rich task tooltip |
| `sheet.tsx` | Side panels | Task detail slide-over |
| `calendar.tsx` | Date picker, calendar page | react-day-picker based |
| `chart.tsx` | Finance charts | Recharts wrapper |
| `accordion.tsx` | Settings sections, FAQ | Expandable sections |

### 2.2 Components We ADAPT

| Component | Adaptation Needed | Reason |
|-----------|-------------------|--------|
| `sidebar.tsx` | **Slim down significantly** | Figma Make has full sidebar with user profile, search, etc. We only need 48px nav rail with icon buttons. Use as reference for structure but rewrite to match design doc layout. |
| `navigation-menu.tsx` | **Skip in favor of NavLink rail** | Our nav is a simple icon rail, not a full navigation menu. Use `NavLink` from React Router directly. |
| `resizable.tsx` | **Use for chat + task detail** | Resizable panels between message list and sidebar. Adapt panel sizes. |
| `pagination.tsx` | **Add if API supports pagination** | Pending architect decision on envelope format `{ data, total, page }`. |

### 2.3 Components We SKIP

| Component | Reason |
|-----------|--------|
| `breadcrumb.tsx` | No deep navigation hierarchy — nav rail + pages is flat |
| `carousel.tsx` | No carousel content in dashboard |
| `context-menu.tsx` | No right-click menus planned |
| `input-otp.tsx` | No OTP/verification flows |
| `menubar.tsx` | No top menu bar (web app, not desktop) |
| `aspect-ratio.tsx` | No media content requiring aspect ratio |
| `use-mobile.ts` | Desktop-only — no mobile detection needed |

### 2.4 Components We ADD (not in Figma Make)

| Component | Purpose | Implementation |
|-----------|---------|----------------|
| `ThinkingIndicator` | Shows agent processing phases | Custom: animated dots + phase label + strategy badge |
| `ToolCallCard` | Inline tool execution display | Custom: collapsible card with name, args (JSON tree), duration, status icon |
| `InteractionForm` | Dynamic form from ask_user | Custom: renders questions by type (SingleSelect, MultiSelect, YesNo, FreeText) |
| `StreamingText` | Character-by-character text display | Custom: accumulates contentChunk events, renders with cursor |
| `StatusBar` | Bottom bar with connection status | Custom: model name, session key, token cost, WebSocket status |
| `MoneyDisplay` | Formats i64 cents to currency | Custom: respects currency field, locale formatting |
| `RelativeTime` | "2 hours ago", "in 3 days" | Custom or use `date-fns/formatDistanceToNow` |
| `PriorityIcon` | Color-coded priority indicator | Custom: maps 1-4 to colors (red/orange/default/blue) |
| `EmptyState` | No-data placeholders per page | Custom: icon + message + optional action button |
| `ErrorBoundary` | React error boundary per page | Custom: catches render errors, shows retry option |
| `CommandPalette` | Cmd+K quick actions | Built on `command.tsx` (cmdk): navigate pages, create task, search |
| `FIRECalculator` | Financial independence calculator | Custom: compound interest formula with user-overridable inputs (7% return, 3% inflation defaults). Monthly compounding. Client-side only. |

---

## 3. Interaction States

### 3.1 State Matrix by Page Area

Each page area has 5 possible states: **loading**, **empty**, **populated**, **error**, **streaming** (chat only).

#### Chat Page (`/`)

| Area | Loading | Empty | Populated | Error | Streaming |
|------|---------|-------|-----------|-------|-----------|
| Message list | Skeleton: 3 message bubbles | "Start a conversation" + suggestion cards (3-4 prompts) | Messages rendered by role | "Failed to load session" + retry | Content chunks accumulating |
| Input bar | Disabled, placeholder: "Connecting..." | Enabled, placeholder: "Message klyntbot..." | Enabled | Disabled if WebSocket down | Shows "Cancel" button, input disabled |
| Thinking indicator | N/A | N/A | N/A | N/A | Phase dots + labels + badges |
| Tool cards | N/A | N/A | N/A | N/A | Appear/update during execution |
| Session selector | Skeleton: 3 rows | "No sessions yet" | Session list with timestamps | "Failed to load sessions" | Unchanged |

#### Tasks Page (`/tasks`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Summary bar | Skeleton: 3 stat cards | "0 / 0 / 0" counters | "5 todo / 3 doing / 12 done" | Hidden |
| Task list | Skeleton: 8 task rows | "No tasks yet" + "Create your first task" button | Task rows with status/priority/due date | "Failed to load tasks" + retry |
| Filter panel | Disabled | Enabled (no visual change) | Enabled, active filters highlighted | Disabled |
| Create dialog | N/A | N/A | Form with validation | Form error messages inline |

#### Task Detail (`/tasks/:id`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Title | Skeleton: 1 line | N/A (always has title) | Editable heading | "Task not found" (404) |
| Description | Skeleton: 3 lines | "Add a description..." placeholder | Editable textarea | Inline save error |
| Properties panel | Skeleton: 6 fields | Default values shown | Current values, editable | Inline save error per field |
| Subtask list | Skeleton: 3 rows | "No subtasks" + "Add subtask" button | Subtask list with checkboxes | "Failed to load subtasks" |
| Time tracking | Skeleton | "No time tracked" + Start button | Timer display + entry list | "Failed to save time entry" |

#### Plans Page (`/plans`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Plan list | Skeleton: 4 plan cards | "No plans yet" + "Create Plan" button (navigates to Chat with prefilled prompt) | Plan cards with status, step progress bar + "Create Plan" button in header | "Failed to load plans" + retry |
| Plan detail (expanded) | Skeleton: step list | N/A | Steps with status icons, current step highlighted | Step error shown inline |
| Live progress | N/A | N/A | WebSocket events (`planStepCompleted`, `planCompleted`) update step status in real-time — same WS connection as chat, no separate subscription | Shows last known state |

**"Create Plan" button behavior:** Navigates to `/` (Chat) with query param `?prompt=Create+a+plan+for:+` — Chat page reads this, prefills the input with `"Create a plan for: "` and auto-focuses cursor at the end. Plans always go through the agent, never direct REST.

#### Calendar Page (`/calendar`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Calendar grid | Skeleton: month grid | "No events" + "Sync your calendar" button | Events rendered on date cells | "Failed to load events" + retry |
| Event detail popover | N/A | N/A | Event info on click/hover | N/A |
| Sync button | Disabled during load | Enabled | Enabled | Shows last sync error |

#### Cron Page (`/cron`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Cron list | Skeleton: 4 rows | "No scheduled jobs" — created via chat | Table: name, schedule, next run, status | "Failed to load jobs" + retry |
| Job detail | N/A | N/A | Collapsible: payload, last error, history | Inline error |

#### Skills Page (`/skills`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Skill list | Skeleton: 6 cards | "No skills available" (shouldn't happen) | Card grid: name, description, source badge, toggle | "Failed to load skills" + retry |
| Toggle action | Disabled | N/A | Switch with optimistic toggle | Revert toggle + toast error |

#### Finance Page (`/finance`) — 6 Tabs

| Tab | Loading | Empty | Populated | Error |
|-----|---------|-------|-----------|-------|
| Overview | Skeleton: 4 stat cards + chart | "Set up your first account" | Net worth, income/expense chart, recent txns | "Failed to load overview" |
| Accounts | Skeleton: 3 cards | "No accounts" + create button | Account cards with balances | "Failed to load accounts" |
| Transactions | Skeleton: 8 table rows | "No transactions" + add button | Sortable table, category filters | "Failed to load transactions" |
| Budgets | Skeleton: 4 progress bars | "No budgets" + create button | Budget cards with usage progress | "Failed to load budgets" |
| Investments | Skeleton: portfolio cards | "No portfolios" + create button | Portfolio summary + holdings table | "Failed to load investments" |
| Goals | Skeleton: 3 goal cards | "No goals" + create button | Goal cards with progress, deadline | "Failed to load goals" |

#### Settings Page (`/settings`)

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Section list | Skeleton: 14 rows | N/A (always shows sections) | Section names with icons | "Failed to load settings" |
| Section content | Skeleton: form fields | N/A | Form fields with current values | Inline validation errors |
| Save action | Disabled | N/A | "Save" button (disabled until changed) | "Failed to save" + retry |

#### Setup Page (`/setup`) — First-Run Wizard

| Area | Loading | Empty | Populated | Error |
|------|---------|-------|-----------|-------|
| Step indicator | N/A | Step 1 active | Current step highlighted | Step with error indicator |
| Step content | N/A | Wizard form for current step | Filled form | Validation errors inline |
| Navigation | N/A | "Next" enabled when valid | Back/Next/Finish buttons | "Next" disabled on error |

### 3.2 Skeleton Patterns

Use consistent `Skeleton` component from shadcn/ui:

```
Text line:    <Skeleton className="h-4 w-[250px]" />
Card:         <Skeleton className="h-24 w-full rounded-lg" />
Avatar:       <Skeleton className="h-8 w-8 rounded-full" />
Table row:    <Skeleton className="h-12 w-full" /> (repeated)
Stat number:  <Skeleton className="h-8 w-16" />
```

Loading states appear for max 5 seconds. If data hasn't loaded by then, show error state with retry.

### 3.3 Empty State Patterns

Each empty state includes:
- Relevant Lucide icon (muted, 48px)
- Short message ("No tasks yet")
- Optional action button ("Create your first task") or context message ("Plans are created via chat")
- Consistent `text-codex-text-tertiary` color

### 3.4 Error State Patterns

Two error tiers:
1. **Page-level error**: Full page replacement with icon + message + "Retry" button
2. **Inline error**: Below the failed component, with red tint + message + retry link

All errors include:
- Error icon (AlertTriangle from Lucide)
- Human-readable message (not raw error codes)
- Retry action where applicable
- Console logging of full error details for debugging

---

## 4. Accessibility Requirements

### 4.1 Keyboard Navigation

| Context | Keys | Behavior |
|---------|------|----------|
| **Global** | `Cmd+K` | Open command palette |
| **Nav rail** | `Tab` / `Shift+Tab` | Move between nav items |
| **Nav rail** | `Enter` / `Space` | Activate nav link |
| **Chat input** | `Enter` | Send message |
| **Chat input** | `Shift+Enter` | New line |
| **Chat input** | `Escape` | Cancel streaming (when active) |
| **Task list** | `Arrow Up/Down` | Move between task rows |
| **Task list** | `Enter` | Open task detail |
| **Task list** | `Space` | Toggle task status |
| **Task list** | `n` | Create new task (opens dialog) |
| **Dialog** | `Escape` | Close dialog |
| **Dialog** | `Tab` | Move between form fields |
| **Dialog** | `Enter` | Submit form (when on submit button) |
| **Command palette** | `Arrow Up/Down` | Navigate options |
| **Command palette** | `Enter` | Select option |
| **Command palette** | `Escape` | Close palette |
| **Settings** | `Cmd+S` | Save current section |

### 4.2 ARIA Attributes

| Element | ARIA | Purpose |
|---------|------|---------|
| Nav rail | `role="navigation"`, `aria-label="Main navigation"` | Screen reader landmark |
| Nav item (active) | `aria-current="page"` | Indicates current page |
| Chat message list | `role="log"`, `aria-live="polite"`, `aria-label="Chat messages"` | Live region for new messages |
| Streaming content | `aria-busy="true"` during streaming | Indicates content is updating |
| Thinking indicator | `role="status"`, `aria-live="polite"` | Announces phase changes |
| Task list | `role="list"` | Semantic list |
| Task row status | `aria-label="Status: todo"` | Describes current state |
| Priority indicator | `aria-label="Priority: urgent"` | Describes priority level |
| Tool call card | `aria-expanded="true/false"` | Collapsible state |
| Dialog | Handled by Radix Dialog | Focus trap, ESC close |
| Tooltip | Handled by Radix Tooltip | Trigger + content association |
| Toast (sonner) | `role="alert"`, `aria-live="assertive"` | Announces notifications |
| Form fields | `aria-invalid="true"` on error, `aria-describedby` for error text | Form validation feedback |
| Loading skeleton | `aria-hidden="true"` | Hide decorative loading |
| Empty state | `role="status"` | Announces empty state |
| Error state | `role="alert"` | Announces error |

### 4.3 Focus Management

| Transition | Focus Behavior |
|------------|----------------|
| Page navigation | Focus moves to main content heading (h1) |
| Dialog opens | Focus moves to first focusable element (autofocus field) |
| Dialog closes | Focus returns to trigger element |
| Toast appears | No focus change (announced via aria-live) |
| Task created | Focus moves to new task in list |
| Task deleted | Focus moves to next task in list (or previous if last) |
| Error occurs | Focus moves to error message retry button |
| Chat message sent | Focus returns to input (re-enabled after done) |
| Interaction form appears | Focus moves to first question input |
| Command palette opens | Focus moves to search input |

### 4.4 Color Contrast

All text meets WCAG AA contrast requirements against `#0d0d0d` background:
- Primary text (`#e5e5e5`): 14.7:1 ratio (AAA)
- Secondary text (`#999999`): 7.3:1 ratio (AAA)
- Tertiary text (`#666666`): 4.2:1 ratio (AA)
- Accent (`#10a37f`): 5.8:1 ratio (AA)
- Danger (`#ef4444`): 5.2:1 ratio (AA)
- Warning (`#f59e0b`): 8.6:1 ratio (AAA)

Interactive elements have visible focus indicators: `ring-2 ring-codex-accent ring-offset-2 ring-offset-codex-bg`.

### 4.5 Reduced Motion

Respect `prefers-reduced-motion`:
- Disable all Motion (framer-motion) animations
- Disable thinking indicator pulse animation
- Disable streaming text cursor blink
- Keep functional transitions (dialog open/close) but make them instant
- Apply via Tailwind: `motion-safe:animate-pulse`, `motion-reduce:animate-none`

---

## 5. Adaptations from Figma Make Source

### 5.1 Layout Adaptations

| Figma Make | Adaptation | Reason |
|------------|------------|--------|
| macOS traffic lights (close/minimize/maximize) | **Remove entirely** | Web mode, not desktop. Replace with simple top area that's part of the nav rail or remove title bar. |
| Full sidebar with user profile, search, notifications | **Replace with 48px icon nav rail** | Design doc specifies narrow rail. Sidebar has too much chrome for a personal tool. |
| Window drag region | **Remove** | Not applicable in browser |
| `ImageWithFallback.tsx` | **Keep but adapt** | Used for avatars and icons; keep fallback logic |

### 5.2 Data Adaptations

| Figma Make Pattern | Real API Adaptation | Implementation |
|-------------------|---------------------|----------------|
| Hardcoded mock messages in Chat.tsx | WebSocket streaming via `useAgent` hook | Replace static array with WebSocket event accumulation |
| Static task list in Tasks.tsx | REST `GET /api/tasks` via `useApi` hook | Replace hardcoded data with API fetch + loading/error states |
| Mock plan data | REST `GET /api/plans` + WebSocket plan events | Replace with dual data source (REST for list, WS for live updates) |
| Static calendar events | REST `GET /api/calendar/events` | Replace with API fetch, add sync trigger button |
| Mock cron jobs | REST `GET /api/cron` | Replace with API fetch |
| Static skill cards | REST `GET /api/skills` | Replace with API fetch + toggle action |
| Mock finance data | REST finance endpoints (6 sub-resources) | Replace all mock data across 6 tabs |
| Static settings forms | REST `GET/PATCH /api/settings/:section` | Replace with live config + save flow |
| Task `template` badge | **Remove** | `is_template` exists in DB but not exposed in API scope |
| Task `attachment` count | **Keep** | `TodoAttachmentRow` exists; add attachment count to task list |
| Task subtask progress | **Compute client-side** | Count subtasks from list response, show "3/5 done" |
| Time tracking timer | **Client-side timer** | Start/stop timer in browser, PATCH `total_tracked_secs` on stop |
| Finance FIRE calculator | **Client-side only** | Uses investment + goal data from API, computes locally |

### 5.3 Type Mapping: Rust -> TypeScript

All monetary values (`i64` in Rust) are integer cents. Frontend must:
1. Receive as `number` (JSON integer)
2. Format for display: `(amount / 100).toLocaleString('en-US', { style: 'currency', currency: currencyCode })`
3. Send back as integer cents (multiply user input by 100)

Date mapping:
- `DateTime<Utc>` -> ISO 8601 string (`"2026-02-24T10:30:00Z"`) -> `new Date(str)`
- `NaiveDate` -> date string (`"2026-02-24"`) -> displayed as-is (no timezone conversion)
- `Option<T>` -> `T | null` in JSON

### 5.4 Field Adaptations per Entity

**TodoRow -> Task TypeScript interface:**
- `tags: Vec<String>` -> `tags: string[]` (JSON array passes through)
- `recurrence_rule` -> advanced field, hidden in basic view, shown in task detail
- `calendar_event_uid` -> show "Synced to calendar" badge if present
- `is_template`, `next_instance_date` -> hidden (not in dashboard scope)
- `focus_expired_count` -> internal metric, not displayed to user

**CronJobRow -> CronJob TypeScript interface:**
- `schedule: serde_json::Value` -> display parsed schedule string (e.g., "Every day at 9am")
- `payload: serde_json::Value` -> display as collapsible JSON viewer
- `next_run_at_ms`, `last_run_at_ms` -> convert epoch ms to Date, display relative time
- `delete_after_run` -> show "One-shot" badge

**Finance amounts -> display formatting:**
- Always show 2 decimal places for display
- Use `Intl.NumberFormat` with currency code
- Negative amounts show in red
- Budget usage: show progress bar (spent/amount) with color coding (green < 80%, yellow 80-100%, red > 100%)

---

## 6. Form Validation Strategy

### 6.1 Library: react-hook-form + zod

Use `react-hook-form` for all forms with `zod` schemas for validation:

```typescript
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
```

### 6.2 Validation Rules by Form

**Create Task:** (BA-confirmed: only `title` required, all others optional)
```typescript
const createTaskSchema = z.object({
  title: z.string().min(1, "Title is required").max(500, "Title too long"),
  description: z.string().max(10000).optional().nullable(),
  priority: z.number().int().min(1).max(5).optional().nullable(), // BA spec: 1-5 range
  dueDate: z.string().datetime().optional().nullable(),
  tags: z.array(z.string().max(50)).max(20).optional().nullable(),
  projectId: z.string().optional().nullable(),
  parentId: z.string().optional().nullable(),
  estimatedMinutes: z.number().int().positive().optional().nullable(),
});
```

**Chat Input:** (No backend limit; LLM context window is practical constraint)
```typescript
const chatInputSchema = z.object({
  message: z.string().min(1, "Message cannot be empty"),
  sessionKey: z.string().optional(),
});
// Show character counter when > 5,000 chars (subtle, codex-text-tertiary)
// Warn at 10K (counter turns codex-warning)
// No hard block — trust the user
```

**Create Finance Account:**
```typescript
const createAccountSchema = z.object({
  name: z.string().min(1).max(200),
  accountType: z.enum(["checking", "savings", "credit", "investment"]),
  currency: z.string().length(3), // ISO 4217
  balance: z.number().int(), // cents
  institution: z.string().max(200).optional(),
  notes: z.string().max(5000).optional(),
});
```

**Create Finance Transaction:**
```typescript
const createTransactionSchema = z.object({
  accountId: z.string().min(1, "Account is required"),
  txType: z.enum(["income", "expense", "transfer"]),
  amount: z.number().int().positive("Amount must be positive"), // cents
  currency: z.string().length(3),
  category: z.string().max(100).optional(),
  subcategory: z.string().max(100).optional(),
  counterparty: z.string().max(200).optional(),
  notes: z.string().max(5000).optional(),
  txDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/, "Invalid date format"),
});
```

**Settings (per section):**
- No client-side schema — server validates on PATCH
- Client shows server-returned validation errors inline
- Secret fields: only validate non-empty when user explicitly edits

### 6.3 Validation UX Patterns

| Pattern | Behavior |
|---------|----------|
| **When to validate** | On blur for each field + on submit for all fields |
| **Error display** | Red text below field, field border turns red (`border-codex-danger`) |
| **Error clearing** | Error clears on next valid input (not on focus) |
| **Submit button** | Disabled while form has errors; shows loading spinner during API call |
| **Server errors** | Map to specific field if possible (e.g., `{ field: "title", message: "already exists" }`), otherwise show as toast |
| **Optimistic updates** | Apply immediately, revert on server error with toast explanation |

### 6.4 Form Integration with shadcn/ui

Use the shadcn/ui `Form` component which wraps react-hook-form:

```tsx
<Form {...form}>
  <FormField
    control={form.control}
    name="title"
    render={({ field }) => (
      <FormItem>
        <FormLabel>Title</FormLabel>
        <FormControl>
          <Input {...field} />
        </FormControl>
        <FormMessage /> {/* Auto-shows zod error */}
      </FormItem>
    )}
  />
</Form>
```

---

## 7. Command Palette (Cmd+K)

Built on shadcn/ui `command.tsx` (cmdk):

### 7.1 Actions

| Category | Actions |
|----------|---------|
| **Navigate** | Go to Chat, Tasks, Plans, Calendar, Cron, Skills, Finance, Settings |
| **Create** | New Task, New Note (chat) |
| **Search** | Search tasks (keyword), Search tasks (semantic) |
| **Quick actions** | Sync calendar, Toggle focus mode, Clear session |

### 7.2 Behavior

- Opens on `Cmd+K` (global keybinding)
- Search input at top, auto-focused
- Results grouped by category
- Arrow keys to navigate, Enter to select
- Escape to close
- Recent items shown before search results
- Fuzzy matching on action names

---

## 8. Status Bar

Fixed at bottom of viewport, spans full width below nav rail:

```
[WebSocket status dot] [Model name] | [Session key] | [Token cost: $0.042] | [Uptime: 2h 15m]
```

| Element | Source | Update Frequency |
|---------|--------|-----------------|
| WebSocket status | Connection state | Real-time |
| Model name | `GET /api/status` | On page load |
| Session key | Current chat session | On session change |
| Token cost | `done` event metadata or `GET /api/status` | Per message |
| Uptime | `GET /api/status` | Every 60s |

Status dot colors:
- Green: WebSocket connected
- Yellow: Reconnecting
- Red: Disconnected (after max retries)
- Gray: No active session

---

## 9. Figma Make Source File Usage Summary

### Files to copy and adapt:
- `src/app/App.tsx` -> Adapt routing setup
- `src/app/routes.tsx` -> Match our 10 routes
- `src/app/components/Layout.tsx` -> Major rewrite (remove traffic lights, slim sidebar)
- `src/app/pages/*.tsx` (all 10) -> Replace mock data with API hooks
- `src/styles/theme.css` -> Adapt to our Codex dark theme tokens
- `src/styles/fonts.css` -> Keep (Inter + JetBrains Mono)
- `src/app/components/ui/*.tsx` (all 46 components) -> Use as-is (standard shadcn/ui)

### Files to skip:
- `src/app/components/ui/use-mobile.ts` -> Desktop only
- `src/app/components/figma/ImageWithFallback.tsx` -> Evaluate if needed
- `guidelines/Guidelines.md` -> Reference only
- `ATTRIBUTIONS.md` -> Include in build output

### Files to create (not in Figma Make):
- `src/lib/api.ts` -> REST client
- `src/lib/ws.ts` -> WebSocket client
- `src/lib/hooks/useAgent.ts` -> Chat streaming hook
- `src/lib/hooks/useApi.ts` -> REST data fetching hook
- `src/lib/types.ts` -> TypeScript interfaces
- `src/app/components/ThinkingIndicator.tsx` -> Agent processing phases
- `src/app/components/ToolCallCard.tsx` -> Inline tool display
- `src/app/components/InteractionForm.tsx` -> ask_user form renderer
- `src/app/components/StreamingText.tsx` -> Chunk accumulator
- `src/app/components/StatusBar.tsx` -> Bottom status bar
- `src/app/components/EmptyState.tsx` -> Reusable empty state
- `src/app/components/CommandPalette.tsx` -> Cmd+K quick actions

---

## 10. BA Acceptance Criteria Validation

### Task 15: Scaffold React + Vite + Tailwind

| AC | Criteria | UX Impact | Covered |
|----|----------|-----------|---------|
| AC-15.1 | Project structure exists | N/A (dev tooling) | N/A |
| AC-15.2 | Full dependency set | All components in Section 2 depend on these deps | Yes — component inventory assumes full dep set |
| AC-15.3 | Vite proxy to :18790 | Transparent to UX; API calls work via relative paths | Yes — api.ts uses `''` base |
| AC-15.4 | Codex dark theme vars | All color references in Section 4.4 use these vars | Yes — contrast ratios verified |
| AC-15.5 | TypeScript strict mode | Type safety for all interfaces in Section 5.3 | Yes — zod schemas enforce runtime types too |
| AC-15.6 | .gitignore | N/A (dev tooling) | N/A |

### Task 16: Layout Shell + Routing

| AC | Criteria | UX Impact | Covered |
|----|----------|-----------|---------|
| AC-16.1 | 48px nav rail, 7 items + settings | Section 2.2: sidebar.tsx adapted to 48px rail | Yes |
| AC-16.2 | Active state = accent color | Section 4.1: keyboard nav, NavLink isActive styling | Yes |
| AC-16.3 | 10 routes | Section 3.1: state matrix covers all 10 pages | Yes |
| AC-16.4 | Placeholder pages | Section 3.1: empty states defined for each | Yes |
| AC-16.5 | Setup outside layout | Section 3.1: Setup page has full-screen states | Yes |
| Edge | Unknown route → index | Not explicitly in UX spec — **add: 404 fallback redirects to `/`** | Added below |
| Edge | Browser refresh → SPA routing | Vite handles; no UX impact | N/A |

**Addition — 404 handling:** Unknown routes redirect to `/` (Chat). No dedicated 404 page (over-engineering for a personal tool).

### Task 17: API Client + WebSocket Hook

| AC | Criteria | UX Impact | Covered |
|----|----------|-----------|---------|
| AC-17.1 | `apiFetch<T>` with ApiError | Section 3: all error states use `error` from useApi | Yes |
| AC-17.2 | AgentSocket, auto-reconnect 2s | Section 1.4: reconnection flow, Section 8: status bar dot | Yes |
| AC-17.3 | useAgent: messages, thinking, isStreaming, sendMessage, cancel | Section 1.1: full streaming flow maps to these states | Yes |
| AC-17.4 | useApi: data, loading, error, refetch | Section 3: loading/error states per page use these | Yes |
| AC-17.5 | TypeScript types matching camelCase JSON | Section 5.3: type mapping Rust → TS, Section 6.2: zod schemas | Yes |
| AC-17.6 | Event type → thinking phase mapping | Section 1.1: each phase mapped to specific events | Yes |
| Edge | Disconnect during stream | Section 1.4: sonner toast, session recoverable via REST | Yes |
| Edge | sendMessage while streaming | Section 1.4: blocked (input disabled during streaming) | Yes |
| Edge | Component unmount | Not in UX spec — **implementation note: AbortController + disconnect() cleanup** | Implementation concern |

---

## All Questions Resolved

All 10 open questions have been resolved by BA spec + team lead decisions + architect alignment.

| # | Question | Resolution | Source |
|---|----------|------------|--------|
| 1 | **WebSocket event type naming** | **camelCase** — `{ "type": "camelCaseVariant", ...fields }`. Matches `#[serde(tag = "type", rename_all = "camelCase")]` on AgentEvent. Design doc dot-separated format is outdated. | Team lead + Architect |
| 2 | **REST pagination** | **Bare `T[]`** — no pagination wrapper. Optional `?limit=N` param (default 100). Cursor-based pagination deferred. | Team lead |
| 3 | **Task filters as query params** | **Confirmed** — `GET /api/tasks?status=todo&priorityMin=1&tags=bug,urgent&projectId=abc&limit=50`. All camelCase. All optional. | Team lead + Architect |
| 4 | **Content chunk accumulation** | **Both** — frontend accumulates `contentChunk` events for streaming display. `done` event includes full accumulated content as authoritative final text. Use `done` as source of truth for final message. | Team lead |
| 5 | **Max message length** | **Soft limit** — no backend enforcement. Show character counter above 5,000 chars. Warn at 10K. No hard block. LLM context window is real constraint. | Team lead |
| 6 | **Required fields for task creation** | **Only `title`** — all other fields optional/nullable. Priority range 1-5. | BA spec |
| 7 | **Read-only settings fields** | `dataDir` is read-only (restart + migration required). Show "restart required" badge next to fields that need restart (model changes, channel configs). Secrets need dedicated "update key" input. | Team lead + BA |
| 8 | **Session resumption** | **Start fresh** — don't auto-resume. Show "Recent Sessions" section in right sidebar for manual resume. User might want clean context. | Team lead |
| 9 | **Plan creation from dashboard** | **"Create Plan" button opens chat** — prefills prompt `"Create a plan for: "` with cursor positioned. Plans always go through the agent, never direct REST. | Team lead |
| 10 | **Finance FIRE calculator** | **Client-side compound interest** — formula: `years = ln(target / (current + monthly * 12 * ((1+r)^n - 1) / r)) / ln(1+r)`. Monthly compounding. Defaults: 7% annual return, 3% inflation. All inputs user-overridable. No backend needed. | Team lead |
