//! Pure MCP exposure projection and override validation.
//!
//! Shared by AppCore post_init, klyntbot-server diagnostics/stdio, and tests.
//! Does not execute tools or own HTTP/stdio servers.

use tools_core::{McpExposure, ToolRegistry};

/// Closed, server-owned MCP builtin identities (ADR-0101 / EXPO-3.1, EXPO-3.7).
///
/// Not registry tools — handlers keep schemas; this enum is the single
/// authority for names and mandatory / default-on semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinId {
    /// Mandatory; always advertised, unaffected by overrides.
    GetStatus,
    /// Configurable; default-on when the override list is empty.
    Agent,
}

impl BuiltinId {
    /// Wire name advertised to MCP clients.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GetStatus => "get_status",
            Self::Agent => "agent",
        }
    }

    /// Parse a closed-set builtin name.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "get_status" => Some(Self::GetStatus),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }

    /// Every builtin in the closed set (stable order).
    pub const fn all() -> &'static [Self] {
        &[Self::GetStatus, Self::Agent]
    }

    /// Always included in the effective builtin set.
    pub const fn is_mandatory(self) -> bool {
        matches!(self, Self::GetStatus)
    }

    /// Included when the override list is empty / absent.
    pub const fn default_on(self) -> bool {
        matches!(self, Self::GetStatus | Self::Agent)
    }
}

/// Why an override name was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RejectionReason {
    /// Name is neither a live-registry tool nor a closed builtin.
    Unknown,
    /// Live-registry tool whose MCP exposure is [`McpExposure::Forbidden`].
    Forbidden,
}

impl RejectionReason {
    /// Wire / diagnostic reason token (`unknown` | `forbidden`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Forbidden => "forbidden",
        }
    }
}

/// One rejected override entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedEntry {
    pub name: String,
    pub reason: RejectionReason,
}

/// Runtime MCP subsystem state after validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeState {
    /// Server enabled and override validated.
    Ready,
    /// Server disabled in config (projection may still be computed).
    Disabled,
    /// Server enabled but override contains rejected names.
    Invalid,
}

impl RuntimeState {
    /// Diagnostic / UI token (`ready` | `disabled` | `invalid`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Disabled => "disabled",
            Self::Invalid => "invalid",
        }
    }
}

/// Inputs to [`validate_mcp_exposure`].
///
/// Callers holding `tokio::sync::RwLock` on the registry pass `&*guard`.
pub struct ExposureInput<'a> {
    pub registry: &'a ToolRegistry,
    pub server_enabled: bool,
    /// `None` or empty slice → auto-default (all MCP `Default` tools + default-on builtins).
    pub override_tools: Option<&'a [String]>,
}

/// Structured validation / projection result (EXPO-3.10, EXPO-4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureValidation {
    pub runtime_state: RuntimeState,
    /// Requested override names (empty when auto-default).
    pub requested: Vec<String>,
    /// Registry-backed tools accepted into the effective MCP set.
    pub effective_registry_tools: Vec<String>,
    /// Selected builtins (`GetStatus` always; `Agent` when default-on or named).
    pub effective_builtins: Vec<BuiltinId>,
    /// Rejected override entries with reasons.
    pub rejected: Vec<RejectedEntry>,
}

/// Project live-registry MCP membership and validate an optional override.
///
/// Pure: no app-core / agent / I/O dependencies.
pub fn validate_mcp_exposure(input: ExposureInput<'_>) -> ExposureValidation {
    let requested: Vec<String> = input
        .override_tools
        .map(|names| names.to_vec())
        .unwrap_or_default();
    let use_defaults = requested.is_empty();

    let mut effective_registry_tools = Vec::new();
    let mut effective_builtins = vec![BuiltinId::GetStatus];
    let mut rejected = Vec::new();

    if use_defaults {
        for tool in input.registry.dyn_tools() {
            if tool.exposure_policy().mcp == McpExposure::Default {
                effective_registry_tools.push(tool.name().to_string());
            }
        }
        effective_registry_tools.sort();
        for builtin in BuiltinId::all() {
            if builtin.default_on() && !matches!(builtin, BuiltinId::GetStatus) {
                effective_builtins.push(*builtin);
            }
        }
    } else {
        let mut seen = std::collections::HashSet::new();
        let mut agent_selected = false;

        for name in &requested {
            if !seen.insert(name.as_str()) {
                continue;
            }

            match BuiltinId::parse(name) {
                Some(BuiltinId::GetStatus) => {
                    // Mandatory; already in effective_builtins.
                }
                Some(BuiltinId::Agent) => {
                    agent_selected = true;
                }
                None => match input.registry.get(name) {
                    Some(tool) => match tool.exposure_policy().mcp {
                        McpExposure::Default | McpExposure::OptIn => {
                            effective_registry_tools.push(name.clone());
                        }
                        McpExposure::Forbidden => {
                            rejected.push(RejectedEntry {
                                name: name.clone(),
                                reason: RejectionReason::Forbidden,
                            });
                        }
                    },
                    None => {
                        rejected.push(RejectedEntry {
                            name: name.clone(),
                            reason: RejectionReason::Unknown,
                        });
                    }
                },
            }
        }

        if agent_selected {
            effective_builtins.push(BuiltinId::Agent);
        }
    }

    let runtime_state = if !input.server_enabled {
        RuntimeState::Disabled
    } else if !rejected.is_empty() {
        RuntimeState::Invalid
    } else {
        RuntimeState::Ready
    };

    ExposureValidation {
        runtime_state,
        requested,
        effective_registry_tools,
        effective_builtins,
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use tools_core::{ExposurePolicy, RoutingContext, Tool};

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
            "test tool"
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

    fn registry_with(tools: &[(&'static str, McpExposure)]) -> ToolRegistry {
        let mut reg = ToolRegistry::new();
        for (name, mcp) in tools {
            reg.register(NamedTool {
                name,
                mcp: *mcp,
            });
        }
        reg
    }

    fn names(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    #[test]
    fn builtin_id_closed_set_and_wire_names() {
        assert_eq!(BuiltinId::all(), &[BuiltinId::GetStatus, BuiltinId::Agent]);
        assert_eq!(BuiltinId::GetStatus.as_str(), "get_status");
        assert_eq!(BuiltinId::Agent.as_str(), "agent");
        assert!(BuiltinId::GetStatus.is_mandatory());
        assert!(!BuiltinId::Agent.is_mandatory());
        assert!(BuiltinId::GetStatus.default_on());
        assert!(BuiltinId::Agent.default_on());
        assert_eq!(BuiltinId::parse("get_status"), Some(BuiltinId::GetStatus));
        assert_eq!(BuiltinId::parse("agent"), Some(BuiltinId::Agent));
        assert_eq!(BuiltinId::parse("tasks"), None);
        assert_eq!(RejectionReason::Unknown.as_str(), "unknown");
        assert_eq!(RejectionReason::Forbidden.as_str(), "forbidden");
        assert_eq!(RuntimeState::Ready.as_str(), "ready");
        assert_eq!(RuntimeState::Disabled.as_str(), "disabled");
        assert_eq!(RuntimeState::Invalid.as_str(), "invalid");
    }

    #[test]
    fn empty_override_projects_defaults_only_plus_default_on_builtins() {
        let reg = registry_with(&[
            ("tasks", McpExposure::Default),
            ("notes", McpExposure::Default),
            ("shell", McpExposure::Forbidden),
            ("web_fetch", McpExposure::OptIn),
        ]);

        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: None,
        });

        assert_eq!(result.runtime_state, RuntimeState::Ready);
        assert!(result.requested.is_empty());
        assert_eq!(names(&result.effective_registry_tools), vec!["notes", "tasks"]);
        assert_eq!(
            result.effective_builtins,
            vec![BuiltinId::GetStatus, BuiltinId::Agent]
        );
        assert!(result.rejected.is_empty());
    }

    #[test]
    fn empty_slice_override_same_as_none() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);
        let empty: &[String] = &[];
        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(empty),
        });
        assert_eq!(result.runtime_state, RuntimeState::Ready);
        assert_eq!(names(&result.effective_registry_tools), vec!["tasks"]);
        assert!(result.effective_builtins.contains(&BuiltinId::Agent));
    }

    #[test]
    fn override_accepts_default_and_opt_in_rejects_forbidden_and_unknown() {
        let reg = registry_with(&[
            ("tasks", McpExposure::Default),
            ("web_fetch", McpExposure::OptIn),
            ("shell", McpExposure::Forbidden),
        ]);
        let override_tools = vec![
            "tasks".into(),
            "web_fetch".into(),
            "shell".into(),
            "nope".into(),
        ];

        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&override_tools),
        });

        assert_eq!(result.runtime_state, RuntimeState::Invalid);
        assert_eq!(result.requested, override_tools);
        assert_eq!(
            names(&result.effective_registry_tools),
            vec!["tasks", "web_fetch"]
        );
        // Explicit override without `agent` excludes the configurable builtin.
        assert_eq!(result.effective_builtins, vec![BuiltinId::GetStatus]);
        assert_eq!(
            result.rejected,
            vec![
                RejectedEntry {
                    name: "shell".into(),
                    reason: RejectionReason::Forbidden,
                },
                RejectedEntry {
                    name: "nope".into(),
                    reason: RejectionReason::Unknown,
                },
            ]
        );
    }

    #[test]
    fn get_status_always_in_effective_builtins() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);

        let auto = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: None,
        });
        assert!(auto.effective_builtins.contains(&BuiltinId::GetStatus));

        let override_tools = vec!["tasks".into()];
        let explicit = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&override_tools),
        });
        assert!(explicit.effective_builtins.contains(&BuiltinId::GetStatus));
        assert!(!explicit.effective_builtins.contains(&BuiltinId::Agent));
    }

    #[test]
    fn agent_default_on_and_overrideable() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);

        let auto = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: None,
        });
        assert!(auto.effective_builtins.contains(&BuiltinId::Agent));

        let with_agent = vec!["tasks".into(), "agent".into()];
        let included = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&with_agent),
        });
        assert_eq!(
            included.effective_builtins,
            vec![BuiltinId::GetStatus, BuiltinId::Agent]
        );
        assert_eq!(names(&included.effective_registry_tools), vec!["tasks"]);
        assert!(included.rejected.is_empty());
        assert_eq!(included.runtime_state, RuntimeState::Ready);

        let without_agent = vec!["tasks".into()];
        let excluded = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&without_agent),
        });
        assert_eq!(excluded.effective_builtins, vec![BuiltinId::GetStatus]);
    }

    #[test]
    fn naming_get_status_in_override_is_not_rejected() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);
        let override_tools = vec!["get_status".into(), "tasks".into()];
        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&override_tools),
        });
        assert_eq!(result.runtime_state, RuntimeState::Ready);
        assert!(result.rejected.is_empty());
        assert_eq!(result.effective_builtins, vec![BuiltinId::GetStatus]);
        assert_eq!(names(&result.effective_registry_tools), vec!["tasks"]);
    }

    #[test]
    fn server_disabled_yields_disabled_even_when_otherwise_valid() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);
        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: false,
            override_tools: None,
        });
        assert_eq!(result.runtime_state, RuntimeState::Disabled);
        assert_eq!(names(&result.effective_registry_tools), vec!["tasks"]);
        assert!(result.rejected.is_empty());
    }

    #[test]
    fn server_disabled_precedes_invalid_when_override_rejects() {
        let reg = registry_with(&[("shell", McpExposure::Forbidden)]);
        let override_tools = vec!["shell".into(), "ghost".into()];
        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: false,
            override_tools: Some(&override_tools),
        });
        assert_eq!(result.runtime_state, RuntimeState::Disabled);
        assert_eq!(result.rejected.len(), 2);
        assert!(result.effective_registry_tools.is_empty());
    }

    #[test]
    fn forbidden_and_unknown_never_enter_effective_set() {
        let reg = registry_with(&[
            ("tasks", McpExposure::Default),
            ("shell", McpExposure::Forbidden),
        ]);
        let override_tools = vec!["tasks".into(), "shell".into(), "unknown_tool".into()];
        let result = validate_mcp_exposure(ExposureInput {
            registry: &reg,
            server_enabled: true,
            override_tools: Some(&override_tools),
        });
        assert!(!result
            .effective_registry_tools
            .iter()
            .any(|n| n == "shell" || n == "unknown_tool"));
        assert_eq!(names(&result.effective_registry_tools), vec!["tasks"]);
        assert_eq!(result.runtime_state, RuntimeState::Invalid);
    }
}
