//! Repository for the `insight_personas` and `insight_persona_pins` tables.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonaRow {
    pub id: String,
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub source: String,
    pub domains: String,
    pub is_active: i64,
    pub relevance_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewPersona {
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub domains: Vec<String>,
}

// ── Builtin definitions ─────────────────────────────────────────

struct BuiltinPersona {
    id: &'static str,
    name: &'static str,
    role: &'static str,
    expertise: &'static str,
    perspective: &'static str,
    tone: &'static str,
    icon: &'static str,
    domains: &'static str,
}

const BUILTINS: &[BuiltinPersona] = &[
    BuiltinPersona {
        id: "builtin-skeptic",
        name: "Skeptic",
        role: "Critical analyst",
        expertise: "Logical reasoning, bias detection, assumption challenging",
        perspective: "Questions claims and looks for weak spots in arguments",
        tone: "direct",
        icon: "🔍",
        domains: r#"["general","research"]"#,
    },
    BuiltinPersona {
        id: "builtin-practitioner",
        name: "Practitioner",
        role: "Implementation expert",
        expertise: "Real-world application, pragmatic solutions, operational knowledge",
        perspective: "Focuses on what actually works in practice",
        tone: "practical",
        icon: "🛠️",
        domains: r#"["engineering","operations"]"#,
    },
    BuiltinPersona {
        id: "builtin-connector",
        name: "Connector",
        role: "Cross-domain synthesizer",
        expertise: "Pattern recognition, interdisciplinary links, analogy finding",
        perspective: "Spots connections between seemingly unrelated ideas",
        tone: "curious",
        icon: "🔗",
        domains: r#"["general","research","creative"]"#,
    },
    BuiltinPersona {
        id: "builtin-student",
        name: "Student",
        role: "Curious learner",
        expertise: "Asking clarifying questions, simplification, knowledge gaps",
        perspective: "Approaches topics as a newcomer seeking understanding",
        tone: "inquisitive",
        icon: "📚",
        domains: r#"["general","learning"]"#,
    },
    BuiltinPersona {
        id: "builtin-strategist",
        name: "Strategist",
        role: "Long-term planner",
        expertise: "Strategic thinking, risk assessment, opportunity identification",
        perspective: "Evaluates decisions through a strategic lens",
        tone: "analytical",
        icon: "♟️",
        domains: r#"["business","planning","finance"]"#,
    },
    BuiltinPersona {
        id: "builtin-devils-advocate",
        name: "Devil's Advocate",
        role: "Contrarian challenger",
        expertise: "Counter-arguments, edge cases, stress-testing ideas",
        perspective: "Deliberately argues the opposing position to strengthen thinking",
        tone: "provocative",
        icon: "😈",
        domains: r#"["general","debate"]"#,
    },
];

// ── Repository ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PersonaRepo {
    pool: SqlitePool,
}

impl PersonaRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Seed all builtin personas. Idempotent — skips if already present.
    pub async fn seed_builtins(&self) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        for b in BUILTINS {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO insight_personas
                    (id, name, role, expertise, perspective, tone, icon, source, domains,
                     is_active, relevance_score, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'builtin', ?8, 1, 0.5, ?9, ?9)
                "#,
            )
            .bind(b.id)
            .bind(b.name)
            .bind(b.role)
            .bind(b.expertise)
            .bind(b.perspective)
            .bind(b.tone)
            .bind(b.icon)
            .bind(b.domains)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Create a user-defined persona.
    pub async fn create(&self, persona: &NewPersona) -> Result<PersonaRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let domains_json = serde_json::to_string(&persona.domains).unwrap_or_else(|_| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO insight_personas
                (id, name, role, expertise, perspective, tone, icon, source, domains,
                 is_active, relevance_score, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'user', ?8, 1, 0.5, ?9, ?9)
            "#,
        )
        .bind(&id)
        .bind(&persona.name)
        .bind(&persona.role)
        .bind(&persona.expertise)
        .bind(&persona.perspective)
        .bind(&persona.tone)
        .bind(&persona.icon)
        .bind(&domains_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }

    /// List all active personas.
    pub async fn list_active(&self) -> Result<Vec<PersonaRow>, sqlx::Error> {
        sqlx::query_as::<_, PersonaRow>(
            "SELECT * FROM insight_personas WHERE is_active = 1 ORDER BY source ASC, name ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get a persona by ID.
    pub async fn get(&self, id: &str) -> Result<Option<PersonaRow>, sqlx::Error> {
        sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Toggle a persona's active state.
    pub async fn set_active(&self, id: &str, active: bool) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE insight_personas SET is_active = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(active as i64)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete a persona. Returns error for builtin personas.
    pub async fn delete(&self, id: &str) -> Result<bool, sqlx::Error> {
        // Check if builtin
        let persona = self.get(id).await?;
        if let Some(p) = &persona {
            if p.source == "builtin" {
                return Err(sqlx::Error::Protocol(
                    "Cannot delete builtin persona".into(),
                ));
            }
        }

        let result = sqlx::query("DELETE FROM insight_personas WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Select personas for a note: pinned personas first, then domain matches, then defaults.
    /// Returns up to `limit` personas total.
    pub async fn select_for_note(
        &self,
        note_id: &str,
        note_domains: &[String],
        limit: usize,
    ) -> Result<Vec<PersonaRow>, sqlx::Error> {
        // 1. Get pinned persona IDs
        let pins = self.get_pins(note_id).await?;

        let mut result: Vec<PersonaRow> = Vec::new();

        // 2. Fetch pinned personas
        for pin_id in &pins {
            if let Some(p) = self.get(pin_id).await? {
                if p.is_active == 1 {
                    result.push(p);
                }
            }
        }

        if result.len() >= limit {
            result.truncate(limit);
            return Ok(result);
        }

        // 3. Domain match: active personas whose domains overlap with note_domains
        let all_active = self.list_active().await?;
        let pinned_ids: Vec<&str> = pins.iter().map(|s| s.as_str()).collect();

        for persona in &all_active {
            if result.len() >= limit {
                break;
            }
            if pinned_ids.contains(&persona.id.as_str()) {
                continue;
            }
            // Parse persona domains
            let persona_domains: Vec<String> =
                serde_json::from_str(&persona.domains).unwrap_or_default();
            let matches = note_domains
                .iter()
                .any(|d| persona_domains.iter().any(|pd| pd == d));
            if matches {
                result.push(persona.clone());
            }
        }

        if result.len() >= limit {
            result.truncate(limit);
            return Ok(result);
        }

        // 4. Fill remaining with any active personas not yet included
        let included_ids: Vec<String> = result.iter().map(|p| p.id.clone()).collect();
        for persona in &all_active {
            if result.len() >= limit {
                break;
            }
            if !included_ids.contains(&persona.id) {
                result.push(persona.clone());
            }
        }

        result.truncate(limit);
        Ok(result)
    }

    /// Get pinned persona IDs for a note.
    pub async fn get_pins(&self, note_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT persona_id FROM insight_persona_pins WHERE note_id = ?1 ORDER BY created_at ASC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Set pinned personas for a note (replaces existing pins).
    pub async fn set_pins(&self, note_id: &str, persona_ids: &[String]) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM insight_persona_pins WHERE note_id = ?1")
            .bind(note_id)
            .execute(&mut *tx)
            .await?;

        for pid in persona_ids {
            sqlx::query(
                "INSERT INTO insight_persona_pins (note_id, persona_id, created_at) VALUES (?1, ?2, ?3)",
            )
            .bind(note_id)
            .bind(pid)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_seed_builtins_and_list() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());

        repo.seed_builtins().await.unwrap();
        let active = repo.list_active().await.unwrap();
        assert_eq!(active.len(), 6);

        // Idempotent: seeding again should not duplicate
        repo.seed_builtins().await.unwrap();
        let active2 = repo.list_active().await.unwrap();
        assert_eq!(active2.len(), 6);

        // Verify known persona
        let skeptic = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert_eq!(skeptic.name, "Skeptic");
        assert_eq!(skeptic.source, "builtin");
    }

    #[tokio::test]
    async fn test_cannot_delete_builtin() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());

        repo.seed_builtins().await.unwrap();
        let result = repo.delete("builtin-skeptic").await;
        assert!(result.is_err());

        // Builtin should still be there
        let skeptic = repo.get("builtin-skeptic").await.unwrap();
        assert!(skeptic.is_some());
    }

    #[tokio::test]
    async fn test_select_with_defaults() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());

        repo.seed_builtins().await.unwrap();

        // Select for a note with "business" domain — strategist should be included
        let selected = repo
            .select_for_note("note-1", &["business".into()], 3)
            .await
            .unwrap();
        assert_eq!(selected.len(), 3);

        // Strategist has "business" in domains, should appear
        let names: Vec<&str> = selected.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Strategist"),
            "Expected Strategist in {names:?}"
        );
    }

    #[tokio::test]
    async fn test_pins_override_selection() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());

        repo.seed_builtins().await.unwrap();

        // Create a user persona
        let custom = repo
            .create(&NewPersona {
                name: "Custom Expert".into(),
                role: "Domain expert".into(),
                expertise: "Niche knowledge".into(),
                perspective: "Deep dive".into(),
                tone: "formal".into(),
                icon: "🎯".into(),
                domains: vec!["niche".into()],
            })
            .await
            .unwrap();

        // Pin the custom persona to a note
        repo.set_pins("note-1", std::slice::from_ref(&custom.id))
            .await
            .unwrap();

        // Verify pins
        let pins = repo.get_pins("note-1").await.unwrap();
        assert_eq!(pins, vec![custom.id.clone()]);

        // Select for the note — pinned persona should be first
        let selected = repo
            .select_for_note("note-1", &["general".into()], 3)
            .await
            .unwrap();
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].id, custom.id);

        // Can delete user persona
        let deleted = repo.delete(&custom.id).await.unwrap();
        assert!(deleted);
    }
}
