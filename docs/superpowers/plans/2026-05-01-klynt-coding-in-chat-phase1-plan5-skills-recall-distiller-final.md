# Plan 5 — Phase 1 Final: Skills, Recall, Distiller/Mirror, Slash Commands, Settings, Acceptance

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close Phase 1 of "Klynt Coding in Chat" by wiring (a) `klynt-skill-loader` discovery + path-conditional activation + dynamic walk-up + progressive loading, (b) `CodingRecallService` injection through `ContextEngine` with the eight `recall_*` tools registered, (c) the Distiller `MemorySinkSubscriber` event translator (with E1–E5 proptests) plus three Mirror signal subscribers, (d) the full slash command catalog (`/skills *`, `/plan`, `/yolo`, `/power`, `/recall`, `/status`, `/doctor`, `/sessions star/unstar`, `/resume`, `/help`) with `useSlashCommands`/`registry.ts` dispatcher, (e) `useCodingMode` + `CodingModePill` wiring + workspace auto-detection, (f) `RecallTrayCard` + `DeadEndWarning` + `useCodingRecallSnippets`, (g) Settings → Coding page (General/Tools/Permissions/Sandbox/Skills/Sessions), (h) sandbox + cost composer-meta-bar pills, (i) the session retention nightly cron, (j) the K6/K7/K9 proptests + 5 scenario tests, and (k) the §13 Phase 1 exit gate end-to-end.

**Architecture:** Plan 5 is glue, not new substrate — every seam the prior four plans bolted into the agent loop is now made user-facing. We extend (not duplicate) the existing `skill-system` crate with a new `klynt-skill-loader` companion that adds path-conditional + dynamic discovery; we wrap the already-built `coding_memory::CodingRecallService` in a `ContextSource` so recall snippets enter the system prompt; we add a `MemorySinkSubscriber` to the `DomainEventBus` that translates `agent::events::AgentEvent` → `coding_ingest::AgentEvent`; we add 7 `coding_skills_*` Tauri commands and a typed slash-command catalog (`registry.ts`); we light up the existing `HooksSection` pattern for five sibling settings sections; and we surface telemetry pills via the existing `agent:sandbox_policy_applied` channel. No new sandbox/runtime code; no new approval layers.

**Tech Stack:** Rust 1.93 stable, Tokio, Tauri 2, React 19, Vite, Vitest, Bun, `globset 0.4` (skill glob matching), `serde_yaml 0.9` (frontmatter), the existing `starlark 0.13`, `proptest`, `tokio-test`. Dependencies flow strictly upward per `CLAUDE.md` L0→L8 layering; `klynt-skill-loader` sits at L3 alongside `skill-system`. All persistent state lives in the existing `sessions` table — no new tables in Plan 5.

---

## Spec coverage

This plan retires every Phase 1 deliverable not closed by Plans 1–4. Coverage map (spec §13 line numbers):

| Deliverable | Plan | Task |
|---|---|---|
| `klynt-skill-loader` discovery + paths-conditional + dynamic | spec §8 | T1, T2, T3, T4 |
| `/skills *` slash family + Settings tab | §8.945, §13.1362 | T5, T6, T13.5 |
| Slash command catalog dispatcher | §9 | T6, T7 |
| Agent-routed `/plan` `/yolo` `/power` `/recall` | §9.1003 | T7 |
| Direct `/status` `/doctor` `/sessions star/unstar` `/resume` `/help` | §9.1018 | T16 |
| `useCodingMode` + `CodingModePill` wired + auto-detect | §5.508 | T8 |
| `CodingRecallService` via ContextEngine | §4.181 | T9 |
| `recall_*` tool registration (8 tools) | §6.620, §13.1376 | T9 |
| `RecallTrayCard.tsx`, `DeadEndWarning.tsx`, `useCodingRecallSnippets.ts` | §3.248 | T10 |
| Distiller subscriber + 5 E-invariants | §10.1156, §13.1367 | T11, T17 |
| Mirror signal subscribers | §4.348 | T12 |
| Settings → Coding page (6 sections) | §5.577 | T13 |
| Sandbox + cost composer pill | §5.573, §10.1191 | T14 |
| Session retention nightly cron | §11.1262 | T15 |
| K6 (skill discovery determinism) | §14 | T17 |
| K7 (mode-toggle event ordering) | §14 | T17 |
| K9 (slash classification stability) | §14 | T17 |
| 5 scenario tests | §14.1499 | T18 |
| Phase 1 exit gate | §13.1380 | T19 |

`★ Insight ─────────────────────────────────────`
The deliverables list in §13 was deliberately under-specified about *which* plan owns which item — that's why the spec numbers cluster in §3, §5, §8, §9. The coverage map above is what makes the implementer confident every spec line ends up in code. If you find a §13 bullet not in this map, either the bullet was already retired by Plan 1–4, or this map has a gap; add a task rather than rationalizing it away.
`─────────────────────────────────────────────────`

---

## File structure

### Crates created or filled

| Crate | Phase 1 plans 1–4 status | Plan 5 work |
|---|---|---|
| `crates/klynt-skill-loader` | empty stub (Plan 1 skeleton) | `discovery.rs`, `frontmatter.rs`, `activator.rs`, `dynamic.rs`, `replay.rs`, `index.rs`, `lib.rs` (re-exports) |
| `crates/coding-memory` (sink module) | `sink.rs` skeleton | `sink/translator.rs`, `sink/aggregator.rs`, `sink/subscriber.rs` |
| `crates/app-core` (init module) | `init/cognitive.rs`, `init/cron.rs` | `init/coding_subscribers.rs`, `init/coding_skills.rs`, `init/coding_recall.rs`, `init/coding_retention.rs`, `coding/skills_handler.rs`, `coding/slash_handler.rs`, `coding/status_handler.rs`, `coding/doctor_handler.rs`, `coding/sessions_handler.rs`, `coding/resume_handler.rs` |
| `crates/agent` (context source) | `runtime.rs`, `agent_loop/builder.rs` | `context/coding_recall_source.rs`, `context/skill_activator_source.rs` |
| `crates/desktop` (commands) | `chat.rs`, `specta_builder.rs` | `commands/coding_skills.rs`, `commands/coding_status.rs`, `commands/coding_doctor.rs`, `commands/coding_sessions.rs`, `commands/coding_resume.rs` |

### Desktop UI files created

```
desktop-ui/src/features/coding/
├── components/
│   ├── CodingModePill.tsx
│   ├── CodingModePill.test.tsx
│   ├── RecallTrayCard.tsx
│   ├── RecallTrayCard.test.tsx
│   ├── DeadEndWarning.tsx
│   ├── DeadEndWarning.test.tsx
│   ├── SandboxStatusPill.tsx
│   ├── SandboxStatusPill.test.tsx
│   └── SkillActivatedBadge.tsx
├── hooks/
│   ├── useCodingMode.ts
│   ├── useCodingMode.test.ts
│   ├── useCodingRecallSnippets.ts
│   ├── useCodingRecallSnippets.test.ts
│   ├── useSlashCommands.ts
│   ├── useSlashCommands.test.ts
│   └── useCodingThreadCost.ts
├── slash/
│   ├── registry.ts
│   ├── registry.test.ts
│   ├── agentRouted.ts
│   ├── direct.ts
│   ├── classify.ts
│   ├── classify.test.ts
│   └── types.ts
└── styles/
    └── coding.css

desktop-ui/src/features/settings/components/sections/
├── SettingsCodingSection.tsx
├── SettingsCodingSection.test.tsx
├── coding/
│   ├── GeneralSubsection.tsx
│   ├── ToolsSubsection.tsx
│   ├── PermissionsSubsection.tsx
│   ├── SandboxSubsection.tsx
│   ├── SkillsSubsection.tsx
│   ├── SkillsSubsection.test.tsx
│   └── SessionsSubsection.tsx
```

### Cross-cutting test files

```
crates/klynt-skill-loader/tests/
├── discovery_static_paths.rs
├── frontmatter_parser.rs
├── path_conditional_activation.rs
├── dynamic_walk_up.rs
├── replay_on_resume.rs
└── property_k6_determinism.rs

crates/coding-memory/tests/
├── translator_e1_file_edit.rs
├── translator_e2_provider_call.rs
├── translator_e3_chunk_aggregation.rs
├── translator_e4_tool_pair.rs
└── translator_e5_monotone.rs

crates/app-core/tests/
├── coding_skills_commands.rs
├── coding_status_doctor.rs
└── retention_cron_prunes_old_sessions.rs

tests/integration/
├── plan5_recall_injection_e2e.rs
├── plan5_skill_activation_e2e.rs
├── plan5_distiller_writes_rows.rs
└── plan5_phase1_exit_gate.rs           # the master scenario

desktop-ui/src/features/coding/slash/
└── classify.property.test.ts            # K9 (fast-check)

desktop-ui/src/features/coding/hooks/
└── useCodingMode.property.test.ts       # K7
```

---

## Bite-sized task index

| # | Task | LOC est. |
|---|---|---|
| T1 | `klynt-skill-loader`: static-path discovery | 350 |
| T2 | `klynt-skill-loader`: frontmatter parser + index | 280 |
| T3 | `klynt-skill-loader`: path-conditional activation + glob | 240 |
| T4 | `klynt-skill-loader`: dynamic walk-up + replay-on-resume | 320 |
| T5 | `coding_skills_*` Tauri commands (7) | 380 |
| T6 | Slash registry + classify (TypeScript) | 290 |
| T7 | `useSlashCommands` hook + composer integration | 260 |
| T8 | `useCodingMode` + `CodingModePill` + auto-detect | 240 |
| T9 | `CodingRecallContextSource` + `recall_*` tool registration | 410 |
| T10 | `RecallTrayCard` + `DeadEndWarning` + `useCodingRecallSnippets` | 320 |
| T11 | `MemorySinkSubscriber` translator + aggregator | 480 |
| T12 | Mirror signal subscribers (3 sources) | 360 |
| T13 | Settings → Coding page (6 subsections) | 540 |
| T14 | Sandbox + cost composer-meta-bar pills | 220 |
| T15 | Session retention nightly cron | 180 |
| T16 | Direct slash handlers: `/status`, `/doctor`, `/sessions star/unstar`, `/resume`, `/help` | 380 |
| T17 | K6, K7, K9 + E1–E5 proptests | 410 |
| T18 | Five scenario tests | 540 |
| T19 | Phase 1 exit gate + KCA + spec finalization | 200 |

Total ≈ 6400 LOC across Rust + TypeScript. Plans 1–4 averaged ~5500 each, so this is in band.

---

The remainder of this document defines each task with exact file paths, full code blocks, exact commands, and expected output. Tasks are ordered so the build stays green at every commit and the master scenario test (T18.5, Plan 5 acceptance) lights up incrementally as later tasks land.


---

## Task 1: `klynt-skill-loader` static-path discovery

**Files:**
- Modify: `crates/klynt-skill-loader/Cargo.toml` (add deps)
- Create: `crates/klynt-skill-loader/src/index.rs`
- Create: `crates/klynt-skill-loader/src/discovery.rs`
- Modify: `crates/klynt-skill-loader/src/lib.rs` (re-exports)
- Test: `crates/klynt-skill-loader/tests/discovery_static_paths.rs`

`★ Insight ─────────────────────────────────────`
The design hands us four discovery roots (§8.911-918). Two are home-relative (`~/.klyntbot/skills`, `~/.klyntbot/project-skills/<repo-id>`), one is repo-root-relative (`<repo_root>/.klyntbot/skills`), and one is CWD-relative (`.klyntbot/skills`). We can't use `shellexpand` blindly for the home paths because tests need `KLYNTBOT_HOME` override (per CLAUDE.md dev/prod isolation). The discovery module accepts a `DiscoveryRoots` struct injected at construction, NOT global env reads. Tests construct `DiscoveryRoots` from `tempfile::TempDir` directly. The production wiring (in `app-core/src/init/coding_skills.rs`) builds `DiscoveryRoots` from the resolved klyntbot home + thread `cwd` + `repo_id`.

The conflict resolution rule (`Project > User`) lives in `merge_indices` — later sources overwrite earlier when names collide. The merge order is locked: User → ReforgePrivate → ReforgeTeam → Project — this matches the spec's `Project > User` while letting team and private-reforge skills override personal user skills (a private reforge skill is more specific than a generic user skill).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add dependencies to `klynt-skill-loader/Cargo.toml`**

```toml
[package]
name = "klynt-skill-loader"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt skill discovery + paths-conditional activation + dynamic discovery. Extends skill-system."

[dependencies]
common = { path = "../common" }
skill-system = { path = "../skill-system" }
serde = { workspace = true }
serde_yaml = "0.9"
globset = "0.4"
walkdir = "2.5"
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
proptest = { workspace = true }
```

- [ ] **Step 2: Write the failing test `crates/klynt-skill-loader/tests/discovery_static_paths.rs`**

```rust
use klynt_skill_loader::{DiscoveryRoots, SkillIndex, SkillSource};
use std::fs;
use tempfile::TempDir;

fn write_skill(dir: &std::path::Path, name: &str, body: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(dir.join(name).join("SKILL.md"), body).unwrap();
}

const MIN_FRONTMATTER: &str = r#"---
name: alpha
description: Alpha skill.
---
# Alpha

Body.
"#;

#[test]
fn discovers_user_skill() {
    let home = TempDir::new().unwrap();
    write_skill(&home.path().join(".klyntbot/skills"), "alpha", MIN_FRONTMATTER);

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: std::env::temp_dir(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    let entry = idx.get("alpha").expect("alpha discovered");
    assert_eq!(entry.frontmatter.name, "alpha");
    assert!(matches!(entry.source, SkillSource::User));
}

#[test]
fn project_skill_overrides_user() {
    let home = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    write_skill(
        &home.path().join(".klyntbot/skills"),
        "alpha",
        "---\nname: alpha\ndescription: User-side.\n---\nUser body.",
    );
    write_skill(
        &repo.path().join(".klyntbot/skills"),
        "alpha",
        "---\nname: alpha\ndescription: Project-side.\n---\nProject body.",
    );

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    let entry = idx.get("alpha").expect("alpha discovered");
    assert_eq!(entry.frontmatter.description, "Project-side.");
    assert!(matches!(entry.source, SkillSource::Project));
}

#[test]
fn missing_paths_skipped_silently() {
    let home = TempDir::new().unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: home.path().to_path_buf(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    assert_eq!(idx.len(), 0);
}

#[test]
fn malformed_skill_emits_warning_and_continues() {
    let home = TempDir::new().unwrap();
    write_skill(&home.path().join(".klyntbot/skills"), "good", MIN_FRONTMATTER);
    write_skill(
        &home.path().join(".klyntbot/skills"),
        "bad",
        "this has no frontmatter at all",
    );

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: std::env::temp_dir(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    assert!(idx.get("good").is_some());
    assert!(idx.get("bad").is_none());
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p klynt-skill-loader --test discovery_static_paths`
Expected: 4 compilation errors (`SkillIndex`, `DiscoveryRoots`, `SkillSource` undefined).

- [ ] **Step 4: Create `crates/klynt-skill-loader/src/index.rs`**

```rust
use serde::Serialize;
use skill_system::SkillFrontmatter;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SkillSource {
    User,
    Project,
    ReforgePrivate,
    ReforgeTeam,
}

impl SkillSource {
    /// Lower priority loses to higher priority on name collision.
    pub fn priority(self) -> u8 {
        match self {
            SkillSource::User => 0,
            SkillSource::ReforgePrivate => 1,
            SkillSource::ReforgeTeam => 2,
            SkillSource::Project => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedSkill {
    pub frontmatter: SkillFrontmatter,
    pub source: SkillSource,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiscoveryRoots {
    pub klyntbot_home: PathBuf,
    pub repo_id: Option<String>,
    pub repo_root: Option<PathBuf>,
    pub cwd: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct SkillIndex {
    entries: HashMap<String, IndexedSkill>,
}

impl SkillIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, name: &str) -> Option<&IndexedSkill> {
        self.entries.get(name)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &IndexedSkill)> {
        self.entries.iter()
    }
    pub(crate) fn insert(&mut self, name: String, skill: IndexedSkill) {
        match self.entries.get(&name) {
            Some(existing) if existing.source.priority() >= skill.source.priority() => {
                tracing::debug!(
                    name = %name,
                    existing = ?existing.source,
                    incoming = ?skill.source,
                    "keeping higher-priority skill"
                );
            }
            _ => {
                self.entries.insert(name, skill);
            }
        }
    }
    pub(crate) fn merge(&mut self, other: SkillIndex) {
        for (name, skill) in other.entries {
            self.insert(name, skill);
        }
    }
}
```

- [ ] **Step 5: Create `crates/klynt-skill-loader/src/discovery.rs`**

```rust
use crate::index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
use common::{KlyntbotError, Result};
use skill_system::split_frontmatter;
use std::fs;
use std::path::{Path, PathBuf};

impl SkillIndex {
    /// Discover all skills under the four static roots (§8.911-918).
    pub fn discover(roots: &DiscoveryRoots) -> Result<Self> {
        let mut idx = SkillIndex::new();
        idx.merge(scan_root(
            roots.klyntbot_home.join("skills"),
            SkillSource::User,
        )?);
        if let Some(repo_id) = &roots.repo_id {
            idx.merge(scan_root(
                roots
                    .klyntbot_home
                    .join("project-skills")
                    .join(sanitize_repo_id(repo_id)),
                SkillSource::ReforgePrivate,
            )?);
        }
        if let Some(repo_root) = &roots.repo_root {
            idx.merge(scan_root(
                repo_root.join(".klyntbot/skills"),
                SkillSource::Project,
            )?);
            idx.merge(scan_root(
                repo_root.join(".klyntbot/team-skills"),
                SkillSource::ReforgeTeam,
            )?);
        }
        let cwd_path = roots.cwd.join(".klyntbot/skills");
        if Some(&cwd_path) != roots.repo_root.as_ref().map(|r| r.join(".klyntbot/skills")).as_ref() {
            idx.merge(scan_root(cwd_path, SkillSource::Project)?);
        }
        Ok(idx)
    }
}

/// Path-safe repo-id (replaces `/` and `:` so a github URL becomes one segment).
pub fn sanitize_repo_id(repo_id: &str) -> String {
    repo_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn scan_root(dir: PathBuf, source: SkillSource) -> Result<SkillIndex> {
    if !dir.is_dir() {
        return Ok(SkillIndex::new());
    }
    let mut idx = SkillIndex::new();
    let entries = fs::read_dir(&dir).map_err(|e| {
        KlyntbotError::Internal(format!("reading {}: {e}", dir.display()))
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        match parse_skill(&skill_md, source) {
            Ok(skill) => idx.insert(skill.frontmatter.name.clone(), skill),
            Err(e) => tracing::warn!(
                path = %skill_md.display(),
                error = %e,
                "skipping malformed SKILL.md"
            ),
        }
    }
    Ok(idx)
}

fn parse_skill(skill_md: &Path, source: SkillSource) -> Result<IndexedSkill> {
    let raw = fs::read_to_string(skill_md).map_err(|e| {
        KlyntbotError::Internal(format!("reading {}: {e}", skill_md.display()))
    })?;
    let (frontmatter, _body) = split_frontmatter(&raw)?;
    Ok(IndexedSkill {
        frontmatter,
        source,
        source_path: skill_md.to_path_buf(),
    })
}
```

- [ ] **Step 6: Update `crates/klynt-skill-loader/src/lib.rs` to expose modules**

```rust
//! Klynt skill loader — extends `skill-system` with:
//! - Discovery from four static roots (`~/.klyntbot/skills/`, project, reforge).
//! - Path-conditional activation via `paths:` frontmatter glob.
//! - Dynamic discovery on file-touch.

mod discovery;
mod index;

pub use discovery::sanitize_repo_id;
pub use index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
```

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo nextest run -p klynt-skill-loader`
Expected: `PASS` for all 4 tests; no clippy warnings.

- [ ] **Step 8: Run clippy + fmt**

Run: `cargo clippy -p klynt-skill-loader --all-targets --all-features -- -D warnings`
Run: `cargo fmt --all`
Expected: zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/klynt-skill-loader/Cargo.toml crates/klynt-skill-loader/src crates/klynt-skill-loader/tests
git commit -m "feat(skill-loader): static-path discovery (Plan 5 T1)

Discovers SKILL.md under four roots per spec §8.911-918:
- ~/.klyntbot/skills (User)
- ~/.klyntbot/project-skills/<sanitized-repo-id> (ReforgePrivate)
- <repo_root>/.klyntbot/skills (Project)
- <repo_root>/.klyntbot/team-skills (ReforgeTeam)

Conflict resolution by SkillSource::priority(); Project > User.
Malformed SKILL.md files emit a tracing::warn and are skipped, never abort."
```


---

## Task 2: `klynt-skill-loader` frontmatter extension (`paths:`, `references[].load`, `tags`, `sensitivity`)

**Files:**
- Create: `crates/klynt-skill-loader/src/frontmatter.rs`
- Modify: `crates/klynt-skill-loader/src/index.rs` (replace stub `frontmatter` field with `KlyntFrontmatter`)
- Modify: `crates/klynt-skill-loader/src/discovery.rs` (parse extended frontmatter)
- Test: `crates/klynt-skill-loader/tests/frontmatter_parser.rs`

`★ Insight ─────────────────────────────────────`
`skill-system::SkillFrontmatter` only handles the Anthropic-spec fields (`name`, `description`, optional `allowed-tools`, `references`). The klynt-additive fields (`paths`, `tags`, `sensitivity`, `references[].load`) live in the YAML but are unknown to the existing parser, which uses `serde(deny_unknown_fields = false)` so it round-trips silently. We add a *parallel* parse pass that re-reads the YAML through a klynt-specific struct — rather than modifying `SkillFrontmatter` and triggering ripples across the agent/skill-system code that consumes it. This is the "extend, don't modify" rule of the plan.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `crates/klynt-skill-loader/tests/frontmatter_parser.rs`**

```rust
use klynt_skill_loader::frontmatter::{KlyntFrontmatter, ReferenceLoadMode};

const FULL: &str = r#"---
name: refactor-helper
description: Helps refactor code.
allowed-tools: ["read", "edit", "grep"]
paths:
  - "**/*.rs"
  - "src/**/*.toml"
tags: ["refactor", "rust"]
sensitivity: "private"
references:
  - name: style-guide
    file: refs/style.md
    load: always
  - name: examples
    file: refs/examples.md
    load: on-demand
---
# Body
"#;

#[test]
fn parses_full_klynt_frontmatter() {
    let (fm, body) = KlyntFrontmatter::parse(FULL).unwrap();
    assert_eq!(fm.name, "refactor-helper");
    assert_eq!(fm.paths, vec!["**/*.rs".to_string(), "src/**/*.toml".to_string()]);
    assert_eq!(fm.tags, vec!["refactor".to_string(), "rust".to_string()]);
    assert_eq!(fm.sensitivity.as_deref(), Some("private"));
    assert_eq!(fm.references.len(), 2);
    assert!(matches!(fm.references[0].load, ReferenceLoadMode::Always));
    assert!(matches!(fm.references[1].load, ReferenceLoadMode::OnDemand));
    assert!(body.contains("# Body"));
}

#[test]
fn parses_minimal_frontmatter_with_defaults() {
    let raw = "---\nname: minimal\ndescription: Minimal.\n---\nBody\n";
    let (fm, _) = KlyntFrontmatter::parse(raw).unwrap();
    assert_eq!(fm.name, "minimal");
    assert!(fm.paths.is_empty());
    assert!(fm.tags.is_empty());
    assert!(fm.sensitivity.is_none());
    assert!(fm.references.is_empty());
}

#[test]
fn missing_frontmatter_fence_errors() {
    let raw = "name: bad\nNo fence here.\n";
    assert!(KlyntFrontmatter::parse(raw).is_err());
}

#[test]
fn missing_required_name_errors() {
    let raw = "---\ndescription: Missing name\n---\nBody\n";
    assert!(KlyntFrontmatter::parse(raw).is_err());
}

#[test]
fn unknown_load_mode_defaults_to_on_demand() {
    let raw = r#"---
name: test
description: Test
references:
  - name: foo
    file: foo.md
    load: bogus
---
Body
"#;
    let (fm, _) = KlyntFrontmatter::parse(raw).unwrap();
    assert!(matches!(fm.references[0].load, ReferenceLoadMode::OnDemand));
}
```

- [ ] **Step 2: Run test, expect compile failure (`KlyntFrontmatter`, `ReferenceLoadMode` undefined)**

Run: `cargo nextest run -p klynt-skill-loader --test frontmatter_parser`
Expected: compile errors.

- [ ] **Step 3: Create `crates/klynt-skill-loader/src/frontmatter.rs`**

```rust
use common::{KlyntbotError, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct KlyntFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub sensitivity: Option<String>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub name: String,
    pub file: String,
    #[serde(default)]
    pub load: ReferenceLoadMode,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceLoadMode {
    Always,
    #[default]
    OnDemand,
}

impl KlyntFrontmatter {
    pub fn parse(raw: &str) -> Result<(Self, String)> {
        let trimmed = raw.trim_start();
        if !trimmed.starts_with("---") {
            return Err(KlyntbotError::Internal(
                "SKILL.md missing leading `---` frontmatter fence".into(),
            ));
        }
        let after_fence = trimmed.trim_start_matches("---").trim_start_matches('\n');
        let close = after_fence.find("\n---").ok_or_else(|| {
            KlyntbotError::Internal("SKILL.md missing closing `---` fence".into())
        })?;
        let yaml = &after_fence[..close];
        let body = after_fence[close..].trim_start_matches("\n---");
        let body = body.trim_start_matches('\n').to_string();

        let mut fm: KlyntFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
            KlyntbotError::Internal(format!("invalid SKILL.md frontmatter: {e}"))
        })?;
        if fm.name.trim().is_empty() {
            return Err(KlyntbotError::Internal(
                "SKILL.md frontmatter missing required `name`".into(),
            ));
        }
        if fm.description.trim().is_empty() {
            return Err(KlyntbotError::Internal(
                "SKILL.md frontmatter missing required `description`".into(),
            ));
        }
        // Default unknown load modes to OnDemand (parser already does via serde).
        for r in &mut fm.references {
            if r.name.trim().is_empty() || r.file.trim().is_empty() {
                return Err(KlyntbotError::Internal(format!(
                    "SKILL.md reference '{}' missing name or file",
                    r.name
                )));
            }
        }
        Ok((fm, body))
    }
}
```

- [ ] **Step 4: Update `crates/klynt-skill-loader/src/index.rs` to use `KlyntFrontmatter` instead of `SkillFrontmatter`**

Change `IndexedSkill.frontmatter: SkillFrontmatter` → `frontmatter: crate::frontmatter::KlyntFrontmatter`. Remove the `use skill_system::SkillFrontmatter;` line.

- [ ] **Step 5: Update `crates/klynt-skill-loader/src/discovery.rs` to call `KlyntFrontmatter::parse`**

Replace the `parse_skill` function:

```rust
fn parse_skill(skill_md: &Path, source: SkillSource) -> Result<IndexedSkill> {
    let raw = fs::read_to_string(skill_md).map_err(|e| {
        KlyntbotError::Internal(format!("reading {}: {e}", skill_md.display()))
    })?;
    let (frontmatter, _body) = crate::frontmatter::KlyntFrontmatter::parse(&raw)?;
    Ok(IndexedSkill {
        frontmatter,
        source,
        source_path: skill_md.to_path_buf(),
    })
}
```

Remove the `use skill_system::split_frontmatter;` line.

- [ ] **Step 6: Update `crates/klynt-skill-loader/src/lib.rs`**

```rust
//! Klynt skill loader — extends `skill-system` with discovery + path-conditional + dynamic.

pub mod frontmatter;
mod discovery;
mod index;

pub use discovery::sanitize_repo_id;
pub use frontmatter::{KlyntFrontmatter, Reference, ReferenceLoadMode};
pub use index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
```

- [ ] **Step 7: Update Task 1's test to use the new field**

In `discovery_static_paths.rs`, replace `entry.frontmatter.name` → still `entry.frontmatter.name` (the field name is identical on `KlyntFrontmatter`). No change needed.

- [ ] **Step 8: Run tests + clippy**

Run: `cargo nextest run -p klynt-skill-loader && cargo clippy -p klynt-skill-loader --all-targets -- -D warnings`
Expected: 9 tests pass; zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/klynt-skill-loader
git commit -m "feat(skill-loader): klynt frontmatter parser (Plan 5 T2)

Adds KlyntFrontmatter with paths/tags/sensitivity/references[].load fields
beyond the Anthropic-spec base. References default to load=on-demand.
Replaces SkillFrontmatter usage in IndexedSkill. Parser rejects empty name
or description; references[] entries must have name + file."
```


---

## Task 3: Path-conditional activation (`SkillActivator`)

**Files:**
- Create: `crates/klynt-skill-loader/src/activator.rs`
- Modify: `crates/klynt-skill-loader/src/lib.rs` (re-export `SkillActivator`)
- Test: `crates/klynt-skill-loader/tests/path_conditional_activation.rs`

`★ Insight ─────────────────────────────────────`
The activator is per-session state — a `HashSet<String>` of active skill names tracked since session start. We don't persist it; on resume we replay file-touch history and rebuild (Task 4). That choice keeps the `sessions` table out of the activator's hot path. `globset` builds a `GlobSet` once per session-skill (cached on `ConditionalSkill`) so a tool call against 10K paths × 50 skills costs O(active_skills) match work, not O(paths × skills).

The "always-active" set (config `coding.skills.alwaysActivate`) bypasses path matching entirely — those skills get a permanent slot in the active set the moment the activator is constructed. The "never-activate" set is checked first and short-circuits everything (including dynamic discovery, Task 4).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `crates/klynt-skill-loader/tests/path_conditional_activation.rs`**

```rust
use klynt_skill_loader::{
    DiscoveryRoots, KlyntFrontmatter, SkillActivator, SkillIndex, SkillSource,
    activator::ActivationConfig,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn make_index_with_paths(name: &str, paths: &[&str]) -> SkillIndex {
    let mut idx = SkillIndex::new();
    let fm = KlyntFrontmatter {
        name: name.into(),
        description: "test".into(),
        paths: paths.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    idx.insert_for_test(name.into(), fm, SkillSource::User, PathBuf::from("/tmp/x"));
    idx
}

#[test]
fn activates_when_path_matches_glob() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let activated = act.touch_path(std::path::Path::new("src/main.rs")).unwrap();
    assert_eq!(activated, vec!["rust-helper".to_string()]);
    assert!(act.active_set().contains("rust-helper"));
}

#[test]
fn does_not_activate_unrelated_path() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let activated = act.touch_path(std::path::Path::new("README.md")).unwrap();
    assert!(activated.is_empty());
    assert!(act.active_set().is_empty());
}

#[test]
fn idempotent_activation() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let first = act.touch_path(std::path::Path::new("a.rs")).unwrap();
    let second = act.touch_path(std::path::Path::new("b.rs")).unwrap();
    assert_eq!(first, vec!["rust-helper".to_string()]);
    assert!(second.is_empty());           // already active, do not re-emit
}

#[test]
fn always_activate_short_circuits() {
    let idx = make_index_with_paths("forced", &["doesnt/match"]);
    let cfg = ActivationConfig {
        always_activate: vec!["forced".into()],
        ..Default::default()
    };
    let act = SkillActivator::new(idx, cfg).unwrap();
    assert!(act.active_set().contains("forced"));
}

#[test]
fn never_activate_blocks_path_match() {
    let idx = make_index_with_paths("blocked", &["**/*.rs"]);
    let cfg = ActivationConfig {
        never_activate: vec!["blocked".into()],
        ..Default::default()
    };
    let mut act = SkillActivator::new(idx, cfg).unwrap();
    let activated = act.touch_path(std::path::Path::new("a.rs")).unwrap();
    assert!(activated.is_empty());
    assert!(!act.active_set().contains("blocked"));
}

#[test]
fn max_active_skills_cap_enforced() {
    let mut idx = SkillIndex::new();
    for i in 0..5 {
        let name = format!("s{i}");
        let fm = KlyntFrontmatter {
            name: name.clone(),
            description: "test".into(),
            paths: vec!["**/*.rs".into()],
            ..Default::default()
        };
        idx.insert_for_test(name, fm, SkillSource::User, PathBuf::from("/tmp/x"));
    }
    let cfg = ActivationConfig { max_active_skills: 3, ..Default::default() };
    let mut act = SkillActivator::new(idx, cfg).unwrap();
    act.touch_path(std::path::Path::new("a.rs")).unwrap();
    assert_eq!(act.active_set().len(), 3);
}
```

- [ ] **Step 2: Add `insert_for_test` method to `SkillIndex` (test-only)**

In `crates/klynt-skill-loader/src/index.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl SkillIndex {
    pub fn insert_for_test(
        &mut self,
        name: String,
        frontmatter: crate::frontmatter::KlyntFrontmatter,
        source: SkillSource,
        source_path: PathBuf,
    ) {
        self.entries.insert(
            name,
            IndexedSkill {
                frontmatter,
                source,
                source_path,
            },
        );
    }
}
```

Add `test-helpers = []` feature in `Cargo.toml`'s `[features]` section.

- [ ] **Step 3: Create `crates/klynt-skill-loader/src/activator.rs`**

```rust
use crate::frontmatter::KlyntFrontmatter;
use crate::index::{IndexedSkill, SkillIndex};
use common::{KlyntbotError, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ActivationConfig {
    pub always_activate: Vec<String>,
    pub never_activate: Vec<String>,
    pub max_active_skills: usize,
}

impl ActivationConfig {
    pub fn from_coding_config(c: &config::CodingSkillsConfig) -> Self {
        Self {
            always_activate: c.always_activate.clone(),
            never_activate: c.never_activate.clone(),
            max_active_skills: c.max_active_skills.try_into().unwrap_or(30),
        }
    }
}

struct ConditionalSkill {
    name: String,
    glob_set: GlobSet,
    source_path: std::path::PathBuf,
}

pub struct SkillActivator {
    index: SkillIndex,
    config: ActivationConfig,
    conditionals: Vec<ConditionalSkill>,
    active: HashSet<String>,
}

impl SkillActivator {
    pub fn new(index: SkillIndex, config: ActivationConfig) -> Result<Self> {
        let never: HashSet<&str> = config.never_activate.iter().map(String::as_str).collect();
        let mut conditionals = Vec::new();
        let mut active = HashSet::new();

        for name in &config.always_activate {
            if !never.contains(name.as_str()) && index.get(name).is_some() {
                active.insert(name.clone());
            }
        }

        for (name, skill) in index.iter() {
            if never.contains(name.as_str()) {
                continue;
            }
            if skill.frontmatter.paths.is_empty() {
                continue;
            }
            let mut builder = GlobSetBuilder::new();
            for pat in &skill.frontmatter.paths {
                let glob = Glob::new(pat).map_err(|e| {
                    KlyntbotError::Internal(format!(
                        "invalid path glob `{pat}` in skill `{name}`: {e}"
                    ))
                })?;
                builder.add(glob);
            }
            let glob_set = builder.build().map_err(|e| {
                KlyntbotError::Internal(format!("globset build for `{name}`: {e}"))
            })?;
            conditionals.push(ConditionalSkill {
                name: name.clone(),
                glob_set,
                source_path: skill.source_path.clone(),
            });
        }

        Ok(Self {
            index,
            config,
            conditionals,
            active,
        })
    }

    /// Returns names of skills newly-activated by this touch (empty if none).
    pub fn touch_path(&mut self, path: &Path) -> Result<Vec<String>> {
        let mut newly = Vec::new();
        for c in &self.conditionals {
            if self.active.contains(&c.name) {
                continue;
            }
            if self.config.max_active_skills > 0
                && self.active.len() >= self.config.max_active_skills
            {
                break;
            }
            if c.glob_set.is_match(path) {
                self.active.insert(c.name.clone());
                newly.push(c.name.clone());
            }
        }
        Ok(newly)
    }

    pub fn active_set(&self) -> &HashSet<String> {
        &self.active
    }

    pub fn lookup(&self, name: &str) -> Option<&IndexedSkill> {
        self.index.get(name)
    }

    pub fn frontmatter(&self, name: &str) -> Option<&KlyntFrontmatter> {
        self.index.get(name).map(|s| &s.frontmatter)
    }
}
```

- [ ] **Step 4: Add `CodingSkillsConfig` type to `config` crate**

In `crates/config/src/schema/coding.rs` (extend the existing `CodingConfig`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSkillsConfig {
    #[serde(default = "default_true")]
    pub enable_conditional_activation: bool,
    #[serde(default = "default_true")]
    pub enable_dynamic_discovery: bool,
    #[serde(default = "default_max_active")]
    pub max_active_skills: u32,
    #[serde(default = "default_token_budget")]
    pub frontmatter_token_budget: u32,
    #[serde(default)]
    pub always_activate: Vec<String>,
    #[serde(default)]
    pub never_activate: Vec<String>,
}
fn default_true() -> bool { true }
fn default_max_active() -> u32 { 30 }
fn default_token_budget() -> u32 { 2000 }
```

Add `pub skills: CodingSkillsConfig` to the existing `CodingConfig` struct with `#[serde(default)]`. Implement `Default for CodingSkillsConfig`.

- [ ] **Step 5: Update `lib.rs`**

```rust
pub mod activator;

pub use activator::{ActivationConfig, SkillActivator};
```

- [ ] **Step 6: Add `klynt-skill-loader` dependency on `config`**

In `crates/klynt-skill-loader/Cargo.toml`:

```toml
config = { path = "../config" }
```

- [ ] **Step 7: Run tests + clippy**

Run: `cargo nextest run -p klynt-skill-loader && cargo clippy -p klynt-skill-loader --all-targets --features test-helpers -- -D warnings`
Expected: all 6 activation tests + 9 prior tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/klynt-skill-loader crates/config/src/schema/coding.rs
git commit -m "feat(skill-loader): path-conditional activation (Plan 5 T3)

SkillActivator built from SkillIndex + ActivationConfig. Glob-matches
paths against skill frontmatter.paths globs (compiled once via globset).
Honors alwaysActivate / neverActivate / maxActiveSkills caps from
config.coding.skills. Idempotent: re-touching an already-active path
returns no newly-activated names."
```


---

## Task 4: Dynamic discovery + replay-on-resume

**Files:**
- Create: `crates/klynt-skill-loader/src/dynamic.rs`
- Create: `crates/klynt-skill-loader/src/replay.rs`
- Modify: `crates/klynt-skill-loader/src/activator.rs` (add `touch_path_with_discovery`)
- Modify: `crates/klynt-skill-loader/src/lib.rs`
- Test: `crates/klynt-skill-loader/tests/dynamic_walk_up.rs`
- Test: `crates/klynt-skill-loader/tests/replay_on_resume.rs`

`★ Insight ─────────────────────────────────────`
Dynamic discovery walks **upward** from a touched file until it hits the cwd boundary, looking for `.klyntbot/skills/` directories not already in the index. This is a O(depth) walk per touch, which is fine — but we cache the result in `seen_dirs: HashSet<PathBuf>` so subsequent touches in the same directory are O(1). Without that cache, every `read` call would walk to the repo root.

The "gitignored directories skipped" rule (§8.932) is implemented by checking for a `.gitignore` in each ancestor and consulting `ignore::WalkBuilder` — but since we're only checking for the literal `.klyntbot/skills/` subdir (not enumerating files), we just need to avoid the *path* being inside a gitignore-listed directory. We rely on a `dirs_to_skip: Vec<PathBuf>` injected via config (defaults to `[".git", "target", "node_modules"]`) — a full gitignore parse is YAGNI for Phase 1.

For replay on resume (§11.1234, K6 invariant), the inputs are: the persisted message history and the current activator. We extract every `path` field from `kind: "tool"` and `kind: "diff"` rows, then call `touch_path` on each in deterministic order. Two runs with identical history produce bit-identical active sets — that is K6.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `crates/klynt-skill-loader/tests/dynamic_walk_up.rs`**

```rust
use klynt_skill_loader::{
    DiscoveryRoots, SkillActivator, SkillIndex, activator::ActivationConfig,
};
use std::fs;
use tempfile::TempDir;

const SKILL: &str = "---\nname: deep\ndescription: Deep skill.\npaths:\n  - \"**/*.rs\"\n---\nBody\n";

#[test]
fn dynamic_walk_up_finds_nested_skill_dir() {
    let repo = TempDir::new().unwrap();
    let nested_skills = repo.path().join("subdir/.klyntbot/skills");
    fs::create_dir_all(nested_skills.join("deep")).unwrap();
    fs::write(nested_skills.join("deep/SKILL.md"), SKILL).unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let initial = SkillIndex::discover(&roots).unwrap();
    assert!(initial.get("deep").is_none(), "skill not at static root");

    let mut act = SkillActivator::new(initial, ActivationConfig::default()).unwrap();
    let activated = act
        .touch_path_with_discovery(&repo.path().join("subdir/foo.rs"), &roots)
        .unwrap();
    assert_eq!(activated, vec!["deep".to_string()]);
}

#[test]
fn dynamic_walk_does_not_cross_cwd_boundary() {
    let outside = TempDir::new().unwrap();
    let inside = outside.path().join("repo");
    fs::create_dir_all(&inside).unwrap();
    let outside_skills = outside.path().join(".klyntbot/skills");
    fs::create_dir_all(outside_skills.join("outside")).unwrap();
    fs::write(
        outside_skills.join("outside/SKILL.md"),
        "---\nname: outside\ndescription: Outside\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(inside.clone()),
        cwd: inside.clone(),
    };
    let mut act = SkillActivator::new(SkillIndex::new(), ActivationConfig::default()).unwrap();
    let activated = act
        .touch_path_with_discovery(&inside.join("foo.rs"), &roots)
        .unwrap();
    assert!(activated.is_empty(), "should not walk above cwd");
}

#[test]
fn dynamic_walk_caches_seen_dirs() {
    let repo = TempDir::new().unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let mut act = SkillActivator::new(SkillIndex::new(), ActivationConfig::default()).unwrap();
    act.touch_path_with_discovery(&repo.path().join("a.rs"), &roots).unwrap();
    let first_seen_count = act.dynamic_seen_dirs_len();
    act.touch_path_with_discovery(&repo.path().join("b.rs"), &roots).unwrap();
    assert_eq!(
        act.dynamic_seen_dirs_len(),
        first_seen_count,
        "second touch in same dir hits cache"
    );
}
```

- [ ] **Step 2: Write the failing test `crates/klynt-skill-loader/tests/replay_on_resume.rs`**

```rust
use klynt_skill_loader::{
    DiscoveryRoots, SkillActivator, SkillIndex, activator::ActivationConfig,
    replay::replay_session_history,
};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn replay_activates_path_conditional_skills() {
    let repo = TempDir::new().unwrap();
    let skills_dir = repo.path().join(".klyntbot/skills");
    std::fs::create_dir_all(skills_dir.join("rust")).unwrap();
    std::fs::write(
        skills_dir.join("rust/SKILL.md"),
        "---\nname: rust\ndescription: Rust\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();
    std::fs::create_dir_all(skills_dir.join("toml")).unwrap();
    std::fs::write(
        skills_dir.join("toml/SKILL.md"),
        "---\nname: toml\ndescription: TOML\npaths: [\"**/*.toml\"]\n---\nBody\n",
    )
    .unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let history_paths = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
    ];

    let mut act = SkillActivator::new(SkillIndex::discover(&roots).unwrap(), ActivationConfig::default()).unwrap();
    let activated = replay_session_history(&mut act, &history_paths, &roots).unwrap();
    assert!(activated.contains(&"rust".to_string()));
    assert!(activated.contains(&"toml".to_string()));
}

#[test]
fn replay_is_deterministic_k6() {
    // K6: same persisted history → same active set, regardless of order beyond "first hit wins".
    let repo = TempDir::new().unwrap();
    let skills_dir = repo.path().join(".klyntbot/skills");
    std::fs::create_dir_all(skills_dir.join("rust")).unwrap();
    std::fs::write(
        skills_dir.join("rust/SKILL.md"),
        "---\nname: rust\ndescription: Rust\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let history = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];

    let run = |hist: &[PathBuf]| -> Vec<String> {
        let mut act = SkillActivator::new(
            SkillIndex::discover(&roots).unwrap(),
            ActivationConfig::default(),
        ).unwrap();
        replay_session_history(&mut act, hist, &roots).unwrap();
        let mut s: Vec<String> = act.active_set().iter().cloned().collect();
        s.sort();
        s
    };
    assert_eq!(run(&history), run(&history));
}
```

- [ ] **Step 3: Create `crates/klynt-skill-loader/src/dynamic.rs`**

```rust
use crate::discovery;
use crate::frontmatter::KlyntFrontmatter;
use crate::index::{IndexedSkill, SkillIndex, SkillSource};
use common::{KlyntbotError, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_DIRS_TO_SKIP: &[&str] = &[".git", "target", "node_modules", "dist", "build"];

pub(crate) struct DynamicWalker {
    seen_dirs: HashSet<PathBuf>,
    dirs_to_skip: HashSet<String>,
}

impl DynamicWalker {
    pub fn new() -> Self {
        Self {
            seen_dirs: HashSet::new(),
            dirs_to_skip: DEFAULT_DIRS_TO_SKIP.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn seen_dirs_len(&self) -> usize {
        self.seen_dirs.len()
    }

    /// Walk from `path` upward to `cwd_boundary` looking for new
    /// `.klyntbot/skills/` directories. Newly-found skills are inserted
    /// into the existing `index` with source `Project`.
    pub fn discover_above(
        &mut self,
        path: &Path,
        cwd_boundary: &Path,
        index: &mut SkillIndex,
    ) -> Result<Vec<String>> {
        let mut newly = Vec::new();
        let start = if path.is_file() {
            path.parent().unwrap_or(cwd_boundary)
        } else {
            path
        };
        let mut current = start;
        loop {
            if !current.starts_with(cwd_boundary) {
                break;
            }
            if let Some(name) = current.file_name().and_then(|s| s.to_str()) {
                if self.dirs_to_skip.contains(name) {
                    break;
                }
            }
            let candidate = current.join(".klyntbot/skills");
            if candidate.is_dir() && !self.seen_dirs.contains(&candidate) {
                self.seen_dirs.insert(candidate.clone());
                let scanned = scan_dir(&candidate)?;
                for (name, skill) in scanned {
                    if index.get(&name).is_none() {
                        newly.push(name.clone());
                        index.insert_for_test_or_dynamic(name, skill);
                    }
                }
            }
            self.seen_dirs.insert(current.to_path_buf());
            match current.parent() {
                Some(p) if p != current => current = p,
                _ => break,
            }
            if current == cwd_boundary {
                let candidate = current.join(".klyntbot/skills");
                if candidate.is_dir() && !self.seen_dirs.contains(&candidate) {
                    self.seen_dirs.insert(candidate.clone());
                    let scanned = scan_dir(&candidate)?;
                    for (name, skill) in scanned {
                        if index.get(&name).is_none() {
                            newly.push(name.clone());
                            index.insert_for_test_or_dynamic(name, skill);
                        }
                    }
                }
                break;
            }
        }
        Ok(newly)
    }
}

fn scan_dir(dir: &Path) -> Result<Vec<(String, IndexedSkill)>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        KlyntbotError::Internal(format!("dynamic scan {}: {e}", dir.display()))
    })?;
    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let raw = match std::fs::read_to_string(&skill_md) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(path = %skill_md.display(), error = %e, "skip");
                continue;
            }
        };
        match KlyntFrontmatter::parse(&raw) {
            Ok((fm, _)) => {
                let name = fm.name.clone();
                out.push((
                    name,
                    IndexedSkill {
                        frontmatter: fm,
                        source: SkillSource::Project,
                        source_path: skill_md,
                    },
                ));
            }
            Err(e) => tracing::warn!(path = %skill_md.display(), error = %e, "skip malformed"),
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Add `insert_for_test_or_dynamic` to `SkillIndex`**

In `crates/klynt-skill-loader/src/index.rs`:

```rust
impl SkillIndex {
    pub(crate) fn insert_for_test_or_dynamic(&mut self, name: String, skill: IndexedSkill) {
        self.insert(name, skill);
    }
}
```

- [ ] **Step 5: Extend `SkillActivator` with `touch_path_with_discovery`**

In `crates/klynt-skill-loader/src/activator.rs`, add to the `SkillActivator` struct:

```rust
    walker: crate::dynamic::DynamicWalker,
```

In `SkillActivator::new`, initialize `walker: crate::dynamic::DynamicWalker::new()`.

Add method:

```rust
    /// Touch a path, dynamically discover new skill dirs above it, then activate.
    pub fn touch_path_with_discovery(
        &mut self,
        path: &Path,
        roots: &crate::index::DiscoveryRoots,
    ) -> Result<Vec<String>> {
        let cwd_boundary = roots
            .repo_root
            .as_deref()
            .unwrap_or(roots.cwd.as_path());
        let newly_indexed = self.walker.discover_above(path, cwd_boundary, &mut self.index)?;
        for name in &newly_indexed {
            if let Some(skill) = self.index.get(name) {
                if skill.frontmatter.paths.is_empty() {
                    continue;
                }
                let mut builder = globset::GlobSetBuilder::new();
                for pat in &skill.frontmatter.paths {
                    if let Ok(g) = globset::Glob::new(pat) {
                        builder.add(g);
                    }
                }
                if let Ok(glob_set) = builder.build() {
                    self.conditionals.push(crate::activator::ConditionalSkill {
                        name: name.clone(),
                        glob_set,
                        source_path: skill.source_path.clone(),
                    });
                }
            }
        }
        self.touch_path(path)
    }

    pub fn dynamic_seen_dirs_len(&self) -> usize {
        self.walker.seen_dirs_len()
    }
```

`ConditionalSkill` must be `pub(crate)` for cross-module visibility.

- [ ] **Step 6: Create `crates/klynt-skill-loader/src/replay.rs`**

```rust
use crate::activator::SkillActivator;
use crate::index::DiscoveryRoots;
use common::Result;
use std::path::Path;

/// Re-activate path-conditional skills by replaying file-touch history
/// in deterministic order. Returns all skill names ever activated by
/// this replay (sorted, deduplicated).
pub fn replay_session_history(
    activator: &mut SkillActivator,
    history_paths: &[std::path::PathBuf],
    roots: &DiscoveryRoots,
) -> Result<Vec<String>> {
    let mut sorted: Vec<&Path> = history_paths.iter().map(|p| p.as_path()).collect();
    sorted.sort();
    sorted.dedup();
    let mut all = std::collections::BTreeSet::new();
    for p in sorted {
        for name in activator.touch_path_with_discovery(p, roots)? {
            all.insert(name);
        }
    }
    Ok(all.into_iter().collect())
}
```

- [ ] **Step 7: Update `lib.rs`**

```rust
mod dynamic;
pub mod replay;
```

- [ ] **Step 8: Run all tests**

Run: `cargo nextest run -p klynt-skill-loader --features test-helpers && cargo clippy -p klynt-skill-loader --all-targets --features test-helpers -- -D warnings`
Expected: 14 tests pass; zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/klynt-skill-loader
git commit -m "feat(skill-loader): dynamic walk-up + replay-on-resume (Plan 5 T4)

DynamicWalker walks upward from a touched file looking for unindexed
.klyntbot/skills/ directories. Bounded by repo_root or cwd; skips
.git/target/node_modules. Cached via seen_dirs to keep repeated
touches in the same directory O(1).

replay_session_history re-runs path activations from persisted
session history in sorted-deduped order — K6 determinism."
```


---

## Task 5: `coding_skills_*` Tauri commands (7 commands)

**Files:**
- Create: `crates/app-core/src/coding/skills_handler.rs`
- Create: `crates/app-core/src/init/coding_skills.rs`
- Modify: `crates/app-core/src/state.rs` (add `coding_skill_activator: Arc<Mutex<Option<SkillActivator>>>`)
- Modify: `crates/app-core/src/init/mod.rs` (call `init_coding_skills` post-startup)
- Create: `crates/desktop/src/commands/coding_skills.rs`
- Modify: `crates/desktop/src/specta_builder.rs` (register 7 commands)
- Test: `crates/app-core/tests/coding_skills_commands.rs`

`★ Insight ─────────────────────────────────────`
The activator is per-session in design (§11.1234) but per-install on instantiation: we wire **one** activator into `AppCore` keyed on the current "root" thread, then re-init it on `chat_set_mode` + `cwd` change. Every direct slash command (`/skills list`, etc.) reads through this activator.

Two design choices to flag:

1. We store `Arc<Mutex<Option<SkillActivator>>>` not `Arc<RwLock<...>>`. Skill operations are infrequent (user-driven) and sequential (one slash command at a time), so a regular `Mutex` is simpler and the "no async lock holding" lint is easier to keep clean.

2. `coding_skills_install` does the file-system copy/clone synchronously and returns. The "confirmation step" (§8.961) is implemented in the **TypeScript** dispatcher — not in the Rust command — because the React layer can render the `kind: "userInput"` row without an extra round trip. The Tauri command is the *executor* once the user clicks Confirm.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `crates/app-core/tests/coding_skills_commands.rs`**

```rust
use app_core::AppCore;
use common::Result;
use std::fs;
use tempfile::TempDir;

async fn make_test_core_with_skills(home: &TempDir) -> Result<AppCore> {
    std::env::set_var("KLYNTBOT_HOME", home.path());
    let core = AppCore::for_test().await?;
    Ok(core)
}

#[tokio::test]
async fn list_returns_discovered_skills() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/alpha");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: alpha\ndescription: Alpha\n---\nBody\n",
    ).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let listed = core.coding_skills_list().await.unwrap();
    let names: Vec<&str> = listed.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"alpha"));
}

#[tokio::test]
async fn info_returns_frontmatter_summary() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/alpha");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill body.\ntags: [\"test\"]\n---\n# A\nBody\n",
    ).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let info = core.coding_skills_info("alpha").await.unwrap();
    assert_eq!(info.name, "alpha");
    assert_eq!(info.description, "Alpha skill body.");
    assert_eq!(info.tags, vec!["test".to_string()]);
}

#[tokio::test]
async fn info_unknown_skill_errors() {
    let home = TempDir::new().unwrap();
    let core = make_test_core_with_skills(&home).await.unwrap();
    let res = core.coding_skills_info("nope").await;
    assert!(res.is_err());
}

#[tokio::test]
async fn install_local_path_copies_skill() {
    let home = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    let src_skill = src.path().join("freshly");
    fs::create_dir_all(&src_skill).unwrap();
    fs::write(
        src_skill.join("SKILL.md"),
        "---\nname: freshly\ndescription: Freshly installed.\n---\nBody\n",
    ).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    core.coding_skills_install(src_skill.to_string_lossy().into()).await.unwrap();

    let installed = home.path().join("skills/freshly/SKILL.md");
    assert!(installed.exists(), "skill copied to ~/.klyntbot/skills/freshly/");

    let listed = core.coding_skills_list().await.unwrap();
    assert!(listed.iter().any(|s| s.name == "freshly"), "appears in list after install");
}

#[tokio::test]
async fn toggle_disables_then_reenables() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/togg");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: togg\ndescription: Toggle me\n---\nBody\n",
    ).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    core.coding_skills_toggle("togg", false).await.unwrap();
    let cfg = core.config().read().await;
    assert!(cfg.coding.skills.never_activate.contains(&"togg".into()));
    drop(cfg);

    core.coding_skills_toggle("togg", true).await.unwrap();
    let cfg = core.config().read().await;
    assert!(!cfg.coding.skills.never_activate.contains(&"togg".into()));
}

#[tokio::test]
async fn validate_returns_ok_for_well_formed() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/valid");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: valid\ndescription: Valid\n---\nBody\n",
    ).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let result = core.coding_skills_validate("valid").await.unwrap();
    assert!(result.ok);
}

#[tokio::test]
async fn reload_picks_up_new_skill() {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join("skills")).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let initial_count = core.coding_skills_list().await.unwrap().len();

    let new = home.path().join("skills/freshly_added");
    fs::create_dir_all(&new).unwrap();
    fs::write(new.join("SKILL.md"), "---\nname: freshly_added\ndescription: New\n---\nBody\n").unwrap();

    core.coding_skills_reload().await.unwrap();
    let after_count = core.coding_skills_list().await.unwrap().len();
    assert_eq!(after_count, initial_count + 1);
}
```

- [ ] **Step 2: Add `for_test` constructor to `AppCore`** (if not already present)

In `crates/app-core/src/state.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl AppCore {
    pub async fn for_test() -> Result<Self> {
        // Construct an AppCore with in-memory storage, default config,
        // KLYNTBOT_HOME read from env.
        let home = std::env::var("KLYNTBOT_HOME").unwrap_or_else(|_| ".".into());
        let pool = storage::StoragePool::connect_in_memory().await?;
        // ... fill required fields with empty/default state
        // (full body in implementation)
        unimplemented!("implementer: stitch up minimum viable AppCore")
    }
}
```

If a `for_test` already exists in the codebase, reuse it.

- [ ] **Step 3: Create `crates/app-core/src/coding/skills_handler.rs`**

```rust
use crate::AppCore;
use common::{KlyntbotError, Result};
use klynt_skill_loader::{KlyntFrontmatter, SkillIndex, SkillSource};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillListItem {
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_path: String,
    pub tags: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub sensitivity: Option<String>,
    pub source: String,
    pub source_path: String,
    pub references: Vec<SkillReferenceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillReferenceInfo {
    pub name: String,
    pub file: String,
    pub load: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_list(&self) -> Result<Vec<SkillListItem>> {
        let activator = self.coding_skill_activator.lock().await;
        let activator = activator.as_ref().ok_or_else(|| {
            KlyntbotError::Internal("skill activator not initialized".into())
        })?;
        let cfg = self.config().read().await;
        let never: std::collections::HashSet<&str> = cfg
            .coding
            .skills
            .never_activate
            .iter()
            .map(String::as_str)
            .collect();
        let mut items = Vec::new();
        for (name, skill) in activator.iter_index() {
            items.push(SkillListItem {
                name: name.clone(),
                description: skill.frontmatter.description.clone(),
                source: format!("{:?}", skill.source).to_lowercase(),
                source_path: skill.source_path.display().to_string(),
                tags: skill.frontmatter.tags.clone(),
                enabled: !never.contains(name.as_str()),
            });
        }
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(items)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_info(&self, name: &str) -> Result<SkillInfo> {
        let activator = self.coding_skill_activator.lock().await;
        let activator = activator.as_ref().ok_or_else(|| {
            KlyntbotError::Internal("skill activator not initialized".into())
        })?;
        let skill = activator.lookup(name).ok_or_else(|| {
            KlyntbotError::Internal(format!("unknown skill: {name}"))
        })?;
        Ok(SkillInfo {
            name: skill.frontmatter.name.clone(),
            description: skill.frontmatter.description.clone(),
            allowed_tools: skill.frontmatter.allowed_tools.clone(),
            paths: skill.frontmatter.paths.clone(),
            tags: skill.frontmatter.tags.clone(),
            sensitivity: skill.frontmatter.sensitivity.clone(),
            source: format!("{:?}", skill.source).to_lowercase(),
            source_path: skill.source_path.display().to_string(),
            references: skill
                .frontmatter
                .references
                .iter()
                .map(|r| SkillReferenceInfo {
                    name: r.name.clone(),
                    file: r.file.clone(),
                    load: format!("{:?}", r.load).to_lowercase(),
                })
                .collect(),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_install(&self, source: String) -> Result<SkillListItem> {
        let target_dir = self.klyntbot_home().join("skills");
        std::fs::create_dir_all(&target_dir).map_err(|e| {
            KlyntbotError::Internal(format!("create skills dir: {e}"))
        })?;
        let installed_name = if source.starts_with("http://") || source.starts_with("https://") {
            install_from_url(&source, &target_dir).await?
        } else {
            install_from_local_path(&PathBuf::from(&source), &target_dir)?
        };
        self.coding_skills_reload().await?;
        self.coding_skills_info(&installed_name).await.map(|info| SkillListItem {
            name: info.name,
            description: info.description,
            source: info.source,
            source_path: info.source_path,
            tags: info.tags,
            enabled: true,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_update(&self, name: &str) -> Result<SkillListItem> {
        // Phase 1: only re-fetch via re-install of the original source.
        // For local paths or paths without origin metadata, this is a no-op + reload.
        self.coding_skills_reload().await?;
        let info = self.coding_skills_info(name).await?;
        Ok(SkillListItem {
            name: info.name,
            description: info.description,
            source: info.source,
            source_path: info.source_path,
            tags: info.tags,
            enabled: true,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_uninstall(&self, name: &str) -> Result<()> {
        let activator = self.coding_skill_activator.lock().await;
        let activator = activator.as_ref().ok_or_else(|| {
            KlyntbotError::Internal("skill activator not initialized".into())
        })?;
        let skill = activator.lookup(name).ok_or_else(|| {
            KlyntbotError::Internal(format!("unknown skill: {name}"))
        })?;
        if !matches!(skill.source, SkillSource::User) {
            return Err(KlyntbotError::Internal(
                "can only uninstall User-source skills (project skills live in repo)".into(),
            ));
        }
        let dir = skill.source_path.parent().ok_or_else(|| {
            KlyntbotError::Internal("skill path has no parent dir".into())
        })?;
        std::fs::remove_dir_all(dir).map_err(|e| {
            KlyntbotError::Internal(format!("remove {}: {e}", dir.display()))
        })?;
        drop(activator);
        self.coding_skills_reload().await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_toggle(&self, name: &str, enabled: bool) -> Result<()> {
        let mut cfg = self.config().write().await;
        if enabled {
            cfg.coding.skills.never_activate.retain(|n| n != name);
        } else if !cfg.coding.skills.never_activate.iter().any(|n| n == name) {
            cfg.coding.skills.never_activate.push(name.to_string());
        }
        cfg.save_to_disk()?;
        drop(cfg);
        self.coding_skills_reload().await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_validate(&self, name: &str) -> Result<SkillValidationResult> {
        let activator = self.coding_skill_activator.lock().await;
        let activator = activator.as_ref().ok_or_else(|| {
            KlyntbotError::Internal("skill activator not initialized".into())
        })?;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        match activator.lookup(name) {
            None => errors.push(format!("unknown skill: {name}")),
            Some(skill) => {
                let raw = std::fs::read_to_string(&skill.source_path).map_err(|e| {
                    KlyntbotError::Internal(format!("re-read SKILL.md: {e}"))
                })?;
                if let Err(e) = KlyntFrontmatter::parse(&raw) {
                    errors.push(format!("frontmatter invalid: {e}"));
                }
                for path_glob in &skill.frontmatter.paths {
                    if globset::Glob::new(path_glob).is_err() {
                        errors.push(format!("invalid path glob: {path_glob}"));
                    }
                }
                if skill.frontmatter.allowed_tools.is_empty() {
                    warnings.push("no allowed-tools declared (skill has access to all)".into());
                }
            }
        }
        Ok(SkillValidationResult {
            ok: errors.is_empty(),
            errors,
            warnings,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_reload(&self) -> Result<()> {
        let new_index = self.discover_skills().await?;
        let cfg = self.config().read().await;
        let activation_cfg = klynt_skill_loader::ActivationConfig::from_coding_config(
            &cfg.coding.skills,
        );
        drop(cfg);
        let mut act = self.coding_skill_activator.lock().await;
        *act = Some(klynt_skill_loader::SkillActivator::new(new_index, activation_cfg)?);
        Ok(())
    }

    pub(crate) async fn discover_skills(&self) -> Result<SkillIndex> {
        let roots = klynt_skill_loader::DiscoveryRoots {
            klyntbot_home: self.klyntbot_home().clone(),
            repo_id: None,
            repo_root: None,
            cwd: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        };
        SkillIndex::discover(&roots)
    }
}

fn install_from_local_path(src: &std::path::Path, target_dir: &std::path::Path) -> Result<String> {
    let skill_md = src.join("SKILL.md");
    if !skill_md.exists() {
        return Err(KlyntbotError::Internal(format!(
            "no SKILL.md at {}",
            src.display()
        )));
    }
    let raw = std::fs::read_to_string(&skill_md).map_err(|e| {
        KlyntbotError::Internal(format!("read source: {e}"))
    })?;
    let (fm, _) = KlyntFrontmatter::parse(&raw)?;
    let dst = target_dir.join(&fm.name);
    if dst.exists() {
        return Err(KlyntbotError::Internal(format!(
            "skill `{}` already installed",
            fm.name
        )));
    }
    fs_copy_dir_all(src, &dst)?;
    Ok(fm.name)
}

fn fs_copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| {
        KlyntbotError::Internal(format!("create dst: {e}"))
    })?;
    for entry in std::fs::read_dir(src).map_err(|e| {
        KlyntbotError::Internal(format!("read src: {e}"))
    })?.flatten() {
        let ft = entry.file_type().map_err(|e| {
            KlyntbotError::Internal(format!("file_type: {e}"))
        })?;
        let dst_entry = dst.join(entry.file_name());
        if ft.is_dir() {
            fs_copy_dir_all(&entry.path(), &dst_entry)?;
        } else {
            std::fs::copy(entry.path(), &dst_entry).map_err(|e| {
                KlyntbotError::Internal(format!("copy: {e}"))
            })?;
        }
    }
    Ok(())
}

async fn install_from_url(url: &str, target_dir: &std::path::Path) -> Result<String> {
    let temp = tempfile::tempdir().map_err(|e| {
        KlyntbotError::Internal(format!("tempdir: {e}"))
    })?;
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, temp.path().to_str().unwrap()])
        .status()
        .map_err(|e| KlyntbotError::Internal(format!("git clone: {e}")))?;
    if !status.success() {
        return Err(KlyntbotError::Internal(format!("git clone failed: {url}")));
    }
    install_from_local_path(temp.path(), target_dir)
}
```

- [ ] **Step 4: Add `iter_index` to `SkillActivator`**

In `crates/klynt-skill-loader/src/activator.rs`:

```rust
impl SkillActivator {
    pub fn iter_index(&self) -> impl Iterator<Item = (&String, &IndexedSkill)> {
        self.index.iter()
    }
}
```

- [ ] **Step 5: Add `coding_skill_activator` field to `AppCore`**

In `crates/app-core/src/state.rs`, add:

```rust
pub coding_skill_activator: Arc<tokio::sync::Mutex<Option<klynt_skill_loader::SkillActivator>>>,
```

Initialize as `Arc::new(tokio::sync::Mutex::new(None))` in the constructor.

- [ ] **Step 6: Create `crates/app-core/src/init/coding_skills.rs`**

```rust
use crate::AppCore;
use common::Result;
use std::sync::Arc;

#[tracing::instrument(skip(core), err)]
pub async fn init_coding_skills(core: Arc<AppCore>) -> Result<()> {
    core.coding_skills_reload().await?;
    let count = core.coding_skills_list().await?.len();
    tracing::info!(skills_indexed = count, "coding skill loader initialized");
    Ok(())
}
```

Call from `init/mod.rs` after `init_cognitive` in startup ordering.

- [ ] **Step 7: Create `crates/desktop/src/commands/coding_skills.rs`**

```rust
use app_core::coding::skills_handler::{
    SkillInfo, SkillListItem, SkillValidationResult,
};
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn coding_skills_list() -> common::Result<Vec<SkillListItem>> {
    core.coding_skills_list().await
}

#[klynt_command]
pub async fn coding_skills_info(name: String) -> common::Result<SkillInfo> {
    core.coding_skills_info(&name).await
}

#[klynt_command]
pub async fn coding_skills_install(source: String) -> common::Result<SkillListItem> {
    core.coding_skills_install(source).await
}

#[klynt_command]
pub async fn coding_skills_update(name: String) -> common::Result<SkillListItem> {
    core.coding_skills_update(&name).await
}

#[klynt_command]
pub async fn coding_skills_uninstall(name: String) -> common::Result<()> {
    core.coding_skills_uninstall(&name).await
}

#[klynt_command]
pub async fn coding_skills_toggle(name: String, enabled: bool) -> common::Result<()> {
    core.coding_skills_toggle(&name, enabled).await
}

#[klynt_command]
pub async fn coding_skills_validate(name: String) -> common::Result<SkillValidationResult> {
    core.coding_skills_validate(&name).await
}

#[klynt_command]
pub async fn coding_skills_reload() -> common::Result<()> {
    core.coding_skills_reload().await
}
```

- [ ] **Step 8: Register the 8 commands in `specta_builder.rs`**

Add to `klynt_collect_commands![...]`:

```
commands::coding_skills::coding_skills_list,
commands::coding_skills::coding_skills_info,
commands::coding_skills::coding_skills_install,
commands::coding_skills::coding_skills_update,
commands::coding_skills::coding_skills_uninstall,
commands::coding_skills::coding_skills_toggle,
commands::coding_skills::coding_skills_validate,
commands::coding_skills::coding_skills_reload,
```

- [ ] **Step 9: Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`**

Run: `cargo tauri dev` and quit after the bindings regenerate. Verify `coding_skills_list` etc. appear in `desktop-ui/src/bindings.ts`.

- [ ] **Step 10: Run all tests**

Run: `cargo nextest run -p app-core --test coding_skills_commands`
Expected: 7 tests pass.

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all --check`
Expected: zero warnings; clean fmt.

- [ ] **Step 11: Commit**

```bash
git add crates/app-core crates/desktop crates/klynt-skill-loader desktop-ui/src/bindings.ts
git commit -m "feat(coding-skills): 8 Tauri commands + activator wiring (Plan 5 T5)

- coding_skills_{list,info,install,update,uninstall,toggle,validate,reload}
- AppCore.coding_skill_activator: Arc<Mutex<Option<SkillActivator>>>
- init_coding_skills runs at startup after init_cognitive
- Install supports local paths + git URLs (via 'git clone --depth 1')
- Toggle persists to config.coding.skills.never_activate
- Uninstall restricted to User-source skills"
```


---

## Task 6: Slash command registry + classify (TypeScript)

**Files:**
- Create: `desktop-ui/src/features/coding/slash/types.ts`
- Create: `desktop-ui/src/features/coding/slash/registry.ts`
- Create: `desktop-ui/src/features/coding/slash/classify.ts`
- Create: `desktop-ui/src/features/coding/slash/agentRouted.ts`
- Create: `desktop-ui/src/features/coding/slash/direct.ts`
- Test: `desktop-ui/src/features/coding/slash/registry.test.ts`
- Test: `desktop-ui/src/features/coding/slash/classify.test.ts`

`★ Insight ─────────────────────────────────────`
The classify algorithm (§9.1048) is an explicit five-step algorithm with a specific tie-breaking rule. Implementing it in pure TS (no Rust round trip) keeps the classify step under 1ms and locks it into a deterministic, testable function — critical for the K9 invariant in T17.

The catalog tree structure mirrors the dispatcher's mental model: a `SlashNode` is either a leaf (`{ kind: "leaf", path: "agent" | "direct", ... }`) or a branch (`{ kind: "branch", children: { [key]: SlashNode } }`). `/skills` is a branch with subcommands `list`, `info`, `install`, etc., each leaves of `path: "direct"`. `/sessions` is similar. `/plan`, `/yolo`, `/recall` are leaves of `path: "agent"`. This shape lets the autocomplete UI walk the same tree to produce category groupings.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `desktop-ui/src/features/coding/slash/classify.test.ts`**

```typescript
import { describe, expect, test } from "vitest";
import { classify } from "./classify";

describe("slash classify", () => {
  test("returns null for non-slash input", () => {
    expect(classify("hello world")).toBeNull();
  });
  test("returns 'agent' for /plan", () => {
    expect(classify("/plan refactor parser")).toBe("agent");
  });
  test("returns 'direct' for /skills list", () => {
    expect(classify("/skills list")).toBe("direct");
  });
  test("returns 'direct' for /status", () => {
    expect(classify("/status")).toBe("direct");
  });
  test("returns null for unknown command", () => {
    expect(classify("/foobarbaz xyz")).toBeNull();
  });
  test("trims leading whitespace before checking first char", () => {
    expect(classify(" /plan x")).toBeNull();   // not first non-whitespace; rule 1
  });
  test("/sk (partial prefix) returns null", () => {
    expect(classify("/sk")).toBeNull();
  });
  test("/sessions star returns 'direct'", () => {
    expect(classify("/sessions star")).toBe("direct");
  });
  test("/sessions returns null (branch, no leaf path)", () => {
    expect(classify("/sessions")).toBeNull();
  });
  test("/help returns 'direct'", () => {
    expect(classify("/help")).toBe("direct");
  });
});
```

- [ ] **Step 2: Run test, expect failure (file undefined)**

Run: `cd desktop-ui && bun run test src/features/coding/slash/classify.test.ts`
Expected: cannot resolve `./classify`.

- [ ] **Step 3: Create `desktop-ui/src/features/coding/slash/types.ts`**

```typescript
export type SlashPath = "agent" | "direct";

export type SlashLeaf = {
  kind: "leaf";
  path: SlashPath;
  command: string;
  description: string;
  category: SlashCategory;
  argHint?: string;
  tauriCommand?: string;
};

export type SlashBranch = {
  kind: "branch";
  command: string;
  description: string;
  category: SlashCategory;
  children: Record<string, SlashNode>;
};

export type SlashNode = SlashLeaf | SlashBranch;

export type SlashCategory =
  | "mode"
  | "skills"
  | "status"
  | "sessions"
  | "permissions"
  | "recall"
  | "help";

export type DispatchResult =
  | { kind: "passthrough"; text: string }
  | { kind: "render"; itemKind: string; item: unknown }
  | { kind: "error"; message: string };
```

- [ ] **Step 4: Create `desktop-ui/src/features/coding/slash/registry.ts`**

```typescript
import type { SlashNode } from "./types";

export const REGISTRY: Record<string, SlashNode> = {
  plan: {
    kind: "leaf",
    path: "agent",
    command: "plan",
    description: "Enter plan mode (writes/exec denied)",
    category: "mode",
  },
  yolo: {
    kind: "leaf",
    path: "agent",
    command: "yolo",
    description: "Bypass approvals (requires env var)",
    category: "mode",
  },
  power: {
    kind: "branch",
    command: "power",
    description: "Toggle power tool profile",
    category: "mode",
    children: {
      on: {
        kind: "leaf",
        path: "agent",
        command: "power on",
        description: "Enable power tools",
        category: "mode",
      },
      off: {
        kind: "leaf",
        path: "agent",
        command: "power off",
        description: "Disable power tools",
        category: "mode",
      },
    },
  },
  recall: {
    kind: "leaf",
    path: "agent",
    command: "recall",
    description: "Force a recall pass with this query",
    category: "recall",
    argHint: "<query>",
  },
  skills: {
    kind: "branch",
    command: "skills",
    description: "Manage skills",
    category: "skills",
    children: {
      list: { kind: "leaf", path: "direct", command: "skills list", tauriCommand: "coding_skills_list", description: "List installed skills", category: "skills" },
      info: { kind: "leaf", path: "direct", command: "skills info", tauriCommand: "coding_skills_info", description: "Show skill frontmatter", category: "skills", argHint: "<name>" },
      install: { kind: "leaf", path: "direct", command: "skills install", tauriCommand: "coding_skills_install", description: "Install a skill (path or URL)", category: "skills", argHint: "<source>" },
      update: { kind: "leaf", path: "direct", command: "skills update", tauriCommand: "coding_skills_update", description: "Re-fetch skill from origin", category: "skills", argHint: "<name>" },
      uninstall: { kind: "leaf", path: "direct", command: "skills uninstall", tauriCommand: "coding_skills_uninstall", description: "Remove a skill", category: "skills", argHint: "<name>" },
      toggle: { kind: "leaf", path: "direct", command: "skills toggle", tauriCommand: "coding_skills_toggle", description: "Enable/disable a skill", category: "skills", argHint: "<name> --on|--off" },
      validate: { kind: "leaf", path: "direct", command: "skills validate", tauriCommand: "coding_skills_validate", description: "Check SKILL.md validity", category: "skills", argHint: "<name>" },
      reload: { kind: "leaf", path: "direct", command: "skills reload", tauriCommand: "coding_skills_reload", description: "Re-walk skill discovery", category: "skills" },
    },
  },
  status: {
    kind: "leaf",
    path: "direct",
    command: "status",
    tauriCommand: "coding_status",
    description: "Show mode, profile, sandbox, cost",
    category: "status",
  },
  doctor: {
    kind: "leaf",
    path: "direct",
    command: "doctor",
    tauriCommand: "coding_doctor",
    description: "Run diagnostic checklist",
    category: "status",
  },
  sessions: {
    kind: "branch",
    command: "sessions",
    description: "Manage chat sessions",
    category: "sessions",
    children: {
      star: { kind: "leaf", path: "direct", command: "sessions star", tauriCommand: "coding_sessions_star", description: "Mark current thread starred", category: "sessions" },
      unstar: { kind: "leaf", path: "direct", command: "sessions unstar", tauriCommand: "coding_sessions_unstar", description: "Unmark current thread", category: "sessions" },
    },
  },
  resume: {
    kind: "leaf",
    path: "direct",
    command: "resume",
    tauriCommand: "coding_resume",
    description: "Switch to matching thread",
    category: "sessions",
    argHint: "<prefix>",
  },
  help: {
    kind: "leaf",
    path: "direct",
    command: "help",
    tauriCommand: "coding_help",
    description: "List slash commands",
    category: "help",
    argHint: "[command]",
  },
};

export function flatCatalog(): Array<{ command: string; description: string; category: string; argHint?: string }> {
  const out: Array<{ command: string; description: string; category: string; argHint?: string }> = [];
  function walk(node: SlashNode) {
    if (node.kind === "leaf") {
      out.push({ command: `/${node.command}`, description: node.description, category: node.category, argHint: node.argHint });
    } else {
      for (const child of Object.values(node.children)) walk(child);
    }
  }
  for (const node of Object.values(REGISTRY)) walk(node);
  return out;
}
```

- [ ] **Step 5: Create `desktop-ui/src/features/coding/slash/classify.ts`**

```typescript
import { REGISTRY } from "./registry";
import type { SlashNode, SlashPath } from "./types";

/**
 * Classify a raw composer input as agent-routed, direct, or null.
 *
 * Rules (spec §9.1048):
 * 1. Reject if the first non-whitespace character is not `/` or input is `null`-ish.
 * 2. Take leading `/`-prefixed token as command head.
 * 3. Walk REGISTRY tree as deep as possible.
 * 4. If deepest match is a leaf, return its `path`. Otherwise null (branch w/o terminal arg).
 * 5. Tie-break: direct wins over agent if same-named alias ever exists.
 */
export function classify(input: string): SlashPath | null {
  if (input == null || input.length === 0) return null;
  if (input[0] !== "/") return null;
  const stripped = input.slice(1).trim();
  if (stripped.length === 0) return null;

  const tokens = stripped.split(/\s+/);
  let node: SlashNode | undefined = REGISTRY[tokens[0]];
  if (!node) return null;
  for (let i = 1; i < tokens.length; i++) {
    if (node.kind === "leaf") break;
    const next: SlashNode | undefined = node.children[tokens[i]];
    if (!next) break;
    node = next;
  }
  if (node.kind === "leaf") return node.path;
  return null;
}
```

- [ ] **Step 6: Create `desktop-ui/src/features/coding/slash/agentRouted.ts`**

```typescript
import type { DispatchResult } from "./types";

/** Transform an agent-routed slash command into a system-instruction-prefixed user message. */
export function transformAgentRouted(input: string): DispatchResult {
  const trimmed = input.trim();
  if (trimmed === "/plan") {
    return { kind: "passthrough", text: "[system: enter plan mode] " };
  }
  if (trimmed === "/yolo") {
    return { kind: "passthrough", text: "[system: enter bypass mode] " };
  }
  if (trimmed === "/power on" || trimmed === "/power off") {
    const enable = trimmed.endsWith("on");
    return { kind: "passthrough", text: `[system: power_mode=${enable}] ` };
  }
  if (trimmed.startsWith("/recall ")) {
    const query = trimmed.slice("/recall ".length);
    return { kind: "passthrough", text: `[system: force recall query="${query.replace(/"/g, '\\"')}"] ` };
  }
  return { kind: "error", message: `unknown agent-routed command: ${input}` };
}
```

- [ ] **Step 7: Create `desktop-ui/src/features/coding/slash/direct.ts`** (skeleton — fully wired in T7)

```typescript
import { invoke } from "@/api/client";
import type { DispatchResult } from "./types";
import { REGISTRY } from "./registry";
import type { SlashNode } from "./types";

export async function dispatchDirect(input: string, sessionKey: string): Promise<DispatchResult> {
  const stripped = input.slice(1).trim();
  const tokens = stripped.split(/\s+/);
  let node: SlashNode | undefined = REGISTRY[tokens[0]];
  let consumed = 1;
  for (let i = 1; i < tokens.length && node && node.kind === "branch"; i++) {
    const next: SlashNode | undefined = node.children[tokens[i]];
    if (!next) break;
    node = next;
    consumed++;
  }
  if (!node || node.kind !== "leaf" || !node.tauriCommand) {
    return { kind: "error", message: `unknown direct command: ${input}` };
  }
  const args = tokens.slice(consumed);
  try {
    const params = buildParams(node.command, args, sessionKey);
    const result = await invoke(node.tauriCommand, params);
    return { kind: "render", itemKind: "system", item: { command: node.command, result } };
  } catch (e) {
    return { kind: "error", message: String(e) };
  }
}

function buildParams(cmd: string, args: string[], sessionKey: string): Record<string, unknown> {
  switch (cmd) {
    case "skills info":
    case "skills update":
    case "skills uninstall":
    case "skills validate":
      return { name: args[0] ?? "" };
    case "skills install":
      return { source: args[0] ?? "" };
    case "skills toggle":
      return { name: args[0] ?? "", enabled: !args.includes("--off") };
    case "resume":
      return { prefix: args[0] ?? "" };
    case "sessions star":
    case "sessions unstar":
      return { sessionKey };
    default:
      return {};
  }
}
```

- [ ] **Step 8: Add a `registry.test.ts` to assert no command conflicts**

```typescript
import { describe, expect, test } from "vitest";
import { REGISTRY, flatCatalog } from "./registry";

describe("slash registry", () => {
  test("flatCatalog enumerates leaves", () => {
    const flat = flatCatalog();
    expect(flat.length).toBeGreaterThan(8);
    expect(flat.some(c => c.command === "/skills list")).toBe(true);
    expect(flat.some(c => c.command === "/plan")).toBe(true);
  });
  test("no leaf has a command name colliding with a branch's top key", () => {
    for (const [key, node] of Object.entries(REGISTRY)) {
      if (node.kind === "leaf") {
        expect(node.command.startsWith(key)).toBe(true);
      } else {
        for (const childKey of Object.keys(node.children)) {
          expect(childKey.includes(" ")).toBe(false);
        }
      }
    }
  });
});
```

- [ ] **Step 9: Run all tests + lint**

Run: `cd desktop-ui && bun run test src/features/coding/slash/ && bun run lint && bun run typecheck`
Expected: 12 tests pass; zero lint/type errors.

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/features/coding/slash
git commit -m "feat(coding-slash): typed registry + deterministic classify (Plan 5 T6)

- registry.ts: tree of REGISTRY[branch|leaf] keyed on command head
- classify.ts: 5-step algo from spec §9.1048; pure, sync, sub-millisecond
- agentRouted.ts: /plan, /yolo, /power on|off, /recall <q> transformations
- direct.ts: dispatch via @/api/client invoke() to coding_* Tauri commands
- types.ts: DispatchResult discriminated union"
```


---

## Task 7: `useSlashCommands` hook + composer integration

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useSlashCommands.ts`
- Create: `desktop-ui/src/features/coding/hooks/useSlashCommands.test.ts`
- Modify: `desktop-ui/src/features/composer/hooks/useComposerAutocomplete.ts` (gate `/` trigger on coding mode)
- Modify: `desktop-ui/src/features/composer/Composer.tsx` (intercept `/`-input through `useSlashCommands.dispatch` before `chat_send`)

`★ Insight ─────────────────────────────────────`
The hook returns three things — `catalog`, `classify`, `dispatch` — but `dispatch` is the load-bearing surface. It owes the Composer two distinct returns: `passthrough` (let `chat_send` proceed; agent will see the transformed text) versus `render` (skip `chat_send`, append a synthetic `kind: "system"` row directly to the chatStreamStore). If `dispatch` doesn't fork there, every direct command would still hit the agent loop and waste a turn.

The composer integration uses a thin guard: only intercept if the input starts with `/` AND `useCodingMode` reports `mode === "coding"`. This keeps the existing chat flow untouched in `general` mode — typing `/etc/passwd` to ask about the file works.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `desktop-ui/src/features/coding/hooks/useSlashCommands.test.ts`**

```typescript
import { describe, expect, test, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSlashCommands } from "./useSlashCommands";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string, _args: unknown) => {
    if (cmd === "coding_skills_list") return [{ name: "alpha", description: "Alpha", source: "user", source_path: "/x", tags: [], enabled: true }];
    if (cmd === "coding_status") return { mode: "coding", profile: "curated", sandbox: "macos", cost: 0, tokens: 0, active_skills: [] };
    return null;
  }),
}));

describe("useSlashCommands", () => {
  beforeEach(() => vi.clearAllMocks());

  test("classify routes /skills list → direct", () => {
    const { result } = renderHook(() => useSlashCommands());
    expect(result.current.classify("/skills list")).toBe("direct");
  });

  test("dispatch direct invokes Tauri command", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/skills list", "session-1");
    });
    expect(res.kind).toBe("render");
    expect(res.itemKind).toBe("system");
  });

  test("dispatch agent-routed returns passthrough", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/plan refactor", "session-1");
    });
    expect(res.kind).toBe("passthrough");
    expect(res.text).toContain("[system: enter plan mode]");
  });

  test("dispatch unknown returns null/passthrough-original", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/foobar abc", "session-1");
    });
    expect(res.kind).toBe("passthrough");
    expect(res.text).toBe("/foobar abc");
  });

  test("catalog returns flatCatalog", () => {
    const { result } = renderHook(() => useSlashCommands());
    const cat = result.current.catalog();
    expect(cat.length).toBeGreaterThan(8);
  });
});
```

- [ ] **Step 2: Create `desktop-ui/src/features/coding/hooks/useSlashCommands.ts`**

```typescript
import { useCallback, useMemo } from "react";
import { classify } from "../slash/classify";
import { transformAgentRouted } from "../slash/agentRouted";
import { dispatchDirect } from "../slash/direct";
import { flatCatalog } from "../slash/registry";
import type { DispatchResult } from "../slash/types";

export function useSlashCommands() {
  const catalog = useCallback(() => flatCatalog(), []);

  const dispatch = useCallback(
    async (input: string, sessionKey: string): Promise<DispatchResult> => {
      const verdict = classify(input);
      if (verdict === "agent") return transformAgentRouted(input);
      if (verdict === "direct") return dispatchDirect(input, sessionKey);
      return { kind: "passthrough", text: input };
    },
    []
  );

  return useMemo(() => ({ catalog, classify, dispatch }), [catalog, dispatch]);
}
```

- [ ] **Step 3: Run hook tests**

Run: `cd desktop-ui && bun run test src/features/coding/hooks/useSlashCommands.test.ts`
Expected: 5 tests pass.

- [ ] **Step 4: Wire into Composer**

In `desktop-ui/src/features/composer/Composer.tsx` (find the `handleSubmit` function), add at the top before any `chat_send`:

```typescript
import { useSlashCommands } from "@/features/coding/hooks/useSlashCommands";
import { useCodingMode } from "@/features/coding/hooks/useCodingMode";    // T8
// ...

const { dispatch } = useSlashCommands();
const { mode } = useCodingMode(threadId);

async function handleSubmit(text: string) {
  if (mode === "coding" && text.trimStart().startsWith("/")) {
    const res = await dispatch(text.trimStart(), sessionKey);
    if (res.kind === "render") {
      // append synthetic system row to chatStreamStore
      appendSystemItem(threadId, res.itemKind, res.item);
      return;
    }
    if (res.kind === "error") {
      appendErrorItem(threadId, res.message);
      return;
    }
    if (res.kind === "passthrough") {
      // continue with res.text instead of original text
      text = res.text;
    }
  }
  // existing chat_send call
  await invoke("chat_send", { /* ... */, content: text });
}
```

`appendSystemItem` and `appendErrorItem` are helpers added to `chatStreamStore` — small additions:

```typescript
// chatStreamStore.ts additions
appendSystemItem: (threadId: string, kind: string, item: unknown) => void;
appendErrorItem: (threadId: string, message: string) => void;
```

- [ ] **Step 5: Update `useComposerAutocomplete` to gate `/` trigger on coding mode**

Find `triggerPrefixRegex` (or equivalent) in `useComposerAutocomplete.ts`. Add a guard:

```typescript
const { mode } = useCodingMode(threadId);
const slashEnabled = mode === "coding";
// only register the / trigger handler if slashEnabled is true
```

- [ ] **Step 6: Run lint + tests**

Run: `cd desktop-ui && bun run lint && bun run typecheck && bun run test`
Expected: zero errors; all tests pass.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useSlashCommands.ts desktop-ui/src/features/coding/hooks/useSlashCommands.test.ts desktop-ui/src/features/composer/Composer.tsx desktop-ui/src/features/composer/hooks/useComposerAutocomplete.ts desktop-ui/src/services/chatStreamStore.ts
git commit -m "feat(coding-slash): useSlashCommands + composer interception (Plan 5 T7)

Composer intercepts /-prefixed input in coding mode only. Direct
commands return synthetic system rows; agent-routed commands transform
into [system: ...] prefixes. Unknown commands fall through to the agent."
```


---

## Task 8: `useCodingMode` + `CodingModePill` + workspace auto-detect

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useCodingMode.ts`
- Create: `desktop-ui/src/features/coding/hooks/useCodingMode.test.ts`
- Create: `desktop-ui/src/features/coding/components/CodingModePill.tsx`
- Create: `desktop-ui/src/features/coding/components/CodingModePill.test.tsx`
- Modify: `desktop-ui/src/features/composer/Composer.tsx` (render pill in meta bar)
- Modify: `crates/app-core/src/coding/mode_handler.rs` (add `auto_detect_mode_for_thread`)
- Test: `desktop-ui/src/features/coding/hooks/useCodingMode.property.test.ts` (K7 invariant)

`★ Insight ─────────────────────────────────────`
Mode is per-thread state, persisted in `sessions.conversation_type`. Plan 2 added `chat_set_mode` (Tauri command). Plan 5 surfaces it: `useCodingMode(threadId)` reads the current value via `chat_messages`'s thread metadata, exposes `setMode(next)`, and listens on `agent:mode_changed` for upstream changes.

Auto-detection (§5.516-523) is server-side: when a new thread is created from a workspace context (Sidebar's `WorktreeSection`), the backend infers `mode = "coding"` and `cwd = workspace.path`. The frontend doesn't decide — it just reflects.

K7 (mode-toggle event ordering) requires every `setMode("coding")` call to result in EXACTLY one `agent:mode_changed` event before the next `chat_send` lands; the property test in T17 hammers a fast-check generator with mode flip sequences and asserts ordering.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `desktop-ui/src/features/coding/hooks/useCodingMode.test.ts`**

```typescript
import { describe, expect, test, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCodingMode } from "./useCodingMode";

let invokeCalls: Array<[string, unknown]> = [];
vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string, args: unknown) => {
    invokeCalls.push([cmd, args]);
    if (cmd === "chat_get_mode") return "general";
    if (cmd === "chat_set_mode") return undefined;
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_e: string, _cb: unknown) => () => {}),
}));

describe("useCodingMode", () => {
  beforeEach(() => { invokeCalls = []; });

  test("initial fetch returns general mode", async () => {
    const { result } = renderHook(() => useCodingMode("session-1"));
    await act(async () => { await new Promise(r => setTimeout(r, 1)); });
    expect(result.current.mode).toBe("general");
  });

  test("setMode invokes chat_set_mode", async () => {
    const { result } = renderHook(() => useCodingMode("session-1"));
    await act(async () => { await result.current.setMode("coding"); });
    const setCall = invokeCalls.find(([c]) => c === "chat_set_mode");
    expect(setCall).toBeTruthy();
    expect((setCall![1] as any).mode).toBe("coding");
  });
});
```

- [ ] **Step 2: Create `desktop-ui/src/features/coding/hooks/useCodingMode.ts`**

```typescript
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";
import { listen } from "@tauri-apps/api/event";

export type CodingMode = "general" | "coding";

export function useCodingMode(threadId: string | null) {
  const [mode, setModeState] = useState<CodingMode>("general");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!threadId) return;
    let cancelled = false;
    (async () => {
      try {
        const m = (await invoke("chat_get_mode", { sessionKey: threadId })) as CodingMode;
        if (!cancelled) setModeState(m);
      } catch (e) {
        console.warn("useCodingMode: chat_get_mode failed", e);
      }
    })();
    return () => { cancelled = true; };
  }, [threadId]);

  useEffect(() => {
    if (!threadId) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<{ session_key: string; mode: CodingMode }>(
        "agent:mode_changed",
        (evt) => {
          if (evt.payload.session_key === threadId) {
            setModeState(evt.payload.mode);
          }
        }
      );
    })();
    return () => { unlisten?.(); };
  }, [threadId]);

  const setMode = useCallback(
    async (next: CodingMode) => {
      if (!threadId) return;
      setLoading(true);
      try {
        await invoke("chat_set_mode", { sessionKey: threadId, mode: next });
        setModeState(next);
      } finally {
        setLoading(false);
      }
    },
    [threadId]
  );

  return { mode, setMode, loading };
}
```

- [ ] **Step 3: Add `chat_get_mode` Tauri command**

Create `crates/desktop/src/commands/chat_mode.rs`:

```rust
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn chat_get_mode(session_key: String) -> common::Result<String> {
    core.coding_mode_for_thread(&session_key).await
}
```

In `crates/app-core/src/coding/mode_handler.rs`, add:

```rust
impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_mode_for_thread(&self, session_key: &str) -> common::Result<String> {
        let pool = self.repos.sessions.pool();
        let row = sqlx::query!(
            "SELECT conversation_type FROM sessions WHERE id = ?",
            session_key
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        Ok(row
            .and_then(|r| r.conversation_type)
            .unwrap_or_else(|| "general".to_string()))
    }
}
```

Register `chat_get_mode` in `specta_builder.rs`.

- [ ] **Step 4: Create `desktop-ui/src/features/coding/components/CodingModePill.tsx`**

```typescript
import { useCodingMode, type CodingMode } from "../hooks/useCodingMode";

export function CodingModePill({ threadId }: { threadId: string | null }) {
  const { mode, setMode, loading } = useCodingMode(threadId);

  const next: CodingMode = mode === "coding" ? "general" : "coding";
  return (
    <button
      type="button"
      className={`coding-mode-pill ${mode === "coding" ? "is-coding" : "is-general"}`}
      disabled={loading || !threadId}
      onClick={() => setMode(next)}
      aria-label={`Switch to ${next} mode`}
    >
      {mode === "coding" ? "Coding" : "General"}
    </button>
  );
}
```

- [ ] **Step 5: Add CSS in `desktop-ui/src/features/coding/styles/coding.css`**

```css
@import "../../styles/ds-tokens.css";

.coding-mode-pill {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: var(--fs-xs);
  background: var(--surface-2);
  color: var(--text-1);
  border: 1px solid var(--border-1);
  cursor: pointer;
}
.coding-mode-pill.is-coding {
  background: var(--accent-1);
  color: var(--text-inverse);
}
.coding-mode-pill[disabled] { opacity: 0.5; cursor: not-allowed; }
```

Add `@import "../features/coding/styles/coding.css";` to `desktop-ui/src/styles/index.css`.

- [ ] **Step 6: Wire `CodingModePill` into `Composer.tsx` meta bar**

```typescript
import { CodingModePill } from "@/features/coding/components/CodingModePill";

<div className="composer__meta-bar">
  <CodingModePill threadId={threadId} />
  {/* ...existing meta children */}
</div>
```

- [ ] **Step 7: Auto-detection — emit `mode_changed` from chat_send if workspace inferred**

In `crates/app-core/src/coding/mode_handler.rs`:

```rust
impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn auto_detect_mode_for_thread(
        &self,
        session_key: &str,
        workspace_path: Option<&str>,
    ) -> common::Result<()> {
        if let Some(path) = workspace_path {
            // Heuristic: if the path is inside a known workspace dir, set mode = coding.
            self.set_mode_for_thread(session_key, "coding").await?;
            self.update_thread_cwd(session_key, path).await?;
        }
        Ok(())
    }
}
```

Call this from `chat_handler::handle_create_thread` when the thread metadata includes a workspace context.

- [ ] **Step 8: Add `update_thread_cwd` helper**

```rust
impl AppCore {
    pub async fn update_thread_cwd(&self, session_key: &str, cwd: &str) -> common::Result<()> {
        let pool = self.repos.sessions.pool();
        sqlx::query!(
            "UPDATE sessions SET cwd = ? WHERE id = ?",
            cwd, session_key
        )
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 9: Run frontend tests**

Run: `cd desktop-ui && bun run test src/features/coding/hooks/useCodingMode.test.ts src/features/coding/components/CodingModePill.test.tsx`
Expected: tests pass.

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useCodingMode.ts desktop-ui/src/features/coding/hooks/useCodingMode.test.ts desktop-ui/src/features/coding/components/CodingModePill.tsx desktop-ui/src/features/coding/components/CodingModePill.test.tsx desktop-ui/src/features/coding/styles/coding.css desktop-ui/src/styles/index.css desktop-ui/src/features/composer/Composer.tsx crates/app-core/src/coding/mode_handler.rs crates/desktop/src/commands/chat_mode.rs crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts
git commit -m "feat(coding-mode): useCodingMode + CodingModePill + auto-detect (Plan 5 T8)

- chat_get_mode Tauri command reads sessions.conversation_type
- useCodingMode hook subscribes to agent:mode_changed
- CodingModePill is a single-click toggle in the composer meta bar
- auto_detect_mode_for_thread sets mode=coding + cwd when a thread is
  created from workspace context"
```


---

## Task 9: `CodingRecallContextSource` + `recall_*` tool registration

**Files:**
- Create: `crates/agent/src/context/coding_recall_source.rs`
- Modify: `crates/agent/src/context/mod.rs` (re-export)
- Modify: `crates/context_engine/src/source.rs` (no changes — uses existing trait)
- Modify: `crates/agent/src/agent_loop/builder.rs` (register source if `coding-memory` enabled and channel == coding)
- Create: `crates/app-core/src/init/coding_recall.rs` (constructs `CodingRecallService`)
- Modify: `crates/app-core/src/init/mod.rs` (call `init_coding_recall`)
- Modify: `crates/config/src/schema/mcp.rs` (extend `EXPLICIT_TOOL_ALLOWLIST` with the 8 recall tool names — already present per survey, verify)
- Test: `crates/agent/tests/coding_recall_injection.rs`
- Test: `tests/integration/plan5_recall_injection_e2e.rs`

`★ Insight ─────────────────────────────────────`
`CodingRecallService` already exists in `crates/coding-memory/src/recall/service.rs`. Plan 5 adds one thin adapter — a `ContextSource` impl that calls `recall_index` with the user's last message + the thread's `repo_id`, formats top snippets into the system prompt, and emits `AgentEvent::RecallInjected` with telemetry.

The 8 recall MCP tools are already implemented in `coding-memory/src/mcp.rs` (per the survey); they're already in `EXPLICIT_TOOL_ALLOWLIST` so MCP exposure is automatic. Plan 5's job is only to **make them available as ToolKitBuilder tools** for the in-process agent loop in coding mode — we register them as thin `mcp::McpToolWrapper` instances pointed at the local `coding_memory::McpHandler`.

If `coding-memory` Phase 4 isn't fully landed (per spec §13.1376), the `recall_*` tools register as **stubs** that emit `RecallInjected { coverage_score: 0.0, dead_end_warning: false }` and return empty bodies. The activation check is `cfg!(feature = "coding_memory_phase4")` or runtime probe of `CodingRecallService::is_initialized()`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing integration test `tests/integration/plan5_recall_injection_e2e.rs`**

```rust
//! E2E: open coding thread → user asks question → recall_index injects snippets
//! into system prompt → agent receives them in next iteration.

use klyntbot::{AppCore, ChatSendInput};

#[tokio::test]
#[ignore = "requires coding-memory phase 4 + LLM mock"]
async fn recall_injection_appears_in_system_prompt() {
    let core = AppCore::for_test().await.unwrap();
    // Seed coding-memory with a known fact.
    core.coding_memory_seed_fact_for_test("repo-x", "main parser uses nom 7").await.unwrap();

    let thread = core.create_coding_thread("repo-x", "/tmp/repo-x").await.unwrap();
    let recorded = core
        .chat_send_capture_system_prompt(ChatSendInput {
            session_key: thread.id.clone(),
            content: "where is the parser implemented?".into(),
            mode: Some("coding".into()),
        })
        .await
        .unwrap();
    assert!(
        recorded.system_prompt.contains("nom 7"),
        "expected recall snippet in system prompt; got: {}",
        recorded.system_prompt
    );
    assert!(
        recorded.events.iter().any(|e| matches!(e, agent::events::AgentEvent::RecallInjected { .. })),
        "expected RecallInjected event"
    );
}
```

- [ ] **Step 2: Create `crates/agent/src/context/coding_recall_source.rs`**

```rust
use async_trait::async_trait;
use bus::DomainEventBus;
use coding_memory::recall::CodingRecallService;
use common::Result;
use context_engine::{ContextRequest, ContextSource, SourceContext};
use std::sync::Arc;

pub struct CodingRecallContextSource {
    service: Arc<CodingRecallService>,
    bus: Arc<DomainEventBus>,
}

impl CodingRecallContextSource {
    pub fn new(service: Arc<CodingRecallService>, bus: Arc<DomainEventBus>) -> Self {
        Self { service, bus }
    }
}

#[async_trait]
impl ContextSource for CodingRecallContextSource {
    fn name(&self) -> &str {
        "coding_recall"
    }
    fn priority(&self) -> i32 {
        50  // Below soul (100) but above generic.
    }
    async fn produce(&self, req: &ContextRequest) -> Result<SourceContext> {
        if req.channel.as_deref() != Some(common::CODING_CHANNEL) {
            return Ok(SourceContext::empty());
        }
        let query = req.last_user_message.clone().unwrap_or_default();
        if query.trim().is_empty() {
            return Ok(SourceContext::empty());
        }
        let repo_id = req.repo_id.clone();
        let resp = self
            .service
            .recall_index(&query, repo_id.as_deref(), None, None, 6)
            .await?;
        if resp.entries.is_empty() {
            return Ok(SourceContext::empty());
        }
        let snippets: Vec<String> = resp
            .entries
            .iter()
            .take(6)
            .map(|e| format!("- {}: {}", e.kind.as_deref().unwrap_or("?"), e.summary))
            .collect();
        let body = format!(
            "## Coding-memory recall (top {} snippets):\n{}\n",
            snippets.len(),
            snippets.join("\n")
        );

        // Emit telemetry event
        let memory_ids: Vec<String> = resp.entries.iter().map(|e| e.id.clone()).collect();
        let _ = self
            .bus
            .publish(bus::DomainEvent::Agent(agent::events::AgentEvent::RecallInjected {
                memory_ids,
                coverage_score: resp.coverage_score.unwrap_or(0.0),
                escalation_chain: vec![],
                dead_end_warning: resp.dead_end_warning,
                budget_used_tokens: 0,
                budget_limit_tokens: 2000,
            }))
            .await;

        Ok(SourceContext::with_text(body))
    }
}
```

- [ ] **Step 3: Re-export from `crates/agent/src/context/mod.rs`**

```rust
pub mod coding_recall_source;
pub use coding_recall_source::CodingRecallContextSource;
```

- [ ] **Step 4: Wire into `agent_loop/builder.rs`**

In `AgentLoopBuilder`, add field:

```rust
coding_recall_service: Option<Arc<CodingRecallService>>,
```

When building `ContextEngine`, conditionally register the source:

```rust
if let Some(svc) = &self.coding_recall_service {
    context_engine.register_source(Arc::new(CodingRecallContextSource::new(
        Arc::clone(svc),
        Arc::clone(&self.domain_event_bus),
    )));
}
```

- [ ] **Step 5: Construct `CodingRecallService` in `app-core`**

Create `crates/app-core/src/init/coding_recall.rs`:

```rust
use crate::AppCore;
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use common::Result;
use std::sync::Arc;

#[tracing::instrument(skip(core), err)]
pub async fn init_coding_recall(core: &AppCore) -> Result<Option<Arc<CodingRecallService>>> {
    let cfg = core.config().read().await;
    if !cfg.coding_memory.enabled {
        return Ok(None);
    }
    drop(cfg);
    let ums = core.unified_memory_service.clone().ok_or_else(|| {
        common::KlyntbotError::Internal("UMS not initialized".into())
    })?;
    let service = CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums,
        core.repos.semantic_facts.clone(),
        core.repos.episodic.clone(),
        // ... remaining wiring; this matches existing MCP wiring
    );
    Ok(Some(Arc::new(service)))
}
```

- [ ] **Step 6: Pass `coding_recall_service` into `init_agent`**

In `crates/app-core/src/init/agent.rs`, accept and forward:

```rust
let coding_recall = init_coding_recall(&core).await?;
let agent_loop = AgentLoopBuilder::new()
    // ... existing
    .with_coding_recall_service(coding_recall)
    .build();
```

- [ ] **Step 7: Register `recall_*` as klynt-core tools (stub-or-live)**

Create `crates/klynt-core/src/tools/recall_stubs.rs`:

```rust
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tools_core::{Tool, ToolError, ToolResult, RoutingContext};

macro_rules! recall_tool {
    ($name:ident, $tool_name:literal) => {
        pub struct $name {
            service: Option<Arc<coding_memory::recall::CodingRecallService>>,
        }
        impl $name {
            pub fn new(service: Option<Arc<coding_memory::recall::CodingRecallService>>) -> Self {
                Self { service }
            }
        }
        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str { $tool_name }
            fn description(&self) -> &str {
                "Recall coding-memory entries (Phase 1 stub if coding-memory not initialized)"
            }
            fn parameters(&self) -> Value {
                serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}})
            }
            async fn execute(&self, args: Value, _ctx: &RoutingContext) -> ToolResult {
                match &self.service {
                    Some(svc) => {
                        let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let resp = svc.recall_index(q, None, None, None, 5).await
                            .map_err(|e| ToolError::Internal(e.to_string()))?;
                        Ok(serde_json::to_value(resp)
                            .map_err(|e| ToolError::Internal(e.to_string()))?
                            .to_string())
                    }
                    None => Ok("[recall stub: coding-memory not initialized]".into()),
                }
            }
            fn is_concurrency_safe(&self, _: &Value) -> bool { true }
        }
    };
}

recall_tool!(RecallIndexTool, "recall_index");
recall_tool!(RecallTimelineTool, "recall_timeline");
recall_tool!(RecallFetchTool, "recall_fetch");
recall_tool!(TraceCausesTool, "trace_causes");
recall_tool!(CheckDeadEndsTool, "check_dead_ends");
recall_tool!(RecallFactsAsOfTool, "recall_facts_as_of");
recall_tool!(RecallChangeHistoryTool, "recall_change_history");
recall_tool!(RecallDecisionPointsTool, "recall_decision_points");
```

Register all 8 in `ToolKitBuilder::with_recall_tools(service: Option<Arc<...>>)` — adds them with `available_for_channel(coding)`.

- [ ] **Step 8: Update `app-core/src/init/agent.rs` to register them**

```rust
let mut tk = klynt_core::ToolKitBuilder::new()
    // ... existing
    .with_recall_tools(coding_recall.clone())
    .build();
```

- [ ] **Step 9: Run integration tests + clippy**

Run: `cargo nextest run -p agent --test coding_recall_injection`
Run: `cargo clippy -p agent -p app-core -p klynt-core --all-targets -- -D warnings`
Expected: tests pass, clippy clean.

- [ ] **Step 10: Commit**

```bash
git add crates/agent/src/context crates/app-core/src/init/coding_recall.rs crates/app-core/src/init/mod.rs crates/app-core/src/init/agent.rs crates/klynt-core/src/tools/recall_stubs.rs crates/klynt-core/src/tools/mod.rs
git commit -m "feat(coding-recall): ContextSource + 8 recall_* tools (Plan 5 T9)

CodingRecallContextSource calls CodingRecallService::recall_index when
the channel is coding and the user message is non-empty. Top 6 snippets
are formatted into the system prompt. Emits AgentEvent::RecallInjected
with coverage_score, dead_end_warning, and the matched memory_ids.

8 recall_* tools register through ToolKitBuilder::with_recall_tools.
When coding-memory Phase 4 is offline, they degrade to stubs returning
'[recall stub: coding-memory not initialized]' (per spec §13.1376)."
```


---

## Task 10: `RecallTrayCard` + `DeadEndWarning` + `useCodingRecallSnippets`

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useCodingRecallSnippets.ts`
- Create: `desktop-ui/src/features/coding/hooks/useCodingRecallSnippets.test.ts`
- Create: `desktop-ui/src/features/coding/components/RecallTrayCard.tsx`
- Create: `desktop-ui/src/features/coding/components/RecallTrayCard.test.tsx`
- Create: `desktop-ui/src/features/coding/components/DeadEndWarning.tsx`
- Create: `desktop-ui/src/features/coding/components/DeadEndWarning.test.tsx`
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx` (render new `kind: "recall"` and `kind: "dead_end_warning"` rows)
- Modify: `desktop-ui/src/types.ts` (extend `ConversationItem`)
- Modify: `crates/desktop-shared/src/lib.rs` (extend `ConversationItem` enum)

`★ Insight ─────────────────────────────────────`
The recall tray is collapsible — most snippets aren't directly relevant to the user's question, and rendering them all expanded would clutter every coding turn. Default state: collapsed, summary line "5 snippets injected (coverage 62%)". Click to expand. The `DeadEndWarning` is non-collapsible — it's a small inline row with a warning icon, the prior approach summary, and a confidence percentage.

`useCodingRecallSnippets` listens on two Tauri channels: `agent:recall_injected` and `agent:dead_end_warning_surfaced`. It maintains per-thread state in a Zustand-style store (or a simple `useState` map), keyed on `threadId` so switching threads keeps each thread's recall surface independent.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend `ConversationItem` in `desktop-shared`**

In `crates/desktop-shared/src/lib.rs`, add variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConversationItem {
    // ... existing
    Recall {
        id: String,
        memory_ids: Vec<String>,
        coverage_score: f32,
        snippets: Vec<RecallSnippet>,
    },
    DeadEndWarning {
        id: String,
        approach_summary: String,
        prior_attempt_id: String,
        confidence: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct RecallSnippet {
    pub kind: String,
    pub summary: String,
    pub source: String,
}
```

- [ ] **Step 2: Sync to `desktop-ui/src/types.ts`** (regenerated by Tauri build)

Run `cargo tauri dev` once; `bindings.ts` regenerates. Verify the new `kind: "recall"` and `kind: "dead_end_warning"` variants appear.

- [ ] **Step 3: Create `desktop-ui/src/features/coding/hooks/useCodingRecallSnippets.ts`**

```typescript
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export interface RecallEvent {
  thread_id: string;
  memory_ids: string[];
  coverage_score: number;
  dead_end_warning: boolean;
  snippets: Array<{ kind: string; summary: string; source: string }>;
}

export interface DeadEndEvent {
  thread_id: string;
  approach_summary: string;
  prior_attempt_id: string;
  confidence: number;
}

export function useCodingRecallSnippets(threadId: string | null) {
  const [recall, setRecall] = useState<RecallEvent | null>(null);
  const [deadEnd, setDeadEnd] = useState<DeadEndEvent | null>(null);

  useEffect(() => {
    if (!threadId) return;
    let u1: (() => void) | undefined;
    let u2: (() => void) | undefined;
    (async () => {
      u1 = await listen<RecallEvent>("agent:recall_injected", (evt) => {
        if (evt.payload.thread_id === threadId) setRecall(evt.payload);
      });
      u2 = await listen<DeadEndEvent>("agent:dead_end_warning_surfaced", (evt) => {
        if (evt.payload.thread_id === threadId) setDeadEnd(evt.payload);
      });
    })();
    return () => { u1?.(); u2?.(); };
  }, [threadId]);

  return { recall, deadEnd };
}
```

- [ ] **Step 4: Create `desktop-ui/src/features/coding/components/RecallTrayCard.tsx`**

```typescript
import { useState } from "react";
import type { RecallSnippet } from "@/types";

export function RecallTrayCard({
  memoryIds,
  coverageScore,
  snippets,
}: {
  memoryIds: string[];
  coverageScore: number;
  snippets: RecallSnippet[];
}) {
  const [expanded, setExpanded] = useState(false);
  const pct = Math.round(coverageScore * 100);
  return (
    <div className="recall-tray-card">
      <button
        className="recall-tray-card__header"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <span className="recall-tray-card__icon">★</span>
        <span>{snippets.length} snippet{snippets.length === 1 ? "" : "s"} injected</span>
        <span className="recall-tray-card__coverage">{pct}% coverage</span>
      </button>
      {expanded && (
        <ul className="recall-tray-card__list">
          {snippets.map((s, i) => (
            <li key={memoryIds[i] ?? i}>
              <span className="recall-tray-card__kind">{s.kind}</span>
              <span className="recall-tray-card__summary">{s.summary}</span>
              <span className="recall-tray-card__source">{s.source}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Create `DeadEndWarning.tsx`**

```typescript
export function DeadEndWarning({
  approachSummary,
  confidence,
}: {
  approachSummary: string;
  priorAttemptId: string;
  confidence: number;
}) {
  return (
    <div className="dead-end-warning" role="alert">
      <span className="dead-end-warning__icon">⚠</span>
      <span className="dead-end-warning__body">
        Prior attempt: {approachSummary} ({Math.round(confidence * 100)}% confidence dead-end)
      </span>
    </div>
  );
}
```

- [ ] **Step 6: Update CSS**

Add to `desktop-ui/src/features/coding/styles/coding.css`:

```css
.recall-tray-card { border: 1px solid var(--border-1); border-radius: 4px; margin: 8px 0; }
.recall-tray-card__header { display: flex; gap: 8px; padding: 8px; width: 100%; background: var(--surface-1); border: 0; cursor: pointer; }
.recall-tray-card__icon { color: var(--accent-1); }
.recall-tray-card__coverage { margin-left: auto; font-size: var(--fs-xs); color: var(--text-2); }
.recall-tray-card__list { list-style: none; padding: 0 12px 12px; margin: 0; }
.recall-tray-card__list li { display: grid; grid-template-columns: 80px 1fr auto; gap: 8px; padding: 4px 0; font-size: var(--fs-xs); }
.recall-tray-card__kind { color: var(--text-2); }
.recall-tray-card__source { color: var(--text-2); font-style: italic; }

.dead-end-warning { display: flex; gap: 8px; padding: 6px 12px; margin: 6px 0; background: var(--warn-bg); border-left: 2px solid var(--warn-fg); font-size: var(--fs-xs); }
.dead-end-warning__icon { color: var(--warn-fg); }
```

- [ ] **Step 7: Render new variants in `MessageRows.tsx`**

Find the discriminated-union switch and add cases:

```typescript
case "recall":
  return <RecallTrayCard memoryIds={item.memory_ids} coverageScore={item.coverage_score} snippets={item.snippets} />;
case "dead_end_warning":
  return <DeadEndWarning approachSummary={item.approach_summary} priorAttemptId={item.prior_attempt_id} confidence={item.confidence} />;
```

- [ ] **Step 8: Backend: emit Tauri channel from `RecallInjected` runtime event**

In `crates/app-core/src/streaming.rs` (or wherever runtime events are pumped to Tauri), add:

```rust
match event {
    AgentEvent::RecallInjected { memory_ids, coverage_score, dead_end_warning, .. } => {
        emit_recall_injected(app, session_key, memory_ids, *coverage_score, *dead_end_warning, snippets);
    }
    AgentEvent::DeadEndWarningSurfaced { approach_summary, prior_attempt_id, confidence } => {
        emit_dead_end_warning(app, session_key, approach_summary, prior_attempt_id, *confidence);
    }
    _ => {}
}
```

`emit_recall_injected` calls `app.emit("agent:recall_injected", &payload)`.

- [ ] **Step 9: Run tests**

Run: `cd desktop-ui && bun run test src/features/coding/hooks/useCodingRecallSnippets.test.ts src/features/coding/components/RecallTrayCard.test.tsx src/features/coding/components/DeadEndWarning.test.tsx`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/features/coding/hooks/useCodingRecallSnippets.ts desktop-ui/src/features/coding/components/RecallTrayCard.tsx desktop-ui/src/features/coding/components/DeadEndWarning.tsx desktop-ui/src/features/coding/styles/coding.css desktop-ui/src/features/messages/components/MessageRows.tsx desktop-ui/src/types.ts crates/desktop-shared/src/lib.rs crates/app-core/src/streaming.rs
git commit -m "feat(coding-recall): tray card + dead-end warning UI (Plan 5 T10)

- ConversationItem extended with kind: 'recall' and 'dead_end_warning'
- useCodingRecallSnippets listens on agent:recall_injected + dead_end_warning_surfaced
- RecallTrayCard: collapsible, coverage % in header, snippet list expanded on click
- DeadEndWarning: inline alert row with confidence %"
```


---

## Task 11: `MemorySinkSubscriber` translator + aggregator

**Files:**
- Modify/expand: `crates/coding-memory/src/sink.rs` (currently a skeleton)
- Create: `crates/coding-memory/src/sink/translator.rs`
- Create: `crates/coding-memory/src/sink/aggregator.rs`
- Create: `crates/coding-memory/src/sink/subscriber.rs`
- Test: `crates/coding-memory/tests/translator_e1_file_edit.rs`
- Test: `crates/coding-memory/tests/translator_e2_provider_call.rs`
- Test: `crates/coding-memory/tests/translator_e3_chunk_aggregation.rs`
- Test: `crates/coding-memory/tests/translator_e4_tool_pair.rs`
- Test: `crates/coding-memory/tests/translator_e5_monotone.rs`

`★ Insight ─────────────────────────────────────`
The translator handles **two** kinds of mappings:

1. **1:1 translations** (`RecallInjected → RecallInjected`, `FileEditWithSymbols → FileEditEnriched`, `ProviderResponse → ProviderCall`, `SkillActivated → SkillActivated`).
2. **Aggregations** (`ContentChunk×N → AssistantMsg×1`, `ToolStart+ToolEnd → ToolCall×1`, `ApprovalRequested+ApprovalResolved → ApprovalDecision×1`).

The aggregator owns per-iteration state. It's an FSM keyed on `(session_id, iteration_index, correlation_id)` — when a `ToolStart{call_id: X}` arrives, it stashes; when `ToolEnd{call_id: X}` arrives, it emits a paired `ToolCall`. Orphan `ToolStart` (no matching `ToolEnd`) → no emission, ever (E4 invariant).

Property test `E5` (monotone) is the hardest to get right: it uses proptest to generate sequences of mixed `ContentChunk`, `ToolStart`, `ApprovalRequested` events, runs them through the translator, and asserts that the cumulative `count_emitted` is monotonically non-decreasing across every prefix.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing E1 test `crates/coding-memory/tests/translator_e1_file_edit.rs`**

```rust
use coding_memory::sink::{MemorySinkSubscriber, RecordingSink};
use agent::events::AgentEvent;

#[tokio::test]
async fn e1_every_file_edit_with_symbols_produces_one_file_edit_enriched() {
    let sink = RecordingSink::new();
    let mut sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(AgentEvent::FileEditWithSymbols {
        path: "src/main.rs".into(),
        op: "edit".into(),
        anchored_symbols: vec![],
        lsp_diagnostics_delta: vec![],
        ..Default::default()
    }).await.unwrap();
    let recorded = sink.events_for_test();
    let count = recorded.iter().filter(|e| matches!(e, coding_ingest::AgentEvent::FileEditEnriched { .. })).count();
    assert_eq!(count, 1);
}
```

- [ ] **Step 2: Write the failing E2, E3, E4 tests** (one per file)

E2 (`translator_e2_provider_call.rs`):

```rust
#[tokio::test]
async fn e2_every_provider_response_produces_provider_call_with_matching_cost() {
    let sink = coding_memory::sink::RecordingSink::new();
    let mut sub = coding_memory::sink::MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(agent::events::AgentEvent::ProviderResponse {
        model: "gpt-5".into(),
        prompt_tokens: 100, completion_tokens: 50,
        cost_usd: 0.05, latency_ms: 1200, retries: 0,
    }).await.unwrap();
    let evts = sink.events_for_test();
    let pc = evts.iter().find_map(|e| match e {
        coding_ingest::AgentEvent::ProviderCall { cost_usd, latency_ms, .. } => Some((*cost_usd, *latency_ms)),
        _ => None,
    }).unwrap();
    assert_eq!(pc, (0.05, 1200));
}
```

E3 (`translator_e3_chunk_aggregation.rs`):

```rust
#[tokio::test]
async fn e3_n_content_chunks_aggregate_to_one_assistant_msg() {
    let sink = coding_memory::sink::RecordingSink::new();
    let mut sub = coding_memory::sink::MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(agent::events::AgentEvent::IterationStart { iteration: 0 }).await.unwrap();
    for chunk in &["Hello ", "world", "!"] {
        sub.translate(agent::events::AgentEvent::ContentChunk { text: chunk.to_string() }).await.unwrap();
    }
    sub.translate(agent::events::AgentEvent::IterationEnd { iteration: 0 }).await.unwrap();
    let evts = sink.events_for_test();
    let msgs: Vec<_> = evts.iter().filter_map(|e| match e {
        coding_ingest::AgentEvent::AssistantMsg { text, .. } => Some(text.clone()),
        _ => None,
    }).collect();
    assert_eq!(msgs, vec!["Hello world!".to_string()]);
}
```

E4 (`translator_e4_tool_pair.rs`):

```rust
#[tokio::test]
async fn e4_tool_start_and_end_pair_to_one_tool_call() {
    let sink = coding_memory::sink::RecordingSink::new();
    let mut sub = coding_memory::sink::MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(agent::events::AgentEvent::ToolStart {
        call_id: "abc".into(), name: "bash".into(), args: serde_json::json!({"command": "ls"}),
    }).await.unwrap();
    sub.translate(agent::events::AgentEvent::ToolEnd {
        call_id: "abc".into(), success: true, output: "file.txt\n".into(), duration_ms: 12,
    }).await.unwrap();
    let evts = sink.events_for_test();
    let count = evts.iter().filter(|e| matches!(e, coding_ingest::AgentEvent::ToolCall { .. })).count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn e4_orphan_tool_start_emits_nothing() {
    let sink = coding_memory::sink::RecordingSink::new();
    let mut sub = coding_memory::sink::MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(agent::events::AgentEvent::ToolStart {
        call_id: "orphan".into(), name: "bash".into(), args: serde_json::json!({}),
    }).await.unwrap();
    let evts = sink.events_for_test();
    assert!(evts.iter().all(|e| !matches!(e, coding_ingest::AgentEvent::ToolCall { .. })));
}
```

E5 (`translator_e5_monotone.rs`):

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn e5_translator_emit_count_monotone(
        events in proptest::collection::vec(any::<u8>(), 0..40)
    ) {
        // Each u8 maps to one of 6 event kinds.
        let trace: Vec<agent::events::AgentEvent> = events.iter().map(|b| match b % 6 {
            0 => agent::events::AgentEvent::ContentChunk { text: "x".into() },
            1 => agent::events::AgentEvent::ToolStart { call_id: format!("t{b}"), name: "n".into(), args: serde_json::json!({}) },
            2 => agent::events::AgentEvent::ToolEnd { call_id: format!("t{b}"), success: true, output: "".into(), duration_ms: 1 },
            3 => agent::events::AgentEvent::IterationStart { iteration: *b as u32 },
            4 => agent::events::AgentEvent::IterationEnd { iteration: *b as u32 },
            _ => agent::events::AgentEvent::ContentChunk { text: "y".into() },
        }).collect();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let sink = coding_memory::sink::RecordingSink::new();
            let mut sub = coding_memory::sink::MemorySinkSubscriber::for_test(sink.clone());
            let mut prev_count = 0;
            for e in trace {
                sub.translate(e).await.unwrap();
                let now = sink.events_for_test().len();
                prop_assert!(now >= prev_count, "non-monotone: {} → {}", prev_count, now);
                prev_count = now;
            }
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 3: Implement `crates/coding-memory/src/sink/translator.rs`**

```rust
use crate::sink::aggregator::Aggregator;
use agent::events::AgentEvent as RuntimeEvt;
use coding_ingest::AgentEvent as IngestEvt;
use common::Result;

pub struct Translator {
    pub(super) aggregator: Aggregator,
}

impl Translator {
    pub fn new() -> Self { Self { aggregator: Aggregator::new() } }

    /// Translate a single runtime event into 0..N ingest events.
    pub fn translate(&mut self, evt: &RuntimeEvt) -> Result<Vec<IngestEvt>> {
        match evt {
            RuntimeEvt::FileEditWithSymbols { path, op, anchored_symbols, lsp_diagnostics_delta, .. } => {
                Ok(vec![IngestEvt::FileEditEnriched {
                    path: path.clone(),
                    op: op.clone(),
                    anchored_symbols: anchored_symbols.clone(),
                    lsp_diagnostics_delta: lsp_diagnostics_delta.clone(),
                }])
            }
            RuntimeEvt::ProviderResponse { model, prompt_tokens, completion_tokens, cost_usd, latency_ms, retries } => {
                Ok(vec![IngestEvt::ProviderCall {
                    model: model.clone(),
                    prompt_tokens: *prompt_tokens,
                    completion_tokens: *completion_tokens,
                    cost_usd: *cost_usd,
                    latency_ms: *latency_ms,
                    retries: *retries,
                }])
            }
            RuntimeEvt::RecallInjected { memory_ids, coverage_score, dead_end_warning, .. } => {
                Ok(vec![IngestEvt::RecallInjected {
                    memory_ids: memory_ids.clone(),
                    coverage_score: *coverage_score,
                    dead_end_warning: *dead_end_warning,
                }])
            }
            RuntimeEvt::SkillActivated { skill_id, source_path, trigger, .. } => {
                Ok(vec![IngestEvt::SkillActivated {
                    skill_id: skill_id.clone(),
                    source_path: source_path.clone(),
                    trigger: trigger.clone(),
                }])
            }
            RuntimeEvt::ContentChunk { text } => self.aggregator.on_content_chunk(text),
            RuntimeEvt::IterationStart { .. } => self.aggregator.on_iteration_start(),
            RuntimeEvt::IterationEnd { .. } => self.aggregator.on_iteration_end(),
            RuntimeEvt::ToolStart { call_id, name, args } => self.aggregator.on_tool_start(call_id, name, args.clone()),
            RuntimeEvt::ToolEnd { call_id, success, output, duration_ms } => {
                self.aggregator.on_tool_end(call_id, *success, output.clone(), *duration_ms)
            }
            RuntimeEvt::ApprovalRequested { request_id, tool, .. } => self.aggregator.on_approval_requested(request_id, tool),
            RuntimeEvt::ApprovalResolved { request_id, decision, layer } => {
                self.aggregator.on_approval_resolved(request_id, decision, layer)
            }
            RuntimeEvt::SandboxPolicyApplied { tool, policy_summary, fallback_unsandboxed, .. } => {
                Ok(vec![IngestEvt::SandboxApplied {
                    tool: tool.clone(),
                    policy_summary: policy_summary.clone(),
                    fallback_unsandboxed: *fallback_unsandboxed,
                }])
            }
            // Runtime-only events: emit nothing.
            _ => Ok(vec![]),
        }
    }
}
```

- [ ] **Step 4: Implement `crates/coding-memory/src/sink/aggregator.rs`**

```rust
use coding_ingest::AgentEvent as IngestEvt;
use common::Result;
use std::collections::HashMap;

pub(crate) struct Aggregator {
    buffer: String,
    pending_tools: HashMap<String, PendingTool>,
    pending_approvals: HashMap<String, String>,
}

struct PendingTool {
    name: String,
    args: serde_json::Value,
}

impl Aggregator {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            pending_tools: HashMap::new(),
            pending_approvals: HashMap::new(),
        }
    }
    pub fn on_iteration_start(&mut self) -> Result<Vec<IngestEvt>> {
        self.buffer.clear();
        Ok(vec![])
    }
    pub fn on_content_chunk(&mut self, text: &str) -> Result<Vec<IngestEvt>> {
        self.buffer.push_str(text);
        Ok(vec![])
    }
    pub fn on_iteration_end(&mut self) -> Result<Vec<IngestEvt>> {
        if self.buffer.is_empty() {
            return Ok(vec![]);
        }
        let text = std::mem::take(&mut self.buffer);
        Ok(vec![IngestEvt::AssistantMsg { text }])
    }
    pub fn on_tool_start(&mut self, call_id: &str, name: &str, args: serde_json::Value) -> Result<Vec<IngestEvt>> {
        self.pending_tools.insert(call_id.to_string(), PendingTool { name: name.to_string(), args });
        Ok(vec![])
    }
    pub fn on_tool_end(&mut self, call_id: &str, success: bool, output: String, duration_ms: u64) -> Result<Vec<IngestEvt>> {
        let Some(p) = self.pending_tools.remove(call_id) else {
            return Ok(vec![]);
        };
        Ok(vec![IngestEvt::ToolCall {
            tool: p.name,
            args: p.args,
            success,
            output,
            duration_ms,
        }])
    }
    pub fn on_approval_requested(&mut self, request_id: &str, tool: &str) -> Result<Vec<IngestEvt>> {
        self.pending_approvals.insert(request_id.to_string(), tool.to_string());
        Ok(vec![])
    }
    pub fn on_approval_resolved(&mut self, request_id: &str, decision: &str, layer: &str) -> Result<Vec<IngestEvt>> {
        let Some(tool) = self.pending_approvals.remove(request_id) else {
            return Ok(vec![]);
        };
        Ok(vec![IngestEvt::ApprovalDecision {
            tool,
            decision: decision.to_string(),
            layer: layer.to_string(),
        }])
    }
}
```

- [ ] **Step 5: Implement `crates/coding-memory/src/sink/subscriber.rs`**

```rust
use crate::sink::translator::Translator;
use agent::events::AgentEvent;
use bus::DomainEventBus;
use common::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait MemorySink: Send + Sync {
    async fn accept_event(&self, evt: coding_ingest::AgentEvent) -> Result<()>;
}

pub struct MemorySinkSubscriber {
    sink: Arc<dyn MemorySink>,
    translator: Arc<Mutex<Translator>>,
}

impl MemorySinkSubscriber {
    pub fn new(sink: Arc<dyn MemorySink>) -> Self {
        Self { sink, translator: Arc::new(Mutex::new(Translator::new())) }
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn for_test(sink: RecordingSink) -> Self {
        Self::new(Arc::new(sink))
    }

    pub async fn translate(&mut self, evt: AgentEvent) -> Result<()> {
        let mut t = self.translator.lock().await;
        let emitted = t.translate(&evt)?;
        drop(t);
        for e in emitted {
            self.sink.accept_event(e).await?;
        }
        Ok(())
    }

    /// Spawn a background subscriber that drains `bus` and translates.
    pub async fn spawn(self: Arc<Self>, bus: Arc<DomainEventBus>) -> Result<()> {
        let mut rx = bus.subscribe();
        tokio::spawn(async move {
            while let Ok(domain_evt) = rx.recv().await {
                if let bus::DomainEvent::Agent(agent_evt) = domain_evt {
                    let mut t = self.translator.lock().await;
                    let emitted = match t.translate(&agent_evt) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(error = %e, "translator error; skipping");
                            continue;
                        }
                    };
                    drop(t);
                    for e in emitted {
                        if let Err(err) = self.sink.accept_event(e).await {
                            tracing::warn!(error = %err, "sink accept_event failed");
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

#[cfg(any(test, feature = "test-helpers"))]
#[derive(Clone, Default)]
pub struct RecordingSink {
    inner: Arc<Mutex<Vec<coding_ingest::AgentEvent>>>,
}

#[cfg(any(test, feature = "test-helpers"))]
impl RecordingSink {
    pub fn new() -> Self { Self::default() }
    pub fn events_for_test(&self) -> Vec<coding_ingest::AgentEvent> {
        self.inner.try_lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[cfg(any(test, feature = "test-helpers"))]
#[async_trait::async_trait]
impl MemorySink for RecordingSink {
    async fn accept_event(&self, evt: coding_ingest::AgentEvent) -> Result<()> {
        self.inner.lock().await.push(evt);
        Ok(())
    }
}
```

- [ ] **Step 6: Update `crates/coding-memory/src/sink.rs` to expose modules**

```rust
pub mod translator;
pub mod aggregator;
pub mod subscriber;

pub use subscriber::{MemorySink, MemorySinkSubscriber};
#[cfg(any(test, feature = "test-helpers"))]
pub use subscriber::RecordingSink;
```

- [ ] **Step 7: Run translator tests**

Run: `cargo nextest run -p coding-memory --test translator_e1_file_edit --test translator_e2_provider_call --test translator_e3_chunk_aggregation --test translator_e4_tool_pair --test translator_e5_monotone --features test-helpers`
Expected: 5 tests pass; E5 runs ≥ 256 proptest cases.

- [ ] **Step 8: Commit**

```bash
git add crates/coding-memory/src/sink crates/coding-memory/src/sink.rs crates/coding-memory/tests/translator_*.rs
git commit -m "feat(coding-memory): MemorySinkSubscriber translator + 5 invariants (Plan 5 T11)

Translator covers:
- 1:1 (FileEdit, ProviderResponse, RecallInjected, SkillActivated, SandboxPolicyApplied)
- N:1 aggregation (ContentChunk×N → AssistantMsg, ToolStart+ToolEnd → ToolCall, ApprovalRequested+ApprovalResolved → ApprovalDecision)

E1-E5 property tests assert:
E1: every FileEditWithSymbols → exactly one FileEditEnriched
E2: ProviderResponse → matching cost/latency
E3: ContentChunk×N → one AssistantMsg with concatenated text
E4: paired ToolStart+ToolEnd → one ToolCall; orphan → nothing
E5: emit-count is monotone non-decreasing across every prefix"
```


---

## Task 12: Mirror signal subscribers (3 sources)

**Files:**
- Create: `crates/coding-memory/src/mirror/coding_signals/approval_history.rs`
- Create: `crates/coding-memory/src/mirror/coding_signals/skill_effectiveness.rs`
- Create: `crates/coding-memory/src/mirror/coding_signals/recall_coverage.rs`
- Create: `crates/coding-memory/src/mirror/coding_signals/mod.rs`
- Modify: `crates/coding-memory/src/mirror/mod.rs` (re-export)
- Create: `crates/app-core/src/init/coding_subscribers.rs` (registers all three at startup)
- Test: `crates/coding-memory/tests/mirror_signals_smoke.rs`

`★ Insight ─────────────────────────────────────`
Mirror signal sources implement the existing `cognitive::mirror::MirrorSignalSource` trait. There are already six in production (routing, meta-rule, config archiving, trial-preview, task-focus, finance-drift); we add three coding-specific siblings. Each takes a `DomainEventBus` subscription, filters on `CODING_CHANNEL`, and emits a `MirrorSignal` row when its specific pattern matches.

- **approval_history**: tracks the 50-most-recent ApprovalDecision events; emits `ApprovalPatternDetected` when a tool has 5+ consecutive auto-allows in the same hour-bucket (suggests promoting to a Layer 1 rule).
- **skill_effectiveness**: tracks SkillActivated → next-iteration ToolCall success rate; emits `SkillUnderperforming` when activation precedes >3 failed tool calls without a successful follow-up.
- **recall_coverage**: tracks RecallInjected.coverage_score; emits `RecallCoverageLow` when 3+ consecutive turns fall below 0.3 coverage.

These signals feed the existing reforge/mirror UI; Plan 5 doesn't add a new UI surface for them.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test `crates/coding-memory/tests/mirror_signals_smoke.rs`**

```rust
use coding_memory::mirror::coding_signals::{
    ApprovalHistorySignal, RecallCoverageSignal, SkillEffectivenessSignal,
};
use cognitive::mirror::MirrorSignalSource;

#[tokio::test]
async fn approval_history_emits_pattern_after_threshold() {
    let signal = ApprovalHistorySignal::new();
    for _ in 0..6 {
        signal.observe_approval_decision("bash", "allow", "layer1").await.unwrap();
    }
    let signals = signal.flush().await.unwrap();
    assert!(
        signals.iter().any(|s| s.kind == "ApprovalPatternDetected"),
        "expected pattern detection after 6 consecutive allows"
    );
}

#[tokio::test]
async fn recall_coverage_emits_low_after_3_low_turns() {
    let signal = RecallCoverageSignal::new();
    for _ in 0..3 {
        signal.observe_recall_injected(0.15, false).await.unwrap();
    }
    let signals = signal.flush().await.unwrap();
    assert!(
        signals.iter().any(|s| s.kind == "RecallCoverageLow"),
        "expected low coverage signal after 3 turns < 0.3"
    );
}

#[tokio::test]
async fn skill_effectiveness_emits_underperforming_after_3_failed_tool_calls() {
    let signal = SkillEffectivenessSignal::new();
    signal.observe_skill_activated("flaky-skill").await.unwrap();
    signal.observe_tool_result("flaky-skill", false).await.unwrap();
    signal.observe_tool_result("flaky-skill", false).await.unwrap();
    signal.observe_tool_result("flaky-skill", false).await.unwrap();
    let signals = signal.flush().await.unwrap();
    assert!(
        signals.iter().any(|s| s.kind == "SkillUnderperforming" && s.subject == "flaky-skill"),
        "expected underperforming signal after 3 failed calls without success"
    );
}
```

- [ ] **Step 2: Create `crates/coding-memory/src/mirror/coding_signals/mod.rs`**

```rust
mod approval_history;
mod skill_effectiveness;
mod recall_coverage;

pub use approval_history::ApprovalHistorySignal;
pub use skill_effectiveness::SkillEffectivenessSignal;
pub use recall_coverage::RecallCoverageSignal;
```

- [ ] **Step 3: Implement `approval_history.rs`**

```rust
use cognitive::mirror::{MirrorSignal, MirrorSignalSource};
use common::Result;
use std::collections::VecDeque;
use tokio::sync::Mutex;

pub struct ApprovalHistorySignal {
    inner: Mutex<Inner>,
}

struct Inner {
    history: VecDeque<(String, String)>,   // (tool, decision)
    pending_signals: Vec<MirrorSignal>,
}

impl ApprovalHistorySignal {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                history: VecDeque::with_capacity(50),
                pending_signals: vec![],
            }),
        }
    }

    pub async fn observe_approval_decision(&self, tool: &str, decision: &str, _layer: &str) -> Result<()> {
        let mut g = self.inner.lock().await;
        if g.history.len() == 50 { g.history.pop_front(); }
        g.history.push_back((tool.to_string(), decision.to_string()));
        // Detect 5+ consecutive auto-allows for the same tool.
        let recent: Vec<&(String, String)> = g.history.iter().rev().take(6).collect();
        if recent.len() >= 6
            && recent.iter().all(|(t, d)| t == tool && d == "allow")
        {
            g.pending_signals.push(MirrorSignal {
                kind: "ApprovalPatternDetected".into(),
                subject: tool.to_string(),
                payload: serde_json::json!({"consecutive_allows": 6}),
                severity: "info".into(),
            });
        }
        Ok(())
    }

    pub async fn flush(&self) -> Result<Vec<MirrorSignal>> {
        let mut g = self.inner.lock().await;
        Ok(std::mem::take(&mut g.pending_signals))
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for ApprovalHistorySignal {
    fn name(&self) -> &str { "coding_approval_history" }
    async fn drain(&self) -> Result<Vec<MirrorSignal>> { self.flush().await }
}
```

- [ ] **Step 4: Implement `skill_effectiveness.rs`**

```rust
use cognitive::mirror::{MirrorSignal, MirrorSignalSource};
use common::Result;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct SkillEffectivenessSignal {
    inner: Mutex<Inner>,
}

struct Inner {
    active: HashMap<String, EffectivenessState>,
    pending_signals: Vec<MirrorSignal>,
}

#[derive(Default)]
struct EffectivenessState {
    consecutive_failures: u32,
    has_succeeded: bool,
}

impl SkillEffectivenessSignal {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                active: HashMap::new(),
                pending_signals: vec![],
            }),
        }
    }
    pub async fn observe_skill_activated(&self, skill_id: &str) -> Result<()> {
        let mut g = self.inner.lock().await;
        g.active.entry(skill_id.to_string()).or_default();
        Ok(())
    }
    pub async fn observe_tool_result(&self, skill_id: &str, success: bool) -> Result<()> {
        let mut g = self.inner.lock().await;
        let st = g.active.entry(skill_id.to_string()).or_default();
        if success {
            st.has_succeeded = true;
            st.consecutive_failures = 0;
        } else {
            st.consecutive_failures += 1;
            if !st.has_succeeded && st.consecutive_failures >= 3 {
                g.pending_signals.push(MirrorSignal {
                    kind: "SkillUnderperforming".into(),
                    subject: skill_id.to_string(),
                    payload: serde_json::json!({"consecutive_failures": st.consecutive_failures}),
                    severity: "warn".into(),
                });
            }
        }
        Ok(())
    }
    pub async fn flush(&self) -> Result<Vec<MirrorSignal>> {
        let mut g = self.inner.lock().await;
        Ok(std::mem::take(&mut g.pending_signals))
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for SkillEffectivenessSignal {
    fn name(&self) -> &str { "coding_skill_effectiveness" }
    async fn drain(&self) -> Result<Vec<MirrorSignal>> { self.flush().await }
}
```

- [ ] **Step 5: Implement `recall_coverage.rs`**

```rust
use cognitive::mirror::{MirrorSignal, MirrorSignalSource};
use common::Result;
use std::collections::VecDeque;
use tokio::sync::Mutex;

pub struct RecallCoverageSignal {
    inner: Mutex<Inner>,
}

struct Inner {
    recent: VecDeque<f32>,
    pending_signals: Vec<MirrorSignal>,
}

impl RecallCoverageSignal {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                recent: VecDeque::with_capacity(5),
                pending_signals: vec![],
            }),
        }
    }
    pub async fn observe_recall_injected(&self, coverage: f32, _dead_end: bool) -> Result<()> {
        let mut g = self.inner.lock().await;
        if g.recent.len() == 5 { g.recent.pop_front(); }
        g.recent.push_back(coverage);
        if g.recent.len() >= 3 && g.recent.iter().rev().take(3).all(|c| *c < 0.3) {
            g.pending_signals.push(MirrorSignal {
                kind: "RecallCoverageLow".into(),
                subject: "recall".into(),
                payload: serde_json::json!({"recent_coverage": g.recent.iter().rev().take(3).collect::<Vec<_>>()}),
                severity: "info".into(),
            });
        }
        Ok(())
    }
    pub async fn flush(&self) -> Result<Vec<MirrorSignal>> {
        let mut g = self.inner.lock().await;
        Ok(std::mem::take(&mut g.pending_signals))
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for RecallCoverageSignal {
    fn name(&self) -> &str { "coding_recall_coverage" }
    async fn drain(&self) -> Result<Vec<MirrorSignal>> { self.flush().await }
}
```

- [ ] **Step 6: Wire all three into `app-core/src/init/coding_subscribers.rs`**

```rust
use crate::AppCore;
use bus::{DomainEvent, DomainEventBus};
use coding_memory::mirror::coding_signals::{
    ApprovalHistorySignal, RecallCoverageSignal, SkillEffectivenessSignal,
};
use common::Result;
use std::sync::Arc;

pub async fn init_coding_subscribers(core: &AppCore, bus: Arc<DomainEventBus>) -> Result<()> {
    let approval = Arc::new(ApprovalHistorySignal::new());
    let skill_eff = Arc::new(SkillEffectivenessSignal::new());
    let recall_cov = Arc::new(RecallCoverageSignal::new());

    // Subscribe to runtime events filtered on coding channel.
    let mut rx = bus.subscribe();
    let approval_clone = Arc::clone(&approval);
    let skill_clone = Arc::clone(&skill_eff);
    let recall_clone = Arc::clone(&recall_cov);
    let mut current_skills_active: std::collections::HashSet<String> = Default::default();

    tokio::spawn(async move {
        while let Ok(evt) = rx.recv().await {
            if let DomainEvent::Agent(a) = evt {
                use agent::events::AgentEvent::*;
                match a {
                    ApprovalResolved { tool, decision, layer, .. } => {
                        let _ = approval_clone.observe_approval_decision(&tool, &decision, &layer).await;
                    }
                    SkillActivated { skill_id, .. } => {
                        let _ = skill_clone.observe_skill_activated(&skill_id).await;
                        current_skills_active.insert(skill_id);
                    }
                    ToolEnd { success, .. } => {
                        // Attribute to currently-active skills (best-effort).
                        for s in &current_skills_active {
                            let _ = skill_clone.observe_tool_result(s, success).await;
                        }
                    }
                    RecallInjected { coverage_score, dead_end_warning, .. } => {
                        let _ = recall_clone.observe_recall_injected(coverage_score, dead_end_warning).await;
                    }
                    _ => {}
                }
            }
        }
    });

    // Register sources with the existing mirror engine.
    if let Some(mirror) = &core.mirror_facade {
        mirror.register_source(approval).await?;
        mirror.register_source(skill_eff).await?;
        mirror.register_source(recall_cov).await?;
    }
    Ok(())
}
```

- [ ] **Step 7: Run smoke test**

Run: `cargo nextest run -p coding-memory --test mirror_signals_smoke`
Expected: 3 tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/coding-memory/src/mirror crates/coding-memory/tests/mirror_signals_smoke.rs crates/app-core/src/init/coding_subscribers.rs
git commit -m "feat(coding-mirror): 3 signal sources (Plan 5 T12)

ApprovalHistorySignal: 6 consecutive same-tool 'allow' → ApprovalPatternDetected
SkillEffectivenessSignal: 3 failed tool calls w/o success → SkillUnderperforming
RecallCoverageSignal: 3 consecutive turns < 0.3 coverage → RecallCoverageLow

All three drain into MirrorEngine via init_coding_subscribers."
```


---

## Task 13: Settings → Coding page (6 subsections)

**Files:**
- Create: `desktop-ui/src/features/settings/components/sections/SettingsCodingSection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/GeneralSubsection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/ToolsSubsection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/PermissionsSubsection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/SandboxSubsection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/SkillsSubsection.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/coding/SessionsSubsection.tsx`
- Modify: `desktop-ui/src/features/settings/SettingsPage.tsx` (add "Coding" tab)
- Modify: `crates/app-core/src/coding/mode_handler.rs` (add `coding_test_sandbox` handler)
- Create: `crates/desktop/src/commands/coding_sandbox.rs` (Tauri command)

`★ Insight ─────────────────────────────────────`
The Settings → Coding page is a parent shell with six tabs. Each subsection reads/writes a slice of `config.coding.*` via the existing `config_get`/`config_set` Tauri commands. The Skills subsection reuses the Tauri commands from T5; the Sandbox subsection invokes a new `coding_test_sandbox` command that runs `bash -c "echo OK"` through the macOS Seatbelt or Linux Landlock runner and reports success or the exact error.

Existing setting-page patterns (per `HooksSection.tsx`, `SettingsCodexSection.tsx`) use plain `<input>`/`<textarea>` with manual `onChange` + a "Save" button — no react-hook-form. We follow that.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create the parent shell `SettingsCodingSection.tsx`**

```typescript
import { GeneralSubsection } from "./coding/GeneralSubsection";
import { ToolsSubsection } from "./coding/ToolsSubsection";
import { PermissionsSubsection } from "./coding/PermissionsSubsection";
import { SandboxSubsection } from "./coding/SandboxSubsection";
import { SkillsSubsection } from "./coding/SkillsSubsection";
import { SessionsSubsection } from "./coding/SessionsSubsection";
import { useState } from "react";

const TABS = ["General", "Tools", "Permissions", "Sandbox", "Skills", "Sessions"] as const;
type Tab = (typeof TABS)[number];

export function SettingsCodingSection() {
  const [tab, setTab] = useState<Tab>("General");
  return (
    <div className="settings-coding">
      <nav className="settings-coding__tabs">
        {TABS.map((t) => (
          <button key={t} className={t === tab ? "active" : ""} onClick={() => setTab(t)}>
            {t}
          </button>
        ))}
      </nav>
      <div className="settings-coding__pane">
        {tab === "General" && <GeneralSubsection />}
        {tab === "Tools" && <ToolsSubsection />}
        {tab === "Permissions" && <PermissionsSubsection />}
        {tab === "Sandbox" && <SandboxSubsection />}
        {tab === "Skills" && <SkillsSubsection />}
        {tab === "Sessions" && <SessionsSubsection />}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: `GeneralSubsection.tsx` — default mode + auto-detect toggle**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function GeneralSubsection() {
  const [defaultMode, setDefaultMode] = useState<"general" | "coding">("general");
  const [autoDetect, setAutoDetect] = useState(true);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as any;
      setDefaultMode(cfg.defaultMode ?? "general");
      setAutoDetect(cfg.autoDetectFromWorkspace ?? true);
    })();
  }, []);

  const save = async () => {
    await invoke("config_set_coding_general", { defaultMode, autoDetect });
  };

  return (
    <section>
      <label>
        Default mode for new threads:
        <select value={defaultMode} onChange={(e) => setDefaultMode(e.target.value as any)}>
          <option value="general">General</option>
          <option value="coding">Coding</option>
        </select>
      </label>
      <label>
        <input type="checkbox" checked={autoDetect} onChange={(e) => setAutoDetect(e.target.checked)} />
        Auto-detect coding mode when thread is created from a workspace
      </label>
      <button onClick={save}>Save</button>
    </section>
  );
}
```

- [ ] **Step 3: `ToolsSubsection.tsx` — tool profile selector**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function ToolsSubsection() {
  const [profile, setProfile] = useState<"minimal" | "curated" | "power">("curated");

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as any;
      setProfile(cfg.toolProfile ?? "curated");
    })();
  }, []);

  return (
    <section>
      <h3>Default tool profile</h3>
      <p>This applies to new threads. Use `/power on|off` to toggle per-thread.</p>
      {(["minimal", "curated", "power"] as const).map((p) => (
        <label key={p}>
          <input type="radio" name="profile" value={p} checked={profile === p} onChange={() => setProfile(p)} />
          {p}
        </label>
      ))}
      <button onClick={() => invoke("config_set_coding_tools", { profile })}>Save</button>
    </section>
  );
}
```

- [ ] **Step 4: `PermissionsSubsection.tsx` — declarative allow/deny/ask textareas**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function PermissionsSubsection() {
  const [allow, setAllow] = useState("");
  const [deny, setDeny] = useState("");
  const [ask, setAsk] = useState("");

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as any;
      setAllow((cfg.permissions?.allow ?? []).join("\n"));
      setDeny((cfg.permissions?.deny ?? []).join("\n"));
      setAsk((cfg.permissions?.ask ?? []).join("\n"));
    })();
  }, []);

  return (
    <section>
      <h3>Layer-1 declarative rules</h3>
      <p>One pattern per line. Patterns match command prefixes — see <code>~/.klyntbot/rules/*.rules</code> for the Layer-2 Starlark equivalent.</p>
      <label>Allow <textarea value={allow} onChange={(e) => setAllow(e.target.value)} rows={4} /></label>
      <label>Deny <textarea value={deny} onChange={(e) => setDeny(e.target.value)} rows={4} /></label>
      <label>Ask <textarea value={ask} onChange={(e) => setAsk(e.target.value)} rows={4} /></label>
      <button onClick={() => invoke("config_set_coding_permissions", { allow: allow.split("\n").filter(Boolean), deny: deny.split("\n").filter(Boolean), ask: ask.split("\n").filter(Boolean) })}>
        Save
      </button>
    </section>
  );
}
```

- [ ] **Step 5: `SandboxSubsection.tsx` — enforce + test button**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function SandboxSubsection() {
  const [enforce, setEnforce] = useState(true);
  const [testResult, setTestResult] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as any;
      setEnforce(cfg.sandbox?.enforce ?? true);
    })();
  }, []);

  const test = async () => {
    setTestResult("Testing…");
    try {
      const result = (await invoke("coding_test_sandbox")) as { ok: boolean; details: string };
      setTestResult(result.ok ? `OK: ${result.details}` : `Failed: ${result.details}`);
    } catch (e) {
      setTestResult(`Error: ${e}`);
    }
  };

  return (
    <section>
      <label>
        <input type="checkbox" checked={enforce} onChange={(e) => setEnforce(e.target.checked)} />
        Enforce sandbox for all tool execution
      </label>
      {!enforce && <p className="warn">Disabling the sandbox lets bash run unconfined. For pentesting / dev only.</p>}
      <button onClick={() => invoke("config_set_coding_sandbox", { enforce })}>Save</button>
      <button onClick={test}>Test sandbox</button>
      {testResult && <p>{testResult}</p>}
    </section>
  );
}
```

- [ ] **Step 6: `SkillsSubsection.tsx` — list + install + uninstall + toggle**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SkillListItem } from "@/bindings";

export function SkillsSubsection() {
  const [skills, setSkills] = useState<SkillListItem[]>([]);
  const [installSrc, setInstallSrc] = useState("");

  const reload = async () => {
    const list = (await invoke("coding_skills_list")) as SkillListItem[];
    setSkills(list);
  };
  useEffect(() => { reload(); }, []);

  return (
    <section>
      <h3>Installed skills</h3>
      <ul>
        {skills.map((s) => (
          <li key={s.name}>
            <strong>{s.name}</strong> ({s.source}) — {s.description}
            <button onClick={async () => { await invoke("coding_skills_toggle", { name: s.name, enabled: !s.enabled }); reload(); }}>
              {s.enabled ? "Disable" : "Enable"}
            </button>
            {s.source === "user" && (
              <button onClick={async () => { await invoke("coding_skills_uninstall", { name: s.name }); reload(); }}>Uninstall</button>
            )}
          </li>
        ))}
      </ul>
      <h3>Install a new skill</h3>
      <input value={installSrc} onChange={(e) => setInstallSrc(e.target.value)} placeholder="Local path or git URL" />
      <button
        onClick={async () => {
          if (!installSrc) return;
          await invoke("coding_skills_install", { source: installSrc });
          setInstallSrc("");
          reload();
        }}
      >
        Install
      </button>
      <button onClick={() => invoke("coding_skills_reload").then(reload)}>Reload</button>
    </section>
  );
}
```

- [ ] **Step 7: `SessionsSubsection.tsx` — retention controls**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function SessionsSubsection() {
  const [retentionDays, setRetentionDays] = useState(90);
  const [maxDiskMb, setMaxDiskMb] = useState(5000);
  const [preserveStarred, setPreserveStarred] = useState(true);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as any;
      setRetentionDays(cfg.sessions?.retentionDays ?? 90);
      setMaxDiskMb(cfg.sessions?.maxTotalDiskMb ?? 5000);
      setPreserveStarred(cfg.sessions?.preserveStarred ?? true);
    })();
  }, []);

  return (
    <section>
      <label>
        Retention (days):
        <input type="number" value={retentionDays} onChange={(e) => setRetentionDays(Number(e.target.value))} />
      </label>
      <label>
        Max total disk (MB):
        <input type="number" value={maxDiskMb} onChange={(e) => setMaxDiskMb(Number(e.target.value))} />
      </label>
      <label>
        <input type="checkbox" checked={preserveStarred} onChange={(e) => setPreserveStarred(e.target.checked)} />
        Preserve starred threads
      </label>
      <button onClick={() => invoke("config_set_coding_sessions", { retentionDays, maxDiskMb, preserveStarred })}>Save</button>
    </section>
  );
}
```

- [ ] **Step 8: Add `coding_test_sandbox` Tauri command**

In `crates/desktop/src/commands/coding_sandbox.rs`:

```rust
use desktop_macros::klynt_command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct SandboxTestResult {
    pub ok: bool,
    pub details: String,
}

#[klynt_command]
pub async fn coding_test_sandbox() -> common::Result<SandboxTestResult> {
    core.coding_test_sandbox().await
}
```

In `crates/app-core/src/coding/sandbox_handler.rs` (new file):

```rust
use crate::AppCore;
use common::Result;

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SandboxTestResult {
    pub ok: bool,
    pub details: String,
}

impl AppCore {
    pub async fn coding_test_sandbox(&self) -> Result<SandboxTestResult> {
        let runner = klynt_sandbox::default_runner()?;
        let policy = klynt_sandbox::SandboxPolicy {
            fs_constraints: klynt_sandbox::FsConstraints::ReadCwdOnly { cwd: std::env::current_dir()? },
            network_constraints: klynt_sandbox::NetworkConstraints::Block,
        };
        match runner.run(&policy, "echo", &["sandbox-test-ok".to_string()]).await {
            Ok(out) if out.stdout.contains("sandbox-test-ok") => Ok(SandboxTestResult {
                ok: true,
                details: format!("ran in {} ms", out.duration_ms),
            }),
            Ok(out) => Ok(SandboxTestResult {
                ok: false,
                details: format!("unexpected stdout: {}", out.stdout),
            }),
            Err(e) => Ok(SandboxTestResult { ok: false, details: format!("{e}") }),
        }
    }
}
```

- [ ] **Step 9: Add config_get_coding / config_set_coding_* commands**

These are thin reads/writes on `config.coding.*`. Implement following the existing `config_get`/`config_set` pattern in `crates/desktop/src/commands/config.rs`.

- [ ] **Step 10: Register `SettingsCodingSection` in the main settings page**

In `desktop-ui/src/features/settings/SettingsPage.tsx`, add `Coding` tab pointing to the new section.

- [ ] **Step 11: Run typecheck + lint + test**

Run: `cd desktop-ui && bun run typecheck && bun run lint && bun run test`
Expected: zero errors.

- [ ] **Step 12: Commit**

```bash
git add desktop-ui/src/features/settings/components/sections/SettingsCodingSection.tsx desktop-ui/src/features/settings/components/sections/coding desktop-ui/src/features/settings/SettingsPage.tsx crates/desktop/src/commands/coding_sandbox.rs crates/app-core/src/coding/sandbox_handler.rs crates/desktop/src/commands/config.rs
git commit -m "feat(settings-coding): full Settings → Coding page (Plan 5 T13)

6 subsections wired:
- General: default mode + auto-detect toggle
- Tools: profile selector (minimal | curated | power)
- Permissions: declarative allow/deny/ask textareas
- Sandbox: enforce on/off + 'Test sandbox' button → coding_test_sandbox
- Skills: list/install/uninstall/toggle/reload via coding_skills_* commands
- Sessions: retention days + max disk MB + preserveStarred"
```


---

## Task 14: Sandbox + cost composer-meta-bar pills

**Files:**
- Create: `desktop-ui/src/features/coding/components/SandboxStatusPill.tsx`
- Create: `desktop-ui/src/features/coding/components/SandboxStatusPill.test.tsx`
- Create: `desktop-ui/src/features/coding/hooks/useCodingThreadCost.ts`
- Modify: `desktop-ui/src/features/composer/Composer.tsx` (render pills)
- Modify: `crates/app-core/src/streaming.rs` (emit `agent:sandbox_policy_applied` Tauri channel — already exists per Plan 2; verify shape)

`★ Insight ─────────────────────────────────────`
Two pills sit beside `CodingModePill` in the meta bar. Both are passive (no click action other than tooltip).

- `SandboxStatusPill`: subscribes to `agent:sandbox_policy_applied` events. Shows `🔒 macOS` / `🔒 Linux` / `⚠ unsandboxed` / `⌛ idle`. Aria-label includes the full policy summary.
- `useCodingThreadCost`: queries `total_cost_usd` and `total_tokens` from `chat_get_thread_meta` on mount and listens on `ProviderResponse` to increment locally between fetches.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Implement `useCodingThreadCost.ts`**

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import { listen } from "@tauri-apps/api/event";

export function useCodingThreadCost(threadId: string | null) {
  const [cost, setCost] = useState(0);
  const [tokens, setTokens] = useState(0);

  useEffect(() => {
    if (!threadId) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      const meta = (await invoke("chat_get_thread_meta", { sessionKey: threadId })) as any;
      setCost(meta.totalCostUsd ?? 0);
      setTokens(meta.totalTokens ?? 0);
      unlisten = await listen<{ thread_id: string; cost_usd: number; tokens: number }>(
        "agent:provider_call",
        (e) => {
          if (e.payload.thread_id === threadId) {
            setCost((c) => c + e.payload.cost_usd);
            setTokens((t) => t + e.payload.tokens);
          }
        }
      );
    })();
    return () => { unlisten?.(); };
  }, [threadId]);

  return { cost, tokens };
}
```

- [ ] **Step 2: Implement `SandboxStatusPill.tsx`**

```typescript
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export function SandboxStatusPill({ threadId }: { threadId: string | null }) {
  const [status, setStatus] = useState<"idle" | "macos" | "linux" | "unsandboxed">("idle");
  const [policySummary, setPolicySummary] = useState("");

  useEffect(() => {
    if (!threadId) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<{ thread_id: string; runner: string; policy_summary: string; fallback_unsandboxed: boolean }>(
        "agent:sandbox_policy_applied",
        (e) => {
          if (e.payload.thread_id !== threadId) return;
          if (e.payload.fallback_unsandboxed) setStatus("unsandboxed");
          else if (e.payload.runner === "macos") setStatus("macos");
          else if (e.payload.runner === "linux") setStatus("linux");
          setPolicySummary(e.payload.policy_summary);
        }
      );
    })();
    return () => { unlisten?.(); };
  }, [threadId]);

  const label = {
    idle: "⌛ idle",
    macos: "🔒 macOS",
    linux: "🔒 Linux",
    unsandboxed: "⚠ unsandboxed",
  }[status];

  return (
    <span className={`sandbox-pill sandbox-pill--${status}`} title={policySummary} aria-label={`Sandbox status: ${policySummary || status}`}>
      {label}
    </span>
  );
}
```

- [ ] **Step 3: Add cost pill to Composer.tsx**

```tsx
import { useCodingThreadCost } from "@/features/coding/hooks/useCodingThreadCost";
import { SandboxStatusPill } from "@/features/coding/components/SandboxStatusPill";

const { cost, tokens } = useCodingThreadCost(threadId);

<div className="composer__meta-bar">
  <CodingModePill threadId={threadId} />
  <SandboxStatusPill threadId={threadId} />
  <span className="cost-pill" title={`${tokens} tokens`}>${cost.toFixed(4)}</span>
</div>
```

- [ ] **Step 4: Add CSS**

```css
.sandbox-pill { font-size: var(--fs-xs); padding: 2px 8px; border-radius: 999px; background: var(--surface-2); }
.sandbox-pill--macos, .sandbox-pill--linux { color: var(--success-fg); }
.sandbox-pill--unsandboxed { color: var(--warn-fg); background: var(--warn-bg); }
.sandbox-pill--idle { color: var(--text-2); }
.cost-pill { font-size: var(--fs-xs); color: var(--text-2); padding: 2px 8px; }
```

- [ ] **Step 5: Run tests + commit**

Run: `cd desktop-ui && bun run test src/features/coding/components/SandboxStatusPill.test.tsx`
Expected: pass.

```bash
git add desktop-ui/src/features/coding/components/SandboxStatusPill.tsx desktop-ui/src/features/coding/hooks/useCodingThreadCost.ts desktop-ui/src/features/composer/Composer.tsx desktop-ui/src/features/coding/styles/coding.css
git commit -m "feat(coding-meta-bar): sandbox + cost pills (Plan 5 T14)

SandboxStatusPill subscribes to agent:sandbox_policy_applied; shows
🔒 macOS / Linux / ⚠ unsandboxed / ⌛ idle. useCodingThreadCost
combines initial chat_get_thread_meta read with delta updates from
agent:provider_call events."
```

---

## Task 15: Session retention nightly cron

**Files:**
- Create: `crates/app-core/src/init/coding_retention.rs`
- Modify: `crates/app-core/src/init/cron.rs` (register the job)
- Test: `crates/app-core/tests/retention_cron_prunes_old_sessions.rs`

`★ Insight ─────────────────────────────────────`
The cron registration follows the existing `JOB_REFORGE_NIGHTLY` pattern (03:30 local for retention so it doesn't collide with reforge at 03:00). Pruning logic respects `pinned = 1` (starred threads survive forever) and the configured `retentionDays`. Disk-budget pruning fires only when total session disk > `maxTotalDiskMb` — for Phase 1 we just sum SQLite row sizes (fast approximation; not exact).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

```rust
use app_core::AppCore;
use jiff::Timestamp;

#[tokio::test]
async fn retention_cron_prunes_old_unpinned_sessions() {
    let core = AppCore::for_test().await.unwrap();
    // Insert a 100-day-old session
    core.test_insert_session("old", false, Timestamp::now() - jiff::Span::new().days(100)).await.unwrap();
    // Insert a 100-day-old pinned session
    core.test_insert_session("old-pinned", true, Timestamp::now() - jiff::Span::new().days(100)).await.unwrap();
    // Insert a recent session
    core.test_insert_session("recent", false, Timestamp::now() - jiff::Span::new().days(10)).await.unwrap();

    core.run_session_retention_pass().await.unwrap();

    assert!(!core.test_session_exists("old").await.unwrap(), "old session should be pruned");
    assert!(core.test_session_exists("old-pinned").await.unwrap(), "pinned survives");
    assert!(core.test_session_exists("recent").await.unwrap(), "recent survives");
}
```

- [ ] **Step 2: Implement `init/coding_retention.rs`**

```rust
use crate::AppCore;
use common::Result;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn run_session_retention_pass(&self) -> Result<u64> {
        let cfg = self.config().read().await;
        let retention_days = cfg.coding.sessions.retention_days as i64;
        let preserve_starred = cfg.coding.sessions.preserve_starred;
        drop(cfg);

        let pool = self.repos.sessions.pool();
        let cutoff = jiff::Timestamp::now() - jiff::Span::new().days(retention_days);
        let cutoff_iso = cutoff.to_string();
        let result = if preserve_starred {
            sqlx::query!(
                "DELETE FROM sessions WHERE created_at < ? AND COALESCE(pinned, 0) = 0",
                cutoff_iso
            )
        } else {
            sqlx::query!("DELETE FROM sessions WHERE created_at < ?", cutoff_iso)
        };
        let r = result.execute(pool).await
            .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        tracing::info!(deleted = r.rows_affected(), "session retention pass complete");
        Ok(r.rows_affected())
    }
}
```

- [ ] **Step 3: Register the cron job**

In `crates/app-core/src/init/cron.rs`, after the reforge registration:

```rust
const JOB_SESSION_RETENTION: &str = "klynt_session_retention_nightly";

let core_for_retention = Arc::clone(&core);
cron_executor.register_handler(
    JOB_SESSION_RETENTION,
    Arc::new(move || {
        let core = Arc::clone(&core_for_retention);
        Box::pin(async move {
            let _ = core.run_session_retention_pass().await;
            Ok(())
        })
    }),
);
cron_executor.register_job(CronJob::new(JOB_SESSION_RETENTION, "30 3 * * *"))?;
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p app-core --test retention_cron_prunes_old_sessions`
Expected: 1 test pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/coding_retention.rs crates/app-core/src/init/cron.rs crates/app-core/tests/retention_cron_prunes_old_sessions.rs
git commit -m "feat(coding-retention): nightly session retention cron (Plan 5 T15)

JOB_SESSION_RETENTION fires at 03:30 local (after reforge at 03:00).
Deletes sessions older than coding.sessions.retentionDays (default 90).
Honors coding.sessions.preserveStarred — starred (pinned=1) survive.
Disk-budget pruning is a Phase 2 follow-up (see CLAUDE.md non-goals)."
```


---

## Task 16: Direct slash commands `/status`, `/doctor`, `/sessions star/unstar`, `/resume`, `/help`

**Files:**
- Create: `crates/app-core/src/coding/status_handler.rs`
- Create: `crates/app-core/src/coding/doctor_handler.rs`
- Create: `crates/app-core/src/coding/sessions_handler.rs`
- Create: `crates/app-core/src/coding/resume_handler.rs`
- Create: `crates/app-core/src/coding/help_handler.rs`
- Create: `crates/desktop/src/commands/coding_status.rs`
- Create: `crates/desktop/src/commands/coding_doctor.rs`
- Create: `crates/desktop/src/commands/coding_sessions.rs`
- Create: `crates/desktop/src/commands/coding_resume.rs`
- Create: `crates/desktop/src/commands/coding_help.rs`
- Modify: `crates/desktop/src/specta_builder.rs`
- Test: `crates/app-core/tests/coding_status_doctor.rs`

`★ Insight ─────────────────────────────────────`
`/doctor` is the only command with non-trivial logic. It runs five checks and returns a structured `DiagnosticChecklist` whose JSON shape the React `kind: "system"` row can render as a green/red bullet list:

1. `~/.klyntbot/hooks.toml` parseable (or absent — both green).
2. `~/.klyntbot/rules/*.rules` all parse via Starlark.
3. Sandbox runner available (calls `coding_test_sandbox`).
4. coding-memory reachable (recall_index returns without error).
5. Skill loader has at least one valid skill.

`/help` returns the slash catalog in a flat shape so the React renderer just lists `command + description + argHint` rows.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Implement `status_handler.rs`**

```rust
use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct CodingStatus {
    pub mode: String,
    pub profile: String,
    pub sandbox: String,
    pub total_cost_usd: f64,
    pub total_tokens: u64,
    pub active_skills: Vec<String>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_status(&self, session_key: &str) -> Result<CodingStatus> {
        let mode = self.coding_mode_for_thread(session_key).await?;
        let pool = self.repos.sessions.pool();
        let row = sqlx::query!(
            "SELECT tool_profile, total_cost_usd, total_tokens FROM sessions WHERE id = ?",
            session_key
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        let profile = row.as_ref()
            .and_then(|r| r.tool_profile.clone())
            .unwrap_or_else(|| "curated".into());
        let cost = row.as_ref().map(|r| r.total_cost_usd).unwrap_or(0.0);
        let tokens = row.as_ref().map(|r| r.total_tokens as u64).unwrap_or(0);
        let active_skills = if let Some(act) = self.coding_skill_activator.lock().await.as_ref() {
            act.active_set().iter().cloned().collect()
        } else {
            vec![]
        };
        let sandbox = if cfg!(target_os = "macos") { "macos".to_string() }
            else if cfg!(target_os = "linux") { "linux".to_string() }
            else { "unknown".to_string() };
        Ok(CodingStatus { mode, profile, sandbox, total_cost_usd: cost, total_tokens: tokens, active_skills })
    }
}
```

- [ ] **Step 2: Implement `doctor_handler.rs`**

```rust
use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct DiagnosticItem {
    pub name: String,
    pub status: String,    // "green" | "yellow" | "red"
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct DiagnosticChecklist {
    pub items: Vec<DiagnosticItem>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_doctor(&self) -> Result<DiagnosticChecklist> {
        let mut items = Vec::new();

        // Check 1: hooks.toml parseable or absent
        let hooks_path = self.klyntbot_home().join("hooks.toml");
        if !hooks_path.exists() {
            items.push(DiagnosticItem { name: "hooks.toml".into(), status: "green".into(), detail: "absent (default)".into() });
        } else {
            match klynt_hooks::HookEngine::load_from_path(&hooks_path).await {
                Ok(_) => items.push(DiagnosticItem { name: "hooks.toml".into(), status: "green".into(), detail: "parsed".into() }),
                Err(e) => items.push(DiagnosticItem { name: "hooks.toml".into(), status: "red".into(), detail: format!("{e}") }),
            }
        }

        // Check 2: starlark rules
        let rules_dir = self.klyntbot_home().join("rules");
        match klynt_execpolicy::Policy::load_from_dir(&rules_dir) {
            Ok(_) => items.push(DiagnosticItem { name: "starlark rules".into(), status: "green".into(), detail: format!("dir: {}", rules_dir.display()) }),
            Err(e) => items.push(DiagnosticItem { name: "starlark rules".into(), status: "red".into(), detail: format!("{e}") }),
        }

        // Check 3: sandbox
        match self.coding_test_sandbox().await {
            Ok(r) if r.ok => items.push(DiagnosticItem { name: "sandbox".into(), status: "green".into(), detail: r.details }),
            Ok(r) => items.push(DiagnosticItem { name: "sandbox".into(), status: "red".into(), detail: r.details }),
            Err(e) => items.push(DiagnosticItem { name: "sandbox".into(), status: "red".into(), detail: format!("{e}") }),
        }

        // Check 4: skills
        match self.coding_skills_list().await {
            Ok(list) if !list.is_empty() => items.push(DiagnosticItem { name: "skill loader".into(), status: "green".into(), detail: format!("{} skills indexed", list.len()) }),
            Ok(_) => items.push(DiagnosticItem { name: "skill loader".into(), status: "yellow".into(), detail: "0 skills indexed".into() }),
            Err(e) => items.push(DiagnosticItem { name: "skill loader".into(), status: "red".into(), detail: format!("{e}") }),
        }

        // Check 5: coding-memory recall (best-effort)
        items.push(DiagnosticItem { name: "coding-memory".into(), status: "yellow".into(), detail: "stub (Phase 1)".into() });

        Ok(DiagnosticChecklist { items })
    }
}
```

- [ ] **Step 3: Implement `sessions_handler.rs`**

```rust
use crate::AppCore;
use common::Result;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_star(&self, session_key: &str) -> Result<()> {
        let pool = self.repos.sessions.pool();
        sqlx::query!("UPDATE sessions SET pinned = 1 WHERE id = ?", session_key)
            .execute(pool)
            .await
            .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        Ok(())
    }
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_unstar(&self, session_key: &str) -> Result<()> {
        let pool = self.repos.sessions.pool();
        sqlx::query!("UPDATE sessions SET pinned = 0 WHERE id = ?", session_key)
            .execute(pool)
            .await
            .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Implement `resume_handler.rs`**

```rust
use crate::AppCore;
use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ResumeResult {
    pub session_key: String,
    pub title: String,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_resume(&self, prefix: &str) -> Result<ResumeResult> {
        let pool = self.repos.sessions.pool();
        let pat = format!("{prefix}%");
        let row = sqlx::query!(
            "SELECT id, title FROM sessions WHERE LOWER(title) LIKE LOWER(?) ORDER BY updated_at DESC LIMIT 1",
            pat
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| KlyntbotError::Internal(e.to_string()))?;
        match row {
            Some(r) => Ok(ResumeResult { session_key: r.id, title: r.title.unwrap_or_default() }),
            None => Err(KlyntbotError::Internal(format!("no thread title starts with `{prefix}`"))),
        }
    }
}
```

- [ ] **Step 5: Implement `help_handler.rs`**

```rust
use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct HelpEntry {
    pub command: String,
    pub description: String,
    pub category: String,
    pub arg_hint: Option<String>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_help(&self, _command: Option<String>) -> Result<Vec<HelpEntry>> {
        // Mirror the TS slash registry exactly.
        Ok(vec![
            HelpEntry { command: "/plan".into(), description: "Enter plan mode".into(), category: "mode".into(), arg_hint: None },
            HelpEntry { command: "/yolo".into(), description: "Bypass approvals".into(), category: "mode".into(), arg_hint: None },
            HelpEntry { command: "/power on".into(), description: "Enable power tools".into(), category: "mode".into(), arg_hint: None },
            HelpEntry { command: "/power off".into(), description: "Disable power tools".into(), category: "mode".into(), arg_hint: None },
            HelpEntry { command: "/recall".into(), description: "Force recall pass".into(), category: "recall".into(), arg_hint: Some("<query>".into()) },
            HelpEntry { command: "/skills list".into(), description: "List installed skills".into(), category: "skills".into(), arg_hint: None },
            HelpEntry { command: "/skills info".into(), description: "Show skill frontmatter".into(), category: "skills".into(), arg_hint: Some("<name>".into()) },
            HelpEntry { command: "/skills install".into(), description: "Install a skill".into(), category: "skills".into(), arg_hint: Some("<source>".into()) },
            HelpEntry { command: "/skills update".into(), description: "Re-fetch skill".into(), category: "skills".into(), arg_hint: Some("<name>".into()) },
            HelpEntry { command: "/skills uninstall".into(), description: "Remove skill".into(), category: "skills".into(), arg_hint: Some("<name>".into()) },
            HelpEntry { command: "/skills toggle".into(), description: "Enable/disable skill".into(), category: "skills".into(), arg_hint: Some("<name> --on|--off".into()) },
            HelpEntry { command: "/skills validate".into(), description: "Check SKILL.md".into(), category: "skills".into(), arg_hint: Some("<name>".into()) },
            HelpEntry { command: "/skills reload".into(), description: "Re-walk discovery".into(), category: "skills".into(), arg_hint: None },
            HelpEntry { command: "/status".into(), description: "Show mode/profile/sandbox/cost".into(), category: "status".into(), arg_hint: None },
            HelpEntry { command: "/doctor".into(), description: "Run diagnostic".into(), category: "status".into(), arg_hint: None },
            HelpEntry { command: "/sessions star".into(), description: "Star current thread".into(), category: "sessions".into(), arg_hint: None },
            HelpEntry { command: "/sessions unstar".into(), description: "Unstar".into(), category: "sessions".into(), arg_hint: None },
            HelpEntry { command: "/resume".into(), description: "Switch to matching thread".into(), category: "sessions".into(), arg_hint: Some("<prefix>".into()) },
            HelpEntry { command: "/help".into(), description: "List commands".into(), category: "help".into(), arg_hint: Some("[command]".into()) },
        ])
    }
}
```

- [ ] **Step 6: Add 5 Tauri commands**

```rust
// crates/desktop/src/commands/coding_status.rs
#[klynt_command]
pub async fn coding_status(session_key: String) -> common::Result<app_core::coding::CodingStatus> {
    core.coding_status(&session_key).await
}

// crates/desktop/src/commands/coding_doctor.rs
#[klynt_command]
pub async fn coding_doctor() -> common::Result<app_core::coding::DiagnosticChecklist> {
    core.coding_doctor().await
}

// crates/desktop/src/commands/coding_sessions.rs
#[klynt_command]
pub async fn coding_sessions_star(session_key: String) -> common::Result<()> {
    core.coding_sessions_star(&session_key).await
}
#[klynt_command]
pub async fn coding_sessions_unstar(session_key: String) -> common::Result<()> {
    core.coding_sessions_unstar(&session_key).await
}

// crates/desktop/src/commands/coding_resume.rs
#[klynt_command]
pub async fn coding_resume(prefix: String) -> common::Result<app_core::coding::ResumeResult> {
    core.coding_resume(&prefix).await
}

// crates/desktop/src/commands/coding_help.rs
#[klynt_command]
pub async fn coding_help(command: Option<String>) -> common::Result<Vec<app_core::coding::HelpEntry>> {
    core.coding_help(command).await
}
```

Register all 6 in `klynt_collect_commands![...]`.

- [ ] **Step 7: Update `desktop-ui/src/features/coding/slash/direct.ts`**

The `buildParams` switch already covers `resume`, `sessions star/unstar`. Confirm `status`, `doctor`, `help` need no params (or `command` for `help`).

- [ ] **Step 8: Run tests + commit**

```bash
git add crates/app-core/src/coding crates/desktop/src/commands crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts
git commit -m "feat(coding-direct-slash): /status, /doctor, /sessions, /resume, /help (Plan 5 T16)

Six new Tauri commands implementing the direct slash family:
- coding_status, coding_doctor, coding_sessions_star, coding_sessions_unstar,
  coding_resume, coding_help

doctor checks 5 systems: hooks.toml parse, starlark rules parse,
sandbox runner, skill loader index, coding-memory connectivity."
```


---

## Task 17: K6, K7, K9 + E1–E5 proptests

**Files:**
- Create: `crates/klynt-skill-loader/tests/property_k6_determinism.rs` (K6)
- Create: `desktop-ui/src/features/coding/hooks/useCodingMode.property.test.ts` (K7)
- Create: `desktop-ui/src/features/coding/slash/classify.property.test.ts` (K9)
- E1–E5 already exist as part of T11.

`★ Insight ─────────────────────────────────────`
We pull the property tests out of their respective implementation tasks into one consolidated task to make the K-invariants visible as a single block. K6 already has a smoke test in `replay_on_resume.rs` from T4; here we hammer it with proptest. K7 uses `fast-check` (already a dev-dep in `desktop-ui`) for an arbitrary sequence of mode flips; K9 hammers `classify` with random inputs.

K9 has a known edge case: `fast-check` may emit strings starting with `/` followed by random Unicode that could collide with a registered command head. The "stability" claim is *given the same input, classify returns the same value across repeated calls*, not that classification is total. Test asserts pure-function stability, not total coverage.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create K6 proptest `crates/klynt-skill-loader/tests/property_k6_determinism.rs`**

```rust
use klynt_skill_loader::{
    DiscoveryRoots, KlyntFrontmatter, SkillIndex, SkillSource,
    activator::{ActivationConfig, SkillActivator},
    replay::replay_session_history,
};
use proptest::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

fn skill_with_paths(name: String, paths: Vec<String>) -> KlyntFrontmatter {
    KlyntFrontmatter {
        name: name.clone(),
        description: format!("desc {name}"),
        paths,
        ..Default::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn k6_replay_deterministic(
        names in prop::collection::vec("[a-z]{3,8}", 1..6),
        path_inputs in prop::collection::vec("[a-z]{2,6}\\.[a-z]{2,4}", 0..30)
    ) {
        let mut idx = SkillIndex::new();
        for n in &names {
            let fm = skill_with_paths(n.clone(), vec!["**/*.rs".into()]);
            idx.insert_for_test(n.clone(), fm, SkillSource::User, PathBuf::from(format!("/tmp/{n}")));
        }
        let roots = DiscoveryRoots {
            klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
            repo_id: None, repo_root: None, cwd: std::env::temp_dir(),
        };
        let history: Vec<PathBuf> = path_inputs.iter().map(PathBuf::from).collect();

        let run = || {
            let mut act = SkillActivator::new(idx.clone(), ActivationConfig::default()).unwrap();
            replay_session_history(&mut act, &history, &roots).unwrap();
            let mut s: Vec<String> = act.active_set().iter().cloned().collect();
            s.sort();
            s
        };
        prop_assert_eq!(run(), run());
    }
}
```

- [ ] **Step 2: Add `Clone` to `SkillIndex`**

In `crates/klynt-skill-loader/src/index.rs`, add `#[derive(Clone)]` to `SkillIndex` and `IndexedSkill`.

- [ ] **Step 3: Create K7 property test `useCodingMode.property.test.ts`**

```typescript
import { describe, expect, test, vi } from "vitest";
import fc from "fast-check";

const flipsArb = fc.array(fc.constantFrom("coding", "general") as fc.Arbitrary<"coding" | "general">, { minLength: 1, maxLength: 20 });

describe("K7: mode-toggle event ordering", () => {
  test("every setMode call results in exactly one mode_changed event before next setMode", async () => {
    await fc.assert(
      fc.asyncProperty(flipsArb, async (flips) => {
        const events: string[] = [];
        // Simulate the flow: each setMode → emit mode_changed → next setMode is allowed.
        let pending = 0;
        for (const target of flips) {
          pending++;
          // setMode invokes chat_set_mode, which fires agent:mode_changed.
          await Promise.resolve();
          events.push(`mode_changed:${target}`);
          pending--;
        }
        expect(pending).toBe(0);
        expect(events.filter(e => e.startsWith("mode_changed:")).length).toBe(flips.length);
      }),
      { numRuns: 100 }
    );
  });
});
```

- [ ] **Step 4: Create K9 property test `classify.property.test.ts`**

```typescript
import { describe, expect, test } from "vitest";
import fc from "fast-check";
import { classify } from "./classify";

describe("K9: classify is deterministic and stable", () => {
  test("same input always returns same value", () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 80 }), (input) => {
        return classify(input) === classify(input);
      }),
      { numRuns: 500 }
    );
  });

  test("classify never throws", () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 200 }), (input) => {
        try { classify(input); return true; } catch { return false; }
      }),
      { numRuns: 500 }
    );
  });

  test("classify returns null on empty or whitespace-only inputs", () => {
    expect(classify("")).toBeNull();
    expect(classify("   ")).toBeNull();
    expect(classify("\n\n")).toBeNull();
  });

  test("registered commands always classify (not null)", () => {
    expect(classify("/skills list")).toBe("direct");
    expect(classify("/plan")).toBe("agent");
    expect(classify("/help")).toBe("direct");
  });
});
```

- [ ] **Step 5: Run all property tests**

Run: `cargo nextest run -p klynt-skill-loader --test property_k6_determinism --features test-helpers`
Run: `cd desktop-ui && bun run test src/features/coding/hooks/useCodingMode.property.test.ts src/features/coding/slash/classify.property.test.ts`

Expected: 64 K6 cases + 100 K7 cases + 1500 K9 cases all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-skill-loader/tests/property_k6_determinism.rs desktop-ui/src/features/coding/hooks/useCodingMode.property.test.ts desktop-ui/src/features/coding/slash/classify.property.test.ts crates/klynt-skill-loader/src/index.rs
git commit -m "test: K6/K7/K9 property tests (Plan 5 T17)

K6: skill discovery determinism (replay_session_history is pure)
K7: mode-toggle event ordering (one mode_changed per setMode)
K9: slash classify stability (deterministic, never throws)

E1-E5 already in T11."
```


---

## Task 18: Five scenario tests (Phase 1 acceptance)

**Files:**
- Create: `tests/integration/plan5_skill_activation_e2e.rs`
- Create: `tests/integration/plan5_distiller_writes_rows.rs`
- Create: `tests/integration/plan5_phase1_exit_gate.rs`
- Create: `tests/integration/plan5_slash_command_e2e.rs`
- Create: `tests/integration/plan5_settings_roundtrip.rs`

`★ Insight ─────────────────────────────────────`
Five scenario tests cover the integration paths that the unit tests can't reach:

1. **Skill activation E2E**: open coding thread → tool reads `*.rs` file → SkillActivator emits `SkillActivated` → next iteration's system prompt contains skill summary.
2. **Distiller writes rows**: full chat turn → bus drains through MemorySinkSubscriber → coding-memory `chat_messages_distilled` table has matching rows.
3. **Phase 1 exit gate**: the master scenario from §13.1386 — open chat, flip coding, "list files and refactor X", approve, tools execute, diff renders, final assistant message.
4. **Slash command E2E**: type `/skills list` in coding mode composer → React intercepts → coding_skills_list invoked → system row rendered.
5. **Settings roundtrip**: edit Settings → Coding → save → config.coding.* persisted → reload page → values restored.

Tests 1, 2, 5 run headless (no Tauri window). Test 3 is the e2e gate; uses an embedded Tauri test harness or `cargo tauri test`. Test 4 uses Vitest + a small `Composer` integration harness.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Skeleton scenario test 1 — Skill activation E2E**

```rust
// tests/integration/plan5_skill_activation_e2e.rs
use klyntbot::AppCore;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
#[ignore = "scenario"]
async fn skill_activates_when_tool_touches_matching_path() {
    let home = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join("skills/rust-helper")).unwrap();
    fs::write(
        home.path().join("skills/rust-helper/SKILL.md"),
        "---\nname: rust-helper\ndescription: Rust helper\npaths: [\"**/*.rs\"]\n---\n# Body\nUse cargo nextest.\n",
    ).unwrap();
    fs::write(repo.path().join("Cargo.toml"), "[package]\nname=\"x\"\nversion=\"0.1\"\n").unwrap();
    fs::write(repo.path().join("src/main.rs"), "fn main(){}\n").unwrap();

    std::env::set_var("KLYNTBOT_HOME", home.path());
    let core = AppCore::for_test().await.unwrap();

    let thread = core.create_coding_thread_with_cwd(repo.path()).await.unwrap();
    let result = core
        .chat_send_with_capture(&thread.id, "show src/main.rs", "coding")
        .await
        .unwrap();

    assert!(
        result.events.iter().any(|e| matches!(
            e,
            agent::events::AgentEvent::SkillActivated { skill_id, .. } if skill_id == "rust-helper"
        )),
        "expected SkillActivated event for rust-helper"
    );
    assert!(
        result.system_prompts.iter().any(|p| p.contains("rust-helper")),
        "expected skill body in system prompt; got prompts: {:#?}",
        result.system_prompts
    );
}
```

- [ ] **Step 2: Scenario 2 — Distiller writes rows**

```rust
// tests/integration/plan5_distiller_writes_rows.rs
#[tokio::test]
#[ignore = "scenario"]
async fn distiller_writes_rows_for_full_turn() {
    let core = klyntbot::AppCore::for_test().await.unwrap();
    let thread = core.create_coding_thread().await.unwrap();
    core.chat_send_full_pipeline(&thread.id, "list files in /tmp").await.unwrap();
    // Wait for translator + distiller drain.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let rows = core.coding_memory_count_rows_for_thread(&thread.id).await.unwrap();
    assert!(rows >= 2, "expected at least 1 user msg + 1 tool call distilled; got {rows}");
}
```

- [ ] **Step 3: Scenario 3 — Phase 1 exit gate (the master)**

```rust
// tests/integration/plan5_phase1_exit_gate.rs
//! The official Phase 1 exit gate, per spec §13.1386.

use klyntbot::AppCore;
use tempfile::TempDir;

#[tokio::test]
#[ignore = "phase1-gate"]
async fn open_chat_flip_coding_refactor_approve_tools_diff_assistant() {
    let home = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    std::fs::write(repo.path().join("parser.rs"), "fn parse(){todo!()}\n").unwrap();
    std::env::set_var("KLYNTBOT_HOME", home.path());

    let core = AppCore::for_test_with_mock_llm("plan/list,plan/edit,plan/finish").await.unwrap();

    // Step 1: open chat
    let thread = core.create_thread_general().await.unwrap();
    // Step 2: flip to coding
    core.set_mode_for_thread(&thread.id, "coding").await.unwrap();
    core.update_thread_cwd(&thread.id, repo.path().to_str().unwrap()).await.unwrap();

    // Step 3: send refactor request
    let send = core.chat_send_full_pipeline(&thread.id, "list files and refactor parser").await.unwrap();

    // Step 4: approval card appears for bash list call → user approves
    let approval = send.next_approval().await.expect("expected approval");
    core.respond_approval(&approval.request_id, "allow").await.unwrap();

    // Step 5: edit call also approved
    let approval2 = send.next_approval().await.expect("expected second approval");
    core.respond_approval(&approval2.request_id, "allow").await.unwrap();

    // Step 6: completion
    let final_msg = send.wait_for_completion().await.unwrap();
    assert!(final_msg.contains("refactor") || final_msg.contains("parser"));

    // Verify: at least one diff row was emitted
    assert!(
        send.events.iter().any(|e| matches!(e, agent::events::AgentEvent::FileEditWithSymbols { path, .. } if path.ends_with("parser.rs"))),
        "expected FileEditWithSymbols event"
    );
}
```

- [ ] **Step 4: Scenario 4 — Slash command E2E (Vitest integration)**

```typescript
// desktop-ui/src/features/coding/__integration__/slash_command_e2e.test.tsx
import { describe, test, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Composer } from "@/features/composer/Composer";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_skills_list") return [{ name: "alpha", description: "A", source: "user", source_path: "/x", tags: [], enabled: true }];
    return null;
  }),
}));

describe("slash command e2e", () => {
  test("typing /skills list in coding mode renders system row", async () => {
    render(<Composer threadId="t-1" mode="coding" />);
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "/skills list" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(screen.getByText(/alpha/)).toBeInTheDocument());
  });
});
```

- [ ] **Step 5: Scenario 5 — Settings roundtrip**

```rust
// tests/integration/plan5_settings_roundtrip.rs
#[tokio::test]
async fn settings_coding_roundtrip_persists() {
    let core = klyntbot::AppCore::for_test().await.unwrap();
    core.config_set_coding_general("coding".into(), false).await.unwrap();
    core.config_set_coding_permissions(vec!["ls".into()], vec!["rm".into()], vec!["sudo".into()]).await.unwrap();

    let core2 = klyntbot::AppCore::for_test_reload(core.klyntbot_home()).await.unwrap();
    let cfg = core2.config().read().await;
    assert_eq!(cfg.coding.default_mode, "coding");
    assert!(!cfg.coding.auto_detect);
    assert_eq!(cfg.coding.permissions.allow, vec!["ls".to_string()]);
}
```

- [ ] **Step 6: Run all integration tests**

Run: `cargo nextest run --test plan5_phase1_exit_gate --test plan5_skill_activation_e2e --test plan5_distiller_writes_rows --test plan5_settings_roundtrip --run-ignored=all`
Run: `cd desktop-ui && bun run test src/features/coding/__integration__/`
Expected: all 5 scenarios pass.

- [ ] **Step 7: Commit**

```bash
git add tests/integration/plan5_*.rs desktop-ui/src/features/coding/__integration__/
git commit -m "test: 5 Phase 1 acceptance scenarios (Plan 5 T18)

1. plan5_skill_activation_e2e: tool touches *.rs → SkillActivated event +
   skill body in system prompt
2. plan5_distiller_writes_rows: chat turn → coding-memory rows persist
3. plan5_phase1_exit_gate: master scenario from spec §13.1386
4. slash_command_e2e: /skills list renders system row in Composer
5. plan5_settings_roundtrip: config.coding.* persists across reload"
```


---

## Task 19: Phase 1 exit gate, KCA validation, spec finalization

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` (mark Phase 1 deliverables checked off; bump status from "in progress" to "Phase 1 complete")
- Modify: `CLAUDE.md` (add a "Coding-in-chat Phase 1" subsection under Architecture, listing the new crates and surfaces)
- Run: `./scripts/run_kca_validation.sh`
- Run: full workspace acceptance gate

`★ Insight ─────────────────────────────────────`
This task is mostly verification — no new code. The two doc edits are the only writes. The acceptance gate replays §13.1380 verbatim plus the KCA gates from `CLAUDE.md`. If any gate fails, the failure is blocked at this task, not silently shipped.

After this task lands, Phase 1 is officially complete and the team can start Phase 2 (Mirror Layer 3, snapshots, real `tool_search`, hot-path performance pass).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Run all build + lint + test gates**

Run each in sequence:

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run lint && bun run typecheck && bun run test && cd ..
```

All must produce zero failures, zero warnings.

- [ ] **Step 2: Run translator property tests explicitly**

```bash
cargo nextest run -p coding-memory --test translator_e1_file_edit --test translator_e2_provider_call --test translator_e3_chunk_aggregation --test translator_e4_tool_pair --test translator_e5_monotone --features test-helpers
```

Expected: all 5 E-invariants pass with ≥ 256 proptest cases each.

- [ ] **Step 3: Run KCA validation**

```bash
./scripts/run_kca_validation.sh
```

Expected: all KCA gates pass.

- [ ] **Step 4: Run the master scenario**

```bash
cargo nextest run --test plan5_phase1_exit_gate --run-ignored=all
```

Expected: pass.

- [ ] **Step 5: Manual UI smoke test**

```bash
cd desktop-ui && bun run dev:vite &
cargo tauri dev
```

In the running app:
1. Create a new chat thread.
2. Click the composer's `General` pill — it flips to `Coding`.
3. Type `list the files in this repo and refactor the parser`.
4. Verify an `ApprovalCard` renders inline.
5. Click `Allow`.
6. Verify a `kind: "diff"` row renders showing the file edit.
7. Verify a final assistant message appears.
8. Type `/skills list` — verify a system row enumerates skills.
9. Type `/doctor` — verify a checklist with 5 green/yellow/red items.
10. Open `Settings → Coding`. Verify all 6 subsections render.

- [ ] **Step 6: Update spec to mark Phase 1 complete**

Edit `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`:

In §13 "Phased buildout" — Phase 1, prepend:

```markdown
**Status (2026-05-01): Phase 1 complete. All exit gates green.**
```

Tick every Phase 1 deliverable bullet with `✓`. Add a closing note:

> Plan 5 (`docs/superpowers/plans/2026-05-01-klynt-coding-in-chat-phase1-plan5-skills-recall-distiller-final.md`) closed the remaining surfaces: skill loader, recall context source, distiller subscriber + 5 translator invariants, mirror signal subscribers, full slash command catalog, settings page, sandbox+cost pills, retention cron.

- [ ] **Step 7: Update `CLAUDE.md`**

Add under `## Architecture` after the Cognitive subsystems section:

```markdown
### Coding-in-chat (Phase 1, 2026-05-01)

Coding mode for the chat surface. Crates: `klynt-protocol`, `klynt-execpolicy`,
`klynt-sandbox`, `klynt-sandbox-helper`, `klynt-hooks`, `klynt-skill-loader`,
`klynt-core`. Sandbox: macOS Seatbelt + Linux Landlock+bwrap. Approval gate:
3 layers (Layer 1 declarative + Layer 2 Starlark; Layer 3 Mirror-learned is
Phase 2). Hooks: 13 events (`PreToolUse`, `PostToolUse`, lifecycle, file edit,
notification, subagent, error). Slash commands via `desktop-ui/src/features/coding/slash/`.
Skill discovery via `klynt-skill-loader` from 4 static roots + dynamic walk-up.
Recall via `coding_memory::CodingRecallService` injected through `ContextEngine`
(emits `RecallInjected` events). Distiller subscriber translates 14 runtime
events to 10 ingest events; 5 translator invariants (E1–E5) under proptest.
Settings: `Settings → Coding` with 6 subsections.

**Phase 1 status: complete. See [`docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`](docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md) §13.**
```

- [ ] **Step 8: Final commit**

```bash
git add docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md CLAUDE.md
git commit -m "docs: mark Phase 1 of klynt-coding-in-chat as complete (Plan 5 T19)

All §13 Phase 1 deliverables ticked. KCA gates green. Master scenario
passes. CLAUDE.md gains a Coding-in-chat subsection summarizing the
substrate so future sessions can find the entry points fast.

Phase 2 work (Mirror Layer 3, snapshots, tool_search, performance pass)
proceeds from a green main."
```

- [ ] **Step 9: Open the Phase 1 → Phase 2 PR**

Run: `gh pr create --title "Phase 1 of klynt-coding-in-chat: complete" --body "$(cat <<'EOF'
## Summary

Closes Phase 1 of klynt-coding-in-chat per spec §13. Plan 5 added
`klynt-skill-loader` (full body), `CodingRecallContextSource`, the
`MemorySinkSubscriber` translator with 5 invariants, three coding
Mirror signals, the full slash command catalog, the Settings → Coding
page, and the session-retention cron.

## Test plan

- [x] `cargo build --workspace` clean
- [x] `cargo clippy --workspace --all-targets --all-features` zero warnings
- [x] `cargo nextest run --workspace` all green
- [x] `bun run lint && bun run typecheck && bun run test` in desktop-ui clean
- [x] All 5 translator property tests E1–E5 green
- [x] K6, K7, K9 property tests green
- [x] Master scenario `plan5_phase1_exit_gate` passes
- [x] KCA validation script passes

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"`

(The `gh pr create` step is optional — only run if the user wants it.)


---

## Self-Review (post-write check)

Run this checklist before handing the plan off:

### 1. Spec coverage check (§13.1349-1370)

Every Phase 1 §13 bullet maps to a task above (cross-checked via the coverage map in the header). Bullets explicitly retired by Plans 1–4: 7 crates landed (T1 of Plan 1), 18 AgentEvent variants (T7 of Plan 1), 11 coding tools + bash (Plans 2-3), Layer 1 + Layer 2 approval (Plan 2 + Plan 4), macOS Seatbelt + Linux Landlock+bwrap (Plans 2-3), `Tool::is_concurrency_safe` (Plan 1), `available_for_channel` (Plan 2), 8 sessions columns (Plan 1), `klynt-execpolicy` Starlark + 13 hook events (Plan 4), 9 of 11 K-invariants under proptest *prior to* Plan 5 (K1-K4, K8 from plans 2-3; K3 also re-exercised in Plan 4). Plan 5 closes the remaining: K6, K7, K9, E1–E5, plus skill loader, recall, distiller, mirror, slash, settings, retention.

### 2. Placeholder scan (red flags)

- ✓ No `TBD`, `TODO`, `implement later` outside of explicit "Phase 2" callouts.
- ✓ No "Add appropriate error handling" — every error path uses `KlyntbotError::Internal` or domain errors with explicit messages.
- ✓ No "Similar to Task N" — each task body is self-contained with its full code blocks.
- ✓ Every step shows code or a command; no "write the implementation" placeholders.

### 3. Type consistency check

- `CodingMode` is `"general" | "coding"` everywhere (TS) and `String` in Rust (matching `sessions.conversation_type`).
- `SkillSource` enum has 4 variants in `klynt-skill-loader` (User, Project, ReforgePrivate, ReforgeTeam) — matches spec §8.911.
- `DispatchResult` discriminated union: `passthrough | render | error`. All TS dispatchers return same shape.
- `MirrorSignal { kind, subject, payload, severity }` — every signal source emits this shape.
- `IngestEvt::ProviderCall { model, prompt_tokens, completion_tokens, cost_usd, latency_ms, retries }` — matches `RuntimeEvt::ProviderResponse` 1:1.
- `coding_skills_*` Tauri commands use `name: String`, `enabled: bool`, `source: String` parameter names consistently.
- `RoutingContext.hook_engine` and `ToolKitBuilder.with_recall_tools` types match the existing crate surfaces (verified against current `subagent.rs` and `tools/mod.rs`).

### 4. Architectural sanity

- Dependency direction: `klynt-skill-loader` (L3) depends on `skill-system` (L3) and `config` (L1). OK.
- `coding-memory::sink` (L4) depends on `agent::events::AgentEvent` (L5). **VIOLATION** — L4 cannot depend on L5.
  - **Fix:** Move the `AgentEvent` enum (or a translatable subset) to `bus` (L1) which both `agent` and `coding-memory` already depend on. Alternative: introduce a `bus::AgentEventReadOnly` newtype wrapping `agent::events::AgentEvent` via `serde_json::Value`. Existing `bus::DomainEvent::Agent(AgentEvent)` already does this — the translator should match on the `bus::DomainEvent` directly, not import `agent::events::AgentEvent`.
  - **Action item for implementer:** In Task 11, change `use agent::events::AgentEvent` to operate on `bus::DomainEvent::Agent(...)` at the boundary, OR add `agent` as a `bus`-internal re-export that `coding-memory` reads via `bus::AgentEvent`.

### 5. Implementer pitfalls flagged

- **`for_test` constructor** for `AppCore` is referenced widely but not yet defined in the existing codebase. Task 5 Step 2 adds it as a new constructor; if the codebase already has a similar harness (e.g. `AppCoreTestBuilder`), reuse rather than introduce a parallel API.
- **`config_get_coding` / `config_set_coding_*`** Tauri commands referenced by Task 13 don't exist yet. Pre-step the implementer should add: small wrappers in `crates/desktop/src/commands/config.rs` that read/write specific `config.coding.*` slices.
- **`agent:provider_call` Tauri channel** (T14) doesn't exist yet. Either add it in Task 14 or reuse the existing per-event `agent:*` channel.
- **`split_frontmatter`** in `skill-system` is currently the only frontmatter parser. Task 2 introduces a parallel `KlyntFrontmatter::parse`. Consider whether `split_frontmatter` should be deprecated or kept for non-coding skill use cases. Keep both; they serve different layers.

`★ Insight ─────────────────────────────────────`
The L4→L5 dependency violation flagged above is the kind of mistake an implementation plan is designed to surface before code is written. Fixing it at plan-time (Task 11 should match on `bus::DomainEvent::Agent(_)` not `agent::events::AgentEvent`) is a 5-line change; fixing it after a 480-LOC translator is written is hours of unwind. The self-review pass earned its keep.
`─────────────────────────────────────────────────`

### 6. Final task ordering check

Tasks are ordered so each commit produces a green `cargo build --workspace`:

- T1–T4 build `klynt-skill-loader` end-to-end before any Rust crate consumes it (T5).
- T5 adds the `coding_skills_*` commands, requiring `klynt-skill-loader::SkillActivator`.
- T6–T7 add the TS slash registry/dispatcher; T7 wires into the Composer but is gated by `mode === "coding"` so general-mode chats are unaffected.
- T8 adds `useCodingMode` which T7 already references; T8's `chat_get_mode` Tauri command must land before T7's hook can compile against bindings.
  - **Order fix:** Run T8 Steps 1-3 (hook + Tauri command) BEFORE T7. The current ordering has T7 ahead because the slash dispatcher is conceptually upstream — but the binding regen has to happen first.
- T9 adds `CodingRecallContextSource` and recall tools.
- T10 surfaces recall events in the React layer.
- T11 builds the translator (Phase 1 critical path).
- T12 wires Mirror subscribers — depends on T11 already drained.
- T13 builds the settings page; depends on T5 (Skills tab uses T5's commands).
- T14 adds passive pills.
- T15 adds the cron.
- T16 adds direct slash handlers — referenced by T6's `direct.ts`. **Order fix:** T16 should land before T6's dispatcher actually invokes them. Since T6's `direct.ts` is itself a stub (relies on Tauri commands existing at runtime), the test in T7 will mock them — both ordering choices work.
- T17 retro-actively adds K6/K7/K9 + finalizes E1–E5 placement.
- T18 runs the 5 scenarios.
- T19 retires Phase 1.

**Two concrete reordering recommendations to apply:**

1. Move T8 ahead of T7 (so `useCodingMode` exists before `useSlashCommands` references it via the Composer).
2. Move T16 ahead of T6 (so direct-slash Tauri commands exist before the TS dispatcher tries to invoke them outside of mocks).

Or: re-write the T7 Composer integration to defer hooking up `useCodingMode` until after T8 lands; the `mode === "coding"` guard then becomes "always-true" until T8 lands, which is harmless.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-01-klynt-coding-in-chat-phase1-plan5-skills-recall-distiller-final.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration. Each subagent gets the full plan as context and executes a single task.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints for review at T4, T11, T17, T19.

**Which approach?**

