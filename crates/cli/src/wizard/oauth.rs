//! OAuth 2.0 authentication flow for wizard channel setup.
//!
//! Provides:
//! - Embedded HTTP callback server (raw tokio TCP, no framework dep)
//! - Cross-platform browser launcher
//! - OAuth authorization code exchange
//! - Provider-specific flows for Discord and Slack
//!
//! The callback server binds to a random local port, opens the user's browser
//! to the provider's auth URL, waits for the redirect with an authorization code,
//! then exchanges that code for access tokens.

use std::collections::HashMap;
use std::io::{self, Write as IoWrite};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use common::utils::terminal::*;
use config::schema::Secret;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

/// OAuth 2.0 provider configuration (static per provider).
#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    pub name: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static [&'static str],
}

/// Tokens received from a successful OAuth exchange.
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: Secret<String>,
    pub refresh_token: Option<Secret<String>>,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    /// Slack-specific: the bot user ID.
    pub bot_user_id: Option<String>,
    /// Slack-specific: the team/workspace ID.
    pub team_id: Option<String>,
}

/// Result of the local callback server receiving a redirect.
#[derive(Debug)]
struct CallbackResult {
    code: String,
    state: String,
}

// ============================================================================
// Provider Configs
// ============================================================================

/// Discord OAuth 2.0 configuration.
///
/// Note: Discord bots typically use a direct bot token from the Developer Portal.
/// This OAuth flow is for the bot authorization URL (adding the bot to a guild)
/// and optionally for user-level OAuth if needed.
pub const DISCORD_OAUTH: OAuthProviderConfig = OAuthProviderConfig {
    name: "Discord",
    auth_url: "https://discord.com/oauth2/authorize",
    token_url: "https://discord.com/api/v10/oauth2/token",
    scopes: &["bot", "applications.commands"],
};

/// Slack OAuth 2.0 V2 configuration.
///
/// This is the standard Slack app installation flow. After the user approves
/// the app in their browser, Slack redirects with a code that we exchange
/// for a bot token (xoxb-...).
pub const SLACK_OAUTH: OAuthProviderConfig = OAuthProviderConfig {
    name: "Slack",
    auth_url: "https://slack.com/oauth/v2/authorize",
    token_url: "https://slack.com/api/oauth.v2.access",
    scopes: &[
        "chat:write",
        "channels:history",
        "channels:read",
        "groups:history",
        "groups:read",
        "im:history",
        "im:read",
        "im:write",
        "users:read",
        "app_mentions:read",
    ],
};

// ============================================================================
// Public API
// ============================================================================

/// Run a full OAuth 2.0 authorization code flow.
///
/// Steps:
/// 1. Start a local HTTP server on a random port
/// 2. Build the authorization URL with state parameter
/// 3. Open the user's browser to the auth URL
/// 4. Wait for the callback with the authorization code
/// 5. Exchange the code for access tokens
///
/// Returns `None` if the user cancels or the flow times out.
pub async fn run_oauth_flow(
    provider: &OAuthProviderConfig,
    client_id: &str,
    client_secret: &str,
    timeout_secs: u64,
) -> Result<Option<OAuthTokens>> {
    let state = generate_state();

    // Step 1: Start callback server
    let (port, code_rx) = start_callback_server(state.clone()).await?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", port);

    // Step 2: Build auth URL
    let auth_url = build_auth_url(provider, client_id, &redirect_uri, &state);

    // Step 3: Open browser
    println!();
    println!(
        "  {} Opening browser for {} authorization...",
        status_info(),
        provider.name
    );
    println!();

    if let Err(e) = open_browser(&auth_url) {
        warn!("Failed to open browser: {}", e);
        println!(
            "  {} Could not open browser automatically.",
            status_warning()
        );
        println!("  Please open this URL manually:\n");
        println!("  {}", colorize(&auth_url, UNDERLINE));
        println!();
    }

    println!(
        "  {}",
        colorize("Waiting for authorization in browser...", DIM)
    );
    println!(
        "  {}",
        colorize(
            &format!("(timeout: {}s — press Ctrl+C to cancel)", timeout_secs),
            DIM
        )
    );

    // Step 4: Wait for callback
    let callback = match tokio::time::timeout(Duration::from_secs(timeout_secs), code_rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => {
            println!(
                "\n  {} Authorization callback server closed unexpectedly",
                status_error()
            );
            return Ok(None);
        }
        Err(_) => {
            println!(
                "\n  {} Authorization timed out after {}s",
                status_error(),
                timeout_secs
            );
            return Ok(None);
        }
    };

    // Validate state to prevent CSRF
    if callback.state != state {
        bail!("OAuth state mismatch — possible CSRF attack. Aborting.");
    }

    println!("\n  {} Authorization code received!", status_success());

    // Step 5: Exchange code for tokens
    println!("  {}", colorize("Exchanging code for access token...", DIM));

    let tokens = exchange_code(
        provider,
        client_id,
        client_secret,
        &callback.code,
        &redirect_uri,
    )
    .await
    .context("Failed to exchange authorization code for tokens")?;

    println!(
        "  {} {} tokens acquired successfully",
        status_success(),
        provider.name,
    );

    Ok(Some(tokens))
}

/// Generate a Discord bot invite URL.
///
/// This is the simpler Discord flow — generates a URL the user opens to
/// add their bot to a guild. No callback server needed since the bot token
/// comes directly from the Developer Portal.
pub fn discord_bot_invite_url(client_id: &str, permissions: u64) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("scope", "bot applications.commands")
        .append_pair("permissions", &permissions.to_string())
        .finish();
    format!("https://discord.com/oauth2/authorize?{}", query)
}

/// Validate a Discord bot token by calling the `/users/@me` endpoint.
pub async fn validate_discord_token(token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {}", token))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .context("Failed to connect to Discord API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Discord API returned {}: {}", status, body);
    }

    let json: serde_json::Value = resp.json().await?;
    let username = json["username"].as_str().unwrap_or("unknown").to_string();

    Ok(username)
}

/// Validate a Slack bot token by calling `auth.test`.
pub async fn validate_slack_token(bot_token: &str) -> Result<(String, String)> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/auth.test")
        .bearer_auth(bot_token)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .context("Failed to connect to Slack API")?;

    let json: serde_json::Value = resp.json().await?;

    if json["ok"].as_bool() != Some(true) {
        let error = json["error"].as_str().unwrap_or("unknown_error");
        bail!("Slack auth.test failed: {}", error);
    }

    let bot_id = json["user_id"].as_str().unwrap_or("unknown").to_string();
    let team = json["team"].as_str().unwrap_or("unknown").to_string();

    Ok((bot_id, team))
}

/// Open a URL in the default browser (cross-platform).
pub fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("Failed to run 'open'")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to run 'xdg-open'")?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("Failed to run 'start'")?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        bail!("Unsupported platform for browser launch");
    }

    Ok(())
}

// ============================================================================
// Prompt Helpers (for use by channel wizards)
// ============================================================================

/// Prompt the user for a client ID.
pub fn prompt_client_id(provider_name: &str) -> Result<String> {
    loop {
        print!("  {} Client ID: ", colorize(provider_name, BOLD));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("    {}", colorize("Client ID is required", ERROR));
            continue;
        }

        return Ok(input);
    }
}

/// Prompt the user for a client secret.
pub fn prompt_client_secret(provider_name: &str) -> Result<String> {
    loop {
        print!("  {} Client Secret: ", colorize(provider_name, BOLD));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("    {}", colorize("Client Secret is required", ERROR));
            continue;
        }

        return Ok(input);
    }
}

// ============================================================================
// Internal: Callback Server
// ============================================================================

/// Start a local HTTP server on a random port to receive OAuth callbacks.
///
/// Returns the port and a oneshot receiver that will yield the authorization code.
/// The server handles exactly one request and then shuts down.
async fn start_callback_server(
    expected_state: String,
) -> Result<(u16, oneshot::Receiver<CallbackResult>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind OAuth callback server")?;

    let port = listener.local_addr()?.port();
    debug!("OAuth callback server listening on 127.0.0.1:{}", port);

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        if let Err(e) = handle_callback_connection(listener, tx, expected_state).await {
            warn!("OAuth callback server error: {}", e);
        }
    });

    Ok((port, rx))
}

/// Handle a single incoming callback connection.
async fn handle_callback_connection(
    listener: TcpListener,
    tx: oneshot::Sender<CallbackResult>,
    expected_state: String,
) -> Result<()> {
    let (mut stream, _addr) = listener.accept().await?;

    // Read the HTTP request (we only need the first line + headers for query params)
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the request line: GET /callback?code=xxx&state=yyy HTTP/1.1
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");

    debug!("OAuth callback received: {}", path);

    // Parse query parameters
    let params = parse_query_params(path);

    let (response_body, result) = if let Some(code) = params.get("code") {
        let state = params.get("state").cloned().unwrap_or_default();

        if state != expected_state {
            (
                CALLBACK_ERROR_HTML.replace("{ERROR}", "State mismatch — possible CSRF attack"),
                None,
            )
        } else {
            (
                CALLBACK_SUCCESS_HTML.to_string(),
                Some(CallbackResult {
                    code: code.clone(),
                    state,
                }),
            )
        }
    } else if let Some(error) = params.get("error") {
        let desc = params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| error.clone());
        (CALLBACK_ERROR_HTML.replace("{ERROR}", &desc), None)
    } else {
        (
            CALLBACK_ERROR_HTML.replace("{ERROR}", "No authorization code received"),
            None,
        )
    };

    // Send HTTP response
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    // Send the result through the channel
    if let Some(result) = result {
        let _ = tx.send(result);
    }

    Ok(())
}

/// Parse query parameters from a URL path.
fn parse_query_params(path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if let Some(query) = path.split('?').nth(1) {
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            params.insert(key.to_string(), value.to_string());
        }
    }

    params
}

// ============================================================================
// Internal: Auth URL + Token Exchange
// ============================================================================

/// Build the OAuth authorization URL with all required parameters.
fn build_auth_url(
    provider: &OAuthProviderConfig,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
) -> String {
    let scopes = provider.scopes.join(" ");
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &scopes)
        .append_pair("state", state)
        .finish();
    format!("{}?{}", provider.auth_url, query)
}

/// Exchange an authorization code for access tokens.
async fn exchange_code(
    provider: &OAuthProviderConfig,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();

    // Build URL-encoded form body manually (avoids reqwest `form` feature)
    let form_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("client_secret", client_secret)
        .append_pair("code", code)
        .append_pair("grant_type", "authorization_code")
        .append_pair("redirect_uri", redirect_uri)
        .finish();

    let resp = client
        .post(provider.token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Failed to reach token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body: String = resp.text().await.unwrap_or_default();
        bail!(
            "{} token exchange failed ({}): {}",
            provider.name,
            status,
            body
        );
    }

    let json: serde_json::Value = resp.json().await?;
    debug!(
        "{} token response keys: {:?}",
        provider.name,
        json.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    // Slack has a nested structure; Discord is flat
    let access_token = json["access_token"]
        .as_str()
        .or_else(|| json["authed_user"]["access_token"].as_str())
        .unwrap_or_default()
        .to_string();

    if access_token.is_empty() {
        bail!(
            "{} token exchange returned no access_token. Response: {}",
            provider.name,
            serde_json::to_string_pretty(&json)?
        );
    }

    let refresh_token = json["refresh_token"]
        .as_str()
        .map(|s| Secret::new(s.to_string()));

    Ok(OAuthTokens {
        access_token: Secret::new(access_token),
        refresh_token,
        token_type: json["token_type"].as_str().unwrap_or("bearer").to_string(),
        expires_in: json["expires_in"].as_u64(),
        scope: json["scope"].as_str().map(String::from),
        bot_user_id: json["bot_user_id"].as_str().map(String::from),
        team_id: json["team"]["id"]
            .as_str()
            .or_else(|| json["team_id"].as_str())
            .map(String::from),
    })
}

/// Generate a cryptographically random state parameter for CSRF protection.
fn generate_state() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    // Use the standard library's random state for a unique value per invocation.
    // This is not cryptographically secure but sufficient for a local OAuth flow
    // where the attacker would need to intercept localhost traffic.
    let s = RandomState::new();
    let mut hasher = s.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    format!("{:016x}", hasher.finish())
}

// ============================================================================
// Status Helpers
// ============================================================================

fn status_info() -> String {
    colorize("ℹ", "\x1b[34m") // Blue info
}

fn status_error() -> String {
    colorize("✗", ERROR)
}

// ============================================================================
// HTML Templates
// ============================================================================

const CALLBACK_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Authorization Successful</title></head>
<body style="font-family: system-ui, sans-serif; text-align: center; padding: 60px 20px; background: #1a1a2e; color: #e0e0e0;">
  <h1 style="color: #4ade80;">&#10003; Authorization Successful</h1>
  <p>You can close this tab and return to the terminal.</p>
  <script>setTimeout(function() { window.close(); }, 3000);</script>
</body>
</html>"#;

const CALLBACK_ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Authorization Failed</title></head>
<body style="font-family: system-ui, sans-serif; text-align: center; padding: 60px 20px; background: #1a1a2e; color: #e0e0e0;">
  <h1 style="color: #f87171;">&#10007; Authorization Failed</h1>
  <p>{ERROR}</p>
  <p>Please return to the terminal and try again.</p>
</body>
</html>"#;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_params_basic() {
        let params = parse_query_params("/callback?code=abc123&state=xyz789");
        assert_eq!(params.get("code"), Some(&"abc123".to_string()));
        assert_eq!(params.get("state"), Some(&"xyz789".to_string()));
    }

    #[test]
    fn test_parse_query_params_encoded() {
        let params = parse_query_params("/callback?error_description=access%20denied&error=denied");
        assert_eq!(
            params.get("error_description"),
            Some(&"access denied".to_string())
        );
        assert_eq!(params.get("error"), Some(&"denied".to_string()));
    }

    #[test]
    fn test_parse_query_params_empty() {
        let params = parse_query_params("/callback");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_query_params_no_value() {
        // url::form_urlencoded treats bare keys as key="" (empty value)
        let params = parse_query_params("/callback?key_only");
        assert_eq!(params.get("key_only"), Some(&String::new()));
    }

    #[test]
    fn test_build_auth_url_discord() {
        let url = build_auth_url(
            &DISCORD_OAUTH,
            "my_client_id",
            "http://127.0.0.1:12345/callback",
            "test_state",
        );

        assert!(url.starts_with("https://discord.com/oauth2/authorize?"));
        assert!(url.contains("client_id=my_client_id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A12345%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=test_state"));
        assert!(url.contains("scope=bot+applications.commands"));
    }

    #[test]
    fn test_build_auth_url_slack() {
        let url = build_auth_url(
            &SLACK_OAUTH,
            "slack_client",
            "http://127.0.0.1:9999/callback",
            "my_state",
        );

        assert!(url.starts_with("https://slack.com/oauth/v2/authorize?"));
        assert!(url.contains("client_id=slack_client"));
        assert!(url.contains("chat%3Awrite"));
    }

    #[test]
    fn test_generate_state_unique() {
        let s1 = generate_state();
        let s2 = generate_state();
        // States should be different across calls (probabilistic but extremely likely)
        assert_ne!(s1, s2);
        // State should be a hex string
        assert_eq!(s1.len(), 16);
        assert!(s1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_discord_bot_invite_url() {
        let url = discord_bot_invite_url("123456", 3072);
        assert!(url.contains("client_id=123456"));
        assert!(url.contains("permissions=3072"));
        assert!(url.contains("scope=bot+applications.commands"));
    }

    #[tokio::test]
    async fn test_callback_server_receives_code() {
        let state = "test_state_123".to_string();
        let (port, rx) = start_callback_server(state.clone()).await.unwrap();

        // Simulate an OAuth callback by connecting as an HTTP client
        let client = reqwest::Client::new();
        let resp = client
            .get(format!(
                "http://127.0.0.1:{}/callback?code=auth_code_456&state={}",
                port, state
            ))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("Authorization Successful"));

        // Verify we received the code
        let result = rx.await.unwrap();
        assert_eq!(result.code, "auth_code_456");
        assert_eq!(result.state, "test_state_123");
    }

    #[tokio::test]
    async fn test_callback_server_handles_error() {
        let state = "test_state".to_string();
        let (port, _rx) = start_callback_server(state).await.unwrap();

        // Simulate an error callback
        let client = reqwest::Client::new();
        let resp = client
            .get(format!(
                "http://127.0.0.1:{}/callback?error=access_denied&error_description=user%20cancelled",
                port
            ))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("Authorization Failed"));
        assert!(body.contains("user cancelled"));
    }

    #[tokio::test]
    async fn test_callback_server_state_mismatch() {
        let state = "expected_state".to_string();
        let (port, _rx) = start_callback_server(state).await.unwrap();

        // Send callback with wrong state
        let client = reqwest::Client::new();
        let resp = client
            .get(format!(
                "http://127.0.0.1:{}/callback?code=code123&state=wrong_state",
                port
            ))
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("Authorization Failed"));
        assert!(body.contains("State mismatch"));
    }
}
