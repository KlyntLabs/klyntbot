//! MCP settings handlers.

use std::collections::HashMap;

use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpServerResponse,
    McpToggleParams, McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;
use serde_json::Value;

use crate::errors::{map_config_save_err, map_serialization_err};
use crate::state::AppCore;

// ── Helper functions ──────────────────────────────────────────────────

/// Convert an `McpServerDef` into a response DTO.
pub fn server_to_response(s: &config::McpServerDef) -> McpServerResponse {
    let oauth_provider = s.oauth.as_ref().map(|o| o.provider.clone());
    let oauth_connected = s.oauth.as_ref().is_some_and(|o| !o.access_token.is_empty());

    match &s.transport {
        config::McpTransport::Stdio { command, args, env } => McpServerResponse {
            name: s.name.clone(),
            transport: "stdio".to_string(),
            enabled: s.enabled,
            command: Some(command.clone()),
            args: Some(args.clone()),
            env: Some(env.clone()),
            url: None,
            headers: None,
            oauth_provider,
            oauth_connected,
        },
        config::McpTransport::Http { url, headers } => McpServerResponse {
            name: s.name.clone(),
            transport: "http".to_string(),
            enabled: s.enabled,
            command: None,
            args: None,
            env: None,
            url: Some(url.clone()),
            headers: Some(headers.clone()),
            oauth_provider,
            oauth_connected,
        },
    }
}

/// Build the full MCP config response from config.
pub fn build_mcp_response(cfg: &config::Config) -> McpConfigResponse {
    McpConfigResponse {
        enabled: cfg.mcp.enabled,
        servers: cfg.mcp.servers.iter().map(server_to_response).collect(),
    }
}

/// Find an MCP server by name or return a NOT_FOUND error.
pub fn find_server_mut<'a>(
    servers: &'a mut [config::McpServerDef],
    name: &str,
) -> Result<&'a mut config::McpServerDef, ApiError> {
    servers
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("MCP server '{name}' not found")))
}

/// Build an `McpTransport` from user-provided params.
pub fn build_transport(
    transport_type: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> Result<config::McpTransport, ApiError> {
    match transport_type {
        "stdio" => {
            let command = command.ok_or_else(|| {
                ApiError::new("VALIDATION", "command is required for stdio transport")
            })?;
            Ok(config::McpTransport::Stdio {
                command,
                args: args.unwrap_or_default(),
                env: env.unwrap_or_default(),
            })
        }
        "http" => {
            let url = url
                .ok_or_else(|| ApiError::new("VALIDATION", "url is required for http transport"))?;
            Ok(config::McpTransport::Http {
                url,
                headers: headers.unwrap_or_default(),
            })
        }
        other => Err(ApiError::new(
            "VALIDATION",
            format!("unknown transport type: {other}"),
        )),
    }
}

/// Recursively merge `patch` into `base`. Objects merge keys;
/// arrays and scalars replace entirely; explicit `null` removes a key.
fn deep_merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if value.is_null() {
                    base_map.remove(&key);
                } else {
                    let entry = base_map.entry(key).or_insert(Value::Null);
                    deep_merge(entry, value);
                }
            }
        }
        (base, patch) => *base = patch,
    }
}

// ── AppCore methods ───────────────────────────────────────────────────

impl AppCore {
    pub async fn mcp_get_config(&self) -> Result<McpConfigResponse, ApiError> {
        let cfg = self.config.read().await;
        Ok(build_mcp_response(&cfg))
    }

    pub async fn mcp_add_server(
        &self,
        params: McpAddServerParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let server_def = {
            let mut cfg = self.config.write().await;

            if cfg.mcp.servers.iter().any(|s| s.name == params.name) {
                return Err(ApiError::new(
                    "CONFLICT",
                    format!("MCP server '{}' already exists", params.name),
                ));
            }

            let transport = build_transport(
                &params.transport,
                params.command,
                params.args,
                params.env,
                params.url,
                params.headers,
            )?;

            let def = config::McpServerDef {
                name: params.name,
                transport,
                enabled: true,
                oauth: None,
                startup_timeout_sec: config::DEFAULT_STARTUP_TIMEOUT_SEC,
                tool_timeout_sec: config::DEFAULT_TOOL_TIMEOUT_SEC,
                enabled_tools: None,
                disabled_tools: None,
            };

            cfg.mcp.servers.push(def.clone());
            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            // Drop config lock before calling agent
            drop(cfg);
            (def, response)
        };

        let (def, response) = server_def;
        // Connect the new server in the live agent (no OAuth requirement)
        self.agent.reconnect_mcp_server(&def).await;

        Ok(response)
    }

    pub async fn mcp_remove_server(
        &self,
        params: McpRemoveParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let response = {
            let mut cfg = self.config.write().await;

            let before = cfg.mcp.servers.len();
            cfg.mcp.servers.retain(|s| s.name != params.name);

            if cfg.mcp.servers.len() == before {
                return Err(ApiError::new(
                    "NOT_FOUND",
                    format!("MCP server '{}' not found", params.name),
                ));
            }

            config::save(&cfg).await.map_err(map_config_save_err)?;
            build_mcp_response(&cfg)
            // Drop config lock before calling agent
        };

        // Disconnect the removed server and unregister its tools
        self.agent.disconnect_mcp_server(&params.name).await;

        Ok(response)
    }

    pub async fn mcp_toggle_server(
        &self,
        params: McpToggleParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let server_def = {
            let mut cfg = self.config.write().await;

            let server = find_server_mut(&mut cfg.mcp.servers, &params.name)?;

            server.enabled = params.enabled;

            // Clone before releasing the mutable borrow on `server`
            let def = if params.enabled {
                Some(server.clone())
            } else {
                None
            };

            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            drop(cfg);
            (def, response)
        };

        let (def, response) = server_def;
        if let Some(def) = def {
            // Enabling: reconnect the server
            self.agent.reconnect_mcp_server(&def).await;
        } else {
            // Disabling: disconnect and unregister tools
            self.agent.disconnect_mcp_server(&params.name).await;
        }

        Ok(response)
    }

    pub async fn mcp_update_server(
        &self,
        params: McpUpdateServerParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let result = {
            let mut cfg = self.config.write().await;

            let server = find_server_mut(&mut cfg.mcp.servers, &params.name)?;

            if let Some(transport_type) = &params.transport {
                server.transport = build_transport(
                    transport_type,
                    params.command,
                    params.args,
                    params.env,
                    params.url,
                    params.headers,
                )?;
            } else {
                // Update fields within existing transport
                match &mut server.transport {
                    config::McpTransport::Stdio { command, args, env } => {
                        if let Some(c) = params.command {
                            *command = c;
                        }
                        if let Some(a) = params.args {
                            *args = a;
                        }
                        if let Some(e) = params.env {
                            *env = e;
                        }
                    }
                    config::McpTransport::Http { url, headers } => {
                        if let Some(u) = params.url {
                            *url = u;
                        }
                        if let Some(h) = params.headers {
                            *headers = h;
                        }
                    }
                }
            }

            // Clone the server_def before releasing the mutable borrow
            let def = if server.enabled {
                Some(server.clone())
            } else {
                None
            };

            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            drop(cfg);
            (def, response)
        };

        let (def, response) = result;
        // Disconnect old tools, then reconnect if enabled
        self.agent.disconnect_mcp_server(&params.name).await;
        if let Some(def) = def {
            self.agent.reconnect_mcp_server(&def).await;
        }

        Ok(response)
    }

    // ── Generic config commands ──────────────────────────────────────

    pub async fn app_info(&self) -> Result<AppInfoResponse, ApiError> {
        let cfg = self.config.read().await;
        Ok(AppInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: cfg.data_dir_path().to_string_lossy().to_string(),
            setup_completed: cfg.setup_completed,
        })
    }

    pub async fn config_get_section(&self, section: String) -> Result<Value, ApiError> {
        let cfg = self.config.read().await;
        let full = serde_json::to_value(&*cfg).map_err(map_serialization_err)?;
        match full.get(&section) {
            Some(val) => Ok(val.clone()),
            None => Err(ApiError::new(
                "NOT_FOUND",
                format!("config section '{section}' not found"),
            )),
        }
    }

    pub async fn config_update_section(
        &self,
        section: String,
        patch: Value,
    ) -> Result<Value, ApiError> {
        let mut cfg = self.config.write().await;

        let mut full = serde_json::to_value(&*cfg).map_err(map_serialization_err)?;

        {
            let section_val = full.get_mut(&section).ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("config section '{section}' not found"))
            })?;
            deep_merge(section_val, patch);
        }

        // Extract the merged section before from_value consumes `full`
        let section_result = full.get(&section).cloned().unwrap_or(Value::Null);

        let updated: config::Config = serde_json::from_value(full)
            .map_err(|e| ApiError::new("VALIDATION", format!("invalid config: {e}")))?;

        config::save(&updated).await.map_err(map_config_save_err)?;

        *cfg = updated;

        Ok(section_result)
    }

    pub async fn config_mark_setup_completed(&self) -> Result<(), ApiError> {
        let mut cfg = self.config.write().await;
        cfg.setup_completed = true;
        config::save(&cfg).await.map_err(map_config_save_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deep_merge_objects_recursive() {
        let mut base = json!({"a": {"b": 1, "c": 2}, "d": 3});
        let patch = json!({"a": {"b": 10, "e": 5}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"b": 10, "c": 2, "e": 5}, "d": 3}));
    }

    #[test]
    fn test_deep_merge_scalars_overwrite() {
        let mut base = json!({"x": 1});
        let patch = json!({"x": 99});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"x": 99}));
    }

    #[test]
    fn test_deep_merge_arrays_replace() {
        let mut base = json!({"tags": [1, 2, 3]});
        let patch = json!({"tags": [4, 5]});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"tags": [4, 5]}));
    }

    #[test]
    fn test_deep_merge_null_removes_key() {
        let mut base = json!({"a": 1, "b": 2});
        let patch = json!({"a": null});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"b": 2}));
    }

    #[test]
    fn test_deep_merge_adds_new_keys() {
        let mut base = json!({"a": 1});
        let patch = json!({"b": 2, "c": {"d": 3}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 1, "b": 2, "c": {"d": 3}}));
    }

    #[test]
    fn test_deep_merge_nested_null_removes() {
        let mut base = json!({"a": {"b": 1, "c": 2}});
        let patch = json!({"a": {"b": null}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"c": 2}}));
    }

    #[test]
    fn test_deep_merge_empty_patch() {
        let mut base = json!({"a": 1});
        let patch = json!({});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 1}));
    }

    #[test]
    fn test_deep_merge_replaces_scalar_with_object() {
        let mut base = json!({"a": 1});
        let patch = json!({"a": {"nested": true}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"nested": true}}));
    }

    #[test]
    fn test_deep_merge_replaces_object_with_scalar() {
        let mut base = json!({"a": {"nested": true}});
        let patch = json!({"a": 42});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 42}));
    }
}
