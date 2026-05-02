//! Trace analyzer for KCA bench JSONL output.
//!
//! Single-file:  `analyze-trace path/to/trace-X.jsonl`
//!   Reports failure-mode counts (zero-retrieval / retrieved-but-refused /
//!   retry-failed) and per-category breakdowns.
//!
//! Two-file diff: `analyze-trace baseline.jsonl phase-N.jsonl`
//!   Joins on (conv_id, qa_index). Prints A↔C grade transitions and which
//!   mechanism fields differ between runs. Useful to attribute lift.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use kca_bench::trace::QaTraceEvent;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.len() {
        2 => single_file(&args[1]),
        3 => diff_files(&args[1], &args[2]),
        _ => {
            eprintln!("usage: analyze-trace <trace.jsonl> [<phase-N.jsonl>]");
            std::process::exit(1);
        }
    }
}

fn read_events(path: &str) -> std::io::Result<Vec<QaTraceEvent>> {
    let f = File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<QaTraceEvent>(&line) {
            Ok(ev) => out.push(ev),
            Err(e) => eprintln!("[skip] parse error: {e}"),
        }
    }
    Ok(out)
}

fn single_file(path: &str) -> std::io::Result<()> {
    let events = read_events(path)?;
    let total = events.len();
    let a = events.iter().filter(|e| e.grade == 'A').count();
    let b = events.iter().filter(|e| e.grade == 'B').count();
    let c = events.iter().filter(|e| e.grade == 'C').count();

    println!("=== {} ===", path);
    println!("total: {total}");
    println!(
        "  A: {a} ({:.1}%)  B: {b} ({:.1}%)  C: {c} ({:.1}%)",
        pct(a, total),
        pct(b, total),
        pct(c, total)
    );

    // Failure-mode breakdown for C-graded questions
    let c_events: Vec<&QaTraceEvent> = events.iter().filter(|e| e.grade == 'C').collect();
    let zero_retrieval = c_events
        .iter()
        .filter(|e| e.fts_hits == 0 && e.vector_hits == 0 && e.episodic_hits == 0)
        .count();
    let retrieved_but_refused = c_events
        .iter()
        .filter(|e| (e.fts_hits > 0 || e.vector_hits > 0) && e.predicted_was_refusal)
        .count();
    let retry_fired_failed = c_events.iter().filter(|e| e.retry_fired).count();

    println!("\nC-grade failure modes:");
    println!(
        "  zero-retrieval (blind refusal): {zero_retrieval} ({:.1}% of C)",
        pct(zero_retrieval, c_events.len())
    );
    println!(
        "  retrieved-but-refused          : {retrieved_but_refused} ({:.1}% of C)",
        pct(retrieved_but_refused, c_events.len())
    );
    println!(
        "  retry-fired-still-failed       : {retry_fired_failed} ({:.1}% of C)",
        pct(retry_fired_failed, c_events.len())
    );

    // Per-category breakdown
    println!("\nPer-category accuracy:");
    let mut by_cat: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
    for e in &events {
        let entry = by_cat.entry(e.category).or_insert((0, 0));
        entry.1 += 1;
        if e.grade == 'A' {
            entry.0 += 1;
        }
    }
    for (cat, (correct, total)) in &by_cat {
        let acc = if *total > 0 {
            *correct as f64 / *total as f64 * 100.0
        } else {
            0.0
        };
        println!("  cat {cat}: {correct}/{total} = {acc:.1}%");
    }

    // Mechanism stats (when phases are on)
    let any_entities = events
        .iter()
        .filter(|e| !e.entities_extracted.is_empty())
        .count();
    let any_reorganize = events.iter().filter(|e| e.reorganize_fired).count();
    let any_retry = events.iter().filter(|e| e.retry_fired).count();
    println!("\nMechanism activity:");
    println!(
        "  entities extracted on   : {any_entities} ({:.1}%)",
        pct(any_entities, total)
    );
    println!(
        "  reorganize fired before : {any_reorganize} ({:.1}%)",
        pct(any_reorganize, total)
    );
    println!(
        "  retry fired             : {any_retry} ({:.1}%)",
        pct(any_retry, total)
    );

    // Latency
    let mut latencies: Vec<u64> = events.iter().map(|e| e.qa_latency_ms).collect();
    latencies.sort();
    if !latencies.is_empty() {
        let p50 = latencies[latencies.len() / 2];
        let p95 = latencies[((latencies.len() as f64) * 0.95) as usize];
        println!("\nLatency: p50={p50}ms p95={p95}ms");
    }

    Ok(())
}

fn diff_files(baseline: &str, candidate: &str) -> std::io::Result<()> {
    let base = read_events(baseline)?;
    let cand = read_events(candidate)?;
    let base_map: BTreeMap<(String, u32), &QaTraceEvent> = base
        .iter()
        .map(|e| ((e.conv_id.clone(), e.qa_index), e))
        .collect();

    let mut a_to_c = 0;
    let mut c_to_a = 0;
    let mut b_to_a = 0;
    let mut a_to_b = 0;
    let mut transitions: Vec<(char, char, &QaTraceEvent, &QaTraceEvent)> = Vec::new();

    for c_ev in &cand {
        let key = (c_ev.conv_id.clone(), c_ev.qa_index);
        let Some(&b_ev) = base_map.get(&key) else { continue };
        if b_ev.grade != c_ev.grade {
            transitions.push((b_ev.grade, c_ev.grade, b_ev, c_ev));
            match (b_ev.grade, c_ev.grade) {
                ('A', 'C') => a_to_c += 1,
                ('C', 'A') => c_to_a += 1,
                ('B', 'A') => b_to_a += 1,
                ('A', 'B') => a_to_b += 1,
                _ => {}
            }
        }
    }

    println!("=== diff: {baseline} → {candidate} ===");
    println!("matched QA pairs: {}", cand.len().min(base.len()));
    println!(
        "C→A (gained): {c_to_a}  A→C (lost): {a_to_c}  B→A: {b_to_a}  A→B: {a_to_b}"
    );

    // Sample 5 wins and 5 losses
    println!("\nSample C→A transitions (mechanism that changed):");
    let mut shown = 0;
    for (b_grade, c_grade, b_ev, c_ev) in &transitions {
        if *b_grade == 'C' && *c_grade == 'A' && shown < 5 {
            println!(
                "  q={:?}\n    base: vec={} fts={} epi={} entities={} reorg={} retry={}",
                b_ev.question.chars().take(60).collect::<String>(),
                b_ev.vector_hits,
                b_ev.fts_hits,
                b_ev.episodic_hits,
                b_ev.entities_extracted.len(),
                b_ev.reorganize_fired,
                b_ev.retry_fired,
            );
            println!(
                "    cand: vec={} fts={} epi={} entities={} reorg={} retry={}",
                c_ev.vector_hits,
                c_ev.fts_hits,
                c_ev.episodic_hits,
                c_ev.entities_extracted.len(),
                c_ev.reorganize_fired,
                c_ev.retry_fired,
            );
            shown += 1;
        }
    }

    if a_to_c > 0 {
        println!("\nSample A→C regressions (worth investigating):");
        let mut shown = 0;
        for (b_grade, c_grade, b_ev, c_ev) in &transitions {
            if *b_grade == 'A' && *c_grade == 'C' && shown < 5 {
                println!(
                    "  q={:?}\n    pred(base)={:?}\n    pred(cand)={:?}",
                    b_ev.question.chars().take(60).collect::<String>(),
                    b_ev.predicted.chars().take(60).collect::<String>(),
                    c_ev.predicted.chars().take(60).collect::<String>(),
                );
                shown += 1;
            }
        }
    }

    Ok(())
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64 * 100.0
    }
}
