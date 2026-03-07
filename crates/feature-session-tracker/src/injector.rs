use chrono::Utc;
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Inject a message into a Claude Code session by writing a queue-operation enqueue.
pub async fn send_to_session(
    jsonl_path: &Path,
    session_id: &str,
    content: &str,
) -> std::io::Result<()> {
    let entry = json!({
        "type": "queue-operation",
        "operation": "enqueue",
        "timestamp": Utc::now().to_rfc3339(),
        "sessionId": session_id,
        "content": content
    });

    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(false)
        .append(true)
        .open(jsonl_path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.flush().await?;

    info!(
        "Injected message into session {session_id} at {}",
        jsonl_path.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::fs;

    #[tokio::test]
    async fn test_send_to_session() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write an initial line so the file exists with content
        fs::write(&path, "{\"type\":\"user\"}\n").await.unwrap();

        send_to_session(&path, "test-session-123", "Hello from klyntbot")
            .await
            .unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let injected: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(injected["type"], "queue-operation");
        assert_eq!(injected["operation"], "enqueue");
        assert_eq!(injected["sessionId"], "test-session-123");
        assert_eq!(injected["content"], "Hello from klyntbot");
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_file_fails() {
        let result = send_to_session(
            Path::new("/tmp/nonexistent-session-tracker-test.jsonl"),
            "test",
            "hello",
        )
        .await;
        assert!(result.is_err());
    }
}
