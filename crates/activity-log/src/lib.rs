pub mod consumer;
pub mod context_action_repo;
pub mod context_resource_repo;
pub mod context_source;
pub mod inference;
pub mod inference_loop;
pub mod normalizers;
pub mod privacy;
pub mod repo;
pub mod resource_edge_repo;
pub mod service;

pub mod types;
pub mod work_context_repo;
pub mod work_context_tool;
pub mod work_resource_repo;

pub use consumer::NormalizerSignalConsumer;
pub use context_action_repo::ContextTaskRepo;
pub use context_resource_repo::ContextResourceRepo;
pub use context_source::WorkContextSource;
pub use normalizers::{
    parse_rfc3339, ActivityNormalizer, ChatMessageInput, ChatMessageNormalizer, ToolCallInput,
    ToolCallNormalizer, WindowEventInput, WindowEventNormalizer,
};
pub use privacy::PrivacyFilter;
pub use repo::ActivityLogRepo;
pub use resource_edge_repo::ResourceEdgeRepo;
pub use service::{ActivityIngestionService, BatchIngestionService};

pub use types::{
    ActivityActor, ActivityLogEntry, ActivitySource, ContextAssignment, ResourceEdge, WorkContext,
    WorkContextStatus, WorkContextType, WorkResource, MAX_PREVIEW_LEN,
};
pub use work_context_repo::WorkContextRepo;
pub use work_context_tool::WorkContextTool;
pub use work_resource_repo::WorkResourceRepo;

use tools_core::FeatureMigration;

pub struct ActivityLog;

impl ActivityLog {
    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "activity_log".to_string(),
            version: 1,
            description: "Activity log tables".to_string(),
            sql: include_str!("../migrations/001_unified_activity_log.sql").to_string(),
        }]
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
