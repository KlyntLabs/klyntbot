# Skills Marketplace — Design

**Status:** Design approved — ready for planning
**Date:** 2026-04-14
**Companion docs:** builds on `crates/skill-system/` (existing `SkillStore`), `crates/entity-store/src/templates.rs` (existing database-template installer), and `skills/workspace/SKILL.md` (already demonstrates the `klyntbot:` frontmatter extensions).

## Context

Klynt already speaks the Agent Skills format (SKILL.md + YAML frontmatter) and ships six bundled skills. Users cannot currently install, upgrade, remove, or discover third-party skills — every skill arrives pre-baked in the binary.

We want a proper marketplace so users can:
- Browse available skills from our curated set, the skills.sh community index, or a direct GitHub repo URL
- Install / uninstall / upgrade skills with versioning and a visual diff between versions
- Have "Klynt-native" skills install not just prompt text but also the databases, templates, and agent hooks they declare (per the `klyntbot:` frontmatter block)
- Convert generic prompt-only skills from skills.sh into Klynt-native ones via an LLM-driven adapter, keeping us compatible with the broader ecosystem without manual porting

The long-term goal is a community-contributed registry hosted at our own domain. This design is **local-first**: the registry is just one more source type in a system that works fully offline against GitHub and local paths today. Registry support drops in later with zero client rework.

## Decisions (from brainstorming)

| # | Decision |
|---|---|
| 1 | Browse tab is hybrid — our curated list + live skills.sh index + paste-URL input |
| 2 | Install flow is preview-then-consent: `installer.preview()` returns an `InstallPlan`; user confirms before any write |
| 3 | Versioning uses both semver (frontmatter) and git SHA (reproducibility). Upgrade path shows a full diff timeline across all intermediate versions |
| 4 | Dedicated top-level route `/skills` with list + detail pages (skills.sh-style leaderboard layout) |
| 5 | Uninstall offers three modes: *skill only*, *skill + archive databases*, *skill + delete databases* |
| 6 | `skills-adapter` crate uses the cognitive LLM provider to transform prompt-only skills into Klynt-native ones; result goes through normal install preview for user review |

---

## Architecture

Three new Rust crates plus UI. Layer assignments follow existing workspace conventions (`crates/common` is L0).

### `skills-registry` (L3)

Source resolution and fetching. One input: a `SkillSource`. One output: a `SkillPackage`.

```rust
pub enum SkillSource {
    GitHub { owner: String, repo: String, subpath: String, ref_: GitRef },
    SkillsDotSh { slug: String },  // resolves to a GitHub source at fetch time (see note below)
    LocalPath(PathBuf),
    CuratedBuiltin { name: String },
}

pub enum GitRef {
    Latest,
    Tag(String),       // e.g. "v1.0.3"
    Commit(String),    // git SHA
}

pub struct SkillPackage {
    pub name: String,
    pub source: SkillSource,
    pub resolved_sha: String,           // canonical identity
    pub semver: Option<String>,         // from frontmatter.metadata.klyntbot.version
    pub skill_md_content: String,       // raw file contents
    pub frontmatter: SkillFrontmatter,  // parsed
    pub klyntbot_meta: Option<KlyntbotMeta>,  // None = prompt-only skill
    pub references: Vec<(PathBuf, Vec<u8>)>,  // referenced files
    pub templates: Vec<(String, Value)>,       // parsed JSON template manifests
}
```

Caches fetched archives under `~/.klyntbot-dev/skills_cache/{sha}/` to avoid re-downloading across preview / install / diff operations within a session.

**skills.sh integration detail:** the skills.sh leaderboard slugs (e.g. `anthropics/skills/frontend-design`) follow the GitHub `owner/repo/subpath` pattern. During implementation we inspect the public skills.sh endpoints to see if they expose a JSON index we can proxy for Browse. If they do, `SkillSource::SkillsDotSh { slug }` resolves through that endpoint to a GitHub ref + install count metadata. If they don't, we fall back to treating the slug as a direct GitHub source and omit install-count metadata from the right rail. Either way the install mechanics are identical.

### `skills-installer` (L4)

Lifecycle operations: preview, apply, check-updates, diff, upgrade, uninstall.

Key entry points:

- `preview_install(source, version) → InstallPlan` — pure read
- `apply_install(plan) → Result<InstallResult>` — transactional, all-or-nothing
- `check_updates(name) → Vec<AvailableVersion>`
- `diff_versions(name, from_sha, to_sha) → DiffResult`
- `preview_upgrade(name, target_sha) → UpgradePlan`
- `apply_upgrade(plan) → Result<UpgradeResult>`
- `uninstall(name, mode) → Result<UninstallResult>`

`InstallPlan` is the consent document shown to the user:

```rust
pub struct InstallPlan {
    pub package: SkillPackage,
    pub files_to_write: Vec<PathBuf>,
    pub databases_to_bootstrap: Vec<TemplatePreview>,
    pub tools_to_register: Vec<String>,
    pub warnings: Vec<InstallWarning>,   // e.g. unsupported field types, deprecated API
}
```

Transactional guarantees: on any mid-apply failure, all files written so far are deleted and any databases created are dropped before returning the error. No partial installs.

### `skills-adapter` (L4)

Transforms a prompt-only `SkillPackage` into a Klynt-native one via the cognitive provider.

Single entry point:

```rust
pub async fn adapt(
    pkg: &SkillPackage,
    ctx: AdaptationContext,
) -> Result<AdaptedSkill>;

pub struct AdaptationContext {
    pub supported_field_types: Vec<FieldType>,    // our 16 types
    pub existing_databases: Vec<DatabaseSchema>,  // so adapter can suggest linking
    pub provider: Arc<dyn LlmProvider>,           // cognitive_provider
}

pub struct AdaptedSkill {
    pub adapted_skill_md: String,       // new SKILL.md with klyntbot: block
    pub generated_templates: Vec<(String, Value)>,
    pub rationale: String,
    pub adapter_model: String,          // "claude-opus-4-6 @ 2026-04-14" for cache invalidation
}
```

The prompt contract lives at `crates/skills-adapter/prompts/adapt.md` and injects:
- The 16 supported field types as authoritative schema
- 3 bundled skills as few-shot examples of well-formed `klyntbot:` blocks
- The user's current databases so the adapter can link rather than duplicate

Output is schema-validated — LLM-invented field types or unsupported frontmatter keys are rejected, forcing the adapter to retry or return `AdaptationFailed`. Results cache to the `adapted_skills` table keyed by `hash(source_ref + resolved_sha)`; re-installing the same skill reuses the cached adaptation.

Fallback: if no cognitive_provider is configured or the call fails, the "Adapt for Klynt" button is disabled with an explanatory tooltip. Install-as-prompt-only always remains available.

### Frontend

New top-level route `/skills` (list) + `/skills/:source` (detail). Leaderboard-style list, skills.sh-style detail page (see UI section).

Tauri commands thinly wrap the installer:
- `skill_list` — returns installed + curated list (paginated)
- `skill_browse` — proxy for skills.sh + curated index with search
- `skill_detail` — detail view for one skill (installed metadata + upstream metadata)
- `skill_install_preview` / `skill_install_apply`
- `skill_check_updates`
- `skill_diff_versions`
- `skill_upgrade_preview` / `skill_upgrade_apply`
- `skill_uninstall`
- `skill_adapt_preview` — runs the adapter on a prompt-only source, returns the adapted `SkillPackage` for review
- `skill_toggle_enabled`

Every command has a matching `dispatch_dev` entry (enforced by the existing `dev_server_covers_all_tauri_commands` test).

---

## Data model

Two new tables in a fresh `skills_marketplace` feature migration.

```sql
CREATE TABLE installed_skills (
  name                   TEXT PRIMARY KEY,
  source_type            TEXT NOT NULL,       -- 'github' | 'skills_sh' | 'local' | 'bundled'
  source_ref             TEXT NOT NULL,       -- 'owner/repo/subpath' | '/abs/path' | 'bundled'
  installed_version      TEXT NOT NULL,       -- semver from frontmatter
  installed_sha          TEXT NOT NULL,       -- git commit SHA (or 'bundled-{build}')
  enabled                INTEGER NOT NULL DEFAULT 1,
  is_adapted             INTEGER NOT NULL DEFAULT 0,
  bootstrapped_databases TEXT,                -- JSON array of database IDs this install created
  installed_at           TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);
CREATE INDEX idx_installed_skills_source ON installed_skills(source_type, source_ref);

CREATE TABLE adapted_skills (
  cache_key              TEXT PRIMARY KEY,    -- hash(source_ref + resolved_sha)
  adapted_skill_md       TEXT NOT NULL,
  generated_templates    TEXT NOT NULL,       -- JSON array of {name, manifest_json}
  rationale              TEXT NOT NULL,
  adapter_model          TEXT NOT NULL,
  created_at             TEXT NOT NULL
);
```

Bundled skills (the six we ship) are inserted on first boot with `source_type='bundled'`, `source_ref='bundled'`, `installed_sha='bundled-{build_hash}'`. They cannot be uninstalled but can be disabled.

### Version identity

Two fields, two jobs:
- **`installed_version`** (semver) — what humans read in the UI and upgrade dialogs
- **`installed_sha`** (git commit SHA) — what the diff view and update detection use; always present even when semver is missing

`check_updates` runs `git ls-remote` on the source repo, returns refs newer than `installed_sha`, sorted by commit date. Each update entry carries both the SHA and any associated tag/semver.

### Diff view

For each pair of adjacent SHAs in the update list, the diff view:
- Fetches SKILL.md at each SHA (cached in the session archive cache)
- **Frontmatter diff** — field-by-field table with add/remove/change status
- **Body diff** — line-based side-by-side using the `similar` crate (already a cognitive dep)
- **Bootstraps diff** — new databases the upgrade would add, removed databases (become orphaned), breaking template changes (e.g. a bootstrapped database's field type changed)

UI presents this as a timeline across all intermediate versions; user can jump to any version or view the cumulative diff from installed → latest.

---

## Skill package format

Fully compatible with the Agent Skills spec used by skills.sh. Klynt-specific behavior lives in the optional `metadata.klyntbot:` block (already parsed today). New MVP additions:

```yaml
---
name: reading-list
description: Track books with notes, ratings, and reading sessions
whenToUse: When the user mentions books, reading, or notes on articles
metadata:
  klyntbot:
    type: orchestrator
    tools: [database]
    version: 1.0.3                          # NEW — required for marketplace
    klyntbot_min_version: ">=0.1.0"         # NEW — client compat floor
    triggers: ["book", "reading"]
    bootstraps:                             # NEW — declares install-time side effects
      databases:
        - template: reading_list.json
        - template: highlights.json
    salience:                               # already supported
      extract_on:
        - field: rating
          to_values: ["5"]
          importance: 0.9
---

[skill body — unchanged]
```

On-disk layout (same for Klynt skills and plain skills.sh skills):

```
reading-list/
  SKILL.md
  references/           # optional
    how-to-rate.md
  templates/            # optional, referenced by bootstraps.databases
    reading_list.json
    highlights.json
```

Prompt-only skills (no `klyntbot:` block) install successfully and appear with a `Prompt-only` badge. No bootstraps, no tool registrations — they only contribute prompt text.

No separate `klynt.toml` manifest. Everything lives in SKILL.md frontmatter to match the skills.sh workflow and eliminate drift between two manifest files.

---

## Flows

### Install (new skill)

```
User clicks "Install" on a source
  → installer.preview_install(source, version)
      ├─ registry.fetch(source, version)  → SkillPackage
      ├─ If no klyntbot_meta AND user requested adapter:
      │     adapter.adapt(pkg, ctx) → adapted SkillPackage
      └─ Build InstallPlan and return
  → UI renders InstallPlan in consent dialog:
      ├─ "Install + Bootstrap"  → installer.apply_install(plan)
      ├─ "Install Skill Only"   → installer.apply_install(plan.skill_only())
      └─ "Cancel"                → drop plan
  → apply_install() is transactional:
      1. Write files to ~/.klyntbot-dev/skills/{name}/
      2. For each bootstrap template: EntityStore.install_template() → collect IDs
      3. Insert row into installed_skills (+ bootstrapped_databases IDs)
      4. SkillStore.reload()
      5. Emit SkillInstalled event
  → On any step failure: roll back (delete files, drop databases, no row inserted).
```

### Upgrade (existing skill)

```
User clicks "Upgrade to v1.0.3" from Updates tab
  → installer.preview_upgrade(name, target_sha)
      ├─ Fetch SKILL.md at target_sha
      ├─ Diff against installed_sha (frontmatter + body + bootstraps)
      └─ Build UpgradePlan { diffs, new_bootstraps, removed_bootstraps, breaking_changes }
  → UI shows DiffViewer + consent dialog
  → apply_upgrade():
      1. Overwrite skill files atomically (tmp + rename)
      2. For each NEW bootstrap: install_template with consent (same flow as fresh install)
      3. Never auto-modify existing bootstrapped databases — user data is sacred
      4. Update installed_skills row (new version/sha/bootstrapped_databases)
      5. Emit SkillUpgraded event
```

### Uninstall

```
User clicks "Uninstall"
  → Query installed_skills → find bootstrapped_databases
  → UI shows three-radio dialog:
      ├─ "Remove skill only"           → delete files, drop row, leave databases
      ├─ "Remove skill + archive"      → same + rename databases to "Archived: {name}"
      └─ "Remove skill + delete data"  → same + EntityStore.delete_database() for each
  → installer.uninstall(name, mode):
      1. Execute user's mode
      2. SkillStore.reload()
      3. Emit SkillUninstalled event
```

### Errors

Every failure mode renders a user-visible message:
- Network failure: "Couldn't reach GitHub. Check connection or try a different source."
- Schema violation: "This skill declares field types we don't support: [list]. Install as prompt-only?"
- Adapter timeout: "Couldn't adapt this skill automatically. Install as-is?"
- Rollback fired: "Install failed halfway; everything rolled back. No data was modified."

### Domain events

Published on `DomainEventBus` so future Reforge subscribers can react:
- `SkillInstalled { name, source, version }`
- `SkillUpgraded { name, from_version, to_version }`
- `SkillUninstalled { name, mode }`
- `SkillAdapted { name, adapter_model }`

---

## UI

### `/skills` — list (leaderboard layout)

- Header: title + `Offline / Audits / Docs / Settings` action row
- Search bar (client-side over the loaded list for Installed; server-side for Browse)
- Tab row: `Installed (N)` · `All Time` · `Trending` · `Updates (N)`
- Single table renderer reused across tabs:

  ```
  #   SKILL                                  INSTALLS     STATUS
  1   find-skills           vercel-labs/s…   1.0M         [Install]
  2   vercel-react-best…    vercel-labs/s…   315.3K       [Install]
  ✓   reading-list          klynt-skills/…   —            [Installed]
  ✓   task-management       bundled          —            [Built-in]
  ```

- Klynt-compatible skills (klyntbot block present) carry a `Klynt` badge; bootstraps count in a subtitle line
- Paste-source input at the top of `Browse` accepts `owner/repo/subpath` or full GitHub URL

### `/skills/:source` — detail page

Grid layout with left content + fixed right rail.

- **Breadcrumb** — `Skills / owner / repo / skill-path`
- **Title** — skill name + type badge (`orchestrator` / `skill` / `Prompt-only`)
- **Installation CTA** — `[Install + Bootstrap]` `[Install Skill Only]` for new skills; `[Upgrade to v1.0.3]` `[View diff]` `[Uninstall]` for installed ones
- **Summary** — trigger badges + description + tags
- **SKILL.md** — full rendered body using the existing notes markdown renderer

Right rail (fixed 240px, glass-card styling):
- **Weekly installs** — from skills.sh API when applicable; omitted otherwise
- **Repository** — source link + external-link icon
- **Repo stars** — GitHub API
- **First seen** — date
- **Security audits** — badges if the source provides them; `Unverified` for non-curated sources
- **Installed** section — if installed, lists bootstrapped databases with click-through

### Modals

- `InstallPreviewDialog` — renders `InstallPlan` with per-section diff/bootstrap preview. Contains the Adapt-for-Klynt button for prompt-only skills.
- `UninstallDialog` — three radio options per section above.
- `DiffViewer` — frontmatter table + body diff + bootstraps diff, with a timeline slider across intermediate versions.

### Shared UI conventions

- Uses only design tokens (`bg-surface-base`, `border-border`, `text-foreground`, etc.) — no hardcoded colors
- `glass-card` for the right-rail panel, dropdowns, modals
- `useQuery` / `useMutation` hooks for all data access; invalidation key `"skills:updated"` fires on install/upgrade/uninstall events
- Route lazy-loaded per existing router convention

---

## Trust model

For MVP:

- **No signing, no sandboxing** — the security boundary is the install preview. Users see every file that will be written and every database that will be created before anything executes.
- **Third-party badge** — any source not in our curated allowlist renders a `Third-party source` warning on the detail page and install preview.
- **HTTPS-only** — all fetches go over HTTPS. `git` sources use `https://github.com/...` form only; no SSH.
- **Content-hash pinning** — the `installed_sha` acts as a content pin; re-installing the same version from a different ref produces a mismatch warning.

Future (explicitly out of scope for MVP):
- Manifest signatures (Sigstore / PGP)
- WASM sandboxing for skills that grow beyond prompt + bootstraps
- Reputation / audit integration with the registry

---

## Testing

| Layer | Tests |
|---|---|
| `skills-registry` | Unit: parse `owner/repo/subpath`; GitHub tree/ref resolution; fetch-cache hits; malformed frontmatter rejection; skills.sh slug → GitHub source resolution. |
| `skills-installer` | Integration (ephemeral SQLite): fresh-install happy path; rollback on mid-install failure (files deleted, databases dropped, no row inserted); upgrade preserves bootstrapped database IDs; all three uninstall modes; bootstrap consent respected. |
| `skills-adapter` | Unit with mocked LLM provider: 3 golden-file cases — a prompt-only coding skill (returns "adapter not useful"), a Klynt-shaped skill (returns rich klyntbot block + 2 templates), an ambiguous skill (returns a conservative klyntbot block and warns). Schema-validation rejection path covered. |
| Tauri commands | `dev_server_covers_all_tauri_commands` catches missing dispatch wiring. |
| Frontend | Vitest for `useMarketplace` hook, `InstallPreviewDialog` renders plan fields correctly, `DiffViewer` handles empty-diff and breaking-changes paths. |
| E2E | Not automated for MVP. Manual script in the verification section. |

### Manual verification (run before shipping)

1. `cargo tauri dev` + `bun run dev`
2. Navigate to `/skills`; confirm Installed tab lists 6 bundled skills, all with `Built-in` status
3. Browse tab: paste `anthropics/skills/frontend-design` — detail page renders with right-rail metadata
4. Click `Install Skill Only` — SkillStore reloads, `skill_list` shows new row
5. Browse tab: paste a prompt-only skill source — click `Adapt for Klynt` — preview shows generated klyntbot block + rationale — click `Install + Bootstrap` — verify databases appear in sidebar
6. Visit Updates tab; upgrade a skill — DiffViewer renders frontmatter + body diff — click Apply — verify version bumps in Installed tab
7. Uninstall the adapted skill with `Remove skill + delete data` — verify bootstrapped databases gone and sidebar refreshes
8. Disable all cognitive providers in settings — Adapt button becomes disabled with tooltip

### Regression guards

- `cargo nextest run --workspace` — all existing tests green
- `cargo check --workspace` — zero new warnings
- `bun run lint` — zero Biome errors

---

## Files that will be added

**New crates**
- `crates/skills-registry/` (L3)
- `crates/skills-installer/` (L4)
- `crates/skills-adapter/` (L4)

**New migration**
- `crates/storage/migrations/NNN_skills_marketplace.sql` — NNN is assigned at implementation time based on the current migration count

**New handlers**
- `crates/app-core/src/handlers/skills/mod.rs` — thin wrappers around installer

**New Tauri commands**
- `crates/desktop/src/commands/skills.rs` — #[tauri::command] wrappers + `DEV_COMMANDS` + `dispatch_dev`
- `crates/desktop/src/main.rs` — invoke_handler registrations
- `crates/desktop/src/dev_server/mod.rs` — add to `dev_command_names()` list

**New frontend**
- `desktop-ui/src/features/skills/` — full feature with pages, hooks, components
  - `pages/SkillsListPage.tsx`, `pages/SkillDetailPage.tsx`
  - `components/SkillRow.tsx`, `SkillDetailSidebar.tsx`, `InstallCta.tsx`, `InstallPreviewDialog.tsx`, `UninstallDialog.tsx`, `DiffViewer.tsx`, `SkillMarkdown.tsx`
  - `hooks/useSkillList.ts`, `useSkillDetail.ts`, `useInstallPreview.ts`, `useSkillUpdates.ts`
- `desktop-ui/src/app/router.tsx` — new route registrations
- `desktop-ui/src/app/layouts/Sidebar.tsx` — Store icon entry

**Files modified**
- `crates/skill-system/src/store.rs` — no change; SkillStore.reload() already exists
- `crates/cognitive/src/` — no change; adapter uses the cognitive provider trait directly

---

## Non-goals (for MVP)

- Publishing / authoring skills from inside Klynt (users edit via filesystem or via the community website once it exists)
- Ratings, reviews, comments
- Forking / customizing a skill inside the UI — users edit on disk or in git
- Offline-only mode — MVP requires network for Browse, but works offline for Installed/Upgrade of already-cached versions
- Per-skill permission prompts beyond the install preview (no "this skill wants access to X" runtime prompts)
- Migration of existing user-installed skills from old paths — we ship with only the 6 bundled skills today, so there are no existing third-party installs to migrate
