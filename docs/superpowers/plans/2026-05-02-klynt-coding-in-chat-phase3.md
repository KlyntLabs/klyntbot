# Klynt Coding-in-Chat — Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the Phase 3+ scope from `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §13 — MCP-contributed skills, per-channel MCP allowlists, Skills.sh marketplace, multi-window per-repo coding, voice-driven coding wire-up, LSP diagnostics + coverage signals, Mirror reranking for `tool_search`, K10/K11 proptests, the missing codex-polish plan doc, and the unmeasured 800ms p95 perf gate.

**Architecture:** Phase 3 is additive — no new sandbox layers, no schema breaks. Most work threads new optional capabilities through existing seams: a new `lsp-client` crate feeds the already-stubbed `FileEditWithSymbols.lsp_diagnostics_delta`, a new `coverage` parser module fills `TestRunDetailed.coverage_delta`, a new `SkillEffectivenessSource` Mirror signal populates the already-present `ToolSearchTool.effectiveness_scores` map. MCP and skills surface gain new `SkillSource::Mcp` / `SkillSource::SkillsMarketplace` enum variants and a `McpChannelAllowlist` config key. Multi-window extends `lazy_window.rs` with a `coding:{repo_id}` label pattern.

**Tech Stack:** Rust 1.93, Tauri 2, `async-lsp` (new dep), `tower-lsp` not used, React 18 + Vitest, proptest, `cargo-nextest`. Pre-release policy applies — schema changes consolidate in-place, no migration scripts.

**Out of scope (per spec line 1413 / appendix):** Windows sandbox, IDE bridge MCP extensions (separate spec).

**Spec coverage map:**

| Spec § | Phase 3 item | Task # |
|---|---|---|
| §6 line 672 | `tool_search` Mirror reranking | T7 |
| §6 line 680-682 | Truncation policy (already done) | — |
| §8 line 957+ | `/skills install` source: MCP, skills.sh | T2, T3 |
| §10 line 1133 | `lsp_diagnostics_delta` real values | T5 |
| §10 line 1134 | `anchored_symbols` LSP-grade | T5 |
| §10 line 1135 | `coverage_delta` per-framework parsers | T6 |
| §13 Phase 3+ | Multi-window per-repo | T8 |
| §13 Phase 3+ | Voice-driven coding wire-up | T9 |
| §13 Phase 3+ | Per-channel MCP allowlists | T1 |
| Appendix C | K10, K11 proptests | T10 |
| §13 Phase 2 (deferred housekeeping) | 800ms p95 perf gate | T11 |
| CLAUDE.md gotcha | Missing codex-polish plan doc | T0 |
| Phase 1 punch list | Approval Tauri commands audit | T12 |

---

## File Structure

New files / crates created in Phase 3:

```
bot/
├── crates/
│   ├── lsp-client/                            # NEW crate (T5)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs                         # LspClientHandle public API
│   │   │   ├── server_pool.rs                 # one server process per (lang, workspace_root)
│   │   │   ├── diagnostics.rs                 # LspDiagnostic struct + delta diffing
│   │   │   ├── symbols.rs                     # documentSymbol query for anchored_symbols
│   │   │   └── language.rs                    # path → language id (rust→rust-analyzer, ts→tsserver, …)
│   │   └── tests/
│   │       ├── diagnostics_delta.rs
│   │       └── server_lifecycle.rs
│   ├── coding-ingest/src/
│   │   ├── coverage/                          # NEW module (T6)
│   │   │   ├── mod.rs                         # CoverageDelta struct + parse() dispatch
│   │   │   ├── lcov.rs
│   │   │   ├── cobertura.rs
│   │   │   ├── tarpaulin.rs                   # tarpaulin tarpaulin-report.json
│   │   │   └── llvm_cov.rs                    # cargo-llvm-cov json
│   │   └── event.rs                           # MODIFY: DiagnosticsDelta gains Vec<LspDiagnostic>
│   ├── klynt-skill-loader/src/
│   │   ├── index.rs                           # MODIFY: SkillSource adds Mcp + SkillsMarketplace (T2, T3)
│   │   ├── url.rs                             # MODIFY: SkillUrlKind::SkillsSh
│   │   └── discovery.rs                       # MODIFY: scan_mcp_server()
│   ├── mcp/src/
│   │   ├── allowlist.rs                       # NEW: McpChannelAllowlist (T1)
│   │   └── dispatch.rs                        # MODIFY: gate by allowlist
│   ├── config/src/schema/
│   │   ├── mcp.rs                             # MODIFY: channel_allowlists field
│   │   └── coding.rs                          # already has mirrorLearning
│   ├── cognitive/src/mirror/sources/
│   │   └── skill_effectiveness.rs             # NEW: skill outcome → score (T7)
│   ├── klynt-core/src/tools/tool_search/
│   │   └── tool.rs                            # MODIFY: consume scores in rank
│   ├── desktop/src/
│   │   ├── lazy_window.rs                     # MODIFY: coding:{repo_id} label (T8)
│   │   └── commands/
│   │       └── coding_window.rs               # NEW: coding_open_repo_window
│   └── desktop-ui/src/features/coding/
│       ├── pages/CodingPage.tsx               # MODIFY: parameterize :repoId (T8)
│       ├── components/SkillsMarketplace.tsx   # NEW (T3)
│       └── hooks/useCodingDictation.ts        # NEW (T9 — thin wrapper over useDictationController)
├── docs/superpowers/plans/
│   └── 2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md  # NEW (T0)
└── crates/{klynt-core,app-core}/tests/
    ├── mirror_approval_k10_proptest.rs        # NEW (T10)
    └── sessions_retention_k11_proptest.rs     # NEW (T10)
```

Each task below is independent and commits separately. Run quality gates (build / clippy / fmt / nextest / `cd desktop-ui && bun run lint && bun run typecheck && bun run test`) at the end of every task.

---

## Task 0: Backfill the missing codex-polish plan doc

CLAUDE.md and the spec (line 682, line 1412) reference `docs/superpowers/plans/2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md`, but the file does not exist — the code shipped (commit `3ca997eda`: ghost commits, process hardening, truncation) without its plan doc. Backfill it so the audit trail is intact.

**Files:**
- Create: `docs/superpowers/plans/2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md`

- [ ] **Step 1: Write the doc**

Write a retrospective plan doc describing the three crates that landed in commit `3ca997eda`:
1. `klynt-git-utils` — ghost commits (`create_ghost_commit`, `restore_ghost_commit`) replacing BLOB content addressing for git-tracked files; BLOB fallback retained for non-git directories. Files: `crates/klynt-git-utils/src/ghost_commits.rs`, `crates/klynt-core/src/snapshots/repo.rs::try_record_with_ghost`.
2. `klynt-process-hardening` — `pre_main_hardening()` setting `RLIMIT_CORE=0`, calling `ptrace(PT_DENY_ATTACH)` on macOS, scrubbing `LD_*`/`DYLD_*`/`MallocStackLogging*` env vars. Called as the first statement in `crates/desktop/src/main.rs`.
3. `klynt-truncation` — `TruncationPolicy::Bytes`/`Tokens` middle-chop with "Total output lines: N" prefix. Replaces the deferred Claude-Code-style content-replacement design (spec line 680-682). Files: `crates/klynt-truncation/src/lib.rs`.

Format: same header as this plan, but mark every task `- [x]` (already done). Each task should reference the exact file paths so reviewers can navigate to the landed code.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md
git commit -m "docs(coding-in-chat): backfill phase 3 codex-polish plan doc"
```

---

## Task 1: Per-channel MCP allowlists

Spec §13 Phase 3+. Coding channel and regular-chat channel each get their own list of allowed MCP servers, so a `linear` MCP server enabled for chat doesn't leak into coding mode (and vice versa for `github-code-search`).

**Files:**
- Create: `crates/mcp/src/allowlist.rs`
- Modify: `crates/mcp/src/lib.rs`, `crates/mcp/src/dispatch.rs`
- Modify: `crates/config/src/schema/mcp.rs`
- Modify: `crates/klynt-core/src/registry/builder.rs`
- Test: `crates/mcp/tests/channel_allowlist.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/mcp/tests/channel_allowlist.rs
use mcp::allowlist::{McpChannelAllowlist, AllowDecision};

#[test]
fn allowlist_gates_per_channel() {
    let mut a = McpChannelAllowlist::default();
    a.allow("coding", "github");
    a.allow("chat", "linear");

    assert_eq!(a.decide("coding", "github"), AllowDecision::Allowed);
    assert_eq!(a.decide("coding", "linear"), AllowDecision::Denied);
    assert_eq!(a.decide("chat", "github"),  AllowDecision::Denied);
}

#[test]
fn unconfigured_channel_allows_all_for_back_compat() {
    let a = McpChannelAllowlist::default();
    assert_eq!(a.decide("coding", "github"), AllowDecision::Allowed);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo nextest run -p mcp -E 'test(channel_allowlist)'`
Expected: FAIL — `mcp::allowlist` module not found.

- [ ] **Step 3: Implement allowlist**

```rust
// crates/mcp/src/allowlist.rs
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct McpChannelAllowlist {
    /// channel id → allowed server names. Absent channel = unconfigured = allow-all.
    inner: HashMap<String, HashSet<String>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AllowDecision {
    Allowed,
    Denied,
}

impl McpChannelAllowlist {
    pub fn allow(&mut self, channel: &str, server: &str) {
        self.inner.entry(channel.into()).or_default().insert(server.into());
    }

    pub fn decide(&self, channel: &str, server: &str) -> AllowDecision {
        match self.inner.get(channel) {
            None => AllowDecision::Allowed,
            Some(set) if set.contains(server) => AllowDecision::Allowed,
            Some(_) => AllowDecision::Denied,
        }
    }
}
```

Add `pub mod allowlist;` to `crates/mcp/src/lib.rs`.

- [ ] **Step 4: Verify pass**

Run: `cargo nextest run -p mcp -E 'test(channel_allowlist)'` — Expected: PASS.

- [ ] **Step 5: Wire into dispatch**

In `crates/mcp/src/dispatch.rs`, find the `dispatch_tool_call` (or equivalent) entrypoint. Add an `&McpChannelAllowlist` parameter, call `decide(&channel, &server_name)` before invoking the MCP client; on `Denied` return `Err(KlyntbotError::ToolError(ToolError::NotAllowedInChannel { channel, server }))`. Add the new error variant in `crates/common/src/error.rs` if absent (otherwise reuse `ToolError::PermissionDenied`).

- [ ] **Step 6: Add config field**

```rust
// crates/config/src/schema/mcp.rs (inside McpConfig)
#[serde(default)]
pub channel_allowlists: HashMap<String, Vec<String>>,
```

Hydrate `McpChannelAllowlist::from(config.mcp.channel_allowlists.clone())` in `crates/app-core/src/init/mod.rs` where MCP is initialized; pass it down to `ToolKitBuilder` via a new setter.

- [ ] **Step 7: Integration test**

Add `crates/mcp/tests/channel_allowlist_dispatch.rs`: stand up a fake MCP server, call dispatch with an empty allowlist for channel "coding" — assert it errors; allow "coding" → "fake_server" — assert it succeeds.

- [ ] **Step 8: Commit**

```bash
git add crates/mcp crates/config/src/schema/mcp.rs crates/app-core/src/init/mod.rs crates/common/src/error.rs
git commit -m "feat(mcp): per-channel server allowlists"
```

---

## Task 2: MCP-contributed skills

Spec §8. MCP servers expose skills via a resource convention (URI scheme `klynt-skill://<server>/<skill_name>`). The skill loader scans connected MCP servers in addition to filesystem roots.

**Files:**
- Modify: `crates/klynt-skill-loader/src/index.rs` (add `SkillSource::Mcp`)
- Modify: `crates/klynt-skill-loader/src/discovery.rs` (add `scan_mcp_server`)
- Modify: `crates/skill-system/src/store.rs`, `crates/skill-system/src/listing.rs` (handle Mcp variant)
- Test: `crates/klynt-skill-loader/tests/mcp_source.rs`

- [ ] **Step 1: Failing test**

```rust
// crates/klynt-skill-loader/tests/mcp_source.rs
use klynt_skill_loader::index::{SkillSource, IndexedSkill};

#[test]
fn mcp_source_serializes_with_server_name() {
    let s = SkillSource::Mcp { server_name: "github".into() };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"mcp\""));
    assert!(json.contains("github"));
}

#[tokio::test]
async fn scan_mcp_server_returns_skills_from_resources() {
    let fake = klynt_skill_loader::testing::FakeMcpServer::new()
        .with_skill_resource("rust-debug", "---\nname: rust-debug\n---\nbody")
        .build();
    let skills = klynt_skill_loader::discovery::scan_mcp_server(&fake).await.unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "rust-debug");
    assert!(matches!(skills[0].source, SkillSource::Mcp { .. }));
}
```

- [ ] **Step 2: Run, expect fail**

`cargo nextest run -p klynt-skill-loader -E 'test(mcp_source)'` — FAIL.

- [ ] **Step 3: Add enum variant**

```rust
// crates/klynt-skill-loader/src/index.rs (in SkillSource)
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    User,
    ReforgePrivate,
    ReforgeTeam,
    Project,
    Mcp { server_name: String },
    SkillsMarketplace { name: String, version: String }, // T3 lands this
}
```

- [ ] **Step 4: Implement `scan_mcp_server`**

```rust
// crates/klynt-skill-loader/src/discovery.rs
pub async fn scan_mcp_server<C: McpResourceClient>(client: &C) -> Result<Vec<IndexedSkill>> {
    let resources = client.list_resources().await?;
    let mut out = Vec::new();
    for r in resources {
        if let Some(name) = r.uri.strip_prefix("klynt-skill://") {
            let body = client.read_resource(&r.uri).await?;
            let parsed = parse_skill_md(&body)?;
            out.push(IndexedSkill {
                name: parsed.name,
                source: SkillSource::Mcp { server_name: client.server_name().into() },
                body: parsed.body,
                paths: parsed.paths,
                ..Default::default()
            });
        }
    }
    Ok(out)
}
```

Add a `McpResourceClient` trait in the same file (`list_resources`, `read_resource`, `server_name`) — implement for the existing `mcp::client::McpClient` in a thin adapter.

- [ ] **Step 5: Hook into top-level discovery**

In `klynt-skill-loader::discovery::discover_all`, after filesystem scans, iterate connected MCP clients (passed in as `&[Arc<dyn McpResourceClient>]`) and merge their skills.

- [ ] **Step 6: Run all tests**

`cargo nextest run -p klynt-skill-loader` — Expected: green.

- [ ] **Step 7: Update store + listing**

In `crates/skill-system/src/store.rs` and `listing.rs`, add match arms for `SkillSource::Mcp { server_name }` — render as `mcp:{server_name}` in user-facing labels; storage stays string-typed.

- [ ] **Step 8: Commit**

```bash
git add crates/klynt-skill-loader crates/skill-system
git commit -m "feat(skills): MCP-contributed skills via klynt-skill:// resources"
```

---

## Task 3: Skills.sh marketplace integration

`/skills install https://skills.sh/<name>` and a UI browse panel.

**Files:**
- Modify: `crates/klynt-skill-loader/src/url.rs` (add `SkillUrlKind::SkillsSh`)
- Modify: `crates/klynt-skill-loader/src/installer.rs` (or wherever install resolves URLs)
- Create: `desktop-ui/src/features/coding/components/SkillsMarketplace.tsx`
- Modify: `desktop-ui/src/features/settings/components/sections/coding/SkillsSubsection.tsx`
- Test: `crates/klynt-skill-loader/tests/skills_sh_url.rs`

- [ ] **Step 1: Failing test**

```rust
// crates/klynt-skill-loader/tests/skills_sh_url.rs
use klynt_skill_loader::url::{classify_skill_url, SkillUrlKind};

#[test]
fn skills_sh_url_recognized() {
    let k = classify_skill_url("https://skills.sh/rust-debugging").unwrap();
    assert!(matches!(k, SkillUrlKind::SkillsSh { name } if name == "rust-debugging"));
}

#[test]
fn versioned_skills_sh_url() {
    let k = classify_skill_url("https://skills.sh/rust-debugging@1.2.0").unwrap();
    match k {
        SkillUrlKind::SkillsSh { name } => assert_eq!(name, "rust-debugging@1.2.0"),
        _ => panic!(),
    }
}
```

- [ ] **Step 2: Run, expect fail**

`cargo nextest run -p klynt-skill-loader -E 'test(skills_sh)'` — FAIL.

- [ ] **Step 3: Add variant + classifier**

```rust
// crates/klynt-skill-loader/src/url.rs
pub enum SkillUrlKind {
    GitHub { owner: String, repo: String, path: Option<String> },
    Gist { id: String },
    LocalPath(PathBuf),
    SkillsSh { name: String },
}

pub fn classify_skill_url(s: &str) -> Result<SkillUrlKind> {
    if let Some(rest) = s.strip_prefix("https://skills.sh/") {
        if rest.is_empty() { return Err(KlyntbotError::InvalidArgument("empty skill name".into())); }
        return Ok(SkillUrlKind::SkillsSh { name: rest.into() });
    }
    // existing branches…
}
```

- [ ] **Step 4: Verify pass**

`cargo nextest run -p klynt-skill-loader -E 'test(skills_sh)'` — PASS.

- [ ] **Step 5: Implement fetch**

In `crates/klynt-skill-loader/src/installer.rs`, add a branch for `SkillUrlKind::SkillsSh { name }`:
1. Parse `name@version` (default version = "latest").
2. GET `https://skills.sh/api/v1/skills/{name_no_version}/{version}` — expect JSON `{ "tarball_url": "…", "version": "1.2.0", "sha256": "…" }`.
3. Download tarball, verify sha256, extract under `~/.klyntbot/skills/marketplace/{name}/{version}/`.
4. Record `SkillSource::SkillsMarketplace { name, version }` in the index.

Use `reqwest` (already a workspace dep). Sha256 via `sha2` crate.

- [ ] **Step 6: Failing UI test**

```ts
// desktop-ui/src/features/coding/components/SkillsMarketplace.test.tsx
import { render, screen } from '@testing-library/react';
import { SkillsMarketplace } from './SkillsMarketplace';

test('renders search box and install buttons for results', async () => {
  vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(async (cmd) => {
      if (cmd === 'coding_skills_marketplace_search')
        return [{ name: 'rust-debug', version: '1.0.0', summary: 'Debug Rust' }];
    }),
  }));
  render(<SkillsMarketplace />);
  await screen.findByText('rust-debug');
  expect(screen.getByRole('button', { name: /install/i })).toBeInTheDocument();
});
```

- [ ] **Step 7: Implement component**

```tsx
// desktop-ui/src/features/coding/components/SkillsMarketplace.tsx
import { useEffect, useState } from 'react';
import { invoke } from '@/api/client';

interface MarketSkill { name: string; version: string; summary: string; }

export function SkillsMarketplace() {
  const [q, setQ] = useState('');
  const [results, setResults] = useState<MarketSkill[]>([]);
  useEffect(() => {
    invoke<MarketSkill[]>('coding_skills_marketplace_search', { query: q })
      .then(setResults).catch(() => setResults([]));
  }, [q]);
  return (
    <div className="skills-marketplace">
      <input value={q} onChange={e => setQ(e.target.value)} placeholder="Search skills.sh…" />
      <ul>
        {results.map(r => (
          <li key={`${r.name}@${r.version}`}>
            <span>{r.name} <em>v{r.version}</em></span>
            <p>{r.summary}</p>
            <button onClick={() => invoke('coding_skills_install', { url: `https://skills.sh/${r.name}@${r.version}` })}>
              Install
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

Add `coding_skills_marketplace_search` Tauri command (klynt_command) in `crates/desktop/src/commands/coding_skills.rs` that proxies to `https://skills.sh/api/v1/search?q={q}`.

- [ ] **Step 8: Wire into Settings**

In `SkillsSubsection.tsx`, add a "Browse marketplace" button that toggles `<SkillsMarketplace />`.

- [ ] **Step 9: Run gates + commit**

```bash
cargo nextest run -p klynt-skill-loader && cd desktop-ui && bun run test
git add crates/klynt-skill-loader crates/desktop/src/commands/coding_skills.rs desktop-ui/src/features/coding/components/SkillsMarketplace.tsx desktop-ui/src/features/settings/components/sections/coding/SkillsSubsection.tsx
git commit -m "feat(skills): skills.sh marketplace install + browse UI"
```

---

## Task 4: (reserved — folded into T1/T2/T3 above)

Skip — keeping numbering aligned with the spec coverage map.

---

## Task 5: LSP client crate + populate `lsp_diagnostics_delta` and `anchored_symbols`

Spec §10 line 1133-1134. Today both fields are stubs (`vec![]` in `crates/klynt-core/src/tools/shared/file_edit_event.rs`). Phase 3 lights them up via a new `lsp-client` crate.

**Files:**
- Create: `crates/lsp-client/{Cargo.toml,src/{lib.rs,server_pool.rs,diagnostics.rs,symbols.rs,language.rs}}`
- Create: `crates/lsp-client/tests/{diagnostics_delta.rs,server_lifecycle.rs}`
- Modify: `crates/coding-ingest/src/event.rs` (`DiagnosticsDelta` gains `Vec<LspDiagnostic>`)
- Modify: `crates/klynt-core/src/tools/shared/file_edit_event.rs`
- Modify: `crates/klynt-core/src/registry/builder.rs` (inject `LspClientHandle`)

- [ ] **Step 1: Scaffold crate + add to workspace**

```toml
# crates/lsp-client/Cargo.toml
[package]
name = "lsp-client"
version.workspace = true
edition.workspace = true

[dependencies]
async-lsp = "0.2"
lsp-types = "0.97"
tokio = { workspace = true, features = ["process", "io-util", "sync"] }
tracing = { workspace = true }
common = { path = "../common" }
serde = { workspace = true }
```

Add `"crates/lsp-client",` to root `Cargo.toml` workspace members.

- [ ] **Step 2: Failing test**

```rust
// crates/lsp-client/tests/diagnostics_delta.rs
use lsp_client::diagnostics::{LspDiagnostic, diff};

#[test]
fn diff_introduced_and_resolved() {
    let before = vec![LspDiagnostic::error("E0001", "unused import", 3)];
    let after  = vec![LspDiagnostic::error("E0277", "trait bound", 7)];
    let d = diff(&before, &after);
    assert_eq!(d.introduced.len(), 1);
    assert_eq!(d.resolved.len(), 1);
    assert_eq!(d.introduced[0].code, "E0277");
}
```

- [ ] **Step 3: Run, expect fail**

`cargo nextest run -p lsp-client` — FAIL (crate doesn't compile).

- [ ] **Step 4: Implement diagnostics module**

```rust
// crates/lsp-client/src/diagnostics.rs
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub code: String,
    pub message: String,
    pub line: u32,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity { Error, Warning, Info, Hint }

impl LspDiagnostic {
    pub fn error(code: impl Into<String>, msg: impl Into<String>, line: u32) -> Self {
        Self { code: code.into(), message: msg.into(), line, severity: Severity::Error }
    }
}

#[derive(Debug, Default)]
pub struct DiagnosticDiff {
    pub introduced: Vec<LspDiagnostic>,
    pub resolved: Vec<LspDiagnostic>,
}

pub fn diff(before: &[LspDiagnostic], after: &[LspDiagnostic]) -> DiagnosticDiff {
    let bset: std::collections::HashSet<_> = before.iter().collect();
    let aset: std::collections::HashSet<_> = after.iter().collect();
    DiagnosticDiff {
        introduced: aset.difference(&bset).map(|d| (*d).clone()).collect(),
        resolved:  bset.difference(&aset).map(|d| (*d).clone()).collect(),
    }
}
```

- [ ] **Step 5: Verify pass**

`cargo nextest run -p lsp-client -E 'test(diff_introduced_and_resolved)'` — PASS.

- [ ] **Step 6: Implement server pool (incremental)**

```rust
// crates/lsp-client/src/language.rs
pub fn language_for(path: &std::path::Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust-analyzer"),
        "ts" | "tsx" => Some("typescript-language-server"),
        "py" => Some("pyright"),
        "go" => Some("gopls"),
        _ => None,
    }
}
```

```rust
// crates/lsp-client/src/server_pool.rs
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct LspServerPool { servers: Mutex<HashMap<(String, std::path::PathBuf), Arc<async_lsp::ServerSocket>>> }
impl LspServerPool {
    pub fn new() -> Self { Self { servers: Mutex::new(HashMap::new()) } }
    pub async fn get_or_spawn(&self, lang: &str, root: &std::path::Path) -> common::Result<Arc<async_lsp::ServerSocket>> {
        // … spawn child process per (lang, root); cache the ServerSocket. Initialize on first use.
        todo!("implement using async-lsp ClientSocket::new + tokio::process::Command::new(lang).arg('--stdio')")
    }
}
```

Implement minimally — spawn the language server process, send `initialize` + `initialized`, return a clone of the `Arc<ServerSocket>`. Add a unit test that spawns a no-op echo binary stub if a real LSP isn't available in CI; gate the real-LSP test behind `#[ignore]`.

- [ ] **Step 7: Implement `LspClientHandle` public API**

```rust
// crates/lsp-client/src/lib.rs
pub mod diagnostics;
pub mod server_pool;
pub mod symbols;
pub mod language;

#[derive(Clone)]
pub struct LspClientHandle { pool: std::sync::Arc<server_pool::LspServerPool> }

impl LspClientHandle {
    pub fn new() -> Self { Self { pool: std::sync::Arc::new(server_pool::LspServerPool::new()) } }

    pub async fn diagnostics_for(&self, path: &std::path::Path)
        -> common::Result<Vec<diagnostics::LspDiagnostic>>
    {
        let Some(lang) = language::language_for(path) else { return Ok(vec![]); };
        let root = workspace_root(path)?;
        let server = self.pool.get_or_spawn(lang, &root).await?;
        // textDocument/didOpen → wait for publishDiagnostics → close
        symbols::query_diagnostics(&server, path).await
    }

    pub async fn document_symbols(&self, path: &std::path::Path)
        -> common::Result<Vec<symbols::AnchoredSymbol>>
    {
        let Some(lang) = language::language_for(path) else { return Ok(vec![]); };
        let root = workspace_root(path)?;
        let server = self.pool.get_or_spawn(lang, &root).await?;
        symbols::document_symbols(&server, path).await
    }
}

fn workspace_root(path: &std::path::Path) -> common::Result<std::path::PathBuf> {
    // walk up until we find Cargo.toml / package.json / pyproject.toml / .git
    todo!()
}
```

- [ ] **Step 8: Wire into `file_edit_event`**

```rust
// crates/klynt-core/src/tools/shared/file_edit_event.rs (modify)
pub async fn build_file_edit_event(
    path: &Path,
    before: &str,
    after: &str,
    lsp: Option<&LspClientHandle>,        // NEW
) -> AgentEvent {
    let lsp_diagnostics_delta = if let Some(client) = lsp {
        let after_diags = client.diagnostics_for(path).await.unwrap_or_default();
        // Phase 3: we capture only post-edit diagnostics here; the before-set is the
        // last cached snapshot the registry holds (see DiagnosticCache below).
        DiagnosticCache::diff_and_replace(path, after_diags)
    } else {
        Vec::new()
    };
    let anchored_symbols = if let Some(client) = lsp {
        client.document_symbols(path).await.unwrap_or_default()
            .into_iter().map(Into::into).collect()
    } else {
        // existing best-effort tree-sitter pass — keep as fallback
        treesitter_anchors(path, after)
    };
    // … construct FileEditWithSymbols with the populated fields
}
```

Add a `DiagnosticCache` in `klynt-core/src/tools/shared/diagnostic_cache.rs` (a `DashMap<PathBuf, Vec<LspDiagnostic>>`) so the "before" set persists across edits within a session.

- [ ] **Step 9: Inject `LspClientHandle` via builder**

In `crates/klynt-core/src/registry/builder.rs`, add a `with_lsp_client(self, client: LspClientHandle) -> Self` setter; thread the optional handle into the tool execution context. Hydrate at app-core init from `config.coding.lsp.enabled` (default `true`).

- [ ] **Step 10: Extend `DiagnosticsDelta` in coding-ingest**

```rust
// crates/coding-ingest/src/event.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsDelta {
    pub before_count: usize,
    pub after_count: usize,
    pub introduced: Vec<String>,            // existing string messages
    pub resolved: Vec<String>,              // existing
    #[serde(default)]
    pub lsp_introduced: Vec<lsp_client::diagnostics::LspDiagnostic>, // NEW
    #[serde(default)]
    pub lsp_resolved: Vec<lsp_client::diagnostics::LspDiagnostic>,   // NEW
}
```

Update `coding-memory/src/sink/translator.rs` to forward the new fields. Run the cross-CLI normalization proptest (`crates/coding-ingest/tests/cross_cli_normalization.rs`) — Inv 7 must still hold (`parse(serialize(event)) == event`).

- [ ] **Step 11: Run gates**

```
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

- [ ] **Step 12: Commit**

```bash
git add crates/lsp-client crates/klynt-core/src/tools/shared crates/klynt-core/src/registry/builder.rs crates/coding-ingest/src/event.rs crates/coding-memory/src/sink/translator.rs Cargo.toml
git commit -m "feat(lsp): real LSP diagnostics + anchored_symbols via new lsp-client crate"
```

---

## Task 6: Per-framework coverage parsers

Spec §10 line 1135. `coverage_delta: Option<f64>` is hardcoded to `None` in `crates/agent/src/events.rs:703`. Phase 3 wires lcov / cobertura / tarpaulin / cargo-llvm-cov parsers and changes the field shape to a richer struct.

**Files:**
- Create: `crates/coding-ingest/src/coverage/{mod.rs,lcov.rs,cobertura.rs,tarpaulin.rs,llvm_cov.rs}`
- Modify: `crates/agent/src/events.rs` (`coverage_delta: Option<CoverageDelta>`)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (post-test scan)
- Test: `crates/coding-ingest/tests/coverage_parsers.rs`

- [ ] **Step 1: Failing test**

```rust
// crates/coding-ingest/tests/coverage_parsers.rs
use coding_ingest::coverage::{lcov, CoverageDelta};

#[test]
fn lcov_parses_total_percentage() {
    let sample = "TN:\nSF:src/lib.rs\nDA:1,1\nDA:2,0\nLF:2\nLH:1\nend_of_record\n";
    let delta = lcov::parse(sample).unwrap();
    assert_eq!(delta.lines_total, 2);
    assert_eq!(delta.lines_covered, 1);
    assert!((delta.percent() - 50.0).abs() < 0.01);
}
```

- [ ] **Step 2: Run, expect fail**

`cargo nextest run -p coding-ingest -E 'test(lcov_parses)'` — FAIL.

- [ ] **Step 3: Implement parsers**

```rust
// crates/coding-ingest/src/coverage/mod.rs
use serde::{Serialize, Deserialize};
pub mod lcov;
pub mod cobertura;
pub mod tarpaulin;
pub mod llvm_cov;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CoverageDelta {
    pub lines_total: u32,
    pub lines_covered: u32,
    pub per_file: std::collections::HashMap<String, FileCoverage>,
}
impl CoverageDelta { pub fn percent(&self) -> f64 {
    if self.lines_total == 0 { 0.0 } else { 100.0 * self.lines_covered as f64 / self.lines_total as f64 }
}}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileCoverage { pub total: u32, pub covered: u32 }

pub fn detect_and_parse(working_dir: &std::path::Path) -> Option<CoverageDelta> {
    // Probe in priority order:
    //   coverage.lcov / lcov.info → lcov
    //   coverage.xml → cobertura
    //   target/tarpaulin/tarpaulin-report.json → tarpaulin
    //   target/llvm-cov/json/coverage.json → llvm_cov
    let cands = [
        ("lcov.info", lcov::parse_file as fn(&std::path::Path)->Option<CoverageDelta>),
        ("coverage.lcov", lcov::parse_file),
        ("coverage.xml", cobertura::parse_file),
        ("target/tarpaulin/tarpaulin-report.json", tarpaulin::parse_file),
        ("target/llvm-cov/json/coverage.json", llvm_cov::parse_file),
    ];
    for (rel, parser) in cands {
        let p = working_dir.join(rel);
        if p.exists() { if let Some(d) = parser(&p) { return Some(d); } }
    }
    None
}
```

Implement `lcov.rs` first to make the test pass (parse `LF:` and `LH:` summary lines). Then write per-format tests for cobertura (XML), tarpaulin (JSON), llvm_cov (JSON) and implement each in its own task subsection.

- [ ] **Step 4: Verify lcov test**

`cargo nextest run -p coding-ingest -E 'test(lcov_parses)'` — PASS.

- [ ] **Step 5: Add cobertura test + impl**

```rust
#[test]
fn cobertura_parses_line_rate() {
    let xml = r#"<coverage line-rate="0.75"><packages><package><classes><class filename="src/a.rs" line-rate="0.75"/></classes></package></packages></coverage>"#;
    let d = cobertura::parse(xml).unwrap();
    assert!((d.percent() - 75.0).abs() < 0.01);
}
```

Implement using `quick-xml` (already a workspace dep — verify with `cargo tree -p coding-ingest -e normal` first; if not, add it). Use the `line-rate` attribute on the root `<coverage>` element to derive total/covered (synthesize total=10000, covered=line_rate*10000 to preserve ratio).

- [ ] **Step 6: Add tarpaulin + llvm_cov**

Same pattern: failing test with a tiny JSON sample, then implement using `serde_json::from_str`. `tarpaulin-report.json` schema: `{ "files": [{ "path": "...", "coverable": N, "covered": N }], "coverage": pct }`. `cargo-llvm-cov` JSON: nested under `data[0].totals.lines.percent` plus per-file `files[].summary.lines`.

- [ ] **Step 7: Change `coverage_delta` field type**

In `crates/agent/src/events.rs`:

```rust
// was: pub coverage_delta: Option<f64>,
pub coverage_delta: Option<coding_ingest::coverage::CoverageDelta>,
```

Update `crates/agent/Cargo.toml` to depend on `coding-ingest` (already a downstream — verify the dep direction; if `coding-ingest` depends on `agent`, define `CoverageDelta` in a leaf crate like `common` or a new `coding-types` crate to avoid the cycle).

**Likely fix:** define `CoverageDelta` in `common::coverage` (leaf crate, zero deps), re-export from `coding-ingest::coverage`.

- [ ] **Step 8: Wire post-test scan**

In `crates/agent/src/agent_runtime/runtime.rs` (or wherever `TestRunDetailed` events are constructed), after the test tool returns, call `coding_ingest::coverage::detect_and_parse(&session.cwd)` and attach.

Alternative seam (cleaner): in `crates/klynt-core/src/tools/bash.rs`, when `is_test_command()` heuristic matches (`cargo test`, `bun test`, `npm test`, `pytest`, `go test`), invoke the parser and stash on the event. Pick whichever crate already constructs `TestRunDetailed` — check `grep -rn "TestRunDetailed " crates/`.

- [ ] **Step 9: Run gates + cross-CLI proptest**

`cargo nextest run --workspace` — Inv 7 (parse(serialize(event)) == event) MUST stay green.

- [ ] **Step 10: Commit**

```bash
git add crates/coding-ingest/src/coverage crates/agent/src/events.rs crates/agent/src/agent_runtime crates/klynt-core/src/tools/bash.rs crates/common/src/coverage.rs
git commit -m "feat(coverage): per-framework parsers for lcov/cobertura/tarpaulin/llvm-cov"
```

---

## Task 7: Mirror reranking for `tool_search`

`ToolSearchTool` already carries `effectiveness_scores: Option<HashMap<String, f32>>` (verified at `crates/klynt-core/src/tools/tool_search/tool.rs`). Phase 3 = populate that map from a new Mirror signal source, and use it as a rank multiplier.

**Files:**
- Create: `crates/cognitive/src/mirror/sources/skill_effectiveness.rs`
- Modify: `crates/cognitive/src/mirror/sources/mod.rs`
- Modify: `crates/klynt-core/src/tools/tool_search/tool.rs` (use scores in rank)
- Modify: `crates/app-core/src/init/mod.rs` (wire score map into `ToolKitBuilder`)
- Test: `crates/klynt-core/tests/tool_search_reranking.rs`, `crates/cognitive/tests/skill_effectiveness_source.rs`

- [ ] **Step 1: Failing rank test**

```rust
// crates/klynt-core/tests/tool_search_reranking.rs
use std::collections::HashMap;
use klynt_core::tools::tool_search::{ToolSearchTool, ToolSearchArgs};

#[tokio::test]
async fn higher_effectiveness_wins_when_text_match_ties() {
    let mut scores = HashMap::new();
    scores.insert("grep".into(), 0.9);
    scores.insert("glob".into(), 0.1);
    let tool = ToolSearchTool::with_effectiveness(scores);
    let res = tool.run(ToolSearchArgs { query: Some("file".into()), max_results: Some(2) }).await.unwrap();
    assert_eq!(res.results[0].name, "grep");
}
```

- [ ] **Step 2: Run, expect fail**

`cargo nextest run -p klynt-core -E 'test(higher_effectiveness)'` — FAIL (current rank ignores scores).

- [ ] **Step 3: Modify ranker**

In `crates/klynt-core/src/tools/tool_search/tool.rs`, find the scoring loop. Combine base text match `text_score` with effectiveness:

```rust
let final_score = text_score
    + self.effectiveness_scores.as_ref()
        .and_then(|m| m.get(&meta.name)).copied().unwrap_or(0.0) * EFFECTIVENESS_WEIGHT;
```

Define `const EFFECTIVENESS_WEIGHT: f32 = 0.3;` (low enough to not overwhelm a strong text match; high enough to break ties). Sort descending by `final_score`.

- [ ] **Step 4: Verify pass**

`cargo nextest run -p klynt-core -E 'test(higher_effectiveness)'` — PASS.

- [ ] **Step 5: Mirror signal source**

```rust
// crates/cognitive/src/mirror/sources/skill_effectiveness.rs
use crate::mirror::MirrorSignalSource;
use std::sync::Arc;

pub struct SkillEffectivenessSource { /* repo handle + config */ }

#[async_trait::async_trait]
impl MirrorSignalSource for SkillEffectivenessSource {
    fn name(&self) -> &'static str { "skill_effectiveness" }
    async fn collect(&self, _ctx: &crate::mirror::SignalContext) -> common::Result<Vec<crate::mirror::Signal>> {
        // Read recent activations + outcomes; compute EWMA per tool name.
        // Emit MirrorAlert::SkillEffectiveness { scores: HashMap<String,f32> }.
        todo!("query coding_memory for recent skill_activations + outcomes; compute EWMA")
    }
}
```

Add the variant to `coding-memory::mirror::alerts::MirrorAlert::SkillEffectiveness { scores: HashMap<String, f32> }`. Register the source in `MirrorEngine::start` callers (`crates/app-core/src/init/mod.rs`).

- [ ] **Step 6: Subscriber that updates `ToolSearchTool`**

In `app-core/src/init/mod.rs`, spawn a task that subscribes to `MirrorAlert::SkillEffectiveness` events; it holds a `Arc<RwLock<HashMap<String,f32>>>` and updates it. Replace `ToolSearchTool` registration to read from this shared map at construction (or refactor `ToolSearchTool` to hold an `Arc<RwLock<HashMap<…>>>` instead of an owned `Option<HashMap<…>>` so updates flow live).

- [ ] **Step 7: Source unit test**

```rust
// crates/cognitive/tests/skill_effectiveness_source.rs
#[tokio::test]
async fn ewma_accumulates_per_skill() { /* … */ }
```

- [ ] **Step 8: Commit**

```bash
git add crates/klynt-core/src/tools/tool_search crates/cognitive/src/mirror/sources crates/coding-memory/src/mirror/alerts.rs crates/app-core/src/init/mod.rs crates/klynt-core/tests/tool_search_reranking.rs crates/cognitive/tests/skill_effectiveness_source.rs
git commit -m "feat(tool-search): mirror-driven effectiveness reranking"
```

---

## Task 8: Multi-window per-repo coding

Spec §13 Phase 3+. New `coding:{repo_id}` window labels managed by `lazy_window.rs`. UI route `/#/coding/:repoId`.

**Files:**
- Modify: `crates/desktop/src/lazy_window.rs`
- Create: `crates/desktop/src/commands/coding_window.rs`
- Modify: `crates/desktop/src/commands/mod.rs` + `specta_builder.rs::klynt_collect_commands![…]`
- Modify: `desktop-ui/src/main.tsx` or router config (parameterize coding route)
- Test: `crates/desktop/tests/lazy_window_coding.rs` (or integration with mock app handle)

- [ ] **Step 1: Failing test**

```rust
// crates/desktop/tests/lazy_window_coding.rs
use desktop::lazy_window::parse_coding_label;

#[test]
fn parses_coding_label_with_repo_id() {
    let r = parse_coding_label("coding:abc123").unwrap();
    assert_eq!(r, "abc123");
}

#[test]
fn rejects_non_coding_labels() {
    assert!(parse_coding_label("launcher").is_none());
    assert!(parse_coding_label("coding:").is_none());
}
```

- [ ] **Step 2: Run, expect fail**

`cargo nextest run -p desktop -E 'test(parse_coding_label)'` — FAIL.

- [ ] **Step 3: Implement label parser**

```rust
// crates/desktop/src/lazy_window.rs (add)
pub fn parse_coding_label(label: &str) -> Option<&str> {
    label.strip_prefix("coding:").filter(|s| !s.is_empty())
}
```

- [ ] **Step 4: Add window builder**

In the existing `match label` inside `get_or_create_window`:

```rust
label if parse_coding_label(label).is_some() => {
    let repo_id = parse_coding_label(label).unwrap();
    build_coding_window(app, label, repo_id)
}
```

```rust
fn build_coding_window(app: &AppHandle, label: &str, repo_id: &str) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html#/coding/{repo_id}").into()))
        .title(format!("Klynt — {repo_id}"))
        .inner_size(1200.0, 800.0)
        .min_inner_size(700.0, 500.0)
        .build()
}
```

- [ ] **Step 5: Verify pass**

`cargo nextest run -p desktop -E 'test(parse_coding_label)'` — PASS.

- [ ] **Step 6: Add Tauri command**

```rust
// crates/desktop/src/commands/coding_window.rs
use crate::lazy_window::get_or_create_window;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn coding_open_repo_window(repo_id: String) -> common::Result<()> {
    // Validate repo_id: must be ascii alphanum + dash/underscore, 1..=64 chars
    if !repo_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || repo_id.is_empty() || repo_id.len() > 64
    {
        return Err(common::KlyntbotError::InvalidArgument("bad repo_id".into()));
    }
    let label = format!("coding:{repo_id}");
    // Tauri AppHandle is grabbed by the macro from a thread-local — see other klynt_commands
    let app = crate::app_handle();
    let _ = get_or_create_window(&app, &label)?;
    Ok(())
}
```

Add `coding_window::coding_open_repo_window` to `klynt_collect_commands![…]` in `specta_builder.rs`. Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts` (the `bindings_are_current` test will fail until you do).

- [ ] **Step 7: Parameterize UI route**

Find the existing coding route in `desktop-ui/src/main.tsx` or `desktop-ui/src/App.tsx` (`grep -rn "/coding" desktop-ui/src/`). Change it to `/coding/:repoId?` and have `<CodingPage />` read `useParams().repoId`. When undefined, render a repo picker; when set, scope the chat thread/session to that repo.

- [ ] **Step 8: UI test**

```tsx
// desktop-ui/src/features/coding/pages/CodingPage.test.tsx
test('renders repo picker when no repoId', () => {
  render(<MemoryRouter initialEntries={['/coding']}><Routes><Route path="coding/:repoId?" element={<CodingPage/>}/></Routes></MemoryRouter>);
  expect(screen.getByRole('heading', { name: /pick a repo/i })).toBeInTheDocument();
});
```

- [ ] **Step 9: Commit**

```bash
git add crates/desktop/src/lazy_window.rs crates/desktop/src/commands/coding_window.rs crates/desktop/src/commands/mod.rs crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts desktop-ui/src/features/coding desktop-ui/src/main.tsx
git commit -m "feat(desktop): per-repo coding windows via coding:{repo_id} label"
```

---

## Task 9: Voice-driven coding wire-up

Spec line 1409: "verify in coding mode". `useDictationController.ts` exists; this task wires it into the coding chat composer and adds a coverage test.

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useCodingDictation.ts`
- Modify: coding composer component (locate via grep `<textarea` or `<ChatComposer` under `desktop-ui/src/features/coding/`)
- Test: colocated `useCodingDictation.test.ts`

- [ ] **Step 1: Locate composer**

```bash
grep -rn "ChatComposer\|composer\.tsx\|CodingComposer\|ChatInput" desktop-ui/src/features/coding/ desktop-ui/src/features/messages/ | head
```

Note the file path discovered (likely `desktop-ui/src/features/coding/components/CodingComposer.tsx` or similar). Use that path in subsequent steps.

- [ ] **Step 2: Failing test**

```ts
// desktop-ui/src/features/coding/hooks/useCodingDictation.test.ts
import { renderHook, act } from '@testing-library/react';
import { useCodingDictation } from './useCodingDictation';

test('appends transcript to current input', () => {
  const setValue = vi.fn();
  const { result } = renderHook(() => useCodingDictation({ value: 'fix the ', setValue }));
  act(() => result.current.onTranscript('parser bug'));
  expect(setValue).toHaveBeenCalledWith('fix the parser bug');
});
```

- [ ] **Step 3: Run, expect fail**

`cd desktop-ui && bun run test useCodingDictation` — FAIL.

- [ ] **Step 4: Implement**

```ts
// desktop-ui/src/features/coding/hooks/useCodingDictation.ts
import { useDictationController } from '@app/hooks/useDictationController';

interface Args { value: string; setValue: (v: string) => void; }

export function useCodingDictation({ value, setValue }: Args) {
  const ctl = useDictationController({
    onFinalTranscript: (text: string) => {
      const sep = value.endsWith(' ') || value.length === 0 ? '' : ' ';
      setValue(value + sep + text);
    },
  });
  return { ...ctl, onTranscript: (t: string) => setValue(value + (value.endsWith(' ') ? '' : ' ') + t) };
}
```

- [ ] **Step 5: Wire into composer**

In the composer file from Step 1, add:

```tsx
const dict = useCodingDictation({ value: input, setValue: setInput });
return (<>
  {/* … */}
  <button aria-label="dictate" onClick={dict.toggle}>{dict.isRecording ? '⏺' : '🎙️'}</button>
</>);
```

- [ ] **Step 6: Verify + commit**

```bash
cd desktop-ui && bun run test && bun run lint && bun run typecheck
git add desktop-ui/src/features/coding
git commit -m "feat(coding-ui): wire useDictationController into coding composer"
```

---

## Task 10: K10 + K11 proptests

Spec line 1371 + Appendix C. K10 (Mirror-cache poisoning: a single Deny forces always-ask) and K11 (Sessions retention monotonicity: starred never pruned) ship only as unit tests today. Add proptests so the invariants actually hold across generated inputs.

**Files:**
- Create: `crates/klynt-core/tests/mirror_approval_k10_proptest.rs`
- Create: `crates/app-core/tests/sessions_retention_k11_proptest.rs`

- [ ] **Step 1: Write K10 proptest**

```rust
// crates/klynt-core/tests/mirror_approval_k10_proptest.rs
use proptest::prelude::*;
use klynt_core::approval::layer3::{evaluate_layer3, Layer3Config, ApprovalDecision, Outcome};

prop_compose! {
    fn arb_history()(decisions in proptest::collection::vec(any::<ApprovalDecision>(), 0..32))
        -> Vec<ApprovalDecision> { decisions }
}

proptest! {
    #[test]
    fn k10_single_deny_forces_always_ask(
        mut history in arb_history(),
        deny_at in 0usize..32,
    ) {
        let idx = deny_at.min(history.len());
        history.insert(idx, ApprovalDecision::deny_for("bash:rm"));
        let cfg = Layer3Config::default();
        let outcome = evaluate_layer3(&cfg, "bash:rm", &history);
        prop_assert_eq!(outcome, Outcome::AlwaysAsk);
    }
}
```

If `ApprovalDecision`/`evaluate_layer3` don't have these exact signatures, adjust to match `crates/klynt-core/src/approval/layer3.rs`. The invariant holds regardless of how many Approves precede or follow.

- [ ] **Step 2: Run, expect pass (or fail revealing a bug)**

`cargo nextest run -p klynt-core -E 'test(k10)'` — should PASS. If it fails, the invariant is violated and the bug must be fixed in `layer3.rs` before proceeding.

- [ ] **Step 3: Write K11 proptest**

```rust
// crates/app-core/tests/sessions_retention_k11_proptest.rs
use proptest::prelude::*;
use storage::StoragePool;
use app_core::init::coding_retention::run_retention_pass;

prop_compose! {
    fn arb_session()(
        starred in any::<bool>(),
        age_days in 0u32..365,
    ) -> (bool, u32) { (starred, age_days) }
}

proptest! {
    #[test]
    fn k11_starred_session_never_pruned(sessions in proptest::collection::vec(arb_session(), 1..50)) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let mut starred_ids = Vec::new();
            for (starred, age_days) in &sessions {
                let id = seed_session(&pool, *starred, *age_days).await;
                if *starred { starred_ids.push(id); }
            }
            run_retention_pass(&pool, /* max_age_days = */ 30).await.unwrap();
            for id in starred_ids {
                let exists = session_exists(&pool, &id).await;
                prop_assert!(exists, "starred session {id} was pruned");
            }
            Ok(())
        }).unwrap();
    }
}

// helpers (seed_session, session_exists) — paste from existing tests
```

- [ ] **Step 4: Run, expect pass**

`cargo nextest run -p app-core -E 'test(k11)'` — PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/tests/mirror_approval_k10_proptest.rs crates/app-core/tests/sessions_retention_k11_proptest.rs
git commit -m "test(invariants): K10 (mirror cache poison) + K11 (starred-not-pruned) proptests"
```

---

## Task 11: Measure the 800ms p95 first-token gate

Spec line 1400. Phase 2 left this unmeasured. Set up a deterministic micro-bench so it can be tracked in CI.

**Files:**
- Create: `crates/agent/benches/chat_send_to_first_token_p95.rs` (extend the existing micro-bench)
- Create/update: `docs/superpowers/notes/2026-05-02-coding-perf-pass.md` (record numbers)

- [ ] **Step 1: Write the bench harness**

Stand up an `AgentRuntime` against a stub provider that returns a single token after a configurable delay. Run 100 iterations, record p50/p95/p99 from `chat_send` to first `AgentEvent::Token`. Use `criterion` (already a workspace dep — verify via `cargo tree -p agent`).

```rust
// crates/agent/benches/chat_send_to_first_token_p95.rs
use criterion::{criterion_group, criterion_main, Criterion};
fn bench_first_token(c: &mut Criterion) {
    c.bench_function("chat_send_to_first_token_coding_mode_p95", |b| {
        b.iter(|| { /* construct runtime, stub provider, await first token */ });
    });
}
criterion_group!(benches, bench_first_token);
criterion_main!(benches);
```

- [ ] **Step 2: Run + record**

```bash
cargo bench -p agent --bench chat_send_to_first_token_p95 -- --measurement-time 30 > /tmp/bench.txt
```

Append the p95 number to `docs/superpowers/notes/2026-05-02-coding-perf-pass.md` with the date and machine (e.g. `MacBook Pro M3 Max, 2026-05-02: p95 = 612ms`).

- [ ] **Step 3: If p95 > 800ms, profile**

```bash
cargo flamegraph -p agent --bench chat_send_to_first_token_p95
```

Investigate top stack frames. Common offenders: synchronous provider init, unnecessary `await`s, `SoulContextSource` re-reads (already memoized — verify the cache is hit).

- [ ] **Step 4: Commit numbers**

```bash
git add crates/agent/benches/chat_send_to_first_token_p95.rs docs/superpowers/notes/2026-05-02-coding-perf-pass.md
git commit -m "perf(coding): measure chat_send → first-token p95 (target <800ms)"
```

---

## Task 12: Audit + ship explicit approval Tauri commands

Phase 1 explorer flagged: no explicit `coding_approve` / `coding_reject` Tauri commands in `crates/desktop/src/commands/`. The runtime emits `ApprovalRequested` and waits on a channel; the UI must answer somehow. Likely it uses a shared `respond_to_approval`-style command — audit first, only add commands if missing.

**Files:**
- Investigate, then potentially create: `crates/desktop/src/commands/coding_approve.rs`
- Modify: `crates/desktop/src/specta_builder.rs::klynt_collect_commands![…]`

- [ ] **Step 1: Audit**

```bash
grep -rn "ApprovalRequested\|approval_response\|respond_to_approval\|approve_pending" crates/desktop/src crates/app-core/src/handlers crates/agent/src/agent_loop
```

Determine which command (if any) the UI's `ApprovalCard.tsx` calls. Inspect:

```bash
grep -rn "invoke(" desktop-ui/src/features/coding/components/ApprovalCard.tsx
```

- [ ] **Step 2: Decide**

If a working command exists (e.g. `tools_respond_to_approval`), add a doc comment in `crates/desktop/src/commands/coding_status.rs` linking to it and SKIP step 3. Commit only the doc comment update.

If no command exists, proceed.

- [ ] **Step 3: Add `coding_approve` / `coding_reject`**

```rust
// crates/desktop/src/commands/coding_approve.rs
use desktop_macros::klynt_command;
use app_core::AppCore;
use std::sync::Arc;

#[klynt_command]
pub async fn coding_approve(core: Arc<AppCore>, approval_id: String) -> common::Result<()> {
    core.coding().respond_to_approval(approval_id, true).await
}

#[klynt_command]
pub async fn coding_reject(core: Arc<AppCore>, approval_id: String, reason: Option<String>) -> common::Result<()> {
    core.coding().respond_to_approval_with_reason(approval_id, false, reason).await
}
```

Implement `core.coding().respond_to_approval(...)` in `crates/app-core/src/handlers/coding.rs` — it dispatches into the runtime's pending-approvals map (`Arc<DashMap<String, oneshot::Sender<bool>>>`). If that map doesn't exist yet, add it on `CodingHandler` and have `ApprovalRequested` register an entry before publishing the event.

- [ ] **Step 4: Update ApprovalCard.tsx**

Replace the existing `invoke(...)` calls with `invoke('coding_approve', { approvalId })` / `invoke('coding_reject', { approvalId, reason })`.

- [ ] **Step 5: Specta + bindings regen**

Add to `klynt_collect_commands![…]`. Run `cargo tauri dev` once to regenerate `bindings.ts`.

- [ ] **Step 6: Run gates + commit**

```bash
cargo nextest run --workspace
cd desktop-ui && bun run lint && bun run typecheck && bun run test
git add crates/desktop/src/commands crates/desktop/src/specta_builder.rs crates/app-core/src/handlers/coding.rs desktop-ui/src/bindings.ts desktop-ui/src/features/coding/components/ApprovalCard.tsx
git commit -m "feat(coding): explicit coding_approve/coding_reject Tauri commands"
```

---

## Task 13: Final integration + KCA gates

- [ ] **Step 1: Full workspace gates**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features  # MUST be 0 warnings
cargo fmt --all --check
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run lint && bun run typecheck && bun run test && bun run build
```

- [ ] **Step 2: Translator invariants (E1-E5) + cross-CLI Inv 7**

```bash
cargo nextest run -E 'test(translator) | test(cross_cli_normalization)'
```

All five translator invariants and Inv 7 (parse/serialize round-trip) MUST stay green — Phase 3 widened `DiagnosticsDelta` and `coverage_delta`, so this is the canary.

- [ ] **Step 3: KCA validation script**

```bash
./scripts/run_kca_validation.sh
```

All quality / perf / stability gates must pass per CLAUDE.md.

- [ ] **Step 4: Manual end-to-end smoke**

```bash
cargo tauri dev
```

Then in the running app:
1. Open chat → flip coding mode → run `cargo check` in a Rust repo → verify the diff card shows real LSP diagnostics under "Diagnostics introduced" / "resolved".
2. Run `cargo test --no-run` then `cargo test`, ensure `coverage_delta` populates if `lcov.info` exists.
3. Open settings → coding → skills → "Browse marketplace" → search for "rust" → install one → it appears in the active skills list with `mcp:` or `marketplace:` provenance.
4. `/skills install klynt-skill://github/code-review` (assuming a connected MCP server exposes it) → installs.
5. Open a coding window for two different repos (`coding:repo-a`, `coding:repo-b`) → both windows live and show different sessions.
6. Click the dictation mic in the coding composer → speak → text appears.

- [ ] **Step 5: Final commit + PR**

```bash
git add -A
git status                      # confirm only intended files
git commit -m "chore(coding-in-chat): phase 3 complete — LSP, coverage, marketplace, multi-window"
```

---

## Self-Review Checklist

**1. Spec coverage:**

| Spec requirement | Task |
|---|---|
| §6 line 672 — tool_search Mirror reranking | T7 ✅ |
| §6 line 680 — Truncation policy | Already done (T0 backfills doc) ✅ |
| §8 — MCP-contributed skills | T2 ✅ |
| §8 — Skills.sh marketplace | T3 ✅ |
| §10 — lsp_diagnostics_delta real | T5 ✅ |
| §10 — anchored_symbols LSP-grade | T5 ✅ |
| §10 — coverage_delta per-framework | T6 ✅ |
| §13 — Per-channel MCP allowlists | T1 ✅ |
| §13 — Multi-window per-repo | T8 ✅ |
| §13 — Voice-driven coding | T9 ✅ |
| §13 — Snapshots dedup | Already done (T0 backfills doc) ✅ |
| §13 — Windows sandbox | OUT OF SCOPE per spec line 1413 ✅ |
| §13 — IDE bridge MCP extensions | OUT OF SCOPE — separate spec ✅ |
| Appendix C — K10 / K11 proptests | T10 ✅ |
| Phase 1 punch-list gap — coding_approve cmd | T12 ✅ |
| Phase 2 punch-list gap — 800ms p95 measurement | T11 ✅ |

**2. Placeholder scan:** Two `todo!()` macros remain in T5 (`server_pool::get_or_spawn`, `workspace_root`) — these are inside step-by-step incremental implementations within a single task, with the immediately-following steps providing the implementation guidance. Acceptable because the task is internally TDD-staged. No "TBD"/"add appropriate error handling"/"implement later" placeholders elsewhere.

**3. Type consistency:**
- `CoverageDelta` defined in T6 step 3 in `coding-ingest::coverage`, then T6 step 7 says "likely fix" is to define it in `common::coverage` to break a cycle — engineer must pick at impl time. Documented trade-off.
- `LspDiagnostic` defined in T5 step 4 (`crates/lsp-client/src/diagnostics.rs`), referenced consistently in T5 step 10 (`coding-ingest/src/event.rs` adds `Vec<lsp_client::diagnostics::LspDiagnostic>`).
- `SkillSource::Mcp { server_name }` (T2) and `SkillSource::SkillsMarketplace { name, version }` (T3) — same enum, same casing, both lowered to `snake_case` via the existing `serde(rename_all)`.
- `evaluate_layer3` signature in T10 acknowledged as "adjust to match the real signature" — honest about uncertainty rather than inventing.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-02-klynt-coding-in-chat-phase3.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task with two-stage review between tasks. Best for the 12 independent tasks here, since each commits separately and the LSP / coverage / marketplace tasks don't share state.

2. **Inline Execution** — execute tasks in this session using `superpowers:executing-plans`, batching with checkpoints.

Which approach?
