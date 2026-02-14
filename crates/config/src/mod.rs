//! Configuration module for klyntbot.

pub mod loader;
pub mod schema;

pub use loader::{
    config_dir, config_path, init, load, load_sync, load_with_env_overrides, save, save_sync,
};
pub use schema::{
    Config, DiscordConfig, EmailConfig, QQConfig, SlackConfig, TelegramConfig, WhatsAppConfig,
};
