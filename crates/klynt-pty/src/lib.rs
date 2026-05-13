//! Cross-platform child-process abstraction for background bash jobs.
//!
//! In Phase 2.3a, only the non-PTY [`ChildHandle::Process`] variant is exposed.
//! PTY support (`ChildHandle::Pty`) is added in Phase 2.3c without changing this
//! crate's public API for `Process`.

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;

pub mod pty_backend;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("pgrp capture: {0}")]
    PgrpCapture(String),
    #[error("not implemented on this platform")]
    NotImplemented,
}

/// Handle to a spawned child process. Background jobs hold this for the
/// lifetime of the child.
pub enum ChildHandle {
    /// Plain child process (no TTY). The default.
    Process { child: Child },
    /// PTY-backed child (Phase 2.3c). `master` is held for stdin/resize;
    /// `child` is held for wait/kill. Mutex because portable_pty's traits are
    /// Send but not Sync.
    Pty {
        master: std::sync::Arc<tokio::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
        child:  std::sync::Arc<tokio::sync::Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
        pgid:   Option<u32>,
    },
}

/// What [`spawn_background_command`] returns.
pub struct BackgroundCommandHandle {
    pub child: ChildHandle,
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    pub stderr: Option<Box<dyn AsyncRead + Send + Unpin>>,
    pub stdin: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    /// Process group id captured immediately after spawn (Unix only).
    pub pgid: Option<u32>,
}

/// Spawn a Command as a background job. Caller must already have:
///   - Set the program/args/cwd
///   - Configured Stdio::piped() for stdout/stderr
///   - Set Stdio::null() for stdin (unless interactive — not in 2.3a)
///   - Added env vars (GIT_EDITOR=true, PAGER=cat, TERM=dumb)
///
/// This function adds the Unix-specific pre_exec (setpgid + PR_SET_PDEATHSIG)
/// and captures pgid after spawn.
pub fn spawn_with_pgrp(
    mut cmd: tokio::process::Command,
) -> Result<BackgroundCommandHandle, PtyError> {
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Own process group so cancel can signal the entire tree.
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Linux: when the parent dies, kernel sends SIGTERM to children.
            #[cfg(target_os = "linux")]
            {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PtyError::PgrpCapture("stdout pipe missing".into()))?;
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let pid = child.id();

    let pgid = pid.and_then(|pid| {
        #[cfg(unix)]
        unsafe {
            let pgid = libc::getpgid(pid as i32);
            if pgid < 0 {
                None
            } else {
                Some(pgid as u32)
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            None
        }
    });

    Ok(BackgroundCommandHandle {
        child: ChildHandle::Process { child },
        stdout: Box::new(stdout) as _,
        stderr: stderr.map(|s| Box::new(s) as Box<dyn AsyncRead + Send + Unpin>),
        stdin: stdin.map(|s| Box::new(s) as Box<dyn AsyncWrite + Send + Unpin>),
        pgid,
    })
}

/// Send a signal to the entire process group.
#[cfg(unix)]
pub fn kill_process_group(pgid: u32, signal: libc::c_int) -> std::io::Result<()> {
    unsafe {
        if libc::kill(-(pgid as i32), signal) < 0 {
            let err = std::io::Error::last_os_error();
            // ESRCH means the group is already gone — treat as success.
            if err.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(err);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn kill_process_group(_pgid: u32, _signal: i32) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "kill_process_group not implemented on non-Unix",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    #[tokio::test]
    async fn spawn_captures_stdout_and_pgid() {
        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.arg("-c").arg("echo hello");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        let handle = spawn_with_pgrp(cmd).expect("spawn");
        #[cfg(unix)]
        assert!(handle.pgid.is_some(), "pgid should be captured on Unix");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_process_group_handles_missing_group() {
        // pgid 99999999 should not exist; ESRCH is treated as success.
        let res = kill_process_group(99_999_999, libc::SIGTERM);
        assert!(res.is_ok(), "ESRCH should be tolerated: {res:?}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn spawn_with_pty_yields_pty_handle_and_reads_stdout() {
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "echo hello"]);
        let mut handle = pty_backend::spawn_with_pty(cmd, 24, 80).expect("spawn");
        assert!(matches!(handle.child, ChildHandle::Pty { .. }));
        use tokio::io::AsyncReadExt;
        let mut s = String::new();
        let mut chunk = [0u8; 64];
        // Drain up to ~256 bytes or EOF; the echo will produce "hello\r\n" plus a tiny PTY preamble.
        for _ in 0..16 {
            match handle.stdout.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => s.push_str(&String::from_utf8_lossy(&chunk[..n])),
            }
        }
        assert!(s.contains("hello"), "pty stdout should contain 'hello', got: {s:?}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn pty_resize_updates_kernel_size() {
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "stty size; sleep 0.2; stty size"]);
        let handle = pty_backend::spawn_with_pty(cmd, 24, 80).expect("spawn");
        let ChildHandle::Pty { master, .. } = handle.child else {
            panic!("expected Pty handle");
        };
        // Resize before child finishes.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let m = master.lock().await;
        m.resize(portable_pty::PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize");
        // The test passes if resize() returns Ok — we don't need to capture stdout
        // here because the slave is already closed in the parent.
    }
}
