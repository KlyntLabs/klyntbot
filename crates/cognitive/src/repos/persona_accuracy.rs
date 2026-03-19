//! Repository for the `persona_accuracy` table — FSRS-based persona learning from debate outcomes.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::services::fsrs5;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonaAccuracy {
    pub id: String,
    pub persona_id: String,
    pub squad_id: String,
    pub domain: String,
    pub total_debates: i64,
    pub consensus_hits: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub last_debate_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl PersonaAccuracy {
    pub fn accuracy_rate(&self) -> f64 {
        if self.total_debates == 0 {
            0.0
        } else {
            self.consensus_hits as f64 / self.total_debates as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersonaAccuracyRepo {
    pool: SqlitePool,
}

impl PersonaAccuracyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(
        &self,
        persona_id: &str,
        squad_id: &str,
        domain: &str,
    ) -> Result<Option<PersonaAccuracy>, sqlx::Error> {
        sqlx::query_as::<_, PersonaAccuracy>(
            "SELECT * FROM persona_accuracy WHERE persona_id = ?1 AND squad_id = ?2 AND domain = ?3",
        )
        .bind(persona_id)
        .bind(squad_id)
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
    }

    /// Record the outcome of a debate for a persona.
    /// `in_consensus` = true if this persona's final position aligned with the group consensus.
    pub async fn record_outcome(
        &self,
        persona_id: &str,
        squad_id: &str,
        domain: &str,
        in_consensus: bool,
    ) -> Result<PersonaAccuracy, sqlx::Error> {
        let existing = self.get(persona_id, squad_id, domain).await?;

        let (total, hits, old_stability, old_difficulty) = match &existing {
            Some(a) => (a.total_debates, a.consensus_hits, a.stability, a.difficulty),
            None => (0, 0, 1.0, 5.0),
        };

        let new_total = total + 1;
        let new_hits = if in_consensus { hits + 1 } else { hits };

        // FSRS update: treat consensus hit as "Good" (3), miss as "Again" (1)
        let rating = if in_consensus { 3 } else { 1 };
        let w = fsrs5::DEFAULT_WEIGHTS;
        let new_stability = if total == 0 {
            fsrs5::initial_stability(rating, &w)
        } else {
            let elapsed = 1.0; // simplified: 1 day equivalent per debate
            let r = fsrs5::retrievability(elapsed, old_stability);
            if in_consensus {
                fsrs5::next_stability_success(old_stability, old_difficulty, r, rating, &w)
            } else {
                fsrs5::next_stability_failure(old_stability, old_difficulty, r, &w)
            }
        };
        let new_difficulty = if total == 0 {
            fsrs5::initial_difficulty(rating, &w)
        } else {
            fsrs5::next_difficulty(old_difficulty, rating, &w)
        };

        let id = existing
            .map(|a| a.id)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        sqlx::query_as::<_, PersonaAccuracy>(
            "INSERT INTO persona_accuracy (id, persona_id, squad_id, domain, total_debates, consensus_hits, stability, difficulty, last_debate_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))
             ON CONFLICT (persona_id, squad_id, domain) DO UPDATE SET
               total_debates = ?5, consensus_hits = ?6, stability = ?7, difficulty = ?8,
               last_debate_at = datetime('now'), updated_at = datetime('now')
             RETURNING *",
        )
        .bind(&id)
        .bind(persona_id)
        .bind(squad_id)
        .bind(domain)
        .bind(new_total)
        .bind(new_hits)
        .bind(new_stability)
        .bind(new_difficulty)
        .fetch_one(&self.pool)
        .await
    }

    /// List accuracy records for a persona across all squads.
    pub async fn list_for_persona(
        &self,
        persona_id: &str,
    ) -> Result<Vec<PersonaAccuracy>, sqlx::Error> {
        sqlx::query_as::<_, PersonaAccuracy>(
            "SELECT * FROM persona_accuracy WHERE persona_id = ?1 ORDER BY updated_at DESC",
        )
        .bind(persona_id)
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_debate_outcome() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = PersonaAccuracyRepo::new(pool);

        // Record a successful debate
        repo.record_outcome(
            "builtin-deep-analyst",
            "builtin-squad-finance",
            "finance",
            true,
        )
        .await
        .unwrap();

        let acc = repo
            .get("builtin-deep-analyst", "builtin-squad-finance", "finance")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(acc.total_debates, 1);
        assert_eq!(acc.consensus_hits, 1);
        assert!(acc.stability > 1.0);

        // Record a miss
        repo.record_outcome(
            "builtin-deep-analyst",
            "builtin-squad-finance",
            "finance",
            false,
        )
        .await
        .unwrap();
        let acc = repo
            .get("builtin-deep-analyst", "builtin-squad-finance", "finance")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(acc.total_debates, 2);
        assert_eq!(acc.consensus_hits, 1);
    }

    #[test]
    fn test_accuracy_rate() {
        let acc = PersonaAccuracy {
            id: "1".into(),
            persona_id: "p".into(),
            squad_id: "s".into(),
            domain: "d".into(),
            total_debates: 10,
            consensus_hits: 7,
            stability: 2.0,
            difficulty: 5.0,
            last_debate_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!((acc.accuracy_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_accuracy_rate_zero_debates() {
        let acc = PersonaAccuracy {
            id: "1".into(),
            persona_id: "p".into(),
            squad_id: "s".into(),
            domain: "d".into(),
            total_debates: 0,
            consensus_hits: 0,
            stability: 1.0,
            difficulty: 5.0,
            last_debate_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!((acc.accuracy_rate()).abs() < f64::EPSILON);
    }
}
