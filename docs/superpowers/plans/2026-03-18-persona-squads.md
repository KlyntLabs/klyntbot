# Persona Squads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat persona system with squad-based groups where each persona has agent-like skills, enabling purpose-built teams for Insight Review perspectives.

**Architecture:** Layered composition — squads link one orchestrator skill + N personas. Persona skills add perspective (expertise, frameworks, tone) without tool access. Each persona gets its own parallel LLM call during perspective generation. Three-tier memory scoping: Global → Squad → Persona.

**Tech Stack:** Rust (cognitive/skill-system/app-core/desktop crates), SQLite, TypeScript/React (desktop-ui), Tauri IPC, SSE streaming.

**Spec:** `docs/superpowers/specs/2026-03-18-persona-squads-design.md`

---

## File Map

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/repos/squad.rs` | SquadRow, SquadMemberRow, SquadRepo (CRUD + resolve + seed) |
| `crates/skill-system/src/persona.rs` | PersonaSkillMetadata struct + PersonaSkillParser |
| `personas/skeptic/PERSONA.md` | Builtin persona skill file (×10 personas) |
| `squads/general-analysis/SQUAD.md` | Builtin squad definition (×4 squads) |
| `crates/desktop/src/commands/squads.rs` | Tauri commands for squad CRUD |
| `crates/desktop-shared/src/commands/squads.rs` | Squad IPC types (DTOs) |
| `crates/app-core/src/handlers/squads.rs` | AppCore squad handler methods |
| `desktop-ui/src/features/notes/hooks/useSquads.ts` | Squad IPC hook |
| `desktop-ui/src/features/notes/components/insight/SquadPicker.tsx` | Squad selection dropdown |
| `desktop-ui/src/features/notes/components/insight/SquadManager.tsx` | Squad management UI |

### Modified Files
| File | Changes |
|------|---------|
| `crates/cognitive/src/repos/persona.rs` | Extend PersonaRow with skill fields, update seed_builtins() for 10 personas, fix test assertions |
| `crates/cognitive/src/repos/mod.rs:61-65` | Export squad module, bump FeatureMigration version 6→7 |
| `crates/cognitive/src/types.rs` | Add scope_type/scope_id to SemanticFact, EpisodicMemory, ProceduralRule |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add squads + squad_members tables, scope columns, indexes |
| `crates/cognitive/src/lib.rs` | Re-export squad types |
| `crates/skill-system/src/types.rs` | Add SkillType::Persona variant |
| `crates/skill-system/src/discovery.rs` | Add SkillSource::Personas variant, persona_skills() accessor, update catalog_prompt() |
| `crates/skill-system/src/lib.rs` | Export persona module |
| `crates/app-core/src/handlers/notes/insight.rs` | Replace select_personas() with resolve_squad(), parallel LLM calls |
| `crates/app-core/src/handlers/notes/insight_prompts.rs` | Per-persona prompt with skill body injection |
| `crates/app-core/src/handlers/mod.rs` | Register squads handler module |
| `crates/app-core/src/state.rs:38-103` | Add `squad_repo: Option<cognitive::SquadRepo>` field to AppCore |
| `crates/app-core/src/init/mod.rs` | Initialize SquadRepo, call seed_builtins() at startup |
| `crates/app-core/Cargo.toml` | Add `futures` dependency for `join_all` |
| `crates/desktop/src/commands/mod.rs` | Register squads command module |
| `crates/desktop-shared/src/commands/mod.rs` | Register squads types module |
| `crates/desktop-shared/src/commands/notes.rs` | Add squad_id to insight params, extend PersonaMetaResponse |
| `crates/agent/src/context_sources/mod.rs` | Remove AnalysisPersonaContextSource registration |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add squad_id to generation, handle N parallel SSE streams |
| `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx` | Squad header + independent streaming card grid |
| `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx` | Nest inside squad context |

### Deleted Files
| File | Reason |
|------|--------|
| `crates/agent/src/context_sources/analysis_persona.rs` | Replaced by PersonaPerspectiveSource (Phase 2) |

---

## Task 1: Schema — Add Squad Tables + Scope Columns

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/types.rs:20-71`

- [ ] **Step 1: Add squad tables to migration SQL**

Append to `crates/cognitive/migrations/001_cognitive_tables.sql` after the `insight_persona_pins` table:

```sql
-- ── Squads ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS squads (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    icon                TEXT NOT NULL DEFAULT '',
    orchestrator_skill  TEXT NOT NULL DEFAULT 'general',
    source              TEXT NOT NULL DEFAULT 'user',
    domains             TEXT NOT NULL DEFAULT '[]',
    is_active           INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_squads_name_user
    ON squads(name) WHERE source = 'user';

CREATE TABLE IF NOT EXISTS squad_members (
    squad_id        TEXT NOT NULL REFERENCES squads(id) ON DELETE CASCADE,
    persona_id      TEXT NOT NULL,
    role_in_squad   TEXT NOT NULL DEFAULT 'member',
    sort_order      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (squad_id, persona_id)
);

CREATE INDEX IF NOT EXISTS idx_squad_members_persona ON squad_members(persona_id);
```

- [ ] **Step 2: Add scope columns to cognitive tables**

In the same migration file, add scope columns to `semantic_facts`, `episodic_memories`, and `procedural_rules` `CREATE TABLE` statements. Add these two columns after `project_id` in each table:

```sql
    scope_type      TEXT NOT NULL DEFAULT 'system',
    scope_id        TEXT
```

Also add to `semantic_facts_archive` (after `project_id`).

Add indexes after each table's existing indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_semantic_facts_scope ON semantic_facts(scope_type, scope_id);
CREATE INDEX IF NOT EXISTS idx_episodic_memories_scope ON episodic_memories(scope_type, scope_id);
CREATE INDEX IF NOT EXISTS idx_procedural_rules_scope ON procedural_rules(scope_type, scope_id);
```

- [ ] **Step 3: Add new columns to PersonaRow (insight_personas table)**

In the `insight_personas` `CREATE TABLE` statement, add after `relevance_score`:

```sql
    skill_path          TEXT,
    questioning_style   TEXT NOT NULL DEFAULT 'analytical',
    cognitive_bias      TEXT NOT NULL DEFAULT 'balanced',
    analysis_frameworks TEXT NOT NULL DEFAULT '[]'
```

- [ ] **Step 4: Update Rust row structs**

In `crates/cognitive/src/types.rs`, add to `SemanticFact` (after `memory_type`):

```rust
    pub scope_type: String,
    pub scope_id: Option<String>,
```

Add to `EpisodicMemory` (after `project_id`):

```rust
    pub scope_type: String,
    pub scope_id: Option<String>,
```

Add to `ProceduralRule` (after `project_id`):

```rust
    pub scope_type: String,
    pub scope_id: Option<String>,
```

- [ ] **Step 5: Bump FeatureMigration version**

In `crates/cognitive/src/repos/mod.rs`, find the `FeatureMigration` definition (around line 61) and bump the version from `6` to `7`. This ensures the migration re-runs on existing developer databases:

```rust
FeatureMigration {
    name: "cognitive",
    version: 7,  // was 6
    // ...
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build -p cognitive 2>&1 | tail -5`
Expected: Compilation may fail if repo methods don't include new columns yet — that's OK, we fix those in Task 2+3. If there are `sqlx::FromRow` derivation issues, ensure the column order in the struct matches the SELECT order in repo queries.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/types.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add squad tables, scope columns, persona skill fields to schema"
```

---

## Task 2: Extend PersonaRow + Update Existing Repo Methods

**Files:**
- Modify: `crates/cognitive/src/repos/persona.rs:10-25` (PersonaRow), `28-36` (NewPersona), `39-47` (PersonaUpdate), `51-123` (builtins), `137-460` (methods)

- [ ] **Step 1: Write test for extended PersonaRow fields**

Add to the existing test module at the bottom of `persona.rs`:

```rust
#[tokio::test]
async fn test_persona_skill_fields() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = PersonaRepo::new(pool.inner().clone());
    repo.seed_builtins().await.unwrap();

    let persona = repo.get("builtin-skeptic").await.unwrap().unwrap();
    assert_eq!(persona.questioning_style, "interrogative");
    assert_eq!(persona.cognitive_bias, "precision");
    assert!(!persona.analysis_frameworks.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_persona_skill_fields)'`
Expected: FAIL — `PersonaRow` doesn't have `questioning_style` field yet.

- [ ] **Step 3: Extend PersonaRow struct**

Add to `PersonaRow` after `relevance_score`:

```rust
    pub skill_path: Option<String>,
    pub questioning_style: String,
    pub cognitive_bias: String,
    pub analysis_frameworks: String, // JSON array
```

- [ ] **Step 4: Extend NewPersona struct**

Add optional skill fields:

```rust
    pub questioning_style: Option<String>,
    pub cognitive_bias: Option<String>,
    pub analysis_frameworks: Option<Vec<String>>,
```

- [ ] **Step 5: Update BuiltinPersona + seed data**

Extend the `BuiltinPersona` struct and all 6 existing builtin definitions with the new fields. **Important:** The existing `domains` field is `&'static str` (pre-serialized JSON like `r#"["general","research"]"#`), not `&[&str]`. Keep that type. Example for Skeptic:

```rust
BuiltinPersona {
    id: "builtin-skeptic",
    name: "Skeptic",
    role: "Critical analyst",
    expertise: "Logical reasoning, bias detection, evidence evaluation",
    perspective: "Questions claims and looks for weak spots",
    tone: "direct",
    icon: "🔍",
    domains: r#"["general","research"]"#,  // existing &str type
    questioning_style: "interrogative",
    cognitive_bias: "precision",
    analysis_frameworks: r#"["critical-analysis","evidence-evaluation"]"#,
},
```

Add 4 new builtins: Academic Reviewer, Methodologist, Deep Analyst, Risk Reviewer.

- [ ] **Step 6: Update INSERT/UPDATE SQL queries in PersonaRepo methods**

Only explicit INSERT and UPDATE queries need the new columns added: `seed_builtins()`, `create()`, `create_auto()`, `update()`. The `SELECT *` queries in `list_active()`, `list_all()`, `get()` automatically pick up new columns — do NOT rewrite these.

- [ ] **Step 6b: Update existing test assertions**

Existing tests assert `active.len() == 6` (e.g., `persona.rs:478`, `persona.rs:577-579`). Update these to `10` to account for the 4 new builtin personas.

- [ ] **Step 7: Run the test**

Run: `cargo nextest run -p cognitive -E 'test(persona)'`
Expected: All 8+ tests pass (including the new one).

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/repos/persona.rs
git commit -m "feat(cognitive): extend PersonaRow with skill fields, add 4 new builtin personas"
```

---

## Task 3: SquadRepo — CRUD + Resolve + Seed

**Files:**
- Create: `crates/cognitive/src/repos/squad.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Write tests for SquadRepo**

Create `crates/cognitive/src/repos/squad.rs` with a test module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> (SquadRepo, PersonaRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // PersonaRepo and SquadRepo both take the inner SqlitePool
        let inner = pool.inner().clone();
        let squad_repo = SquadRepo::new(inner.clone());
        let persona_repo = PersonaRepo::new(inner);
        persona_repo.seed_builtins().await.unwrap();
        (squad_repo, persona_repo)
    }

    #[tokio::test]
    async fn test_squad_crud() {
        let (repo, _) = setup().await;
        let squad = repo.create(&NewSquad {
            name: "Test Squad".into(),
            description: "A test squad".into(),
            icon: "🧪".into(),
            orchestrator_skill: "general".into(),
            domains: vec!["testing".into()],
        }).await.unwrap();

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
        let squad = repo.create(&NewSquad {
            name: "Member Test".into(),
            description: "".into(),
            icon: "🧪".into(),
            orchestrator_skill: "general".into(),
            domains: vec![],
        }).await.unwrap();

        repo.add_member(&squad.id, "builtin-skeptic", "lead", 0).await.unwrap();
        repo.add_member(&squad.id, "builtin-connector", "member", 1).await.unwrap();

        let members = repo.list_members(&squad.id).await.unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].persona_id, "builtin-skeptic");
        assert_eq!(members[0].role_in_squad, "lead");

        repo.remove_member(&squad.id, "builtin-skeptic").await.unwrap();
        let members = repo.list_members(&squad.id).await.unwrap();
        assert_eq!(members.len(), 1);
    }

    #[tokio::test]
    async fn test_resolve_squad() {
        let (repo, _) = setup().await;
        let squad = repo.create(&NewSquad {
            name: "Resolve Test".into(),
            description: "".into(),
            icon: "🔬".into(),
            orchestrator_skill: "general".into(),
            domains: vec![],
        }).await.unwrap();

        repo.add_member(&squad.id, "builtin-skeptic", "lead", 0).await.unwrap();
        repo.add_member(&squad.id, "builtin-student", "member", 1).await.unwrap();

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
        assert_eq!(squads.len(), 4); // general, research, finance, strategy

        let general = squads.iter().find(|s| s.name == "General Analysis").unwrap();
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
    async fn test_persona_in_multiple_squads() {
        let (repo, _) = setup().await;
        repo.seed_builtins().await.unwrap();

        // Skeptic should be in both General Analysis and Research
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(test_squad)'`
Expected: FAIL — module doesn't have the types yet.

- [ ] **Step 3: Implement SquadRepo**

Above the test module, implement the full `SquadRepo`:

```rust
use common::Result;
use serde::{Deserialize, Serialize};
use storage::StoragePool;

use super::persona::{PersonaRepo, PersonaRow};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SquadRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub source: String,
    pub domains: String,
    pub is_active: i64,
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

#[derive(Clone)]
pub struct SquadRepo {
    pool: SqlitePool,  // same as PersonaRepo — uses raw SqlitePool, not StoragePool
}
```

Implement methods: `new()`, `create()`, `get()`, `list()`, `update()`, `delete()` (reject builtin), `add_member()`, `remove_member()`, `list_members()`, `resolve_squad()`, `seed_builtins()`.

`seed_builtins()` uses `INSERT OR IGNORE` with hardcoded IDs like `"builtin-squad-general"`, `"builtin-squad-research"`, `"builtin-squad-finance"`, `"builtin-squad-strategy"`. Links to persona IDs via `INSERT OR IGNORE INTO squad_members`.

`resolve_squad()` JOINs `squad_members` with `insight_personas` ordered by `sort_order`, returning `ResolvedSquad`.

- [ ] **Step 4: Export from mod.rs and lib.rs**

Add to `crates/cognitive/src/repos/mod.rs`:
```rust
pub mod squad;
pub use squad::{NewSquad, ResolvedSquad, SquadMemberRow, SquadRepo, SquadRow};
```

Re-export from `crates/cognitive/src/lib.rs` alongside existing persona exports.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(test_squad)'`
Expected: All 6 squad tests pass.

- [ ] **Step 6: Run all cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: All tests pass (existing persona tests + new squad tests).

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/repos/squad.rs crates/cognitive/src/repos/mod.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add SquadRepo with CRUD, resolve, seed, and member management"
```

---

## Task 4: PersonaSkillMetadata + Parser in skill-system

**Files:**
- Create: `crates/skill-system/src/persona.rs`
- Modify: `crates/skill-system/src/types.rs:12-17` (SkillType)
- Modify: `crates/skill-system/src/discovery.rs:354-399` (accessors + catalog_prompt)
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Write test for PersonaSkillMetadata parsing**

Create `crates/skill-system/src/persona.rs` with test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PERSONA: &str = r#"---
name: deep-analyst
description: >
  Rigorous financial analyst specializing in DCF valuation.
persona_only: true
version: "1.0.0"
icon: 📊
domains: [finance, productivity]
metadata:
  expertise_areas:
    - DCF valuation
    - ratio analysis
  analysis_frameworks:
    - bottom-up
    - comparative
  questioning_style: interrogative
  tone: rigorous
  cognitive_bias: precision
  references:
    - dcf-guide
---

You are a rigorous financial analyst.
"#;

    #[test]
    fn test_parse_persona_skill() {
        let result = parse_persona_skill(SAMPLE_PERSONA).unwrap();
        assert_eq!(result.name, "deep-analyst");
        assert_eq!(result.icon, "📊");
        assert_eq!(result.domains, vec!["finance", "productivity"]);
        assert_eq!(result.metadata.expertise_areas, vec!["DCF valuation", "ratio analysis"]);
        assert_eq!(result.metadata.questioning_style, "interrogative");
        assert_eq!(result.metadata.tone, "rigorous");
        assert!(result.body.contains("rigorous financial analyst"));
    }

    #[test]
    fn test_persona_skill_ignores_tool_fields() {
        let md = r#"---
name: test
description: Test persona
persona_only: true
metadata:
  tools: [task, notes]
  mcp_tools: ["*"]
  expertise_areas: [testing]
  questioning_style: direct
  tone: neutral
  cognitive_bias: balanced
---
Body text.
"#;
        let result = parse_persona_skill(md).unwrap();
        // Tool fields are ignored — PersonaSkillMetadata has no tools/mcp_tools
        assert_eq!(result.metadata.expertise_areas, vec!["testing"]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p skill-system -E 'test(test_parse_persona)'`
Expected: FAIL — `parse_persona_skill` not defined.

- [ ] **Step 3: Implement PersonaSkillMetadata + parser**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSkillMetadata {
    pub expertise_areas: Vec<String>,
    pub analysis_frameworks: Vec<String>,
    pub questioning_style: String,
    pub tone: String,
    pub cognitive_bias: String,
    #[serde(default)]
    pub references: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedPersonaSkill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon: String,
    pub domains: Vec<String>,
    pub metadata: PersonaSkillMetadata,
    pub body: String,
}

pub fn parse_persona_skill(content: &str) -> common::Result<ParsedPersonaSkill> {
    // Split on --- frontmatter delimiters
    // Parse YAML frontmatter
    // Extract metadata block
    // Return ParsedPersonaSkill with body = everything after second ---
}
```

- [ ] **Step 4: Add SkillType::Persona variant**

In `crates/skill-system/src/types.rs`, extend:

```rust
pub enum SkillType {
    #[default]
    Skill,
    Orchestrator,
    Persona,
}
```

- [ ] **Step 5: Add SkillSource::Personas variant**

In `crates/skill-system/src/discovery.rs`, extend the `SkillSource` enum (lines 114-122):

```rust
pub enum SkillSource {
    BuiltIn(Vec<(String, String)>),
    Directory(std::path::PathBuf, SkillScope),
    Personas(Vec<(String, String)>),  // (name, PERSONA.md content)
    #[cfg(test)]
    Inline(Vec<(String, String)>, SkillScope),
}
```

In `discover()`, add a handler for `SkillSource::Personas` that calls `parse_persona_skill()` from the `persona` module and inserts the result as a `SkillPackage` with `skill_type: SkillType::Persona`.

- [ ] **Step 6: Add persona_skills() accessor + update catalog_prompt()**

Add `persona_skills()` method:
```rust
pub fn persona_skills(&self) -> Vec<&Arc<SkillPackage>> {
    self.skills.values()
        .filter(|s| matches!(s.skill_type, SkillType::Persona) && s.trusted)
        .collect()
}
```

In `catalog_prompt()` (lines 374-398), the `match pkg.skill_type` block currently handles `Orchestrator` and `Skill`. Add the `Persona` arm **before** the formatting code so `continue` skips the rest of the loop body:

```rust
for pkg in sorted {
    match pkg.skill_type {
        SkillType::Persona => continue,  // exclude from catalog prompt
        SkillType::Orchestrator => { /* existing orchestrator formatting */ }
        SkillType::Skill => { /* existing skill formatting */ }
    }
}
```

- [ ] **Step 6: Export from lib.rs**

Add to `crates/skill-system/src/lib.rs`:
```rust
pub mod persona;
pub use persona::{ParsedPersonaSkill, PersonaSkillMetadata, parse_persona_skill};
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/skill-system/src/persona.rs crates/skill-system/src/types.rs crates/skill-system/src/discovery.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): add PersonaSkillMetadata, parser, and SkillType::Persona"
```

---

## Task 5: Builtin Persona + Squad Markdown Files

**Files:**
- Create: `personas/skeptic/PERSONA.md` (and 9 more)
- Create: `squads/general-analysis/SQUAD.md` (and 3 more)

- [ ] **Step 1: Create flat personas/ directory with 10 PERSONA.md files**

Each follows the locked-in format with `persona_only: true`, `description`, `icon`, `domains`, `metadata` block, and body.

Files to create:
- `personas/skeptic/PERSONA.md`
- `personas/connector/PERSONA.md`
- `personas/student/PERSONA.md`
- `personas/devils-advocate/PERSONA.md`
- `personas/practitioner/PERSONA.md`
- `personas/strategist/PERSONA.md`
- `personas/academic-reviewer/PERSONA.md`
- `personas/methodologist/PERSONA.md`
- `personas/deep-analyst/PERSONA.md` (+ `references/dcf-guide.md`, `references/ratio-cheatsheet.md`)
- `personas/risk-reviewer/PERSONA.md`

- [ ] **Step 2: Create squads/ directory with 4 SQUAD.md files**

- `squads/general-analysis/SQUAD.md` — orchestrator: general, personas: [skeptic, connector, student, devils-advocate]
- `squads/research-academic/SQUAD.md` — orchestrator: general, personas: [skeptic, academic-reviewer, methodologist, student]
- `squads/finance-analysis/SQUAD.md` — orchestrator: finance-management, personas: [deep-analyst, risk-reviewer, strategist]
- `squads/strategy-planning/SQUAD.md` — orchestrator: task-management, personas: [strategist, practitioner, devils-advocate]

- [ ] **Step 3: Add parse test for each builtin persona**

In `crates/skill-system/src/persona.rs` tests, add:

```rust
#[test]
fn test_parse_all_builtin_personas() {
    // Paths relative to crates/skill-system/src/persona.rs → workspace root is ../../../
    let personas = vec![
        include_str!("../../../personas/skeptic/PERSONA.md"),
        include_str!("../../../personas/connector/PERSONA.md"),
        include_str!("../../../personas/student/PERSONA.md"),
        include_str!("../../../personas/devils-advocate/PERSONA.md"),
        include_str!("../../../personas/practitioner/PERSONA.md"),
        include_str!("../../../personas/strategist/PERSONA.md"),
        include_str!("../../../personas/academic-reviewer/PERSONA.md"),
        include_str!("../../../personas/methodologist/PERSONA.md"),
        include_str!("../../../personas/deep-analyst/PERSONA.md"),
        include_str!("../../../personas/risk-reviewer/PERSONA.md"),
    ];
    for (i, content) in personas.iter().enumerate() {
        let result = parse_persona_skill(content);
        assert!(result.is_ok(), "Failed to parse persona {}: {:?}", i, result.err());
    }
}
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p skill-system -E 'test(test_parse_all_builtin)'`
Expected: PASS — all 10 personas parse correctly.

- [ ] **Step 5: Commit**

```bash
git add personas/ squads/ crates/skill-system/src/persona.rs
git commit -m "feat: add 10 builtin persona skills and 4 builtin squad definitions"
```

---

## Task 6: Squad IPC Types + Tauri Commands

**Files:**
- Create: `crates/desktop-shared/src/commands/squads.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Create: `crates/app-core/src/handlers/squads.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Create: `crates/desktop/src/commands/squads.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

- [ ] **Step 1: Create squad IPC types**

Create `crates/desktop-shared/src/commands/squads.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub source: String,
    pub domains: Vec<String>,
    pub is_active: bool,
    pub members: Vec<SquadMemberResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadMemberResponse {
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub role_in_squad: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSquadParams {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub domains: Vec<String>,
    pub member_persona_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSquadParams {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadMemberParams {
    pub squad_id: String,
    pub persona_id: String,
    pub role_in_squad: Option<String>,
    pub sort_order: Option<i64>,
}
```

Register in `commands/mod.rs`.

- [ ] **Step 1b: Add squad_repo to AppCore state**

In `crates/app-core/src/state.rs` (around line 38-103), add `squad_repo: Option<cognitive::SquadRepo>` field to the `AppCore` struct, following the same pattern as `persona_repo`.

In `crates/app-core/src/init/mod.rs`, initialize `SquadRepo` alongside `PersonaRepo` and call `squad_repo.seed_builtins().await` at startup (same place where `persona_repo.seed_builtins()` is called, around line 291).

- [ ] **Step 1c: Add `futures` dependency to app-core**

In `crates/app-core/Cargo.toml`, add:
```toml
futures = "0.3"
```

This is needed for `futures::future::join_all` in Task 7's parallel persona calls.

- [ ] **Step 2: Create AppCore squad handler**

Create `crates/app-core/src/handlers/squads.rs` with methods:
- `list_squads()` → `Vec<SquadResponse>`
- `get_squad(id)` → `SquadResponse`
- `create_squad(params)` → `SquadResponse`
- `update_squad(params)` → `SquadResponse`
- `delete_squad(id)`
- `add_squad_member(params)`
- `remove_squad_member(squad_id, persona_id)`

Each delegates to `SquadRepo` and converts `SquadRow`/`SquadMemberRow` → `SquadResponse`/`SquadMemberResponse`.

Register in `handlers/mod.rs`.

- [ ] **Step 3: Create Tauri command module**

Create `crates/desktop/src/commands/squads.rs` with thin Tauri adapter commands:

```rust
#[tauri::command]
pub async fn list_squads(state: tauri::State<'_, AppState>) -> Result<Vec<SquadResponse>, String> {
    state.app_core.list_squads().await.map_err(|e| e.to_string())
}
// ... create_squad, update_squad, delete_squad, add_squad_member, remove_squad_member
```

Add the `DEV_COMMANDS` constant with `#[cfg(test)]` (matching existing command modules like `capture.rs:52-53`):

```rust
#[cfg(test)]
pub const DEV_COMMANDS: &[&str] = &["list_squads", "get_squad", "create_squad", "update_squad", "delete_squad", "add_squad_member", "remove_squad_member"];
```

Register in `commands/mod.rs` and add to `dev_server/mod.rs` coverage list.

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p desktop`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands/squads.rs crates/app-core/src/handlers/squads.rs crates/desktop/src/commands/squads.rs
git add crates/desktop-shared/src/commands/mod.rs crates/app-core/src/handlers/mod.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(desktop): add squad IPC types, AppCore handler, and Tauri commands"
```

---

## Task 7: Rewrite InsightHandler — Squad-Based Parallel Persona Calls

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs:849-861` (select_personas → resolve_squad)
- Modify: `crates/app-core/src/handlers/notes/insight.rs:879-979` (run_insight_pipeline)
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs:110-199`
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add squad_id to params)

- [ ] **Step 1: Add squad_id to insight review params**

In `crates/desktop-shared/src/commands/notes.rs`, add `squad_id: Option<String>` to the insight review request params and regenerate tab params.

- [ ] **Step 2: Replace select_personas with resolve_squad**

In `insight.rs`, replace the `select_personas()` method (lines 849-861):

```rust
async fn resolve_squad_for_note(
    &self,
    note_id: &str,
    squad_id: Option<&str>,
    domains: &[String],
) -> (cognitive::SquadRow, Vec<cognitive::PersonaRow>) {
    if let Some(sid) = squad_id {
        if let Some(squad_repo) = &self.squad_repo {
            if let Ok(Some(resolved)) = squad_repo.resolve_squad(sid).await {
                return (resolved.squad, resolved.personas);
            }
        }
    }
    // Fallback: resolve default squad (General Analysis)
    if let Some(squad_repo) = &self.squad_repo {
        if let Ok(Some(resolved)) = squad_repo.resolve_squad("builtin-squad-general").await {
            return Ok((resolved.squad, resolved.personas));
        }
    }
    Err(common::KlyntbotError::api("NO_SQUAD", "No squad available"))
}
```

- [ ] **Step 3: Rewrite perspective generation for parallel calls**

In `run_insight_pipeline()`, replace the single perspectives prompt with parallel per-persona calls:

```rust
// Build per-persona futures
let persona_futures: Vec<_> = personas.iter().map(|persona| {
    let context = context.clone();
    let persona = persona.clone();
    async move {
        let persona_prompt = insight_prompts::single_persona_prompt(&context, &persona);
        // Call LLM with persona-specific system prompt
        // Return (persona_id, perspective_text)
    }
}).collect();

let results = futures::future::join_all(persona_futures).await;
```

- [ ] **Step 4: Create single_persona_prompt in insight_prompts.rs**

Add a new function that generates a prompt for a single persona, including its skill body and metadata:

```rust
pub fn single_persona_prompt(
    context: &str,
    persona_name: &str,
    persona_role: &str,
    persona_expertise: &str,
    persona_perspective: &str,
    persona_tone: &str,
    persona_skill_body: Option<&str>,
) -> String {
    // Build prompt that focuses this single persona on the note content
}
```

- [ ] **Step 5: Update SSE streaming for per-persona events**

Emit separate `insight:persona-perspective` events for each persona as they complete, instead of a single `insight:perspectives` event. Each event includes `persona_id` and the perspective text.

- [ ] **Step 6: Run existing insight tests**

Run: `cargo nextest run -p app-core -E 'test(insight)'`
Expected: Tests pass (update any tests that relied on the old single-call pattern).

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs crates/app-core/src/handlers/notes/insight_prompts.rs
git add crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(insight): rewrite perspective generation for squad-based parallel persona calls"
```

---

## Task 8: Frontend — useSquads Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useSquads.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";  // use project's IPC wrapper, not bare invoke
import { useState, useEffect, useCallback } from "react";

export interface Squad {
  id: string;
  name: string;
  description: string;
  icon: string;
  orchestratorSkill: string;
  source: string;
  domains: string[];
  isActive: boolean;
  members: SquadMember[];
  createdAt: string;
  updatedAt: string;
}

export interface SquadMember {
  personaId: string;
  personaName: string;
  personaIcon: string;
  personaRole: string;
  roleInSquad: string;
  sortOrder: number;
}

export function useSquads() {
  const [squads, setSquads] = useState<Squad[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    const data = await ipc<Squad[]>("list_squads");
    setSquads(data);
    setLoading(false);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const createSquad = useCallback(async (params: {
    name: string;
    description: string;
    icon: string;
    orchestratorSkill: string;
    domains: string[];
    memberPersonaIds: string[];
  }) => {
    const squad = await ipc<Squad>("create_squad", { params });
    await refresh();
    return squad;
  }, [refresh]);

  const deleteSquad = useCallback(async (id: string) => {
    await ipc("delete_squad", { id });
    await refresh();
  }, [refresh]);

  const addMember = useCallback(async (squadId: string, personaId: string, roleInSquad?: string) => {
    await ipc("add_squad_member", { params: { squadId, personaId, roleInSquad } });
    await refresh();
  }, [refresh]);

  const removeMember = useCallback(async (squadId: string, personaId: string) => {
    await ipc("remove_squad_member", { squadId, personaId });
    await refresh();
  }, [refresh]);

  return { squads, loading, refresh, createSquad, deleteSquad, addMember, removeMember };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useSquads.ts
git commit -m "feat(ui): add useSquads hook for squad IPC operations"
```

---

## Task 9: Frontend — SquadPicker Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/SquadPicker.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx`

- [ ] **Step 1: Create SquadPicker**

A dropdown/popover that shows available squads with member previews. When a squad is selected, it updates the parent's `squadId` state. Shows squad icon + name + member chips.

```tsx
interface SquadPickerProps {
  selectedSquadId: string | null;
  onSelect: (squadId: string) => void;
}
```

Uses `useSquads()` hook internally. Renders squads as selectable cards with member persona icons as chips.

- [ ] **Step 2: Integrate SquadPicker into PerspectivesTab**

Replace the "Manage Personas" button at the top of `PerspectivesTab.tsx` with `<SquadPicker>`. Pass `squadId` to the insight generation params.

- [ ] **Step 3: Update PerspectivesTab rendering for independent streaming**

Update `PerspectivesTab` to:
- Show squad header (icon + name + member count)
- Render persona cards in a 2-column grid
- Each card shows independent streaming status ("done" / "streaming..." / "queued")
- Listen for per-persona `insight:persona-perspective` events instead of parsing a single `---`-delimited response

- [ ] **Step 4: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SquadPicker.tsx
git add desktop-ui/src/features/notes/components/insight/PerspectivesTab.tsx
git commit -m "feat(ui): add SquadPicker, update PerspectivesTab for squad-based independent streaming"
```

---

## Task 10: Frontend — SquadManager Component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/SquadManager.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx`

- [ ] **Step 1: Create SquadManager**

Full management screen for squads:
- List all squads (builtin badge, member count)
- Create new squad form (name, description, icon picker, orchestrator skill selector)
- Edit squad members (add/remove personas, drag-to-reorder via sort_order, role assignment)
- Delete user squads (builtin squads protected)
- Nest existing `ManagePersonasModal` inside for individual persona CRUD

```tsx
interface SquadManagerProps {
  open: boolean;
  onClose: () => void;
}
```

- [ ] **Step 2: Update ManagePersonasModal**

Modify to accept optional `squadId` prop. When inside squad context, the "Add Persona" action adds the persona to the current squad rather than just toggling active state.

- [ ] **Step 3: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors.

- [ ] **Step 4: Visual smoke test**

Run: `cd desktop-ui && bun run dev` (+ `cargo tauri dev` in another terminal)
Navigate to a note → Insight Review → Perspectives tab.
Verify: SquadPicker shows 4 builtin squads, selecting one triggers generation with all squad members.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/SquadManager.tsx
git add desktop-ui/src/features/notes/components/insight/ManagePersonasModal.tsx
git commit -m "feat(ui): add SquadManager for squad CRUD and member management"
```

---

## Task 11: Delete AnalysisPersonaContextSource + Clippy/Format

**Files:**
- Delete: `crates/agent/src/context_sources/analysis_persona.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`

- [ ] **Step 1: Remove AnalysisPersonaContextSource**

Delete `crates/agent/src/context_sources/analysis_persona.rs`.

Remove `pub mod analysis_persona;` from `crates/agent/src/context_sources/mod.rs`.

Remove its registration from the `ContextEngine` builder. Search for `AnalysisPersonaContextSource::new()` in `crates/agent/src/` to find the exact registration site (grep for it — it's in the context engine setup where all sources are registered).

- [ ] **Step 2: Verify no duplicate persona injection**

Run: `cargo build --workspace`
Expected: PASS — no references to the deleted type remain.

- [ ] **Step 3: Run full lint + format check**

Run: `cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: 0 warnings, formatting clean.

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(agent): remove AnalysisPersonaContextSource, replaced by squad-based personas"
```

---

## Task 12: Final Integration Test + Cleanup

**Files:**
- All modified files from Tasks 1-11

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean build, no warnings.

- [ ] **Step 2: Run full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: All tests pass.

- [ ] **Step 3: Run frontend checks**

Run: `cd desktop-ui && bun run lint:fix && bun run test`
Expected: Clean.

- [ ] **Step 4: Smoke test with running app**

Run: `cd desktop-ui && bun run dev` (+ `cargo tauri dev`)
Test flow:
1. Open a note with content
2. Click "Insight Review"
3. Verify SquadPicker shows 4 builtin squads
4. Select "General Analysis" → verify 4 persona cards stream independently
5. Select "Finance Analysis" → verify 3 persona cards stream
6. Open SquadManager → verify CRUD works
7. Create custom squad → verify it appears in picker

- [ ] **Step 5: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "feat(squads): Phase 1 complete — squad-based perspective generation in Insight Review"
```
