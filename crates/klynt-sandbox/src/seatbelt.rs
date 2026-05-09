use crate::error::SandboxError;
use crate::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use crate::runner::{CommandOutput, SandboxRunner};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;

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

    /// Build a fully-configured Command (sandbox-exec wrapper) without spawning.
    /// Used by both run_command (foreground) and feature-coding-bash (background).
    /// Caller is responsible for setting cwd, stdin, stdout, stderr, env, pre_exec.
    pub fn build_sandboxed_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
    ) -> Result<tokio::process::Command, SandboxError> {
        let policy_str = Self::render_policy(policy)?;
        let mut cmd = tokio::process::Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p").arg(&policy_str);
        cmd.arg(program).args(args);
        Ok(cmd)
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
        let mut cmd = self.build_sandboxed_command(policy, program, args)?;
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();

        let stdout_fut = async move {
            let mut buf = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut stdout_pipe, &mut buf)
                .await
                .ok();
            buf
        };
        let stderr_fut = async move {
            let mut buf = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut buf)
                .await
                .ok();
            buf
        };

        let timeout_result = tokio::time::timeout(timeout, async {
            let (status, stdout_bytes, stderr_bytes) =
                tokio::join!(child.wait(), stdout_fut, stderr_fut);
            let status = status?;
            Ok::<_, std::io::Error>((status, stdout_bytes, stderr_bytes))
        })
        .await;

        match timeout_result {
            Ok(Ok((status, stdout_bytes, stderr_bytes))) => Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&stdout_bytes).into_owned()
                    + &String::from_utf8_lossy(&stderr_bytes),
                exit_code: status.code().unwrap_or(-1),
            }),
            Ok(Err(e)) => Err(SandboxError::Spawn(e)),
            Err(_) => {
                let _ = child.kill().await;
                Err(SandboxError::ChildExit(124))
            }
        }
    }
}
