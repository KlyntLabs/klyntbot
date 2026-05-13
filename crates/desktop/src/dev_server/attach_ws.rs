//! WebSocket attach endpoint for PTY background jobs.
//!
//! Route: GET /api/coding/jobs/{job_id}/attach?token={token}

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tools_core::JobSupervisorHandle;
use tracing::{info, warn};

use crate::dev_server::DevState;

#[derive(Deserialize)]
pub struct AttachQuery {
    token: String,
}

pub async fn attach_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    Path(job_id): Path<String>,
    Query(query): Query<AttachQuery>,
    State(state): State<DevState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_attach(socket, job_id, query.token, state))
}

async fn handle_attach(
    socket: axum::extract::ws::WebSocket,
    job_id: String,
    token: String,
    state: DevState,
) {
    if let Err(e) = run_attach(socket, &job_id, &token, &state).await {
        warn!("attach ws error for {job_id}: {e}");
    }
}

async fn run_attach(
    socket: axum::extract::ws::WebSocket,
    job_id: &str,
    token: &str,
    state: &DevState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate token against storage.
    let repo = storage::repos::BashJobRepo::new(state.core.storage_pool.inner().clone());
    let row = repo
        .find_by_attach_token(token)
        .await
        .map_err(|e| format!("storage error: {e}"))?
        .ok_or("invalid attach token")?;

    if row.id != job_id {
        return Err("token does not match job_id".into());
    }

    let supervisor = state
        .core
        .job_supervisor()
        .map_err(|e| format!("supervisor unavailable: {e}"))?;

    let job_id = tools_core::JobId::from_str(job_id)
        .map_err(|e| format!("invalid job_id: {e}"))?;

    info!("pty attach started for job {job_id:?}");

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    supervisor
        .set_attach_channel(&job_id, out_tx)
        .await
        .map_err(|e| format!("set_attach_channel failed: {e}"))?;

    let outbound = async move {
        while let Some(bytes) = out_rx.recv().await {
            if ws_tx
                .send(axum::extract::ws::Message::Binary(bytes.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    };

    let supervisor2 = supervisor.clone();
    let job_id2 = job_id.clone();
    let inbound = async move {
        while let Some(msg) = ws_rx.next().await {
            let msg = msg.map_err(|e| format!("ws recv error: {e}"))?;
            match msg {
                axum::extract::ws::Message::Binary(bytes) => {
                    supervisor2
                        .write_stdin(&job_id2, &bytes)
                        .await
                        .map_err(|e| format!("write_stdin failed: {e}"))?;
                }
                axum::extract::ws::Message::Text(s) => {
                    if let Ok(frame) = serde_json::from_str::<ControlFrame>(&s) {
                        match frame {
                            ControlFrame::Resize { rows, cols } => {
                                supervisor2
                                    .resize(&job_id2, rows, cols)
                                    .await
                                    .map_err(|e| format!("resize failed: {e}"))?;
                            }
                        }
                    } else {
                        supervisor2
                            .write_stdin(&job_id2, s.as_bytes())
                            .await
                            .map_err(|e| format!("write_stdin failed: {e}"))?;
                    }
                }
                axum::extract::ws::Message::Close(_) => break,
                _ => {}
            }
        }
        Ok::<(), String>(())
    };

    tokio::select! {
        _ = outbound => {}
        r = inbound => { r?; }
    }

    supervisor.detach(&job_id).await.map_err(|e| format!("detach failed: {e}"))?;
    info!("pty attach ended for job {job_id:?}");
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind")]
enum ControlFrame {
    #[serde(rename = "resize")]
    Resize { rows: u16, cols: u16 },
}
