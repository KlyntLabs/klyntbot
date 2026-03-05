# Smart Distraction Overlay — Design

## Problem

The distraction detection system fires `DistractionAlert` events during focus sessions but the frontend ignores them. Users get no real-time intervention when they switch to distracting apps. Additionally, the system can't distinguish educational content (YouTube tutorial) from entertainment (YouTube cat videos).

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Trigger scope | Focus sessions only | Non-annoying, purposeful — user opted into focus mode |
| Intervention form | Floating overlay window | Creates deliberate pause; harder to ignore than notifications |
| Content analysis | Hybrid (heuristics + LLM) | Instant for obvious cases, intelligent for ambiguous ones |
| User actions | Three-way (back to work / allow 5 min / work-related) | Temporary pass prevents "cry wolf" dismissal training |
| Learning model | Persistent + confirmable | System learns over time, user can review/edit rules |

## Architecture

```
ActivityTracker (existing)
  → DistractionAlert (mpsc)
  → DistractionInterceptor (NEW)
      1. Check session whitelist → skip if allowed
      2. Check learned rules → auto-allow with indicator
      3. Run title heuristics → confident? decide immediately
      4. Ambiguous? → show overlay + async LLM classify
      5. Emit InterventionEvent to frontend
  → Overlay Window (Tauri webview, always-on-top, frameless)
  → User choice → IPC command → DistractionLearner updates state
```

Existing `ActivityTracker` and `DistractionAlert` channel are untouched. `DistractionInterceptor` sits between the alert channel and UI.

## Overlay Window

Separate Tauri webview window (`distraction-overlay`), created on-demand.

**Properties:** ~420x280px, always-on-top, frameless, transparent background with glass blur, cannot be minimized.

**Layout:**
- Warning header ("Focus Session Active")
- App name + window title excerpt
- LLM verdict line (starts "Analyzing..." for ambiguous, updates async)
- Three action buttons: "Back to work", "Allow 5 min", "This is work-related"
- Session timer context

**Button behaviors:**
- "Back to work" — close overlay, record distraction, re-trigger after cooldown (60s) if still on app
- "Allow 5 min" — close overlay, start in-memory countdown, re-trigger after expiry
- "This is work-related" — close overlay, whitelist title for session, record as learning candidate

## Heuristic Classifier

Fast synchronous check before showing overlay:

**Confident distracting** (show overlay immediately, no LLM):
- App names: Netflix, TikTok, Instagram, Twitter/X, Twitch
- Title keywords: "reddit.com", "Facebook", "Hacker News"

**Confident productive** (skip overlay entirely):
- Title keywords: "Stack Overflow", "MDN", "docs.rs", "GitHub Issues"
- Matches a learned rule classified as work-related

**Ambiguous** (show overlay + async LLM):
- YouTube (any title)
- Reddit (could be technical or entertainment)
- Mixed signals in title

## LLM Classifier

Fires async for ambiguous cases only.

- Uses cheapest/fastest available model (Haiku) via existing `providers` crate
- Input: window title + focus session context
- Output: `educational | work_research | entertainment | social_media`
- Timeout: 3 seconds — shows "Unable to classify" on timeout
- Result pushed to overlay via Tauri event, updating verdict text
- Informational only — user always makes the final decision

## Learning System

Three tiers of memory:

| Tier | Storage | Lifetime |
|------|---------|----------|
| Session whitelist | In-memory `HashSet` | Current focus session |
| Temporary pass | In-memory with `Instant` expiry | 5 minutes (configurable) |
| Learned rules | SQLite table | Persistent across sessions |

**New table:**
```sql
CREATE TABLE distraction_learned_rules (
    id INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL,
    pattern_type TEXT NOT NULL,     -- 'title_keyword' | 'app_name' | 'url_pattern'
    classification TEXT NOT NULL,   -- 'educational' | 'work_research'
    confidence REAL DEFAULT 0.5,
    hit_count INTEGER DEFAULT 1,
    last_used_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

**Auto-learning:** When user marks "work-related" for the same pattern 3+ times across different sessions, promote to persistent learned rule. Confidence increases with each confirmation.

**Settings panel:** Productivity settings UI section to view, edit, and delete learned rules.

## Config

New fields in `FocusConfig`:

```rust
pub soft_block_enabled: bool,        // existing, now wired up (default: true)
pub soft_block_cooldown_secs: u64,   // default: 60
pub soft_block_temp_pass_mins: u64,  // default: 5
pub soft_block_llm_enabled: bool,    // default: true
pub soft_block_llm_timeout_ms: u64,  // default: 3000
pub learned_rule_threshold: u64,     // default: 3
```

## New IPC Commands

| Command | Purpose |
|---------|---------|
| `distraction_dismiss` | "Back to work" — close overlay, record distraction |
| `distraction_allow_temp` | "Allow 5 min" — close overlay, start temp pass |
| `distraction_allow_session` | "This is work-related" — whitelist + learning candidate |
| `distraction_learned_rules` | List learned rules (settings panel) |
| `distraction_delete_rule` | Delete a learned rule |

## Crate Placement

- `DistractionInterceptor` + `DistractionLearner` + heuristics → `feature-productivity::distraction` (new module)
- `DistractionClassifierHandler` trait → `feature-productivity` (dependency inversion)
- LLM classifier implementation → `desktop` crate (implements handler trait)
- Overlay window management + IPC commands → `desktop` crate
- Overlay UI → `desktop-ui` (new component)

## Not Building (YAGNI)

- Hard blocking (no app-killing or website blocking)
- URL extraction from browsers (window title is sufficient)
- Cross-platform (macOS only, matching existing tracker)
- Break session interventions
- Native notification fallback
- Distraction pattern analytics UI
