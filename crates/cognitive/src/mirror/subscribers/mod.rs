//! Event-driven subscribers for the Mirror self-reflection layer.

pub mod meta_rule;
pub mod routing;
pub mod schema;
pub mod trial;
pub mod version;
pub mod workspace_skill;
pub use meta_rule::MetaRuleDetector;
pub use routing::RoutingMirrorSubscriber;
pub use schema::SchemaMirrorSubscriber;
pub use trial::TrialPreviewSubscriber;
pub use version::ConfigArchiver;
pub use workspace_skill::WorkspaceSkillSubscriber;
