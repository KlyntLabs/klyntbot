//! Cron command handlers for scheduled jobs

use anyhow::Result;
use crate::cli::CronCommands;
use crate::cron::{CronSchedule, CronService};

/// Handle cron commands
pub async fn handle_cron(cmd: CronCommands) -> Result<()> {
    let config = crate::config::load()?;
    let cron_store_path = config.workspace_path().join(".klyntbot").join("cron.json");
    let cron_service = CronService::new(cron_store_path);

    match cmd {
        CronCommands::List { all } => {
            let jobs = cron_service.list_jobs(all).await;
            if jobs.is_empty() {
                println!("No scheduled jobs");
                return Ok(());
            }

            println!("Scheduled jobs:\n");
            for job in jobs {
                let status = if job.enabled { "✓" } else { "✗" };
                let next_run = if let Some(next_ms) = job.state.next_run_at_ms {
                    crate::utils::format_timestamp_ms(next_ms)
                } else {
                    "never".to_string()
                };

                println!("{} {} ({})", status, job.name, job.id);
                println!("   Schedule: {:?}", job.schedule);
                println!("   Next run: {}", next_run);
                if let Some(last_status) = &job.state.last_status {
                    println!("   Last status: {}", last_status);
                }
                println!();
            }
        }

        CronCommands::Add {
            name,
            message,
            every,
            cron,
            at,
            deliver,
            to,
            channel,
        } => {
            // Parse schedule
            let schedule = if let Some(every_s) = every {
                CronSchedule::Every {
                    every_ms: every_s * 1000,
                }
            } else if let Some(cron_expr) = cron {
                CronSchedule::Cron {
                    expr: cron_expr,
                    tz: None,
                }
            } else if let Some(at_str) = at {
                // Parse ISO timestamp
                use chrono::DateTime;
                let dt = DateTime::parse_from_rfc3339(&at_str)
                    .map_err(|e| anyhow::anyhow!("Invalid timestamp: {}", e))?;
                CronSchedule::At {
                    at_ms: dt.timestamp_millis(),
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Schedule is required\n\nSpecify one of:\n  --every <seconds>  (e.g., --every 3600)\n  --cron <expr>      (e.g., --cron \"0 9 * * *\")\n  --at <timestamp>   (e.g., --at \"2026-12-31T23:59:59Z\")"
                ));
            };

            let job = cron_service
                .add_job(name, schedule, message, deliver, channel, to, false)
                .await?;

            println!("✓ Added job: {} ({})", job.name, job.id);
        }

        CronCommands::Remove { job_id } => {
            let removed = cron_service.remove_job(&job_id).await?;
            if removed {
                println!("✓ Removed job: {}", job_id);
            } else {
                println!("✗ Job not found: {}", job_id);
            }
        }

        CronCommands::Run { job_id, force } => {
            let ran = cron_service.run_job(&job_id, force).await?;
            if ran {
                println!("✓ Executed job: {}", job_id);
            } else {
                println!("✗ Job not found or disabled: {}", job_id);
            }
        }

        CronCommands::Enable { job_id } => {
            let job = cron_service.enable_job(&job_id, true).await?;
            if let Some(job) = job {
                println!("✓ Enabled job: {}", job.name);
            } else {
                println!("✗ Job not found: {}", job_id);
            }
        }

        CronCommands::Disable { job_id } => {
            let job = cron_service.enable_job(&job_id, false).await?;
            if let Some(job) = job {
                println!("✓ Disabled job: {}", job.name);
            } else {
                println!("✗ Job not found: {}", job_id);
            }
        }
    }
    Ok(())
}
