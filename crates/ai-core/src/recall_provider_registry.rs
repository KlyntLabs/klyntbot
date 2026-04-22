use crate::{RecallDomain, RecallProvider, RecallQuery};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct RecallProviderRegistry {
    providers: Vec<Arc<dyn RecallProvider>>,
}

impl RecallProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<P: RecallProvider + 'static>(mut self, p: P) -> Self {
        self.providers.push(Arc::new(p));
        self
    }

    pub fn register<P: RecallProvider + 'static>(&mut self, p: P) {
        self.providers.push(Arc::new(p));
    }

    /// Score every provider for the query; return `(domain, score)` pairs
    /// sorted descending, dropping zeros.
    pub fn rank(&self, query: &RecallQuery) -> Vec<(RecallDomain, f64)> {
        let mut out: Vec<_> = self
            .providers
            .iter()
            .map(|p| (p.domain(), p.score_query(query)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn RecallProvider>> {
        self.providers.iter()
    }
}
