//! MCP (Model Context Protocol) configuration.
//!
//! Defines config types for MCP client connections and server settings.
//! The `mcp` crate imports these types from `config` (not the other way around)
//! to avoid circular dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::Secret;

/// Top-level MCP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(default = "super::core::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerDef>,
    #[serde(default)]
    pub server: McpServerSettings,
}

impl McpConfig {
    /// Whether any MCP server connections are configured and enabled.
    pub fn has_active_servers(&self) -> bool {
        self.enabled && self.servers.iter().any(|s| s.enabled)
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: Vec::new(),
            server: McpServerSettings::default(),
        }
    }
}

/// A single MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDef {
    pub name: String,
    #[serde(flatten)]
    pub transport: McpTransport,
    #[serde(default = "super::core::default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<McpOAuthCredentials>,

    /// Startup timeout in seconds (connection + tool discovery). Default: 10.
    #[serde(default = "default_startup_timeout_sec")]
    pub startup_timeout_sec: u64,

    /// Per-tool-call timeout in seconds. Default: 120.
    #[serde(default = "default_tool_timeout_sec")]
    pub tool_timeout_sec: u64,

    /// Allowlist of tool names (original MCP names, not namespaced).
    /// When set, only these tools are registered from this server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_tools: Option<Vec<String>>,

    /// Denylist of tool names.
    /// When set, these tools are excluded from registration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_tools: Option<Vec<String>>,
}

impl McpServerDef {
    /// Check whether a tool (by its original MCP name) is allowed by the
    /// allowlist/denylist filters. Allowlist takes precedence over denylist.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if let Some(ref allowed) = self.enabled_tools {
            return allowed.iter().any(|n| n == tool_name);
        }
        if let Some(ref denied) = self.disabled_tools {
            return !denied.iter().any(|n| n == tool_name);
        }
        true
    }
}

/// Default startup timeout for MCP server connections (seconds).
pub const DEFAULT_STARTUP_TIMEOUT_SEC: u64 = 10;
/// Default per-tool-call timeout for MCP tools (seconds).
pub const DEFAULT_TOOL_TIMEOUT_SEC: u64 = 120;

fn default_startup_timeout_sec() -> u64 {
    DEFAULT_STARTUP_TIMEOUT_SEC
}

fn default_tool_timeout_sec() -> u64 {
    DEFAULT_TOOL_TIMEOUT_SEC
}

/// OAuth credentials stored alongside an MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthCredentials {
    /// Provider identifier (e.g. "linear", "github").
    pub provider: String,
    pub access_token: Secret<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<Secret<String>>,
    /// ISO-8601 expiry timestamp, if the token expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Environment variable name to inject into the MCP subprocess.
    pub env_var: String,
}

/// Transport configuration — either stdio subprocess or HTTP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "camelCase")]
pub enum McpTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// Authentication configuration for the MCP server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<String>>,
}

/// Server-side MCP settings (expose klyntbot as an MCP server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_port")]
    pub port: u16,
    #[serde(default = "default_localhost")]
    pub host: String,
    #[serde(default = "default_exposed_tools")]
    pub exposed_tools: Vec<String>,
    #[serde(default)]
    pub auth: McpAuthConfig,
}

impl Default for McpServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_mcp_port(),
            host: default_localhost(),
            exposed_tools: default_exposed_tools(),
            auth: McpAuthConfig::default(),
        }
    }
}

fn default_mcp_port() -> u16 {
    3100
}
fn default_localhost() -> String {
    "127.0.0.1".to_string()
}
fn default_exposed_tools() -> Vec<String> {
    [
        "project",
        "area",
        "notes",
        "memory",
        "okr",
        "productivity",
        "work_context",
        "agent",
        "annotate",
        "learning",
        "cron",
        "mirror",
        "temporal",
        "database",
    ]
    .map(String::from)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_defaults() {
        let cfg = McpConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.servers.is_empty());
        assert!(!cfg.server.enabled);
        assert_eq!(cfg.server.port, 3100);
        assert_eq!(cfg.server.host, "127.0.0.1");
    }

    #[test]
    fn test_mcp_config_serde_roundtrip() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "linear",
                    "transport": "stdio",
                    "command": "npx",
                    "args": ["-y", "@anthropic/linear-mcp-server"],
                    "env": {"LINEAR_API_KEY": "test-key"},
                    "enabled": true
                },
                {
                    "name": "notion",
                    "transport": "http",
                    "url": "https://mcp.notion.so/v1",
                    "headers": {"Authorization": "Bearer ntn_test"},
                    "enabled": true
                }
            ],
            "server": {
                "enabled": false,
                "port": 3100,
                "host": "127.0.0.1"
            }
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.servers.len(), 2);
        assert_eq!(cfg.servers[0].name, "linear");
        assert_eq!(cfg.servers[1].name, "notion");

        match &cfg.servers[0].transport {
            McpTransport::Stdio { command, args, env } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y", "@anthropic/linear-mcp-server"]);
                assert_eq!(env.get("LINEAR_API_KEY").unwrap(), "test-key");
            }
            _ => panic!("Expected Stdio transport"),
        }

        match &cfg.servers[1].transport {
            McpTransport::Http { url, headers } => {
                assert_eq!(url, "https://mcp.notion.so/v1");
                assert_eq!(headers.get("Authorization").unwrap(), "Bearer ntn_test");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_mcp_config_camel_case_keys() {
        let cfg = McpConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        assert!(json.get("enabled").is_some());
        assert!(json.get("servers").is_some());
        assert!(json.get("server").is_some());
    }

    #[test]
    fn test_mcp_config_disabled_server() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "disabled-server",
                    "transport": "stdio",
                    "command": "some-cmd",
                    "enabled": false
                }
            ]
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.servers[0].enabled);
    }

    #[test]
    fn test_mcp_oauth_credentials_roundtrip() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "linear",
                    "transport": "stdio",
                    "command": "npx",
                    "args": ["-y", "@anthropic/linear-mcp-server"],
                    "enabled": true,
                    "oauth": {
                        "provider": "linear",
                        "accessToken": "lin_api_test123",
                        "refreshToken": null,
                        "envVar": "LINEAR_API_KEY"
                    }
                }
            ]
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).unwrap();
        let oauth = cfg.servers[0].oauth.as_ref().unwrap();
        assert_eq!(oauth.provider, "linear");
        assert_eq!(oauth.access_token.expose(), "lin_api_test123");
        assert!(oauth.refresh_token.is_none());
        assert_eq!(oauth.env_var, "LINEAR_API_KEY");

        // Roundtrip: serialize back and check oauth is present
        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(serialized.contains("\"oauth\""));
        assert!(serialized.contains("\"accessToken\""));
    }

    #[test]
    fn test_mcp_server_without_oauth_defaults_to_none() {
        let json = r#"{
            "name": "test",
            "transport": "stdio",
            "command": "test-cmd",
            "enabled": true
        }"#;

        let server: McpServerDef = serde_json::from_str(json).unwrap();
        assert!(server.oauth.is_none());

        // Serializing should omit the oauth field entirely
        let serialized = serde_json::to_string(&server).unwrap();
        assert!(!serialized.contains("oauth"));
    }

    #[test]
    fn test_mcp_server_timeout_defaults() {
        let json = r#"{
            "name": "test",
            "transport": "stdio",
            "command": "test-cmd",
            "enabled": true
        }"#;

        let server: McpServerDef = serde_json::from_str(json).unwrap();
        assert_eq!(server.startup_timeout_sec, 10);
        assert_eq!(server.tool_timeout_sec, 120);
        assert!(server.enabled_tools.is_none());
        assert!(server.disabled_tools.is_none());
    }

    #[test]
    fn test_mcp_server_custom_timeouts_roundtrip() {
        let json = r#"{
            "name": "slow-server",
            "transport": "stdio",
            "command": "npx",
            "startupTimeoutSec": 30,
            "toolTimeoutSec": 300,
            "enabledTools": ["list_issues", "get_issue"],
            "enabled": true
        }"#;

        let server: McpServerDef = serde_json::from_str(json).unwrap();
        assert_eq!(server.startup_timeout_sec, 30);
        assert_eq!(server.tool_timeout_sec, 300);
        assert_eq!(
            server.enabled_tools.as_ref().unwrap(),
            &["list_issues", "get_issue"]
        );

        // Roundtrip
        let serialized = serde_json::to_string(&server).unwrap();
        let deserialized: McpServerDef = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.startup_timeout_sec, 30);
        assert_eq!(deserialized.tool_timeout_sec, 300);
    }

    #[test]
    fn test_is_tool_allowed_no_filters() {
        let server: McpServerDef = serde_json::from_str(
            r#"{
            "name": "test", "transport": "stdio", "command": "cmd"
        }"#,
        )
        .unwrap();
        assert!(server.is_tool_allowed("anything"));
    }

    #[test]
    fn test_is_tool_allowed_with_allowlist() {
        let server: McpServerDef = serde_json::from_str(
            r#"{
            "name": "test", "transport": "stdio", "command": "cmd",
            "enabledTools": ["list_issues", "get_issue"]
        }"#,
        )
        .unwrap();
        assert!(server.is_tool_allowed("list_issues"));
        assert!(server.is_tool_allowed("get_issue"));
        assert!(!server.is_tool_allowed("delete_issue"));
    }

    #[test]
    fn test_is_tool_allowed_with_denylist() {
        let server: McpServerDef = serde_json::from_str(
            r#"{
            "name": "test", "transport": "stdio", "command": "cmd",
            "disabledTools": ["dangerous_tool"]
        }"#,
        )
        .unwrap();
        assert!(server.is_tool_allowed("list_issues"));
        assert!(!server.is_tool_allowed("dangerous_tool"));
    }

    #[test]
    fn test_is_tool_allowed_allowlist_takes_precedence() {
        // When both are set, allowlist takes precedence
        let server: McpServerDef = serde_json::from_str(
            r#"{
            "name": "test", "transport": "stdio", "command": "cmd",
            "enabledTools": ["safe_tool"],
            "disabledTools": ["safe_tool"]
        }"#,
        )
        .unwrap();
        assert!(server.is_tool_allowed("safe_tool"));
        assert!(!server.is_tool_allowed("other_tool"));
    }
}
