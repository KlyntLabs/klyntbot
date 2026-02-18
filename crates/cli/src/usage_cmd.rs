//! Usage report CLI handler.

use anyhow::Result;

use agent::output::CostTracker;
use common::utils::terminal::*;

use super::commands::UsageCommands;

/// Handle usage subcommands.
pub async fn handle_usage(cmd: UsageCommands) -> Result<()> {
    match cmd {
        UsageCommands::Report { days } => handle_usage_report(days).await,
    }
}

async fn handle_usage_report(days: u32) -> Result<()> {
    let data_dir = config::config_dir()?.join("data");
    let tracker = CostTracker::new(data_dir);
    let report = tracker.report(days).await?;

    println!("LLM Usage Report (last {} days)", days);
    println!("{}", "━".repeat(50));
    println!();

    println!("Total requests:  {}", report.total_requests);
    println!("Total tokens:    {}", format_tokens(report.total_tokens));
    println!(
        "Estimated cost:  ${}",
        format_cost(report.total_cost_usd)
    );
    println!();

    if !report.by_model.is_empty() {
        println!("By Model");
        println!("{}", "─".repeat(50));
        println!("{:<30} {:>10} {:>8}", "Model", "Tokens", "Cost");
        println!("{}", "─".repeat(50));

        let mut models: Vec<_> = report.by_model.iter().collect();
        models.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());

        for (model, (tokens, cost)) in &models {
            println!(
                "{:<30} {:>10} {:>8}",
                model,
                format_tokens(*tokens),
                format!("${}", format_cost(*cost))
            );
        }
        println!();
    }

    if !report.by_day.is_empty() {
        println!("By Day");
        println!("{}", "─".repeat(50));
        println!("{:<20} {:>8}", "Date", "Cost");
        println!("{}", "─".repeat(50));

        for (day, cost) in &report.by_day {
            println!("{:<20} {:>8}", day, format!("${}", format_cost(*cost)));
        }
        println!();
    }

    if report.total_requests == 0 {
        println!(
            "{} No usage data found. Usage tracking starts when the agent processes messages.",
            colorize("Note:", DIM)
        );
    }

    Ok(())
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("{:.4}", cost)
    } else {
        format!("{:.2}", cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_500_000), "1.5M");
        assert_eq!(format_tokens(1_000_000), "1.0M");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(50_000), "50.0K");
        assert_eq!(format_tokens(1_000), "1.0K");
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(0), "0");
    }

    #[test]
    fn test_format_cost_small() {
        assert_eq!(format_cost(0.005), "0.0050");
        assert_eq!(format_cost(0.0001), "0.0001");
    }

    #[test]
    fn test_format_cost_normal() {
        assert_eq!(format_cost(4.50), "4.50");
        assert_eq!(format_cost(123.45), "123.45");
    }
}
