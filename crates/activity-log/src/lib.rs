pub mod normalizers;
pub mod privacy;
pub mod repo;
pub mod service;
pub mod subscriber;
pub mod types;

pub use normalizers::{
    normalize_domain_event, ActivityNormalizer, ChatMessageInput, ChatMessageNormalizer,
    DomainEventNormalizer, ToolCallInput, ToolCallNormalizer, WindowEventInput,
    WindowEventNormalizer,
};
pub use privacy::PrivacyFilter;
pub use repo::ActivityLogRepo;
pub use service::ActivityIngestionService;
pub use subscriber::ActivityLogSubscriber;
pub use types::{ActivityActor, ActivityLogEntry, ActivitySource, MAX_PREVIEW_LEN};

use tools_core::FeatureMigration;

pub struct ActivityLog;

impl ActivityLog {
    fn migration_sql() -> &'static str {
        include_str!("../migrations/001_unified_activity_log.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "activity_log".to_string(),
            version: 1,
            description: "Create unified activity log table".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }
}
