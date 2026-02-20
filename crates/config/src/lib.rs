//! Klyntbot Config - Configuration schema and loading
//!
//! This crate handles configuration schema definition and file I/O.

pub mod loader;
pub mod schema;

pub use loader::{
    config_dir, config_path, init, load, load_sync, load_with_env_overrides, save, save_sync,
};
pub use schema::{
    AppleCalendarConfig, CalendarConfig, CalendarProviderConfig, Config, CreationMode,
    DiscordConfig, EmailConfig, ExtendedThinkingConfig, FinanceBudgetingConfig,
    FinanceCategoryConfig, FinanceConfig, FinanceExpectedReturnsConfig, FinanceInflationConfig,
    FinancePriceRefreshConfig, FinanceSchedulingConfig, GenericCalDavConfig, GoogleCalendarConfig,
    LearningConfig, PackTier, PacksConfig, PermissionsConfig, ProviderManagerConfig, QQConfig,
    Secret, SixJarRatios, SlackConfig, TelegramConfig, TodoEnrichmentConfig, WhatsAppConfig,
};
