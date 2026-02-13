//! Shell execution tool with safety guards.

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn};

use super::{RoutingContext, Tool};
use common::{Result, ToolError};

/// Tool to execute shell commands
pub struct ExecTool {
    timeout: Duration,
    working_dir: Option<PathBuf>,
    deny_patterns: Vec<Regex>,
    allow_patterns: Vec<Regex>,
    restrict_to_workspace: bool,
}

impl ExecTool {
    pub fn new(
        timeout_secs: u64,
        working_dir: Option<PathBuf>,
        restrict_to_workspace: bool,
    ) -> Self {
        // Default deny patterns for safety
        let deny_patterns = [
            r"\brm\s+-[rf]{1,2}\b",            // rm -r, rm -rf, rm -fr
            r"\bdel\s+/[fq]\b",                // del /f, del /q
            r"\brmdir\s+/s\b",                 // rmdir /s
            r"\b(format|mkfs|diskpart)\b",     // disk operations
            r"\bdd\s+if=",                     // dd
            r">\s*/dev/sd",                    // write to disk
            r"\b(shutdown|reboot|poweroff)\b", // system power
            r":\(\)\s*\{.*\};\s*:",            // fork bomb
            r"\bcurl\s+.*\|\s*(sh|bash)\b",    // curl pipe to shell
            r"\bwget\s+.*\|\s*(sh|bash)\b",    // wget pipe to shell
            r"\bnc\s+-[el]",                   // netcat listeners
            r"\bchmod\s+[0-7]*777\b",          // world-writable permissions
            r"\bchown\s+root\b",               // chown to root
            r"\bsudo\b",                       // sudo commands
            r"\bsu\s+-\b",                     // switch user
            r"\b(iptables|firewall-cmd)\b",    // firewall changes
            r"\bcrontab\s+-[re]\b",            // crontab edit/remove
            r"\bpasswd\b",                     // password changes
        ];

        let compiled_deny = deny_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            timeout: Duration::from_secs(timeout_secs),
            working_dir,
            deny_patterns: compiled_deny,
            allow_patterns: Vec::new(),
            restrict_to_workspace,
        }
    }

    /// Guard command for safety
    fn guard_command(&self, command: &str, cwd: &Path) -> std::result::Result<(), String> {
        const MAX_COMMAND_LENGTH: usize = 4096;
        if command.len() > MAX_COMMAND_LENGTH {
            return Err("Command exceeds maximum length".to_string());
        }

        let cmd = command.trim();
        let lower = cmd.to_lowercase();

        // Check deny patterns
        for pattern in &self.deny_patterns {
            if pattern.is_match(&lower) {
                return Err(
                    "Command blocked by safety guard (dangerous pattern detected)".to_string(),
                );
            }
        }

        // Check allow patterns if configured
        if !self.allow_patterns.is_empty() {
            let allowed = self.allow_patterns.iter().any(|p| p.is_match(&lower));
            if !allowed {
                return Err("Command blocked by safety guard (not in allowlist)".to_string());
            }
        }

        // Workspace restriction
        if self.restrict_to_workspace {
            if cmd.contains("../") || cmd.contains("..\\") {
                return Err("Command blocked by safety guard (path traversal detected)".to_string());
            }

            let cwd_resolved = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

            // Check for absolute paths in command
            let win_path_re = Regex::new(r#"[A-Za-z]:\\[^\\"']+"#).unwrap();
            let posix_path_re = Regex::new(r#"(?:^|[\s|>])(/[^\s"'>]+)"#).unwrap();

            for cap in win_path_re.captures_iter(cmd) {
                if let Some(path_match) = cap.get(0) {
                    if let Ok(p) = PathBuf::from(path_match.as_str().trim()).canonicalize() {
                        if p.is_absolute() && !p.starts_with(&cwd_resolved) && p != cwd_resolved {
                            return Err(
                                "Command blocked by safety guard (path outside working dir)"
                                    .to_string(),
                            );
                        }
                    }
                }
            }

            for cap in posix_path_re.captures_iter(cmd) {
                if let Some(path_match) = cap.get(1) {
                    if let Ok(p) = PathBuf::from(path_match.as_str().trim()).canonicalize() {
                        if p.is_absolute() && !p.starts_with(&cwd_resolved) && p != cwd_resolved {
                            return Err(
                                "Command blocked by safety guard (path outside working dir)"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for ExecTool {
    fn name(&self) -> &str {
        "exec"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output. Use with caution."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'command' parameter".to_string()))?;

        let working_dir = args
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(|| self.working_dir.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        debug!("Executing command: {} in {:?}", command, working_dir);

        // Safety guard
        if let Err(msg) = self.guard_command(command, &working_dir) {
            warn!("Command blocked: {}", msg);
            return Err(ToolError::ExecutionFailed(msg).into());
        }

        // Execute command
        let shell = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        };

        let shell_arg = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg)
            .arg(command)
            .current_dir(&working_dir)
            .kill_on_drop(true);

        let child = cmd.output();

        match timeout(self.timeout, child).await {
            Ok(Ok(output)) => {
                let mut result_parts = Vec::new();

                if !output.stdout.is_empty() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    result_parts.push(stdout.to_string());
                }

                if !output.stderr.is_empty() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stderr_text = stderr.trim();
                    if !stderr_text.is_empty() {
                        result_parts.push(format!("STDERR:\n{}", stderr_text));
                    }
                }

                if !output.status.success() {
                    result_parts.push(format!(
                        "\nExit code: {}",
                        output.status.code().unwrap_or(-1)
                    ));
                }

                let result = if result_parts.is_empty() {
                    "(no output)".to_string()
                } else {
                    result_parts.join("\n")
                };

                // Truncate very long output
                const MAX_LEN: usize = 10000;
                if result.len() > MAX_LEN {
                    Ok(format!(
                        "{}... (truncated, {} more chars)",
                        &result[..MAX_LEN],
                        result.len() - MAX_LEN
                    ))
                } else {
                    Ok(result)
                }
            }
            Ok(Err(e)) => {
                Err(ToolError::ExecutionFailed(format!("Error executing command: {}", e)).into())
            }
            Err(_) => Err(ToolError::ExecutionFailed(format!(
                "Command timed out after {} seconds",
                self.timeout.as_secs()
            ))
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RoutingContext;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    #[tokio::test]
    async fn test_exec_tool_simple_command() {
        let tool = ExecTool::new(10, None, false);

        let args = serde_json::json!({
            "command": "echo 'Hello, World!'"
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_exec_tool_deny_rm_rf() {
        let tool = ExecTool::new(10, None, false);

        // Test various forms of dangerous rm commands
        let dangerous_commands = vec!["rm -rf /tmp/test", "rm -fr /tmp/test", "rm -r -f /tmp/test"];

        for cmd in dangerous_commands {
            let args = serde_json::json!({
                "command": cmd
            });

            let result = tool.execute(args, &test_ctx()).await;
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("blocked by safety guard"));
        }
    }

    #[tokio::test]
    async fn test_exec_tool_deny_shutdown() {
        let tool = ExecTool::new(10, None, false);

        let args = serde_json::json!({
            "command": "shutdown -h now"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("blocked by safety guard"));
    }

    #[tokio::test]
    async fn test_exec_tool_deny_dd() {
        let tool = ExecTool::new(10, None, false);

        let args = serde_json::json!({
            "command": "dd if=/dev/zero of=/tmp/test"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("blocked by safety guard"));
    }

    #[tokio::test]
    async fn test_exec_tool_workspace_restriction_path_traversal() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();

        let tool = ExecTool::new(10, Some(temp_dir.path().to_path_buf()), true);

        let args = serde_json::json!({
            "command": "ls ../"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("path traversal detected"));
    }

    #[tokio::test]
    async fn test_exec_tool_working_directory() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();

        let tool = ExecTool::new(10, Some(temp_dir.path().to_path_buf()), false);

        let args = serde_json::json!({
            "command": "pwd"
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        let temp_path = temp_dir.path().to_string_lossy();
        assert!(result.contains(&*temp_path));
    }

    #[tokio::test]
    async fn test_exec_tool_stderr_capture() {
        let tool = ExecTool::new(10, None, false);

        // This command should produce stderr output
        let args = serde_json::json!({
            "command": if cfg!(target_os = "windows") {
                "dir nonexistent_directory 2>&1"
            } else {
                "ls /nonexistent_directory 2>&1"
            }
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        // Should capture error output
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_exec_tool_timeout() {
        let tool = ExecTool::new(1, None, false); // 1 second timeout

        let args = serde_json::json!({
            "command": if cfg!(target_os = "windows") {
                "timeout /t 5 /nobreak"
            } else {
                "sleep 5"
            }
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn test_guard_command_deny_patterns() {
        let tool = ExecTool::new(10, None, false);
        let cwd = std::env::current_dir().unwrap();

        // These should be blocked
        assert!(tool.guard_command("rm -rf /", &cwd).is_err());
        assert!(tool.guard_command("shutdown now", &cwd).is_err());
        assert!(tool.guard_command("format c:", &cwd).is_err());

        // These should be allowed
        assert!(tool.guard_command("echo hello", &cwd).is_ok());
        assert!(tool.guard_command("ls -la", &cwd).is_ok());
    }

    #[tokio::test]
    async fn test_exec_tool_output_truncation() {
        let tool = ExecTool::new(10, None, false);

        // Generate very long output
        let args = serde_json::json!({
            "command": if cfg!(target_os = "windows") {
                "for /L %i in (1,1,1000) do @echo This is a long line of text that will be repeated many times"
            } else {
                "for i in {1..1000}; do echo 'This is a long line of text that will be repeated many times'; done"
            }
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();

        // Should be truncated if over 10000 chars
        if result.len() > 10000 {
            assert!(result.contains("truncated"));
        }
    }

    #[test]
    fn test_tool_schema() {
        let tool = ExecTool::new(10, None, false);
        let schema = tool.to_schema();

        assert_eq!(schema["type"], "function");
        assert_eq!(schema["function"]["name"], "exec");
        assert!(schema["function"]["parameters"]["properties"]["command"].is_object());
    }
}
