//! `CodingRecallService` — single facade for passive injection + MCP tools.

use crate::recall::{
    budget::TokenBudgeter,
    change_history::ChangeHistoryService,
    dead_end::DeadEndChecker,
    decision_points::DecisionPointsService,
    facts_as_of::FactsAsOfService,
    fetch_builder::FetchBuilder,
    index_builder::IndexBuilder,
    probe::{ProbeVerdict, RetrievalQualityProbe},
    telemetry::{RecallInvocationRepo, RecallInvocationRow},
    timeline_builder::{TimelineBuilder, TimelineInput},
    ChangeHistoryResponse, DeadEndResponse, DecisionPointsResponse, FactsAsOfResponse, FullEntry,
    IndexEntry, RecallIndexResponse, RecallQuery, TimelineEntry,
};
use crate::retrieval_skills::RetrievalSkillRegistry;
use cognitive::UnifiedMemoryService;
use jiff::Timestamp;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Construction-time wiring.
pub struct CodingRecallServiceConfig {
    /// Coverage threshold below which the C3 selector dispatches.
    pub probe_threshold: f32,
    /// Default top-k for `recall_index`.
    pub default_limit: u32,
}

impl Default for CodingRecallServiceConfig {
    fn default() -> Self {
        Self {
            probe_threshold: 0.3,
            default_limit: 10,
        }
    }
}

/// Single facade.
pub struct CodingRecallService {
    config: CodingRecallServiceConfig,
    ums: Arc<UnifiedMemoryService>,
    #[allow(dead_code)]
    fact_repo: Arc<cognitive::SemanticFactRepo>,
    #[allow(dead_code)]
    ep_repo: Arc<cognitive::EpisodicMemoryRepo>,
    telemetry: RecallInvocationRepo,
    index_builder: IndexBuilder,
    timeline_builder: TimelineBuilder,
    fetch_builder: FetchBuilder,
    facts_as_of: FactsAsOfService,
    change_history: ChangeHistoryService,
    decision_points: DecisionPointsService,
    dead_end: DeadEndChecker,
    probe: RetrievalQualityProbe,
    skills: Option<Arc<RetrievalSkillRegistry>>,
    #[allow(dead_code)]
    budgeter: Arc<dyn TokenBudgeter>,
    causal_repo: Option<Arc<crate::causal::CausalEdgeRepo>>,
    query_pipeline: Option<Arc<context_engine::QueryPipeline>>,
}

impl std::fmt::Debug for CodingRecallService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingRecallService").finish()
    }
}

impl CodingRecallService {
    /// Construct from raw repos + `UnifiedMemoryService`.
    pub fn new(
        config: CodingRecallServiceConfig,
        ums: Arc<UnifiedMemoryService>,
        fact_repo: Arc<cognitive::SemanticFactRepo>,
        ep_repo: Arc<cognitive::EpisodicMemoryRepo>,
        telemetry: RecallInvocationRepo,
        budgeter: Arc<dyn TokenBudgeter>,
    ) -> Self {
        Self {
            probe: RetrievalQualityProbe::new(config.probe_threshold),
            index_builder: IndexBuilder::with_budgeter(budgeter.clone()),
            timeline_builder: TimelineBuilder::new(),
            fetch_builder: FetchBuilder::new(fact_repo.clone(), ep_repo.clone()),
            facts_as_of: FactsAsOfService::new(fact_repo.clone()),
            change_history: ChangeHistoryService::new(fact_repo.clone()),
            decision_points: DecisionPointsService::new(ep_repo.clone()),
            dead_end: DeadEndChecker::new(fact_repo.clone(), Default::default()),
            ums,
            fact_repo,
            ep_repo,
            telemetry,
            skills: None,
            budgeter,
            config,
            causal_repo: None,
            query_pipeline: None,
        }
    }

    /// Attach the C3 skill registry post-construction (avoids Arc cycles in builders).
    #[must_use]
    pub fn with_skills(mut self, skills: Arc<RetrievalSkillRegistry>) -> Self {
        self.skills = Some(skills);
        self
    }

    /// Attach the causal repo (Phase-6 wiring).
    #[must_use]
    pub fn with_causal_repo(mut self, repo: Arc<crate::causal::CausalEdgeRepo>) -> Self {
        self.causal_repo = Some(repo);
        self
    }

    /// Attach a query enhancement pipeline (Phase-4 wiring).
    #[must_use]
    pub fn with_query_pipeline(mut self, pipeline: Arc<context_engine::QueryPipeline>) -> Self {
        self.query_pipeline = Some(pipeline);
        self
    }

    /// Access the optional query pipeline.
    pub fn query_pipeline(&self) -> &Option<Arc<context_engine::QueryPipeline>> {
        &self.query_pipeline
    }

    /// Layer-1 — compact index with C3 escalation.
    pub async fn recall_index(
        &self,
        query: &str,
        repo: Option<&str>,
        kinds: Option<&[&str]>,
        days: Option<u32>,
        limit: u32,
    ) -> common::Result<RecallIndexResponse> {
        let started = Instant::now();
        let limit = if limit == 0 {
            self.config.default_limit
        } else {
            limit
        };
        let scope = repo
            .map(|r| {
                if r == "*" {
                    vec![("repo".to_string(), Some("*".to_string()))]
                } else {
                    vec![("repo".to_string(), Some(r.to_string()))]
                }
            })
            .unwrap_or_default();
        let scored = self
            .ums
            .retrieve_with_overrides(query, limit as usize, 0.0, default_weights())
            .await?;
        let entries: Vec<IndexEntry> = scored
            .iter()
            .map(|s| self.index_builder.from_scored_fact(s))
            .filter(|e| match &kinds {
                Some(allowed) if !allowed.is_empty() => allowed.iter().any(|k| k == &e.kind),
                _ => true,
            })
            .filter(|e| match days {
                Some(d) => {
                    let cutoff = jiff::Timestamp::now()
                        .checked_sub(jiff::ToSpan::days(d as i64))
                        .unwrap_or_else(|_| jiff::Timestamp::MIN);
                    e.when >= cutoff
                }
                None => true,
            })
            .collect();
        let sims: Vec<f32> = scored.iter().map(|s| s.score as f32).collect();
        let mut coverage = self.probe.score(&sims);
        let verdict = self.probe.verdict(&sims);

        // Optional escalation.
        let mut skill_used: Option<String> = None;
        if verdict == ProbeVerdict::Escalate {
            if let Some(skills) = &self.skills {
                let ctx = crate::retrieval_skills::EscalationContext {
                    query: query.to_string(),
                    coverage_score: coverage,
                    budget_tier: crate::retrieval_skills::BudgetTier::DeepThink,
                    repo: repo.map(|s| s.to_string()),
                };
                let res = skills.escalate(&ctx).await?;
                coverage = res.final_outcome.coverage_after;
                if !res.skills_tried.is_empty() {
                    skill_used = Some(res.skills_tried.join(","));
                }
            }
        }

        let result_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
        let row = RecallInvocationRow {
            id: Uuid::new_v4(),
            occurred_at: Timestamp::now(),
            session_id: None,
            turn_id: None,
            repo_id: repo.map(|s| s.to_string()),
            layer: "index".into(),
            query: query.to_string(),
            coverage_score: Some(coverage),
            skill_used,
            latency_ms: started.elapsed().as_millis() as i64,
            result_ids,
            rendered_tokens: None,
            metadata: serde_json::json!({"scope": scope}),
        };
        self.telemetry.insert(&row).await?;

        Ok(RecallIndexResponse {
            results: entries,
            coverage_score: coverage,
            escalation_available: verdict == ProbeVerdict::Escalate && self.skills.is_some(),
        })
    }

    /// Layer-2 — chronological framing.
    pub async fn recall_timeline(
        &self,
        ids_or_query: RecallQuery,
        repo: Option<&str>,
        days: u32,
    ) -> common::Result<Vec<TimelineEntry>> {
        let started = Instant::now();
        let inputs: Vec<TimelineInput> = match ids_or_query {
            RecallQuery::Ids(ids) => {
                let entries = self.fetch_builder.fetch(&ids, false, false).await?;
                entries
                    .into_iter()
                    .map(|e| TimelineInput {
                        id: e.id,
                        kind: e.kind,
                        when: e
                            .metadata
                            .get("recorded_at")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(Timestamp::now),
                        snippet: e.content.to_string(),
                        related_ids: vec![],
                    })
                    .collect()
            }
            RecallQuery::Text(q) => {
                let scored = self
                    .ums
                    .retrieve_with_overrides(&q, 25, 0.0, default_weights())
                    .await?;
                let _ = days;
                let _ = repo;
                scored
                    .into_iter()
                    .map(|s| TimelineInput {
                        id: s.fact.id.parse().unwrap_or_else(|_| Uuid::nil()),
                        kind: s.fact.memory_type.clone(),
                        when: s
                            .fact
                            .recorded_at
                            .parse()
                            .unwrap_or_else(|_| Timestamp::now()),
                        snippet: format!(
                            "{} {} {}",
                            s.fact.subject, s.fact.predicate, s.fact.object
                        ),
                        related_ids: vec![],
                    })
                    .collect()
            }
        };
        let entries = self.timeline_builder.build(inputs);
        let result_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
        self.telemetry
            .insert(&RecallInvocationRow {
                id: Uuid::new_v4(),
                occurred_at: Timestamp::now(),
                session_id: None,
                turn_id: None,
                repo_id: repo.map(|s| s.to_string()),
                layer: "timeline".into(),
                query: String::new(),
                coverage_score: None,
                skill_used: None,
                latency_ms: started.elapsed().as_millis() as i64,
                result_ids,
                rendered_tokens: None,
                metadata: serde_json::json!({"days": days}),
            })
            .await?;
        Ok(entries)
    }

    /// Layer-3 — full fetch.
    pub async fn recall_fetch(
        &self,
        ids: &[String],
        include_provenance: bool,
        include_causal_graph: bool,
    ) -> common::Result<Vec<FullEntry>> {
        self.fetch_builder
            .fetch(ids, include_provenance, include_causal_graph)
            .await
    }

    /// Counterfactual check.
    pub async fn check_dead_ends(
        &self,
        approach: &str,
        repo: Option<&str>,
    ) -> common::Result<DeadEndResponse> {
        self.dead_end.check(approach, repo).await
    }

    /// `recall_facts_as_of`.
    pub async fn recall_facts_as_of(
        &self,
        subject: &str,
        predicate: &str,
        as_of: Timestamp,
    ) -> common::Result<FactsAsOfResponse> {
        self.facts_as_of.query(subject, predicate, as_of).await
    }

    /// `recall_change_history`.
    pub async fn recall_change_history(
        &self,
        subject: &str,
        predicate: &str,
        repo: Option<&str>,
    ) -> common::Result<ChangeHistoryResponse> {
        self.change_history.query(subject, predicate, repo).await
    }

    /// `recall_decision_points`.
    pub async fn recall_decision_points(
        &self,
        domain: Option<&str>,
        repo: Option<&str>,
        limit: i64,
    ) -> common::Result<DecisionPointsResponse> {
        self.decision_points.list(domain, repo, limit).await
    }

    /// List open (unfinished) turn traces for the given repo and time window.
    pub async fn open_threads(
        &self,
        repo: Option<&str>,
        since_days: u32,
        limit: usize,
    ) -> common::Result<Vec<crate::recall::open_threads::OpenThread>> {
        crate::recall::open_threads::list_open_threads(&self.ep_repo, repo, since_days, limit).await
    }

    /// Walk the causal graph from `subject` up to `depth` levels.
    pub async fn trace_causes(
        &self,
        subject: Uuid,
        _repo: Option<&str>,
        depth: u32,
    ) -> common::Result<crate::recall::CausalTraceResponse> {
        let Some(repo) = &self.causal_repo else {
            return Err(common::KlyntbotError::Storage(
                "trace_causes requires causal repo wiring".into(),
            ));
        };
        crate::recall::causal_walker::CausalWalker::new(repo.clone())
            .walk(subject, depth)
            .await
    }

    /// Test seam — read telemetry rows.
    pub fn telemetry_repo(&self) -> &RecallInvocationRepo {
        &self.telemetry
    }
}

/// Default 12-axis relevance weight vector used by the recall service. Public
/// so registry-wiring sites in `app-core` can reuse the same weights.
pub fn default_weights() -> [f64; 12] {
    // Modestly bias for semantic + recency + path coherence; train in Phase 6.
    [
        0.35, 0.05, 0.10, 0.05, 0.05, 0.20, 0.05, 0.05, 0.02, 0.02, 0.05, 0.01,
    ]
}
