//! Event-driven subscribers for the Mirror self-reflection layer.

pub mod meta_rule;
pub mod routing;
pub mod version;
pub use meta_rule::MetaRuleDetector;
pub use routing::RoutingMirrorSubscriber;
pub use version::ConfigArchiver;
