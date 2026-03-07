pub mod areas;
pub mod chat;
pub mod cognitive;
pub mod distraction;
pub mod finance;
pub mod key_results;
pub mod notes;
pub mod objectives;
pub mod permissions;
pub mod productivity;
pub mod projects;
pub mod settings;
pub mod status;
pub mod tasks;
pub mod window;

use chrono::{DateTime, Utc};
use desktop_shared::commands::{McpConfigResponse, McpServerResponse};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{EntityUpdatedPayload, ENTITY_UPDATED};
use desktop_shared::types::EntityKind;
use tauri::Emitter;

/// Convert a `KlyntbotError` into a productivity-flavored `ApiError`.
pub(crate) fn map_prod_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("PRODUCTIVITY_ERROR", e.to_string())
}

/// Convert a cognitive/sqlx error into an `ApiError`.
pub(crate) fn map_cognitive_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("STORAGE_ERROR", e.to_string())
}

/// Convert a `StorageError` into an `ApiError`, preserving specific error codes
/// for NotFound and Conflict variants.
pub(crate) fn map_storage_err(e: storage::StorageError) -> ApiError {
    match e {
        storage::StorageError::NotFound(msg) => ApiError::new("NOT_FOUND", msg),
        storage::StorageError::Conflict(msg) => ApiError::new("CONFLICT", msg),
        other => ApiError::new("STORAGE_ERROR", other.to_string()),
    }
}

/// Parse a "YYYY-MM-DD" string into a midnight UTC DateTime.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

/// Parse a "YYYY-MM-DD" string or return a validation `ApiError`.
pub(crate) fn parse_date_or_err(s: &str) -> Result<DateTime<Utc>, ApiError> {
    parse_date(s).ok_or_else(|| ApiError::new("VALIDATION", format!("invalid date: {s}")))
}

/// Convert a config save error into an `ApiError`.
pub(crate) fn map_config_save_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("CONFIG_SAVE", e.to_string())
}

/// Convert an `McpServerDef` into a response DTO.
pub(crate) fn server_to_response(s: &config::McpServerDef) -> McpServerResponse {
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
pub(crate) fn build_mcp_response(cfg: &config::Config) -> McpConfigResponse {
    McpConfigResponse {
        enabled: cfg.mcp.enabled,
        servers: cfg.mcp.servers.iter().map(server_to_response).collect(),
    }
}

/// Find an MCP server by name or return a NOT_FOUND error.
pub(crate) fn find_server_mut<'a>(
    servers: &'a mut [config::McpServerDef],
    name: &str,
) -> Result<&'a mut config::McpServerDef, ApiError> {
    servers
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("MCP server '{name}' not found")))
}

/// Build an `McpTransport` from user-provided params.
pub(crate) fn build_transport(
    transport_type: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
    url: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
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

/// Emit an entity-updated event so the frontend can refetch affected data.
pub fn emit_entity_updated(app: &tauri::AppHandle, kind: EntityKind, id: &str) {
    let payload = EntityUpdatedPayload {
        entity_kind: kind,
        id: id.to_string(),
    };
    if let Err(e) = app.emit(ENTITY_UPDATED, &payload) {
        tracing::warn!("failed to emit entity:updated event: {e}");
    }
}
