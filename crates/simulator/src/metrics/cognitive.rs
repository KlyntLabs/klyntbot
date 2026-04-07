//! Cognitive depth metrics: FSRS-5 retrievability decay, meta-rule counting.

/// Distribution of retrievability scores across all active facts.
pub struct RetrievabilityDistribution {
    pub avg: f64,
    pub min: f64,
    pub p25: f64,
    pub p50: f64,
}

/// Measure retrievability distribution of all active semantic facts.
///
/// Returns percentiles (min, p25, p50) alongside the average, giving visibility
/// into the tail — are *any* facts being forgotten, even if the average looks healthy?
pub async fn measure_retrievability_distribution(
    pool: &sqlx::SqlitePool,
    simulated_now: &str,
) -> RetrievabilityDistribution {
    let rows: Vec<(f64, f64)> = sqlx::query_as(
        "SELECT stability, CAST(strftime('%s', valid_from) AS REAL) \
         FROM semantic_facts \
         WHERE superseded_at IS NULL AND stability > 0",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return RetrievabilityDistribution {
            avg: 0.0,
            min: 0.0,
            p25: 0.0,
            p50: 0.0,
        };
    }

    let now = chrono::DateTime::parse_from_rfc3339(simulated_now)
        .unwrap_or_else(|_| chrono::Utc::now().into())
        .timestamp() as f64;

    let mut scores: Vec<f64> = rows
        .iter()
        .map(|&(stability, recorded_unix)| {
            let elapsed_days = ((now - recorded_unix) / 86400.0).max(0.0);
            cognitive::services::fsrs5::retrievability(elapsed_days, stability)
        })
        .collect();

    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = scores.len();
    let avg = scores.iter().sum::<f64>() / n as f64;
    let min = scores[0];
    let p25 = scores[n / 4];
    let p50 = scores[n / 2];

    RetrievabilityDistribution { avg, min, p25, p50 }
}

/// Measure average retrievability of all active semantic facts.
///
/// Uses the FSRS-5 stability and elapsed time to compute retrievability
/// for each fact, then returns the average. A score of 1.0 means all facts
/// are perfectly retrievable; a score near 0.0 means memory has decayed.
pub async fn measure_average_retrievability(pool: &sqlx::SqlitePool, simulated_now: &str) -> f64 {
    let dist = measure_retrievability_distribution(pool, simulated_now).await;
    if dist.avg == 0.0 && dist.min == 0.0 {
        // Preserve legacy behavior: 1.0 when no facts exist.
        let count: Result<(i64,), _> = sqlx::query_as(
            "SELECT COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL AND stability > 0",
        )
        .fetch_one(pool)
        .await;
        if count.map(|(n,)| n).unwrap_or(0) == 0 {
            return 1.0;
        }
    }
    dist.avg
}

/// Count the number of meta-rules that were proposed (pending or approved)
/// during the simulation.
pub async fn count_meta_rules(pool: &sqlx::SqlitePool) -> u32 {
    let count: Result<(i64,), _> = sqlx::query_as("SELECT COUNT(*) FROM mirror_meta_rules")
        .fetch_one(pool)
        .await;

    count.map(|(n,)| n as u32).unwrap_or(0)
}

/// Record a domain event's salience verdict on the metric accumulator.
pub fn record_salience(event: &bus::DomainEvent, acc: &mut super::EpochAccumulator) {
    match cognitive::services::salience::evaluate_salience(event) {
        cognitive::types::SalienceVerdict::Extract => acc.salience_extract += 1,
        cognitive::types::SalienceVerdict::Accumulate => acc.salience_accumulate += 1,
        cognitive::types::SalienceVerdict::Discard => acc.salience_discard += 1,
    }
}

/// Score a text against a reference answer using embedding cosine similarity.
///
/// Returns a score in [0, 1] where 1.0 = perfect semantic match.
/// Returns `None` if embedding fails (engine not available, etc.).
///
/// When `ref_embedding` is provided (pre-cached), skips re-embedding the reference.
pub fn score_response_quality(
    engine: &tools::EmbeddingEngine,
    text: &str,
    ref_embedding: Option<&[f32]>,
    reference: &str,
) -> Option<f64> {
    let text_emb = engine.embed(text).ok()?;
    let ref_emb = match ref_embedding {
        Some(cached) => cached.to_vec(),
        None => engine.embed(reference).ok()?,
    };
    Some(common::helpers::cosine_similarity(&text_emb, &ref_emb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retrievability_returns_one_for_empty() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let r = measure_average_retrievability(&pool, "2026-04-01T00:00:00Z").await;
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn meta_rules_returns_zero_on_missing_table() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let count = count_meta_rules(&pool).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn retrievability_distribution_with_facts() {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("pool");
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &cognitive::cognitive_migrations())
            .await
            .expect("migrations");
        std::mem::forget(pool);

        // Insert 4 facts with different stabilities and ages.
        let now = "2026-01-10T00:00:00Z";
        for (id, stab, recorded) in [
            ("f1", 10.0, "2026-01-09T00:00:00Z"), // 1 day, high stability
            ("f2", 1.0, "2026-01-01T00:00:00Z"),  // 9 days, low stability
            ("f3", 2.0, "2025-12-23T00:00:00Z"),  // 18 days, medium stability
            ("f4", 0.5, "2026-01-01T00:00:00Z"),  // 9 days, very low stability
        ] {
            sqlx::query(
                "INSERT INTO semantic_facts \
                 (id, domain, subject, predicate, object, confidence, source, \
                  valid_from, recorded_at, stability, access_count, memory_type, scope_type) \
                 VALUES (?1, 'test', 's', 'p', 'o', 0.9, 'sim', ?2, ?2, ?3, 0, 'semantic', 'session')",
            )
            .bind(id)
            .bind(recorded)
            .bind(stab)
            .execute(&inner)
            .await
            .unwrap();
        }

        let dist = measure_retrievability_distribution(&inner, now).await;
        assert!(dist.avg > 0.3 && dist.avg < 0.8, "avg={}", dist.avg);
        assert!(dist.min > 0.2 && dist.min < 0.5, "min={}", dist.min);
        assert!(dist.p25 > 0.3 && dist.p25 < 0.6, "p25={}", dist.p25);
    }

    #[test]
    fn score_response_quality_identical_texts() {
        let engine = tools::EmbeddingEngine::new();
        let score = score_response_quality(
            &engine,
            "Here are your tasks for today",
            None,
            "Here are your tasks for today",
        );
        // If embedding engine is available, identical texts score near 1.0
        // If not available (no semantic-search feature), score is None — OK
        if let Some(s) = score {
            assert!(s > 0.95, "identical texts should score > 0.95, got {s}");
        }
    }
}
