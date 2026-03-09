use sqlx::SqlitePool;

use crate::types::Forecast;

#[derive(Debug, Clone)]
pub struct ForecastRepo {
    pool: SqlitePool,
}

impl ForecastRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, forecast: &Forecast) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_forecasts (id, forecast_date, forecast_type, window_start, window_end, predicted_value, confidence, stability, auto_protected, user_overrode, actual_value, prediction_error, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        )
        .bind(&forecast.id)
        .bind(&forecast.forecast_date)
        .bind(&forecast.forecast_type)
        .bind(&forecast.window_start)
        .bind(&forecast.window_end)
        .bind(forecast.predicted_value)
        .bind(forecast.confidence)
        .bind(forecast.stability)
        .bind(forecast.auto_protected)
        .bind(forecast.user_overrode)
        .bind(forecast.actual_value)
        .bind(forecast.prediction_error)
        .bind(&forecast.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_for_date(&self, date: &str) -> common::Result<Vec<Forecast>> {
        let rows = sqlx::query_as::<_, Forecast>(
            "SELECT * FROM productivity_forecasts WHERE forecast_date = ?1 ORDER BY created_at DESC",
        )
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn update_actual(&self, id: &str, actual_value: f64) -> common::Result<()> {
        sqlx::query(
            r#"UPDATE productivity_forecasts SET
                   actual_value = ?2,
                   prediction_error = ABS(predicted_value - ?2)
               WHERE id = ?1"#,
        )
        .bind(id)
        .bind(actual_value)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_pending_evaluation(&self) -> common::Result<Vec<Forecast>> {
        let rows = sqlx::query_as::<_, Forecast>(
            "SELECT * FROM productivity_forecasts WHERE actual_value IS NULL AND forecast_date < date('now') ORDER BY forecast_date DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn sample_forecast(id: &str, date: &str) -> Forecast {
        Forecast {
            id: id.to_string(),
            forecast_date: date.to_string(),
            forecast_type: "energy".to_string(),
            window_start: Some("09:00".to_string()),
            window_end: Some("12:00".to_string()),
            predicted_value: 0.85,
            confidence: 0.7,
            stability: 0.9,
            auto_protected: false,
            user_overrode: false,
            actual_value: None,
            prediction_error: None,
            created_at: "2026-03-09T08:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_and_list_for_date() {
        let pool = setup_pool().await;
        let repo = ForecastRepo::new(pool);

        let f1 = sample_forecast("fc-1", "2026-03-09");
        let f2 = Forecast {
            id: "fc-2".to_string(),
            forecast_type: "focus_window".to_string(),
            ..sample_forecast("fc-2", "2026-03-09")
        };
        let f3 = sample_forecast("fc-3", "2026-03-10");

        repo.create(&f1).await.unwrap();
        repo.create(&f2).await.unwrap();
        repo.create(&f3).await.unwrap();

        let results = repo.list_for_date("2026-03-09").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_update_actual() {
        let pool = setup_pool().await;
        let repo = ForecastRepo::new(pool);

        let f = sample_forecast("fc-actual-1", "2026-03-09");
        repo.create(&f).await.unwrap();

        repo.update_actual("fc-actual-1", 0.75).await.unwrap();

        let results = repo.list_for_date("2026-03-09").await.unwrap();
        assert_eq!(results.len(), 1);
        let updated = &results[0];
        assert_eq!(updated.actual_value, Some(0.75));
        // prediction_error = |0.85 - 0.75| = 0.10
        assert!(updated.prediction_error.is_some());
        assert!((updated.prediction_error.unwrap() - 0.10).abs() < 0.01);
    }
}
