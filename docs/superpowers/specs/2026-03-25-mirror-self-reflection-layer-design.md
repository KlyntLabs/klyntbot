# The Mirror — Self-Reflection Layer for Klyntbot

**Date:** 2026-03-25
**Status:** Design approved, ready for implementation planning
**Origin:** Hyperagents (arXiv:2603.19461) comparative analysis — extracting meta-level self-improvement patterns into a safe, user-visible, second-brain-native feature

---

## Vision

> "Klyntbot doesn't just help you work. It watches *how* you work, spots patterns, tests improvements, and shows you the mirror."

The Mirror is a self-awareness layer inside the cognitive system that turns invisible optimization into a personal, interactive journal. It surfaces how the brain routes messages, proposes rules about its own thinking, previews experiments, tracks its configuration history, and generates weekly narratives — all in warm, first-person language that feels like a trusted inner voice.

## Core Principles

1. **Personal, not technical** — the user sees stories, not metrics. "I misunderstood you less often this week" not "correction_rate decreased 12%."
2. **Collaborative, not autonomous** — meta-rules start pending, experiments show previews, the user approves or dismisses. The brain proposes; the human decides.
3. **Alive, not scheduled** — event-driven architecture means the Mirror updates in real-time as the brain evolves, not just on nightly runs.
4. **Connected, not isolated** — every Mirror action ripples into Notes, Episodic memory, Coaching, and Tasks. One decision enriches the whole second brain.

---

## Prerequisites: New Domain Events

The Mirror depends on two events that do not yet exist in `DomainEvent`. These must be added to `crates/bus/src/domain_events.rs` before any Mirror subscriber can function.

### `SkillRouted` (new variant)

Emitted by `AgentRuntime` after skill selection (step 1) and intent classification (step 5):

```rust
SkillRouted {
    skill_name: String,
    confidence: f64,
    source: String,              // "heuristic" | "embedding" | "llm" | "cognitive"
    trigger_phrases: Vec<String>,
    session_key: String,
}
```

**Emit site:** `crates/agent/src/agent_runtime/runtime.rs`, after `select_orchestrator()` returns and intent classification completes. This is the same point where `AgentSelected` transparency event is already emitted — add a `DomainEvent::SkillRouted` publish alongside it.

### `TrialActivated` (new variant)

Emitted when the autotuner activates a new trial for shadow evaluation:

```rust
TrialActivated {
    trial_id: String,
    hypothesis: String,
    params_summary: String,      // human-readable summary of changed params
}
```

**Emit site:** `crates/agent/src/autotuner/mod.rs`, in the trial activation path where new trials are registered. Currently only `AutotunerDecision` is emitted on promotion — this new event fires on trial *start*.

---

## Architecture

### Home: `crates/cognitive/src/mirror/`

The Mirror extends the cognitive crate (L3-L4) with a dedicated `mirror/` module. This is the natural home — the Mirror IS cognitive self-reflection. It reuses existing repos, event bus patterns, and trait-injection for cross-layer access.

### Approach: Event-Driven Pipeline + Thin Facade

Four event subscribers accumulate data reactively. A narrative generator synthesizes weekly. A facade provides the clean API for UI and MCP.

```
mirror/
├── engine.rs          # MirrorEngine — starts subscribers, owns lifecycle
├── subscribers/
│   ├── routing.rs     # RoutingMirrorSubscriber (SkillRouted events)
│   ├── trial.rs       # TrialPreviewSubscriber (TrialActivated + 4h timer)
│   ├── meta_rule.rs   # MetaRuleDetector (UserCorrectedAI + correction streaks)
│   └── version.rs     # ConfigArchiver (AutotunerDecision::Promoted)
├── narratives.rs      # TrendNarrativeGenerator + alert snippet templates
├── types.rs           # All Mirror types
├── repo.rs            # MirrorRepo — shared tables
└── facade.rs          # MirrorFacade — public API
```

### Data Flow

```
DomainEventBus
  ├── SkillRouted (new)      → RoutingMirrorSubscriber → mirror_routing_snapshots
  ├── TrialActivated (new)   → TrialPreviewSubscriber  → mirror_trial_previews
  ├── UserCorrectedAI        → MetaRuleDetector         → mirror_meta_rules
  ├── AutotunerDecision      → ConfigArchiver            → mirror_brain_versions
  └── MirrorAlert (any)      → NarrativeSnippet          → mirror_snippets

Weekly Cron → NarrativeGenerator → mirror_trend_narratives

MirrorFacade → MirrorRepo → All tables above
MirrorFacade → Mirror Tab UI (via Tauri commands)
```

---

## Data Model

### Core Types (`mirror/types.rs`)

```rust
/// Aggregate state for the Mirror tab — one call for the whole UI
pub struct MirrorState {
    pub last_routing_snapshot: Option<RoutingSnapshot>,
    pub recent_trial_previews: Vec<TrialPreview>,
    pub latest_brain_version: Option<BrainVersion>,
    pub latest_trend_narrative: Option<TrendNarrative>,
    pub active_meta_rules: Vec<MetaRule>,
    pub pending_snippets: Vec<NarrativeSnippet>,
}

/// Point-in-time snapshot of how the agent routes messages across skills
pub struct RoutingSnapshot {
    pub id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub window_hours: u8,                         // 1h, 24h, or 168h
    pub total_messages: u32,
    pub distribution: HashMap<String, SkillRouteStats>,
    pub fallback_rate: f64,                       // % routed to "general" as fallback
    pub avg_routing_confidence: f64,
    pub low_confidence_count: u32,
    pub user_feedback: Option<UserFeedback>,
}

pub struct SkillRouteStats {
    pub count: u32,
    pub percentage: f64,
    pub avg_confidence: f64,
    pub top_triggers: Vec<String>,
}

/// 4-hour early evaluation of an autotuner trial
pub struct TrialPreview {
    pub trial_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub preview_at: DateTime<Utc>,
    pub messages_scored: u32,
    pub early_signals: TrialEarlySignals,
    pub recommendation: PreviewRecommendation,    // Continue | Kill | NeedMoreData
    pub narrative: String,
}

pub struct TrialEarlySignals {
    pub correction_rate_delta: f64,
    pub confidence_trend: TrendDirection,          // Rising | Falling | Stable
    pub dominant_skill_shift: Option<String>,
}

/// Snapshot of a promoted champion configuration
pub struct BrainVersion {
    pub version: u32,
    pub trial_id: Option<Uuid>,
    pub promoted_at: DateTime<Utc>,
    pub params: serde_json::Value,                // full TrialParams snapshot
    pub reason: String,
    pub parent_version: Option<u32>,
    pub metrics_at_promotion: serde_json::Value,
    pub reverted: bool,
}

/// Weekly synthesis combining all mirror data
pub struct TrendNarrative {
    pub id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
    pub experiment_summary: String,
    pub meta_rule_updates: Vec<String>,
    pub full_narrative: String,                   // LLM-generated, first-person
    pub user_feedback: Option<UserFeedback>,
}

/// Real-time alert card surfaced in the Mirror tab
pub struct NarrativeSnippet {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub alert_type: MirrorAlertType,
    pub headline: String,
    pub body: String,
    pub suggested_action: Option<SuggestedAction>,
    pub user_feedback: Option<UserFeedback>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

pub enum SuggestedAction {
    BoostSkill { skill: String },
    KillTrial { trial_id: Uuid },
    ContinueTrial { trial_id: Uuid },
    ApproveMetaRule { rule_id: Uuid },
    DismissMetaRule { rule_id: Uuid },
    RevertBrainVersion { version: u32 },
    ViewDetails,
}

pub enum UserFeedback { Helpful, NotHelpful, Dismissed }

/// Meta-rule — stored in dedicated mirror_meta_rules table (not procedural_rules)
/// because the structured action enum and pending/active state machine don't fit
/// the simple rule_text + confidence schema of procedural_rules.
pub struct MetaRule {
    pub id: Uuid,
    pub trigger_condition: String,
    pub action: MetaRuleAction,
    pub source: MetaRuleSource,                   // UserCreated | ReflectionGenerated | CorrectionDerived
    pub effectiveness_score: f64,
    pub status: MetaRuleStatus,                   // Pending | Active | Disabled
    pub signal_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum MetaRuleAction {
    AdjustRouting { skill: String, direction: String },
    ForceClarification,
    SwitchMode { mode: String },
    CreateExperiment { hypothesis: String },
    SurfaceInsight { message: String },
    Custom { payload: serde_json::Value },        // extensible
}

pub enum MetaRuleSource { UserCreated, ReflectionGenerated, CorrectionDerived }
pub enum MetaRuleStatus { Pending, Active, Disabled }
```

### Storage (`mirror/repo.rs`)

Added as cognitive feature migration (update version in-place per pre-release convention):

```sql
CREATE TABLE IF NOT EXISTS mirror_routing_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    total_messages INTEGER NOT NULL,
    distribution_json TEXT NOT NULL,
    fallback_rate REAL NOT NULL,
    avg_routing_confidence REAL NOT NULL,
    low_confidence_count INTEGER NOT NULL DEFAULT 0,
    user_feedback TEXT
);
CREATE INDEX idx_routing_snapshots_time ON mirror_routing_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS mirror_trial_previews (
    id TEXT PRIMARY KEY,
    trial_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    preview_at TEXT NOT NULL,
    messages_scored INTEGER NOT NULL,
    early_signals_json TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    narrative TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mirror_brain_versions (
    version INTEGER PRIMARY KEY,
    trial_id TEXT,
    promoted_at TEXT NOT NULL,
    params_json TEXT NOT NULL,
    reason TEXT NOT NULL,
    parent_version INTEGER REFERENCES mirror_brain_versions(version),
    metrics_json TEXT NOT NULL,
    reverted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS mirror_trend_narratives (
    id TEXT PRIMARY KEY,
    generated_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    routing_summary TEXT NOT NULL,
    improvement_highlights_json TEXT NOT NULL,
    experiment_summary TEXT NOT NULL,
    meta_rule_updates_json TEXT NOT NULL,
    full_narrative TEXT NOT NULL,
    user_feedback TEXT
);
CREATE INDEX idx_trend_narratives_time ON mirror_trend_narratives(generated_at);

CREATE TABLE IF NOT EXISTS mirror_meta_rules (
    id TEXT PRIMARY KEY,
    trigger_condition TEXT NOT NULL,
    action_json TEXT NOT NULL,             -- serialized MetaRuleAction enum
    source TEXT NOT NULL,                  -- 'user_created' | 'reflection_generated' | 'correction_derived'
    effectiveness_score REAL NOT NULL DEFAULT 0.5,
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending' | 'active' | 'disabled'
    signal_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_meta_rules_status ON mirror_meta_rules(status);

CREATE TABLE IF NOT EXISTS mirror_snippets (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    alert_type TEXT NOT NULL,
    headline TEXT NOT NULL,
    body TEXT NOT NULL,
    action_json TEXT,
    user_feedback TEXT,
    dismissed_at TEXT
);
CREATE INDEX idx_snippets_created ON mirror_snippets(created_at);
```

**Meta-rules** get their own `mirror_meta_rules` table rather than reusing `procedural_rules`. The structured `MetaRuleAction` enum (with variants like `AdjustRouting`, `ForceClarification`, `CreateExperiment`) and the pending/active/disabled state machine don't fit the simple `rule_text + confidence` schema of procedural rules. However, active meta-rules are still loaded alongside procedural rules during context assembly via a unified query in `MirrorRepo`.

**Retention policy:** Hourly routing snapshots kept 30 days, daily aggregates kept forever. Trial previews kept 90 days. Snippets kept 90 days (dismissed ones cleaned after 30 days).

---

## Subscribers

### RoutingMirrorSubscriber (`subscribers/routing.rs`)

**Listens to:** `SkillRouted` (new event, see Prerequisites)

**Behavior:**
- In-memory `DashMap<String, SkillRouteAccum>` accumulates classification events (counts, confidence sums, trigger phrase frequencies)
- Hourly flush via `tokio::time::interval(3600s)` writes a `RoutingSnapshot` with `window_hours=1`
- **Daily aggregation:** A separate `tokio::time::sleep_until(next_midnight)` task (computed on startup via `chrono::Local` to align to midnight) writes a `window_hours=24` aggregate snapshot summarizing the day, then reschedules for the next midnight. This avoids relying on the hourly interval coinciding with midnight. Weekly aggregates (`window_hours=168`) are computed by the narrative generator from daily snapshots.
- **Drift detection** on each flush: compares current distribution to rolling 7-day average (from daily aggregates). If any skill's share shifted >15 percentage points or `fallback_rate` exceeds 0.70, emits `MirrorAlert::RoutingDrift`
- **No cleanup in flush** — retention cleanup is owned exclusively by `JOB_MIRROR_CLEANUP` cron to avoid dual code paths

### TrialPreviewSubscriber (`subscribers/trial.rs`)

**Listens to:** `TrialActivated` (new event, see Prerequisites)

**Behavior:**
- On `TrialActivated`, spawns a 4-hour delayed task
- After 4h, queries early metrics via `EarlyTrialEvaluator` trait (defined in cognitive, implemented in agent/app-core)
- **Recommendation logic:**
  - `Continue` — correction_rate_delta > 0 and confidence trend Rising/Stable
  - `Kill` — correction_rate_delta < -0.10 or insufficient data trending badly
  - `NeedMoreData` — everything else
- Writes `TrialPreview` and emits `MirrorAlert::TrialUnpromising` if Kill
- **Kill does NOT auto-stop** — surfaces the recommendation. User or nightly cycle decides.
- **Concurrency:** Active timers stored in `Arc<DashMap<Uuid, JoinHandle<()>>>` shared between the subscriber task and `MirrorFacade` (which needs to cancel timers on `kill_trial`). The `DashMap` is created during `MirrorEngine` construction and cloned into both the subscriber and the facade.

### MetaRuleDetector (`subscribers/meta_rule.rs`)

**Listens to:** `UserCorrectedAI`, `SkillRouted` (low confidence), `MirrorAlert`

**Behavior:**
- **Correction streak detection:** ≥2 corrections in one session, or ≥3 corrections involving same skill across sessions → propose meta-rule
- **Low-confidence streak:** confidence < 0.4 for 3+ consecutive messages → propose "force clarification"
- **Drift response:** `MirrorAlert::RoutingDrift` → propose routing adjustment rule
- **Heuristic templates** for common patterns (no LLM needed). LLM fallback via `MetaRuleProposer` trait for novel patterns
- **All proposed rules start in "pending" state** in `mirror_meta_rules` — surfaced in Mirror tab as "I think I should... Sound good?" with Approve / Tweak / Dismiss
- On user Approve, status changes to `active` in `mirror_meta_rules` (NOT `procedural_rules` — meta-rules live exclusively in their dedicated table)
- **Effectiveness tracking:** starts at confidence=0.5. Pattern recurrence with rule active → signal_count++. Continued corrections despite rule → confidence decays. Rules with confidence < 0.1 after 30 days auto-deactivated.

### ConfigArchiver (`subscribers/version.rs`)

**Listens to:** `AutotunerDecision::Promoted`

**Behavior:**
- On promotion, increments version counter and writes `BrainVersion` with full `TrialParams` snapshot
- `parent_version` is always the current latest version (linear chain)
- **Bootstrap:** First run creates Version 1 from current config defaults with reason "Initial brain state"
- **Revert logic** (via facade): marks versions after target as `reverted=true`, creates a NEW version with the target's params (timeline always moves forward). Applies params via `AutotunerBridge` trait.
- **Reconciliation with existing revert:** `app-core/src/handlers/autotuner.rs` has an existing `autotuner_revert()` that calls `orch.update_champion(prev_champion)` directly, bypassing any bridge. This must be modified:
  1. Refactor `autotuner_revert()` to call through `AutotunerBridge::apply_champion()` instead of calling `orch.update_champion()` directly
  2. `AutotunerBridge::apply_champion()` implementation must both update the champion in the orchestrator AND write a `BrainVersion` to `mirror_brain_versions`
  3. The existing autotuner revert UI should be deprecated in favor of the Mirror's richer timeline-based revert
  4. Until deprecated, the refactored `autotuner_revert()` will naturally create `BrainVersion` entries since it goes through the bridge

### Subscriber Lifecycle

All four started by `MirrorEngine` during app initialization. The engine produces both the subscriber tasks and the `MirrorFacade`, sharing state between them:

```rust
pub struct MirrorEngine { ... }

impl MirrorEngine {
    /// Build and start the Mirror system. Returns the facade (for UI/Tauri)
    /// and the subscriber join handles (for lifecycle management).
    pub fn start(
        self,
        bus: &DomainEventBus,
        repo: MirrorRepo,
        narrative_handler: Arc<dyn NarrativeHandler>,
        autotuner_bridge: Arc<dyn AutotunerBridge>,
    ) -> (MirrorFacade, Vec<JoinHandle<()>>) {
        let shutdown = CancellationToken::new();

        // Shared state between subscribers and facade
        let active_timers: Arc<DashMap<Uuid, JoinHandle<()>>> = Arc::new(DashMap::new());

        let handles = vec![
            tokio::spawn(self.routing.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(self.trial.run(bus.subscribe(), active_timers.clone(), shutdown.clone())),
            tokio::spawn(self.meta_rule.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(self.version.run(bus.subscribe(), shutdown.clone())),
        ];

        let facade = MirrorFacade {
            repo,
            narrative_handler,
            autotuner_bridge,
            active_timers,  // shared with TrialPreviewSubscriber
            shutdown,
        };

        (facade, handles)
    }
}
```

Each subscriber's `run()` accepts a `CancellationToken` for clean shutdown. The facade and `TrialPreviewSubscriber` share the `active_timers` DashMap for trial cancellation.

---

## Narrative Generation

### Weekly Synthesis

**Trigger:** Cron job (`JOB_MIRROR_WEEKLY_NARRATIVE`), Sunday 7pm local time.

**Flow:** Loads 7 days of routing snapshots, trial previews, brain versions, meta-rule activity, episodic memories (corrections, quality scores). Builds `NarrativeContext` including `past_narrative_feedback` (last 3 user feedbacks for adaptive tone). Calls `NarrativeHandler` trait (defined in cognitive, implemented in agent).

**LLM prompt tone:** First person ("I noticed...", "I struggled with..."). Warm, specific, honest. No jargon. Translates all metrics into human terms. Adapts voice based on past feedback.

**Structure:**
1. Opening: one sentence capturing the week's vibe
2. Routing: how message understanding changed
3. Experiments: what was tested, what worked
4. Meta-rules: new self-awareness insights
5. Looking ahead: one concrete thing to watch next week

**Output:** `TrendNarrative` saved to `mirror_trend_narratives`. Also saved as episodic memory tagged "mirror-reflection" with importance=0.9 for natural salience decay and searchability.

### Alert Snippets

Every `MirrorAlert` is immediately converted to a `NarrativeSnippet` via templates (no LLM). Examples:

- **RoutingDrift:** "I'm routing more to {skill} lately — your {skill} usage shifted {delta}%. Want me to lean into this?"
- **TrialUnpromising:** "An experiment isn't looking great after 4 hours. Want to kill it early or let it finish?"
- **MetaRuleProposed:** "I learned something about how I think: '{rule_text}'. Does this sound right?"

Snippets stored in `mirror_snippets` table, surfaced as real-time cards via SSE events.

### Conversational Mirror

User types questions in the MirrorInput box → calls `MirrorFacade::generate_mirror_response(query, period)` → loads relevant mirror data + routing history → LLM generates first-person answer that can also propose meta-rules on the spot.

---

## Trait Boundaries (Dependency Inversion)

| Trait | Defined in | Implemented in | Purpose |
|-------|-----------|---------------|---------|
| `NarrativeHandler` | cognitive | agent | LLM calls for weekly narrative + conversational responses |
| `MetaRuleProposer` | cognitive | agent | LLM fallback for novel meta-rule patterns |
| `EarlyTrialEvaluator` | cognitive | app-core | Query autotuner metrics at 4h mark |
| `AutotunerBridge` | cognitive | app-core | Apply champion params on revert, kill trials |

---

## MirrorFacade — Public API

```rust
pub struct MirrorFacade {
    repo: MirrorRepo,
    narrative_handler: Arc<dyn NarrativeHandler>,
    autotuner_bridge: Arc<dyn AutotunerBridge>,
    active_timers: Arc<DashMap<Uuid, JoinHandle<()>>>,  // shared with TrialPreviewSubscriber
    shutdown: CancellationToken,
}

impl MirrorFacade {
    // State queries
    pub async fn get_state(&self) -> Result<MirrorState>;
    pub async fn get_routing_history(&self, days: u32) -> Result<Vec<RoutingSnapshot>>;
    pub async fn get_brain_versions(&self) -> Result<Vec<BrainVersion>>;
    pub async fn get_pending_snippets(&self) -> Result<Vec<NarrativeSnippet>>;
    pub async fn get_narratives(&self, limit: u32) -> Result<Vec<TrendNarrative>>;

    // User actions
    pub async fn revert_to_version(&self, target: u32) -> Result<BrainVersion>;
    pub async fn approve_meta_rule(&self, rule_id: Uuid) -> Result<()>;
    pub async fn dismiss_meta_rule(&self, rule_id: Uuid) -> Result<()>;
    pub async fn kill_trial(&self, trial_id: Uuid) -> Result<()>;
    pub async fn continue_trial(&self, trial_id: Uuid) -> Result<()>;
    pub async fn submit_feedback(&self, item_id: Uuid, target: FeedbackTarget, feedback: UserFeedback) -> Result<()>;
    pub async fn create_meta_rule_from_text(&self, text: String) -> Result<MetaRule>;

    // On-demand (conversational mirror)
    pub async fn generate_mirror_response(
        &self, query: String, period: Option<(DateTime<Utc>, DateTime<Utc>)>
    ) -> Result<MirrorResponse>;
}

/// Conversational response from the Mirror — distinct from TrendNarrative
/// because it answers a freeform question rather than synthesizing a period.
pub struct MirrorResponse {
    pub answer: String,                          // first-person LLM response
    pub data_sources_used: Vec<String>,          // which mirror tables were queried
    pub proposed_meta_rule: Option<MetaRule>,     // if the answer surfaced a new rule
}
```

**Feedback routing:** `submit_feedback(item_id, target, feedback)` uses a `FeedbackTarget` enum to disambiguate which table to update:

```rust
pub enum FeedbackTarget {
    Narrative,    // mirror_trend_narratives
    Snippet,      // mirror_snippets
    Routing,      // mirror_routing_snapshots
}

pub async fn submit_feedback(
    &self, item_id: Uuid, target: FeedbackTarget, feedback: UserFeedback
) -> Result<()>;
```

This replaces the earlier untyped `submit_feedback(item_id, feedback)` which had no way to determine the target table.
```

---

## Integration Points

**Tauri commands** (`desktop/src/commands/mirror.rs`): Thin adapters delegating to `MirrorFacade`. Must export `DEV_COMMANDS` for dev server coverage test.

**Cron jobs** (`app-core/src/init/cron.rs`):
- `JOB_MIRROR_WEEKLY_NARRATIVE` — Sunday 7pm local, generates weekly narrative
- `JOB_MIRROR_CLEANUP` — Daily midnight, retention cleanup (30d hourly snapshots, 90d previews/snippets)

**MCP exposure:** Implement a `MirrorTool` via `#[derive(Tool)]` in the cognitive or a thin wrapper crate, registered in `ToolRegistry` via `FeaturePackage::tools()`. The tool name `mirror` is added to `default_exposed_tools()` in `config/schema/mcp.rs`. Actions: `get_state`, `get_routing_history`, `get_brain_versions`, `get_narratives` (all read-only). The tool delegates to `MirrorFacade` methods.

**Cross-feature ripple:**
- User kills trial → auto-creates note ("Decided to kill trial X because...") + episodic memory
- User approves meta-rule → logged as episodic memory with importance=0.8
- Weekly narrative → saved as episodic memory tagged "mirror-reflection"
- Coaching engine can pull latest `TrendNarrative` for context

---

## UI Surface

The Mirror is a **top-level sidebar item** (peer to Cognitive, Tasks, Finance), not a sub-tab. Icon: reflective orb. This gives the feature the emotional weight it deserves.

### Components (`desktop-ui/src/features/mirror/`)

```
MirrorPage
├── MirrorHeader              — title + last updated
├── NarrativeCard             — latest weekly narrative (first-person)
│   ├── FeedbackButtons       — Helpful / Not Helpful / tone feedback
│   └── ExpandableDetails     — routing summary, highlights, experiments
├── SnippetFeed               — live feed of alert cards (SSE-updated)
│   └── SnippetCard           — headline + body + warm action button
├── RoutingDonut              — skill distribution with hover tooltips + trend arrows
├── BrainTimeline             — vertical timeline of brain versions
│   ├── VersionCard           — "Version {n} — {reason}" + metrics delta
│   │   └── RevertButton      — confirmation modal explaining what changes
│   └── RevertedBadge         — greyed-out styling
├── MetaRulesSection          — active rules (with effectiveness badge) + pending proposals
│   ├── ActiveRuleCard        — rule text + disable toggle
│   └── PendingRuleCard       — "I think I should..." + Approve / Tweak / Dismiss
└── MirrorInput               — "Ask me about how I think..."
    └── on submit → ipc("generate_mirror_response", { query, period })
```

**Data fetching:** `useQuery("get_mirror_state")` on mount. Individual sections use targeted queries when expanded.

**Real-time updates:** Requires three additions to the existing event infrastructure:
1. Add `MirrorSnippetCreated { snippet_id: String, headline: String }` and `MirrorBrainVersionCreated { version: u32 }` variants to `DomainEvent` in `crates/bus/src/domain_events.rs`
2. Add `MirrorSnippet` and `BrainVersion` variants to `EntityKind` in `crates/desktop-shared/src/types.rs`
3. In `app-core/src/events.rs`, map the new `DomainEvent` variants to `emit_entity_updated(EntityKind::MirrorSnippet, snippet_id)` calls

The frontend subscribes to entity-update SSE events filtered by `EntityKind::MirrorSnippet` to append new snippet cards without page refresh. This follows the same pattern as `TaskUpdated` → `EntityKind::Task` → frontend `useQuery` invalidation.

**Styling:** `glass-panel` for cards, theme tokens for colors, skill-specific donut colors derived from skill name hash.

---

## Testing Strategy

### Unit Tests (cognitive crate)

| Component | Tests |
|-----------|-------|
| RoutingMirrorSubscriber | Accumulation, hourly flush, drift detection at 15pp threshold |
| MetaRuleDetector | Correction streak detection, low-confidence streaks, heuristic templates |
| ConfigArchiver | Version numbering, revert marks reverted + creates new version |
| TrialPreviewSubscriber | Early signal computation, Continue/Kill/NeedMoreData thresholds |
| NarrativeSnippet templates | Alert → snippet conversion for each MirrorAlert variant |
| MirrorRepo | CRUD for all mirror tables, retention cleanup |
| MirrorFacade | get_state() composition, revert_to_version() end-to-end |

### Integration Tests (tests/integration/)

| Test | Proves |
|------|--------|
| mirror_routing_accumulation | Events → subscriber → repo → facade query |
| mirror_meta_rule_lifecycle | Corrections → proposal (pending) → approve → active → effectiveness |
| mirror_brain_version_revert | Promotion → version → revert → new version with old params |
| mirror_weekly_narrative | Full synthesis from seeded data → NarrativeHandler called correctly |

### Frontend Tests (Vitest)

- MirrorPage renders with mock MirrorState
- SnippetCard action buttons call correct IPC commands
- BrainTimeline revert shows confirmation modal
- RoutingDonut renders correct percentages

---

## Phasing

| Phase | Features | Delivers |
|-------|----------|----------|
| **1** | RoutingMirrorSubscriber + NarrativeGenerator + MirrorFacade (partial) + Mirror tab (routing donut + narrative card + snippet feed + MirrorInput) | "My brain watches how it thinks and I can ask it why" |
| **2** | MetaRuleDetector + pending approval flow + MetaRulesSection UI | "My brain proposes rules about itself" |
| **3** | ConfigArchiver + BrainTimeline + revert flow | "I can travel through my brain's history" |
| **4** | TrialPreviewSubscriber + experiment watchlist + kill/continue buttons | "I can steer experiments in real-time" |
| **5** | Adaptive tone + cross-feature ripple (auto-notes, episodic entries, coaching integration) | "The Mirror connects everything" |

Each phase is independently shippable and delivers a visible "wow" moment.
