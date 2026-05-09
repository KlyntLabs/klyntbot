//! Cross-platform child-process abstraction for background bash jobs.
//!
//! In Phase 2.3a, only the non-PTY [`ChildHandle::Process`] variant is exposed.
//! PTY support (`ChildHandle::Pty`) is added in Phase 2.3c without changing this
//! crate's public API for `Process`.

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;

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
    /// Plain child process (no TTY). The default in 2.3a.
    Process { child: Child },
    // 2.3c will add: Pty { master: Box<dyn MasterPty + Send>, child: Box<dyn ChildKiller + Send> }
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
}
