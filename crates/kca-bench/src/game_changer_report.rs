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

        s.push_str("## Quality\n\n");
        s.push_str(&format!(
            "- Long-memory accuracy: **{:.1}%**\n",
            self.lmb.accuracy() * 100.0
        ));
        s.push_str(&format!(
            "- LoCoBench single-hop: **{:.1}%**\n",
            self.locobench.single_hop_acc * 100.0
        ));
        s.push_str(&format!(
            "- LoCoBench multi-hop: **{:.1}%**\n",
            self.locobench.multi_hop_acc * 100.0
        ));
        s.push_str(&format!(
            "- LoCoBench temporal: **{:.1}%**\n",
            self.locobench.temporal_acc * 100.0
        ));
        s.push_str(&format!(
            "- Klynt-coding dead-end: **{:.1}%**\n",
            self.klynt_coding.dead_end_recall * 100.0
        ));
        s.push_str(&format!(
            "- Klynt-coding fix-attempt: **{:.1}%**\n",
            self.klynt_coding.fix_attempt_recall * 100.0
        ));
        s.push_str(&format!(
            "- Klynt-coding multi-CLI: **{:.1}%**\n\n",
            self.klynt_coding.multi_cli_transfer_acc * 100.0
        ));

        s.push_str("## Performance\n\n");
        s.push_str(&format!(
            "- Hot-path P95: **{}ms**\n",
            self.latency.hot_path_p95_ms
        ));
        s.push_str(&format!(
            "- Retrieval P95: **{}ms**\n",
            self.latency.retrieval_p95_ms
        ));
        s.push_str(&format!(
            "- Hot-path cost: **${:.4}/turn**\n\n",
            self.cost.hot_path_usd_per_turn
        ));

        s.push_str("## Comparison Matrix\n\n");
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
