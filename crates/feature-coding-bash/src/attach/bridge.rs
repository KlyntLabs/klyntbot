//! PtyAttachBridge — pumps bytes between a WebSocket and the supervisor's
//! `write_stdin`/`set_attach_channel`. Testable without Tauri via
//! `tokio::io::duplex()`.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::WebSocketStream;
use tools_core::{AttachError, JobId, JobSupervisorHandle};

#[derive(serde::Deserialize)]
#[serde(tag = "kind")]
pub enum ControlFrame {
    #[serde(rename = "resize")]
    Resize { rows: u16, cols: u16 },
}

pub struct PtyAttachBridge {
    job_id: JobId,
    supervisor: Arc<dyn JobSupervisorHandle>,
}

impl PtyAttachBridge {
    pub fn new(job_id: JobId, supervisor: Arc<dyn JobSupervisorHandle>) -> Self {
        Self { job_id, supervisor }
    }

    /// Bidirectional pump. Drives until the WebSocket closes or the job
    /// terminates. Calls `detach()` on the supervisor on exit (idempotent).
    pub async fn run<S>(self, ws: WebSocketStream<S>) -> Result<(), AttachError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut ws_tx, mut ws_rx) = ws.split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.supervisor
            .set_attach_channel(&self.job_id, out_tx)
            .await?;

        let outbound = async move {
            while let Some(bytes) = out_rx.recv().await {
                if ws_tx.send(WsMessage::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
        };

        let id = self.job_id.clone();
        let supervisor = self.supervisor.clone();
        let inbound = async move {
            while let Some(msg) = ws_rx.next().await {
                let msg = msg.map_err(|e| AttachError::Ws(e.to_string()))?;
                match msg {
                    WsMessage::Binary(bytes) => {
                        supervisor
                            .write_stdin(&id, &bytes)
                            .await
                            .map_err(|e| AttachError::Supervisor(e.to_string()))?;
                    }
                    WsMessage::Text(s) => {
                        if let Ok(ControlFrame::Resize { rows, cols }) =
                            serde_json::from_str::<ControlFrame>(&s)
                        {
                            supervisor
                                .resize(&id, rows, cols)
                                .await
                                .map_err(|e| AttachError::Supervisor(e.to_string()))?;
                        } else {
                            supervisor
                                .write_stdin(&id, s.as_bytes())
                                .await
                                .map_err(|e| AttachError::Supervisor(e.to_string()))?;
                        }
                    }
                    WsMessage::Close(_) => break,
                    _ => {}
                }
            }
            Ok::<(), AttachError>(())
        };

        tokio::select! {
            _ = outbound => {}
            r = inbound => { r?; }
        }
        self.supervisor.detach(&self.job_id).await?;
        Ok(())
    }
}
