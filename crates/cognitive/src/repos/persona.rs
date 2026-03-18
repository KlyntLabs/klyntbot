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
    pub skill_path: Option<String>,
    pub questioning_style: String,
    pub cognitive_bias: String,
    pub analysis_frameworks: String, // JSON array
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
    pub questioning_style: Option<String>,
    pub cognitive_bias: Option<String>,
    pub analysis_frameworks: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct PersonaUpdate {
    pub name: Option<String>,
    pub role: Option<String>,
    pub expertise: Option<String>,
    pub perspective: Option<String>,
    pub tone: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
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
    questioning_style: &'static str,
    cognitive_bias: &'static str,
    analysis_frameworks: &'static str,
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
        questioning_style: "interrogative",
        cognitive_bias: "precision",
        analysis_frameworks: r#"["critical-analysis","evidence-evaluation"]"#,
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
        questioning_style: "practical",
        cognitive_bias: "pragmatism",
        analysis_frameworks: r#"["feasibility-analysis","implementation-mapping"]"#,
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
        questioning_style: "exploratory",
        cognitive_bias: "breadth",
        analysis_frameworks: r#"["cross-domain-synthesis","analogy-mapping"]"#,
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
        questioning_style: "naive",
        cognitive_bias: "simplicity",
        analysis_frameworks: r#"["socratic-method","first-principles"]"#,
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
        questioning_style: "strategic",
        cognitive_bias: "long-term",
        analysis_frameworks: r#"["swot-analysis","scenario-planning"]"#,
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
        questioning_style: "adversarial",
        cognitive_bias: "contrarian",
        analysis_frameworks: r#"["steelmanning","reductio-ad-absurdum"]"#,
    },
    BuiltinPersona {
        id: "builtin-academic-reviewer",
        name: "Academic Reviewer",
        role: "Peer reviewer",
        expertise: "Methodology critique, literature awareness, rigor assessment",
        perspective: "Reviews content with the rigor of academic peer review",
        tone: "formal",
        icon: "🎓",
        domains: r#"["research","academic"]"#,
        questioning_style: "methodological",
        cognitive_bias: "rigor",
        analysis_frameworks: r#"["peer-review","systematic-review"]"#,
    },
    BuiltinPersona {
        id: "builtin-methodologist",
        name: "Methodologist",
        role: "Methods specialist",
        expertise: "Research design, statistical validity, experimental rigor",
        perspective: "Focuses on the soundness of methods and processes",
        tone: "precise",
        icon: "🔬",
        domains: r#"["research","engineering"]"#,
        questioning_style: "systematic",
        cognitive_bias: "validity",
        analysis_frameworks: r#"["hypothesis-testing","experimental-design"]"#,
    },
    BuiltinPersona {
        id: "builtin-deep-analyst",
        name: "Deep Analyst",
        role: "Financial analyst",
        expertise: "DCF valuation, ratio analysis, financial modeling",
        perspective: "Digs deep into the numbers with rigorous quantitative analysis",
        tone: "rigorous",
        icon: "📊",
        domains: r#"["finance","productivity"]"#,
        questioning_style: "interrogative",
        cognitive_bias: "precision",
        analysis_frameworks: r#"["bottom-up-analysis","comparative-valuation"]"#,
    },
    BuiltinPersona {
        id: "builtin-risk-reviewer",
        name: "Risk Reviewer",
        role: "Risk assessor",
        expertise: "Risk identification, probability assessment, mitigation strategies",
        perspective: "Systematically identifies and evaluates risks across all dimensions",
        tone: "measured",
        icon: "⚠️",
        domains: r#"["finance","operations","planning"]"#,
        questioning_style: "cautious",
        cognitive_bias: "downside-protection",
        analysis_frameworks: r#"["risk-matrix","scenario-analysis"]"#,
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
                     questioning_style, cognitive_bias, analysis_frameworks,
                     is_active, relevance_score, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'builtin', ?8, ?9, ?10, ?11, 1, 0.5, ?12, ?12)
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
            .bind(b.questioning_style)
            .bind(b.cognitive_bias)
            .bind(b.analysis_frameworks)
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
        let questioning_style = persona.questioning_style.as_deref().unwrap_or("analytical");
        let cognitive_bias = persona.cognitive_bias.as_deref().unwrap_or("balanced");
        let analysis_frameworks_json = persona
            .analysis_frameworks
            .as_ref()
            .map(|f| serde_json::to_string(f).unwrap_or_else(|_| "[]".into()))
            .unwrap_or_else(|| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO insight_personas
                (id, name, role, expertise, perspective, tone, icon, source, domains,
                 questioning_style, cognitive_bias, analysis_frameworks,
                 is_active, relevance_score, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'user', ?8, ?9, ?10, ?11, 1, 0.5, ?12, ?12)
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
        .bind(questioning_style)
        .bind(cognitive_bias)
        .bind(&analysis_frameworks_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, PersonaRow>("SELECT * FROM insight_personas WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }

    /// Create an auto-generated persona (source = "auto").
    pub async fn create_auto(&self, persona: &NewPersona) -> Result<PersonaRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let domains_json = serde_json::to_string(&persona.domains).unwrap_or_else(|_| "[]".into());
        let questioning_style = persona.questioning_style.as_deref().unwrap_or("analytical");
        let cognitive_bias = persona.cognitive_bias.as_deref().unwrap_or("balanced");
        let analysis_frameworks_json = persona
            .analysis_frameworks
            .as_ref()
            .map(|f| serde_json::to_string(f).unwrap_or_else(|_| "[]".into()))
            .unwrap_or_else(|| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO insight_personas
                (id, name, role, expertise, perspective, tone, icon, source, domains,
                 questioning_style, cognitive_bias, analysis_frameworks,
                 is_active, relevance_score, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'auto', ?8, ?9, ?10, ?11, 1, 0.5, ?12, ?12)
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
        .bind(questioning_style)
        .bind(cognitive_bias)
        .bind(&analysis_frameworks_json)
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

    /// List all personas (including inactive), for the management UI.
    pub async fn list_all(&self) -> Result<Vec<PersonaRow>, sqlx::Error> {
        sqlx::query_as::<_, PersonaRow>(
            "SELECT * FROM insight_personas ORDER BY source ASC, name ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Update a non-builtin persona. Returns error for builtins.
    pub async fn update(
        &self,
        id: &str,
        updates: &PersonaUpdate,
    ) -> Result<Option<PersonaRow>, sqlx::Error> {
        let persona = self.get(id).await?;
        let Some(existing) = persona else {
            return Ok(None);
        };
        if existing.source == "builtin" {
            return Err(sqlx::Error::Protocol("Cannot edit builtin persona".into()));
        }

        let now = Utc::now().to_rfc3339();
        let name = updates.name.as_deref().unwrap_or(&existing.name);
        let role = updates.role.as_deref().unwrap_or(&existing.role);
        let expertise = updates.expertise.as_deref().unwrap_or(&existing.expertise);
        let perspective = updates
            .perspective
            .as_deref()
            .unwrap_or(&existing.perspective);
        let tone = updates.tone.as_deref().unwrap_or(&existing.tone);
        let icon = updates.icon.as_deref().unwrap_or(&existing.icon);
        let domains_json = if let Some(ref domains) = updates.domains {
            serde_json::to_string(domains).unwrap_or_else(|_| "[]".into())
        } else {
            existing.domains.clone()
        };

        sqlx::query(
            r#"
            UPDATE insight_personas
            SET name = ?1, role = ?2, expertise = ?3, perspective = ?4,
                tone = ?5, icon = ?6, domains = ?7, updated_at = ?8
            WHERE id = ?9
            "#,
        )
        .bind(name)
        .bind(role)
        .bind(expertise)
        .bind(perspective)
        .bind(tone)
        .bind(icon)
        .bind(&domains_json)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get(id).await
    }

    /// Adjust a persona's relevance score (for thumbs up/down feedback).
    pub async fn update_relevance(&self, id: &str, delta: f64) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE insight_personas
            SET relevance_score = MAX(0.0, MIN(1.0, relevance_score + ?1)),
                updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(delta)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
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
        assert_eq!(active.len(), 10);

        // Idempotent: seeding again should not duplicate
        repo.seed_builtins().await.unwrap();
        let active2 = repo.list_active().await.unwrap();
        assert_eq!(active2.len(), 10);

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
                questioning_style: None,
                cognitive_bias: None,
                analysis_frameworks: None,
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

    #[tokio::test]
    async fn test_list_all_includes_inactive() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();
        repo.set_active("builtin-skeptic", false).await.unwrap();
        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 10);
        let active = repo.list_active().await.unwrap();
        assert_eq!(active.len(), 9);
    }

    #[tokio::test]
    async fn test_update_user_persona() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();
        let custom = repo
            .create(&NewPersona {
                name: "Original".into(),
                role: "Tester".into(),
                expertise: "Testing".into(),
                perspective: "Test perspective".into(),
                tone: "neutral".into(),
                icon: "🧪".into(),
                domains: vec!["testing".into()],
                questioning_style: None,
                cognitive_bias: None,
                analysis_frameworks: None,
            })
            .await
            .unwrap();
        let updated = repo
            .update(
                &custom.id,
                &PersonaUpdate {
                    name: Some("Updated Name".into()),
                    role: None,
                    expertise: None,
                    perspective: None,
                    tone: Some("analytical".into()),
                    icon: None,
                    domains: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.role, "Tester");
        assert_eq!(updated.tone, "analytical");
    }

    #[tokio::test]
    async fn test_cannot_update_builtin() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();
        let result = repo
            .update(
                "builtin-skeptic",
                &PersonaUpdate {
                    name: Some("Hacked".into()),
                    role: None,
                    expertise: None,
                    perspective: None,
                    tone: None,
                    icon: None,
                    domains: None,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_relevance() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();
        let before = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((before.relevance_score - 0.5).abs() < f64::EPSILON);
        repo.update_relevance("builtin-skeptic", 0.1).await.unwrap();
        let after = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((after.relevance_score - 0.6).abs() < f64::EPSILON);
        repo.update_relevance("builtin-skeptic", 0.5).await.unwrap();
        let capped = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert!((capped.relevance_score - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_persona_skill_fields() {
        let pool = setup().await;
        let repo = PersonaRepo::new(pool.clone());
        repo.seed_builtins().await.unwrap();

        let persona = repo.get("builtin-skeptic").await.unwrap().unwrap();
        assert_eq!(persona.questioning_style, "interrogative");
        assert_eq!(persona.cognitive_bias, "precision");
        assert!(!persona.analysis_frameworks.is_empty());
    }
}
