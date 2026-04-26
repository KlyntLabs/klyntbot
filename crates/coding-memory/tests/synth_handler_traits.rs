use coding_memory::reforge::types::RepoArtifactPlan;
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
    RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler,
};
use jiff::Timestamp;
use std::path::PathBuf;

struct MockSynth;
#[async_trait::async_trait]
impl CodingSynthesisHandler for MockSynth {
    async fn synthesize_coding(
        &self,
        _input: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        Ok(CodingSynthesisOutput {
            actions: vec![PromoteAction::ExtractPattern {
                repo_id: Some("r1".into()),
                rule: "test".into(),
                confidence: 0.8,
                supporting: vec![],
            }],
            narrative: "ok".into(),
        })
    }
}

struct MockRules;
#[async_trait::async_trait]
impl RuleArtifactsHandler for MockRules {
    async fn synthesize_artifact(
        &self,
        _input: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        Ok(RuleArtifactOutput {
            body: "## Notes\n- test\n".into(),
            section_labels: vec!["Notes".into()],
        })
    }
}

#[tokio::test]
async fn synth_handler_object_safe() {
    let h: Box<dyn CodingSynthesisHandler> = Box::new(MockSynth);
    let out = h
        .synthesize_coding(&CodingSynthesisInput {
            since: Timestamp::now(),
            repo_bundles: vec![],
            recent_counterfactuals: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.actions.len(), 1);
}

#[tokio::test]
async fn rules_handler_object_safe() {
    let h: Box<dyn RuleArtifactsHandler> = Box::new(MockRules);
    let out = h
        .synthesize_artifact(&RuleArtifactInput {
            plan: RepoArtifactPlan {
                repo_id: "r1".into(),
                root: PathBuf::from("/tmp/r1"),
                enabled: vec![coding_memory::reforge_phase::RuleArtifact::ClaudeMd],
                facts: vec![],
                rules: vec![],
            },
            artifact: coding_memory::reforge_phase::RuleArtifact::ClaudeMd,
        })
        .await
        .unwrap();
    assert!(out.body.contains("Notes"));
}
