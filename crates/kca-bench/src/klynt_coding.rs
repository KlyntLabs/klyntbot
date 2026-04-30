use kca_e2e::fixtures::{load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct KlyntCodingReport {
    pub dead_end_recall: f64,
    pub fix_attempt_recall: f64,
    pub multi_cli_transfer_acc: f64,
    /// (correct, total) counts per bucket — surfaced in the report so a 0/0
    /// bucket (no fixtures sampled) is distinguishable from 0/N (real misses).
    pub dead_end_counts: (u32, u32),
    pub fix_attempt_counts: (u32, u32),
    pub multi_cli_counts: (u32, u32),
}

/// Stratify fixtures across the three buckets (dead/fix/xcli) so a small
/// `KCA_BENCH_LIMIT` still covers all categories. Without this, a flat
/// `take(N)` only ever sees `kcb_dead_*` because the file groups them.
fn stratified(fixtures: &[ConversationFixture], limit: usize) -> Vec<&ConversationFixture> {
    fn prefix(id: &str) -> &str {
        match id {
            s if s.starts_with("kcb_dead") => "dead",
            s if s.starts_with("kcb_fix") => "fix",
            s if s.starts_with("kcb_xcli") => "xcli",
            _ => "other",
        }
    }
    let buckets = ["dead", "fix", "xcli"];
    let per_bucket = limit.div_ceil(buckets.len()).max(1);
    let mut out = Vec::new();
    for b in buckets {
        out.extend(
            fixtures
                .iter()
                .filter(|f| prefix(&f.id) == b)
                .take(per_bucket),
        );
    }
    out.truncate(limit.max(out.len()));
    out
}

pub async fn run_klynt_coding(path: &Path) -> common::Result<KlyntCodingReport> {
    let fixtures: Vec<ConversationFixture> = load_jsonl(path)?;
    let limit = std::env::var("KCA_BENCH_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(fixtures.len());
    let mut dead_end = (0u32, 0u32);
    let mut fix_attempt = (0u32, 0u32);
    let mut multi_cli = (0u32, 0u32);

    let sample = stratified(&fixtures, limit);
    for f in sample.iter() {
        let mut ctx = ReplayContext::new().await?;
        ctx.replay(f).await?;
        for q in &f.queries {
            let answer = ctx
                .chat_complete(
                    q.query.clone(),
                    common::SessionKey::from_parts("bench-kc", &f.id).to_string(),
                )
                .await?;
            let correct = super::longmembench::scoring::is_answer_correct(&answer, &q.gold_answer);
            let bucket = match f.id.as_str() {
                s if s.starts_with("kcb_dead") => &mut dead_end,
                s if s.starts_with("kcb_fix") => &mut fix_attempt,
                s if s.starts_with("kcb_xcli") => &mut multi_cli,
                _ => continue,
            };
            bucket.1 += 1;
            if correct {
                bucket.0 += 1;
            }
        }
    }
    Ok(KlyntCodingReport {
        dead_end_recall: ratio(dead_end),
        fix_attempt_recall: ratio(fix_attempt),
        multi_cli_transfer_acc: ratio(multi_cli),
        dead_end_counts: dead_end,
        fix_attempt_counts: fix_attempt,
        multi_cli_counts: multi_cli,
    })
}

fn ratio(t: (u32, u32)) -> f64 {
    if t.1 == 0 {
        0.0
    } else {
        t.0 as f64 / t.1 as f64
    }
}
