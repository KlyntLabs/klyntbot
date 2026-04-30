use kca_e2e::fixtures::{load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct LoCoBenchReport {
    pub single_hop_acc: f64,
    pub multi_hop_acc: f64,
    pub temporal_acc: f64,
    pub open_acc: f64,
}

pub async fn run_locobench(path: &Path) -> common::Result<LoCoBenchReport> {
    let fixtures: Vec<ConversationFixture> = load_jsonl(path)?;
    let limit = std::env::var("KCA_BENCH_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(fixtures.len());
    let mut by_hop: std::collections::HashMap<String, (u32, u32)> = Default::default();
    for f in fixtures.iter().take(limit) {
        let mut ctx = ReplayContext::new().await?;
        ctx.replay(f).await?;
        for q in &f.queries {
            let answer = ctx
                .chat_complete(
                    q.query.clone(),
                    common::SessionKey::from_parts("bench-loco", &f.id).to_string(),
                )
                .await?;
            let correct = super::longmembench::scoring::is_answer_correct(&answer, &q.gold_answer);
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
    Ok(LoCoBenchReport {
        single_hop_acc: acc("single"),
        multi_hop_acc: acc("multi"),
        temporal_acc: acc("temporal"),
        open_acc: acc("open"),
    })
}
