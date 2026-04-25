//! Session-end light Reforge pass — filled in by Task 3.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Session-end light pass owned by `AppCore`.
#[derive(Debug, Default)]
pub struct SessionEndPass {
    _phase_stub: (),
}

impl SessionEndPass {
    /// Run one session-end pass for the given session id.
    pub async fn run(&self, _session_id: &str) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
