//! MCP (Model Context Protocol) configuration.
//!
//! Defines config types for MCP client connections and server settings.
//! The `mcp` crate imports these types from `config` (not the other way around)
//! to avoid circular dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

impl Default for McpServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_mcp_port(),
            host: default_localhost(),
        }
    }
}

fn default_mcp_port() -> u16 {
    3100
}
fn default_localhost() -> String {
    "127.0.0.1".to_string()
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
}
