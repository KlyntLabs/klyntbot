//! Repository for `squads` and `squad_members` tables.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[cfg(test)]
use super::persona::PersonaRepo;
use super::persona::PersonaRow;

// ── Row types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SquadRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub source: String,
    pub domains: String, // JSON array
    pub is_active: i64,
    pub default_interaction_mode: String, // "lead" | "debate" | "smart"
    pub last_smart_mode: Option<String>,
    pub last_smart_updated: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SquadMemberRow {
    pub squad_id: String,
    pub persona_id: String,
    pub role_in_squad: String,
    pub sort_order: i64,
}

pub struct NewSquad {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub domains: Vec<String>,
}

pub struct ResolvedSquad {
    pub squad: SquadRow,
    pub personas: Vec<PersonaRow>,
    pub members: Vec<SquadMemberRow>,
}

// ── Builtin definitions ─────────────────────────────────────────

struct BuiltinSquad {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    orchestrator_skill: &'static str,
    domains: &'static str,
    members: &'static [(&'static str, &'static str, i64)], // (persona_id, role, sort_order)
}

const BUILTIN_SQUADS: &[BuiltinSquad] = &[
    BuiltinSquad {
        id: "builtin-squad-general",
        name: "General Analysis",
        description: "Balanced analytical team for general-purpose insight review",
        icon: "\u{1f50d}",
        orchestrator_skill: "general",
        domains: r#"["general","research"]"#,
        members: &[
            ("builtin-skeptic", "lead", 0),
            ("builtin-connector", "member", 1),
            ("builtin-student", "member", 2),
            ("builtin-devils-advocate", "member", 3),
        ],
    },
    BuiltinSquad {
        id: "builtin-squad-research",
        name: "Research & Academic",
        description: "Research-focused team for academic and scientific content review",
        icon: "\u{1f393}",
        orchestrator_skill: "general",
        domains: r#"["research","academic"]"#,
        members: &[
            ("builtin-skeptic", "lead", 0),
            ("builtin-academic-reviewer", "member", 1),
            ("builtin-methodologist", "member", 2),
            ("builtin-student", "member", 3),
        ],
    },
    BuiltinSquad {
        id: "builtin-squad-finance",
        name: "Finance Analysis",
        description: "Financial analysis team for investment and business decisions",
        icon: "\u{1f4b0}",
        orchestrator_skill: "finance-management",
        domains: r#"["finance","business"]"#,
        members: &[
            ("builtin-deep-analyst", "lead", 0),
            ("builtin-risk-reviewer", "member", 1),
            ("builtin-strategist", "member", 2),
        ],
    },
    BuiltinSquad {
        id: "builtin-squad-strategy",
        name: "Strategy & Planning",
        description: "Strategic planning team for business decisions and long-term planning",
        icon: "\u{265f}\u{fe0f}",
        orchestrator_skill: "task-management",
        domains: r#"["business","planning"]"#,
        members: &[
            ("builtin-strategist", "lead", 0),
            ("builtin-practitioner", "member", 1),
            ("builtin-devils-advocate", "member", 2),
        ],
    },
];

/// Combined row for the squad member + persona JOIN query.
#[derive(Debug, Clone, sqlx::FromRow)]
struct MemberPersonaJoinRow {
    // persona fields
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
    pub analysis_frameworks: String,
    pub created_at: String,
    pub updated_at: String,
    // member fields
    pub role_in_squad: String,
    pub sort_order: i64,
}

// ── Smart mode helpers ──────────────────────────────────────────

/// Interaction counts used to compute the effective smart-mode default.
/// Callers supply these counts (e.g. from session message queries) rather than
/// letting `SquadRepo` query session storage directly.
#[derive(Debug, Clone)]
pub struct InteractionCounts {
    /// Messages delivered directly by the lead persona without multi-voice debate.
    pub direct_count: u64,
    /// Messages in lead / single-voice mode.
    pub lead_count: u64,
    /// Messages in debate / multi-voice mode.
    pub debate_count: u64,
}

/// Returns `true` when `last_smart_updated` is an ISO-8601 / SQLite datetime
/// string that falls within the last 24 hours.
fn is_cache_fresh(updated: &str) -> bool {
    use chrono::{DateTime, Duration, Utc};
    // Accept both RFC-3339 ("…T…Z") and SQLite datetime ("… ") formats
    let parsed = updated.parse::<DateTime<Utc>>().or_else(|_| {
        // SQLite stores datetimes as "YYYY-MM-DD HH:MM:SS"; append " UTC" for parsing
        format!("{updated} UTC").parse::<DateTime<Utc>>()
    });
    match parsed {
        Ok(dt) => Utc::now() - dt < Duration::days(1),
        Err(_) => false,
    }
}

// ── Repository ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SquadRepo {
    pool: SqlitePool,
}

impl SquadRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a user-defined squad.
    pub async fn create(&self, squad: &NewSquad) -> Result<SquadRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let domains_json = serde_json::to_string(&squad.domains).unwrap_or_else(|_| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO squads
                (id, name, description, icon, orchestrator_skill, source, domains,
                 is_active, default_interaction_mode, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6, 1, 'lead', ?7, ?7)
            "#,
        )
        .bind(&id)
        .bind(&squad.name)
        .bind(&squad.description)
        .bind(&squad.icon)
        .bind(&squad.orchestrator_skill)
        .bind(&domains_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, SquadRow>("SELECT * FROM squads WHERE id = ?1")
            .bind(&id)
            .fetch_one(&self.pool)
            .await
    }

    /// Get a squad by ID.
    pub async fn get(&self, id: &str) -> Result<Option<SquadRow>, sqlx::Error> {
        sqlx::query_as::<_, SquadRow>("SELECT * FROM squads WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// List all active squads.
    pub async fn list(&self) -> Result<Vec<SquadRow>, sqlx::Error> {
        sqlx::query_as::<_, SquadRow>(
            "SELECT * FROM squads WHERE is_active = 1 ORDER BY source ASC, name ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Update a non-builtin squad. Returns error for builtins.
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        icon: Option<&str>,
        domains: Option<&[String]>,
        orchestrator_skill: Option<&str>,
        default_interaction_mode: Option<&str>,
    ) -> Result<Option<SquadRow>, sqlx::Error> {
        let squad = self.get(id).await?;
        let Some(existing) = squad else {
            return Ok(None);
        };
        if existing.source == "builtin" {
            return Err(sqlx::Error::Protocol("Cannot edit builtin squad".into()));
        }

        let now = Utc::now().to_rfc3339();
        let name = name.unwrap_or(&existing.name);
        let description = description.unwrap_or(&existing.description);
        let icon = icon.unwrap_or(&existing.icon);
        let domains_json = if let Some(d) = domains {
            serde_json::to_string(d).unwrap_or_else(|_| "[]".into())
        } else {
            existing.domains.clone()
        };
        let orchestrator_skill = orchestrator_skill.unwrap_or(&existing.orchestrator_skill);
        let default_interaction_mode =
            default_interaction_mode.unwrap_or(&existing.default_interaction_mode);

        sqlx::query(
            r#"
            UPDATE squads
            SET name = ?1, description = ?2, icon = ?3, domains = ?4,
                orchestrator_skill = ?5, default_interaction_mode = ?6, updated_at = ?7
            WHERE id = ?8
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(icon)
        .bind(&domains_json)
        .bind(orchestrator_skill)
        .bind(default_interaction_mode)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get(id).await
    }

    /// Determines the effective interaction mode for a Smart-mode squad.
    ///
    /// Uses the cached value in `last_smart_mode` if it was set within the last day.
    /// Otherwise, computes the mode from the provided interaction counts and caches the result.
    ///
    /// If `counts` is `None` or total interactions are fewer than 5, defaults to `"lead"`.
    pub async fn resolve_smart_mode(
        &self,
        squad_id: &str,
        counts: Option<InteractionCounts>,
    ) -> Result<String, sqlx::Error> {
        // 1. Check cache
        if let Some(squad) = self.get(squad_id).await? {
            if let (Some(ref cached), Some(ref updated)) =
                (&squad.last_smart_mode, &squad.last_smart_updated)
            {
                if is_cache_fresh(updated) {
                    return Ok(cached.clone());
                }
            }
        }

        // 2. Compute from counts
        let mode = match counts {
            Some(c) => {
                let total = c.direct_count + c.lead_count + c.debate_count;
                if total < 5 {
                    "lead"
                } else if c.debate_count as f64 / total as f64 > 0.5 {
                    "debate"
                } else {
                    "lead"
                }
            }
            None => "lead",
        };

        // 3. Cache result
        self.update_smart_cache(squad_id, mode).await?;

        Ok(mode.to_string())
    }

    /// Update the smart-mode cache columns for a squad.
    pub async fn update_smart_cache(&self, squad_id: &str, mode: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE squads SET last_smart_mode = ?2, last_smart_updated = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(squad_id)
        .bind(mode)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a squad. Returns error for builtin squads.
    pub async fn delete(&self, id: &str) -> Result<bool, sqlx::Error> {
        let squad = self.get(id).await?;
        if let Some(s) = &squad {
            if s.source == "builtin" {
                return Err(sqlx::Error::Protocol("Cannot delete builtin squad".into()));
            }
        }

        let result = sqlx::query("DELETE FROM squads WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Add or replace a member in a squad.
    pub async fn add_member(
        &self,
        squad_id: &str,
        persona_id: &str,
        role_in_squad: &str,
        sort_order: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO squad_members
                (squad_id, persona_id, role_in_squad, sort_order)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(squad_id)
        .bind(persona_id)
        .bind(role_in_squad)
        .bind(sort_order)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a member from a squad.
    pub async fn remove_member(&self, squad_id: &str, persona_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM squad_members WHERE squad_id = ?1 AND persona_id = ?2")
            .bind(squad_id)
            .bind(persona_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List members of a squad ordered by sort_order.
    pub async fn list_members(&self, squad_id: &str) -> Result<Vec<SquadMemberRow>, sqlx::Error> {
        sqlx::query_as::<_, SquadMemberRow>(
            "SELECT * FROM squad_members WHERE squad_id = ?1 ORDER BY sort_order ASC",
        )
        .bind(squad_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Resolve a squad into its full representation with persona details.
    /// Returns None if the squad is not found. Uses 2 queries (squad + JOIN).
    pub async fn resolve_squad(
        &self,
        squad_id: &str,
    ) -> Result<Option<ResolvedSquad>, sqlx::Error> {
        let squad = match self.get(squad_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let rows = sqlx::query_as::<_, MemberPersonaJoinRow>(
            r#"
            SELECT p.id, p.name, p.role, p.expertise, p.perspective, p.tone,
                   p.icon, p.source, p.domains, p.is_active, p.relevance_score,
                   p.skill_path, p.questioning_style, p.cognitive_bias,
                   p.analysis_frameworks, p.created_at, p.updated_at,
                   sm.role_in_squad, sm.sort_order
            FROM squad_members sm
            INNER JOIN insight_personas p ON p.id = sm.persona_id
            WHERE sm.squad_id = ?1
            ORDER BY sm.sort_order ASC
            "#,
        )
        .bind(squad_id)
        .fetch_all(&self.pool)
        .await?;

        let mut personas = Vec::with_capacity(rows.len());
        let mut members = Vec::with_capacity(rows.len());
        for row in rows {
            personas.push(PersonaRow {
                id: row.id.clone(),
                name: row.name,
                role: row.role,
                expertise: row.expertise,
                perspective: row.perspective,
                tone: row.tone,
                icon: row.icon,
                source: row.source,
                domains: row.domains,
                is_active: row.is_active,
                relevance_score: row.relevance_score,
                skill_path: row.skill_path,
                questioning_style: row.questioning_style,
                cognitive_bias: row.cognitive_bias,
                analysis_frameworks: row.analysis_frameworks,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
            members.push(SquadMemberRow {
                squad_id: squad_id.to_string(),
                persona_id: row.id,
                role_in_squad: row.role_in_squad,
                sort_order: row.sort_order,
            });
        }

        Ok(Some(ResolvedSquad {
            squad,
            personas,
            members,
        }))
    }

    /// Seed all builtin squads and their members. Idempotent — skips if already present.
    pub async fn seed_builtins(&self) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for b in BUILTIN_SQUADS {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO squads
                    (id, name, description, icon, orchestrator_skill, source, domains,
                     is_active, default_interaction_mode, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, 'builtin', ?6, 1, 'lead', ?7, ?7)
                "#,
            )
            .bind(b.id)
            .bind(b.name)
            .bind(b.description)
            .bind(b.icon)
            .bind(b.orchestrator_skill)
            .bind(b.domains)
            .bind(&now)
            .execute(&mut *tx)
            .await?;

            for &(persona_id, role, sort_order) in b.members {
                sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO squad_members
                        (squad_id, persona_id, role_in_squad, sort_order)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                )
                .bind(b.id)
                .bind(persona_id)
                .bind(role)
                .bind(sort_order)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (SquadRepo, PersonaRepo) {
        let pool = crate::repos::cognitive_test_pool().await;
        let squad_repo = SquadRepo::new(pool.clone());
        let persona_repo = PersonaRepo::new(pool);
        persona_repo.seed_builtins().await.unwrap();
        (squad_repo, persona_repo)
    }

    #[tokio::test]
    async fn test_squad_crud() {
        let (repo, _) = setup().await;
        let squad = repo
            .create(&NewSquad {
                name: "Test Squad".into(),
                description: "A test squad".into(),
                icon: "\u{1f9ea}".into(),
                orchestrator_skill: "general".into(),
                domains: vec!["testing".into()],
            })
            .await
            .unwrap();

        assert_eq!(squad.name, "Test Squad");
        assert_eq!(squad.source, "user");

        let fetched = repo.get(&squad.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Squad");

        let all = repo.list().await.unwrap();
        assert!(all.iter().any(|s| s.id == squad.id));

        repo.delete(&squad.id).await.unwrap();
        assert!(repo.get(&squad.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_squad_members() {
        let (repo, _) = setup().await;
        let squad = repo
            .create(&NewSquad {
                name: "Member Test".into(),
                description: "".into(),
                icon: "\u{1f9ea}".into(),
                orchestrator_skill: "general".into(),
                domains: vec![],
            })
            .await
            .unwrap();

        repo.add_member(&squad.id, "builtin-skeptic", "lead", 0)
            .await
            .unwrap();
        repo.add_member(&squad.id, "builtin-connector", "member", 1)
            .await
            .unwrap();

        let members = repo.list_members(&squad.id).await.unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].persona_id, "builtin-skeptic");
        assert_eq!(members[0].role_in_squad, "lead");

        repo.remove_member(&squad.id, "builtin-skeptic")
            .await
            .unwrap();
        let members = repo.list_members(&squad.id).await.unwrap();
        assert_eq!(members.len(), 1);
    }

    #[tokio::test]
    async fn test_resolve_squad() {
        let (repo, _) = setup().await;
        let squad = repo
            .create(&NewSquad {
                name: "Resolve Test".into(),
                description: "".into(),
                icon: "\u{1f52c}".into(),
                orchestrator_skill: "general".into(),
                domains: vec![],
            })
            .await
            .unwrap();

        repo.add_member(&squad.id, "builtin-skeptic", "lead", 0)
            .await
            .unwrap();
        repo.add_member(&squad.id, "builtin-student", "member", 1)
            .await
            .unwrap();

        let resolved = repo.resolve_squad(&squad.id).await.unwrap().unwrap();
        assert_eq!(resolved.squad.name, "Resolve Test");
        assert_eq!(resolved.personas.len(), 2);
        assert_eq!(resolved.personas[0].name, "Skeptic");
    }

    #[tokio::test]
    async fn test_seed_builtins() {
        let (repo, _) = setup().await;
        repo.seed_builtins().await.unwrap();

        let squads = repo.list().await.unwrap();
        assert_eq!(squads.len(), 4);

        let general = squads
            .iter()
            .find(|s| s.name == "General Analysis")
            .unwrap();
        let members = repo.list_members(&general.id).await.unwrap();
        assert_eq!(members.len(), 4);

        // Idempotent
        repo.seed_builtins().await.unwrap();
        assert_eq!(repo.list().await.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn test_cannot_delete_builtin_squad() {
        let (repo, _) = setup().await;
        repo.seed_builtins().await.unwrap();
        let squads = repo.list().await.unwrap();
        let builtin = squads.iter().find(|s| s.source == "builtin").unwrap();
        let result = repo.delete(&builtin.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_squad_default_interaction_mode() {
        let (repo, _) = setup().await;

        // Create a squad and verify default_interaction_mode is "lead"
        let squad = repo
            .create(&NewSquad {
                name: "Mode Test Squad".into(),
                description: "Testing interaction modes".into(),
                icon: "\u{1f9ea}".into(),
                orchestrator_skill: "general".into(),
                domains: vec!["testing".into()],
            })
            .await
            .unwrap();

        assert_eq!(squad.default_interaction_mode, "lead");
        assert!(squad.last_smart_mode.is_none());
        assert!(squad.last_smart_updated.is_none());

        // Update with new orchestrator_skill and default_interaction_mode
        let updated = repo
            .update(
                &squad.id,
                None,
                None,
                None,
                None,
                Some("task-management"),
                Some("debate"),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.orchestrator_skill, "task-management");
        assert_eq!(updated.default_interaction_mode, "debate");

        // Test update_smart_cache
        repo.update_smart_cache(&squad.id, "debate").await.unwrap();
        let fetched = repo.get(&squad.id).await.unwrap().unwrap();
        assert_eq!(fetched.last_smart_mode.as_deref(), Some("debate"));
        assert!(fetched.last_smart_updated.is_some());
    }

    #[tokio::test]
    async fn test_persona_in_multiple_squads() {
        let (repo, _) = setup().await;
        repo.seed_builtins().await.unwrap();

        let squads = repo.list().await.unwrap();
        let mut skeptic_squads = Vec::new();
        for squad in &squads {
            let members = repo.list_members(&squad.id).await.unwrap();
            if members.iter().any(|m| m.persona_id == "builtin-skeptic") {
                skeptic_squads.push(squad.name.clone());
            }
        }
        assert!(skeptic_squads.len() >= 2);
    }

    // ── Smart mode tests ─────────────────────────────────────────

    async fn create_test_squad(repo: &SquadRepo) -> SquadRow {
        repo.create(&NewSquad {
            name: "Smart Mode Squad".into(),
            description: "Testing smart mode".into(),
            icon: "\u{1f9e0}".into(),
            orchestrator_skill: "general".into(),
            domains: vec!["testing".into()],
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_resolve_smart_mode_defaults_to_lead() {
        // No counts provided → "lead"
        let (repo, _) = setup().await;
        let squad = create_test_squad(&repo).await;

        let mode = repo.resolve_smart_mode(&squad.id, None).await.unwrap();
        assert_eq!(mode, "lead");

        // Cache should have been written
        let fetched = repo.get(&squad.id).await.unwrap().unwrap();
        assert_eq!(fetched.last_smart_mode.as_deref(), Some("lead"));
    }

    #[tokio::test]
    async fn test_resolve_smart_mode_prefers_debate_when_majority() {
        // debate_count > 50% of total → "debate"
        let (repo, _) = setup().await;
        let squad = create_test_squad(&repo).await;

        let counts = InteractionCounts {
            direct_count: 2,
            lead_count: 2,
            debate_count: 6, // 60% of 10 → debate
        };
        let mode = repo
            .resolve_smart_mode(&squad.id, Some(counts))
            .await
            .unwrap();
        assert_eq!(mode, "debate");
    }

    #[tokio::test]
    async fn test_resolve_smart_mode_too_few_interactions() {
        // total < 5 → "lead" regardless of ratios
        let (repo, _) = setup().await;
        let squad = create_test_squad(&repo).await;

        let counts = InteractionCounts {
            direct_count: 0,
            lead_count: 0,
            debate_count: 4, // 100% debate but total = 4 < 5
        };
        let mode = repo
            .resolve_smart_mode(&squad.id, Some(counts))
            .await
            .unwrap();
        assert_eq!(mode, "lead");
    }

    #[tokio::test]
    async fn test_resolve_smart_mode_lead_wins_when_debate_not_majority() {
        // debate_count == 50% (not >50%) → "lead"
        let (repo, _) = setup().await;
        let squad = create_test_squad(&repo).await;

        let counts = InteractionCounts {
            direct_count: 3,
            lead_count: 2,
            debate_count: 5, // 50% of 10, not strictly >50%
        };
        let mode = repo
            .resolve_smart_mode(&squad.id, Some(counts))
            .await
            .unwrap();
        assert_eq!(mode, "lead");
    }

    #[tokio::test]
    async fn test_resolve_smart_mode_uses_cache() {
        // Set a fresh cache entry, then call resolve without counts.
        // The cached value should be returned immediately.
        let (repo, _) = setup().await;
        let squad = create_test_squad(&repo).await;

        // Pre-populate cache with "debate"
        repo.update_smart_cache(&squad.id, "debate").await.unwrap();

        // resolve_smart_mode with None counts — cache is fresh so "debate" is returned
        let mode = repo.resolve_smart_mode(&squad.id, None).await.unwrap();
        assert_eq!(mode, "debate");

        // DB cache still says "debate"
        let fetched = repo.get(&squad.id).await.unwrap().unwrap();
        assert_eq!(fetched.last_smart_mode.as_deref(), Some("debate"));
    }
}
