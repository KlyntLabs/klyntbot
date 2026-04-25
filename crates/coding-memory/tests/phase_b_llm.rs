use coding_memory::distiller::phase_b::{invoke_llm, LlmInvocation};
use coding_memory::distiller::record_observation::Observation;
use coding_memory::distiller::{TurnTrace, TurnTokenUsage};
use jiff::Timestamp;
use providers::{NoopProvider, ProviderManager};
use std::sync::Arc;
use std::time::Duration;

fn trace() -> TurnTrace {
    TurnTrace {
        session_id: "s".into(), turn_id: Some("t".into()),
        files_read: vec![], files_modified: vec![],
        commands_run: vec![], test_outcomes: vec![], errors_encountered: vec![],
        token_usage: Some(TurnTokenUsage { prompt: 1, completion: 1, cached: 0 }),
        started_at: Timestamp::now(), ended_at: None,
    }
}

#[tokio::test]
async fn noop_provider_returns_empty_observations_list() {
    let mgr = Arc::new(ProviderManager::new(Arc::new(NoopProvider), None, None));
    let inv = LlmInvocation {
        provider: mgr,
        model: "noop".into(),
        user_prompt_text: "hi",
        assistant_text: "done",
        trace: &trace(),
        repo_id: None,
        timeout: Duration::from_secs(1),
    };
    let result: Vec<Observation> = invoke_llm(inv).await.unwrap_or_default();
    assert!(result.is_empty(), "NoopProvider produces no observations");
}
