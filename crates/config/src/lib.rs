//! Klyntbot Config - Configuration schema and loading
//!
//! This crate handles configuration schema definition and file I/O.

pub mod loader;
pub mod schema;

pub use loader::{
    config_dir, config_path, init, load, load_sync, load_with_env_overrides, save, save_sync,
};
pub use schema::{
    default_finance_categories, Config, DiscordConfig, EmailConfig, ExtendedThinkingConfig,
    FinanceBudgetingConfig, FinanceCategoryConfig, FinanceConfig, FinanceDefaultCategory,
    FinanceExpectedReturnsConfig, FinanceInflationConfig, FinancePriceRefreshConfig,
    FinanceSchedulingConfig, FireConfig, LearningConfig, McpConfig, McpOAuthCredentials,
    McpServerDef, McpServerSettings, McpTransport, OrchestratorConfig, PackTier, PacksConfig,
    PermissionsConfig, ProviderManagerConfig, Secret, SixJarRatios, SlackConfig, TelegramConfig,
    TodoEnrichmentConfig, TrustLevel, DEFAULT_STARTUP_TIMEOUT_SEC, DEFAULT_TOOL_TIMEOUT_SEC,
};
