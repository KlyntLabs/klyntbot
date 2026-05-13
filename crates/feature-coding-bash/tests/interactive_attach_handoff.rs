//! Use tokio::io::duplex() + tokio_tungstenite::WebSocketStream::from_raw_socket
//! to drive PtyAttachBridge without a real WebSocket. Verifies bytes round-trip.

use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::attach::PtyAttachBridge;
use feature_coding_bash::JobSupervisor;
use futures_util::SinkExt;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn bridge_round_trips_binary_frames() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(&feature_coding_bash::migrations::coding_background_jobs_migration().sql)
        .execute(pool.inner())
        .await
        .unwrap();
    let sup = Arc::new(JobSupervisor::new(
        BashJobRepo::new(pool.inner().clone()),
        Arc::new(DomainEventBus::new(256)),
        Arc::new(bus::context_updates::ContextUpdateQueue::new()),
        tempfile::tempdir().unwrap().into_path(),
        Arc::new(MacOsSeatbeltRunner::new()),
    ));
    let view = sup
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            agent_chain: vec!["a1".into()],
            description: "bridge probe".into(),
            command: "read x; echo bridge_got=$x".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 10_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    sup.attach(&view.id).await.expect("attach");

    let (client, server) = tokio::io::duplex(8192);
    let server_ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
        server,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;
    let bridge = PtyAttachBridge::new(view.id.clone(), sup.clone() as Arc<dyn JobSupervisorHandle>);
    tokio::spawn(async move {
        let _ = bridge.run(server_ws).await;
    });
    let mut client_ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
        client,
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    client_ws
        .send(WsMessage::Binary(b"hello\n".to_vec().into()))
        .await
        .expect("send");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let rd = sup.output_delta(&view.id, 0, false, 0).await.unwrap();
    let s = String::from_utf8_lossy(&rd.bytes);
    assert!(
        s.contains("bridge_got=hello"),
        "expected bridge_got=hello, got: {s:?}"
    );
}
