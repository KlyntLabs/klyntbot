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

    fn note_to_entry(note: feature_notes::models::NoteSearchResult, rank: usize) -> MemoryEntry {
        let body = note.body.as_deref().unwrap_or("");
        let body_preview = if body.len() > 500 {
            format!("{}...", common::truncate_at_boundary(body, 500))
        } else {
            body.to_string()
        };
        MemoryEntry {
            id: note.id.clone(),
            content: format!("[Note: {}] {}", note.title, body_preview),
            score: 1.0 / (1.0 + rank as f64),
            source: MemorySource::Domain {
                name: "notes".into(),
            },
            raw_score: note.rank,
        }
    }

    /// Sanitize the query for FTS5 MATCH: extract meaningful terms,
    /// join with OR so partial matches work, and quote special chars.
    fn sanitize_fts_query(query: &str) -> String {
        static ENGLISH_STOP_WORDS: std::sync::LazyLock<std::collections::HashSet<&str>> =
            std::sync::LazyLock::new(|| {
                [
                    "the", "a", "an", "is", "was", "were", "be", "been", "do", "does", "did",
                    "will", "would", "could", "should", "can", "may", "to", "of", "in", "on", "at",
                    "by", "from", "or", "but", "not", "my", "me", "i", "we", "you", "our", "your",
                    "when", "where", "why", "who", "which",
                ]
                .into_iter()
                .collect()
            });

        let terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| {
                w.len() > 2 && {
                    let lower = w.to_lowercase();
                    !ENGLISH_STOP_WORDS.contains(lower.as_str())
                        && !super::SEARCHER_STOP_WORDS.contains(&lower.as_str())
                }
            })
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
        let mut entries = Vec::new();

        // 1. FTS5 content search
        let fts_query = Self::sanitize_fts_query(query);
        if !fts_query.is_empty() {
            debug!(
                original_query = query,
                fts_query = fts_query.as_str(),
                "📝 NoteSearcher: FTS5 search"
            );

            match self.repo.search_fts(&fts_query).await {
                Ok(results) => {
                    debug!(
                        result_count = results.len(),
                        titles = ?results.iter().take(3).map(|n| n.title.as_str()).collect::<Vec<_>>(),
                        "📝 NoteSearcher: FTS5 found"
                    );
                    for (i, note) in results.into_iter().take(limit).enumerate() {
                        entries.push(Self::note_to_entry(note, i));
                    }
                }
                Err(e) => {
                    debug!(error = %e, query = fts_query.as_str(), "📝 NoteSearcher: FTS5 error");
                }
            }
        }

        // 2. Notebook-name search: if the query mentions a notebook name, also return its notes
        let lower_query = query.to_lowercase();
        if lower_query.contains("notebook") || lower_query.contains("notes in") {
            if let Ok(notebooks) = self.repo.list_notebooks().await {
                for nb in &notebooks {
                    let nb_lower = nb.title.to_lowercase();
                    // Check if any notebook name words appear in the query
                    let nb_words: Vec<&str> = nb_lower.split_whitespace().collect();
                    let matches = nb_words
                        .iter()
                        .any(|w| w.len() > 2 && lower_query.contains(w));
                    if matches {
                        debug!(
                            notebook = nb.title.as_str(),
                            notebook_id = nb.id.as_str(),
                            "📝 NoteSearcher: matched notebook by name"
                        );
                        if let Ok(notes) = self.repo.list_notes(Some(&nb.id)).await {
                            let existing_ids: std::collections::HashSet<String> =
                                entries.iter().map(|e| e.id.clone()).collect();
                            for (i, note) in notes.into_iter().take(limit).enumerate() {
                                if !existing_ids.contains(&note.id) {
                                    entries.push(MemoryEntry {
                                        id: note.id.clone(),
                                        content: format!(
                                            "[Note in {}: {}] {}",
                                            nb.title,
                                            note.title,
                                            note.body.chars().take(300).collect::<String>()
                                        ),
                                        score: 0.8 / (1.0 + i as f64),
                                        source: MemorySource::Domain {
                                            name: "notes".into(),
                                        },
                                        raw_score: 0.0,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.truncate(limit);
        entries
    }
}
