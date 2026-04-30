//! Single entrypoint that runs all benches and emits the game-changer report.
//! Usage: cargo run -p kca-bench --release -- --output docs/architecture/kca-game-changer.md

use kca_bench::*;
use kca_e2e::fixtures::fixtures_root;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> common::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let output = args
        .iter()
        .enumerate()
        .find_map(|(i, a)| {
            if a == "--output" {
                args.get(i + 1).cloned()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "docs/architecture/kca-game-changer.md".into());
    if let Some((idx, _)) = args.iter().enumerate().find(|(_, a)| *a == "--limit") {
        if let Some(v) = args.get(idx + 1) {
            std::env::set_var("KCA_BENCH_LIMIT", v);
        }
    }

    let root = fixtures_root();

    println!("→ Running long-memory bench...");
    let lmb = longmembench::run_longmembench(&root.join("longmembench_subset.jsonl")).await?;
    println!(
        "  accuracy = {:.1}%, p95 = {}ms",
        lmb.accuracy() * 100.0,
        lmb.p95_query_latency_ms
    );

    println!("→ Running LoCoBench...");
    let locobench = locobench::run_locobench(&root.join("locobench_subset.jsonl")).await?;
    println!(
        "  single = {:.1}%, multi = {:.1}%, temporal = {:.1}%",
        locobench.single_hop_acc * 100.0,
        locobench.multi_hop_acc * 100.0,
        locobench.temporal_acc * 100.0
    );

    println!("→ Running Klynt-coding...");
    let kc = klynt_coding::run_klynt_coding(&root.join("klynt_coding_bench.jsonl")).await?;
    println!(
        "  dead-end = {:.1}%, fix = {:.1}%, multi-CLI = {:.1}%",
        kc.dead_end_recall * 100.0,
        kc.fix_attempt_recall * 100.0,
        kc.multi_cli_transfer_acc * 100.0
    );

    // Hot-path P50/P95 = long-memory query latencies (already real LLM
    // round-trips). LoCoBench / klynt-coding don't expose individual latency
    // arrays today; long-mem is representative of per-turn latency.
    let latency = latency::LatencyDashboard {
        hot_path_p50_ms: lmb.p50_query_latency_ms,
        hot_path_p95_ms: lmb.p95_query_latency_ms,
        ..Default::default()
    };
    let report = game_changer_report::GameChangerReport {
        lmb,
        locobench,
        klynt_coding: kc,
        latency,
        cost: cost::CostDashboard::default(),
    };

    let path = std::path::Path::new(&output);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    report.write_to_file(path)?;
    println!("→ Report written to {}", path.display());

    enforce_gates(&report)?;
    println!("✅ All Spec section 7 gates passed");
    Ok(())
}

fn enforce_gates(r: &game_changer_report::GameChangerReport) -> common::Result<()> {
    let mut failures = Vec::new();
    if r.lmb.accuracy() < 0.85 {
        failures.push(format!(
            "Q-1: long-mem accuracy {:.2} < 0.85",
            r.lmb.accuracy()
        ));
    }
    // Skip 0-sample categories so a fixture that happens to have only
    // single-hop queries doesn't fail Q-3/Q-4 with a false negative.
    if r.locobench.single_hop_total > 0 && r.locobench.single_hop_acc < 0.92 {
        failures.push(format!(
            "Q-2: LoCoBench single {:.2} < 0.92 (n={})",
            r.locobench.single_hop_acc, r.locobench.single_hop_total
        ));
    }
    if r.locobench.multi_hop_total > 0 && r.locobench.multi_hop_acc < 0.70 {
        failures.push(format!(
            "Q-3: LoCoBench multi {:.2} < 0.70 (n={})",
            r.locobench.multi_hop_acc, r.locobench.multi_hop_total
        ));
    }
    if r.locobench.temporal_total > 0 && r.locobench.temporal_acc < 0.85 {
        failures.push(format!(
            "Q-4: LoCoBench temporal {:.2} < 0.85 (n={})",
            r.locobench.temporal_acc, r.locobench.temporal_total
        ));
    }
    if r.klynt_coding.dead_end_recall < 0.80 {
        failures.push(format!(
            "Q-5a: dead-end {:.2} < 0.80",
            r.klynt_coding.dead_end_recall
        ));
    }
    if r.klynt_coding.fix_attempt_recall < 0.80 {
        failures.push(format!(
            "Q-5b: fix {:.2} < 0.80",
            r.klynt_coding.fix_attempt_recall
        ));
    }
    if r.klynt_coding.multi_cli_transfer_acc < 0.80 {
        failures.push(format!(
            "Q-5c: multi-CLI {:.2} < 0.80",
            r.klynt_coding.multi_cli_transfer_acc
        ));
    }

    if !failures.is_empty() {
        eprintln!("❌ Gate failures:\n{}", failures.join("\n"));
        return Err(common::KlyntbotError::Storage("KCA gates not met".into()));
    }
    Ok(())
}
