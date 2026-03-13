//! Cron tool for scheduling tasks via the agent.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use crate::params::ParamExtractor;
use crate::{RoutingContext, Tool};
use common::{Result, ToolError};

/// Format timestamp milliseconds to human-readable string
fn format_timestamp_ms(ms: u64) -> String {
    use std::time::{Duration, SystemTime};
    let time = SystemTime::UNIX_EPOCH + Duration::from_millis(ms);
    match time.duration_since(SystemTime::now()) {
        Ok(d) => format!("in {}s", d.as_secs()),
        Err(_) => "past".to_string(),
    }
}

/// Cron schedule type (will be defined in klyntbot-cron)
#[derive(Debug, Clone)]
pub enum CronSchedule {
    /// Run every N milliseconds
    Every { every_ms: u64 },
    /// Run on a cron expression
    Cron { expr: String, tz: Option<String> },
}

/// Cron job information (simplified for trait interface)
#[derive(Debug, Clone)]
pub struct CronJobInfo {
    pub id: String,
    pub name: String,
    pub next_run_at_ms: Option<u64>,
    pub last_status: Option<String>,
}

/// Parameters for adding a cron job
#[derive(Debug, Clone)]
pub struct AddCronJobParams {
    pub name: String,
    pub schedule: CronSchedule,
    pub message: String,
    pub enabled: bool,
    pub channel: Option<String>,
    pub to: Option<String>,
    pub internal: bool,
    pub origin: String,
}

/// Trait for cron job management (dependency inversion to avoid circular dependencies).
/// Implemented by klyntbot-cron's CronService.
#[async_trait]
pub trait CronHandler: Send + Sync {
    /// Add a new cron job
    async fn add_job(&self, params: AddCronJobParams) -> Result<CronJobInfo>;

    /// List all cron jobs
    async fn list_jobs(&self, include_internal: bool) -> Vec<CronJobInfo>;

    /// Remove a cron job by ID
    async fn remove_job(&self, job_id: &str) -> Result<bool>;
}

/// Tool for cron job management via the LLM agent.
pub struct CronTool {
    handler: Option<Arc<dyn CronHandler>>,
}

impl CronTool {
    /// Create a new cron tool without a handler (will error if used)
    pub fn new() -> Self {
        Self { handler: None }
    }

    /// Create with a CronHandler implementation
    pub fn with_handler(handler: Arc<dyn CronHandler>) -> Self {
        Self {
            handler: Some(handler),
        }
    }
}

impl Default for CronTool {
    fn default() -> Self {
        Self::new()
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

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::System,
            tags: vec!["schedule".into(), "cron".into(), "recurring".into()],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
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
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        debug!("Cron action: {}", action);

        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("CronHandler not available".to_string()))?;

        match action {
            "add" => {
                let name = p.str_or("name", "scheduled task")?;
                let message = p.required_str("message")?;

                // Determine schedule type
                let schedule = if let Some(every_s) = p.optional_u64("every_seconds")? {
                    CronSchedule::Every {
                        every_ms: every_s * 1000,
                    }
                } else if let Some(cron_expr) = p.optional_str("cron_expr")? {
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
                let params = AddCronJobParams {
                    name: name.to_string(),
                    schedule,
                    message: message.to_string(),
                    enabled: true,
                    channel: Some(ctx.channel.as_str().to_string()),
                    to: Some(ctx.chat_id.as_str().to_string()),
                    internal: false,
                    origin: "ai".to_string(),
                };

                let job = handler.add_job(params).await?;

                if let Some(ref tx) = ctx.entity_tx {
                    let next_run = job
                        .next_run_at_ms
                        .map(format_timestamp_ms)
                        .unwrap_or_else(|| "pending".to_string());
                    let _ = tx
                        .send(common::EntityCard {
                            entity_type: "cron".to_string(),
                            entity_id: job.id.clone(),
                            title: job.name.clone(),
                            subtitle: Some(format!("Next: {}", next_run)),
                            route: Some("/cron".to_string()),
                            icon_hint: "clock".to_string(),
                            metadata: std::collections::HashMap::new(),
                        })
                        .await;
                }

                Ok(format!(
                    "Scheduled job '{}' (ID: {}). Next run: {}",
                    job.name,
                    job.id,
                    job.next_run_at_ms
                        .map(format_timestamp_ms)
                        .unwrap_or_else(|| "pending".to_string())
                ))
            }
            "list" => {
                let jobs = handler.list_jobs(false).await;
                if jobs.is_empty() {
                    return Ok("No scheduled jobs.".to_string());
                }

                let mut output = format!("{} scheduled job(s):\n", jobs.len());
                for job in jobs {
                    let next = job
                        .next_run_at_ms
                        .map(format_timestamp_ms)
                        .unwrap_or_else(|| "none".to_string());
                    output.push_str(&format!(
                        "\n- {} (ID: {}) — next: {}",
                        job.name, job.id, next
                    ));
                    if let Some(status) = &job.last_status {
                        output.push_str(&format!(", last: {}", status));
                    }
                }

                Ok(output)
            }
            "remove" => {
                let job_id = p.required_str("job_id")?;

                let removed = handler.remove_job(job_id).await?;
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
