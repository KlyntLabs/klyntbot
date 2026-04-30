use crate::{policy::SandboxPolicy, SandboxError};
use async_trait::async_trait;
use std::{path::Path, time::Duration};

pub struct CommandOutput {
    pub stdout: String,
    pub exit_code: i32,
}

#[async_trait]
pub trait SandboxRunner: Send + Sync {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError>;
}
