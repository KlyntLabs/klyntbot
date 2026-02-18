//! Unit tests for UsageRepo (cost tracking).

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn record_usage() {
        // Given: empty DB
        // When: record(model="claude-opus", provider="anthropic", prompt_tokens=500, completion_tokens=200, cost=0.015)
        // Then: usage_records has row with UUID PK
    }

    #[tokio::test]
    async fn report_aggregates_by_model() {
        // Given: 5 records for claude-opus, 3 for gpt-4o
        // When: report(days=30)
        // Then: by_model has 2 entries with correct sums
    }

    #[tokio::test]
    async fn report_aggregates_by_date() {
        // Given: records across 3 different dates
        // When: report(days=30)
        // Then: by_date has 3 entries with correct daily totals
    }
}
