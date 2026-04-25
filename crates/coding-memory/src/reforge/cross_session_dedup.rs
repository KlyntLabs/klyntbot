//! Phase 6.5 cross-session fact dedup. Filled in by Task 9.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Cross-session dedup pass.
#[derive(Debug, Default)]
pub struct CrossSessionDedup;

impl CrossSessionDedup {
    /// Run the pass.
    pub async fn run(_repo: &cognitive::SemanticFactRepo) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
