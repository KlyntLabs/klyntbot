use crate::error::SandboxError;
use crate::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use crate::runner::{CommandOutput, SandboxRunner};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const TEMPLATE: &str = include_str!("seatbelt_template.sbpl");

pub struct MacOsSeatbeltRunner;

impl MacOsSeatbeltRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsSeatbeltRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsSeatbeltRunner {
    fn render_policy(p: &SandboxPolicy) -> Result<String, SandboxError> {
        let cwd = p
            .cwd
            .canonicalize()
            .map_err(|e| SandboxError::PolicyGen(format!("canonicalize cwd: {e}")))?
            .to_string_lossy()
            .into_owned();
        let extra = match &p.fs {
            FsConstraints::WriteCwdReadAll { .. } => String::new(),
            FsConstraints::ReadCwdOnly { .. } => "(deny file-write*)".into(),
            FsConstraints::None => "(deny file-write*)".into(),
        };
        let net = match p.network {
            NetworkConstraints::Allow => "(allow network*)".to_string(),
            NetworkConstraints::Block => "(deny network*)".to_string(),
        };
        Ok(TEMPLATE
            .replace("{{CWD}}", &cwd)
            .replace("{{EXTRA_WRITES}}", &extra)
            .replace("{{NETWORK}}", &net))
    }
}

#[async_trait]
impl SandboxRunner for MacOsSeatbeltRunner {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError> {
        let policy_str = Self::render_policy(policy)?;
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p").arg(&policy_str);
        cmd.arg(program).args(args);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn()?;
        let out = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(r) => r?,
            Err(_) => return Err(SandboxError::ChildExit(124)),
        };
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
            exit_code: out.status.code().unwrap_or(-1),
        })
    }
}
