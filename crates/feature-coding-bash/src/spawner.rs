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

/// Build a `portable_pty::CommandBuilder` for a sandboxed PTY job.
///
/// Mirrors `spawn_background_command`'s setup (cwd, env, sandbox wrapping) but
/// hands the resulting argv to `portable-pty` instead of `tokio::Command`.
pub fn build_pty_command(
    sandbox: &MacOsSeatbeltRunner,
    command: &str,
    cwd: &std::path::Path,
) -> Result<portable_pty::CommandBuilder, SpawnError> {
    let policy = SandboxPolicy::cwd_writes_only(cwd.to_path_buf());
    // We need the same argv that build_sandboxed_command would produce. The
    // simplest portable approach: ask the sandbox runner for the sandbox-exec
    // argv via the tokio::Command path, then extract program + args.
    let cmd = sandbox
        .build_sandboxed_command(&policy, "/bin/bash", &["-c", command])
        .map_err(|e| SpawnError::Sandbox(e.to_string()))?;
    let program = cmd.as_std().get_program().to_os_string();
    let args: Vec<_> = cmd
        .as_std()
        .get_args()
        .map(|a| a.to_os_string())
        .collect();

    let mut pb = portable_pty::CommandBuilder::new(program);
    for a in args {
        pb.arg(a);
    }
    pb.cwd(cwd);
    pb.env("GIT_EDITOR", "true");
    pb.env("PAGER", "cat");
    pb.env("TERM", "xterm-256color");
    Ok(pb)
}

/// PTY-mode counterpart of [`spawn_background_command`].
pub fn spawn_pty(
    sandbox: &MacOsSeatbeltRunner,
    command: &str,
    cwd: &std::path::Path,
    rows: u16,
    cols: u16,
) -> Result<klynt_pty::BackgroundCommandHandle, SpawnError> {
    let cb = build_pty_command(sandbox, command, cwd)?;
    Ok(klynt_pty::pty_backend::spawn_with_pty(cb, rows, cols)?)
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

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn spawn_pty_sets_term_xterm_256() {
        let sandbox = MacOsSeatbeltRunner::new();
        let dir = tempfile::tempdir().unwrap();
        let mut handle = spawn_pty(&sandbox, "echo $TERM", dir.path(), 24, 80)
            .expect("spawn_pty");
        let mut s = String::new();
        let mut chunk = [0u8; 64];
        use tokio::io::AsyncReadExt;
        for _ in 0..16 {
            match handle.stdout.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => s.push_str(&String::from_utf8_lossy(&chunk[..n])),
            }
        }
        assert!(
            s.contains("xterm-256color"),
            "expected TERM=xterm-256color, got: {s:?}"
        );
    }
}
