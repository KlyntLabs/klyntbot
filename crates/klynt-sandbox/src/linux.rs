// crates/klynt-sandbox/src/linux.rs
#![cfg(target_os = "linux")]

use crate::bwrap::build_bwrap_args;
use crate::error::SandboxError;
use crate::helper_proto::{HelperMode, HelperPolicy};
use crate::policy::SandboxPolicy;
use crate::runner::{CommandOutput, SandboxRunner};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// bwrap available, Landlock available — full isolation.
    WithBwrap,
    /// bwrap missing — Landlock-only (network NOT isolated).
    LandlockOnly,
    /// Neither available.
    Unavailable,
}

pub struct LinuxSandboxRunner {
    helper_path: PathBuf,
    mode: SandboxMode,
}

impl LinuxSandboxRunner {
    pub fn new() -> Result<Self, SandboxError> {
        let parent_exe = std::env::current_exe()
            .map_err(|e| SandboxError::Unavailable(format!("current_exe: {e}")))?;
        let helper_path = locate_helper(Some(&parent_exe))?;

        let bwrap_present = which::which("bwrap").is_ok();
        let landlock_present = is_landlock_available();
        let mode = match (bwrap_present, landlock_present) {
            (true, true) => SandboxMode::WithBwrap,
            (true, false) => SandboxMode::WithBwrap, // bwrap suffices for namespaces
            (false, true) => SandboxMode::LandlockOnly,
            (false, false) => SandboxMode::Unavailable,
        };
        Ok(Self { helper_path, mode })
    }

    pub fn mode(&self) -> SandboxMode {
        self.mode
    }
}

pub fn locate_helper(parent_exe: Option<&Path>) -> Result<PathBuf, SandboxError> {
    if let Some(exe) = parent_exe {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("klynt-sandbox-helper");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Ok(p) = which::which("klynt-sandbox-helper") {
        return Ok(p);
    }
    Err(SandboxError::Unavailable(
        "klynt-sandbox-helper not found".into(),
    ))
}

fn is_landlock_available() -> bool {
    // ABI::new_current() returns Unsupported when kernel < 5.13. Probing via
    // a no-op ruleset would require linking landlock crate here; instead we
    // attempt a syscall via the sandbox-helper at LinuxSandboxRunner::new()
    // time. For now, optimistic-true; helper exit code 125 surfaces failure.
    true
}

#[async_trait]
impl SandboxRunner for LinuxSandboxRunner {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError> {
        if matches!(self.mode, SandboxMode::Unavailable) {
            return Err(SandboxError::Unavailable(
                "neither bwrap nor Landlock available on this host".into(),
            ));
        }

        let helper_mode = match self.mode {
            SandboxMode::WithBwrap => HelperMode::WithBwrap,
            SandboxMode::LandlockOnly => HelperMode::LandlockOnly,
            SandboxMode::Unavailable => unreachable!(),
        };
        let policy_b64 = HelperPolicy {
            mode: helper_mode,
            sandbox: policy.clone(),
        }
        .to_base64_json()
        .map_err(|e| SandboxError::PolicyGen(e.to_string()))?;

        let mut command = match self.mode {
            SandboxMode::WithBwrap => {
                // bwrap … -- helper --landlock <b64> -- <program> <args...>
                let helper_str = self.helper_path.to_string_lossy().into_owned();
                let mut helper_args: Vec<&str> =
                    vec!["--landlock", policy_b64.as_str(), "--", program];
                helper_args.extend(args.iter().copied());
                let bwrap_args = build_bwrap_args(policy, &helper_str, &helper_args);
                let mut c = Command::new("/usr/bin/bwrap");
                c.args(&bwrap_args);
                c
            }
            SandboxMode::LandlockOnly => {
                let mut c = Command::new(&self.helper_path);
                c.arg("--landlock-only")
                    .arg(&policy_b64)
                    .arg("--")
                    .arg(program)
                    .args(args);
                c
            }
            SandboxMode::Unavailable => unreachable!(),
        };

        if let Some(d) = cwd {
            command.current_dir(d);
        }
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = command.spawn()?;
        let out = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(r) => r?,
            Err(_) => return Err(SandboxError::ChildExit(124)),
        };

        let exit_code = out.status.code().unwrap_or(-1);
        // Map helper-reserved exit codes
        if exit_code == 125 {
            return Err(SandboxError::Unavailable("landlock not enforced".into()));
        }
        Ok(CommandOutput {
            stdout: format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
            exit_code,
        })
    }
}
