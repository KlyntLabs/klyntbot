//! Channel adapter implementations for each chat platform.
//!
//! Each adapter implements the [`Channel`](crate::Channel) trait for a
//! specific messaging platform (Telegram, Discord, Slack, Email).

pub mod discord;
#[cfg(feature = "email")]
pub mod email;
pub mod slack;
pub mod telegram;
pub mod telegram_approval;

pub use discord::DiscordChannel;
#[cfg(feature = "email")]
pub use email::EmailChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use telegram_approval::TelegramApprovalChannel;
