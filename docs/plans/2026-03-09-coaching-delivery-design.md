# Coaching Delivery Design

> **Goal:** Surface coaching interventions to users via chat nudges and tray popups, with focus mode awareness.

## Two Delivery Channels

### Channel 1: Chat Nudge (in-app, passive)
- Subtle banner above chat input — not a chat message
- Only shows when user navigates to chat or finishes a message exchange (not mid-stream)
- Auto-collapses after 60s if ignored
- Max 1 visible at a time — new interventions queue behind current
- 3 feedback buttons inline: Helpful / Dismiss / Stop Suggesting

### Channel 2: Tray Nudge (background)
- Only fires when app window is not focused
- Shows in tray popup window (same tray used for quick actions)
- Auto-hides after 30s
- No macOS system notifications — stays within our control

### Channel Selection Logic
```
Intervention arrives →
  Is user in active focus/pomodoro session? → Queue (deliver as debrief after session)
  Is AI currently streaming? → Queue (deliver after done)
  Is app window focused? → Channel 1 (chat nudge)
  Is app window unfocused? → Channel 2 (tray nudge)
  Has user ignored last 2 nudges in a row? → Skip entirely
```

## Focus Mode Integration (Option B: Gate in CoachingService)

| Focus State | Signals | Delivery |
|-------------|---------|----------|
| No session | Normal | Normal |
| Focus / Pomodoro | Accumulate silently | Queued — held until session ends |
| Break | Normal | Light only (break reminders OK) |
| Focus just ended | Normal | Post-session debrief (single consolidated card) |

### Post-Session Debrief
When focus session ends, instead of dumping queued interventions individually, consolidate into one coaching card with focus session context:
- Session duration + quality score
- Number of distractions + what apps
- One actionable suggestion

### Implementation in CoachingService
- Before calling reasoner/router, check `focus_manager.get_active()`
- If focus active → push trigger into held queue, skip reasoner
- On `FocusSessionEnded` event → drain queue, build consolidated debrief, route as single intervention
- Debrief uses LLM reasoner with richer input (session quality, distraction apps, duration)

## Noise Prevention

| Mechanism | Value | Source |
|-----------|-------|--------|
| Hourly rate limit | 3/hr | InterventionRouter |
| Daily rate limit | 5/day (lowered from 10) | InterventionRouter |
| Per-trigger cooldown | 15min–1hr | SignalAccumulator |
| Dismissal backoff | Progressive per trigger | InterventionRouter |
| "Stop Suggesting" | Permanent block per trigger | FeedbackTracker |
| Coaching receptivity | Gate at < 0.3 | UserSituation |
| Focus session block | Zero nudges during focus | CoachingService |
| Active streaming block | Queue until AI response done | Delivery layer |
| Consecutive ignore skip | Skip after 2 ignored in a row | Delivery layer |

## Architecture

```
DomainEventBus
  └→ CoachingService (event loop)
       ├─ push_event → SignalAccumulator
       ├─ check focus_active?
       │   ├─ YES → queue trigger, skip reasoner
       │   └─ NO → evaluate triggers → reasoner → router
       ├─ on FocusSessionEnded → drain queue → build debrief → router
       └─ intervention_tx → DeliveryManager (NEW)
            ├─ check window focused?
            ├─ check AI streaming?
            └─ dispatch to chat nudge OR tray nudge
```

`DeliveryManager` is a new thin layer between the coaching pipeline and the UI. It decides _when_ and _where_ to show the intervention. Lives in `app-core` since it needs access to window state and streaming state.
