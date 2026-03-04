//! Tauri commands for OAuth flows.

use desktop_shared::commands::{McpConfigResponse, OAuthStartParams};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{McpOAuthCompletePayload, MCP_OAUTH_COMPLETE, MCP_OAUTH_ERROR};
use rand::distr::Alphanumeric;
use rand::Rng;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, warn};

use crate::app_core::AppCore;
use crate::commands::build_mcp_response;

use super::flow;
use super::registry;

/// Start an OAuth flow: opens the browser and waits for the callback.
#[tauri::command]
pub async fn mcp_oauth_start(
    app: AppHandle,
    state: State<'_, AppCore>,
    params: OAuthStartParams,
) -> Result<(), ApiError> {
    info!(
        provider = %params.provider,
        server = %params.server_name,
        "mcp_oauth_start called"
    );

    let provider_def = registry::find_provider(&params.provider).ok_or_else(|| {
        ApiError::new(
            "NOT_FOUND",
            format!("Unknown OAuth provider: {}", params.provider),
        )
    })?;

    // Verify the server exists (read lock, released immediately)
    {
        let cfg = state.config.read().await;
        if !cfg.mcp.servers.iter().any(|s| s.name == params.server_name) {
            return Err(ApiError::new(
                "NOT_FOUND",
                format!("MCP server '{}' not found", params.server_name),
            ));
        }
    }

    // Start the callback server on the fixed port
    let rx = flow::start_callback_server().await.map_err(|e| {
        ApiError::new(
            "OAUTH_ERROR",
            format!("Failed to start callback server: {e}"),
        )
    })?;
    let redirect_uri = registry::REDIRECT_URI;

    // Generate CSRF state
    let oauth_state: String = rand::rng()
        .sample_iter(Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Resolve credentials (env var override -> bundled default)
    let client_id = provider_def.client_id();
    let client_secret = provider_def.client_secret();

    if client_id.is_empty() {
        return Err(ApiError::new(
            "OAUTH_ERROR",
            format!(
                "No client ID configured for '{}'. Set {} in your environment.",
                params.provider, provider_def.client_id_env
            ),
        ));
    }

    // Build authorize URL
    let mut authorize_url = url::Url::parse(provider_def.authorize_url)
        .map_err(|e| ApiError::new("OAUTH_ERROR", format!("Bad authorize URL: {e}")))?;
    authorize_url
        .query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("state", &oauth_state);
    if !provider_def.scopes.is_empty() {
        authorize_url
            .query_pairs_mut()
            .append_pair("scope", provider_def.scopes);
    }
    // Some providers (Google) require access_type=offline for refresh tokens
    if provider_def.requires_offline {
        authorize_url
            .query_pairs_mut()
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
    }

    // Open the browser
    open::that(authorize_url.as_str())
        .map_err(|e| ApiError::new("OAUTH_ERROR", format!("Failed to open browser: {e}")))?;

    info!(
        provider = %params.provider,
        server = %params.server_name,
        "OAuth flow started, waiting for callback"
    );

    // Capture values for the spawned task
    let server_name = params.server_name.clone();
    let provider_id = params.provider.clone();
    let expected_state = oauth_state;
    let env_var = provider_def.env_var.to_string();
    let supports_refresh = provider_def.supports_refresh;

    // Spawn background task — uses AppHandle to access managed state
    // `provider_def` is &'static so it can be safely captured.
    tokio::spawn(async move {
        // Wait for the callback (server has 5-minute timeout)
        let (code, callback_state) = match rx.await {
            Ok(pair) => pair,
            Err(_) => {
                warn!("OAuth callback channel closed (timeout or error)");
                let _ = app.emit(
                    MCP_OAUTH_ERROR,
                    serde_json::json!({
                        "serverName": server_name,
                        "error": "OAuth flow timed out"
                    }),
                );
                return;
            }
        };

        // Verify CSRF state
        if callback_state != expected_state {
            warn!("OAuth state mismatch");
            let _ = app.emit(
                MCP_OAUTH_ERROR,
                serde_json::json!({
                    "serverName": server_name,
                    "error": "OAuth state mismatch (possible CSRF)"
                }),
            );
            return;
        }

        // Exchange authorization code for tokens
        let token_result = flow::exchange_code(
            provider_def,
            &code,
            redirect_uri,
            &client_id,
            &client_secret,
        )
        .await;

        match token_result {
            Ok(tokens) => {
                // Access AppCore via the AppHandle
                let core = app.state::<AppCore>();
                let mut cfg = core.config.write().await;

                if let Some(server) = cfg.mcp.servers.iter_mut().find(|s| s.name == server_name) {
                    let expires_at = tokens.expires_in.map(|secs| {
                        let expiry = chrono::Utc::now() + chrono::Duration::seconds(secs as i64);
                        expiry.to_rfc3339()
                    });

                    server.oauth = Some(config::McpOAuthCredentials {
                        provider: provider_id.clone(),
                        access_token: config::Secret::new(tokens.access_token),
                        refresh_token: if supports_refresh {
                            tokens.refresh_token.map(config::Secret::new)
                        } else {
                            None
                        },
                        expires_at,
                        env_var,
                    });

                    if let Err(e) = config::save(&cfg).await {
                        warn!(error = %e, "Failed to save config after OAuth");
                        let _ = app.emit(
                            MCP_OAUTH_ERROR,
                            serde_json::json!({
                                "serverName": server_name,
                                "error": format!("Config save failed: {e}")
                            }),
                        );
                        return;
                    }
                }

                info!(provider = %provider_id, server = %server_name, "OAuth tokens stored");

                // Clone OAuth creds for Google Calendar file writing (outside lock)
                let gcal_oauth = if provider_id == "google-calendar" {
                    cfg.mcp.servers.iter()
                        .find(|s| s.name == server_name)
                        .and_then(|s| s.oauth.clone())
                } else {
                    None
                };

                // Drop the config lock before any file I/O or reconnection
                drop(cfg);

                // Google Calendar MCP: write credentials + tokens files
                // that the @cocal/google-calendar-mcp server reads at startup.
                if let Some(oauth_creds) = gcal_oauth {
                    if let Err(e) = write_google_calendar_files(
                        &client_id,
                        &client_secret,
                        &oauth_creds,
                    )
                    .await
                    {
                        warn!(error = %e, "Failed to write Google Calendar token files");
                    } else {
                        // Re-acquire lock briefly to set env var and save
                        let mut cfg = core.config.write().await;
                        if let Some(server) = cfg.mcp.servers.iter_mut().find(|s| s.name == server_name) {
                            let creds_path = google_calendar_credentials_path();
                            if let config::McpTransport::Stdio { ref mut env, .. } =
                                server.transport
                            {
                                env.insert(
                                    "GOOGLE_OAUTH_CREDENTIALS".to_string(),
                                    creds_path,
                                );
                            }
                            let _ = config::save(&cfg).await;
                        }
                        drop(cfg);
                        info!("Google Calendar MCP files written");
                    }
                }

                // Reconnect the MCP server so it picks up the new token
                {
                    let cfg = core.config.read().await;
                    if let Some(server_def) = cfg.mcp.servers.iter().find(|s| s.name == server_name) {
                        let def = server_def.clone();
                        drop(cfg);
                        core.agent.reconnect_mcp_server(&def).await;
                    }
                }

                let _ = app.emit(
                    MCP_OAUTH_COMPLETE,
                    McpOAuthCompletePayload {
                        server_name,
                        provider: provider_id,
                    },
                );
            }
            Err(e) => {
                warn!(error = %e, "OAuth token exchange failed");
                let _ = app.emit(
                    MCP_OAUTH_ERROR,
                    serde_json::json!({
                        "serverName": server_name,
                        "error": format!("Token exchange failed: {e}")
                    }),
                );
            }
        }
    });

    Ok(())
}

/// Disconnect OAuth for a server (clear credentials).
#[tauri::command]
pub async fn mcp_oauth_disconnect(
    state: State<'_, AppCore>,
    server_name: String,
) -> Result<McpConfigResponse, ApiError> {
    let mut cfg = state.config.write().await;

    let server = crate::commands::find_server_mut(&mut cfg.mcp.servers, &server_name)?;

    server.oauth = None;

    config::save(&cfg)
        .await
        .map_err(crate::commands::map_config_save_err)?;

    Ok(build_mcp_response(&cfg))
}

// ── Google Calendar MCP helpers ──────────────────────────────────────────

/// Directory where Google Calendar MCP stores its config/tokens.
fn google_calendar_config_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("google-calendar-mcp")
}

/// Path to the OAuth client credentials file.
fn google_calendar_credentials_path() -> String {
    google_calendar_config_dir()
        .join("gcp-oauth.keys.json")
        .to_string_lossy()
        .into_owned()
}

/// Write the `gcp-oauth.keys.json` and `tokens.json` files that
/// `@cocal/google-calendar-mcp` reads at startup.
async fn write_google_calendar_files(
    client_id: &str,
    client_secret: &str,
    oauth: &config::McpOAuthCredentials,
) -> anyhow::Result<()> {
    let dir = google_calendar_config_dir();
    tokio::fs::create_dir_all(&dir).await?;

    // 1. Write gcp-oauth.keys.json (client credentials)
    let credentials = serde_json::json!({
        "installed": {
            "client_id": client_id,
            "client_secret": client_secret,
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "redirect_uris": ["http://localhost"]
        }
    });
    let creds_path = dir.join("gcp-oauth.keys.json");
    tokio::fs::write(&creds_path, serde_json::to_string_pretty(&credentials)?).await?;

    // 2. Write tokens.json in the format the MCP server expects
    let expiry_date = oauth.expires_at.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.timestamp_millis())
    });

    let mut token_data = serde_json::json!({
        "access_token": oauth.access_token.expose(),
        "scope": "https://www.googleapis.com/auth/calendar",
        "token_type": "Bearer",
    });
    if let Some(ref rt) = oauth.refresh_token {
        token_data["refresh_token"] = serde_json::Value::String(rt.expose().clone());
    }
    if let Some(exp) = expiry_date {
        token_data["expiry_date"] = serde_json::Value::Number(exp.into());
    }

    // Wrap in "normal" account key (the format @cocal/google-calendar-mcp expects)
    let tokens = serde_json::json!({ "normal": token_data });

    let tokens_path = dir.join("tokens.json");
    tokio::fs::write(&tokens_path, serde_json::to_string_pretty(&tokens)?).await?;

    info!(
        creds = %creds_path.display(),
        tokens = %tokens_path.display(),
        "Wrote Google Calendar MCP files"
    );
    Ok(())
}
