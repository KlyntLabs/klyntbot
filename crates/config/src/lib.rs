//! Klyntbot Config - Configuration schema and loading
//!
//! This crate handles configuration schema definition and file I/O.

pub mod env;
pub mod loader;
pub mod schema;

pub use env::load_with_env_overrides;
pub use loader::{config_dir, config_path, init, load, load_sync, save, save_sync};
pub use schema::{
    default_finance_categories, AutoTunerConfig, BookEntityResolutionConfig, BookIndexConfig,
    BookRetrievalCfg, Config, ContentConfig, ContentSourceConfig, DiscordConfig, EmailConfig,
    ExtendedThinkingConfig, FinanceBudgetingConfig, FinanceCategoryConfig, FinanceConfig,
    FinanceDefaultCategory, FinanceExpectedReturnsConfig, FinanceInflationConfig,
    FinancePriceRefreshConfig, FinanceSchedulingConfig, FireConfig, LearningConfig, McpAuthConfig,
    McpConfig, McpOAuthCredentials, McpServerDef, McpServerSettings, McpTransport,
    OrchestratorConfig, PackTier, PacksConfig, PermissionsConfig, ProviderManagerConfig, Secret,
    ShortcutsConfig, SixJarRatios, SlackConfig, TelegramConfig, TodoEnrichmentConfig, TrustLevel,
    DEFAULT_STARTUP_TIMEOUT_SEC, DEFAULT_TOOL_TIMEOUT_SEC,
};
