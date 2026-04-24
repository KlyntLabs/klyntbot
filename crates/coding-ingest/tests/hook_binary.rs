use assert_cmd::Command;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;

#[tokio::test]
async fn hook_forwards_session_start_to_socket() {
    let home = TempDir::new().unwrap();
    let sock = home.path().join("ingest.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    let reader = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut len = [0u8; 4]; s.read_exact(&mut len).await.unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        s.read_exact(&mut body).await.unwrap();
        body
    });

    let stdin_body = br#"{"session_id":"abc","cwd":"/tmp","source":"cli","model":"m"}"#;
    Command::cargo_bin("klyntbot-hook").unwrap()
        .env("KLYNTBOT_HOME", home.path())
        .args(["claude-code", "SessionStart"])
        .write_stdin(&stdin_body[..])
        .assert()
        .success();

    let body = tokio::time::timeout(std::time::Duration::from_secs(2), reader)
        .await.unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["v"], "v1");
}

#[test]
fn status_subcommand_exits_zero_even_with_no_desktop() {
    let home = TempDir::new().unwrap();
    Command::cargo_bin("klyntbot-hook").unwrap()
        .env("KLYNTBOT_HOME", home.path())
        .arg("status")
        .assert()
        .success();
}
