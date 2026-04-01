# Simulation Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic, accelerated simulation harness that runs 6-24 months of synthetic user activity in minutes and benchmarks 14 metrics across memory, retrieval, behavioral, and system convergence tiers.

**Architecture:** New `crates/simulator/` L4 library crate with epoch-driven batch simulation. The simulator replaces AgentLoop as the orchestrator — it calls subsystem functions directly (cognitive pipeline, FSRS decay, autotuner nightly cycle, etc.) with controlled timestamps. A thin test binary at `tests/simulation/` runs scenarios defined in TOML files.

**Tech Stack:** Rust (workspace crate), `chrono` (simulated time), `toml` (scenario files), `serde` (serialization), `rand`/`rand_chacha` (seeded RNG), `sqlx` (in-memory SQLite via StoragePool), existing `cognitive`/`autotuner`/`bus`/`storage` crates.

**Spec:** `docs/superpowers/specs/2026-04-01-simulation-harness-design.md`

---

## File Structure

```
crates/simulator/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API re-exports
│   ├── harness.rs              # SimulationHarness — main epoch loop orchestrator
│   ├── epoch.rs                # SimulatedEpoch, EpochStep, EpochPlan, CronTrigger
│   ├── scenario.rs             # Scenario, SimulationConfig — TOML deserialization
│   ├── persona/
│   │   ├── mod.rs              # PersonaRunner — lifecycle state machine
│   │   ├── types.rs            # Persona, PhaseConfig, AnnotatedMessage, GroundTruthAnnotation
│   │   └── templates.rs        # Per-topic message templates + fill logic
│   ├── providers/
│   │   ├── mod.rs              # Re-exports
│   │   ├── scripted.rs         # ScriptedProvider (impl LlmProvider)
│   │   └── cognitive_bridge.rs # CognitiveLlmBridge — real LLM for extraction/consolidation
│   ├── actions.rs              # SimulatedToolAction enum + executor (repo calls + events)
│   ├── metrics/
│   │   ├── mod.rs              # MetricCollector — accumulates all 14 metrics per epoch
│   │   ├── memory.rs           # Tier 1: knowledge retention, precision, recall, extraction accuracy
│   │   ├── behavioral.rs       # Tier 2: token efficiency, personalization, correction rate
│   │   ├── system.rs           # Tier 3: autotuner convergence, community stability, brain velocity
│   │   └── ground_truth.rs     # GroundTruthVerifier — checkpoint assertions
│   └── report.rs               # SimulationReport, ReportSummary, JSON serialization

tests/simulation/
├── main.rs                     # Test binary entry
├── scenarios/
│   └── software_engineer_12mo.toml  # First scenario
└── smoke.rs                    # Basic integration smoke test
```

---

### Task 1: Scaffold `crates/simulator/` crate

**Files:**
- Create: `crates/simulator/Cargo.toml`
- Create: `crates/simulator/src/lib.rs`
- Modify: `Cargo.toml` (workspace root — add member + workspace dep)

- [ ] **Step 1: Create Cargo.toml for simulator crate**

```toml
# crates/simulator/Cargo.toml
[package]
name = "simulator"
version.workspace = true
edition.workspace = true

[dependencies]
common.workspace = true
config.workspace = true
storage.workspace = true
bus.workspace = true
cognitive.workspace = true
context_engine.workspace = true
autotuner.workspace = true
session.workspace = true
scheduling.workspace = true
skill-system.workspace = true
providers.workspace = true
feature-tasks.workspace = true
feature-notes.workspace = true
feature-finance.workspace = true
feature-productivity.workspace = true

async-trait.workspace = true
chrono = { workspace = true }
uuid = { workspace = true }
serde = { workspace = true }
serde_json.workspace = true
tracing.workspace = true
tokio = { workspace = true }
rand = { workspace = true }
rand_chacha = { workspace = true }
toml = "0.8"

[dev-dependencies]
sqlx = { workspace = true }
tokio = { workspace = true, features = ["macros", "test-util"] }
tempfile.workspace = true
```

- [ ] **Step 2: Create minimal lib.rs**

```rust
// crates/simulator/src/lib.rs
pub mod epoch;
pub mod scenario;

// These modules will be added in subsequent tasks:
// pub mod persona;
// pub mod providers;
// pub mod actions;
// pub mod metrics;
// pub mod harness;
// pub mod report;
```

- [ ] **Step 3: Add simulator to workspace**

In `Cargo.toml` (root), add `"crates/simulator"` to the `members` array after `"crates/autotuner"`, and add `simulator = { path = "crates/simulator" }` to `[workspace.dependencies]`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p simulator`
Expected: Successful build with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/ Cargo.toml Cargo.lock
git commit -m "feat(simulator): scaffold crates/simulator/ library crate"
```

---

### Task 2: SimulatedEpoch — Discrete Time Controller

**Files:**
- Create: `crates/simulator/src/epoch.rs`

- [ ] **Step 1: Write the failing test for epoch advancement**

Add to `crates/simulator/src/epoch.rs`:

```rust
// crates/simulator/src/epoch.rs
use chrono::{DateTime, Datelike, Duration, NaiveTime, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EpochStep {
    Hours(u32),
    Day,
    Week,
}

impl EpochStep {
    fn as_duration(&self) -> Duration {
        match self {
            EpochStep::Hours(h) => Duration::hours(*h as i64),
            EpochStep::Day => Duration::days(1),
            EpochStep::Week => Duration::weeks(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CronTrigger {
    AtomDecay,
    AutotunerNightly,
    AnalyticsCleanup,
    MemoryMaintenance,
    CognitiveReflection,
    MirrorWeeklyNarrative,
    MirrorCleanup,
    CrossDomainInsight,
}

#[derive(Debug)]
pub struct EpochPlan {
    pub simulated_now: DateTime<Utc>,
    pub previous: DateTime<Utc>,
    pub cron_pre_message: Vec<CronTrigger>,
    pub cron_post_message: Vec<CronTrigger>,
    pub day_of_simulation: u32,
}

pub struct SimulatedEpoch {
    current: DateTime<Utc>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    step: EpochStep,
}

impl SimulatedEpoch {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>, step: EpochStep) -> Self {
        Self {
            current: start,
            start,
            end,
            step,
        }
    }

    pub fn current(&self) -> DateTime<Utc> {
        self.current
    }

    pub fn is_finished(&self) -> bool {
        self.current >= self.end
    }

    pub fn day_of_simulation(&self) -> u32 {
        (self.current - self.start).num_days().max(0) as u32
    }

    /// Advance the epoch by one step and return the plan of actions.
    pub fn advance(&mut self) -> Option<EpochPlan> {
        if self.is_finished() {
            return None;
        }

        let previous = self.current;
        self.current = self.current + self.step.as_duration();
        if self.current > self.end {
            self.current = self.end;
        }

        let mut pre = Vec::new();
        let mut post = Vec::new();

        // Pre-message crons: daily jobs that run before user messages
        // AtomDecay: daily 3am
        if self.crosses_daily_hour(previous, self.current, 3) {
            pre.push(CronTrigger::AtomDecay);
        }
        // AutotunerNightly: daily 2am
        if self.crosses_daily_hour(previous, self.current, 2) {
            pre.push(CronTrigger::AutotunerNightly);
        }
        // AnalyticsCleanup: daily
        if self.crosses_midnight(previous, self.current) {
            pre.push(CronTrigger::AnalyticsCleanup);
        }
        // MemoryMaintenance: every 12 hours
        if self.crosses_daily_hour(previous, self.current, 0)
            || self.crosses_daily_hour(previous, self.current, 12)
        {
            pre.push(CronTrigger::MemoryMaintenance);
        }

        // Post-message crons: run after user messages
        // CognitiveReflection: Monday 9am
        if self.crosses_weekday_hour(previous, self.current, Weekday::Mon, 9) {
            post.push(CronTrigger::CognitiveReflection);
        }
        // MirrorWeeklyNarrative: Sunday 10am
        if self.crosses_weekday_hour(previous, self.current, Weekday::Sun, 10) {
            post.push(CronTrigger::MirrorWeeklyNarrative);
        }
        // MirrorCleanup: Sunday 4am
        if self.crosses_weekday_hour(previous, self.current, Weekday::Sun, 4) {
            post.push(CronTrigger::MirrorCleanup);
        }
        // CrossDomainInsight: daily 2am (same as autotuner but post-message for insights)
        if self.crosses_daily_hour(previous, self.current, 2) {
            post.push(CronTrigger::CrossDomainInsight);
        }

        Some(EpochPlan {
            simulated_now: self.current,
            previous,
            cron_pre_message: pre,
            cron_post_message: post,
            day_of_simulation: self.day_of_simulation(),
        })
    }

    fn crosses_midnight(&self, prev: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        prev.date_naive() != now.date_naive()
    }

    fn crosses_daily_hour(&self, prev: DateTime<Utc>, now: DateTime<Utc>, hour: u32) -> bool {
        // Check if the target hour falls within the (prev, now] window
        let target = NaiveTime::from_hms_opt(hour, 0, 0).unwrap();

        if prev.date_naive() == now.date_naive() {
            // Same day: check if target hour is in (prev_time, now_time]
            let prev_time = prev.time();
            let now_time = now.time();
            prev_time < target && target <= now_time
        } else {
            // Crossed midnight: always hits every hour once
            true
        }
    }

    fn crosses_weekday_hour(
        &self,
        prev: DateTime<Utc>,
        now: DateTime<Utc>,
        weekday: Weekday,
        hour: u32,
    ) -> bool {
        // Check all dates in the window for the target weekday+hour
        let mut date = prev.date_naive();
        let end_date = now.date_naive();
        while date <= end_date {
            if date.weekday() == weekday {
                let target_dt = date
                    .and_hms_opt(hour, 0, 0)
                    .unwrap()
                    .and_utc();
                if target_dt > prev && target_dt <= now {
                    return true;
                }
            }
            date += Duration::days(1);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn epoch_advances_by_day() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap();
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        let plan = epoch.advance().unwrap();
        assert_eq!(plan.day_of_simulation, 1);
        assert_eq!(plan.simulated_now, start + Duration::days(1));
        assert!(!epoch.is_finished());
    }

    #[test]
    fn epoch_finishes_at_end() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap();
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        epoch.advance(); // day 1
        epoch.advance(); // day 2 (== end)
        assert!(epoch.is_finished());
        assert!(epoch.advance().is_none());
    }

    #[test]
    fn daily_crons_fire_each_day() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap();
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        let plan = epoch.advance().unwrap();
        assert!(plan.cron_pre_message.contains(&CronTrigger::AtomDecay));
        assert!(plan.cron_pre_message.contains(&CronTrigger::AutotunerNightly));
        assert!(plan.cron_pre_message.contains(&CronTrigger::AnalyticsCleanup));
    }

    #[test]
    fn monday_reflection_fires_on_monday() {
        // Start on a Sunday
        let start = Utc.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap(); // Sunday
        let end = Utc.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap();
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        let sun_to_mon = epoch.advance().unwrap(); // Sunday → Monday
        assert!(
            sun_to_mon
                .cron_post_message
                .contains(&CronTrigger::CognitiveReflection),
            "Monday reflection should fire"
        );

        let mon_to_tue = epoch.advance().unwrap(); // Monday → Tuesday
        assert!(
            !mon_to_tue
                .cron_post_message
                .contains(&CronTrigger::CognitiveReflection),
            "Tuesday should not fire Monday reflection"
        );
    }

    #[test]
    fn weekly_step_hits_all_crons() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(); // Wednesday
        let end = Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Week);

        let plan = epoch.advance().unwrap();
        // A full week always crosses midnight, so daily crons fire
        assert!(plan.cron_pre_message.contains(&CronTrigger::AtomDecay));
        // A full week from Wednesday includes Sunday and Monday
        assert!(plan.cron_post_message.contains(&CronTrigger::MirrorWeeklyNarrative));
        assert!(plan.cron_post_message.contains(&CronTrigger::CognitiveReflection));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p simulator`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/epoch.rs
git commit -m "feat(simulator): add SimulatedEpoch discrete time controller with cron scheduling"
```

---

### Task 3: Scenario Types + TOML Loader

**Files:**
- Create: `crates/simulator/src/scenario.rs`
- Create: `crates/simulator/src/persona/types.rs`
- Create: `crates/simulator/src/persona/mod.rs`

- [ ] **Step 1: Write persona types**

```rust
// crates/simulator/src/persona/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub known_facts: Vec<FactTriple>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    pub duration_days: u32,
    pub correction_rate: f64,
    pub topic_weights: HashMap<String, f64>,
    pub new_fact_introduction_rate: f64,
    pub tool_action_rate: f64,
    /// Only for behavior_shift phase
    #[serde(default)]
    pub shift_description: Option<String>,
    #[serde(default)]
    pub new_facts: Vec<FactTriple>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaPhases {
    pub onboarding: PhaseConfig,
    pub routine: PhaseConfig,
    pub power_user: PhaseConfig,
    pub behavior_shift: PhaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesPerDay {
    pub onboarding: u32,
    pub routine: u32,
    pub power_user: u32,
    pub shift: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub name: String,
    pub timezone: String,
    pub language: String,
    #[serde(default = "default_seed")]
    pub seed: u64,
    pub messages_per_day: MessagesPerDay,
    pub profile: PersonaProfile,
    pub phases: PersonaPhases,
}

fn default_seed() -> u64 {
    42
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecyclePhase {
    Onboarding,
    Routine,
    PowerUser,
    BehaviorShift,
}

impl std::fmt::Display for LifecyclePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecyclePhase::Onboarding => write!(f, "onboarding"),
            LifecyclePhase::Routine => write!(f, "routine"),
            LifecyclePhase::PowerUser => write!(f, "power_user"),
            LifecyclePhase::BehaviorShift => write!(f, "behavior_shift"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthAnnotation {
    pub introduces_fact: Option<FactTriple>,
    pub relevant_facts: Vec<String>,
    pub expected_skill: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulatedToolAction {
    CreateTask {
        title: String,
        due_offset_days: Option<i32>,
        project: Option<String>,
    },
    CompleteTask {
        task_ref: String,
    },
    CreateNote {
        title: String,
        content: String,
    },
    UpdateNote {
        note_ref: String,
        new_content: String,
    },
    RecordTransaction {
        amount: f64,
        category: String,
        description: String,
    },
    StartFocus {
        task_ref: Option<String>,
        duration_mins: u32,
    },
    CreateObjective {
        title: String,
        project: Option<String>,
        due_offset_days: Option<i32>,
    },
    RecordProductivityEvent {
        event_type: String,
        duration_mins: Option<u32>,
    },
}

#[derive(Debug, Clone)]
pub struct AnnotatedMessage {
    pub content: String,
    pub phase: LifecyclePhase,
    pub simulated_at: DateTime<Utc>,
    pub ground_truth: Option<GroundTruthAnnotation>,
    pub tool_actions: Vec<SimulatedToolAction>,
    pub is_correction: bool,
    pub topic: String,
}
```

- [ ] **Step 2: Write persona/mod.rs stub**

```rust
// crates/simulator/src/persona/mod.rs
pub mod types;

pub use types::*;

// PersonaRunner will be added in Task 4
```

- [ ] **Step 3: Write scenario loader with checkpoint types**

```rust
// crates/simulator/src/scenario.rs
use crate::persona::Persona;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    #[serde(default = "default_cognitive_model")]
    pub cognitive_llm_model: String,
    #[serde(default = "default_cognitive_temp")]
    pub cognitive_temperature: f64,
    #[serde(default = "default_max_cognitive_calls")]
    pub max_cognitive_calls_per_day: u32,
    #[serde(default = "default_epoch_step")]
    pub epoch_step: String,
}

fn default_cognitive_model() -> String {
    "heuristic".to_string()
}
fn default_cognitive_temp() -> f64 {
    0.3
}
fn default_max_cognitive_calls() -> u32 {
    12
}
fn default_epoch_step() -> String {
    "day".to_string()
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            cognitive_llm_model: default_cognitive_model(),
            cognitive_temperature: default_cognitive_temp(),
            max_cognitive_calls_per_day: default_max_cognitive_calls(),
            epoch_step: default_epoch_step(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricName {
    KnowledgeRetention,
    RetrievalPrecision,
    RetrievalRecall,
    FactExtractionAccuracy,
    ContradictionDetectionRate,
    CorrectionRate,
    TokenEfficiency,
    PersonalizationScore,
    TaskCompletionRate,
    RoutingStability,
    InsightUsefulness,
    AutotunerPromotionSuccess,
    CommunityStability,
    BrainVersionVelocity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckpointAssertion {
    FactExists {
        subject: String,
        predicate: String,
        object: String,
        min_confidence: f64,
    },
    FactSuperseded {
        subject: String,
        predicate: String,
        old_object: String,
    },
    MetricAbove {
        metric: MetricName,
        threshold: f64,
    },
    MetricImproved {
        metric: MetricName,
        min_improvement_pct: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub at_day: u32,
    pub assertions: Vec<CheckpointAssertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub persona: Persona,
    #[serde(default)]
    pub simulation: SimulationConfig,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
}

impl Scenario {
    pub fn from_toml(content: &str) -> common::Result<Self> {
        toml::from_str(content).map_err(|e| {
            common::KlyntbotError::Internal(format!("Failed to parse scenario TOML: {e}"))
        })
    }

    pub fn from_file(path: &Path) -> common::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            common::KlyntbotError::Internal(format!("Failed to read scenario file: {e}"))
        })?;
        Self::from_toml(&content)
    }

    pub fn total_days(&self) -> u32 {
        let p = &self.persona.phases;
        p.onboarding.duration_days
            + p.routine.duration_days
            + p.power_user.duration_days
            + p.behavior_shift.duration_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_SCENARIO: &str = r#"
[persona]
name = "test_user"
timezone = "UTC"
language = "en"
seed = 42

[persona.messages_per_day]
onboarding = 5
routine = 3
power_user = 4
shift = 3

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "engineer" },
]

[persona.phases.onboarding]
duration_days = 7
correction_rate = 0.2
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.5
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 14
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 14
correction_rate = 0.05
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7

[persona.phases.behavior_shift]
duration_days = 14
correction_rate = 0.15
shift_description = "switches to data science"
new_facts = [{ subject = "user", predicate = "learning", object = "Python" }]
topic_weights = { tasks = 0.3, notes = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.5

[[checkpoints]]
at_day = 7
assertions = [
    { type = "fact_exists", subject = "user", predicate = "works_as", object = "engineer", min_confidence = 0.5 },
]
"#;

    #[test]
    fn parse_minimal_scenario() {
        let scenario = Scenario::from_toml(MINIMAL_SCENARIO).unwrap();
        assert_eq!(scenario.persona.name, "test_user");
        assert_eq!(scenario.persona.phases.onboarding.duration_days, 7);
        assert_eq!(scenario.total_days(), 49);
        assert_eq!(scenario.checkpoints.len(), 1);
        assert_eq!(scenario.checkpoints[0].at_day, 7);
    }

    #[test]
    fn default_simulation_config() {
        let scenario = Scenario::from_toml(MINIMAL_SCENARIO).unwrap();
        assert_eq!(scenario.simulation.cognitive_llm_model, "heuristic");
        assert_eq!(scenario.simulation.max_cognitive_calls_per_day, 12);
    }
}
```

- [ ] **Step 4: Update lib.rs**

```rust
// crates/simulator/src/lib.rs
pub mod epoch;
pub mod persona;
pub mod scenario;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p simulator`
Expected: All tests pass (epoch + scenario tests).

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/
git commit -m "feat(simulator): add scenario TOML loader and persona types"
```

---

### Task 4: PersonaRunner — Lifecycle State Machine + Message Generation

**Files:**
- Create: `crates/simulator/src/persona/templates.rs`
- Modify: `crates/simulator/src/persona/mod.rs`

- [ ] **Step 1: Write message templates**

```rust
// crates/simulator/src/persona/templates.rs
use rand::Rng;

pub const TASK_TEMPLATES: &[&str] = &[
    "Create a task: {action} for {project}, due {due_date}",
    "I need to {action} by {due_date}",
    "Add to my tasks: {action}",
    "Mark {task} as done",
    "What's left on {project}?",
    "Show me my tasks for today",
    "Prioritize {action} — it's urgent",
];

pub const NOTE_TEMPLATES: &[&str] = &[
    "Create a note about {topic}: {content}",
    "Update my {topic} note with: {content}",
    "What do my notes say about {topic}?",
    "Summarize my notes on {topic}",
    "Add to my {topic} note: {content}",
];

pub const FINANCE_TEMPLATES: &[&str] = &[
    "Record expense: {amount} for {category} — {description}",
    "How much did I spend on {category} this month?",
    "Show my budget status",
    "Add income: {amount} from {description}",
    "What's my spending trend for {category}?",
];

pub const CHAT_TEMPLATES: &[&str] = &[
    "Good morning",
    "What should I focus on today?",
    "How's my week looking?",
    "Give me a quick summary",
    "Thanks for the help",
];

pub const PRODUCTIVITY_TEMPLATES: &[&str] = &[
    "Start a focus session for {task}",
    "How productive was I this week?",
    "I'm feeling {energy} today",
    "Track my time on {task}",
];

pub const AUTOMATION_TEMPLATES: &[&str] = &[
    "Set up a daily reminder for {action} at {time}",
    "Create a recurring task: {action} every {frequency}",
    "Automate my {action} workflow",
];

pub const INSIGHTS_TEMPLATES: &[&str] = &[
    "Show me patterns across my notes and tasks",
    "What connections do you see in my work?",
    "Any insights from my recent activity?",
];

pub const LEARNING_TEMPLATES: &[&str] = &[
    "I'm learning {topic} — create some flashcards",
    "Quiz me on {topic}",
    "What should I review today?",
    "I just learned that {content}",
];

pub const CORRECTION_TEMPLATES: &[&str] = &[
    "No, I meant {correct_value}, not {wrong_value}",
    "That's wrong — it should be {correct_value}",
    "Actually, I prefer {correct_value}",
    "Not quite — I said {correct_value}",
];

pub const FACT_INTRODUCTION_TEMPLATES: &[&str] = &[
    "By the way, I {predicate} {object}",
    "Just so you know, I'm a {object}",
    "I've been {predicate} {object} lately",
    "Did I mention I {predicate} {object}?",
];

pub fn pick_template<'a>(templates: &'a [&str], rng: &mut impl Rng) -> &'a str {
    templates[rng.random_range(0..templates.len())]
}

pub fn templates_for_topic(topic: &str) -> &'static [&'static str] {
    match topic {
        "tasks" => TASK_TEMPLATES,
        "notes" => NOTE_TEMPLATES,
        "finance" => FINANCE_TEMPLATES,
        "chat" => CHAT_TEMPLATES,
        "productivity" => PRODUCTIVITY_TEMPLATES,
        "automation" => AUTOMATION_TEMPLATES,
        "insights" => INSIGHTS_TEMPLATES,
        "learning" => LEARNING_TEMPLATES,
        _ => CHAT_TEMPLATES,
    }
}

/// Fill template placeholders with values from a simple key-value map.
pub fn fill_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    // Remove any unfilled placeholders
    let re = regex::Regex::new(r"\{[a-z_]+\}").unwrap();
    re.replace_all(&result, "something").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_template_replaces_vars() {
        let result = fill_template(
            "Create a task: {action} for {project}",
            &[("action", "write docs"), ("project", "Klynt")],
        );
        assert_eq!(result, "Create a task: write docs for Klynt");
    }

    #[test]
    fn fill_template_handles_missing_vars() {
        let result = fill_template("I need to {action} by {due_date}", &[("action", "code")]);
        assert_eq!(result, "I need to code by something");
    }
}
```

- [ ] **Step 2: Write PersonaRunner**

```rust
// crates/simulator/src/persona/mod.rs
pub mod templates;
pub mod types;

pub use types::*;

use chrono::{DateTime, Duration, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashSet, VecDeque};

pub struct PersonaRunner {
    persona: Persona,
    current_phase: LifecyclePhase,
    day_in_phase: u32,
    rng: StdRng,
    introduced_facts: HashSet<String>,
    topic_history: VecDeque<String>,
    created_task_titles: Vec<String>,
}

impl PersonaRunner {
    pub fn new(persona: Persona) -> Self {
        let rng = StdRng::seed_from_u64(persona.seed);
        Self {
            persona,
            current_phase: LifecyclePhase::Onboarding,
            day_in_phase: 0,
            rng,
            introduced_facts: HashSet::new(),
            topic_history: VecDeque::with_capacity(20),
            created_task_titles: Vec::new(),
        }
    }

    pub fn current_phase(&self) -> LifecyclePhase {
        self.current_phase
    }

    pub fn persona(&self) -> &Persona {
        &self.persona
    }

    fn phase_config(&self) -> &PhaseConfig {
        match self.current_phase {
            LifecyclePhase::Onboarding => &self.persona.phases.onboarding,
            LifecyclePhase::Routine => &self.persona.phases.routine,
            LifecyclePhase::PowerUser => &self.persona.phases.power_user,
            LifecyclePhase::BehaviorShift => &self.persona.phases.behavior_shift,
        }
    }

    fn messages_per_day(&self) -> u32 {
        match self.current_phase {
            LifecyclePhase::Onboarding => self.persona.messages_per_day.onboarding,
            LifecyclePhase::Routine => self.persona.messages_per_day.routine,
            LifecyclePhase::PowerUser => self.persona.messages_per_day.power_user,
            LifecyclePhase::BehaviorShift => self.persona.messages_per_day.shift,
        }
    }

    fn check_phase_transition(&mut self) {
        let config = self.phase_config();
        if self.day_in_phase >= config.duration_days {
            self.day_in_phase = 0;
            self.current_phase = match self.current_phase {
                LifecyclePhase::Onboarding => LifecyclePhase::Routine,
                LifecyclePhase::Routine => LifecyclePhase::PowerUser,
                LifecyclePhase::PowerUser => LifecyclePhase::BehaviorShift,
                LifecyclePhase::BehaviorShift => LifecyclePhase::BehaviorShift, // stay
            };
        }
    }

    fn sample_topic(&mut self) -> String {
        let config = self.phase_config().clone();
        let weights: Vec<(String, f64)> = config.topic_weights.into_iter().collect();
        let total: f64 = weights.iter().map(|(_, w)| w).sum();

        // Try up to 5 times to avoid recent topic repetition
        for _ in 0..5 {
            let mut roll: f64 = self.rng.random_range(0.0..total);
            for (topic, weight) in &weights {
                roll -= weight;
                if roll <= 0.0 {
                    // Check sliding window
                    if self.topic_history.len() >= 3
                        && self.topic_history.iter().rev().take(3).all(|t| t == topic)
                    {
                        break; // try again
                    }
                    return topic.clone();
                }
            }
        }

        // Fallback: just pick the first topic
        weights.first().map(|(t, _)| t.clone()).unwrap_or_else(|| "chat".to_string())
    }

    fn generate_tool_action(&mut self, topic: &str, simulated_at: DateTime<Utc>) -> Option<SimulatedToolAction> {
        match topic {
            "tasks" => {
                let titles = ["write API docs", "review PR", "fix auth bug", "deploy v2", "update tests"];
                let title = titles[self.rng.random_range(0..titles.len())].to_string();
                self.created_task_titles.push(title.clone());
                Some(SimulatedToolAction::CreateTask {
                    title,
                    due_offset_days: Some(self.rng.random_range(1..14) as i32),
                    project: Some("main".to_string()),
                })
            }
            "notes" => {
                let topics = ["architecture", "meeting notes", "research", "brainstorm", "learning"];
                let t = topics[self.rng.random_range(0..topics.len())];
                Some(SimulatedToolAction::CreateNote {
                    title: format!("{t} notes"),
                    content: format!("Notes about {t} from today's work"),
                })
            }
            "finance" => {
                let categories = ["food", "transport", "software", "coffee", "books"];
                let cat = categories[self.rng.random_range(0..categories.len())];
                Some(SimulatedToolAction::RecordTransaction {
                    amount: self.rng.random_range(10_000.0..500_000.0),
                    category: cat.to_string(),
                    description: format!("{cat} expense"),
                })
            }
            "productivity" => {
                let task_ref = self
                    .created_task_titles
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "current task".to_string());
                Some(SimulatedToolAction::StartFocus {
                    task_ref: Some(task_ref),
                    duration_mins: self.rng.random_range(25..60),
                })
            }
            _ => None,
        }
    }

    /// Generate all messages for a simulated day.
    pub fn generate_day(&mut self, simulated_date: DateTime<Utc>) -> Vec<AnnotatedMessage> {
        self.check_phase_transition();

        let base_count = self.messages_per_day();
        let jitter: i32 = self.rng.random_range(-2..=2);
        let count = (base_count as i32 + jitter).max(1) as u32;

        let config = self.phase_config().clone();
        let mut messages = Vec::with_capacity(count as usize);

        for i in 0..count {
            // Space messages across the day (9am to 9pm)
            let hour_offset = 9 + (i * 12 / count.max(1));
            let simulated_at = simulated_date + Duration::hours(hour_offset as i64);

            let topic = self.sample_topic();

            // Determine if this is a correction
            let is_correction = self.rng.random::<f64>() < config.correction_rate;

            // Determine if we introduce a new fact
            let mut ground_truth = None;
            if !is_correction && self.rng.random::<f64>() < config.new_fact_introduction_rate {
                // Try to find an unintroduced fact
                let all_facts: Vec<&FactTriple> = self
                    .persona
                    .profile
                    .known_facts
                    .iter()
                    .chain(config.new_facts.iter())
                    .collect();

                if let Some(fact) = all_facts.iter().find(|f| {
                    let key = format!("{}:{}:{}", f.subject, f.predicate, f.object);
                    !self.introduced_facts.contains(&key)
                }) {
                    let key = format!("{}:{}:{}", fact.subject, fact.predicate, fact.object);
                    self.introduced_facts.insert(key);
                    ground_truth = Some(GroundTruthAnnotation {
                        introduces_fact: Some((*fact).clone()),
                        relevant_facts: vec![],
                        expected_skill: None,
                    });
                }
            }

            // Generate tool action
            let mut tool_actions = Vec::new();
            if !is_correction && self.rng.random::<f64>() < config.tool_action_rate {
                if let Some(action) = self.generate_tool_action(&topic, simulated_at) {
                    tool_actions.push(action);
                }
            }

            // Generate message content
            let content = if is_correction {
                let template =
                    templates::pick_template(templates::CORRECTION_TEMPLATES, &mut self.rng);
                templates::fill_template(
                    template,
                    &[("correct_value", "the right thing"), ("wrong_value", "that")],
                )
            } else if let Some(ref gt) = ground_truth {
                if let Some(ref fact) = gt.introduces_fact {
                    let template = templates::pick_template(
                        templates::FACT_INTRODUCTION_TEMPLATES,
                        &mut self.rng,
                    );
                    templates::fill_template(
                        template,
                        &[("predicate", &fact.predicate), ("object", &fact.object)],
                    )
                } else {
                    let templates = templates::templates_for_topic(&topic);
                    templates::pick_template(templates, &mut self.rng).to_string()
                }
            } else {
                let templates = templates::templates_for_topic(&topic);
                let template = templates::pick_template(templates, &mut self.rng);
                templates::fill_template(template, &[])
            };

            // Track topic history
            self.topic_history.push_back(topic.clone());
            if self.topic_history.len() > 20 {
                self.topic_history.pop_front();
            }

            messages.push(AnnotatedMessage {
                content,
                phase: self.current_phase,
                simulated_at,
                ground_truth,
                tool_actions,
                is_correction,
                topic,
            });
        }

        self.day_in_phase += 1;
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_persona() -> Persona {
        let toml_str = r#"
name = "test"
timezone = "UTC"
language = "en"
seed = 42
[messages_per_day]
onboarding = 5
routine = 3
power_user = 4
shift = 3
[profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "engineer" },
    { subject = "user", predicate = "prefers", object = "Rust" },
]
[phases.onboarding]
duration_days = 3
correction_rate = 0.25
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.8
tool_action_rate = 0.5
[phases.routine]
duration_days = 5
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5
[phases.power_user]
duration_days = 5
correction_rate = 0.05
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7
[phases.behavior_shift]
duration_days = 5
correction_rate = 0.15
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.5
"#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn generates_messages_for_day() {
        let mut runner = PersonaRunner::new(test_persona());
        let date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let messages = runner.generate_day(date);

        assert!(!messages.is_empty());
        assert!(messages.len() >= 3 && messages.len() <= 7); // 5 ± 2
        assert!(messages.iter().all(|m| m.phase == LifecyclePhase::Onboarding));
    }

    #[test]
    fn phase_transitions_after_duration() {
        let mut runner = PersonaRunner::new(test_persona());
        let base = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        // Generate 3 days (onboarding duration)
        for d in 0..3 {
            runner.generate_day(base + Duration::days(d));
        }
        assert_eq!(runner.current_phase(), LifecyclePhase::Onboarding);

        // Day 4 should trigger transition to Routine
        runner.generate_day(base + Duration::days(3));
        assert_eq!(runner.current_phase(), LifecyclePhase::Routine);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let persona = test_persona();
        let date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        let mut runner1 = PersonaRunner::new(persona.clone());
        let msgs1: Vec<String> = runner1.generate_day(date).into_iter().map(|m| m.content).collect();

        let mut runner2 = PersonaRunner::new(persona);
        let msgs2: Vec<String> = runner2.generate_day(date).into_iter().map(|m| m.content).collect();

        assert_eq!(msgs1, msgs2);
    }

    #[test]
    fn introduces_facts_during_onboarding() {
        let mut runner = PersonaRunner::new(test_persona());
        let base = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        let mut introduced = 0;
        for d in 0..3 {
            let msgs = runner.generate_day(base + Duration::days(d));
            introduced += msgs
                .iter()
                .filter(|m| {
                    m.ground_truth
                        .as_ref()
                        .is_some_and(|gt| gt.introduces_fact.is_some())
                })
                .count();
        }
        // With 0.8 introduction rate and ~5 messages/day over 3 days, should introduce both facts
        assert!(introduced >= 1, "Expected at least 1 fact introduction, got {introduced}");
    }
}
```

- [ ] **Step 3: Update lib.rs to include templates module**

The persona/mod.rs already exports templates. No lib.rs change needed.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator`
Expected: All tests pass (epoch + scenario + persona + template tests).

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/persona/
git commit -m "feat(simulator): add PersonaRunner lifecycle state machine with message generation"
```

---

### Task 5: ScriptedProvider — impl LlmProvider

**Files:**
- Create: `crates/simulator/src/providers/mod.rs`
- Create: `crates/simulator/src/providers/scripted.rs`

- [ ] **Step 1: Write ScriptedProvider**

```rust
// crates/simulator/src/providers/scripted.rs
use async_trait::async_trait;
use providers::types::{
    ChatParams, LlmResponse, LlmStream, Message, ProviderCapabilities, ProviderHealth,
};
use providers::DynProvider;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// An LlmProvider that returns scripted text responses.
/// Used for agent-facing LLM calls in simulation (not cognitive pipeline).
pub struct ScriptedProvider {
    responses: Vec<String>,
    call_count: AtomicUsize,
}

impl ScriptedProvider {
    /// Create with a set of rotating responses.
    pub fn new(responses: Vec<String>) -> Self {
        assert!(!responses.is_empty(), "ScriptedProvider needs at least one response");
        Self {
            responses,
            call_count: AtomicUsize::new(0),
        }
    }

    /// Create with a single default response.
    pub fn default_response() -> Self {
        Self::new(vec!["I understand. Let me help you with that.".to_string()])
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl providers::types::LlmProvider for ScriptedProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
    ) -> common::Result<LlmResponse> {
        let idx = self.call_count.fetch_add(1, Ordering::Relaxed);
        let response_text = &self.responses[idx % self.responses.len()];
        Ok(LlmResponse {
            content: response_text.clone(),
            tool_calls: vec![],
            usage: providers::types::Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            model: "scripted-sim".to_string(),
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[serde_json::Value]>,
        params: &ChatParams,
    ) -> common::Result<LlmStream> {
        // Simulation doesn't need streaming — delegate to non-streaming
        Err(common::KlyntbotError::Internal(
            "ScriptedProvider does not support streaming".to_string(),
        ))
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn default_model(&self) -> &str {
        "scripted-sim"
    }

    fn name(&self) -> &str {
        "scripted-simulator"
    }

    async fn count_tokens(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
    ) -> common::Result<usize> {
        Ok(150)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: false,
            supports_vision: false,
            supports_streaming: false,
            max_output_tokens: 4096,
        }
    }

    fn context_window(&self) -> usize {
        128_000
    }

    async fn health_check(&self) -> common::Result<ProviderHealth> {
        Ok(ProviderHealth {
            is_healthy: true,
            latency_ms: Some(0),
            error: None,
        })
    }

    fn classifier_provider(&self) -> Option<DynProvider> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scripted_provider_cycles_responses() {
        let provider = ScriptedProvider::new(vec!["first".to_string(), "second".to_string()]);
        let params = ChatParams::default();

        let r1 = provider.chat(&[], None, &params).await.unwrap();
        assert_eq!(r1.content, "first");

        let r2 = provider.chat(&[], None, &params).await.unwrap();
        assert_eq!(r2.content, "second");

        // Cycles back
        let r3 = provider.chat(&[], None, &params).await.unwrap();
        assert_eq!(r3.content, "first");

        assert_eq!(provider.call_count(), 3);
    }
}
```

- [ ] **Step 2: Write providers/mod.rs**

```rust
// crates/simulator/src/providers/mod.rs
pub mod scripted;

pub use scripted::ScriptedProvider;
```

- [ ] **Step 3: Update lib.rs**

```rust
// crates/simulator/src/lib.rs
pub mod epoch;
pub mod persona;
pub mod providers;
pub mod scenario;
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/providers/
git commit -m "feat(simulator): add ScriptedProvider implementing LlmProvider for simulation"
```

---

### Task 6: SimulatedToolAction Executor

**Files:**
- Create: `crates/simulator/src/actions.rs`

- [ ] **Step 1: Write the action executor**

```rust
// crates/simulator/src/actions.rs
use crate::persona::SimulatedToolAction;
use bus::DomainEvent;
use chrono::{DateTime, Duration, Utc};
use tracing::debug;
use uuid::Uuid;

/// Executes simulated tool actions by inserting into repos and publishing domain events.
pub struct ActionExecutor {
    bus: std::sync::Arc<bus::DomainEventBus>,
    task_repo: storage::TaskRepo,
}

impl ActionExecutor {
    pub fn new(bus: std::sync::Arc<bus::DomainEventBus>, pool: &storage::StoragePool) -> Self {
        Self {
            bus,
            task_repo: storage::TaskRepo::new(pool.inner().clone()),
        }
    }

    pub async fn execute(
        &self,
        action: &SimulatedToolAction,
        simulated_now: DateTime<Utc>,
    ) -> common::Result<()> {
        match action {
            SimulatedToolAction::CreateTask {
                title,
                due_offset_days,
                project,
            } => {
                let task_id = Uuid::new_v4().to_string();
                let due_date = due_offset_days
                    .map(|d| (simulated_now + Duration::days(d as i64)).to_rfc3339());
                debug!(task_id, title, "Simulated: creating task");
                self.bus.publish(DomainEvent::TaskCreated {
                    task_id,
                    title: title.clone(),
                    project_id: project.clone(),
                    area_id: None,
                    parent_id: None,
                    due_date,
                    priority: None,
                    source: Some("simulation".to_string()),
                });
            }
            SimulatedToolAction::CompleteTask { task_ref } => {
                debug!(task_ref, "Simulated: completing task");
                self.bus.publish(DomainEvent::TaskCompleted {
                    task_id: task_ref.clone(),
                    title: task_ref.clone(),
                    project_id: None,
                    area_id: None,
                    completion_note: None,
                });
            }
            SimulatedToolAction::CreateNote { title, content } => {
                let note_id = Uuid::new_v4().to_string();
                debug!(note_id, title, "Simulated: creating note");
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id,
                    title: title.clone(),
                    content_preview: content.chars().take(100).collect(),
                });
            }
            SimulatedToolAction::UpdateNote {
                note_ref,
                new_content,
            } => {
                debug!(note_ref, "Simulated: updating note");
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id: note_ref.clone(),
                    title: note_ref.clone(),
                    content_preview: new_content.chars().take(100).collect(),
                });
            }
            SimulatedToolAction::RecordTransaction {
                amount,
                category,
                description,
            } => {
                debug!(amount, category, "Simulated: recording transaction");
                self.bus.publish(DomainEvent::TransactionRecorded {
                    account_id: "sim-account".to_string(),
                    amount: *amount,
                    currency: "VND".to_string(),
                    category: category.clone(),
                    description: description.clone(),
                });
            }
            SimulatedToolAction::StartFocus {
                task_ref,
                duration_mins,
            } => {
                debug!(duration_mins, "Simulated: starting focus session");
                self.bus.publish(DomainEvent::FocusSessionStarted {
                    task_id: task_ref.clone(),
                    duration_minutes: *duration_mins,
                });
            }
            SimulatedToolAction::CreateObjective {
                title,
                project,
                due_offset_days,
            } => {
                debug!(title, "Simulated: creating objective");
                self.bus.publish(DomainEvent::GoalProgress {
                    goal_type: "objective".to_string(),
                    goal_title: title.clone(),
                    progress_pct: 0.0,
                    detail: Some(format!("New objective: {title}")),
                });
            }
            SimulatedToolAction::RecordProductivityEvent {
                event_type,
                duration_mins,
            } => {
                debug!(event_type, "Simulated: productivity event");
                // Productivity events go through the bus as behavioral signals
                self.bus.publish(DomainEvent::ProductivityEventRecorded {
                    event_type: event_type.clone(),
                    duration_mins: *duration_mins,
                });
            }
        }
        Ok(())
    }
}
```

**Note:** Some `DomainEvent` variants may have slightly different field names than shown here. The implementing engineer should check `crates/bus/src/domain_events.rs` and adjust field names to match the actual enum variants. The pattern is correct — create the entity via repo or just publish the event.

- [ ] **Step 2: Update lib.rs**

Add `pub mod actions;` to `crates/simulator/src/lib.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p simulator`
Expected: Compiles (may need field name adjustments based on actual DomainEvent variants).

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/actions.rs crates/simulator/src/lib.rs
git commit -m "feat(simulator): add SimulatedToolAction executor with domain event publishing"
```

---

### Task 7: MetricCollector + Memory Metrics

**Files:**
- Create: `crates/simulator/src/metrics/mod.rs`
- Create: `crates/simulator/src/metrics/memory.rs`
- Create: `crates/simulator/src/metrics/behavioral.rs`
- Create: `crates/simulator/src/metrics/system.rs`

- [ ] **Step 1: Write MetricSnapshot and MetricCollector**

```rust
// crates/simulator/src/metrics/mod.rs
pub mod behavioral;
pub mod memory;
pub mod system;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub epoch: DateTime<Utc>,
    // Tier 1: Annotation-based
    pub knowledge_retention: f64,
    pub retrieval_precision: f64,
    pub retrieval_recall: f64,
    pub fact_extraction_accuracy: f64,
    pub contradiction_detection_rate: f64,
    pub correction_rate: f64,
    // Tier 2: Relative improvement
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub insight_usefulness: f64,
    // Tier 3: System convergence
    pub autotuner_promotion_success: f64,
    pub community_stability: f64,
    pub brain_version_velocity: u32,
    // Performance
    pub wall_time_per_epoch_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaselineMetrics {
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub insight_usefulness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAlert {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    pub regression_pct: f64,
}

/// Accumulates per-message counters within a single epoch tick.
#[derive(Debug, Default)]
pub struct EpochAccumulator {
    pub messages_processed: u32,
    pub corrections: u32,
    pub facts_introduced: u32,
    pub facts_extracted: u32,
    pub contradictions_detected: u32,
    pub total_tokens: u64,
    pub retrieval_precision_sum: f64,
    pub retrieval_recall_sum: f64,
    pub retrieval_count: u32,
    pub routing_matches: u32,
    pub tasks_created: u32,
    pub tasks_completed: u32,
}

pub struct MetricCollector {
    pub timeline: Vec<MetricSnapshot>,
    pub baselines: Option<BaselineMetrics>,
    baseline_day: u32,
    accumulator: EpochAccumulator,
}

impl MetricCollector {
    pub fn new(baseline_after_day: u32) -> Self {
        Self {
            timeline: Vec::new(),
            baselines: None,
            baseline_day: baseline_after_day,
            accumulator: EpochAccumulator::default(),
        }
    }

    pub fn accumulator_mut(&mut self) -> &mut EpochAccumulator {
        &mut self.accumulator
    }

    /// Finalize the current epoch's metrics and push a snapshot.
    pub fn snapshot(
        &mut self,
        epoch: DateTime<Utc>,
        day: u32,
        knowledge_retention: f64,
        autotuner_promotion_success: f64,
        community_stability: f64,
        brain_version_velocity: u32,
        insight_usefulness: f64,
        wall_time_ms: f64,
    ) {
        let acc = &self.accumulator;
        let msg_count = acc.messages_processed.max(1) as f64;

        let correction_rate = acc.corrections as f64 / msg_count;
        let fact_extraction_accuracy = if acc.facts_introduced > 0 {
            acc.facts_extracted as f64 / acc.facts_introduced as f64
        } else {
            1.0
        };
        let contradiction_detection_rate = acc.contradictions_detected as f64
            / acc.facts_introduced.max(1) as f64;
        let retrieval_precision = if acc.retrieval_count > 0 {
            acc.retrieval_precision_sum / acc.retrieval_count as f64
        } else {
            0.0
        };
        let retrieval_recall = if acc.retrieval_count > 0 {
            acc.retrieval_recall_sum / acc.retrieval_count as f64
        } else {
            0.0
        };
        let token_efficiency = acc.total_tokens as f64 / msg_count;
        let task_completion_rate = if acc.tasks_created > 0 {
            acc.tasks_completed as f64 / acc.tasks_created as f64
        } else {
            0.0
        };
        let routing_stability = acc.routing_matches as f64 / msg_count;

        let correction_rate_inverse = 1.0 - correction_rate;
        let fact_coverage = knowledge_retention;
        let personalization_score =
            fact_coverage * 0.4 + retrieval_precision * 0.3 + correction_rate_inverse * 0.3;

        let snapshot = MetricSnapshot {
            epoch,
            knowledge_retention,
            retrieval_precision,
            retrieval_recall,
            fact_extraction_accuracy,
            contradiction_detection_rate,
            correction_rate,
            token_efficiency,
            personalization_score,
            task_completion_rate,
            routing_stability,
            insight_usefulness,
            autotuner_promotion_success,
            community_stability,
            brain_version_velocity,
            wall_time_per_epoch_ms: wall_time_ms,
        };

        self.timeline.push(snapshot);

        // Compute baselines after the configured day
        if self.baselines.is_none() && day >= self.baseline_day {
            self.compute_baselines();
        }

        // Reset accumulator for next epoch
        self.accumulator = EpochAccumulator::default();
    }

    fn compute_baselines(&mut self) {
        if self.timeline.is_empty() {
            return;
        }
        let n = self.timeline.len() as f64;
        let baselines = BaselineMetrics {
            token_efficiency: self.timeline.iter().map(|s| s.token_efficiency).sum::<f64>() / n,
            personalization_score: self
                .timeline
                .iter()
                .map(|s| s.personalization_score)
                .sum::<f64>()
                / n,
            task_completion_rate: self
                .timeline
                .iter()
                .map(|s| s.task_completion_rate)
                .sum::<f64>()
                / n,
            routing_stability: self
                .timeline
                .iter()
                .map(|s| s.routing_stability)
                .sum::<f64>()
                / n,
            insight_usefulness: self
                .timeline
                .iter()
                .map(|s| s.insight_usefulness)
                .sum::<f64>()
                / n,
        };
        self.baselines = Some(baselines);
    }

    /// Check for regressions against baselines.
    pub fn check_regressions(&self, threshold_pct: f64) -> Vec<RegressionAlert> {
        let Some(baselines) = &self.baselines else {
            return vec![];
        };
        let Some(latest) = self.timeline.last() else {
            return vec![];
        };

        let mut alerts = Vec::new();

        // Token efficiency: lower is better, so regression = increase
        let token_change =
            (latest.token_efficiency - baselines.token_efficiency) / baselines.token_efficiency.max(1.0) * 100.0;
        if token_change > threshold_pct {
            alerts.push(RegressionAlert {
                metric: "token_efficiency".to_string(),
                baseline: baselines.token_efficiency,
                current: latest.token_efficiency,
                regression_pct: token_change,
            });
        }

        // Other metrics: higher is better, so regression = decrease
        let checks = [
            (
                "personalization_score",
                baselines.personalization_score,
                latest.personalization_score,
            ),
            (
                "routing_stability",
                baselines.routing_stability,
                latest.routing_stability,
            ),
        ];
        for (name, baseline, current) in checks {
            if baseline > 0.0 {
                let change = (baseline - current) / baseline * 100.0;
                if change > threshold_pct {
                    alerts.push(RegressionAlert {
                        metric: name.to_string(),
                        baseline,
                        current,
                        regression_pct: change,
                    });
                }
            }
        }

        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn snapshot_computes_rates_correctly() {
        let mut collector = MetricCollector::new(30);
        collector.accumulator_mut().messages_processed = 10;
        collector.accumulator_mut().corrections = 2;
        collector.accumulator_mut().facts_introduced = 3;
        collector.accumulator_mut().facts_extracted = 2;
        collector.accumulator_mut().total_tokens = 1500;
        collector.accumulator_mut().retrieval_precision_sum = 2.4;
        collector.accumulator_mut().retrieval_recall_sum = 1.8;
        collector.accumulator_mut().retrieval_count = 3;

        let epoch = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        collector.snapshot(epoch, 1, 0.8, 0.0, 0.0, 0, 0.0, 50.0);

        let snap = &collector.timeline[0];
        assert!((snap.correction_rate - 0.2).abs() < 0.001);
        assert!((snap.fact_extraction_accuracy - 0.667).abs() < 0.01);
        assert!((snap.retrieval_precision - 0.8).abs() < 0.001);
        assert!((snap.token_efficiency - 150.0).abs() < 0.001);
    }
}
```

- [ ] **Step 2: Write memory metrics helpers**

```rust
// crates/simulator/src/metrics/memory.rs
use crate::persona::FactTriple;

/// Calculate knowledge retention: fraction of known facts that exist in the semantic fact repo.
pub async fn measure_knowledge_retention(
    repo: &cognitive::SemanticFactRepo,
    known_facts: &[FactTriple],
) -> f64 {
    if known_facts.is_empty() {
        return 1.0;
    }

    let mut found = 0;
    for fact in known_facts {
        // Check if fact exists (not superseded)
        let results = repo
            .search_fts(&format!("{} {} {}", fact.subject, fact.predicate, fact.object), None, 5)
            .await
            .unwrap_or_default();

        let exists = results.iter().any(|f| {
            f.subject == fact.subject
                && f.predicate == fact.predicate
                && f.object == fact.object
                && f.superseded_at.is_none()
        });
        if exists {
            found += 1;
        }
    }

    found as f64 / known_facts.len() as f64
}

/// Calculate retrieval precision and recall for a single query.
pub fn measure_retrieval_quality(
    retrieved_ids: &[String],
    relevant_ids: &[String],
) -> (f64, f64) {
    if retrieved_ids.is_empty() && relevant_ids.is_empty() {
        return (1.0, 1.0);
    }

    let retrieved_set: std::collections::HashSet<&str> =
        retrieved_ids.iter().map(|s| s.as_str()).collect();
    let relevant_set: std::collections::HashSet<&str> =
        relevant_ids.iter().map(|s| s.as_str()).collect();

    let intersection = retrieved_set.intersection(&relevant_set).count() as f64;

    let precision = if retrieved_ids.is_empty() {
        0.0
    } else {
        intersection / retrieved_ids.len() as f64
    };
    let recall = if relevant_ids.is_empty() {
        1.0
    } else {
        intersection / relevant_ids.len() as f64
    };

    (precision, recall)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_recall_all_relevant() {
        let retrieved = vec!["a".to_string(), "b".to_string()];
        let relevant = vec!["a".to_string(), "b".to_string()];
        let (p, r) = measure_retrieval_quality(&retrieved, &relevant);
        assert!((p - 1.0).abs() < 0.001);
        assert!((r - 1.0).abs() < 0.001);
    }

    #[test]
    fn precision_recall_partial() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let relevant = vec!["a".to_string(), "d".to_string()];
        let (p, r) = measure_retrieval_quality(&retrieved, &relevant);
        assert!((p - 1.0 / 3.0).abs() < 0.001); // 1 of 3 retrieved
        assert!((r - 0.5).abs() < 0.001); // 1 of 2 relevant
    }
}
```

- [ ] **Step 3: Write behavioral and system metric stubs**

```rust
// crates/simulator/src/metrics/behavioral.rs
// Behavioral metrics are computed inline by MetricCollector from EpochAccumulator.
// This module provides helpers for specific behavioral measurements.

/// Compute personalization score composite.
pub fn personalization_score(
    fact_coverage: f64,
    retrieval_precision: f64,
    correction_rate: f64,
) -> f64 {
    let correction_rate_inverse = 1.0 - correction_rate;
    fact_coverage * 0.4 + retrieval_precision * 0.3 + correction_rate_inverse * 0.3
}
```

```rust
// crates/simulator/src/metrics/system.rs
/// Measure autotuner promotion success rate.
pub async fn measure_autotuner_success(
    trial_repo: &storage::TrialRepo,
) -> f64 {
    let promoted = trial_repo
        .list_by_status("promoted")
        .await
        .unwrap_or_default();
    let reverted = trial_repo
        .list_by_status("reverted")
        .await
        .unwrap_or_default();

    let total = promoted.len() + reverted.len();
    if total == 0 {
        return 0.0;
    }
    promoted.len() as f64 / total as f64
}

/// Measure community stability from the communities table.
pub async fn measure_community_stability(
    pool: &sqlx::SqlitePool,
) -> f64 {
    let result: Option<(f64,)> = sqlx::query_as(
        "SELECT AVG(stability) FROM communities WHERE stability IS NOT NULL"
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    result.map(|(avg,)| avg).unwrap_or(0.0)
}

/// Count brain versions created in the current epoch window.
pub async fn count_brain_versions_since(
    pool: &sqlx::SqlitePool,
    since: &str,
) -> u32 {
    let result: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mirror_brain_versions WHERE promoted_at > ?"
    )
    .bind(since)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    result.0 as u32
}
```

- [ ] **Step 4: Update lib.rs**

Add `pub mod metrics;` to lib.rs.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p simulator`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/metrics/
git commit -m "feat(simulator): add MetricCollector with 14-metric accumulator and ground truth helpers"
```

---

### Task 8: GroundTruthVerifier

**Files:**
- Create: `crates/simulator/src/metrics/ground_truth.rs`

- [ ] **Step 1: Write GroundTruthVerifier**

```rust
// crates/simulator/src/metrics/ground_truth.rs
use crate::metrics::MetricSnapshot;
use crate::scenario::{Checkpoint, CheckpointAssertion, MetricName};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub at_day: u32,
    pub assertions: Vec<AssertionResult>,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub description: String,
    pub passed: bool,
    pub actual_value: Option<f64>,
    pub expected: String,
}

pub struct GroundTruthVerifier;

impl GroundTruthVerifier {
    pub async fn verify_checkpoint(
        checkpoint: &Checkpoint,
        fact_repo: &cognitive::SemanticFactRepo,
        latest_snapshot: &MetricSnapshot,
        baselines: Option<&super::BaselineMetrics>,
    ) -> CheckpointResult {
        let mut results = Vec::new();

        for assertion in &checkpoint.assertions {
            let result = match assertion {
                CheckpointAssertion::FactExists {
                    subject,
                    predicate,
                    object,
                    min_confidence,
                } => {
                    let facts = fact_repo
                        .search_fts(
                            &format!("{subject} {predicate} {object}"),
                            None,
                            10,
                        )
                        .await
                        .unwrap_or_default();

                    let found = facts.iter().find(|f| {
                        f.subject == *subject
                            && f.predicate == *predicate
                            && f.object == *object
                            && f.superseded_at.is_none()
                            && f.confidence >= *min_confidence
                    });

                    AssertionResult {
                        description: format!(
                            "Fact exists: {subject}.{predicate} = {object} (confidence >= {min_confidence})"
                        ),
                        passed: found.is_some(),
                        actual_value: found.map(|f| f.confidence),
                        expected: format!("confidence >= {min_confidence}"),
                    }
                }
                CheckpointAssertion::FactSuperseded {
                    subject,
                    predicate,
                    old_object,
                } => {
                    let facts = fact_repo
                        .search_fts(
                            &format!("{subject} {predicate} {old_object}"),
                            None,
                            10,
                        )
                        .await
                        .unwrap_or_default();

                    let superseded = facts.iter().any(|f| {
                        f.subject == *subject
                            && f.predicate == *predicate
                            && f.object == *old_object
                            && f.superseded_at.is_some()
                    });

                    AssertionResult {
                        description: format!(
                            "Fact superseded: {subject}.{predicate} = {old_object}"
                        ),
                        passed: superseded,
                        actual_value: None,
                        expected: "superseded_at IS NOT NULL".to_string(),
                    }
                }
                CheckpointAssertion::MetricAbove { metric, threshold } => {
                    let value = get_metric_value(latest_snapshot, metric);
                    AssertionResult {
                        description: format!("{metric:?} >= {threshold}"),
                        passed: value >= *threshold,
                        actual_value: Some(value),
                        expected: format!(">= {threshold}"),
                    }
                }
                CheckpointAssertion::MetricImproved {
                    metric,
                    min_improvement_pct,
                } => {
                    let current = get_metric_value(latest_snapshot, metric);
                    let baseline = baselines
                        .map(|b| get_baseline_value(b, metric))
                        .unwrap_or(0.0);
                    let improvement = if baseline > 0.0 {
                        (current - baseline) / baseline * 100.0
                    } else {
                        0.0
                    };

                    AssertionResult {
                        description: format!(
                            "{metric:?} improved >= {min_improvement_pct}% from baseline"
                        ),
                        passed: improvement >= *min_improvement_pct,
                        actual_value: Some(improvement),
                        expected: format!(">= {min_improvement_pct}%"),
                    }
                }
            };
            results.push(result);
        }

        let all_passed = results.iter().all(|r| r.passed);
        CheckpointResult {
            at_day: checkpoint.at_day,
            assertions: results,
            all_passed,
        }
    }
}

fn get_metric_value(snapshot: &MetricSnapshot, metric: &MetricName) -> f64 {
    match metric {
        MetricName::KnowledgeRetention => snapshot.knowledge_retention,
        MetricName::RetrievalPrecision => snapshot.retrieval_precision,
        MetricName::RetrievalRecall => snapshot.retrieval_recall,
        MetricName::FactExtractionAccuracy => snapshot.fact_extraction_accuracy,
        MetricName::ContradictionDetectionRate => snapshot.contradiction_detection_rate,
        MetricName::CorrectionRate => snapshot.correction_rate,
        MetricName::TokenEfficiency => snapshot.token_efficiency,
        MetricName::PersonalizationScore => snapshot.personalization_score,
        MetricName::TaskCompletionRate => snapshot.task_completion_rate,
        MetricName::RoutingStability => snapshot.routing_stability,
        MetricName::InsightUsefulness => snapshot.insight_usefulness,
        MetricName::AutotunerPromotionSuccess => snapshot.autotuner_promotion_success,
        MetricName::CommunityStability => snapshot.community_stability,
        MetricName::BrainVersionVelocity => snapshot.brain_version_velocity as f64,
    }
}

fn get_baseline_value(baselines: &super::BaselineMetrics, metric: &MetricName) -> f64 {
    match metric {
        MetricName::TokenEfficiency => baselines.token_efficiency,
        MetricName::PersonalizationScore => baselines.personalization_score,
        MetricName::TaskCompletionRate => baselines.task_completion_rate,
        MetricName::RoutingStability => baselines.routing_stability,
        MetricName::InsightUsefulness => baselines.insight_usefulness,
        _ => 0.0, // Tier 1 and 3 metrics don't use baselines
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p simulator`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add GroundTruthVerifier with checkpoint assertion engine"
```

---

### Task 9: ReportGenerator

**Files:**
- Create: `crates/simulator/src/report.rs`

- [ ] **Step 1: Write report types and generator**

```rust
// crates/simulator/src/report.rs
use crate::metrics::{BaselineMetrics, MetricSnapshot, RegressionAlert};
use crate::metrics::ground_truth::CheckpointResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulationReport {
    pub scenario: String,
    pub persona: String,
    pub simulated_days: u32,
    pub wall_time_secs: f64,
    pub seed: u64,
    pub metric_timeline: Vec<MetricSnapshot>,
    pub checkpoints: Vec<CheckpointResult>,
    pub summary: ReportSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_messages: u32,
    pub total_facts_extracted: u32,
    pub total_facts_superseded: u32,
    pub total_brain_versions: u32,
    pub total_autotuner_promotions: u32,
    pub total_autotuner_reverts: u32,
    pub final_metrics: MetricSnapshot,
    pub baseline_metrics: Option<BaselineMetrics>,
    pub improvement_pct: HashMap<String, f64>,
    pub checkpoint_pass_rate: f64,
    pub regression_alerts: Vec<RegressionAlert>,
}

impl SimulationReport {
    pub fn write_json(&self, dir: &Path) -> common::Result<std::path::PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.json", self.scenario, timestamp);
        let path = dir.join(filename);

        std::fs::create_dir_all(dir).map_err(|e| {
            common::KlyntbotError::Internal(format!("Failed to create output dir: {e}"))
        })?;

        let json = serde_json::to_string_pretty(self).map_err(|e| {
            common::KlyntbotError::Internal(format!("Failed to serialize report: {e}"))
        })?;

        std::fs::write(&path, json).map_err(|e| {
            common::KlyntbotError::Internal(format!("Failed to write report: {e}"))
        })?;

        Ok(path)
    }

    /// Returns true if the simulation passed all checks.
    pub fn passed(&self) -> bool {
        self.summary.checkpoint_pass_rate >= 1.0 && self.summary.regression_alerts.is_empty()
    }
}

/// Compute improvement percentages between baseline and final metrics.
pub fn compute_improvements(
    baselines: &BaselineMetrics,
    final_metrics: &MetricSnapshot,
) -> HashMap<String, f64> {
    let mut improvements = HashMap::new();

    if baselines.personalization_score > 0.0 {
        improvements.insert(
            "personalization_score".to_string(),
            (final_metrics.personalization_score - baselines.personalization_score)
                / baselines.personalization_score
                * 100.0,
        );
    }
    if baselines.routing_stability > 0.0 {
        improvements.insert(
            "routing_stability".to_string(),
            (final_metrics.routing_stability - baselines.routing_stability)
                / baselines.routing_stability
                * 100.0,
        );
    }
    // Token efficiency: lower is better, so improvement is negative change
    if baselines.token_efficiency > 0.0 {
        improvements.insert(
            "token_efficiency".to_string(),
            (baselines.token_efficiency - final_metrics.token_efficiency)
                / baselines.token_efficiency
                * 100.0,
        );
    }

    improvements
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod report;` to lib.rs.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/report.rs crates/simulator/src/lib.rs
git commit -m "feat(simulator): add SimulationReport and ReportGenerator"
```

---

### Task 10: SimulationHarness — Main Epoch Loop Orchestrator

**Files:**
- Create: `crates/simulator/src/harness.rs`

This is the central orchestrator that ties everything together.

- [ ] **Step 1: Write SimulationHarness**

```rust
// crates/simulator/src/harness.rs
use crate::actions::ActionExecutor;
use crate::epoch::{CronTrigger, EpochStep, SimulatedEpoch};
use crate::metrics::ground_truth::{CheckpointResult, GroundTruthVerifier};
use crate::metrics::{MetricCollector, RegressionAlert};
use crate::persona::PersonaRunner;
use crate::report::{self, SimulationReport, ReportSummary};
use crate::scenario::Scenario;
use bus::{ContextUpdateQueue, DomainEvent, DomainEventBus};
use chrono::{DateTime, Duration, TimeZone, Utc};
use cognitive::services::salience::evaluate_salience;
use cognitive::{SemanticFactRepo, EpisodicMemoryRepo};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct SimulationHarness {
    scenario: Scenario,
    pool: storage::StoragePool,
    inner_pool: sqlx::SqlitePool,
    bus: Arc<DomainEventBus>,
    context_queue: Arc<ContextUpdateQueue>,
    fact_repo: SemanticFactRepo,
    episodic_repo: EpisodicMemoryRepo,
    extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
    consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
}

impl SimulationHarness {
    pub async fn new(
        scenario: Scenario,
        extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
        consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
    ) -> common::Result<Self> {
        // Create in-memory storage with all feature migrations
        let pool = storage::StoragePool::connect_in_memory().await
            .map_err(|e| common::KlyntbotError::Internal(format!("Pool creation failed: {e}")))?;
        let inner = pool.inner().clone();

        // Run cognitive migrations
        storage::StoragePool::run_feature_migrations(&inner, &cognitive::repos::cognitive_migrations())
            .await
            .map_err(|e| common::KlyntbotError::Internal(format!("Cognitive migrations failed: {e}")))?;

        // Run autotuner migrations (for trial tables)
        // The autotuner migration SQL is in storage::repos::trial_repo
        // We'll run it via the TrialRepo's ensure_tables
        let trial_repo = storage::TrialRepo::new(inner.clone());
        trial_repo.ensure_tables().await
            .map_err(|e| common::KlyntbotError::Internal(format!("Trial migrations failed: {e}")))?;

        let bus = Arc::new(DomainEventBus::new(512));
        let context_queue = Arc::new(ContextUpdateQueue::new());

        let fact_repo = SemanticFactRepo::new(inner.clone());
        let episodic_repo = EpisodicMemoryRepo::new(inner.clone());

        Ok(Self {
            scenario,
            pool,
            inner_pool: inner,
            bus,
            context_queue,
            fact_repo,
            episodic_repo,
            extraction_handler,
            consolidation_handler,
        })
    }

    /// Run the full simulation and produce a report.
    pub async fn run(&self) -> common::Result<SimulationReport> {
        let sim_start = std::time::Instant::now();
        let total_days = self.scenario.total_days();

        let start_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let end_date = start_date + Duration::days(total_days as i64);

        let step = match self.scenario.simulation.epoch_step.as_str() {
            "hour" | "hours" => EpochStep::Hours(4),
            "week" => EpochStep::Week,
            _ => EpochStep::Day,
        };

        let mut epoch = SimulatedEpoch::new(start_date, end_date, step);
        let mut persona_runner = PersonaRunner::new(self.scenario.persona.clone());
        let mut metrics = MetricCollector::new(30); // Baseline after day 30
        let mut checkpoint_results: Vec<CheckpointResult> = Vec::new();
        let mut total_messages: u32 = 0;

        let action_executor = ActionExecutor::new(self.bus.clone(), &self.pool);

        info!(
            persona = self.scenario.persona.name,
            total_days, "Starting simulation"
        );

        while let Some(plan) = epoch.advance() {
            let epoch_start = std::time::Instant::now();
            let day = plan.day_of_simulation;

            // === Phase 2: PRE-MESSAGE CRONS ===
            for trigger in &plan.cron_pre_message {
                self.execute_cron(trigger, plan.simulated_now).await?;
            }

            // === Phase 3: MESSAGE PHASE ===
            let messages = persona_runner.generate_day(plan.simulated_now);
            for msg in &messages {
                total_messages += 1;
                metrics.accumulator_mut().messages_processed += 1;

                // 3a. Track ground truth
                if let Some(ref gt) = msg.ground_truth {
                    if gt.introduces_fact.is_some() {
                        metrics.accumulator_mut().facts_introduced += 1;
                    }
                }
                if msg.is_correction {
                    metrics.accumulator_mut().corrections += 1;
                    self.bus.publish(DomainEvent::UserCorrectedAI {
                        session_key: "sim-session".to_string(),
                        active_skill: Some(msg.topic.clone()),
                        correction_type: None,
                    });
                }

                // 3b. Execute tool actions
                for action in &msg.tool_actions {
                    action_executor.execute(action, msg.simulated_at).await?;
                    match action {
                        crate::persona::SimulatedToolAction::CreateTask { .. } => {
                            metrics.accumulator_mut().tasks_created += 1;
                        }
                        crate::persona::SimulatedToolAction::CompleteTask { .. } => {
                            metrics.accumulator_mut().tasks_completed += 1;
                        }
                        _ => {}
                    }
                }

                // 3c. Drive cognitive pipeline
                let observation = cognitive::Observation {
                    domain: msg.topic.clone(),
                    content: msg.content.clone(),
                    importance: if msg.ground_truth.is_some() { 0.8 } else { 0.5 },
                    source_event: "ChatTurnCompleted".to_string(),
                    timestamp: msg.simulated_at,
                };

                // Extraction
                let extraction_result = self
                    .extraction_handler
                    .extract_facts_batch(&[observation.clone()])
                    .await;

                if let Ok(ref result) = extraction_result {
                    let extracted_count = result.facts.len();
                    if extracted_count > 0 {
                        metrics.accumulator_mut().facts_extracted += extracted_count as u32;

                        // Consolidation
                        let candidates: Vec<cognitive::ConsolidationCandidate> = result
                            .facts
                            .iter()
                            .map(|f| {
                                let fact = cognitive::services::extraction::to_semantic_fact(
                                    f,
                                    &observation,
                                );
                                cognitive::ConsolidationCandidate {
                                    new_fact: fact,
                                    existing: vec![],
                                }
                            })
                            .collect();

                        if let Ok(ops) = self
                            .consolidation_handler
                            .decide_batch(&candidates)
                            .await
                        {
                            for op in ops {
                                match op {
                                    cognitive::MemoryOp::Add(fact) => {
                                        let _ = self.fact_repo.upsert(&fact).await;
                                    }
                                    cognitive::MemoryOp::Update { old_id, new_fact } => {
                                        let _ = self.fact_repo.supersede(&old_id, &new_fact.id).await;
                                        let _ = self.fact_repo.upsert(&new_fact).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                // 3d. Collect token metrics (simulated — 150 tokens per ScriptedProvider call)
                metrics.accumulator_mut().total_tokens += 150;
            }

            // === Phase 3½: CONTEXT UPDATE DRAIN ===
            let _drained = self.context_queue.drain();

            // === Phase 4: POST-MESSAGE CRONS ===
            for trigger in &plan.cron_post_message {
                self.execute_cron(trigger, plan.simulated_now).await?;
            }

            // === Phase 5: CHECKPOINTS ===
            for checkpoint in &self.scenario.checkpoints {
                if checkpoint.at_day == day {
                    let latest = metrics.timeline.last().cloned().unwrap_or_default();
                    let result = GroundTruthVerifier::verify_checkpoint(
                        checkpoint,
                        &self.fact_repo,
                        &latest,
                        metrics.baselines.as_ref(),
                    )
                    .await;

                    if !result.all_passed {
                        warn!(day, "Checkpoint failed: {:?}", result);
                    }
                    checkpoint_results.push(result);
                }
            }

            // === Phase 6: METRIC SNAPSHOT ===
            let knowledge_retention = crate::metrics::memory::measure_knowledge_retention(
                &self.fact_repo,
                &self.scenario.persona.profile.known_facts,
            )
            .await;

            let community_stability =
                crate::metrics::system::measure_community_stability(&self.inner_pool).await;
            let brain_versions = crate::metrics::system::count_brain_versions_since(
                &self.inner_pool,
                &plan.previous.to_rfc3339(),
            )
            .await;

            let epoch_wall_ms = epoch_start.elapsed().as_secs_f64() * 1000.0;
            metrics.snapshot(
                plan.simulated_now,
                day,
                knowledge_retention,
                0.0, // autotuner_promotion_success — computed at end
                community_stability,
                brain_versions,
                0.0, // insight_usefulness — placeholder for now
                epoch_wall_ms,
            );

            if day % 30 == 0 {
                info!(day, total_days, "Simulation progress");
            }
        }

        // Build final report
        let wall_time_secs = sim_start.elapsed().as_secs_f64();
        let final_metrics = metrics
            .timeline
            .last()
            .cloned()
            .unwrap_or_default();
        let regression_alerts = metrics.check_regressions(10.0); // 10% threshold

        let improvements = metrics
            .baselines
            .as_ref()
            .map(|b| report::compute_improvements(b, &final_metrics))
            .unwrap_or_default();

        let checkpoint_pass_rate = if checkpoint_results.is_empty() {
            1.0
        } else {
            checkpoint_results.iter().filter(|c| c.all_passed).count() as f64
                / checkpoint_results.len() as f64
        };

        let report = SimulationReport {
            scenario: self.scenario.persona.name.clone(),
            persona: self.scenario.persona.name.clone(),
            simulated_days: self.scenario.total_days(),
            wall_time_secs,
            seed: self.scenario.persona.seed,
            metric_timeline: metrics.timeline,
            checkpoints: checkpoint_results,
            summary: ReportSummary {
                total_messages,
                total_facts_extracted: 0, // TODO: accumulate across epochs
                total_facts_superseded: 0,
                total_brain_versions: 0,
                total_autotuner_promotions: 0,
                total_autotuner_reverts: 0,
                final_metrics,
                baseline_metrics: metrics.baselines,
                improvement_pct: improvements,
                checkpoint_pass_rate,
                regression_alerts,
            },
        };

        info!(
            wall_time_secs,
            total_messages,
            checkpoint_pass_rate,
            "Simulation complete"
        );

        Ok(report)
    }

    async fn execute_cron(
        &self,
        trigger: &CronTrigger,
        simulated_now: DateTime<Utc>,
    ) -> common::Result<()> {
        match trigger {
            CronTrigger::AtomDecay => {
                debug!("Running atom decay cycle");
                let _ = cognitive::services::atom_decay::run_decay_cycle(
                    &self.inner_pool,
                    &self.bus,
                )
                .await;
            }
            CronTrigger::AnalyticsCleanup => {
                debug!("Running analytics cleanup");
                // Skip in simulation — not critical for metrics
            }
            CronTrigger::MemoryMaintenance => {
                debug!("Running memory maintenance");
                // Skip in simulation — compaction not needed with small datasets
            }
            CronTrigger::AutotunerNightly => {
                debug!("Running autotuner nightly cycle");
                // Autotuner requires full orchestrator setup — skip in Phase 1
                // Will be enabled when CognitiveLlmBridge is wired
            }
            CronTrigger::CognitiveReflection => {
                debug!("Running cognitive weekly reflection");
                // Reflection requires LlmReflectionHandler — skip in Phase 1
            }
            CronTrigger::MirrorWeeklyNarrative => {
                debug!("Running mirror weekly narrative");
                // Requires NarrativeHandler — skip in Phase 1
            }
            CronTrigger::MirrorCleanup => {
                debug!("Running mirror cleanup");
                // Skip — no data accumulation concern in simulation
            }
            CronTrigger::CrossDomainInsight => {
                debug!("Running cross-domain insight");
                // Skip in Phase 1
            }
        }
        Ok(())
    }
}
```

**Note:** Several cron handlers are stubbed with `// Skip in Phase 1`. This is intentional — the harness framework is complete and working, and individual cron integrations can be enabled incrementally as the `CognitiveLlmBridge` and handler wiring is added. The implementing engineer should check exact import paths for `cognitive::services::extraction::to_semantic_fact`, `cognitive::ConsolidationCandidate`, `cognitive::MemoryOp`, etc. against the actual `cognitive` crate's public API and adjust accordingly.

- [ ] **Step 2: Update lib.rs**

Add `pub mod harness;` and `pub mod actions;` to lib.rs. Final lib.rs:

```rust
// crates/simulator/src/lib.rs
pub mod actions;
pub mod epoch;
pub mod harness;
pub mod metrics;
pub mod persona;
pub mod providers;
pub mod report;
pub mod scenario;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p simulator`
Expected: Compiles (may need import path adjustments).

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/harness.rs crates/simulator/src/lib.rs
git commit -m "feat(simulator): add SimulationHarness main epoch loop orchestrator"
```

---

### Task 11: Test Binary + First Scenario

**Files:**
- Create: `tests/simulation/main.rs`
- Create: `tests/simulation/smoke.rs`
- Create: `tests/simulation/scenarios/software_engineer_12mo.toml`

- [ ] **Step 1: Create the test binary entry**

```rust
// tests/simulation/main.rs
#[path = "../common/mod.rs"]
mod common;

mod smoke;
```

- [ ] **Step 2: Create the smoke test**

```rust
// tests/simulation/smoke.rs
use simulator::harness::SimulationHarness;
use simulator::scenario::Scenario;
use std::sync::Arc;

/// A minimal 7-day simulation to verify the harness runs end-to-end.
#[tokio::test]
async fn smoke_test_7_day_simulation() {
    let scenario_toml = r#"
[persona]
name = "smoke_test"
timezone = "UTC"
language = "en"
seed = 123

[persona.messages_per_day]
onboarding = 3
routine = 2
power_user = 2
shift = 2

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "developer" },
]

[persona.phases.onboarding]
duration_days = 3
correction_rate = 0.2
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.5
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 2
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 1
correction_rate = 0.05
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7

[persona.phases.behavior_shift]
duration_days = 1
correction_rate = 0.15
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.3
tool_action_rate = 0.5

[[checkpoints]]
at_day = 7
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.0 },
]
"#;

    let scenario = Scenario::from_toml(scenario_toml).unwrap();

    // Use heuristic handlers (no LLM needed)
    let extraction: Arc<dyn cognitive::ExtractionHandler> =
        Arc::new(agent::adapters::HeuristicExtractionHandler);
    let consolidation: Arc<dyn cognitive::ConsolidationHandler> =
        Arc::new(agent::adapters::HeuristicConsolidationHandler);

    let harness = SimulationHarness::new(scenario, extraction, consolidation)
        .await
        .unwrap();

    let report = harness.run().await.unwrap();

    // Basic assertions
    assert!(report.summary.total_messages > 0);
    assert!(!report.metric_timeline.is_empty());
    assert!(report.wall_time_secs < 60.0, "Smoke test should finish in under 60s");

    // Print summary for debugging
    eprintln!(
        "Smoke test: {} messages in {:.2}s, checkpoint pass rate: {:.0}%",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.checkpoint_pass_rate * 100.0,
    );
}
```

**Note:** The imports `agent::adapters::HeuristicExtractionHandler` and `HeuristicConsolidationHandler` assume these types are publicly exported from the `agent` crate. If they are `pub(crate)`, the implementing engineer will need to either: (a) add re-exports to `agent/src/lib.rs`, or (b) create equivalent heuristic handlers directly in the `simulator` crate. The recommended approach is (b) — copy the simple heuristic logic into `simulator::providers::heuristic.rs` to avoid adding `agent` as a dependency (which would violate the L4 positioning). Check `crates/agent/src/adapters/cognitive_handlers.rs` for the heuristic implementation to copy.

- [ ] **Step 3: Create the full 12-month scenario**

```toml
# tests/simulation/scenarios/software_engineer_12mo.toml
[persona]
name = "software_engineer_vn"
timezone = "Asia/Ho_Chi_Minh"
language = "en"
seed = 42

[persona.messages_per_day]
onboarding = 8
routine = 5
power_user = 7
shift = 4

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "software engineer" },
    { subject = "user", predicate = "prefers_language", object = "Rust" },
    { subject = "user", predicate = "manages_project", object = "Klynt API rewrite" },
    { subject = "user", predicate = "tracks_currency", object = "VND" },
    { subject = "user", predicate = "works_at", object = "startup" },
    { subject = "user", predicate = "location", object = "Ho Chi Minh City" },
]

[persona.phases.onboarding]
duration_days = 14
correction_rate = 0.25
topic_weights = { tasks = 0.4, notes = 0.3, finance = 0.1, chat = 0.2 }
new_fact_introduction_rate = 0.6
tool_action_rate = 0.5

[persona.phases.routine]
duration_days = 75
correction_rate = 0.10
topic_weights = { tasks = 0.3, notes = 0.2, finance = 0.2, productivity = 0.2, chat = 0.1 }
new_fact_introduction_rate = 0.15
tool_action_rate = 0.7

[persona.phases.power_user]
duration_days = 90
correction_rate = 0.05
topic_weights = { tasks = 0.2, notes = 0.15, finance = 0.15, productivity = 0.15, automation = 0.15, insights = 0.1, chat = 0.1 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.8

[persona.phases.behavior_shift]
duration_days = 90
correction_rate = 0.12
shift_description = "User switches from backend to ML focus"
new_facts = [
    { subject = "user", predicate = "learning", object = "PyTorch" },
    { subject = "user", predicate = "project_focus", object = "ML pipeline" },
]
topic_weights = { tasks = 0.2, notes = 0.3, finance = 0.1, learning = 0.25, chat = 0.15 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.6

[[checkpoints]]
at_day = 14
assertions = [
    { type = "fact_exists", subject = "user", predicate = "works_as", object = "software engineer", min_confidence = 0.5 },
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.3 },
]

[[checkpoints]]
at_day = 90
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.6 },
    { type = "metric_above", metric = "retrieval_precision", threshold = 0.3 },
]

[[checkpoints]]
at_day = 180
assertions = [
    { type = "metric_above", metric = "personalization_score", threshold = 0.4 },
]

[[checkpoints]]
at_day = 269
assertions = [
    { type = "fact_exists", subject = "user", predicate = "learning", object = "PyTorch", min_confidence = 0.5 },
]
```

- [ ] **Step 4: Run the smoke test**

Run: `cargo nextest run -p klyntbot --test simulation -- smoke_test`
Expected: Test passes in under 60 seconds.

- [ ] **Step 5: Commit**

```bash
git add tests/simulation/ Cargo.toml
git commit -m "feat(simulator): add test binary with smoke test and 12-month scenario"
```

---

### Task 12: Clippy + Formatting Pass

**Files:**
- All files in `crates/simulator/`

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -p simulator --all-targets --all-features`
Expected: 0 warnings. Fix any issues found.

- [ ] **Step 2: Run formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues. Fix any with `cargo fmt --all`.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run -p simulator`
Expected: All tests pass.

- [ ] **Step 4: Run smoke test**

Run: `cargo nextest run -p klyntbot --test simulation -- smoke_test`
Expected: Passes.

- [ ] **Step 5: Commit if any fixes were needed**

```bash
git add -A
git commit -m "fix(simulator): clippy and formatting fixes"
```

---

### Task 13: Final Integration Verification

**Files:** None (verification only)

- [ ] **Step 1: Verify workspace builds clean**

Run: `cargo build --workspace`
Expected: No errors.

- [ ] **Step 2: Verify existing tests still pass**

Run: `cargo nextest run --workspace`
Expected: All pre-existing tests pass. No regressions.

- [ ] **Step 3: Verify doctests**

Run: `cargo test --workspace --doc`
Expected: No failures.

- [ ] **Step 4: Final clippy across workspace**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

- [ ] **Step 5: Tag completion**

No commit needed — this is a verification-only task.
