//! Migration fixture (EXPO-2.5, EXPO-2.7, EXPO-7.1):
//! policy-derived MCP Defaults = historical AiFeature∪allowlist − agent − eight EXPO-2.3 stubs;
//! advertised = builtins + Defaults; Forbidden tools never silently expand.

use std::collections::HashSet;

use async_trait::async_trait;
use mcp::server::exposure::{validate_mcp_exposure, BuiltinId, ExposureInput, RuntimeState};
use serde_json::{json, Value};
use tools_core::{
    ExposurePolicy, McpExposure, RoutingContext, Tool, ToolRegistry, EXPO_23_FORBIDDEN_STUB_TOOLS,
    HISTORICAL_MCP_DEFAULT_TOOLS,
};

struct NamedTool {
    name: &'static str,
    mcp: McpExposure,
}

#[async_trait]
impl Tool for NamedTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "migration fixture tool"
    }
    fn parameters(&self) -> Value {
        json!({"type": "object"})
    }
    async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
        Ok("ok".into())
    }
    fn exposure_policy(&self) -> ExposurePolicy {
        ExposurePolicy {
            mcp: self.mcp,
            ..Default::default()
        }
    }
}

/// Pre-cutover historical membership: AiFeatureRegistry tool_names ∪ EXPLICIT_TOOL_ALLOWLIST.
fn historical_union() -> HashSet<&'static str> {
    let reg = app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> = config::schema::EXPLICIT_TOOL_ALLOWLIST
        .iter()
        .copied()
        .collect();
    registry_tools.union(&allowlist).copied().collect()
}

#[test]
fn registry_defaults_equal_historical_union_minus_agent_and_expo23_stubs() {
    let historical = historical_union();
    let expected: HashSet<&'static str> = historical
        .iter()
        .copied()
        .filter(|n| *n != "agent" && !EXPO_23_FORBIDDEN_STUB_TOOLS.contains(n))
        .collect();
    let locked: HashSet<&'static str> = HISTORICAL_MCP_DEFAULT_TOOLS.iter().copied().collect();
    assert_eq!(
        locked, expected,
        "HISTORICAL_MCP_DEFAULT_TOOLS must equal pre-cutover union − agent − eight stubs"
    );
    assert!(!locked.contains("agent"));
    for stub in EXPO_23_FORBIDDEN_STUB_TOOLS {
        assert!(
            !locked.contains(stub),
            "{stub} must not remain a registry Default"
        );
        assert!(
            historical.contains(stub),
            "{stub} must appear in historical union as a named removal"
        );
    }
}

#[test]
fn advertised_equals_builtins_plus_defaults_without_forbidden_expansion() {
    let mut reg = ToolRegistry::new();
    for name in HISTORICAL_MCP_DEFAULT_TOOLS {
        reg.register(NamedTool {
            name,
            mcp: McpExposure::Default,
        });
    }
    for name in EXPO_23_FORBIDDEN_STUB_TOOLS {
        reg.register(NamedTool {
            name,
            mcp: McpExposure::Forbidden,
        });
    }
    // Unreviewed / other Forbidden tools must not silently enter Defaults.
    reg.register(NamedTool {
        name: "shell",
        mcp: McpExposure::Forbidden,
    });
    reg.register(NamedTool {
        name: "web_fetch",
        mcp: McpExposure::OptIn,
    });

    let result = validate_mcp_exposure(ExposureInput {
        registry: &reg,
        server_enabled: true,
        override_tools: None,
    });

    assert_eq!(result.runtime_state, RuntimeState::Ready);

    let defaults: HashSet<&str> = result
        .effective_registry_tools
        .iter()
        .map(String::as_str)
        .collect();
    let expected_defaults: HashSet<&str> = HISTORICAL_MCP_DEFAULT_TOOLS.iter().copied().collect();
    assert_eq!(defaults, expected_defaults);

    for forbidden in EXPO_23_FORBIDDEN_STUB_TOOLS
        .iter()
        .copied()
        .chain(["shell", "web_fetch", "agent"])
    {
        assert!(
            !defaults.contains(forbidden),
            "Forbidden/OptIn/builtin {forbidden} must not silently expand Defaults"
        );
    }

    assert_eq!(
        result.effective_builtins,
        vec![BuiltinId::GetStatus, BuiltinId::Agent]
    );

    let mut advertised: HashSet<&str> = defaults;
    for builtin in &result.effective_builtins {
        advertised.insert(builtin.as_str());
    }
    assert!(advertised.contains("get_status"));
    assert!(advertised.contains("agent"));
    assert_eq!(
        advertised.len(),
        HISTORICAL_MCP_DEFAULT_TOOLS.len() + 2,
        "advertised = builtins + registry Defaults"
    );

    // Complete advertised set == pre-cutover advertised − eight stubs
    // (pre-cutover exposed_tools union plus handler-owned get_status).
    let historical = historical_union();
    let mut pre_cutover_advertised: HashSet<&str> = historical;
    pre_cutover_advertised.insert("get_status");
    let expected_complete: HashSet<&str> = pre_cutover_advertised
        .iter()
        .copied()
        .filter(|n| !EXPO_23_FORBIDDEN_STUB_TOOLS.contains(n))
        .collect();
    // agent remains via configurable builtin; already in historical.
    assert_eq!(advertised, expected_complete);
}

#[test]
fn no_overlap_between_aifeature_tool_names_and_allowlist() {
    let reg = app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> = config::schema::EXPLICIT_TOOL_ALLOWLIST
        .iter()
        .copied()
        .collect();
    let overlap: HashSet<_> = registry_tools.intersection(&allowlist).collect();
    assert!(
        overlap.is_empty(),
        "tool {} is registered as both an AiFeature tool_name and in EXPLICIT_TOOL_ALLOWLIST",
        overlap.iter().next().map(|s| **s).unwrap_or("")
    );
}
