# Daily Planning — UX Design Specification

## 1. Overview

The daily planning feature delivers a curated morning plan via chat channels and CLI. It surfaces the most impactful tasks, explains *why* each was chosen, and lets the user confirm, adjust, or dismiss the plan with minimal friction.

---

## 2. User Flows

### 2.1 Morning Notification Flow (Chat)

```
┌─────────────────────────────────────────────────┐
│  Cron trigger (configurable, default 08:00)     │
│         ↓                                       │
│  Planning engine selects top 3 tasks            │
│         ↓                                       │
│  ┌─ Has eligible tasks? ──────────────────┐     │
│  │ YES → Send plan notification           │     │
│  │ NO  → Send "all clear" message         │     │
│  └────────────────────────────────────────┘     │
│         ↓                                       │
│  User responds (yes / swap / skip / defer)      │
│         ↓                                       │
│  ┌─ Valid response? ──────────────────────┐     │
│  │ YES → Execute action, confirm result   │     │
│  │ NO  → Show help with valid options     │     │
│  └────────────────────────────────────────┘     │
│         ↓                                       │
│  Plan confirmed → Focus tasks                   │
└─────────────────────────────────────────────────┘
```

### 2.2 Manual Trigger Flow (CLI)

```
$ klyntbot todo plan              # Show plan interactively
$ klyntbot todo plan --accept     # Auto-accept plan
$ klyntbot todo plan --skip 2     # Skip task #2, promote next
$ klyntbot todo plan --json       # Machine-readable output
```

### 2.3 Response Handling Flow

```
User response received
    ↓
Parse response (case-insensitive, trimmed)
    ↓
┌─ Match type ───────────────────────────┐
│ "yes" / "y" / "ok" / "go"             │
│   → Focus all plan tasks               │
│   → Confirm: "Plan locked in! ✓"       │
│                                        │
│ "swap 1 and 3" / "swap 1,3"           │
│   → Reorder positions                  │
│   → Show updated plan                  │
│   → Await next response               │
│                                        │
│ "skip 2" / "skip #2"                  │
│   → Remove task #2                     │
│   → Promote next eligible task         │
│   → Show updated plan                  │
│   → Await next response               │
│                                        │
│ "defer" / "defer all" / "not today"   │
│   → Dismiss plan entirely             │
│   → Confirm: "Plan dismissed."         │
│                                        │
│ Unrecognized                           │
│   → Show: "I didn't catch that."       │
│   → List valid options                 │
└────────────────────────────────────────┘
```

---

## 3. Notification Format

### 3.1 Standard Plan (Chat — Telegram/Discord/Slack)

Uses the existing `draw_box` / `colorize` design system for CLI, and markdown for chat channels.

**Chat (Markdown):**

```
☀ Good morning! Here's your plan for Monday, Feb 16.

📅 Today's calendar:
  • 10:00 – Team standup
  • 14:00 – Design review

📋 Suggested focus (3 tasks):

  1. ⚡ Fix auth token expiry bug
     P1 · Overdue by 2 days · 30 min est.

  2. 🔨 Implement user settings page
     P2 · Due tomorrow · 60 min est.

  3. 🧹 Update API docs for v2 endpoints
     P3 · No deadline · 15 min est.

Reply: yes · swap 1 and 2 · skip 2 · defer all
```

**Design decisions:**
- Icons are single-character emoji for cross-platform rendering (no custom Unicode)
- Task reasoning appears on the second line in dim/italic style, explaining *why* this task was selected
- Priority uses `P1`–`P5` shorthand (familiar, compact)
- Estimated duration shown when available (from enrichment engine)
- Response options listed at the bottom as a clear prompt

### 3.2 CLI Format (Terminal)

Uses the existing box-drawing and color system (`BRAND`, `BOLD`, `DIM`, `SUCCESS`, `ERROR`, `WARNING`).

```
┌─ Daily Plan ─────────────────────── Mon, Feb 16 ┐
│                                                  │
│  📅 Calendar                                     │
│    10:00  Team standup                           │
│    14:00  Design review                          │
│                                                  │
│  📋 Focus Tasks                                  │
│                                                  │
│  1. Fix auth token expiry bug                    │
│     P1 · Overdue by 2 days · 30 min             │
│                                                  │
│  2. Implement user settings page                 │
│     P2 · Due tomorrow · 60 min                  │
│                                                  │
│  3. Update API docs for v2 endpoints             │
│     P3 · No deadline · 15 min                   │
│                                                  │
└──────────────────────────────────────────────────┘

  Accept this plan? [Y/n/skip N/swap X,Y]
```

Color mapping:
- Box border: `BRAND` (orange)
- Task titles: `BOLD`
- `P1`/`P2` priority labels: `ERROR` for P1, `WARNING` for P2, `DIM` for P3+
- "Overdue" text: `ERROR` (red)
- "Due tomorrow" text: `WARNING` (yellow)
- Estimated duration: `DIM`
- Calendar times: `TOOL` (cyan)

### 3.3 Empty State ("All Clear")

**Chat:**
```
☀ Good morning! All clear for Monday, Feb 16.

No tasks need your attention today. Enjoy your day!
```

**CLI:**
```
┌─ Daily Plan ─────────────────────── Mon, Feb 16 ┐
│                                                  │
│  ✓ All clear! No tasks need attention today.     │
│                                                  │
└──────────────────────────────────────────────────┘
```

### 3.4 Partial Plan (Fewer Than 3 Tasks)

Show 1–2 tasks with adjusted wording:

```
☀ Good morning! Here's your plan for Monday, Feb 16.

📋 Suggested focus (1 task):

  1. ⚡ Fix auth token expiry bug
     P1 · Overdue by 2 days · 30 min est.

Reply: yes · skip 1 · defer all
```

---

## 4. Response Parsing Specification

### 4.1 Accepted Patterns (Case-Insensitive)

| Intent | Accepted Patterns |
|--------|-------------------|
| Accept all | `yes`, `y`, `ok`, `go`, `looks good`, `let's go`, `confirm`, `accept` |
| Swap positions | `swap 1 and 2`, `swap 1,2`, `swap 1 2`, `reorder 1 and 2` |
| Skip a task | `skip 1`, `skip #1`, `remove 1`, `drop 1` |
| Defer all | `defer`, `defer all`, `not today`, `pass`, `dismiss`, `nah`, `no` |

### 4.2 Parsing Rules

1. **Trim and lowercase** the input
2. **Strip leading/trailing punctuation** (handles "yes!" or "ok.")
3. **Match against known patterns** using regex:
   - Accept: `^(yes|y|ok|go|looks?\s*good|let'?s?\s*go|confirm|accept)$`
   - Swap: `^(swap|reorder)\s+(\d+)\s*(and|,|\s)\s*(\d+)$`
   - Skip: `^(skip|remove|drop)\s+#?(\d+)$`
   - Defer: `^(defer(\s+all)?|not\s*today|pass|dismiss|nah|no)$`
4. **Validate numbers** against plan size (1-indexed)
5. **Reject ambiguous input** with help text

### 4.3 Validation Error Messages

| Error Case | Message |
|------------|---------|
| `skip 5` on 3-task plan | "There are only 3 tasks in your plan. Try: skip 1, skip 2, or skip 3." |
| `swap 1 and 1` | "Those are the same position. Try swapping different tasks." |
| Empty input | _(no response — wait for valid input)_ |
| Random text ("hello") | "I didn't catch that. Options: **yes** · **swap 1 and 2** · **skip 2** · **defer all**" |
| Duplicate response after plan confirmed | "Your plan for today is already confirmed." |

---

## 5. State Machine

```
                    ┌──────────┐
                    │ NO_PLAN  │ (initial state)
                    └────┬─────┘
                         │ trigger (cron or manual)
                         ▼
                    ┌──────────┐
                    │ PENDING  │ plan generated, awaiting response
                    └────┬─────┘
                         │
            ┌────────────┼────────────┐
            │            │            │
       swap/skip      accept       defer
            │            │            │
            ▼            ▼            ▼
       ┌──────────┐ ┌──────────┐ ┌──────────┐
       │ PENDING  │ │ ACCEPTED │ │ DEFERRED │
       │ (updated)│ │          │ │          │
       └──────────┘ └──────────┘ └──────────┘

  • PENDING → user can swap/skip (stays PENDING with updated plan)
  • PENDING → user accepts → ACCEPTED (tasks focused)
  • PENDING → user defers → DEFERRED (no action taken)
  • ACCEPTED/DEFERRED → re-trigger resets to PENDING with fresh plan
```

### 5.1 Plan Lifecycle

- **Plan is ephemeral per day**: A new plan is generated each trigger. No persistence of old plans beyond the current session.
- **Session binding**: The plan is bound to the chat session (via `SessionKey`). Each channel gets its own plan state.
- **Expiry**: Plans expire at end of day (midnight local time). A stale plan response gets: "This plan has expired. Run `todo plan` for a fresh one."

---

## 6. Edge Cases

### 6.1 Focus Slots Full

When all focus slots are occupied (e.g., `max_focus_slots = 3` and 3 tasks already focused):

**Notification appends a warning:**
```
⚠ You have 3/3 focus slots in use. Accepting this plan
will require unfocusing current tasks first.

Currently focused:
  • Fix login CSS regression (focused 2h ago)
  • Write unit tests for auth (focused yesterday)
  • Update README (focused 3 days ago)

Reply: yes (replaces current focus) · defer all
```

**Behavior on "yes"**: Unfocus all currently focused tasks (closing their time entries), then focus the new plan tasks. This is a deliberate "replace" semantic — the plan represents the user's fresh priorities.

### 6.2 Task Completed After Plan Sent

When user responds "yes" but a task in the plan was completed between send and response:

1. **Re-validate** each task's status before focusing
2. **Skip completed tasks** silently
3. **Promote** next eligible task to fill the slot
4. **Confirm** with adjusted count: "Plan confirmed! Focused 2 tasks (1 was already completed)."

### 6.3 No Calendar Configured

If calendar sync is not configured, simply omit the calendar section:

```
☀ Good morning! Here's your plan for Monday, Feb 16.

📋 Suggested focus (3 tasks):
  ...
```

No error, no placeholder — clean omission.

### 6.4 Plan Already Confirmed Today

If user triggers plan again after accepting:

```
Your plan for today is already confirmed.

Currently focused:
  1. Fix auth token expiry bug
  2. Implement user settings page
  3. Update API docs for v2 endpoints

Run `todo plan --force` to generate a new plan.
```

### 6.5 Invalid Input Recovery

After 3 consecutive invalid inputs, include the full help inline:

```
Having trouble? Here are your options:

  yes        Accept and focus all suggested tasks
  swap 1,2   Swap the order of tasks 1 and 2
  skip 2     Remove task 2 and promote the next one
  defer all  Dismiss the plan entirely
```

---

## 7. Channel-Specific Adaptations

### 7.1 Telegram

- Uses Markdown formatting (bold, italic)
- Response via regular message reply
- Inline keyboard buttons for common actions: `[Accept] [Defer]`
  - Swap/skip remain text-based (too many button combinations)

### 7.2 Discord

- Uses Discord markdown (same as standard)
- Embed format for the plan notification (richer formatting)
- Text-based responses in the same channel

### 7.3 Slack

- Uses Slack mrkdwn (`*bold*`, `_italic_`)
- Block Kit for structured layout (optional enhancement)
- Text-based responses

### 7.4 CLI

- Uses terminal box-drawing and ANSI colors
- Interactive prompt with readline (`Accept this plan? [Y/n/skip N/swap X,Y]`)
- `--accept`, `--skip N` flags for non-interactive/scripting use

### 7.5 Cross-Channel Plan Sharing

- A single plan is generated per day per user (not per channel)
- Accepting on Telegram makes it confirmed on Discord too
- The plan state lives in the agent's session/memory, keyed by date

---

## 8. Accessibility & Degradation

### 8.1 Plain Text Fallback

When `NO_COLOR=1` or non-TTY, strip all ANSI codes and emoji. Use ASCII:

```
Daily Plan — Mon, Feb 16

Calendar:
  10:00  Team standup
  14:00  Design review

Focus Tasks:

  1. Fix auth token expiry bug
     P1 - Overdue by 2 days - 30 min

  2. Implement user settings page
     P2 - Due tomorrow - 60 min

  3. Update API docs for v2 endpoints
     P3 - No deadline - 15 min

Accept this plan? [Y/n/skip N/swap X,Y]
```

### 8.2 Language

- Clear, concise sentences (no jargon)
- Consistent terminology: "focus" (not "start" or "begin"), "skip" (not "remove"), "defer" (not "cancel")
- Reasoning uses natural language: "Overdue by 2 days" not "overdue_days=2"

---

## 9. Confirmation Messages

| Action | Confirmation Message |
|--------|---------------------|
| Accept all | "Plan locked in! Focused on 3 tasks. Good luck today." |
| Accept (with stale tasks) | "Plan confirmed! Focused 2 tasks (1 was already completed)." |
| Swap | _(shows updated plan, no separate confirmation)_ |
| Skip | _(shows updated plan with promoted task)_ |
| Defer | "Plan dismissed. You can run `todo plan` anytime." |
| Focus slots replaced | "Unfocused 3 previous tasks. Plan locked in with 3 new tasks." |

---

## 10. Data Contract (for Architect)

The UX expects the planning engine to provide:

```rust
struct DailyPlan {
    date: NaiveDate,
    greeting: String,              // "Good morning!" (time-aware)
    calendar_events: Vec<CalendarEvent>,  // from CalendarHandler
    suggestions: Vec<PlanSuggestion>,     // 0–3 tasks
    focus_slots_available: usize,         // current availability
    focus_slots_total: usize,             // max from config
    currently_focused: Vec<FocusedTask>,  // for slot-full warning
}

struct PlanSuggestion {
    rank: u8,                      // 1-indexed position
    task_id: String,
    title: String,
    priority: Option<u8>,          // 1–5
    reasoning: String,             // human-readable explanation
    estimated_duration_mins: Option<u32>,
    due_context: Option<String>,   // "Overdue by 2 days", "Due tomorrow", etc.
}
```

The UX layer (formatter) takes this struct and produces channel-appropriate output. The response parser returns a `PlanAction` enum that the planning engine executes.

```rust
enum PlanAction {
    Accept,
    Swap { pos_a: usize, pos_b: usize },
    Skip { pos: usize },
    DeferAll,
}
```
