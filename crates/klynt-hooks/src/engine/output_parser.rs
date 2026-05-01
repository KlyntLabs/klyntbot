//! Parse hook subprocess stdout into a structured `HookResponse`.

use crate::types::HookResponse;

/// Parse the stdout of a hook subprocess.
///
/// Empty stdout → default "allow" response.
/// Non-empty stdout → try JSON deserialization, fall back to default on error.
pub fn parse_hook_stdout(stdout: &str) -> HookResponse {
    if stdout.trim().is_empty() {
        HookResponse {
            r#continue: true,
            block: false,
            reason: None,
            modify_args: None,
        }
    } else {
        serde_json::from_str(stdout).unwrap_or_else(|_| HookResponse {
            r#continue: true,
            block: false,
            reason: None,
            modify_args: None,
        })
    }
}
