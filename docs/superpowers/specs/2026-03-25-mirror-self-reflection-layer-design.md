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

## Architecture

### Home: `crates/cognitive/src/mirror/`

The Mirror extends the cognitive crate (L3-L4) with a dedicated `mirror/` module. This is the natural home — the Mirror IS cognitive self-reflection. It reuses existing repos, event bus patterns, and trait-injection for cross-layer access.

### Approach: Event-Driven Pipeline + Thin Facade

Four event subscribers accumulate data reactively. A narrative generator synthesizes weekly. A facade provides the clean API for UI and MCP.

```
mirror/
├── engine.rs          # MirrorEngine — starts subscribers, owns lifecycle
├── subscribers/
│   ├── routing.rs     # RoutingMirrorSubscriber (ClassificationComplete events)
│   ├── trial.rs       # TrialPreviewSubscriber (AutotunerDecision + 4h timer)
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
  ├── ClassificationComplete → RoutingMirrorSubscriber → mirror_routing_snapshots
  ├── AutotunerDecision      → TrialPreviewSubscriber  → mirror_trial_previews
  ├── UserCorrectedAI        → MetaRuleDetector         → procedural_rules (domain="meta")
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

/// Meta-rule stored as ProceduralRule with domain="meta"
pub struct MetaRule {
    pub trigger_condition: String,
    pub action: MetaRuleAction,
    pub source: MetaRuleSource,                   // UserCreated | ReflectionGenerated | CorrectionDerived
    pub effectiveness_score: f64,
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

**Meta-rules** reuse the existing `procedural_rules` table with `domain = "meta"`. No new table needed. Effectiveness tracked via existing `confidence` + `signal_count` fields.

**Retention policy:** Hourly routing snapshots kept 30 days, daily aggregates kept forever. Trial previews kept 90 days. Snippets kept 90 days (dismissed ones cleaned after 30 days).

---

## Subscribers

### RoutingMirrorSubscriber (`subscribers/routing.rs`)

**Listens to:** `ClassificationComplete`

**Behavior:**
- In-memory `DashMap<String, SkillRouteAccum>` accumulates classification events (counts, confidence sums, trigger phrase frequencies)
- Hourly flush via `tokio::time::interval(3600s)` writes a `RoutingSnapshot` with `window_hours=1`
- **Drift detection** on each flush: compares current distribution to rolling 7-day average. If any skill's share shifted >15 percentage points or `fallback_rate` exceeds 0.70, emits `MirrorAlert::RoutingDrift`
- Retention cleanup on flush: deletes hourly snapshots older than 30 days

### TrialPreviewSubscriber (`subscribers/trial.rs`)

**Listens to:** `AutotunerDecision` (trial activation/completion)

**Behavior:**
- On `TrialActivated`, spawns a 4-hour delayed task
- After 4h, queries early metrics via `EarlyTrialEvaluator` trait (defined in cognitive, implemented in agent/app-core)
- **Recommendation logic:**
  - `Continue` — correction_rate_delta > 0 and confidence trend Rising/Stable
  - `Kill` — correction_rate_delta < -0.10 or insufficient data trending badly
  - `NeedMoreData` — everything else
- Writes `TrialPreview` and emits `MirrorAlert::TrialUnpromising` if Kill
- **Kill does NOT auto-stop** — surfaces the recommendation. User or nightly cycle decides.
- Tracks active timers in `HashMap<Uuid, JoinHandle<()>>` for early cancellation

### MetaRuleDetector (`subscribers/meta_rule.rs`)

**Listens to:** `UserCorrectedAI`, `ClassificationComplete` (low confidence), `MirrorAlert`

**Behavior:**
- **Correction streak detection:** ≥2 corrections in one session, or ≥3 corrections involving same skill across sessions → propose meta-rule
- **Low-confidence streak:** confidence < 0.4 for 3+ consecutive messages → propose "force clarification"
- **Drift response:** `MirrorAlert::RoutingDrift` → propose routing adjustment rule
- **Heuristic templates** for common patterns (no LLM needed). LLM fallback via `MetaRuleProposer` trait for novel patterns
- **All proposed rules start in "pending" state** — surfaced in Mirror tab as "I think I should... Sound good?" with Approve / Tweak / Dismiss
- Only activates in `procedural_rules` on user Approve
- **Effectiveness tracking:** starts at confidence=0.5. Pattern recurrence with rule active → signal_count++. Continued corrections despite rule → confidence decays. Rules with confidence < 0.1 after 30 days auto-deactivated.

### ConfigArchiver (`subscribers/version.rs`)

**Listens to:** `AutotunerDecision::Promoted`

**Behavior:**
- On promotion, increments version counter and writes `BrainVersion` with full `TrialParams` snapshot
- `parent_version` is always the current latest version (linear chain)
- **Bootstrap:** First run creates Version 1 from current config defaults with reason "Initial brain state"
- **Revert logic** (via facade): marks versions after target as `reverted=true`, creates a NEW version with the target's params (timeline always moves forward). Applies params via `AutotunerBridge` trait.

### Subscriber Lifecycle

All four started by `MirrorEngine` during app initialization:

```rust
pub struct MirrorEngine {
    routing: RoutingMirrorSubscriber,
    trial: TrialPreviewSubscriber,
    meta_rule: MetaRuleDetector,
    version: ConfigArchiver,
    shutdown: CancellationToken,
}

impl MirrorEngine {
    pub fn start(self, bus: &DomainEventBus) -> Vec<JoinHandle<()>> {
        vec![
            tokio::spawn(self.routing.run(bus.subscribe())),
            tokio::spawn(self.trial.run(bus.subscribe())),
            tokio::spawn(self.meta_rule.run(bus.subscribe())),
            tokio::spawn(self.version.run(bus.subscribe())),
        ]
    }
}
```

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
    rule_repo: ProceduralRuleRepo,
    narrative_handler: Arc<dyn NarrativeHandler>,
    autotuner_bridge: Arc<dyn AutotunerBridge>,
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
    pub async fn submit_feedback(&self, item_id: Uuid, feedback: UserFeedback) -> Result<()>;
    pub async fn create_meta_rule_from_text(&self, text: String) -> Result<ProceduralRule>;

    // On-demand
    pub async fn generate_mirror_response(&self, query: String, period: Option<(DateTime<Utc>, DateTime<Utc>)>) -> Result<TrendNarrative>;
}
```

---

## Integration Points

**Tauri commands** (`desktop/src/commands/mirror.rs`): Thin adapters delegating to `MirrorFacade`. Must export `DEV_COMMANDS` for dev server coverage test.

**Cron jobs** (`app-core/src/init/cron.rs`):
- `JOB_MIRROR_WEEKLY_NARRATIVE` — Sunday 7pm local, generates weekly narrative
- `JOB_MIRROR_CLEANUP` — Daily midnight, retention cleanup (30d hourly snapshots, 90d previews/snippets)

**MCP exposure:** Add `mirror` to `default_exposed_tools()` in `config/schema/mcp.rs`. Read-only: `get_state`, `get_routing_history`, `get_brain_versions`, `get_narratives`.

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

**Data fetching:** `useQuery("get_mirror_state")` on mount. Individual sections use targeted queries when expanded. Real-time snippet updates via SSE `PipelineEvent` stream.

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
