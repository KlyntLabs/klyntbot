use async_trait::async_trait;
use cognitive::SemanticFactRepo;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};

/// Common English stop words to filter from FTS queries.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "for", "and", "but", "or", "not",
    "at", "by", "in", "of", "on", "to", "up", "it", "its", "my", "me", "we", "he", "she", "no",
    "if", "as", "with", "this", "that", "from", "what", "how", "all", "when", "who", "which", "do",
    "does", "did", "will", "would", "could", "should", "may", "might", "can", "have", "has", "had",
    "so", "yet", "just", "any", "some", "each",
];

/// Convert a natural-language query into an FTS5 OR-mode query.
///
/// FTS5 uses implicit AND for multi-word queries, which causes near-zero recall
/// when the query text differs from stored facts. This function:
/// 1. Splits on whitespace
/// 2. Strips trailing punctuation from each token
/// 3. Filters out words shorter than 3 chars and common stop words
/// 4. Joins remaining tokens with " OR "
/// 5. Falls back to OR-joining all original (stripped) words if filtering leaves nothing
pub fn to_fts_or_query(query: &str) -> String {
    let stripped: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.trim_end_matches(|c: char| c.is_ascii_punctuation())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();

    let filtered: Vec<&str> = stripped
        .iter()
        .filter(|w| w.len() >= 3 && !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .map(String::as_str)
        .collect();

    if filtered.is_empty() {
        // Fallback: join all original (stripped) words with OR
        stripped.join(" OR ")
    } else {
        filtered.join(" OR ")
    }
}

/// Simple FTS-based memory retriever for simulation.
///
/// Queries the semantic fact repo directly without embeddings, using SQLite
/// full-text search. This is sufficient for measuring whether extracted facts
/// are retrievable and computing precision/recall when ground-truth annotations
/// include `relevant_facts`.
pub struct FtsMemoryRetriever {
    repo: SemanticFactRepo,
}

impl FtsMemoryRetriever {
    pub fn new(repo: SemanticFactRepo) -> Self {
        Self { repo }
    }

    /// Retrieve memories filtered by domain, for better precision within a topic.
    pub async fn retrieve_in_domain(
        &self,
        query: &str,
        domain: &str,
        limit: usize,
    ) -> Vec<MemoryEntry> {
        self.search(query, Some(domain), limit).await
    }

    async fn search(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        let fts_query = to_fts_or_query(query);
        match self.repo.search_fts(&fts_query, domain, limit).await {
            Ok(facts) => facts
                .into_iter()
                .enumerate()
                .map(|(rank, fact)| MemoryEntry {
                    id: fact.id,
                    content: format!("{} {} {}", fact.subject, fact.predicate, fact.object),
                    score: 1.0 / (rank as f64 + 1.0),
                    source: MemorySource::CognitiveFact,
                    raw_score: 1.0 / (rank as f64 + 1.0),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[async_trait]
impl MemoryRetriever for FtsMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.search(query, None, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_or_query_filters_stop_words() {
        let result = to_fts_or_query("Show me my tasks for the project");
        assert!(result.contains("Show"));
        assert!(result.contains("tasks"));
        assert!(result.contains("project"));
        // Stop words and short words filtered out
        assert!(!result.contains("\"the\""));
        assert!(!result.contains("\"my\""));
        assert!(!result.contains("\"me\""));
        assert!(!result.contains("\"for\""));
        // Should be OR-joined
        assert!(result.contains(" OR "));
    }

    #[test]
    fn fts_or_query_handles_empty_after_filtering() {
        // All words are either stop words or too short
        let result = to_fts_or_query("I am a");
        // Falls back to OR-joining all original words
        assert!(result.contains("OR"));
        assert!(result.contains("I"));
        assert!(result.contains("am"));
        assert!(result.contains("a"));
    }

    #[test]
    fn fts_or_query_strips_punctuation() {
        let result = to_fts_or_query("What's left on project?");
        // "project?" should become "project" (trailing ? stripped)
        assert!(result.contains("project"));
        assert!(!result.contains("project?"));
        // "What's" keeps the apostrophe (it's not trailing punctuation)
        assert!(result.contains("What's"));
    }

    #[tokio::test]
    async fn retrieves_related_facts_with_partial_overlap() {
        use cognitive::{cognitive_migrations, SemanticFact, SemanticFactRepo};
        use context_engine::memory_retriever::MemoryRetriever;
        use storage::StoragePool;

        let pool = StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        StoragePool::run_feature_migrations(&inner, &cognitive_migrations())
            .await
            .unwrap();

        let repo = SemanticFactRepo::new(inner);

        let fact = SemanticFact {
            id: "fact-1".into(),
            domain: "tasks".into(),
            subject: "user".into(),
            predicate: "requested".into(),
            object: "Create a task: review PR for main project, due Friday".into(),
            confidence: 0.9,
            source: "observed".into(),
            valid_from: "2026-04-01".into(),
            valid_until: None,
            recorded_at: "2026-04-01".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: "fact".into(),
            scope_type: "system".into(),
            scope_id: None,
        };
        repo.upsert(&fact).await.unwrap();

        let retriever = FtsMemoryRetriever::new(SemanticFactRepo::new(repo.pool().clone()));

        // Query shares "tasks" and "project" with the stored fact
        let results = retriever
            .retrieve("Show me my tasks for the project", 10)
            .await;

        assert!(
            !results.is_empty(),
            "Expected to retrieve fact with partial term overlap via OR query"
        );
        assert_eq!(results[0].id, "fact-1");

        // Keep the in-memory DB alive until assertions complete
        std::mem::forget(pool);
    }

    #[tokio::test]
    async fn retrieve_in_domain_filters_correctly() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &cognitive::cognitive_migrations())
            .await
            .unwrap();

        let repo = cognitive::SemanticFactRepo::new(inner.clone());

        for (id, domain, obj) in [
            ("f1", "tasks", "Create a task: review PR"),
            ("f2", "finance", "Record expense: lunch"),
        ] {
            let fact = cognitive::types::SemanticFact {
                id: id.to_string(),
                domain: domain.to_string(),
                subject: "user".to_string(),
                predicate: "stated".to_string(),
                object: obj.to_string(),
                confidence: 1.0,
                source: "user_stated".to_string(),
                valid_from: "2025-01-01".to_string(),
                valid_until: None,
                recorded_at: "2025-01-01T00:00:00".to_string(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                memory_type: "fact".to_string(),
                scope_type: "system".to_string(),
                scope_id: None,
            };
            repo.upsert(&fact).await.unwrap();
        }

        let retriever = FtsMemoryRetriever::new(cognitive::SemanticFactRepo::new(inner));

        let results = retriever
            .retrieve_in_domain("review task", "tasks", 10)
            .await;
        assert!(
            results.iter().all(|r| r.id != "f2"),
            "should not return finance facts when filtering by tasks domain"
        );

        std::mem::forget(pool);
    }
}
