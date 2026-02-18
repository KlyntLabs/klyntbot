//! Unit tests for StrategyRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn record_strategy() {
        // Given: empty DB
        // When: record(predicted="direct", actual="direct", success=true)
        // Then: strategy_records has row with UUID PK
    }

    #[tokio::test]
    async fn accuracy_calculation() {
        // Given: 10 records for "react_plus" (7 where predicted==actual)
        // When: get_accuracy("react_plus", days=30)
        // Then: returns Some(0.7)
    }

    #[tokio::test]
    async fn accuracy_no_records_returns_none() {
        // Given: no records for "unknown_strategy"
        // When: get_accuracy("unknown_strategy", days=30)
        // Then: returns None
    }
}
