//! Memory retrieval with FSRS-scored relevance and optional vector search.
//!
//! `retrieve_relevant_facts` searches semantic facts using vector similarity
//! (when available) combined with FSRS retrievability, importance, and access
//! frequency. Falls back to importance × stability ranking when vector search
//! is unavailable or returns too few results.

use jiff::Timestamp;
use tracing::{debug, warn};

use crate::decay::{
    relevance_score, retrievability, temporal_recency_score, update_stability, RelevanceWeights,
};
use crate::embedder::SemanticFactEmbedder;
use crate::repos::{CoActivationRepo, SemanticFactRepo};
use crate::services::scoring::{self, CommunityCache, KnowledgeDepthCache};
use crate::types::SemanticFact;

/// Default RRF k parameter (shared with tools-core rrf_merge).
const DEFAULT_RRF_K: f64 = 60.0;

/// Wave-A: reciprocal rank fusion of two ScoredFact lists. Dedupes by
/// fact id; for facts present in both, RRF scores sum (boost). Score
/// field is replaced with the RRF score and list is sorted descending.
pub fn rrf_merge(a: Vec<ScoredFact>, b: Vec<ScoredFact>, k: f64) -> Vec<ScoredFact> {
    let mut by_id: std::collections::HashMap<String, (ScoredFact, f64)> = Default::default();
    for (rank, sf) in a.into_iter().enumerate() {
        let s = 1.0 / (k + (rank + 1) as f64);
        by_id
            .entry(sf.fact.id.clone())
            .and_modify(|v| v.1 += s)
            .or_insert((sf, s));
    }
    for (rank, sf) in b.into_iter().enumerate() {
        let s = 1.0 / (k + (rank + 1) as f64);
        by_id
            .entry(sf.fact.id.clone())
            .and_modify(|v| v.1 += s)
            .or_insert((sf, s));
    }
    let mut out: Vec<_> = by_id
        .into_iter()
        .map(|(_, (mut sf, score))| {
            sf.score = score;
            sf
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// A scored retrieval result with optional similarity from vector search.
#[derive(Debug, Clone)]
pub struct ScoredFact {
    pub fact: SemanticFact,
    pub score: f64,
    /// Cosine similarity from vector search. `None` if fallback path was used.
    pub similarity: Option<f64>,
}

/// Tuning knobs for `retrieve_relevant_facts`.
#[derive(Debug, Clone)]
pub struct RetrievalParams {
    pub limit: usize,
    pub vector_top_k: usize,
    pub min_similarity: f64,
    pub situational_boost: f64,
    pub max_stability: f64,
    pub relevance_weight_semantic: f64,
    pub relevance_weight_retrievability: f64,
    pub relevance_weight_importance: f64,
    pub relevance_weight_frequency: f64,
    pub relevance_weight_situation: f64,
    pub relevance_weight_temporal: f64,
    pub relevance_weight_hierarchy: f64,
    pub relevance_weight_path_coherence: f64,
    pub relevance_weight_community: f64,
    pub relevance_weight_cross_note: f64,
    pub relevance_weight_recall_support: f64,
    pub relevance_weight_graph_path_boost: f64,
    /// Optional scope chain for filtering. When set, only facts matching
    /// these scopes are considered. When empty, all scopes are included (backwards-compatible).
    pub scope_chain: Vec<(String, Option<String>)>,
}

impl RetrievalParams {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            vector_top_k: 30,
            min_similarity: 0.55,
            situational_boost: 0.0,
            max_stability: 30.0,
            relevance_weight_semantic: 0.3,
            relevance_weight_retrievability: 0.2,
            relevance_weight_importance: 0.15,
            relevance_weight_frequency: 0.1,
            relevance_weight_situation: 0.25,
            relevance_weight_temporal: 0.05,
            relevance_weight_hierarchy: 0.10,
            relevance_weight_path_coherence: 0.05,
            relevance_weight_community: 0.15,
            relevance_weight_cross_note: 0.10,
            relevance_weight_recall_support: 0.08,
            relevance_weight_graph_path_boost: 0.06,
            scope_chain: Vec::new(),
        }
    }
}

/// Retrieve and rank facts using FSRS scoring with optional vector search.
///
/// **Vector path** (embedder available + non-empty query + ≥3 vector results):
///   1. `search_similar(query, domains, top_k)` → candidate fact_ids with similarity
///   2. Batch-load facts from SQL
///   3. Score with real `semantic_similarity` in the relevance formula
///
/// **Fallback path** (embedder unavailable, empty query, or <3 vector results):
///   1. Load all active facts across requested domains from SQL
///   2. Score with `semantic_similarity = 0.5` (neutral)
///   3. Secondary sort by `importance × stability`
///
/// Both paths record access events on retrieved facts (increases FSRS stability).
#[allow(clippy::too_many_arguments)]
pub async fn retrieve_relevant_facts(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    params: &RetrievalParams,
    co_activation_repo: Option<&CoActivationRepo>,
    depth_cache: Option<&KnowledgeDepthCache>,
    community_cache: Option<&CommunityCache>,
    entity_repo: Option<&crate::repos::EntityRepo>,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    // Wave 1: extract once at function-top so Wave 2 (speaker boost) and
    // the BM25 alias-expansion block can share the same entity set.
    let query_entities = crate::services::graph_retrieval::extract_query_entities(query);

    let use_vector = !query.is_empty() && embedder.map(|e| e.is_available()).unwrap_or(false);

    let weights = RelevanceWeights {
        semantic: params.relevance_weight_semantic,
        retrievability: params.relevance_weight_retrievability,
        importance: params.relevance_weight_importance,
        frequency: params.relevance_weight_frequency,
        situation: params.relevance_weight_situation,
        temporal: params.relevance_weight_temporal,
        hierarchy: params.relevance_weight_hierarchy,
        path_coherence: params.relevance_weight_path_coherence,
        community: params.relevance_weight_community,
        cross_note: params.relevance_weight_cross_note,
        recall_support: params.relevance_weight_recall_support,
        graph_path_boost: params.relevance_weight_graph_path_boost,
    };

    debug!(
        query = query,
        use_vector = use_vector,
        domains = ?domains,
        "🔎 retrieve_relevant_facts: searching"
    );

    let mut scored = if use_vector {
        let embedder = embedder.unwrap(); // safe: checked above
        match embedder
            .search_similar(query, domains, params.vector_top_k, params.min_similarity)
            .await
        {
            Ok(hits) => {
                crate::bench_hooks::record_hits(hits.len() as u32, 0, 0);
                // Wave-A: hybrid RRF merge — always run both vector and
                // fallback (FTS-anchored) paths and combine via reciprocal
                // rank fusion. Replaces the prior one-or-other branch
                // where vector_path silently masked the FTS-primary
                // signal whenever it returned ≥3 hits.
                let vector_scored =
                    vector_path(repo, &hits, params.situational_boost, &weights, depth_cache)
                        .await?;
                let fallback_scored = fallback_path(
                    repo,
                    domains,
                    params.situational_boost,
                    &weights,
                    &params.scope_chain,
                    depth_cache,
                )
                .await?;
                rrf_merge(vector_scored, fallback_scored, DEFAULT_RRF_K)
            }
            Err(e) => {
                warn!("Vector search failed, using fallback: {e}");
                fallback_path(
                    repo,
                    domains,
                    params.situational_boost,
                    &weights,
                    &params.scope_chain,
                    depth_cache,
                )
                .await?
            }
        }
    } else {
        fallback_path(
            repo,
            domains,
            params.situational_boost,
            &weights,
            &params.scope_chain,
            depth_cache,
        )
        .await?
    };

    // BM25 boost: add FTS5 signal for non-empty queries
    if !query.is_empty() {
        // Pass domain filter when scoped to a single domain to avoid cross-domain noise
        let bm25_domain = if domains.len() == 1 {
            Some(domains[0])
        } else {
            None
        };

        // Build a per-entity FTS term list so proper nouns aren't
        // drowned out by question filler words under BM25. When an
        // EntityRepo is wired, expand each detected entity through its
        // alias table (possessive / short-form / nickname forms) so
        // questions phrased as "Caroline's necklace" or "Mel's husband"
        // resolve to the canonical entity. Always-on now that Tier 1's
        // FTS-as-candidate fix is in place; production paths see
        // identical behavior to pre-Tier 1 when no entity is in the
        // query.
        let entities = &query_entities;
        crate::bench_hooks::record_entities(entities);
        let fts_terms: Vec<String> = if entities.is_empty() {
            vec![query.to_string()]
        } else {
            let mut terms_set: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for e in entities {
                terms_set.insert(format!("\"{}\"", e.replace('"', "")));
                if let Some(er) = entity_repo {
                    if let Ok(rows) = er.find_by_name(e).await {
                        for row in rows {
                            if let Ok(aliases) = er.list_aliases(&row.id).await {
                                for alias in aliases {
                                    terms_set.insert(format!("\"{}\"", alias.replace('"', "")));
                                }
                            }
                        }
                    }
                }
            }
            let mut terms: Vec<String> = terms_set.into_iter().collect();
            terms.push(query.to_string());
            terms
        };

        // Collect rank + full fact row for every FTS hit across all terms.
        // Keeping the SemanticFact (not just the id) lets us PROMOTE hits
        // that the vector/fallback path missed into the scored pool — a
        // pure rank-only merge previously dropped them, which produced
        // 100% zero-retrieval refusals on questions containing proper
        // nouns the embedder under-weighted.
        let mut merged: std::collections::HashMap<String, (usize, SemanticFact)> =
            std::collections::HashMap::new();
        let mut total_hits = 0usize;
        for term in &fts_terms {
            if let Ok(bm25_hits) = repo.search_fts(term, bm25_domain, params.limit * 2).await {
                total_hits += bm25_hits.len();
                for (rank, f) in bm25_hits.into_iter().enumerate() {
                    merged
                        .entry(f.id.clone())
                        .and_modify(|(r, _)| {
                            if rank < *r {
                                *r = rank;
                            }
                        })
                        .or_insert_with(|| (rank, f));
                }
            }
        }
        crate::bench_hooks::record_hits(0, total_hits as u32, 0);

        let scored_ids: std::collections::HashSet<String> =
            scored.iter().map(|s| s.fact.id.clone()).collect();
        let now = Timestamp::now();

        for result in &mut scored {
            if let Some(&(rank, _)) = merged.get(&result.fact.id) {
                let bm25_boost = 1.0 / (DEFAULT_RRF_K + rank as f64 + 1.0);
                result.score += bm25_boost;
            }
        }

        // Promote FTS-only hits into the scored pool. Score them with the
        // same relevance formula fallback_path uses (neutral similarity)
        // plus the RRF rank bonus, so they land on the same scale as
        // fallback candidates and pass the downstream MIN_FACT_SCORE
        // floor. Without this, a fact correctly stored under
        // `subject="Alice"` but missed by the vector path is invisible —
        // which is the root cause of the 100%-zero-retrieval pattern
        // observed in p1-confirm-001.
        for (id, (rank, fact)) in merged {
            if scored_ids.contains(&id) {
                continue;
            }
            let (r, freq) = compute_decay_and_freq(&fact, &now);
            let temporal = temporal_recency_score(&fact.valid_from);
            let hierarchy = match depth_cache {
                Some(cache) => cache.get_or_compute(&fact, repo).await,
                None => 0.0,
            };
            let cross_note = scoring::convergence_score(&fact);
            let base = relevance_score(
                0.5,
                r,
                fact.confidence,
                freq,
                params.situational_boost,
                temporal,
                hierarchy,
                0.5,
                0.0,
                cross_note,
                0.0,
                0.0,
                &weights,
            );
            let bm25_boost = 1.0 / (DEFAULT_RRF_K + rank as f64 + 1.0);
            scored.push(ScoredFact {
                fact,
                score: base + bm25_boost,
                similarity: None,
            });
        }
    }

    // Wave 2: speaker-match boost. When the query mentions a person whose
    // name appears as a fact `speaker`, bump those facts. This is the
    // single-pool minimum-viable form of ENGRAM's R̃(q,A)/R̃(q,B) speaker
    // split — full pool partitioning is a future wave. Boost is small
    // (additive 0.10) so a wrong speaker match doesn't displace a strong
    // semantic match.
    if !query_entities.is_empty() {
        let entity_set: std::collections::HashSet<String> =
            query_entities.iter().map(|e| e.to_lowercase()).collect();
        for result in &mut scored {
            if let Some(spk) = &result.fact.speaker {
                if entity_set.contains(&spk.to_lowercase()) {
                    result.score += 0.10;
                }
            }
        }
    }

    // Second pass: community re-ranking (needs peer IDs from first pass).
    // Uses max(direct co-activation, Louvain transitive community) to capture
    // both direct and transitive relationships between facts.
    if let Some(co_repo) = co_activation_repo {
        let all_ids: Vec<String> = scored.iter().map(|s| s.fact.id.clone()).collect();
        if all_ids.len() >= 2 {
            // Lazily compute Louvain community snapshot (cached for ~5 min)
            let community_snapshot = match community_cache {
                Some(cache) => cache.get_or_compute(co_repo).await,
                None => None,
            };

            for result in &mut scored {
                let peer_ids: Vec<String> = all_ids
                    .iter()
                    .filter(|id| *id != &result.fact.id)
                    .cloned()
                    .collect();

                // Direct pairwise co-activation
                let co_score =
                    scoring::co_activation_score(&result.fact.id, &peer_ids, co_repo).await;

                // Transitive community boost (Louvain)
                let community_score = community_snapshot
                    .as_ref()
                    .map(|snap| scoring::community_boost_score(&result.fact.id, &peer_ids, snap))
                    .unwrap_or(0.0);

                // Use the stronger signal: direct co-activation or transitive community
                let effective = co_score.max(community_score);
                result.score += effective * weights.community;
            }
        }
    }

    // T0.2: Graph-aware boost. When EntityRepo is wired and graph_path_boost
    // weight > 0, boost facts whose subjects sit close to query-entity
    // neighborhoods. Mirrors the post-hoc wiring in `UnifiedMemoryService::retrieve`
    // (memory_retriever.rs) so the static fact-injection path benefits from the
    // same signal as the dynamic path. recall_support is intentionally NOT wired
    // here: this path has no recall corpus to compute "support" against.
    if let Some(er) = entity_repo {
        if weights.graph_path_boost > 0.0 && !scored.is_empty() {
            let fact_refs: Vec<(&str, &str)> = scored
                .iter()
                .map(|s| (s.fact.id.as_str(), s.fact.subject.as_str()))
                .collect();
            let boosts =
                crate::services::graph_retrieval::compute_graph_boosts(er, query, &fact_refs)
                    .await;
            if !boosts.is_empty() {
                for result in &mut scored {
                    if let Some(&b) = boosts.get(&result.fact.id) {
                        result.score += b * weights.graph_path_boost;
                    }
                }
            }
        }
    }

    // Sort by FSRS score regardless of path (vector re-ranking can change order)
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(params.limit);

    // Record access on retrieved facts (increases FSRS stability)
    for result in &scored {
        let new_stability = update_stability(result.fact.stability, true, params.max_stability);
        if let Err(e) = repo.record_access(&result.fact.id, new_stability).await {
            warn!("Failed to record access for fact '{}': {e}", result.fact.id);
        }
    }

    debug!(
        "Retrieved {} facts across {} domains (top score: {:.3}, vector: {})",
        scored.len(),
        domains.len(),
        scored.first().map(|r| r.score).unwrap_or(0.0),
        use_vector,
    );

    Ok(scored)
}

/// Score facts using real vector similarity.
async fn vector_path(
    repo: &SemanticFactRepo,
    hits: &[(String, f64)],
    situational_boost: f64,
    weights: &RelevanceWeights,
    depth_cache: Option<&KnowledgeDepthCache>,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<&str> = hits.iter().map(|(id, _)| id.as_str()).collect();
    let sim_map: std::collections::HashMap<&str, f64> =
        hits.iter().map(|(id, sim)| (id.as_str(), *sim)).collect();

    let facts = repo.get_batch(&ids).await?;
    let now = Timestamp::now();

    let mut scored = Vec::with_capacity(facts.len());
    for fact in facts {
        let similarity = sim_map.get(fact.id.as_str()).copied().unwrap_or(0.5);
        let (r, freq) = compute_decay_and_freq(&fact, &now);
        let temporal = temporal_recency_score(&fact.valid_from);
        let hierarchy = match depth_cache {
            Some(cache) => cache.get_or_compute(&fact, repo).await,
            None => 0.0,
        };
        let cross_note = scoring::convergence_score(&fact);
        // community_score (co-activation) computed in second pass
        // recall_support computed post-hoc in UnifiedMemoryService
        let score = relevance_score(
            similarity,
            r,
            fact.confidence,
            freq,
            situational_boost,
            temporal,
            hierarchy,
            0.5,
            0.0,
            cross_note,
            0.0,
            0.0,
            weights,
        );
        scored.push(ScoredFact {
            fact,
            score,
            similarity: Some(similarity),
        });
    }
    Ok(scored)
}

/// Score facts with neutral similarity (fallback when vector search unavailable).
async fn fallback_path(
    repo: &SemanticFactRepo,
    domains: &[&str],
    situational_boost: f64,
    weights: &RelevanceWeights,
    scope_chain: &[(String, Option<String>)],
    depth_cache: Option<&KnowledgeDepthCache>,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let all_facts = if scope_chain.is_empty() {
        // Backwards-compatible: load all domains
        let mut all = Vec::new();
        for domain in domains {
            all.extend(repo.list_active(domain).await?);
        }
        all
    } else {
        // Scoped: load facts visible to the scope chain
        let chain_refs: Vec<(&str, Option<&str>)> = scope_chain
            .iter()
            .map(|(st, sid)| (st.as_str(), sid.as_deref()))
            .collect();
        repo.list_by_scope_chain(&chain_refs).await?
    };

    let now = Timestamp::now();

    let mut scored = Vec::with_capacity(all_facts.len());
    for fact in all_facts {
        let (r, freq) = compute_decay_and_freq(&fact, &now);
        let temporal = temporal_recency_score(&fact.valid_from);
        let hierarchy = match depth_cache {
            Some(cache) => cache.get_or_compute(&fact, repo).await,
            None => 0.0,
        };
        let cross_note = scoring::convergence_score(&fact);
        // community_score (co-activation) computed in second pass
        // recall_support computed post-hoc in UnifiedMemoryService
        let score = relevance_score(
            0.5,
            r,
            fact.confidence,
            freq,
            situational_boost,
            temporal,
            hierarchy,
            0.5,
            0.0,
            cross_note,
            0.0,
            0.0,
            weights,
        );
        scored.push(ScoredFact {
            fact,
            score,
            similarity: None,
        });
    }

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(scored)
}

/// Compute FSRS retrievability and normalized access frequency for a fact.
fn compute_decay_and_freq(fact: &SemanticFact, now: &Timestamp) -> (f64, f64) {
    let elapsed_days = fact
        .last_accessed
        .as_ref()
        .and_then(|la| la.parse::<Timestamp>().ok())
        .map(|la| (now.as_millisecond() - la.as_millisecond()) as f64 / 86_400_000.0)
        .unwrap_or_else(|| {
            fact.recorded_at
                .parse::<Timestamp>()
                .ok()
                .map(|ra| (now.as_millisecond() - ra.as_millisecond()) as f64 / 86_400_000.0)
                .unwrap_or(30.0)
        });

    let r = retrievability(elapsed_days, fact.stability);
    let freq = 1.0 - (1.0 / (1.0 + fact.access_count as f64));
    (r, freq)
}

/// Retrieve facts across all domains, ranked by FSRS score.
pub async fn retrieve_all_domains(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    limit_per_domain: usize,
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let params = RetrievalParams {
        limit: limit_per_domain * domains.len(),
        situational_boost,
        ..RetrievalParams::new(0)
    };
    // retrieve_relevant_facts already returns results sorted by score.
    retrieve_relevant_facts(
        repo, embedder, query, domains, &params, None, None, None, None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::SemanticFactEmbedder;
    use crate::types::DEFAULT_MEMORY_TYPE;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn default_params(limit: usize) -> RetrievalParams {
        RetrievalParams {
            limit,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        }
    }

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str, predicate: &str, stability: f64, access_count: i64) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: "value".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06T10:00:00".into(),
            superseded_at: None,
            superseded_by: None,
            stability,
            last_accessed: None,
            access_count,
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        }
    }

    /// Mock embedder that returns configurable similarity scores.
    struct MockEmbedder {
        available: AtomicBool,
        similarities: std::sync::Mutex<Vec<(String, f64)>>,
    }

    impl MockEmbedder {
        fn new(similarities: Vec<(String, f64)>) -> Self {
            Self {
                available: AtomicBool::new(true),
                similarities: std::sync::Mutex::new(similarities),
            }
        }

        fn unavailable() -> Self {
            Self {
                available: AtomicBool::new(false),
                similarities: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl SemanticFactEmbedder for MockEmbedder {
        async fn embed_and_store_fact(&self, _fact: &SemanticFact) -> common::Result<()> {
            Ok(())
        }
        async fn remove_embedding(&self, _fact_id: &str) -> common::Result<()> {
            Ok(())
        }
        async fn search_similar(
            &self,
            _query: &str,
            _domains: &[&str],
            _top_k: usize,
            _min_similarity: f64,
        ) -> common::Result<Vec<(String, f64)>> {
            Ok(self.similarities.lock().unwrap().clone())
        }
        async fn reindex_all(&self, _facts: &[SemanticFact]) -> common::Result<usize> {
            Ok(0)
        }
        fn is_available(&self) -> bool {
            self.available.load(Ordering::Relaxed)
        }
    }

    #[tokio::test]
    async fn test_vector_path_uses_real_similarity() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 1.0, 0))
            .await
            .unwrap();

        // f2 has higher similarity than f1
        let embedder = MockEmbedder::new(vec![("f1".into(), 0.6), ("f2".into(), 0.9)]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "when do I take breaks",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        // f2 should rank higher due to 0.9 similarity
        assert_eq!(results[0].fact.id, "f2");
        assert!(results[0].similarity.unwrap() > 0.8);
    }

    #[tokio::test]
    async fn test_fallback_when_embedder_is_none() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 5.0, 10))
            .await
            .unwrap();

        let results = retrieve_relevant_facts(
            &repo,
            None,
            "anything",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        // All should have similarity = None (fallback path)
        assert!(results.iter().all(|r| r.similarity.is_none()));
    }

    #[tokio::test]
    async fn test_fallback_when_query_is_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        let embedder = MockEmbedder::new(vec![]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].similarity.is_none());
    }

    #[tokio::test]
    async fn test_fallback_when_embedder_unavailable() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        let embedder = MockEmbedder::unavailable();

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "query",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].similarity.is_none());
    }

    #[tokio::test]
    async fn test_fallback_when_few_vector_results() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert 5 facts
        for i in 0..5 {
            repo.upsert(&test_fact(&format!("f{i}"), &format!("pred{i}"), 1.0, 0))
                .await
                .unwrap();
        }

        // Vector search returns only 2 results (below threshold of 3)
        let embedder = MockEmbedder::new(vec![("f0".into(), 0.9), ("f1".into(), 0.8)]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "query",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Should have more than 2 results (fallback merged in)
        assert!(results.len() > 2);
    }

    #[tokio::test]
    async fn test_scored_fact_records_access() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        retrieve_relevant_facts(
            &repo,
            None,
            "",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let updated = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(updated.access_count, 1);
        assert!(updated.stability > 1.0);
    }

    #[tokio::test]
    async fn test_respects_limit() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        for i in 0..10 {
            repo.upsert(&test_fact(&format!("f{i}"), &format!("pred{i}"), 1.0, 0))
                .await
                .unwrap();
        }

        let results = retrieve_relevant_facts(
            &repo,
            None,
            "",
            &["productivity"],
            &default_params(3),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_retrieve_empty_domain() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let results = retrieve_relevant_facts(
            &repo,
            None,
            "",
            &["nonexistent"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_retrieval_uses_bm25_when_query_non_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert facts with distinctive text for BM25 matching
        let mut f1 = test_fact("f1", "morning routine", 1.0, 0);
        f1.object = "wakes up early for deep work".to_string();
        repo.upsert(&f1).await.unwrap();
        let mut f2 = test_fact("f2", "afternoon break", 1.0, 0);
        f2.object = "take a walk after lunch".to_string();
        repo.upsert(&f2).await.unwrap();

        // Without vector search, BM25 should still boost relevant facts
        let results = retrieve_relevant_facts(
            &repo,
            None,
            "morning routine",
            &["productivity"],
            &default_params(10),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        // f1 should rank higher because "morning routine" matches query via BM25
        assert_eq!(results[0].fact.id, "f1");
    }

    #[tokio::test]
    async fn test_retrieve_all_domains() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let mut f1 = test_fact("f1", "peak_hours", 1.0, 0);
        f1.domain = "productivity".into();
        let mut f2 = test_fact("f2", "budget", 1.0, 0);
        f2.domain = "finance".into();

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = retrieve_all_domains(&repo, None, "", &["productivity", "finance"], 5, 0.5)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }
}
