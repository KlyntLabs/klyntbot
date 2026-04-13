# Consolidated Brain Page & AI Settings

**Date:** 2026-04-13
**Status:** Draft

## Summary

Consolidate 3 separate AI pages (Coaching, Brain/Mirror, System tabs) into a single "Brain" dashboard page with card-grid + in-page expansion navigation. Consolidate scattered AI settings (General, Personalization) into a single "AI" tab in Settings. Remove the System sidebar item by moving Categories to Settings.

## Current State (What We're Replacing)

### Pages being consolidated:
- **Coaching** (`/coaching`) — 3 sub-tabs: Overview (gauges, signals, interventions, patterns, strategy feedback, router status), Patterns, History
- **Brain/Mirror** (`/brain`) — Weekly reflection, recent insights, experiment watchlist, meta-rules, brain versions timeline, skill routing donut, mirror conversational input
- **System** (`/system`) — 6 tabs: Contexts, Categories, Inference, Memory, Events, Pipeline

### Settings being consolidated:
- **General** → agent defaults (model, temperature, maxTokens), autotuner panel
- **Personalization** → provider keys, cognitive model/mode, learning config, provider routing

## New Brain Page (`/brain`)

### Layout: Card Grid with In-Page Expansion

**Default view (Overview):** Health summary strip + 2x2 card grid + collapsible activity stream.

#### Health Summary Strip (top)
4 metric cards always visible:
- **Knowledge Trust** — percentage + fact/episodic counts
- **Brain Version** — current version + date
- **Coaching** — status (Active/Paused) + focus summary + pending intervention count
- **Experiments** — active trial count

#### Card Grid (2x2)

**1. Memory & Knowledge** (green accent)
- Icon: brain
- Summary metrics: active facts, episodic memories, procedural rules
- Domain pills showing fact distribution (energy, work, identity, etc.)
- **Detail view contains:** User model domain cards, semantic facts table (with Add Fact, delete), episodic memories list, procedural rules list (with deactivate), Run Reflection + Run Compaction actions
- **Tauri commands:** `cognitive_user_model`, `cognitive_facts_list`, `cognitive_episodic_list`, `cognitive_rules_list`, `cognitive_memory_stats`, `cognitive_run_compaction`, `cognitive_run_reflection`, `cognitive_fact_delete`, `cognitive_rule_deactivate`

**2. Coaching & Patterns** (blue accent)
- Icon: target
- Summary: compact situation gauges (energy, focus, deadline, receptivity), pending intervention count, pattern count
- **Detail view contains:** Full-size situation gauges (5 gauges + hours active, break time, context switches), active interventions with feedback buttons, signal accumulator with clear action, detected patterns with confidence, intervention router (hourly/daily limits), strategy feedback table with reset, recent interventions log
- **Polling:** 5-second interval for situation data (only when detail view is expanded)
- **Tauri commands:** `coaching_situation`, `coaching_signals`, `coaching_patterns`, `coaching_feedback_stats`, `coaching_router_status`, `coaching_pending_interventions`, `coaching_intervention_log`, `coaching_clear_signals`, `coaching_reset_dismissals`, `coaching_submit_feedback`

**3. Mirror & Reflection** (purple accent)
- Icon: mirror
- Summary: latest reflection excerpt (or placeholder), brain version, meta-rule count, trial count
- **Detail view contains:** Weekly reflection with helpful/not-helpful feedback, recent insight snippets, experiment watchlist (kill/continue trials), meta-rules (pending: approve/dismiss, active list), brain versions timeline (with revert), skill routing distribution donut chart, mirror conversational input
- **Tauri commands:** `get_mirror_state`, `get_brain_versions`, `revert_brain_version`, `submit_mirror_feedback`, `approve_meta_rule`, `dismiss_meta_rule`, `kill_trial`, `continue_trial`

**4. Contexts & Inference** (amber accent)
- Icon: crystal ball
- Summary: active/archived context count, assignment rate, avg confidence
- **Detail view contains:** Context list with status filter (Active/Paused/Archived), context detail panel (switching to a context shows its resources, apps, time stats), context search dialog, inference stats (events 1h/24h, merges, last run)
- **Tauri commands:** `list_work_contexts`, `get_work_context_detail`, `search_work_contexts`, `get_inference_stats`

#### Activity Stream (bottom, collapsed by default)
Collapsible section labeled "Activity Stream" with sub-tabs or merged view:
- **Events** — domain event stream with salience/domain filters, expandable JSON payloads. Real-time via `cognitive:domain_event` custom event. Max 200 events in memory.
- **Pipeline** — two-column extraction log + consolidation log. Real-time via `cognitive:extraction` and `cognitive:consolidation` events.
- **Tauri commands:** `cognitive_event_log`, `cognitive_pipeline_log`

### Navigation Pattern

- **Overview → Detail:** Clicking a card expands it in-place. Other cards collapse/hide. Animated transition.
- **Detail → Overview:** ← back button in top-left of detail view. Animated collapse back to grid.
- **URL routing:** `/brain` (overview), `/brain/memory`, `/brain/coaching`, `/brain/mirror`, `/brain/contexts` (detail views). Direct URL navigation supported.
- **Activity stream:** Toggle at bottom of overview. Stays collapsed when navigating to/from detail views.

### Card Expansion Behavior

When a card is clicked:
1. Other cards fade out and collapse (200ms)
2. Selected card expands to fill the full content area (300ms, ease-out)
3. Card header transforms into detail header (← back + title + actions)
4. Detail content fades in (200ms, staggered)

On back:
1. Detail content fades out (150ms)
2. Card collapses back to grid position (300ms)
3. Other cards fade back in (200ms)

## New AI Settings Tab (`/settings/ai`)

### What moves here:
- From **General:** model, temperature, maxTokens, monthly budget (remove from General)
- From **Personalization:** cognitive config, learning config, provider routing (remove from Personalization)
- From **System > Inference:** inference config sliders (assignment threshold, merge threshold, weights)
- **AutoTuner** panel (from General)

### What stays elsewhere:
- **Provider API keys** → stay in Providers tab (set-once config, security boundary)
- **Voice** → stays in Voice tab (separate domain)

### Sections (in order):

**1. Agent Defaults**
- Model — dropdown populated from selected provider's model list
- Temperature — slider (0–2, default 0.7)
- Max Tokens — dropdown with common values (2048, 4096, 8192, 16384)
- Monthly Budget — number input with $ prefix

**2. Provider Routing**
- Primary Provider — dropdown of providers with configured API keys
- Fallback Provider — dropdown (same list)
- Classifier Model — dropdown of lightweight models

**3. Cognitive Pipeline**
- Intelligence Mode — toggle/select: Standard vs Deep
- Override Model — dropdown (falls back to agent model if unset)
- Temperature — slider (0–2, default 0.2)
- Atom Extraction — toggle
- **Advanced (collapsed):** retrieval weights (semantic, retrievability, importance, frequency, situation, temporal, recall_support, graph_path_boost), InsightForge toggle + limits, BookIndex config, query enhancement pipeline, history compression

**4. Learning & Adaptation**
- Enabled — toggle
- Analysis Interval — dropdown (15min, 30min, 1hr, 2hr, 4hr)
- Threshold Range — dual-handle slider (min 0.4, max 0.9)
- **Advanced (collapsed):** active recall settings (semantic thresholds, graph propagation)

**5. AutoTuner**
- Schedule — cron expression or friendly preset dropdown
- Min Messages for Promotion — number input
- Rollback After — dropdown (1-7 days)
- **Advanced (collapsed):** promotion constraints (correction improvement, token cost, response time, routing stability, memory relevance, retrieval precision, correction rate, promotion accuracy)

**6. Inference Engine**
- Assignment Threshold — slider (0–1)
- Merge Threshold — slider (0–1)
- Max Active Contexts — number input (5–100)
- Inference Interval — dropdown (1–30 min)
- Max Dormancy Days — dropdown (1–30)
- **Advanced (collapsed):** semantic/temporal/resource weights

### Input patterns:
- All model selectors are **dropdowns** populated from provider model lists, not free text
- All provider selectors are **dropdowns** of configured providers
- Thresholds use **sliders** with numeric readout
- Boolean settings use **toggles**
- Numeric settings use **dropdowns** with sensible presets where possible, **number inputs** otherwise
- Each section reads/writes via `config_get_section` / `config_update_section` with the appropriate section key

## Sidebar Changes

| Before | After |
|--------|-------|
| Chat | Chat |
| Dashboard | Dashboard |
| Tasks | Tasks |
| Notes | Notes |
| Learn | Learn |
| Finance | Finance |
| **Coaching** | **Removed** |
| **Brain** | **Brain** (consolidated) |
| Automations | Automations |
| **System** | **Removed** |
| Settings | Settings |

- **Coaching** sidebar item removed — content lives inside Brain > Coaching & Patterns
- **System** sidebar item removed — Memory/Events/Pipeline/Inference/Contexts moved to Brain, Categories moved to Settings
- **Brain** stays in the same sidebar position with same icon

## Settings Tab Changes

| Before | After |
|--------|-------|
| General (had model, temp, tokens, autotuner) | General (only keyboard shortcuts, system info) |
| Personalization (had providers, cognitive, learning, routing) | Personalization (stripped — maybe remove entirely if empty) |
| **New: AI** | Agent defaults, routing, cognitive, learning, autotuner, inference |
| Providers | Providers (API keys only — unchanged) |
| Voice | Voice (unchanged) |
| Configuration | Configuration (unchanged) |
| Work Contexts | Work Contexts (unchanged) |
| MCP Servers | MCP Servers (unchanged) |
| + Categories | **New** — moved from System page |

## Categories in Settings

The Categories page (productivity categories + tracked apps) moves from System to Settings as a new tab. It's configuration data — defining what apps count as "productive" — not a monitoring view. The 3-panel layout (category list, editor, tracked apps) stays the same, just hosted in the settings frame.

## What Gets Deleted

### Frontend files to remove:
- `desktop-ui/src/features/coaching/` — entire feature folder (components moved/rewritten into Brain page)
- `desktop-ui/src/features/mirror/` — entire feature folder (merged into Brain page)
- `desktop-ui/src/features/system/` — entire feature folder (tabs distributed to Brain + Settings)

### Routes to remove:
- `/coaching`, `/coaching/patterns`, `/coaching/history`
- `/system`, `/system/:tab`

### Settings sections to slim:
- General: remove model/temp/tokens/autotuner fields
- Personalization: remove cognitive/learning/routing sections (may become empty → remove tab)

## Implementation Notes

- All existing Tauri commands stay unchanged — only the frontend is reorganized
- No backend changes needed
- The 5-second polling for coaching data should only activate when the Coaching detail view is expanded, not on the overview
- Real-time event listeners (`cognitive:domain_event`, `cognitive:extraction`, `cognitive:consolidation`) only subscribe when Activity Stream is open
- Animation: use CSS transitions (`transition`, `@starting-style` where supported) and Tailwind utilities for card expansion. No animation library needed — CSS is sufficient for fade/scale/collapse transitions. Avoid adding Framer Motion as a new dependency.
- URL routing enables deep-linking: bookmarking `/brain/memory` opens directly to the Memory detail view
