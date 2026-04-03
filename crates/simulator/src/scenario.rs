use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::persona::{Persona, PersonaPhases};

// ── Simulation config ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    #[serde(default = "default_cognitive_llm_model")]
    pub cognitive_llm_model: String,
    #[serde(default = "default_cognitive_temperature")]
    pub cognitive_temperature: f64,
    #[serde(default = "default_max_cognitive_calls_per_day")]
    pub max_cognitive_calls_per_day: u32,
    #[serde(default = "default_epoch_step")]
    pub epoch_step: String,
    /// Percentage threshold for regression detection (default: 10.0).
    /// Per-epoch metrics with high variance (task_completion_rate, routing_stability)
    /// may need a higher tolerance (e.g. 25.0).
    #[serde(default = "default_regression_threshold")]
    pub regression_threshold: f64,
    /// Enable the agent execution path (dual-mode). Default: false.
    #[serde(default)]
    pub agent_mode: bool,
    /// Maximum breakpoint rate before CI failure (default: 0.20 = 20%).
    #[serde(default = "default_agent_breakpoint_threshold")]
    pub agent_breakpoint_threshold: f64,
    /// ReAct loop iteration limit for agent path.
    #[serde(default = "default_agent_max_iterations")]
    pub agent_max_iterations: u32,
}

fn default_cognitive_llm_model() -> String {
    "heuristic".to_string()
}

fn default_cognitive_temperature() -> f64 {
    0.3
}

fn default_max_cognitive_calls_per_day() -> u32 {
    12
}

fn default_epoch_step() -> String {
    "day".to_string()
}

fn default_regression_threshold() -> f64 {
    10.0
}

fn default_agent_breakpoint_threshold() -> f64 {
    0.20
}

fn default_agent_max_iterations() -> u32 {
    15
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            cognitive_llm_model: default_cognitive_llm_model(),
            cognitive_temperature: default_cognitive_temperature(),
            max_cognitive_calls_per_day: default_max_cognitive_calls_per_day(),
            epoch_step: default_epoch_step(),
            regression_threshold: default_regression_threshold(),
            agent_mode: false,
            agent_breakpoint_threshold: default_agent_breakpoint_threshold(),
            agent_max_iterations: default_agent_max_iterations(),
        }
    }
}

// ── Metric names ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    RoutingAccuracy,
    ResponseQuality,
    SalienceExtractRate,
    MemoryRetrievability,
    MetaRuleCount,
    InsightUsefulness,
    AutotunerPromotionSuccess,
    CommunityStability,
    BrainVersionVelocity,
    // Tier 5 — agent path metrics
    AgentRoutingAccuracy,
    AgentToolSelection,
    AgentModeDistribution,
    ReactConvergenceRate,
    AgentResponseQuality,
}

// ── Checkpoint assertions ──────────────────────────────────────────────

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
    FactNotExists {
        subject: String,
        predicate: String,
        object: String,
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

// ── Checkpoint ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub at_day: u32,
    pub assertions: Vec<CheckpointAssertion>,
}

// ── Scenario ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub persona: Persona,
    #[serde(default)]
    pub simulation: SimulationConfig,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
}

impl Scenario {
    /// Parse a scenario from a TOML string.
    pub fn from_toml(content: &str) -> common::Result<Self> {
        toml::from_str(content).map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to parse scenario TOML: {e}"
            )))
        })
    }

    /// Load a scenario from a TOML file on disk.
    pub fn from_file(path: &Path) -> common::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Total simulation duration across all persona phases.
    pub fn total_days(&self) -> u32 {
        let phases: &PersonaPhases = &self.persona.phases;
        phases.onboarding.duration_days
            + phases.routine.duration_days
            + phases.power_user.duration_days
            + phases.behavior_shift.duration_days
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
known_facts = [{ subject = "user", predicate = "works_as", object = "engineer" }]

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
assertions = [{ type = "fact_exists", subject = "user", predicate = "works_as", object = "engineer", min_confidence = 0.5 }]
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
