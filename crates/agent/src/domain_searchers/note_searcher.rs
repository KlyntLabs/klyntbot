use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use feature_notes::repo::NoteRepo;
use tracing::debug;

pub struct NoteSearcher {
    repo: NoteRepo,
}

impl NoteSearcher {
    pub fn new(repo: NoteRepo) -> Self {
        Self { repo }
    }

    /// Sanitize the query for FTS5 MATCH: extract meaningful terms,
    /// join with OR so partial matches work, and quote special chars.
    fn sanitize_fts_query(query: &str) -> String {
        let stop_words: std::collections::HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "do",
            "does", "did", "will", "would", "could", "should", "can", "may",
            "to", "of", "in", "for", "on", "with", "at", "by", "from", "and",
            "or", "but", "not", "my", "me", "i", "we", "you", "our", "your",
            "how", "what", "when", "where", "why", "who", "which",
            "show", "tell", "give", "remind", "about", "current", "status",
            "background", "context", "related", "people", "teams", "risks",
            "blockers", "overview", "details", "skill",
        ].into_iter().collect();

        let terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2 && !stop_words.contains(&w.to_lowercase().as_str()))
            .collect();

        if terms.is_empty() {
            return String::new();
        }

        // Join with OR for broad matching
        terms.join(" OR ")
    }
}

#[async_trait]
impl DomainSearcher for NoteSearcher {
    fn domain_name(&self) -> &str {
        "notes"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let fts_query = Self::sanitize_fts_query(query);
        if fts_query.is_empty() {
            return Vec::new();
        }

        debug!(
            original_query = query,
            fts_query = fts_query.as_str(),
            "📝 NoteSearcher: searching"
        );

        let results = match self.repo.search_fts(&fts_query).await {
            Ok(r) => r,
            Err(e) => {
                debug!(error = %e, query = fts_query.as_str(), "📝 NoteSearcher: FTS5 error");
                return Vec::new();
            }
        };

        debug!(
            result_count = results.len(),
            titles = ?results.iter().take(3).map(|n| n.title.as_str()).collect::<Vec<_>>(),
            "📝 NoteSearcher: found"
        );

        results
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, note)| {
                let body = note.body.as_deref().unwrap_or("");
                let body_preview = if body.len() > 500 {
                    // Find a safe UTF-8 boundary near byte 500
                    let end = body
                        .char_indices()
                        .map(|(i, _)| i)
                        .take_while(|&i| i <= 500)
                        .last()
                        .unwrap_or(body.len());
                    format!("{}...", &body[..end])
                } else {
                    body.to_string()
                };
                MemoryEntry {
                    id: note.id.clone(),
                    content: format!("[Note: {}] {}", note.title, body_preview),
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain {
                        name: "notes".into(),
                    },
                    raw_score: note.rank,
                }
            })
            .collect()
    }
}
