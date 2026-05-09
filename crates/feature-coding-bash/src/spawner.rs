//! Configured Command builder for background bash jobs.
//!
//! Wraps the seatbelt sandbox + adds the env/stdio/pre_exec needed for
//! a long-lived child process.

use std::path::Path;
use std::process::Stdio;

use klynt_pty::{spawn_with_pgrp, BackgroundCommandHandle, PtyError};
use klynt_sandbox::{MacOsSeatbeltRunner, SandboxPolicy};

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("pty: {0}")]
    Pty(#[from] PtyError),
    #[error("sandbox: {0}")]
    Sandbox(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub fn spawn_background_command(
    sandbox: &MacOsSeatbeltRunner,
    command: &str,
    cwd: &Path,
) -> Result<BackgroundCommandHandle, SpawnError> {
    let policy = SandboxPolicy::cwd_writes_only(cwd.to_path_buf());
    let mut cmd = sandbox
        .build_sandboxed_command(&policy, "/bin/bash", &["-c", command])
        .map_err(|e| SpawnError::Sandbox(e.to_string()))?;
    cmd.current_dir(cwd);
    cmd.env("GIT_EDITOR", "true")
        .env("PAGER", "cat")
        .env("TERM", "dumb");
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(spawn_with_pgrp(cmd)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn spawned_child_inherits_env() {
        let sandbox = MacOsSeatbeltRunner::new();
        let dir = tempfile::tempdir().unwrap();
        let mut handle =
            spawn_background_command(&sandbox, "echo $GIT_EDITOR", dir.path()).expect("spawn");
        let mut buf = Vec::new();
        handle.stdout.read_to_end(&mut buf).await.unwrap();
        let s = String::from_utf8_lossy(&buf);
        assert!(
            s.contains("true"),
            "GIT_EDITOR=true should be set, got: {s:?}"
        );
    }
}
