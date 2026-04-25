//! Sensitivity filter — Phase 5 Task 8 fills in.

use crate::reforge::types::SerializableSemanticFact;

/// Drop facts with sensitivity `high` or `excluded` for externalization.
#[must_use]
pub fn filter_for_externalization(
    facts: &[SerializableSemanticFact],
) -> Vec<SerializableSemanticFact> {
    facts
        .iter()
        .filter(|f| !matches!(f.sensitivity.as_str(), "high" | "excluded"))
        .cloned()
        .collect()
}
