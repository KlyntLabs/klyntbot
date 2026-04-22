//! FinanceSpendingDriftSource — accumulates budget alert signals and flushes periodically.

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use uuid::Uuid;

use crate::mirror::{CategorySpend, FinanceDriftSnapshot, MirrorRepo};

pub struct FinanceSpendingDriftSource {
    repo: MirrorRepo,
    per_category: std::sync::Mutex<HashMap<String, CategorySpend>>,
    total_transactions: AtomicU32,
    over_budget_count: AtomicU32,
}

impl FinanceSpendingDriftSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            per_category: std::sync::Mutex::new(HashMap::new()),
            total_transactions: AtomicU32::new(0),
            over_budget_count: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for FinanceSpendingDriftSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "finance_drift",
            subscribed_kinds: &["TransactionRecorded", "BudgetAlert"],
            flush_interval_secs: Some(3600),
        }
    }

    fn name(&self) -> &'static str {
        "finance-spending-drift-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(raw) = &signal.raw_event {
            match raw {
                bus::DomainEvent::TransactionRecorded {
                    category,
                    amount,
                    is_over_budget,
                } => {
                    let mut cats = self.per_category.lock().unwrap();
                    let entry = cats
                        .entry(category.clone())
                        .or_insert_with(|| CategorySpend {
                            total_amount: 0.0,
                            transaction_count: 0,
                            budget_alerts: 0,
                        });
                    entry.total_amount += *amount;
                    entry.transaction_count += 1;
                    drop(cats);
                    self.total_transactions.fetch_add(1, Ordering::Relaxed);
                    if *is_over_budget {
                        self.over_budget_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                bus::DomainEvent::BudgetAlert { category, .. } => {
                    let mut cats = self.per_category.lock().unwrap();
                    let entry = cats
                        .entry(category.clone())
                        .or_insert_with(|| CategorySpend {
                            total_amount: 0.0,
                            transaction_count: 0,
                            budget_alerts: 0,
                        });
                    entry.budget_alerts += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let per_category = self.per_category.lock().unwrap().clone();
        let snapshot = FinanceDriftSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 24,
            total_transactions: self.total_transactions.load(Ordering::Relaxed),
            over_budget_count: self.over_budget_count.load(Ordering::Relaxed),
            per_category,
        };

        self.repo.insert_finance_drift_snapshot(&snapshot).await?;

        // Reset counters
        self.per_category.lock().unwrap().clear();
        self.total_transactions.store(0, Ordering::Relaxed);
        self.over_budget_count.store(0, Ordering::Relaxed);

        Ok(())
    }
}
