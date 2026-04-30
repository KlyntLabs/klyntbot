use kca_e2e::fixtures::{load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct LoCoBenchReport {
    pub single_hop_acc: f64,
    pub multi_hop_acc: f64,
    pub temporal_acc: f64,
    pub open_acc: f64,
    /// Per-hop sample sizes (single, multi, temporal, open). Used by the
    /// gate enforcer to skip categories with zero samples — otherwise a
    /// fixture that happens to have no temporal queries would fail the
    /// Q-4 gate at 0% accuracy with 0 samples, which is a false negative.
    pub single_hop_total: u32,
    pub multi_hop_total: u32,
    pub temporal_total: u32,
    pub open_total: u32,
}

pub async fn run_locobench(path: &Path) -> common::Result<LoCoBenchReport> {
    let fixtures: Vec<ConversationFixture> = load_jsonl(path)?;
    let limit = std::env::var("KCA_BENCH_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(fixtures.len());
    let debug = std::env::var("KCA_BENCH_DEBUG").is_ok();
    let mut by_hop: std::collections::HashMap<String, (u32, u32)> = Default::default();
    for f in fixtures.iter().take(limit) {
        let mut ctx = ReplayContext::new().await?;
        ctx.replay(f).await?;
        if debug {
            let facts = ctx.dump_facts().await;
            eprintln!("[DEBUG] loco fixture {} stored {} facts:", f.id, facts.len());
            for (s, p, o) in &facts {
                eprintln!("  {s} | {p} | {o}");
            }
        }
        for q in &f.queries {
            let answer = ctx
                .chat_complete(
                    q.query.clone(),
                    common::SessionKey::from_parts("bench-loco", &f.id).to_string(),
                )
                .await?;
            let correct = super::longmembench::scoring::is_answer_correct(&answer, &q.gold_answer);
            if debug {
                eprintln!(
                    "[DEBUG] loco q={:?} hop={:?} gold={:?} answer={:?} ok={}",
                    q.query, q.hop_type, q.gold_answer, answer, correct
                );
            }
            let entry = by_hop.entry(q.hop_type.clone()).or_insert((0, 0));
            entry.1 += 1;
            if correct {
                entry.0 += 1;
            }
        }
    }
    let acc = |k: &str| {
        by_hop
            .get(k)
            .map(|(c, t)| if *t == 0 { 0.0 } else { *c as f64 / *t as f64 })
            .unwrap_or(0.0)
    };
    let total = |k: &str| by_hop.get(k).map(|(_, t)| *t).unwrap_or(0);
    Ok(LoCoBenchReport {
        single_hop_acc: acc("single"),
        multi_hop_acc: acc("multi"),
        temporal_acc: acc("temporal"),
        open_acc: acc("open"),
        single_hop_total: total("single"),
        multi_hop_total: total("multi"),
        temporal_total: total("temporal"),
        open_total: total("open"),
    })
}
