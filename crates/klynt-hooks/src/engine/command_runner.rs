use crate::schema::Hook;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct CommandRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

pub async fn run_command(hook: &Hook, input: &str) -> CommandRunResult {
    let timeout_ms = hook.timeout_ms.unwrap_or(5000);
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&hook.command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CommandRunResult {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("spawn failed: {e}")),
            }
        }
    };

    // Write stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let input = input.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(input.as_bytes()).await;
        });
    }

    let result = timeout(Duration::from_millis(timeout_ms), child.wait_with_output()).await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            CommandRunResult {
                exit_code: output.status.code(),
                stdout,
                stderr,
                error: None,
            }
        }
        Ok(Err(e)) => CommandRunResult {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("io error: {e}")),
        },
        Err(_) => CommandRunResult {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("timed out after {timeout_ms}ms")),
        },
    }
}
