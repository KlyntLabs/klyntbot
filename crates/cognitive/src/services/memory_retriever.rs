use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};
use tokio::sync::Mutex;
use tracing::warn;

use crate::context_source::CognitiveRetrievalConfig;
use crate::conversation_recall::ConversationRecallService;
use crate::embedder::SemanticFactEmbedder;
use crate::repos::{CoActivationRepo, EpisodicMemoryRepo, SemanticFactRepo, USER_MODEL_DOMAINS};
use crate::retrieval::{retrieve_relevant_facts, RetrievalParams, ScoredFact};
use crate::search::bm25::search_episodic_memories;
use crate::services::scoring::{CommunityCache, KnowledgeDepthCache};
use crate::situation::UserSituation;

/// RRF constant — same as used in retrieval.rs BM25 merge.
const RRF_K: f64 = 60.0;
/// Minimum relevance score for a fact to be included in results.
const MIN_FACT_SCORE: f64 = 0.3;

/// Returns a freshness label for a semantic fact based on convergence score,
/// confidence, and recency of last access.
fn freshness_label(fact: &crate::types::SemanticFact) -> &'static str {
    let days_old = fact
        .last_accessed
        .as_ref()
        .and_then(|ts| ts.parse::<jiff::Timestamp>().ok())
        .map(|ts| (jiff::Timestamp::now().as_millisecond() - ts.as_millisecond()) / 86_400_000)
        .unwrap_or(90);
    if fact.convergence_score >= 0.4 || (fact.confidence >= 0.8 && days_old <= 7) {
        "strong"
    } else if fact.confidence >= 0.5 && days_old <= 30 {
        "moderate"
    } else {
        "weak -- verify"
    }
}

/// Unified memory service that merges conversation recall and cognitive facts.
///
/// Replaces `CognitiveMemoryRetriever`. Fetches from both sources concurrently,
/// merges via RRF, deduplicates (facts win), and returns a single ranked list.
pub struct UnifiedMemoryService {
    recall: Option<Arc<ConversationRecallService>>,
    fact_repo: SemanticFactRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    episodic_repo: Option<EpisodicMemoryRepo>,
    config: CognitiveRetrievalConfig,
    situation: Option<Arc<Mutex<UserSituation>>>,
    /// Live champion params from the autotuner. Read on each retrieval to override
    /// config defaults for vector_top_k, min_similarity, and relevance weights.
    champion_overrides: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
    co_activation_repo: Option<CoActivationRepo>,
    depth_cache: KnowledgeDepthCache,
    community_cache: CommunityCache,
    entity_repo: Option<crate::repos::EntityRepo>,
    /// Last-retrieved fact metadata: (id, subject, predicate) for feedback detection.
    last_retrieved_facts: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl UnifiedMemoryService {
    pub fn new(fact_repo: SemanticFactRepo) -> Self {
        Self {
            recall: None,
            fact_repo,
            embedder: None,
            episodic_repo: None,
            config: CognitiveRetrievalConfig::default(),
            situation: None,
            champion_overrides: None,
            co_activation_repo: None,
            entity_repo: None,
            depth_cache: KnowledgeDepthCache::new(60),
            community_cache: CommunityCache::new(300), // 5-minute TTL
            last_retrieved_facts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_recall_opt(mut self, recall: Option<Arc<ConversationRecallService>>) -> Self {
        self.recall = recall;
        self
    }

    pub fn with_episodic_repo(mut self, repo: EpisodicMemoryRepo) -> Self {
        self.episodic_repo = Some(repo);
        self
    }

    pub fn with_embedder_opt(mut self, embedder: Option<Arc<dyn SemanticFactEmbedder>>) -> Self {
        self.embedder = embedder;
        self
    }

    pub fn with_config(mut self, config: CognitiveRetrievalConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_situation(mut self, situation: Arc<Mutex<UserSituation>>) -> Self {
        self.situation = Some(situation);
        self
    }

    pub fn with_champion_overrides(
        mut self,
        overrides: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        self.champion_overrides = Some(overrides);
        self
    }

    pub fn with_co_activation_repo(mut self, repo: CoActivationRepo) -> Self {
        self.co_activation_repo = Some(repo);
        self
    }

    pub fn with_entity_repo(mut self, repo: crate::repos::EntityRepo) -> Self {
        self.entity_repo = Some(repo);
        self
    }

    /// Record retrieval feedback by detecting which facts the LLM referenced.
    pub async fn record_response_feedback(
        &self,
        response_text: &str,
        session_key: &str,
        feedback_repo: &storage::RetrievalFeedbackRepo,
    ) {
        let facts = self.last_retrieved_facts.lock().await;
        if facts.is_empty() {
            return;
        }
        let referenced = detect_referenced_facts(response_text, &facts);
        let retrieved_ids: Vec<String> = facts.iter().map(|(id, _, _)| id.clone()).collect();
        if let Err(e) = feedback_repo
            .insert(&retrieved_ids, &referenced, session_key, None)
            .await
        {
            tracing::debug!("Failed to record retrieval feedback: {e}");
        }
    }

    async fn current_situational_boost(&self) -> f64 {
        let Some(ref s) = self.situation else {
            return 0.0;
        };
        let guard = s.lock().await;
        (guard.energy_level * 0.25
            + guard.focus_state * 0.30
            + guard.deadline_pressure * 0.25
            + (1.0 - guard.distraction_risk) * 0.20)
            .clamp(0.0, 1.0)
    }

    /// Resolve retrieval params, merging champion overrides with config defaults.
    fn resolve_retrieval_params(
        &self,
        limit: usize,
        situational_boost: f64,
        scope_chain: Vec<(String, Option<String>)>,
    ) -> RetrievalParams {
        // Build from config defaults, then patch with champion overrides if present.
        let mut params = RetrievalParams {
            limit,
            vector_top_k: self.config.vector_top_k,
            min_similarity: self.config.min_similarity,
            situational_boost,
            max_stability: self.config.max_stability,
            relevance_weight_semantic: self.config.relevance_weight_semantic,
            relevance_weight_retrievability: self.config.relevance_weight_retrievability,
            relevance_weight_importance: self.config.relevance_weight_importance,
            relevance_weight_frequency: self.config.relevance_weight_frequency,
            relevance_weight_situation: self.config.relevance_weight_situation,
            relevance_weight_temporal: self.config.relevance_weight_temporal,
            relevance_weight_hierarchy: self.config.relevance_weight_hierarchy,
            relevance_weight_path_coherence: self.config.relevance_weight_path_coherence,
            relevance_weight_community: self.config.relevance_weight_community,
            relevance_weight_cross_note: self.config.relevance_weight_cross_note,
            relevance_weight_recall_support: self.config.relevance_weight_recall_support,
            relevance_weight_graph_path_boost: self.config.relevance_weight_graph_path_boost,
            scope_chain,
        };

        if let Some(champion) = self.read_champion_overrides() {
            params.vector_top_k = champion.vector_top_k.unwrap_or(params.vector_top_k);
            params.min_similarity = champion.min_similarity.unwrap_or(params.min_similarity);
            let defaults = [
                self.config.relevance_weight_semantic,
                self.config.relevance_weight_retrievability,
                self.config.relevance_weight_importance,
                self.config.relevance_weight_frequency,
                self.config.relevance_weight_situation,
                self.config.relevance_weight_temporal,
                self.config.relevance_weight_hierarchy,
                self.config.relevance_weight_path_coherence,
                self.config.relevance_weight_community,
                self.config.relevance_weight_cross_note,
                self.config.relevance_weight_recall_support,
                self.config.relevance_weight_graph_path_boost,
            ];
            let w = champion.resolve_full_relevance_weights(&defaults);
            params.relevance_weight_semantic = w[0];
            params.relevance_weight_retrievability = w[1];
            params.relevance_weight_importance = w[2];
            params.relevance_weight_frequency = w[3];
            params.relevance_weight_situation = w[4];
            params.relevance_weight_temporal = w[5];
            params.relevance_weight_hierarchy = w[6];
            params.relevance_weight_path_coherence = w[7];
            params.relevance_weight_community = w[8];
            params.relevance_weight_cross_note = w[9];
            params.relevance_weight_recall_support = w[10];
            params.relevance_weight_graph_path_boost = w[11];
        }

        params
    }

    /// Read champion overrides from the shared lock (non-blocking, returns None on
    /// absent/poisoned lock or no promoted champion).
    fn read_champion_overrides(&self) -> Option<common::TrialParams> {
        let lock = self.champion_overrides.as_ref()?;
        let guard = lock.read().ok()?;
        guard.clone()
    }

    /// Fetch cognitive facts, returning (id, score, content, predicate) tuples.
    async fn fetch_facts(&self, query: &str, limit: usize) -> Vec<(String, f64, String, String)> {
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Vec::new();
        }

        let situational_boost = self.current_situational_boost().await;
        let params = self.resolve_retrieval_params(limit, situational_boost, Vec::new());

        match retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            USER_MODEL_DOMAINS,
            &params,
            self.co_activation_repo.as_ref(),
            Some(&self.depth_cache),
            Some(&self.community_cache),
        )
        .await
        {
            Ok(facts) => {
                // Store for feedback detection
                {
                    let tuples: Vec<(String, String, String)> = facts
                        .iter()
                        .filter(|f| f.score > MIN_FACT_SCORE)
                        .map(|f| {
                            (
                                f.fact.id.clone(),
                                f.fact.subject.clone(),
                                f.fact.predicate.clone(),
                            )
                        })
                        .collect();
                    *self.last_retrieved_facts.lock().await = tuples;
                }
                facts
                    .into_iter()
                    .filter(|f| f.score > MIN_FACT_SCORE)
                    .map(|f| {
                        let label = freshness_label(&f.fact);
                        let content = format!(
                            "{}: {} = {} [{}]",
                            f.fact.subject, f.fact.predicate, f.fact.object, label
                        );
                        let predicate = f.fact.predicate.clone();
                        (f.fact.id, f.score, content, predicate)
                    })
                    .collect()
            }
            Err(e) => {
                warn!("Cognitive fact retrieval failed: {e}");
                Vec::new()
            }
        }
    }

    /// Retrieve memories visible to a specific scope chain.
    ///
    /// `scope_chain` is e.g. `[("system", None)]`.
    pub async fn retrieve_scoped(
        &self,
        query: &str,
        limit: usize,
        scope_chain: Vec<(String, Option<String>)>,
    ) -> Vec<MemoryEntry> {
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Vec::new();
        }
        let situational_boost = self.current_situational_boost().await;
        let params = self.resolve_retrieval_params(limit, situational_boost, scope_chain);
        match retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            USER_MODEL_DOMAINS,
            &params,
            None,
            None,
            None,
        )
        .await
        {
            Ok(facts) => facts
                .into_iter()
                .filter(|f| f.score > MIN_FACT_SCORE)
                .map(|f| {
                    let label = freshness_label(&f.fact);
                    let content = format!(
                        "{}: {} = {} [{}]",
                        f.fact.subject, f.fact.predicate, f.fact.object, label
                    );
                    MemoryEntry {
                        id: f.fact.id,
                        content,
                        score: f.score,
                        source: MemorySource::CognitiveFact,
                        raw_score: f.score,
                    }
                })
                .collect(),
            Err(e) => {
                warn!("Scoped retrieval failed: {e}");
                Vec::new()
            }
        }
    }

    /// Retrieve scored facts with overridden retrieval parameters (for shadow scoring).
    /// Returns `ScoredFact` directly (not `MemoryEntry`) so callers can access
    /// `fact.recorded_at` for age computation.
    pub async fn retrieve_with_overrides(
        &self,
        query: &str,
        vector_top_k: usize,
        min_similarity: f64,
        relevance_weights: [f64; 12],
    ) -> common::Result<Vec<ScoredFact>> {
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Ok(Vec::new());
        }

        let situational_boost = self.current_situational_boost().await;
        let params = RetrievalParams {
            limit: vector_top_k,
            vector_top_k,
            min_similarity,
            situational_boost,
            max_stability: self.config.max_stability,
            relevance_weight_semantic: relevance_weights[0],
            relevance_weight_retrievability: relevance_weights[1],
            relevance_weight_importance: relevance_weights[2],
            relevance_weight_frequency: relevance_weights[3],
            relevance_weight_situation: relevance_weights[4],
            relevance_weight_temporal: relevance_weights[5],
            relevance_weight_hierarchy: relevance_weights[6],
            relevance_weight_path_coherence: relevance_weights[7],
            relevance_weight_community: relevance_weights[8],
            relevance_weight_cross_note: relevance_weights[9],
            relevance_weight_recall_support: relevance_weights[10],
            relevance_weight_graph_path_boost: relevance_weights[11],
            scope_chain: Vec::new(),
        };

        match retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            USER_MODEL_DOMAINS,
            &params,
            None,
            None,
            None,
        )
        .await
        {
            Ok(facts) => Ok(facts
                .into_iter()
                .filter(|f| f.score > MIN_FACT_SCORE)
                .collect()),
            Err(e) => {
                warn!("Shadow retrieval failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    async fn fetch_recalls(&self, query: &str, limit: usize) -> Vec<(String, f64, String)> {
        let Some(ref recall) = self.recall else {
            return Vec::new();
        };

        match recall
            .search(query, limit, recall.config().default_threshold)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|r| (r.id, r.score, r.content))
                .collect(),
            Err(e) => {
                warn!("Conversation recall search failed: {e}");
                Vec::new()
            }
        }
    }

    /// Fetch recent episodic memories matching the query via FTS5 BM25.
    async fn fetch_episodes(&self, query: &str, limit: usize) -> Vec<(String, f64, String)> {
        let Some(ref repo) = self.episodic_repo else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }

        let pool = repo.pool();
        match search_episodic_memories(pool, query, None, limit).await {
            Ok(results) => {
                let mut entries = Vec::with_capacity(results.len());
                for bm25 in &results {
                    // Load full content — prefer summary over raw content.
                    let row: Option<(String, Option<String>)> = sqlx::query_as(
                        "SELECT content, summary FROM episodic_memories WHERE id = ?",
                    )
                    .bind(&bm25.id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

                    if let Some((content, summary)) = row {
                        let text = summary.unwrap_or(content);
                        let truncated = if text.len() > 200 {
                            format!("{}...", &text[..text.floor_char_boundary(200)])
                        } else {
                            text
                        };
                        let display = truncated.to_string();
                        entries.push((bm25.id.clone(), bm25.score, display));
                    }
                }
                entries
            }
            Err(e) => {
                warn!("Episodic BM25 search failed: {e}");
                Vec::new()
            }
        }
    }
}

#[async_trait]
impl MemoryRetriever for UnifiedMemoryService {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Fetch concurrently
        let (mut facts_raw, recalls_raw, episodes_raw) = tokio::join!(
            self.fetch_facts(query, limit),
            self.fetch_recalls(query, limit),
            self.fetch_episodes(query, 5)
        );

        // Cross-retrieval: boost facts corroborated by conversation evidence
        if !recalls_raw.is_empty() && !facts_raw.is_empty() {
            let weight = self.config.relevance_weight_recall_support;
            if weight > 0.0 {
                let recall_texts: Vec<&str> =
                    recalls_raw.iter().map(|(_, _, c)| c.as_str()).collect();
                for (_, score, content, _) in &mut facts_raw {
                    let support = recall_support_score(content, &recall_texts);
                    *score += support * weight;
                }
                // Re-sort after boosting
                facts_raw
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // 1c. Graph-aware retrieval: boost facts connected to query entity neighborhood
        if let Some(ref entity_repo) = self.entity_repo {
            let weight = self.config.relevance_weight_graph_path_boost;
            if weight > 0.0 && !facts_raw.is_empty() {
                let fact_refs: Vec<(&str, &str)> = facts_raw
                    .iter()
                    .map(|(id, _, content, _)| (id.as_str(), content.as_str()))
                    .collect();
                let boosts = crate::services::graph_retrieval::compute_graph_boosts(
                    entity_repo,
                    query,
                    &fact_refs,
                )
                .await;
                if !boosts.is_empty() {
                    for (id, score, _, _) in &mut facts_raw {
                        if let Some(&boost) = boosts.get(id) {
                            *score += boost * weight;
                        }
                    }
                    facts_raw
                        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                }
            }
        }

        if facts_raw.is_empty() && recalls_raw.is_empty() && episodes_raw.is_empty() {
            return Vec::new();
        }

        // 2. Build predicate set for deduplication (facts win)
        let fact_predicates: HashSet<String> = facts_raw
            .iter()
            .map(|(_, _, _, predicate)| predicate.to_lowercase())
            .collect();

        // 3. Filter recalls that overlap with facts (word-split match)
        let recalls_deduped: Vec<&(String, f64, String)> = if fact_predicates.is_empty() {
            recalls_raw.iter().collect()
        } else {
            recalls_raw
                .iter()
                .filter(|(_, _, content)| !content_overlaps(content, &fact_predicates))
                .collect()
        };

        // 4. RRF merge — rank-based scoring, no pre-normalization needed
        let capacity = facts_raw.len() + recalls_deduped.len() + episodes_raw.len();
        let mut rrf_scores: HashMap<String, (f64, String, MemorySource, f64)> =
            HashMap::with_capacity(capacity);

        for (rank, (id, raw_score, content, _)) in facts_raw.iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::CognitiveFact,
                *raw_score,
            ));
            entry.0 += rrf;
        }

        for (rank, (id, raw_score, content)) in recalls_deduped.iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::ConversationRecall,
                *raw_score,
            ));
            entry.0 += rrf;
        }

        for (rank, (id, raw_score, content)) in episodes_raw.iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::EpisodicMemory,
                *raw_score,
            ));
            entry.0 += rrf;
        }

        // 5. Sort and truncate
        let mut results: Vec<MemoryEntry> = rrf_scores
            .into_iter()
            .map(|(id, (score, content, source, raw_score))| MemoryEntry {
                id,
                content,
                score,
                source,
                raw_score,
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        // 5b. Fire-and-forget co-activation recording for co-retrieved facts
        if let Some(ref co_repo) = self.co_activation_repo {
            let fact_ids: Vec<String> = results
                .iter()
                .filter(|e| matches!(e.source, MemorySource::CognitiveFact))
                .map(|e| e.id.clone())
                .collect();
            if fact_ids.len() >= 2 {
                let co_repo = co_repo.clone();
                tokio::spawn(async move {
                    if let Err(e) = co_repo.record_co_retrieval(&fact_ids).await {
                        tracing::debug!("Co-activation recording failed: {e}");
                    }
                });
            }
        }

        // 6. Re-normalize RRF scores to 0.0–1.0 for display
        let rrf_scores_vec: Vec<f64> = results.iter().map(|r| r.score).collect();
        let normalized = normalize_scores(&rrf_scores_vec);
        for (entry, norm_score) in results.iter_mut().zip(normalized) {
            entry.score = norm_score;
        }

        results
    }
}

/// Min-max normalize a list of scores to 0.0–1.0.
///
/// Single-element or all-equal lists return 1.0 for every entry.
fn normalize_scores(scores: &[f64]) -> Vec<f64> {
    if scores.is_empty() {
        return Vec::new();
    }
    let (min, max) = scores
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &s| {
            (mn.min(s), mx.max(s))
        });
    let range = max - min;
    if range < f64::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores.iter().map(|s| (s - min) / range).collect()
}

/// Check if a recall's content overlaps with any fact predicate.
///
/// Splits each predicate on underscores into individual words
/// (e.g., "peak_hours" → ["peak", "hours"]) and checks if ALL words
/// appear in the recall content.
fn content_overlaps(recall_content: &str, fact_predicates: &HashSet<String>) -> bool {
    let recall_lower = recall_content.to_lowercase();
    fact_predicates.iter().any(|pred| {
        pred.split('_')
            .all(|word| !word.is_empty() && recall_lower.contains(word))
    })
}

/// Compute recall support score for a fact based on conversation evidence.
///
/// Returns the best Jaccard word-overlap between the fact's content and any
/// conversation recall snippet, scaled to [0.0, 1.0]. Jaccard >= 0.50 saturates at 1.0.
fn recall_support_score(fact_content: &str, recall_contents: &[&str]) -> f64 {
    use crate::repos::procedural_rule::word_overlap_ratio;

    if recall_contents.is_empty() {
        return 0.0;
    }

    let best = recall_contents
        .iter()
        .map(|recall| word_overlap_ratio(fact_content, recall))
        .fold(0.0_f64, f64::max);

    // Jaccard >= 0.50 saturates at 1.0; below 0.15 yields 0.0
    ((best - 0.15) / 0.35).clamp(0.0, 1.0)
}

/// Heuristically detect which facts the LLM referenced in its response.
///
/// For each retrieved fact, checks whether >50% of the subject's significant words
/// (length > 2) appear in the response text. Returns IDs of referenced facts.
pub fn detect_referenced_facts(
    response_text: &str,
    retrieved_facts: &[(String, String, String)], // (id, subject, predicate)
) -> Vec<String> {
    let response_lower = response_text.to_lowercase();
    retrieved_facts
        .iter()
        .filter(|(_, subject, _)| {
            let lower = subject.to_lowercase();
            let words: Vec<&str> = lower.split_whitespace().filter(|w| w.len() > 2).collect();
            if words.is_empty() {
                return false;
            }
            let matches = words.iter().filter(|w| response_lower.contains(*w)).count();
            matches as f64 / words.len() as f64 > 0.5
        })
        .map(|(id, _, _)| id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_engine::memory_retriever::MemorySource;

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UnifiedMemoryService>();
    }

    #[test]
    fn test_normalize_scores_min_max() {
        let scores = vec![0.3, 0.6, 0.9];
        let normalized = normalize_scores(&scores);
        assert!((normalized[0] - 0.0).abs() < f64::EPSILON);
        assert!((normalized[1] - 0.5).abs() < f64::EPSILON);
        assert!((normalized[2] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_scores_single_element() {
        let scores = vec![0.5];
        let normalized = normalize_scores(&scores);
        assert!((normalized[0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_scores_empty() {
        let scores: Vec<f64> = vec![];
        let normalized = normalize_scores(&scores);
        assert!(normalized.is_empty());
    }

    #[test]
    fn test_normalize_scores_all_equal() {
        let scores = vec![0.5, 0.5, 0.5];
        let normalized = normalize_scores(&scores);
        // All equal -> all get 1.0
        assert!(normalized.iter().all(|&s| (s - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_rrf_score_math() {
        // Verify RRF formula: 1/(k + rank + 1)
        let k = RRF_K;
        let rank_0 = 1.0 / (k + 0.0 + 1.0); // ~0.01639
        let rank_1 = 1.0 / (k + 1.0 + 1.0); // ~0.01613

        // Item at rank 0 in both lists gets 2 * rank_0
        let dual_score = rank_0 + rank_0;
        // Item at rank 0 in one list only
        let single_score = rank_0;

        assert!(
            dual_score > single_score,
            "Item in both lists should score higher than item in one"
        );
        assert!((rank_0 - 1.0 / 61.0).abs() < 1e-10);
        assert!((rank_1 - 1.0 / 62.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_retrieve_facts_only_no_recall() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool);

        // Insert a fact so fallback path returns something
        let fact = crate::types::SemanticFact {
            id: "f1".into(),
            domain: "preferences".into(),
            subject: "user".into(),
            predicate: "editor".into(),
            object: "neovim".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06T10:00:00".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
        };
        fact_repo.upsert(&fact).await.unwrap();

        let service = UnifiedMemoryService::new(fact_repo);
        // No recall wired — should still return facts
        let results = service.retrieve("what editor", 10).await;
        assert!(
            !results.is_empty(),
            "Should return facts even without recall"
        );
        assert!(results
            .iter()
            .all(|r| r.source == MemorySource::CognitiveFact));
    }

    #[tokio::test]
    async fn test_retrieve_empty_when_both_sources_empty() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool);

        let service = UnifiedMemoryService::new(fact_repo);
        let results = service.retrieve("anything", 10).await;
        assert!(results.is_empty(), "Should return empty when no data");
    }

    #[test]
    fn test_dedup_detects_overlap_with_underscore_split() {
        let predicates: HashSet<String> = ["peak_hours".to_string()].into();
        let recall_content = "I mentioned my peak hours are around 10am-12pm yesterday";

        assert!(
            content_overlaps(recall_content, &predicates),
            "Should detect overlap by splitting peak_hours into [peak, hours]"
        );
    }

    #[test]
    fn test_dedup_no_false_positive() {
        let predicates: HashSet<String> = ["peak_hours".to_string()].into();
        let recall_content = "Let's schedule a meeting tomorrow";

        assert!(
            !content_overlaps(recall_content, &predicates),
            "Should not detect overlap when predicate words absent from recall"
        );
    }

    #[test]
    fn test_dedup_requires_all_words() {
        let predicates: HashSet<String> = ["peak_hours".to_string()].into();
        // "peak" appears but "hours" doesn't — should NOT match
        let recall_content = "I hit peak performance today";

        assert!(
            !content_overlaps(recall_content, &predicates),
            "Should require ALL predicate words to match, not just one"
        );
    }

    #[test]
    fn test_detect_referenced_facts() {
        let response = "Jayden is working on a Rust project and prefers morning sessions.";
        let retrieved = vec![
            ("f1".into(), "Jayden".into(), "occupation".into()),
            ("f2".into(), "Rust project".into(), "language".into()),
            ("f3".into(), "afternoon breaks".into(), "schedule".into()),
        ];
        let referenced = detect_referenced_facts(response, &retrieved);
        assert!(referenced.contains(&"f1".to_string()));
        // "Rust" and "project" both appear in the response
        assert!(referenced.contains(&"f2".to_string()));
        // "afternoon" and "breaks" don't appear
        assert!(!referenced.contains(&"f3".to_string()));
    }

    #[test]
    fn test_recall_support_score_no_recalls() {
        assert_eq!(recall_support_score("user peak hours 10am", &[]), 0.0);
    }

    #[test]
    fn test_recall_support_score_strong_overlap() {
        let fact = "user prefers peak hours between 10am and 12pm";
        let recalls = &["user mentioned peak hours are between 10am and 12pm"];
        let score = recall_support_score(fact, recalls);
        assert!(
            score > 0.3,
            "Strong overlap should give high support, got {score}"
        );
    }

    #[test]
    fn test_recall_support_score_no_overlap() {
        let fact = "user prefers peak hours between 10am and 12pm";
        let recalls = &["schedule the deployment for Friday afternoon"];
        let score = recall_support_score(fact, recalls);
        assert!(
            score < 0.1,
            "No overlap should give near-zero support, got {score}"
        );
    }

    #[test]
    fn test_recall_support_score_partial_overlap() {
        let fact = "user favorite language rust for backend";
        let recalls = &["started learning rust for backend development recently"];
        let score = recall_support_score(fact, recalls);
        assert!(
            score > 0.0,
            "Partial overlap should give some support, got {score}"
        );
    }
}
