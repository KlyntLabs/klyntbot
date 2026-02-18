//! Unit tests for OutcomeRepo (learning outcomes + enrichment feedback).

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn record_outcome() {
        // Given: empty DB
        // When: record(tool_name="todo", success=true, duration_ms=150)
        // Then: learning_outcomes has row
    }

    #[tokio::test]
    async fn record_enrichment_feedback() {
        // Given: empty DB
        // When: record_feedback(task_id="t001", field="priority", accepted=true, confidence=0.85)
        // Then: enrichment_feedback has row
    }

    #[tokio::test]
    async fn outcomes_since_filters_by_date() {
        // Given: outcomes from 7 days ago and today
        // When: outcomes_since(3 days ago)
        // Then: returns only today's outcomes
    }

    #[tokio::test]
    async fn get_all_outcomes() {
        // Given: 5 outcome records
        // When: get_all()
        // Then: returns all 5
    }
}
