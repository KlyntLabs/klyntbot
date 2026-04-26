//! Smoke test for Phase-1 public surface — every type is constructable and
//! compiles through the paths downstream phases will use. Runs no business
//! logic. Exists so a rename in later phases trips CI instead of silently
//! breaking the architecture skeleton.

use coding_memory::distiller::{Distiller, TurnTrace};
use coding_memory::error::NotImplementedInPhase;
use coding_memory::facts::{CodingKind, FixAttempt, FixOutcome, StyleScope};
use coding_memory::mcp::CODING_MEMORY_MCP_TOOLS;
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};
use coding_memory::recall::{
    ChangeHistoryResponse, CodingRecallService, DecisionPointsResponse, FactsAsOfResponse,
    IndexEntry, RecallQuery,
};
use coding_memory::reforge_phase::{
    CodingSynthesisPhase, RuleArtifact, RuleArtifactGenerationPhase,
};
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, QueryRewriter, RetrievalSkill,
};
use coding_memory::scope::{AnchoredSymbol, CausalEdgeKind, ProvenanceKind, Sensitivity};
use coding_memory::sink::{InProcessSink, MemorySink};
use coding_memory::skills::{ProjectSkillLocation, SkillId, SkillScope};
use coding_memory::{RecallInvocationRow, RetrievalSkillRegistry};

#[test]
fn phase1_types_are_constructable() {
    let _ = NotImplementedInPhase::new(4);
    let _ = CodingKind::FixAttempt;
    let _ = FixOutcome::Abandoned;
    let _ = StyleScope::Global;
    let _ = ProvenanceKind::DistillerExtractive;
    let _ = Sensitivity::default();
    let _ = CausalEdgeKind::Broke;
    let _ = BudgetTier::DeepThink;
    let _ = ProjectSkillLocation::Private;
    let _ = SkillScope::Global;
    let _ = SkillId("x".into());
    let _ = RuleArtifact::ClaudeMd.relative_path();
}

#[test]
fn phase1_mcp_tool_constant_is_populated() {
    assert_eq!(CODING_MEMORY_MCP_TOOLS.len(), 8);
    for t in CODING_MEMORY_MCP_TOOLS {
        assert!(!t.is_empty());
    }
}

#[tokio::test]
async fn phase1_stub_services_return_not_implemented() {
    // Distiller is no longer a stub — real construction tested in turn_boundary.rs.
    let _: Option<Distiller> = None;

    let sink = InProcessSink::new();
    // Sink without a distiller wired is a no-op — returns Ok.
    assert!(sink.accept_event(dummy_event()).await.is_ok());
    assert!(sink.flush().await.is_ok());

    // CodingRecallService is no longer a stub — verify type exists only.
    let _: Option<CodingRecallService> = None;
    // Verify query structs compile.
    let _: Option<RecallQuery> = None;

    let phase = CodingSynthesisPhase::default();
    assert!(coding_memory::reforge_phase::ReforgePhaseRun::run(&phase)
        .await
        .is_err());
    let phase = RuleArtifactGenerationPhase::default();
    assert!(coding_memory::reforge_phase::ReforgePhaseRun::run(&phase)
        .await
        .is_err());

    // QueryRewriter is no longer a unit struct — construct with dummy retrieve fn.
    let skill = QueryRewriter::new(std::sync::Arc::new(|_q| {
        Box::pin(async { Ok((vec![], vec![])) })
    }));
    let ctx = EscalationContext {
        query: "q".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    // apply returns Result<EscalationOutcome> — just verify it compiles + runs.
    let out = skill.apply(&ctx).await;
    assert!(out.is_ok());

    // Turn trace exists as a type (not returned — just referenced).
    let _: Option<TurnTrace> = None;
    let _ = dummy_fix_attempt();
    let _: Option<IndexEntry> = None;
    let _: Option<AnchoredSymbol> = None;
}

#[test]
fn phase4_recall_surface_is_constructable() {
    // Response DTOs — surface-only assertion.
    let _: Option<FactsAsOfResponse> = None;
    let _: Option<ChangeHistoryResponse> = None;
    let _: Option<DecisionPointsResponse> = None;
    let _: Option<RecallInvocationRow> = None;

    // Budgeter trait + heuristic impl callable.
    let budgeter = HeuristicBudgeter;
    assert!(<HeuristicBudgeter as TokenBudgeter>::count(&budgeter, "abcd") >= 1);

    // Registry constructable (with empty skill set + ambient bus).
    let bus = std::sync::Arc::new(bus::DomainEventBus::new(8));
    let _ = RetrievalSkillRegistry::new(vec![], bus);

    // Top-level facade re-export of the recall service is wired.
    let _: Option<CodingRecallService> = None;

    // EscalationOutcome is a value type, not just a comment.
    use coding_memory::retrieval_skills::EscalationOutcome;
    let _ = EscalationOutcome {
        succeeded: false,
        coverage_after: 0.0,
        added_context: String::new(),
        added_ids: vec![],
    };
}

fn dummy_event() -> coding_ingest::AgentEvent {
    use coding_ingest::{AgentEvent, AgentEventV1, AgentSource, EventKind};
    use jiff::Timestamp;
    use std::path::PathBuf;
    use uuid::Uuid;

    AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::KlyntCli,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/"),
        repo: None,
        occurred_at: Timestamp::from_second(0).unwrap(),
        kind: EventKind::SessionStart {
            model: None,
            source_reason: "test".into(),
        },
    })
}

fn dummy_fix_attempt() -> FixAttempt {
    use coding_memory::scope::ProvenanceMetadata;
    use jiff::Timestamp;
    use uuid::Uuid;

    FixAttempt {
        problem_hash: "h".into(),
        problem: "p".into(),
        files: vec![],
        approach: "a".into(),
        outcome: FixOutcome::Success,
        insight: None,
        duration_ms: 0,
        test_before: None,
        test_after: None,
        anchored_symbols: vec![],
        provenance: ProvenanceMetadata {
            source_events: vec![Uuid::nil()],
            session_id: "s".into(),
            turn_id: None,
            distilled_at: Timestamp::from_second(0).unwrap(),
            distiller_model: "m".into(),
            source_kind: ProvenanceKind::DistillerExtractive,
        },
        sensitivity: Sensitivity::default(),
    }
}
