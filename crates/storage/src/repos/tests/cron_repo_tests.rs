//! Unit tests for CronRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn add_cron_job() {
        // Given: empty DB
        // When: add(id="daily-sync", name="Daily Sync", enabled=true, schedule=JSONB, payload=JSONB)
        // Then: cron_jobs has row
    }

    #[tokio::test]
    async fn list_enabled_jobs() {
        // Given: 2 enabled, 1 disabled job
        // When: list_enabled()
        // Then: returns 2 jobs
    }

    #[tokio::test]
    async fn update_next_run() {
        // Given: existing job
        // When: update_next_run(id, next_run_at_ms=1700000000000)
        // Then: field updated
    }

    #[tokio::test]
    async fn delete_job() {
        // Given: existing job
        // When: delete(id)
        // Then: get returns None
    }
}
