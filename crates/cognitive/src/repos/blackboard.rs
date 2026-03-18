//! Repository for the `blackboard_entries` table — transient shared working memory for debate rounds.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlackboardEntry {
    pub id: String,
    pub session_key: String,
    pub squad_id: String,
    pub round: i64,
    pub persona_id: String,
    pub persona_name: String,
    pub entry_type: String,
    pub content: String,
    pub confidence: f64,
    pub references_entry_id: Option<String>,
    pub created_at: String,
}

pub struct NewBlackboardEntry<'a> {
    pub session_key: &'a str,
    pub squad_id: &'a str,
    pub round: i64,
    pub persona_id: &'a str,
    pub persona_name: &'a str,
    pub entry_type: &'a str,
    pub content: &'a str,
    pub confidence: f64,
    pub references_entry_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct BlackboardRepo {
    pool: SqlitePool,
}

impl BlackboardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        entry: &NewBlackboardEntry<'_>,
    ) -> Result<BlackboardEntry, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query_as::<_, BlackboardEntry>(
            "INSERT INTO blackboard_entries (id, session_key, squad_id, round, persona_id, persona_name, entry_type, content, confidence, references_entry_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             RETURNING *",
        )
        .bind(&id)
        .bind(entry.session_key)
        .bind(entry.squad_id)
        .bind(entry.round)
        .bind(entry.persona_id)
        .bind(entry.persona_name)
        .bind(entry.entry_type)
        .bind(entry.content)
        .bind(entry.confidence)
        .bind(entry.references_entry_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_for_round(
        &self,
        session_key: &str,
        round: i64,
    ) -> Result<Vec<BlackboardEntry>, sqlx::Error> {
        sqlx::query_as::<_, BlackboardEntry>(
            "SELECT * FROM blackboard_entries WHERE session_key = ?1 AND round = ?2 ORDER BY created_at",
        )
        .bind(session_key)
        .bind(round)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_for_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<BlackboardEntry>, sqlx::Error> {
        sqlx::query_as::<_, BlackboardEntry>(
            "SELECT * FROM blackboard_entries WHERE session_key = ?1 ORDER BY round, created_at",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await
    }

    /// Build a formatted blackboard context string for persona prompts.
    pub fn format_for_prompt(entries: &[BlackboardEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }
        let mut rounds: std::collections::BTreeMap<i64, Vec<&BlackboardEntry>> =
            std::collections::BTreeMap::new();
        for e in entries {
            rounds.entry(e.round).or_default().push(e);
        }
        let mut out = String::from("\n\n--- Prior Debate Rounds ---\n");
        for (round, entries) in &rounds {
            out.push_str(&format!("\n## Round {round}\n"));
            for e in entries {
                out.push_str(&format!(
                    "**{}** ({}): {}\n",
                    e.persona_name, e.entry_type, e.content
                ));
            }
        }
        out
    }

    pub async fn clear_session(&self, session_key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM blackboard_entries WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blackboard_crud() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = BlackboardRepo::new(pool);

        let entry = NewBlackboardEntry {
            session_key: "test-session",
            squad_id: "builtin-squad-finance",
            round: 1,
            persona_id: "builtin-deep-analyst",
            persona_name: "Deep Analyst",
            entry_type: "observation",
            content: "Index funds outperform 80% of active managers over 15 years.",
            confidence: 0.95,
            references_entry_id: None,
        };
        let row = repo.insert(&entry).await.unwrap();
        assert_eq!(row.round, 1);
        assert_eq!(row.entry_type, "observation");

        let entries = repo.list_for_round("test-session", 1).await.unwrap();
        assert_eq!(entries.len(), 1);

        let all = repo.list_for_session("test-session").await.unwrap();
        assert_eq!(all.len(), 1);

        repo.clear_session("test-session").await.unwrap();
        assert!(repo
            .list_for_session("test-session")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn test_blackboard_multi_round() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = BlackboardRepo::new(pool);

        // Round 1: two personas observe
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1",
            squad_id: "sq1",
            round: 1,
            persona_id: "p1",
            persona_name: "Analyst",
            entry_type: "observation",
            content: "Claim A",
            confidence: 0.9,
            references_entry_id: None,
        })
        .await
        .unwrap();
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1",
            squad_id: "sq1",
            round: 1,
            persona_id: "p2",
            persona_name: "Skeptic",
            entry_type: "challenge",
            content: "Challenge A",
            confidence: 0.7,
            references_entry_id: None,
        })
        .await
        .unwrap();

        // Round 2: response
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1",
            squad_id: "sq1",
            round: 2,
            persona_id: "p1",
            persona_name: "Analyst",
            entry_type: "claim",
            content: "Revised A with evidence",
            confidence: 0.85,
            references_entry_id: None,
        })
        .await
        .unwrap();

        let round1 = repo.list_for_round("debate-1", 1).await.unwrap();
        assert_eq!(round1.len(), 2);
        let round2 = repo.list_for_round("debate-1", 2).await.unwrap();
        assert_eq!(round2.len(), 1);
        let all = repo.list_for_session("debate-1").await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_format_for_prompt() {
        let entries = vec![
            BlackboardEntry {
                id: "1".into(),
                session_key: "s".into(),
                squad_id: "sq".into(),
                round: 1,
                persona_id: "p1".into(),
                persona_name: "Analyst".into(),
                entry_type: "observation".into(),
                content: "Claim A".into(),
                confidence: 0.9,
                references_entry_id: None,
                created_at: "now".into(),
            },
            BlackboardEntry {
                id: "2".into(),
                session_key: "s".into(),
                squad_id: "sq".into(),
                round: 2,
                persona_id: "p2".into(),
                persona_name: "Skeptic".into(),
                entry_type: "challenge".into(),
                content: "Challenge A".into(),
                confidence: 0.7,
                references_entry_id: None,
                created_at: "now".into(),
            },
        ];
        let prompt = BlackboardRepo::format_for_prompt(&entries);
        assert!(prompt.contains("Prior Debate Rounds"));
        assert!(prompt.contains("Round 1"));
        assert!(prompt.contains("Round 2"));
        assert!(prompt.contains("**Analyst** (observation)"));
    }

    #[test]
    fn test_format_for_prompt_empty() {
        assert!(BlackboardRepo::format_for_prompt(&[]).is_empty());
    }
}
