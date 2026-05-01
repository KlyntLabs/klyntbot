#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolMeta {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub name: String,
    pub score: f32,
    pub description: String,
}

pub struct ToolIndex { meta: Vec<ToolMeta> }

impl ToolIndex {
    pub fn build(meta: &[ToolMeta]) -> Self { Self { meta: meta.to_vec() } }

    pub fn search(&self, query: &str, top_n: usize) -> Vec<SearchHit> {
        let q = query.trim().to_lowercase();
        let mut hits: Vec<SearchHit> = self.meta.iter().map(|m| {
            let mut score = 0.0_f32;
            if !q.is_empty() {
                if m.name.to_lowercase().contains(&q) { score += 2.0; }
                if m.description.to_lowercase().contains(&q) { score += 1.0; }
                if m.aliases.iter().any(|a| a.to_lowercase().contains(&q)) { score += 1.5; }
            } else {
                score = 1.0;
            }
            SearchHit { name: m.name.clone(), score, description: m.description.clone() }
        }).filter(|h| h.score > 0.0).collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name)));
        hits.truncate(top_n);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fake_registry() -> Vec<ToolMeta> {
        vec![
            ToolMeta { name: "bash".into(), aliases: vec![], description: "run a shell command".into() },
            ToolMeta { name: "read".into(), aliases: vec![], description: "read a file".into() },
            ToolMeta { name: "edit".into(), aliases: vec![], description: "edit a file in place".into() },
        ]
    }

    #[test] fn substring_query_returns_matches() {
        let idx = ToolIndex::build(&fake_registry());
        let hits = idx.search("file", 10);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|h| h.name == "read"));
    }

    #[test] fn empty_query_returns_top_n_alphabetical() {
        let idx = ToolIndex::build(&fake_registry());
        let hits = idx.search("", 2);
        assert_eq!(hits.len(), 2);
    }
}
