# Second Brain Comes Alive — Pre-Release Optimization Spec

> **Goal:** Turn Klyntbot from "a powerful app" into a real second brain that feels alive, protective, and personally insightful — optimizing for what users perceive in the first 5 seconds, not what devs see.

> **Timeline:** 3 weeks. Week 1: Ambient Magic. Week 2: Polish + Onboarding. Week 3: Code Diet (scoped).

> **Priority:** Perceived intelligence ("feel alive") before perceived performance (Code Diet). Code Diet is #2 and must not delay or degrade any Week 1/2 feature.

## Design Principles

1. **One brain, one voice.** All intelligence signals (memory promotion, cross-domain dots, coaching, mirror insights) route through a single backend orchestrator (BrainVoice) that decides what/when/where to surface. The frontend renders what the brain decides — zero decision logic in React.
2. **Sparse, first-person, action-oriented.** The brain speaks in first person ("I noticed X — want me to Y?"), limited to one clause per pulse. Badges use neutral voice. Debriefs use reflective voice.
3. **Earn every introduction.** No tutorial modals. Every feature introduces itself in the moment it proves useful (Day 3 Orb Awakening pattern).
4. **Protect, don't nag.** During focus, the brain holds everything and delivers a coherent debrief afterward. Max 2 pulses/hour. Adaptive dampening based on user dismissals.
5. **The brain never dies.** When LLM providers fail, the brain switches to local reasoning and tells the user warmly. Cross-domain dots, memory promotions, and BrainVoice all work without cloud.

---

## Section 1: BrainVoice — Unified Signal Router

### What it is

A thin subscriber on `DomainEventBus` inside AppCore that collects intelligence signals, applies timing/priority/dedup rules, and emits a single `brain:ambient` Tauri event. The frontend renders whatever BrainVoice decides.

### Signal types consumed

| Signal              | Source                          | Trigger                                                |
| ------------------- | ------------------------------- | ------------------------------------------------------ |
| `MemoryPromoted`    | `cognitive` crate (new event)   | Fact promoted persona -> squad -> global                |
| `CrossDomainDot`    | `feature-insights` (new)        | Embedding overlap >= 0.72 + temporal/frequency signals  |
| `CoachingTrigger`   | `feature-coaching` (existing)   | Pattern detection, nudge, debrief                       |
| `MirrorInsight`     | `cognitive/mirror` (existing)   | Weekly narrative ready, meta-rule proposed               |
| `MessageDeferred`   | `channels` (new)                | Inbound message deferred during focus                   |

### Surface modes

| Mode         | When                                        | UX                                                       |
| ------------ | ------------------------------------------- | -------------------------------------------------------- |
| **Pulse**    | User active, not in focus, signal important | Orb glows + 1-line first-person tooltip fades in (3s)    |
| **Badge**    | User active, signal is supplementary        | Orb counter increments silently (no animation)           |
| **Deferred** | Focus session active                        | Queued for post-session debrief                          |
| **Merged**   | 2+ signals within 30s window                | Combined into single pulse with multi-line tooltip       |

### Interaction rules

- **Max 2 pulses/hour** (first release). After cap, new signals become badges.
- **Merge window:** Signals arriving within 30s are batched. BrainVoice holds the first signal for 5s to catch fast followers.
- **Dedup:** Same entity/connection within 24h -> suppressed. Uses `enrichment_feedback` for dismissed connections (30-day cooldown).
- **Focus deferral:** ALL signals defer during focus. No exceptions pre-release. Post-session debrief surfaces everything as one coherent summary.
- **Adaptive dampening:** If user dismisses >2 orb pulses in 48h, silently reduce cap to 1/hour and increase merge window to 60s. Reads from existing `enrichment_feedback` — zero new state.

### Break reminders

Break reminders (wellness signals from coaching) stay on their existing channel (distraction overlay + tray). BrainVoice handles intelligence signals only. Two distinct voices, two distinct surfaces:
- Orb glow = "my second brain is thinking about me" (delight)
- Overlay + tray = "my second brain is looking after my focus" (protection)

### Voice contract

| Surface              | Voice     | Example                                                                        |
| -------------------- | --------- | ------------------------------------------------------------------------------ |
| Pulse tooltip        | 1st person, one clause, action-oriented | "Your Q2 deck connects to $4,200 in consulting spend — want me to pull the numbers?" |
| Badge tooltip (hover)| Neutral, factual                        | "2 new connections - 1 memory promoted"                                        |
| Debrief summary      | 1st person, reflective                  | "While you were focused, I noticed your Q2 prep task connects to last month's spending spike. I held it until now." |

Constraint: never more than one clause. If two thoughts exist, BrainVoice merges them into a single higher-level insight.

### Tauri event shape

```rust
#[derive(Serialize, Clone)]
struct BrainAmbientEvent {
    mode: SignalMode,              // Pulse | Badge | Deferred | Merged
    signals: Vec<SignalSummary>,   // 1+ signals (merged if batched)
    tooltip: String,               // Pre-rendered 1-liner for pulse mode
    detail_route: Option<String>,  // e.g., "/brain?filter=cross-domain"
}
```

### Location

`crates/app-core/src/brain_voice.rs` — new file, ~200 lines. Subscribes to `DomainEventBus`, holds `Arc<AtomicBool>` ref to `FOCUS_ACTIVE`, emits via Tauri `AppHandle::emit`. Stored as `Option<BrainVoice>` in `AppCore` (same pattern as `MirrorFacade`).

---

## Section 2: Memory Pulse Orb — The Brain's Heartbeat

### What it is

A persistent, small glowing dot in the **global top bar** (window header chrome, top-right) that serves as the single visual surface for BrainVoice. Always visible across all views (Chat, Tasks, Finance, Notes, Brain, etc.). Usually quiet. When the brain has something to say, it pulses.

### Why global top bar (not chat header)

The brain watches everything, not just chat. If a user is in the Finance dashboard and a cross-domain dot fires, they must see the pulse in real time. The orb is the visual promise that "your second brain is alive and thinking about you right now" — regardless of which view is active.

### Visual states

| State               | Appearance                                                          | Trigger                           |
| ------------------- | ------------------------------------------------------------------- | --------------------------------- |
| **Idle**            | Soft, dim dot (opacity 0.3, `text-muted` color)                    | No recent signals                 |
| **Pulse**           | Gentle glow + scale (1.0 -> 1.3 -> 1.0, 600ms ease-out), warm amber | BrainVoice emits Pulse            |
| **Badge**           | Idle dot + small counter (e.g., "2") in `text-xs`                  | BrainVoice emits Badge            |
| **Active tooltip**  | Dot stays warm + tooltip panel fades in below                      | User hovers pulsed orb            |
| **Focus-deferred**  | Dot shows tiny shield icon overlay (8px, opacity 0.5)              | Focus session active              |
| **Idle breathing**  | Almost imperceptible 8s slow pulse (opacity 0.3 -> 0.45 -> 0.3)   | Recent signals exist but idle     |

### Animation details

- Pulse is a single wave, not a loop. Glows once, fades back to idle over 2s. If another arrives before fade completes, it chains (re-triggers from current brightness — feels organic).
- Badge counter sits top-right of dot, same pattern as notification badges elsewhere.
- Focus-deferred shield is a micro-icon — just enough to signal "I'm holding things for you."
- Idle breathing is a Week 1.5 nice-to-have. Reinforces "alive even when quiet."

### Tooltip behavior

**On hover after pulse:**
- Fade in (200ms) a compact glass panel below orb, max-width 320px.
- Content: first-person tooltip string from `BrainAmbientEvent.tooltip`.
- Action link: if `detail_route` present, show subtle "See more ->" that navigates to Brain dashboard.
- Auto-dismiss: fades after 8s if cursor moves away. Stays open while hovered.
- Glass panel styling: existing `glass-panel` class.

**On hover with badge (counter > 0):**
- Summary list with neutral voice:
  ```
  2 new connections
  ─────────────────
  - "Q2 budget prep" <-> spending spike (cross-domain)
  - Promoted: dark-mode preference (memory)

  See all -> [opens Brain]
  ```

### Click behavior

- Single signal with `detail_route` -> navigate directly (e.g., `/brain?filter=cross-domain`)
- Multiple signals -> open badge summary panel
- Idle (no signals) -> open Brain dashboard at `/brain` (the orb is always an on-ramp)

### Brain nav indicator

The "Brain" nav item in the sidebar gets a tiny live pulse indicator (4px amber dot, same color as the orb) when BrainVoice has pending signals. Reuses the same `brain:ambient` event — zero new logic.

### Component structure

```
desktop-ui/src/shared/components/BrainOrb.tsx       — dot + animation + tooltip
desktop-ui/src/shared/hooks/useAmbientSignals.ts    — listens to brain:ambient Tauri events
```

Shared (not feature-scoped) because the orb appears in global chrome across all routes.

### What it does NOT do

- No sound effects (pre-release)
- No persistent notification outside the app
- No settings toggle for pulse frequency (adaptive dampening handles this)
- No animation on page load (orb starts idle, earns attention through real signals)

---

## Section 3: Focus Bubble — Protective Silence Across All Channels

### What it is

When a focus session is active, the second brain silently defers ALL incoming noise — channel messages, BrainVoice signals, coaching nudges — and delivers them as one coherent debrief when focus ends.

### What exists today

- `FOCUS_ACTIVE` AtomicBool in `feature-productivity` — already signals focus state globally
- Coaching pipeline already queues triggers during focus + sends post-session debrief
- Channels (`crates/channels/src/manager.rs`) do NOT check focus state
- Distraction overlay already blocks app-switching during focus

### Backend: Channel deferral

In `crates/channels/src/manager.rs`, the inbound message handler gets a focus-aware gate:

```
Inbound message arrives
  -> Is FOCUS_ACTIVE true?
    -> YES: store in deferred_messages queue (in-memory Vec, not persisted)
            + optionally send auto-reply to sender
            + emit DomainEvent::MessageDeferred { channel, sender, preview }
    -> NO: process normally
```

### Auto-reply behavior

- One auto-reply per sender per focus session (not per message).
- Default text: "I'm in a deep focus session right now. I'll get back to you when I'm done."
- Configurable in `config.json` -> `productivity.focusBubble.autoReply`.
- **Opt-in, off by default.** Deferral (holding messages internally) is default-on. Auto-reply (visible to other people) requires explicit user consent. Rationale: auto-replying on someone's Telegram without consent is a social action that could cause "why is my bot replying to my boss?" moments.

### BrainVoice integration

`MessageDeferred` events flow to BrainVoice. During focus, they're counted for the debrief (Deferred mode). No new routing logic — just a new signal type in the existing router.

### Post-focus debrief

When `FocusSessionEnded` fires, BrainVoice collects all deferred items:
1. Deferred channel messages (from `MessageDeferred` events)
2. Deferred intelligence signals (memory promotions, cross-domain dots)
3. Coaching post-session summary (already exists)

Emits a single `brain:ambient` event with mode `Deferred`:

```rust
BrainAmbientEvent {
    mode: SignalMode::Deferred,
    signals: vec![/* all deferred signals */],
    tooltip: "While you were focused for 47 min, I held 3 messages and noticed 1 connection — want to catch up?",
    detail_route: None,  // Debrief opens as inline slide-in panel, not a route
}
```

### Debrief panel (frontend)

Clicking the post-focus orb pulse opens a slide-in panel (same pattern as transparency panel):

```
+------------------------------------------+
|  Focus Session Complete - 47 min         |
|------------------------------------------|
|                                          |
|  Messages held (3)                       |
|  +- Alice (Telegram): "Hey, can you..." |
|  +- #team (Slack): 2 new messages        |
|  +- Dave (Discord): "Quick question..."  |
|    [Reply all] [Open individually]       |
|                                          |
|  Brain activity                          |
|  +- Connected: "Q2 deck" <-> spending    |
|  +- Promoted: meeting-time preference    |
|    [See in Brain ->]                     |
|                                          |
|  Coaching                                |
|  +- "You maintained focus for 47 min --  |
|     12 min longer than your average.     |
|     Your focus score improved."          |
|                                          |
+------------------------------------------+
```

Glass-panel styling. Three collapsible sections. "Reply all" opens each channel's native reply. "See in Brain" navigates to `/brain?filter=session`.

### Component structure

```
desktop-ui/src/features/productivity/components/FocusDebrief.tsx
```

Lives in `features/productivity` (tied to focus lifecycle), but consumes `useAmbientSignals` from shared.

### What it does NOT do

- No email deferral (email is async by nature)
- No UI dimming or modal lockout (distraction overlay already handles app-switching)
- No persistent notification to sender beyond the single opt-in auto-reply
- No focus-session scheduling from this feature (separate launcher/calendar concern)

---

## Section 4: Cross-Domain Dots — The Brain Connecting What You Didn't

### What it is

A high-precision, embedding-based heuristic that detects meaningful connections across tasks, notes, and finance — surfaced through BrainVoice as the "wow" moment of the second brain.

### Week 1: Rule-based heuristic (embedding-powered, zero LLM calls)

#### Trigger points

Runs synchronously when:
- **Task created or viewed** (detail view, not list scroll) -> search note + finance embeddings
- **Note created or viewed** -> search task + finance embeddings
- **Finance entry created or viewed** -> search task + note embeddings

#### The heuristic (3 signal layers, require >= 2 to surface)

```
Layer 1: Semantic overlap
  - Embed the source item (already computed on create)
  - Vector search top-3 in each target domain (LanceDB, <20ms per domain)
  - Require cosine >= 0.72

Layer 2: Temporal proximity
  - Target item created/modified within 7 days of source item

Layer 3: Frequency signal
  - Same entity (existing NER extraction in cognitive pipeline) mentioned
    >= 2 times this week across >= 2 features

Rule: surface only if >= 2 of 3 layers match for the same target item.
```

#### False-positive protection

| Guard              | Mechanism                                                                |
| ------------------ | ------------------------------------------------------------------------ |
| Already dismissed  | Check `enrichment_feedback` — 30-day cooldown per entity pair            |
| Already surfaced   | Dedup same connection within 24h                                         |
| Too many dots      | Max 2 dots per domain per day                                            |
| User dampening     | >2 dismissals in 48h -> require all 3 layers to match                   |
| Low-value items    | Skip items with <10 chars in title/description                           |

#### Output shape

```rust
struct CrossDomainDot {
    source: EntityRef,              // e.g., Task { id, title }
    target: EntityRef,              // e.g., FinanceEntry { id, description }
    layers_matched: Vec<Layer>,     // which 2-3 layers triggered
    confidence: f32,                // average cosine across matched pairs
    suggested_action: Option<String>,
}
```

#### Suggested action templates (7 for Week 1)

| Connection type                    | Template                                                                                  |
| ---------------------------------- | ----------------------------------------------------------------------------------------- |
| Task <-> Finance (spending)        | "Your {task} connects to {amount} in {category} spend last month — want me to pull the numbers?" |
| Task <-> Note (prior research)     | "You wrote about this exact topic in {note_date} — want me to pull that note?"            |
| Task <-> Productivity (pattern)    | "Last time you had a similar task you slipped {days} days — want me to block focus time?" |
| Finance <-> Note (insight)         | "Your notes flag this expense category as '{tag}' — want the full history?"               |
| Task overdue + Finance pressure    | "This task is overdue and your budget is {pct}% over — adjust deadline or reallocate?"    |
| Task upcoming + Finance data ready | "Your upcoming {task} has fresh {month} numbers ready — want me to prep a summary?"       |
| Note <-> Finance (trend)           | "Your note predicted this spending spike — want me to log it as an insight?"              |

Templates are pure match arms (~30 lines). First-person voice contract applies — BrainVoice crafts final tooltip from dot data.

#### Mirror snapshot synergy

When a dot is generated, push a tiny entry into `mirror_routing_snapshots`. This makes cross-domain connections automatically appear in the weekly Brain narrative: "This week your brain connected 4 dots across tasks and finance."

#### Location

```
crates/feature-insights/src/cross_domain.rs   — the heuristic (new file, ~250 lines)
crates/feature-insights/src/service.rs         — wire into existing InsightService
```

The heuristic is a pure function: entity + embeddings + feedback history -> `Option<CrossDomainDot>`. No state, no background thread. Runs inline when triggered.

#### Flow

```
User views task detail
  -> InsightService::check_cross_domain(task)
  -> Heuristic: task "Q2 deck" <-> finance "March consulting spend"
     (cosine 0.78, created 3 days apart, 2 layers matched)
  -> Emits DomainEvent::CrossDomainDotReady { dot }
  -> BrainVoice receives, applies pulse/badge/defer rules
  -> Tauri event -> orb pulses
  -> Tooltip: "Your Q2 deck connects to $4,200 in consulting spend — want me to pull the numbers?"
  -> Click -> Brain dashboard opens filtered to this connection
```

### Week 2: LLM batch enhancement (nightly job)

| Aspect       | Detail                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------- |
| **Trigger**  | Nightly cron (2 AM local), same pattern as `JOB_MIRROR_WEEKLY_NARRATIVE`                 |
| **Input**    | All cross-domain dots surfaced today (from `enrichment_feedback` log)                    |
| **LLM call** | One call per batch (~500 tokens). "Generate 1-3 polished insight sentences for tomorrow." |
| **Output**   | Stored in new `cross_domain_insights` table (id, date, insight_text, dot_refs, surfaced) |
| **Morning**  | BrainVoice checks unsurfaced insights on app launch -> emits as first pulse with LLM copy |
| **Fallback** | If LLM fails, fall back to template copy. User never knows.                              |

### What it does NOT do

- No real-time LLM calls for dot detection (heuristic is the permanent layer)
- No triple-domain connections (task + finance + note simultaneously — too noisy)
- No dots for items older than 14 days
- No "connection graph" visualization (Brain timeline is sufficient)

---

## Section 5: Mirror Discoverability, Onboarding, and Polish

### 5A: Mirror -> "Brain" Rename and Promotion

Three changes to make the Brain dashboard impossible to miss:

1. **Orb click -> Brain.** Clicking the idle orb (no pending signals) opens `/brain`. The orb is a permanent on-ramp. (Already specified in Section 2.)

2. **Top-level "Brain" nav item.** Move from regular sidebar item to visually distinct top-level entry (same tier as Chat, Tasks, Finance). Neuron icon matching the orb. Label: "Brain" not "Mirror." Route: `/brain` (UI rename only — internal crate stays `cognitive/mirror`). Tiny 4px amber pulse indicator on the nav item when BrainVoice has pending signals (reuses `brain:ambient` event).

3. **Weekly "Brain Report" notification.** Every Sunday (aligned with `JOB_MIRROR_WEEKLY_NARRATIVE` at 10 AM UTC), BrainVoice receives `MirrorInsight` signal -> emits pulse: "Your weekly brain report is ready — 3 new patterns this week." Click opens `/brain` pre-scrolled to narrative card.

### 5B: 7-Day Guided Journey

The setup wizard (`ConversationRunner`) handles Day 0. The guided journey extends post-setup with milestone-triggered introductions — no tutorial modals, no fixed timing.

| Day | Trigger                            | What happens                                                                                  | User feels           |
| --- | ---------------------------------- | --------------------------------------------------------------------------------------------- | -------------------- |
| 0   | First launch                       | Existing setup wizard (name, preferences, finance, API keys)                                  | "This is organized"  |
| 1   | Post-setup                         | Prompt to import: "Got existing tasks or notes? Drop them in and I'll start learning."        | "It wants to learn"  |
| 2   | First chat response                | Transparency panel auto-opens once: "See what I used to answer — that's my working memory."   | "It's transparent"   |
| 3   | First BrainVoice pulse             | **Orb Awakening:** orb does first pulse + guided tooltip (see below)                          | "It's alive"         |
| 4   | First focus session ends           | Debrief pulse + guided tooltip: "While you were focused I held everything — protective bubble."| "It protects me"     |
| 5   | First cross-domain dot accepted    | Badge tooltip: "You accepted a connection — I'll look for more like that. I learn from you."  | "It learns from me"  |
| 6   | User opens app, orb idle           | Subtle breathing + one-time tooltip: "I'm still here. I only speak when it's worth saying."   | "It knows when to shut up" |
| 7   | Sunday brain report                | Weekly narrative pulse: "Your first brain report is ready."                                   | "It's evolving"      |

**Day 3 Orb Awakening copy:**

> "Hey — I'm your second brain's heartbeat.
> I only light up when I've connected something worth your attention.
> This one was about your Q2 deck + last month's spending.
> Want me to show you?"

Two buttons: "Show me ->" (opens detail via `detail_route`) and "Got it — keep whispering" (dismisses, recorded in `enrichment_feedback`).

**Adaptive pacing:** If a user skips ahead (starts focus on Day 1), show the relevant guided tooltip on first occurrence regardless of day number. Days are aspirational, not rigid gates.

**Hello pulse guard:** On first launch after install, if no signals have fired yet, the orb does a single ultra-subtle pulse with a tiny label that fades after 4s: "Your second brain's heartbeat is here." Gated: only if user has >= 3 items across any features. Otherwise, wait for Day 3 organic trigger.

### Implementation

```
crates/app-core/src/journey.rs             — JourneyTracker (milestone bitfield in user_preferences)
desktop-ui/src/shared/hooks/useJourney.ts  — exposes milestone state to guided tooltips
```

Each milestone is a one-time check. The tracker stores which milestones have been hit. Guided tooltips render once per milestone, then never again.

### 5C: Graceful LLM Fallback

When the circuit breaker opens or all providers fail:

**Chat UI (warm degradation):**

Primary provider down, fallback available:
> "Claude is taking a moment. I'm working from what I already know about you — cached memory and local reasoning. Give me a sec."

All providers down:
> "All my cloud connections are down right now. I can still search your tasks, notes, and memory locally — just ask."

Enables "local-only" mode: chat input routes to keyword search + `ToolRegistry` search functions. Not AI — but the brain says "I'm impaired but still here."

**BrainVoice:** Cross-domain dots and memory promotions are local — continue normally. Nightly LLM batch silently skips, falls back to template copy.

**Coaching:** Already has heuristic fallback chains. No change needed.

**Implementation:** Chat-level fallback listens for `provider:degraded` Tauri event in `ChatPage.tsx`. Local-only search is a thin wrapper in `app-core` delegating to existing `ToolRegistry`.

### 5D: Week 2 and Week 3 Scoping

**Week 2 — Polish (5 days):**
- Nightly LLM batch for cross-domain insights
- 7-day journey wiring (milestone tracker + guided tooltip variants)
- LLM fallback messages
- Idle orb breathing animation
- Launcher "brain" quick-command entry
- Internal dogfooding begins

**Week 3 — Code Diet (scoped):**
- Binary size target: < 120 MB
- Cold start target: < 1.5s on M1 Air 8 GB
- Resident memory target: < 650 MB under normal chat use
- Lazy-load: WASM plugins, full launcher index, non-critical vector tables
- **Hard constraint:** Nothing in Code Diet may degrade first-message latency or BrainVoice signal delivery. If a lazy-loading change delays the orb's first pulse, it's cut.

---

## New Files Summary

| File                                               | Layer   | Size est. | Purpose                           |
| -------------------------------------------------- | ------- | --------- | --------------------------------- |
| `crates/app-core/src/brain_voice.rs`               | L7      | ~200 LOC  | BrainVoice signal router          |
| `crates/app-core/src/journey.rs`                   | L7      | ~100 LOC  | JourneyTracker milestone bitfield |
| `crates/feature-insights/src/cross_domain.rs`      | L4      | ~250 LOC  | Cross-domain heuristic            |
| `desktop-ui/src/shared/components/BrainOrb.tsx`    | Frontend| ~150 LOC  | Orb + animation + tooltip         |
| `desktop-ui/src/shared/hooks/useAmbientSignals.ts` | Frontend| ~50 LOC   | Tauri event listener              |
| `desktop-ui/src/shared/hooks/useJourney.ts`        | Frontend| ~40 LOC   | Milestone state for tooltips      |
| `desktop-ui/src/features/productivity/components/FocusDebrief.tsx` | Frontend | ~200 LOC | Post-focus debrief panel |

## Modified Files Summary

| File                                        | Change                                              |
| ------------------------------------------- | --------------------------------------------------- |
| `crates/common/src/events.rs` (or equivalent) | Add `MemoryPromoted`, `CrossDomainDotReady`, `MessageDeferred` event variants |
| `crates/cognitive/src/services/memory_promotion.rs` | Emit `MemoryPromoted` event at promotion callsite |
| `crates/channels/src/manager.rs`            | Focus-aware gate on inbound messages                |
| `crates/feature-insights/src/service.rs`    | Wire `check_cross_domain()` into InsightService     |
| `crates/app-core/src/lib.rs`                | Store `BrainVoice` + `JourneyTracker` in AppCore    |
| `crates/config/src/schema/`                 | `productivity.focusBubble.autoReply` config field    |
| `desktop-ui/src/app/` (layout/chrome)       | Mount BrainOrb in global top bar                    |
| `desktop-ui/src/app/router.tsx`             | Rename `/mirror` -> `/brain`                        |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | LLM fallback warm messages                    |
| Sidebar nav component                       | Promote Brain to top-level with pulse indicator     |
| `crates/cognitive/migrations/` (or feature migration) | New `cross_domain_insights` table (Week 2 nightly batch) |
| `crates/bus/src/domain_events.rs`           | New event variants (if events live here, not `common`) |

## Non-Goals

- Sound effects for the orb
- Push notifications outside the app
- Cross-domain triple connections (3+ domains)
- Real-time LLM for dot detection
- Focus-session scheduling
- Email deferral during focus
- Connection graph visualization
- Settings UI for pulse frequency
- Structured observability (per existing CLAUDE.md)
