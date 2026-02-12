//! Cron tool for scheduling tasks via the agent.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{Tool, RoutingContext};
use crate::cron::{CronSchedule, CronService};
use crate::error::{Result, ToolError};

/// Tool for cron job management via the LLM agent.
pub struct CronTool {
    cron_service: Option<Arc<CronService>>,
}

impl CronTool {
    /// Create with a CronService reference
    pub fn with_service(service: Arc<CronService>) -> Self {
        Self {
            cron_service: Some(service),
        }
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn description(&self) -> &str {
        "Schedule reminders and recurring tasks. Actions: add, list, remove."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "remove"],
                    "description": "Action to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Job name (for add)"
                },
                "message": {
                    "type": "string",
                    "description": "Reminder message (for add)"
                },
                "every_seconds": {
                    "type": "integer",
                    "description": "Interval in seconds (for recurring tasks)"
                },
                "cron_expr": {
                    "type": "string",
                    "description": "Cron expression like '0 9 * * *' (for scheduled tasks)"
                },
                "job_id": {
                    "type": "string",
                    "description": "Job ID (for remove)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'action' parameter".to_string()))?;

        debug!("Cron action: {}", action);

        let service = self
            .cron_service
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("CronService not available".to_string()))?;

        match action {
            "add" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("scheduled task");

                let message = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParams("missing 'message' parameter for add".to_string())
                    })?;

                // Determine schedule type
                let schedule =
                    if let Some(every_s) = args.get("every_seconds").and_then(|v| v.as_u64()) {
                        CronSchedule::Every {
                            every_ms: every_s * 1000,
                        }
                    } else if let Some(cron_expr) = args.get("cron_expr").and_then(|v| v.as_str()) {
                        CronSchedule::Cron {
                            expr: cron_expr.to_string(),
                            tz: None,
                        }
                    } else {
                        return Err(ToolError::InvalidParams(
                            "must specify 'every_seconds' or 'cron_expr'".to_string(),
                        )
                        .into());
                    };

                // Use routing context for scheduling
                let channel = Some(ctx.channel.as_str().to_string());
                let to = Some(ctx.chat_id.as_str().to_string());

                let job = service
                    .add_job(name, schedule, message, true, channel, to, false)
                    .await?;

                Ok(format!(
                    "Scheduled job '{}' (ID: {}). Next run: {}",
                    job.name,
                    job.id,
                    job.state
                        .next_run_at_ms
                        .map(crate::utils::format_timestamp_ms)
                        .unwrap_or_else(|| "pending".to_string())
                ))
            }
            "list" => {
                let jobs = service.list_jobs(false).await;
                if jobs.is_empty() {
                    return Ok("No scheduled jobs.".to_string());
                }

                let mut output = format!("{} scheduled job(s):\n", jobs.len());
                for job in jobs {
                    let next = job
                        .state
                        .next_run_at_ms
                        .map(crate::utils::format_timestamp_ms)
                        .unwrap_or_else(|| "none".to_string());
                    output.push_str(&format!(
                        "\n- {} (ID: {}) — next: {}",
                        job.name, job.id, next
                    ));
                    if let Some(status) = &job.state.last_status {
                        output.push_str(&format!(", last: {}", status));
                    }
                }

                Ok(output)
            }
            "remove" => {
                let job_id = args.get("job_id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'job_id' parameter for remove".to_string())
                })?;

                let removed = service.remove_job(job_id).await?;
                if removed {
                    Ok(format!("Removed job: {}", job_id))
                } else {
                    Ok(format!("Job not found: {}", job_id))
                }
            }
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}
