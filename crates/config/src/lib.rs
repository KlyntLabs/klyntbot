//! Klyntbot Config - Configuration schema and loading
//!
//! This crate handles configuration schema definition and file I/O.

pub mod env;
pub mod loader;
pub mod schema;

pub use env::load_with_env_overrides;
pub use loader::{
    config_dir, config_path, init, load, load_sync, reload_if_changed, save, save_sync,
};
pub use schema::hot::{HotConfig, HotConfigDiff};
pub use schema::{
    AutoTunerConfig, BookEntityResolutionConfig, BookIndexConfig,
    BookRetrievalCfg, Config, ContentConfig, ContentSourceConfig, DiscordConfig, EmailConfig,
    ExecutionConfig, ExtendedThinkingConfig, LearningConfig,
    LifecycleConfig, McpAuthConfig, McpConfig, McpOAuthCredentials, McpServerDef,
    McpServerSettings, McpTransport, PackTier, PacksConfig, ProviderManagerConfig, Secret,
    ShortcutsConfig, SlackConfig, TelegramConfig, TodoEnrichmentConfig, TrustLevel,
    WakeDeliveryConfig, DEFAULT_STARTUP_TIMEOUT_SEC, DEFAULT_TOOL_TIMEOUT_SEC,
};
