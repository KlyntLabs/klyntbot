#![recursion_limit = "512"]
//! feature-finance: Self-contained personal finance feature package for klyntbot.
//!
//! Provides:
//! - `FinanceFeature`: implements `FeaturePackage` (tools, migrations, config, health)
//! - `FinanceTool`: unified tool with 40+ finance actions
//! - Domain types: 10 enums + 8 domain structs
//! - Config: `FinanceConfig` (inlined — no dep on config crate)
//! - Services: `PriceService` (live market data), `FinanceHandler` (proactive analysis)

pub mod config;
pub mod currency;
pub mod handler;
pub mod price_service;
pub mod rate_cache;
pub mod rebase;
pub mod tool;
pub mod types;

pub use config::FinanceConfig;
pub use handler::{BudgetAlert, FinanceHandler, PriceUpdateSummary, ProactivityLevel};
pub use price_service::PriceService;
pub use tool::FinanceTool;
pub use types::{
    AccountType, AssetType, BudgetMethod, BudgetPeriod, GoalStatus, GoalType, InvestmentTxType,
    JarType, LiabilityType, TransactionType,
};

// Re-export FeaturePackage so integration tests can bring it into scope without
// depending on tools-core directly.
pub use tools_core::FeaturePackage;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, HealthStatus};

/// Feature package for personal finance management.
///
/// Exposes one tool ("finance") covering accounts, transactions, budgets,
/// investments, goals, liabilities, FIRE planning, and reporting.
pub struct FinanceFeature {
    tool: Arc<FinanceTool>,
}

impl FinanceFeature {
    /// Create a new `FinanceFeature` wrapping a fully configured `FinanceTool`.
    pub fn new(tool: FinanceTool) -> Self {
        Self {
            tool: Arc::new(tool),
        }
    }

    /// Raw SQL for the first finance migration (all tables, idempotent DDL).
    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_finance_tables.sql")
    }

    /// Return the migrations list without needing a `self` reference.
    ///
    /// Useful in tests that want to inspect migrations without constructing
    /// a full `FinanceFeature` instance.
    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "finance".to_string(),
            version: 1,
            description: "Create finance tables: accounts, transactions, budgets, portfolios, \
                          investments, investment_txs, goals, liabilities"
                .to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    /// Return the default config value without needing a `self` reference.
    ///
    /// Useful in tests that want to inspect config shape without constructing
    /// a full `FinanceFeature` instance.
    pub fn default_config_static() -> Value {
        serde_json::to_value(FinanceConfig::default()).unwrap_or(Value::Null)
    }

    /// Construct a `FinanceFeature` backed by a temporary in-directory SQLite database.
    ///
    /// A single-threaded Tokio runtime is created internally to run the async
    /// `StoragePool::connect` call. Suitable for unit tests that exercise
    /// metadata (name, config, migrations) and do not need a persistent store.
    pub fn for_tests() -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build single-threaded tokio runtime for test pool");
        let pool = rt.block_on(async {
            let dir = tempfile::tempdir().expect("temp dir for test pool");
            let p = storage::StoragePool::connect(dir.path())
                .await
                .expect("SQLite test pool");
            // Keep dir alive by leaking — acceptable in test context.
            let _path = dir.keep();
            p
        });
        Self::new(FinanceTool::from_storage_pool(&pool, "USD"))
    }
}

#[async_trait]
impl FeaturePackage for FinanceFeature {
    fn name(&self) -> &str {
        "finance"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![self.tool.clone()]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
