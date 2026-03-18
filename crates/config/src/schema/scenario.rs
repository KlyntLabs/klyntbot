use serde::{Deserialize, Serialize};

/// Configuration for generalized scenario/what-if reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    /// Maximum graph neighborhood depth for cascading effect analysis.
    /// Higher depth = more entities explored but slower queries.
    #[serde(default = "default_max_graph_depth")]
    pub max_graph_depth: u32,
}

fn default_max_graph_depth() -> u32 {
    2
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            max_graph_depth: default_max_graph_depth(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ScenarioConfig::default();
        assert_eq!(config.max_graph_depth, 2);
    }

    #[test]
    fn test_deserialize_empty_object() {
        let json = "{}";
        let config: ScenarioConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_graph_depth, 2);
    }

    #[test]
    fn test_deserialize_custom_depth() {
        let json = r#"{"maxGraphDepth": 3}"#;
        let config: ScenarioConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_graph_depth, 3);
    }
}
