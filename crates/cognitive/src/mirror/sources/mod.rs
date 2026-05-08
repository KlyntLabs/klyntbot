//! Mirror signal sources — each implements `MirrorSignalSource` and is wired
//! to the workspace `SignalRouter` via `MirrorSubscriberRunner`.

pub mod approval_history;
pub mod coding_todo;
pub mod config_archiver;
pub mod cost_ceiling;
pub mod finance_drift;
pub mod meta_rule;
pub mod routing;
pub mod skill_effectiveness;
pub mod task_focus;
pub mod trial;

pub use approval_history::ApprovalHistorySource;
pub use coding_todo::TodoSignalSource;
pub use config_archiver::ConfigArchiverSource;
pub use cost_ceiling::CostCeilingSource;
pub use finance_drift::FinanceSpendingDriftSource;
pub use meta_rule::MetaRuleSignalSource;
pub use routing::RoutingSignalSource;
pub use skill_effectiveness::{EffectivenessScores, SkillEffectivenessSource};
pub use task_focus::TaskFocusPatternSource;
pub use trial::TrialPreviewSource;
