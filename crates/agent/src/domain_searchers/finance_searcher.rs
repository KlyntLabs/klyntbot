use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use storage::Repos;

pub struct FinanceSearcher {
    repos: Repos,
}

impl FinanceSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl DomainSearcher for FinanceSearcher {
    fn domain_name(&self) -> &str {
        "finance"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        use storage::rows::finance::FinanceTransactionFilter;
        let filter = FinanceTransactionFilter {
            query: Some(query.to_string()),
            limit: Some(limit as i64),
            ..Default::default()
        };

        let rows = match self.repos.finance.transactions.list(&filter).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

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
