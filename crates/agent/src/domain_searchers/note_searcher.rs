use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use feature_notes::repo::NoteRepo;

pub struct NoteSearcher {
    repo: NoteRepo,
}

impl NoteSearcher {
    pub fn new(repo: NoteRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl DomainSearcher for NoteSearcher {
    fn domain_name(&self) -> &str {
        "notes"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let results = match self.repo.search_fts(query).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

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
