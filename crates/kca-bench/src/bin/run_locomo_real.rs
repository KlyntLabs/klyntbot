//! Run the REAL LoCoMo benchmark against our memory pipeline.
//!
//! Usage:
//!   OPENAI_API_KEY=sk-… \
//!   KCA_LOCOMO_LIMIT=1 KCA_LOCOMO_QA_LIMIT=10 \
//!   cargo run --release -p kca-bench --bin run-locomo-real
//!
//! Default behavior: load `tests/fixtures/kca/locomo10_real.json` (10
//! conversations), replay every session through our memory pipeline,
//! ask all ~199 QA per conversation, grade each via OpenAI gpt-4.1.

use kca_bench::locomo_real::run_locomo_real;
use kca_e2e::fixtures::fixtures_root;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> common::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,kca_bench=info")),
        )
        .init();

    let path = fixtures_root().join("locomo10_real.json");
    eprintln!("→ Loading {}", path.display());
    let report = run_locomo_real(&path).await?;

    println!("\n=== Real LoCoMo result ===");
    println!("conversations replayed: {}", report.conversations_replayed);
    println!("total QA           : {}", report.total);
    println!(
        "  CORRECT          : {} ({:.1}%)",
        report.correct,
        report.accuracy() * 100.0
    );
    println!(
        "  INCORRECT        : {} ({:.1}%)",
        report.incorrect,
        report.incorrect as f64 / report.total.max(1) as f64 * 100.0
    );
    println!(
        "  NOT_ATTEMPTED    : {} ({:.1}%)",
        report.not_attempted,
        report.not_attempted as f64 / report.total.max(1) as f64 * 100.0
    );
    println!(
        "attempted accuracy : {:.1}% (CORRECT / (CORRECT+INCORRECT))",
        report.attempted_accuracy() * 100.0
    );
    println!(
        "p50 / p95 query latency: {} / {} ms",
        report.p50_query_latency_ms, report.p95_query_latency_ms
    );

    println!("\nBy category (1=single-hop, 2=multi-hop/temporal, 3=open-domain, 4=adversarial, 5=abstention):");
    let mut cats: Vec<_> = report.by_category.iter().collect();
    cats.sort_by_key(|(k, _)| **k);
    for (cat, (correct, total)) in cats {
        let acc = if *total > 0 { *correct as f64 / *total as f64 * 100.0 } else { 0.0 };
        println!("  cat {cat}: {correct}/{total} = {acc:.1}%");
    }

    println!("\nReference numbers (Letta blog 2025):");
    println!("  Letta + gpt-4o-mini : 74.0%");
    println!("  Mem0 graph variant  : 68.5%");

    Ok(())
}
