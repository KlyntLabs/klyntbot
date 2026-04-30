//! Generates docs/architecture/kca-game-changer.md combining bench results with
//! a static comparison matrix vs Graphiti / Mem0 / HippoRAG / GraphRAG / LightRAG / LangMem / Letta.

use crate::cost::CostDashboard;
use crate::klynt_coding::KlyntCodingReport;
use crate::latency::LatencyDashboard;
use crate::locobench::LoCoBenchReport;
use crate::longmembench::LongMemBenchReport;

pub struct GameChangerReport {
    pub lmb: LongMemBenchReport,
    pub locobench: LoCoBenchReport,
    pub klynt_coding: KlyntCodingReport,
    pub latency: LatencyDashboard,
    pub cost: CostDashboard,
}

impl GameChangerReport {
    pub fn render_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# KCA Game-Changer Report\n\n");
        s.push_str(&format!("Generated: {}\n\n", jiff::Timestamp::now()));

        let bench_limit = std::env::var("KCA_BENCH_LIMIT").unwrap_or_else(|_| "all".into());
        s.push_str(&format!(
            "*Run config: `KCA_BENCH_LIMIT={}`, model from `~/.klyntbot/config.json`, scoring = substring + 80% token-overlap recall.*\n\n",
            bench_limit
        ));

        s.push_str("## Quality\n\n");
        s.push_str("| Benchmark | Accuracy | Sample size |\n|---|---|---|\n");
        s.push_str(&format!(
            "| Long-memory | **{:.1}%** | {}/{} queries correct |\n",
            self.lmb.accuracy() * 100.0,
            self.lmb.correct,
            self.lmb.total_queries
        ));
        s.push_str(&format!(
            "| LoCoBench single-hop | **{:.1}%** | — |\n",
            self.locobench.single_hop_acc * 100.0
        ));
        s.push_str(&format!(
            "| LoCoBench multi-hop | **{:.1}%** | — |\n",
            self.locobench.multi_hop_acc * 100.0
        ));
        s.push_str(&format!(
            "| LoCoBench temporal | **{:.1}%** | — |\n",
            self.locobench.temporal_acc * 100.0
        ));
        let kc = &self.klynt_coding;
        s.push_str(&format!(
            "| Klynt-coding dead-end | **{:.1}%** | {}/{} |\n",
            kc.dead_end_recall * 100.0,
            kc.dead_end_counts.0,
            kc.dead_end_counts.1
        ));
        s.push_str(&format!(
            "| Klynt-coding fix-attempt | **{:.1}%** | {}/{} |\n",
            kc.fix_attempt_recall * 100.0,
            kc.fix_attempt_counts.0,
            kc.fix_attempt_counts.1
        ));
        s.push_str(&format!(
            "| Klynt-coding multi-CLI | **{:.1}%** | {}/{} |\n",
            kc.multi_cli_transfer_acc * 100.0,
            kc.multi_cli_counts.0,
            kc.multi_cli_counts.1
        ));

        if !self.lmb.by_hop_type.is_empty() {
            s.push_str("\n### Long-memory by hop type\n\n| Hop | Correct/Total | Accuracy |\n|---|---|---|\n");
            let mut keys: Vec<&String> = self.lmb.by_hop_type.keys().collect();
            keys.sort();
            for k in keys {
                let (c, t) = self.lmb.by_hop_type[k];
                let acc = if t == 0 { 0.0 } else { c as f64 / t as f64 };
                s.push_str(&format!("| {} | {}/{} | {:.1}% |\n", k, c, t, acc * 100.0));
            }
        }

        s.push_str("\n## Performance\n\n");
        s.push_str(&format!(
            "- Hot-path P50 turn latency: **{}ms**\n",
            self.latency.hot_path_p50_ms
        ));
        s.push_str(&format!(
            "- Hot-path P95 turn latency: **{}ms**\n",
            self.latency.hot_path_p95_ms
        ));
        s.push_str(&format!(
            "- Long-mem query P95: **{}ms** (real cloud LLM round-trip)\n",
            self.lmb.p95_query_latency_ms
        ));

        s.push_str("\n## Methodology\n\n");
        s.push_str(METHODOLOGY);

        s.push_str("\n## Comparison Matrix\n\n");
        s.push_str(COMPARISON_MATRIX);
        s.push_str("\n\n## Capabilities Klynt has that competitors lack\n\n");
        s.push_str(EXCLUSIVE_CAPABILITIES);
        s
    }

    pub fn write_to_file(&self, path: &std::path::Path) -> common::Result<()> {
        std::fs::write(path, self.render_markdown())
            .map_err(|e| common::KlyntbotError::Storage(format!("write report: {e}")))?;
        Ok(())
    }
}

const METHODOLOGY: &str = r#"
- **Fixtures**: `tests/fixtures/kca/{longmembench,locobench,klynt_coding_bench}.jsonl`. Each fixture contains a multi-turn conversation followed by gold-answer queries. Klynt-coding fixtures are stratified across three buckets (`kcb_dead_*` dead-ends, `kcb_fix_*` fix attempts, `kcb_xcli_*` cross-CLI patterns).
- **Pipeline**: each fixture instantiates a fresh `AppCore` over an ephemeral SQLite + LanceDB store. Conversation turns are replayed via `chat_complete` (drains the streaming reply). The replayer then waits for the cognitive extraction background task (3s batch window + LLM extraction call) to settle before queries fire — preventing race conditions between memory write and recall.
- **Scoring**: two-pass match. First, normalized substring (handles short factual answers like `"Anthropic"`). Second, token-overlap recall ≥80% over content tokens (handles narrative replies where the LLM phrases the answer in a sentence).
- **Memory layers exercised end-to-end**: FTS5 BM25 over semantic_facts + episodic_memories + procedural_rules; vector recall via Lance; PPR multi-hop expansion; conversation recall service. Reforge nightly cycle is *not* triggered during the bench (it requires real time progression).
- **Latencies** include the full agent loop: system prompt assembly, recall, LLM call, response streaming. Cloud LLM round-trip dominates wall time.

"#;

const COMPARISON_MATRIX: &str = r#"
| Capability | Klynt KCA | Graphiti | Mem0 v3 | HippoRAG-2 | GraphRAG | LightRAG | LangMem | Letta |
|---|---|---|---|---|---|---|---|---|
| Per-turn entity resolution (LLM) | ✅ | ✅ | ⚠ Embed only | ❌ | ❌ | ❌ | ❌ | ❌ |
| Bi-temporal validity | ✅ | ✅ | ⚠ Soft | ❌ | ❌ | ❌ | ❌ | ❌ |
| Edge invalidation on contradiction | ✅ Linker + temporal prune | ✅ | ⚠ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Procedural rules / skill learning | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Episodic memory with stability decay | ✅ FSRS-5 | ✅ | ⚠ | ❌ | ❌ | ❌ | ⚠ | ✅ |
| Causal vs correlational edge typing | ✅ Track 9-typing | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Hebbian co-activation | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Community detection (Louvain + LLM) | ✅ | ❌ | ❌ | ❌ | ✅ Leiden | ❌ | ❌ | ❌ |
| PPR retrieval | ✅ Track 6 | ❌ | ❌ | ✅ Best | ❌ | ❌ | ❌ | ❌ |
| Spaced repetition | ✅ FSRS-5 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Nightly synthesis cycle | ✅ Reforge 9-phase | ❌ | ❌ | ❌ | ⚠ Re-index | ❌ | ❌ | ❌ |
| Meta-cognition (Mirror) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Self-critique loop on extraction | ✅ Track 5 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Predictive cache warming | ✅ Track 7 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Hierarchical episodic compression | ✅ Track 8 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Cross-CLI cognitive transfer | ✅ Track 10 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Memory-grounded skill discovery | ✅ Track 12 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Coding-specific memory tier | ✅ Distiller + multi-CLI | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Backend complexity | SQLite + LanceDB only | Neo4j req | Neo4j optional | Custom | Pipeline-heavy | Custom | Vector store | Vector store |
"#;

const EXCLUSIVE_CAPABILITIES: &str = r#"
1. **Reforge nightly cycle** — 9-phase deferred synthesis no other system has.
2. **Mirror meta-cognition** — observes the agent's own behavior; foundation for self-improvement.
3. **Procedural rules** — observed → reflected → applied promotion path.
4. **Multi-CLI ingest + cross-CLI transfer** — patterns learned in one CLI propagate to others.
5. **FSRS-5 spaced repetition** — memory has a forgetting curve; review schedule trained from feedback.
6. **Skills system with progressive loading** — orchestrator layer above memory.
7. **Self-critique ring** — every extraction judged for hallucination before persisted as ground truth.
8. **Predictive cache warming** — anticipatory pre-computation of likely follow-up retrievals.
9. **Hierarchical episodic compression** — long-term memory navigable in O(log N) instead of O(N).
"#;
