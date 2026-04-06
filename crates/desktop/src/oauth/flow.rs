//! OAuth callback server and token exchange.
//!
//! Spins up a localhost HTTP server on a fixed port to receive the OAuth
//! callback, then exchanges the authorization code for an access token.

use std::collections::HashMap;
use std::future::IntoFuture;
use std::sync::Mutex;
use std::time::Duration;

use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tracing::warn;

use super::registry::{OAuthProviderDef, CALLBACK_PORT};

/// Abort handle for the currently running callback server.
/// Aborting the previous server drops the `TcpListener`, freeing the port.
static CALLBACK_SERVER_HANDLE: Mutex<Option<AbortHandle>> = Mutex::new(None);

/// Result of a successful token exchange.
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
}

/// Callback parameters received from the OAuth provider.
#[derive(serde::Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// Spin up the callback server on the fixed port and return the receiver.
///
/// The receiver yields `(code, state)` when the callback arrives.
/// The server auto-shuts down after the first request or a 5-minute timeout.
pub async fn start_callback_server() -> anyhow::Result<oneshot::Receiver<(String, String)>> {
    let (tx, rx) = oneshot::channel::<(String, String)>();
    let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

    let tx_clone = tx.clone();
    let app = Router::new().route(
        "/callback",
        get(move |Query(params): Query<CallbackParams>| {
            let tx = tx_clone.clone();
            async move {
                if let Some(error) = params.error {
                    warn!(error = %error, "OAuth callback returned error");
                    return Html(
                        "<html><body><h2>Authentication failed</h2>\
                         <p>You can close this window.</p></body></html>"
                            .to_string(),
                    );
                }

                if let (Some(code), Some(state)) = (params.code, params.state) {
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send((code, state));
                    }
                }

                Html(
                    "<html><body><h2>Authentication successful!</h2>\
                     <p>You can close this window and return to Klynt.</p></body></html>"
                        .to_string(),
                )
            }
        }),
    );

    // Abort any previous callback server to free the port
    {
        if let Some(prev) = CALLBACK_SERVER_HANDLE.lock().unwrap().take() {
            prev.abort();
        }
    }
    // Yield to let the runtime drop the old listener
    tokio::task::yield_now().await;

    let addr = format!("127.0.0.1:{CALLBACK_PORT}");
    let listener = TcpListener::bind(&addr).await?;

    // Spawn the server with a 5-minute timeout, storing the handle for cleanup
    let handle = tokio::spawn(async move {
        let server = axum::serve(listener, app);
        let _ = tokio::time::timeout(Duration::from_secs(300), server.into_future()).await;
    });
    *CALLBACK_SERVER_HANDLE.lock().unwrap() = Some(handle.abort_handle());

    Ok(rx)
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    provider: &OAuthProviderDef,
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<TokenResponse> {
    let mut params = HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("code", code);
    params.insert("redirect_uri", redirect_uri);
    params.insert("client_id", client_id);
    params.insert("client_secret", client_secret);

    let client = common::shared_http_client().clone();
    let resp: reqwest::Response = client
        .post(provider.token_url)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed ({status}): {body}");
    }

    let json: serde_json::Value = resp.json().await?;

    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing access_token in response"))?
        .to_string();

    let refresh_token = json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let expires_in = json.get("expires_in").and_then(|v| v.as_u64());

    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in,
    })
}
