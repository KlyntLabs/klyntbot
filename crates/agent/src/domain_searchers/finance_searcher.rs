use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use storage::Repos;
use tracing::debug;

pub struct FinanceSearcher {
    repos: Repos,
}

impl FinanceSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }

    fn extract_search_term(query: &str) -> String {
        super::extract_first_keyword(
            query,
            &[
                "much", "did", "spend", "month", "this", "last", "year", "total",
            ],
        )
    }
}

#[async_trait]
impl DomainSearcher for FinanceSearcher {
    fn domain_name(&self) -> &str {
        "finance"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        use storage::rows::finance::FinanceTransactionFilter;

        let search_term = Self::extract_search_term(query);
        if search_term.is_empty() {
            return Vec::new();
        }

        debug!(
            original_query = query,
            search_term = search_term.as_str(),
            "💰 FinanceSearcher: searching"
        );

        let filter = FinanceTransactionFilter {
            query: Some(search_term.clone()),
            limit: Some(limit as i64),
            ..Default::default()
        };

        let rows = match self.repos.finance.transactions.list(&filter).await {
            Ok(r) => r,
            Err(e) => {
                debug!(error = %e, "💰 FinanceSearcher: error");
                return Vec::new();
            }
        };

        debug!(result_count = rows.len(), "💰 FinanceSearcher: found");

        rows.into_iter()
            .enumerate()
            .map(|(i, tx)| {
                let amount_display = format!("{:.2}", tx.amount as f64 / 100.0);
                let desc = format!(
                    "[Transaction: {} {} {} on {}] {}",
                    tx.tx_type,
                    amount_display,
                    tx.currency,
                    tx.tx_date,
                    tx.notes.as_deref().unwrap_or(""),
                );
                MemoryEntry {
                    id: tx.id.clone(),
                    content: desc,
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain {
                        name: "finance".into(),
                    },
                    raw_score: 0.0,
                }
            })
            .collect()
    }
}
