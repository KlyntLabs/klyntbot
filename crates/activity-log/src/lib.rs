pub mod context_action_repo;
pub mod context_resource_repo;
pub mod inference;
pub mod normalizers;
pub mod privacy;
pub mod repo;
pub mod resource_edge_repo;
pub mod service;
pub mod subscriber;
pub mod types;
pub mod work_context_repo;
pub mod work_resource_repo;

pub use normalizers::{
    normalize_domain_event, parse_rfc3339, ActivityNormalizer, ChatMessageInput,
    ChatMessageNormalizer, DomainEventNormalizer, ToolCallInput, ToolCallNormalizer,
    WindowEventInput, WindowEventNormalizer,
};
pub use context_action_repo::ContextActionRepo;
pub use context_resource_repo::ContextResourceRepo;
pub use privacy::PrivacyFilter;
pub use repo::ActivityLogRepo;
pub use resource_edge_repo::ResourceEdgeRepo;
pub use service::ActivityIngestionService;
pub use subscriber::ActivityLogSubscriber;
pub use work_context_repo::WorkContextRepo;
pub use work_resource_repo::WorkResourceRepo;
pub use types::{
    ActivityActor, ActivityLogEntry, ActivitySource, ContextAssignment, ResourceEdge,
    WorkContext, WorkContextStatus, WorkContextType, WorkResource, MAX_PREVIEW_LEN,
};

use tools_core::FeatureMigration;

pub struct ActivityLog;

impl ActivityLog {
    fn migration_sql() -> &'static str {
        include_str!("../migrations/001_unified_activity_log.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![
            FeatureMigration {
                feature_name: "activity_log".to_string(),
                version: 1,
                description: "Create unified activity log table".to_string(),
                sql: Self::migration_sql().to_string(),
            },
            FeatureMigration {
                feature_name: "activity_log".to_string(),
                version: 2,
                description: "Create work context tables".to_string(),
                sql: include_str!("../migrations/002_work_contexts.sql").to_string(),
            },
        ]
    }
}

#[cfg(test)]
pub(crate) async fn test_pool() -> storage::StoragePool {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &ActivityLog::migrations_static())
        .await
        .unwrap();
    pool
}
