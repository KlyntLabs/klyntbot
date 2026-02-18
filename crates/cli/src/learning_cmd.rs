//! Learning system status CLI handler.

use anyhow::Result;

use agent::learning::{
    compute_stats, OutcomeStore, StrategyLearningStore, StrategyRecord, ToolConfidenceMap,
};
use common::utils::terminal::*;

use super::commands::LearningCommands;

/// Handle learning subcommands.
pub async fn handle_learning(cmd: LearningCommands) -> Result<()> {
    match cmd {
        LearningCommands::Status { days } => handle_learning_status(days).await,
    }
}

async fn handle_learning_status(days: u32) -> Result<()> {
    let data_dir = config::config_dir()?.join("data");

    println!("Learning System Status");
    println!("{}", "━".repeat(50));
    println!();

    // Strategy effectiveness
    let strategy_path = data_dir.join("strategy_learning.jsonl");
    let mut strategy_store = StrategyLearningStore::new(strategy_path);

    if strategy_store.load().await.is_ok() {
        let records = strategy_store.records().to_vec();

        if records.is_empty() {
            println!(
                "Strategy Tracking: {} No data yet",
                colorize("(empty)", DIM)
            );
        } else {
            println!("Strategy Effectiveness (last {} days)", days);
            println!("{}", "─".repeat(50));
            println!(
                "{:<20} {:>8} {:>10} {:>10}",
                "Strategy", "Accuracy", "Avg Time", "Count"
            );
            println!("{}", "─".repeat(50));

            // Group by strategy
            let strategies = ["DirectResponse", "ToolAssisted", "AutonomousTask"];
            for strategy in &strategies {
                let strategy_records: Vec<StrategyRecord> = records
                    .iter()
                    .filter(|r| r.predicted_strategy == *strategy)
                    .cloned()
                    .collect();

                if strategy_records.is_empty() {
                    continue;
                }

                let stats = compute_stats(strategy, &strategy_records);
                let accuracy_pct = (stats.accuracy * 100.0) as u32;
                let avg_time = if stats.sample_count > 0 {
                    strategy_records
                        .iter()
                        .map(|r| r.response_time_ms)
                        .sum::<u64>()
                        / stats.sample_count as u64
                } else {
                    0
                };

                println!(
                    "{:<20} {:>7}% {:>8}ms {:>10}",
                    strategy, accuracy_pct, avg_time, stats.sample_count
                );
            }
            println!();
        }
    } else {
        println!(
            "Strategy Tracking: {} Unable to load store",
            colorize("(error)", DIM)
        );
    }

    // Outcome store stats
    let outcome_path = data_dir.join("outcomes.jsonl");
    let mut outcome_store = OutcomeStore::new(outcome_path);
    if outcome_store.load().await.is_ok() {
        if let Ok(records) = outcome_store.get_all_outcomes().await {
            let total = records.len();
            let successes = records.iter().filter(|r| r.success).count();
            let success_rate = if total > 0 {
                (successes as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            println!("Outcome History");
            println!("{}", "─".repeat(50));
            println!("Total outcomes:  {}", total);
            println!("Success rate:    {:.1}%", success_rate);
            println!();
        }
    }

    // Tool confidence
    let confidence_map = ToolConfidenceMap::new(0.7);
    println!("Tool Confidence");
    println!("{}", "─".repeat(50));
    println!(
        "Default threshold: {:.0}%",
        confidence_map.default_threshold() * 100.0
    );
    println!("Custom overrides:  {}", confidence_map.overrides_count());
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_commands_parse() {
        // Verify the enum variant exists and is constructable
        let cmd = LearningCommands::Status { days: 7 };
        match cmd {
            LearningCommands::Status { days } => assert_eq!(days, 7),
        }
    }
}
