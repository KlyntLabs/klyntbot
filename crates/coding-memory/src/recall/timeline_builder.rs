//! `TimelineEntry` builder — orders by `when` descending; carries `related_ids`.

use crate::recall::TimelineEntry;

/// Pre-built input row for the timeline (decoupled from cognitive types so
/// the service layer can populate from facts, episodes, or a join).
#[derive(Debug, Clone)]
pub struct TimelineInput {
    /// Memory id.
    pub id: uuid::Uuid,
    /// Kind label.
    pub kind: String,
    /// Timestamp.
    pub when: jiff::Timestamp,
    /// Snippet text.
    pub snippet: String,
    /// Pre-resolved related ids.
    pub related_ids: Vec<uuid::Uuid>,
}

/// Timeline builder.
#[derive(Debug, Default, Clone, Copy)]
pub struct TimelineBuilder;

impl TimelineBuilder {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build entries from a list of inputs, sorted newest first.
    #[must_use]
    pub fn build(self, mut inputs: Vec<TimelineInput>) -> Vec<TimelineEntry> {
        inputs.sort_by(|a, b| b.when.cmp(&a.when));
        inputs
            .into_iter()
            .map(|i| TimelineEntry {
                id: i.id,
                kind: i.kind,
                when: i.when,
                snippet: common::helpers::truncate_chars(&i.snippet, 240, "…"),
                related_ids: i.related_ids,
            })
            .collect()
    }
}
