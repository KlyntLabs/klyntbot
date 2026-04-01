use crate::scenario::SimulationConfig;

/// Configuration for optional real-LLM cognitive extraction.
///
/// When `SIMULATION_COGNITIVE_LLM` env var is set to a model name
/// (e.g., "claude-haiku-4-5-20251001"), the test binary can construct
/// real LLM-backed extraction/consolidation handlers instead of heuristic ones.
///
/// Default: "heuristic" (no LLM, use HeuristicExtractionHandler).
pub struct CognitiveBridgeConfig {
    pub model: String,
    pub temperature: f64,
    pub max_calls_per_day: u32,
}

impl From<&SimulationConfig> for CognitiveBridgeConfig {
    fn from(config: &SimulationConfig) -> Self {
        Self {
            model: std::env::var("SIMULATION_COGNITIVE_LLM")
                .unwrap_or_else(|_| config.cognitive_llm_model.clone()),
            temperature: config.cognitive_temperature,
            max_calls_per_day: config.max_cognitive_calls_per_day,
        }
    }
}

impl CognitiveBridgeConfig {
    /// Returns true if heuristic (no-LLM) mode is selected.
    pub fn is_heuristic(&self) -> bool {
        self.model == "heuristic" || self.model.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_heuristic() {
        let sim_config = SimulationConfig::default();
        let bridge = CognitiveBridgeConfig::from(&sim_config);
        assert!(bridge.is_heuristic());
        assert_eq!(bridge.temperature, 0.3);
        assert_eq!(bridge.max_calls_per_day, 12);
    }
}
