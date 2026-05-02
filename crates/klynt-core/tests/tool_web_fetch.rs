use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::web_fetch::{run_for_test as fetch_run, WebFetchArgs};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn fetches_text_from_local_server() {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let server = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .unwrap();
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = tokio::io::AsyncReadExt::read(&mut s, &mut buf).await;
        let body = "<html><body><h1>Title</h1><p>Hello world</p></body></html>";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}",
            body.len(),
            body
        );
        tokio::io::AsyncWriteExt::write_all(&mut s, resp.as_bytes())
            .await
            .unwrap();
    });

    let perms = CodingPermissions {
        allow: vec!["WebFetch(*)".into()],
        ..Default::default()
    };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let url = format!("http://127.0.0.1:{port}/");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let out = fetch_run(
        WebFetchArgs {
            url,
            format: Some("text".into()),
            max_bytes: Some(8192),
        },
        l1,
        pol,
        pri,
        pen,
        Some(tx),
        bus,
        CancellationToken::new(),
        client,
        Channel::Coding,
        NonUiPolicy::Allow,
        Arc::new(klynt_core::approval::HostApprovalCache::default()),
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
    )
    .await
    .unwrap();
    server.await.ok();
    assert!(out.contains("Hello world"));
}
