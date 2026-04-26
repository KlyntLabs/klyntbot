# Coding-Memory Phase 6 — Game-Changers Wired Live (C1 + C2 + C3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the empty schema landed in Phase 1 into live causal-graph + symbol-grounded behavior — auto-populate `memory_causal_edges` from test/fix transitions, anchor every distilled fact to tree-sitter symbols, invalidate facts on git commits, activate `trace_causes`, and ship the Causal Graph + Stale Candidates workbench panels.

**Architecture:** Phase 6 adds **behavior modules** (`causal/`, `symbols/`, `git_invalidation.rs`) inside the existing `coding-memory` crate; introduces one new `EventKind::GitCommit` variant in `coding-ingest` (no new transport, no new tables); extends `Distiller::phase_a` with symbol extraction; extends Phase 5's `SessionEndPass` with causal-chain counting; replaces the `trace_causes` MCP stub and the `CausalContextExpander` retrieval skill body; ships a per-repo `.git/hooks/post-commit` installer mirroring `ClaudeCodeInstaller`; and adds two workbench panels backed by new `app-core` handlers + Tauri commands. Autotuner integration is scoped to making the coverage threshold injectable and emitting the correction-rate signal; the full shadow-eval training loop is deferred to Phase 8.

**Tech Stack:** Rust 1.93, `tree-sitter` 0.22 + `tree-sitter-rust` / `tree-sitter-typescript` / `tree-sitter-python`, SQLite (existing), Tauri 2 (existing), React 18 + Tailwind 4 + Biome 2 (existing), `react-flow-renderer` (NEW for graph viz, optional — fall back to a simple list if dep adds friction).

**Open-question decisions (locked):**
1. `pending_invalidations` is **not** a new table — `EventKind::GitCommit` rides `ingest_event_log` and the existing daemon-startup `drain_buffer` path replays missed commits.
2. Symbol extraction supports a **closed set** of languages: `rust`, `typescript`, `tsx`, `python`. Unknown languages → skip extraction, log `tracing::debug!("symbol_extraction.skipped lang=unknown")`, attach empty `anchored_symbols` array.
3. Causal-edge constants: `BrokeTest` confidence `0.8` (within-turn FileEdit→TestRun{failed>0}); `FixedBy` confidence `0.7` (cross-turn FixAttempt{Failure}→FixAttempt{Success} sharing `problem_hash`, within 24h window).
4. `trace_causes` subject parameter is a tagged JSON enum: `{"kind":"factId","value":"<uuid>"} | {"kind":"problemHash","value":"<hex>"}`.
5. Git hook installs **per-repo** at `<repo>/.git/hooks/post-commit` (atomic write via temp-file + rename; backs up any existing hook to `post-commit.klyntbot-backup`).
6. Autotuner gets a `coverage_threshold` field on `TrialParams` keyed by `session_type`; `RetrievalQualityProbe::threshold` reads from champion params via existing autotuner injection. The shadow-eval correction-rate computation is **out of scope for Phase 6** — Phase 6 only emits `RetrievalThresholdEvaluated{threshold, correction_rate}` domain events for Phase 8 to consume.

---

## File Structure

**New files (`crates/coding-memory/`):**
- `migrations/005_causal_metadata.sql` — adds `metadata` JSON column to `memory_causal_edges`; adds `stale_status` index on `semantic_facts.metadata`.
- `src/causal/mod.rs` — public re-exports + `CausalEdgeRepo`.
- `src/causal/detector.rs` — `CausalDetector` with `BrokeTest` + `FixedBy` heuristics.
- `src/causal/chain_counter.rs` — `ChainCounter` for ≥3-chain cluster flagging at session end.
- `src/causal/graph_walker.rs` — `trace_causes` BFS with `depth=3` cap.
- `src/symbols/mod.rs` — `SymbolExtractor` trait + `Language` enum.
- `src/symbols/tree_sitter_extractor.rs` — concrete impl wrapping `tree_sitter::Parser`.
- `src/symbols/git_status.rs` — `git_head_hash()` + `changed_files_since(parent)` thin shell-out helpers.
- `src/reforge/git_invalidation.rs` — `GitInvalidationHandler` (consumed by `EventKind::GitCommit` in daemon).
- `src/reforge/deep_symbol_validation.rs` — Phase 6.5 deep tree-sitter pass.

**New files (`crates/coding-ingest/`):**
- `src/bin/klyntbot_hook/git_post_commit_cmd.rs` — `klyntbot-hook post-commit` subcommand.

**New files (`crates/app-core/`):**
- `src/coding_memory/git_hook.rs` — `GitHookInstaller` (mirrors `ClaudeCodeInstaller` pattern).
- `src/coding_memory/causal_panel.rs` — handlers for Causal Graph Viewer panel.
- `src/coding_memory/stale_panel.rs` — handlers for Stale Candidates panel.

**New files (`crates/desktop-shared/`, `crates/desktop/`, `desktop-ui/`):**
- `crates/desktop-shared/src/commands/coding_memory_phase6.rs` — DTOs.
- New Tauri commands appended to `crates/desktop/src/commands/coding_memory.rs`.
- `desktop-ui/src/features/coding-memory/CausalGraphViewerPanel.tsx`.
- `desktop-ui/src/features/coding-memory/StaleCandidatesPanel.tsx`.
- `desktop-ui/src/features/settings/sections/GitHookInstallSection.tsx` (small section appended to `CodingCliSettings.tsx`).

**Modified files:**
- `crates/coding-ingest/src/event.rs` — add `EventKind::GitCommit { changed_files, git_hash, parent_hash }`.
- `crates/coding-ingest/src/bin/klyntbot-hook.rs` — register `post-commit` subcommand.
- `crates/coding-memory/Cargo.toml` — add `tree-sitter`, `tree-sitter-rust`, `tree-sitter-typescript`, `tree-sitter-python`.
- `crates/coding-memory/src/lib.rs` — register new modules; pub-re-export new types.
- `crates/coding-memory/src/distiller/phase_a.rs` — call `SymbolExtractor` on `FileEdit` events; attach `anchored_symbols` to fact metadata.
- `crates/coding-memory/src/distiller/fact_builder.rs` — write `git_hash` + `anchored_symbols` into metadata JSON.
- `crates/coding-memory/src/reforge/session_end.rs` — invoke `ChainCounter`.
- `crates/coding-memory/src/reforge/coding_synthesis.rs` — process `flagged_clusters` → emit `PromoteAction::PromoteToProblemSolutionPattern`.
- `crates/coding-memory/src/retrieval_skills/causal_context_expander.rs` — replace stub: query `memory_causal_edges` for real expansion.
- `crates/coding-memory/src/mcp.rs` — replace `trace_causes` stub.
- `crates/coding-memory/src/recall/probe.rs` — accept injected threshold; emit `RetrievalThresholdEvaluated` event.
- `crates/coding-memory/src/distiller/phase_a.rs` (counter) — emit `BrokeTest` causal edges inline.
- `crates/cognitive/src/services/reforge/service.rs` — register Phase 6.5 deep-validation step.
- `crates/autotuner/src/trial.rs` — add `coverage_threshold: Option<f32>` to `TrialParams`.
- `crates/bus/src/domain_events.rs` — add `RetrievalThresholdEvaluated`, `CausalEdgeInferred` variants.
- `crates/app-core/src/coding_memory/mod.rs` — register new handlers.

---

## Task 1: Migration 005 — Causal Edge Metadata + Stale Status Index

**Files:**
- Create: `crates/coding-memory/migrations/005_causal_metadata.sql`
- Modify: `crates/coding-memory/src/lib.rs` (register migration)
- Test: `crates/coding-memory/tests/migration_phase6.rs`

`★ Insight ─────────────────────────────────────`
- Phase 1 created `memory_causal_edges` but didn't include a `metadata` column. We need it to store `problem_hash` on edges so `ChainCounter` can group cheaply, and to store `git_hash` snapshots for replayability. SQLite ALTER TABLE ADD COLUMN is O(1) (just metadata change) — safe even on large tables.
- We're per-spec pre-release, so we're free to alter Phase 1's table directly rather than adding a sibling table.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing migration test**

```rust
// crates/coding-memory/tests/migration_phase6.rs
use coding_memory::coding_memory_migrations;
use storage::StoragePool;

#[tokio::test]
async fn migration_005_adds_causal_metadata_column() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    storage::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive");
    storage::run_feature_migrations(pool.inner(), &coding_memory_migrations())
        .await
        .expect("coding-memory");
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('memory_causal_edges') WHERE name='metadata'",
    )
    .fetch_one(pool.inner())
    .await
    .expect("query");
    assert_eq!(row.0, 1, "metadata column missing");
}

#[tokio::test]
async fn migration_005_is_idempotent() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    storage::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::run_feature_migrations(pool.inner(), &coding_memory_migrations()).await.unwrap();
    storage::run_feature_migrations(pool.inner(), &coding_memory_migrations()).await.unwrap();
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p coding-memory --test migration_phase6`
Expected: FAIL — `metadata column missing` (column does not exist yet).

- [ ] **Step 3: Create the migration SQL**

```sql
-- crates/coding-memory/migrations/005_causal_metadata.sql
ALTER TABLE memory_causal_edges ADD COLUMN metadata TEXT;
CREATE INDEX IF NOT EXISTS idx_causal_edges_problem_hash
  ON memory_causal_edges(json_extract(metadata, '$.problem_hash'));
CREATE INDEX IF NOT EXISTS idx_semantic_facts_stale_status
  ON semantic_facts(json_extract(metadata, '$.status'));
```

- [ ] **Step 4: Register the migration**

In `crates/coding-memory/src/lib.rs`, locate the `coding_memory_migrations()` function and append:

```rust
pub fn coding_memory_migrations() -> Vec<storage::FeatureMigration> {
    vec![
        // ... existing 001..004 entries unchanged ...
        storage::FeatureMigration {
            version: 5,
            name: "005_causal_metadata",
            sql: include_str!("../migrations/005_causal_metadata.sql"),
        },
    ]
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo nextest run -p coding-memory --test migration_phase6`
Expected: PASS (both tests).

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/migrations/005_causal_metadata.sql \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/migration_phase6.rs
git commit -m "feat(coding-memory): Phase 6 — migration 005 causal metadata + stale-status index"
```

---

## Task 2: `EventKind::GitCommit` variant + AgentEvent roundtrip

**Files:**
- Modify: `crates/coding-ingest/src/event.rs`
- Test: `crates/coding-ingest/tests/agent_event_roundtrip.rs` (extend existing)

`★ Insight ─────────────────────────────────────`
- Reusing `EventKind` for `GitCommit` instead of inventing a separate transport keeps the daemon, file-buffer fallback, and `drain_buffer` path identical for both runtime and post-commit-while-desktop-down cases. One transport, one queue, one drain — that's the cheapest "pending invalidations" we can get.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing roundtrip test**

Append to `crates/coding-ingest/tests/agent_event_roundtrip.rs`:

```rust
#[test]
fn git_commit_event_roundtrips() {
    let evt = AgentEvent::V1(AgentEventV1 {
        timestamp: jiff::Timestamp::from_second(1_700_000_000).unwrap(),
        source: AgentSource::ClaudeCode,
        repo_id: Some("github.com/user/repo".into()),
        session_id: None,
        kind: EventKind::GitCommit {
            changed_files: vec!["src/lib.rs".into(), "src/main.rs".into()],
            git_hash: "abc123".into(),
            parent_hash: Some("def456".into()),
        },
    });
    let json = serde_json::to_string(&evt).expect("ser");
    let parsed: AgentEvent = serde_json::from_str(&json).expect("de");
    assert_eq!(parsed, evt);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p coding-ingest --test agent_event_roundtrip`
Expected: FAIL — `EventKind::GitCommit` variant does not exist (compile error).

- [ ] **Step 3: Add the variant**

In `crates/coding-ingest/src/event.rs`, find `pub enum EventKind` and add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EventKind {
    // ... existing variants unchanged ...
    GitCommit {
        changed_files: Vec<String>,
        git_hash: String,
        parent_hash: Option<String>,
    },
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-ingest --test agent_event_roundtrip`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/event.rs crates/coding-ingest/tests/agent_event_roundtrip.rs
git commit -m "feat(coding-ingest): Phase 6 — EventKind::GitCommit variant"
```

---

## Task 3: Tree-sitter `SymbolExtractor` (Rust language)

**Files:**
- Modify: `crates/coding-memory/Cargo.toml`
- Create: `crates/coding-memory/src/symbols/mod.rs`
- Create: `crates/coding-memory/src/symbols/tree_sitter_extractor.rs`
- Test: `crates/coding-memory/tests/symbol_extractor.rs`

`★ Insight ─────────────────────────────────────`
- Tree-sitter parsers are heavy to construct (~ms each). We re-use `Parser::new()` per extractor instance and `set_language()` only when language changes. The `Language` value itself is cheap to clone (it's a thin wrapper over a static C ptr).
- We expose `extract(&str path, &str source)` rather than `extract(path: &Path)` to keep the I/O boundary outside the extractor — this keeps tests fast and lets the Distiller pass already-loaded post-edit content without re-reading from disk.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add tree-sitter deps**

In `crates/coding-memory/Cargo.toml` `[dependencies]`:

```toml
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-python = "0.21"
```

- [ ] **Step 2: Write the failing test**

```rust
// crates/coding-memory/tests/symbol_extractor.rs
use coding_memory::symbols::{Language, SymbolExtractor, TreeSitterSymbolExtractor};

#[test]
fn extracts_rust_function_names() {
    let mut ex = TreeSitterSymbolExtractor::new();
    let src = "pub fn alpha() {}\nfn beta(x: i32) -> i32 { x }\n";
    let syms = ex.extract("src/lib.rs", src, Language::Rust).expect("ok");
    let names: Vec<&str> = syms.iter().map(|s| s.symbol.as_str()).collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
}

#[test]
fn extracts_rust_struct_and_impl() {
    let mut ex = TreeSitterSymbolExtractor::new();
    let src = "struct Foo;\nimpl Foo { fn bar(&self) {} }\n";
    let syms = ex.extract("src/lib.rs", src, Language::Rust).expect("ok");
    let names: Vec<&str> = syms.iter().map(|s| s.symbol.as_str()).collect();
    assert!(names.contains(&"Foo"));
    assert!(names.contains(&"bar"));
}

#[test]
fn unknown_language_returns_empty_not_error() {
    let mut ex = TreeSitterSymbolExtractor::new();
    let syms = ex.extract("data.bin", "...", Language::Unknown).expect("ok");
    assert!(syms.is_empty());
}

#[test]
fn malformed_source_is_partial_not_panic() {
    let mut ex = TreeSitterSymbolExtractor::new();
    let src = "fn broken( { }\nfn ok() {}\n";
    let syms = ex.extract("src/lib.rs", src, Language::Rust).expect("ok");
    let names: Vec<&str> = syms.iter().map(|s| s.symbol.as_str()).collect();
    assert!(names.contains(&"ok"));
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo nextest run -p coding-memory --test symbol_extractor`
Expected: FAIL — module `symbols` does not exist.

- [ ] **Step 4: Implement the module**

`crates/coding-memory/src/symbols/mod.rs`:

```rust
//! Tree-sitter-backed symbol extraction for git invalidation and fact anchoring.

mod tree_sitter_extractor;

pub use tree_sitter_extractor::TreeSitterSymbolExtractor;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    Rust,
    TypeScript,
    Tsx,
    Python,
    Unknown,
}

impl Language {
    /// Detect language from file extension. Returns `Unknown` for unsupported files.
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".rs") { Self::Rust }
        else if lower.ends_with(".tsx") { Self::Tsx }
        else if lower.ends_with(".ts") { Self::TypeScript }
        else if lower.ends_with(".py") { Self::Python }
        else { Self::Unknown }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSymbol {
    pub file_path: String,
    pub symbol: String,
    pub kind: SymbolKind,
    pub start_line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind { Function, Struct, Impl, Class, Method, Other }

pub trait SymbolExtractor: Send + Sync {
    fn extract(
        &mut self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> crate::error::Result<Vec<ExtractedSymbol>>;
}
```

`crates/coding-memory/src/symbols/tree_sitter_extractor.rs`:

```rust
use super::{ExtractedSymbol, Language, SymbolExtractor, SymbolKind};
use tree_sitter::{Parser, Query, QueryCursor};

pub struct TreeSitterSymbolExtractor {
    parser: Parser,
    last_lang: Option<Language>,
}

impl TreeSitterSymbolExtractor {
    pub fn new() -> Self { Self { parser: Parser::new(), last_lang: None } }

    fn ensure_lang(&mut self, lang: Language) -> Option<&'static str> {
        if self.last_lang == Some(lang) {
            return Some(query_for(lang));
        }
        let ts_lang = match lang {
            Language::Rust => tree_sitter_rust::language(),
            Language::TypeScript => tree_sitter_typescript::language_typescript(),
            Language::Tsx => tree_sitter_typescript::language_tsx(),
            Language::Python => tree_sitter_python::language(),
            Language::Unknown => return None,
        };
        self.parser.set_language(&ts_lang).ok()?;
        self.last_lang = Some(lang);
        Some(query_for(lang))
    }
}

fn query_for(lang: Language) -> &'static str {
    match lang {
        Language::Rust => r#"
            (function_item name: (identifier) @sym) @fn
            (struct_item name: (type_identifier) @sym) @st
            (impl_item type: (type_identifier) @sym) @imp
            (function_signature_item name: (identifier) @sym) @fn
        "#,
        Language::TypeScript | Language::Tsx => r#"
            (function_declaration name: (identifier) @sym) @fn
            (class_declaration name: (type_identifier) @sym) @cl
            (method_definition name: (property_identifier) @sym) @md
        "#,
        Language::Python => r#"
            (function_definition name: (identifier) @sym) @fn
            (class_definition name: (identifier) @sym) @cl
        "#,
        Language::Unknown => "",
    }
}

impl SymbolExtractor for TreeSitterSymbolExtractor {
    fn extract(
        &mut self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> crate::error::Result<Vec<ExtractedSymbol>> {
        let Some(query_src) = self.ensure_lang(language) else {
            return Ok(Vec::new());
        };
        let Some(tree) = self.parser.parse(source, None) else { return Ok(Vec::new()); };
        let ts_lang = self.parser.language().expect("set above");
        let query = Query::new(&ts_lang, query_src)
            .map_err(|e| crate::error::CodingMemoryError::Internal(format!("ts query: {e}")))?;
        let mut cursor = QueryCursor::new();
        let bytes = source.as_bytes();
        let mut out = Vec::new();
        let sym_idx = query.capture_index_for_name("sym").unwrap_or(0);
        for m in cursor.matches(&query, tree.root_node(), bytes) {
            for cap in m.captures.iter().filter(|c| c.index == sym_idx) {
                let node = cap.node;
                if let Ok(text) = node.utf8_text(bytes) {
                    out.push(ExtractedSymbol {
                        file_path: file_path.to_string(),
                        symbol: text.to_string(),
                        kind: classify(language, m.pattern_index),
                        start_line: node.start_position().row as u32 + 1,
                    });
                }
            }
        }
        Ok(out)
    }
}

fn classify(lang: Language, pattern: usize) -> SymbolKind {
    match (lang, pattern) {
        (Language::Rust, 0) | (Language::Rust, 3) => SymbolKind::Function,
        (Language::Rust, 1) => SymbolKind::Struct,
        (Language::Rust, 2) => SymbolKind::Impl,
        (Language::TypeScript | Language::Tsx, 0) => SymbolKind::Function,
        (Language::TypeScript | Language::Tsx, 1) => SymbolKind::Class,
        (Language::TypeScript | Language::Tsx, 2) => SymbolKind::Method,
        (Language::Python, 0) => SymbolKind::Function,
        (Language::Python, 1) => SymbolKind::Class,
        _ => SymbolKind::Other,
    }
}
```

Register in `crates/coding-memory/src/lib.rs`:

```rust
pub mod symbols;
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo nextest run -p coding-memory --test symbol_extractor`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/Cargo.toml \
        crates/coding-memory/src/symbols/ \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/symbol_extractor.rs
git commit -m "feat(coding-memory): Phase 6 — tree-sitter SymbolExtractor (Rust/TS/Python)"
```

---

## Task 4: Wire `SymbolExtractor` into `Distiller::phase_a`

**Files:**
- Create: `crates/coding-memory/src/symbols/git_status.rs`
- Modify: `crates/coding-memory/src/distiller/phase_a.rs`
- Modify: `crates/coding-memory/src/distiller/fact_builder.rs`
- Test: `crates/coding-memory/tests/distiller_anchored_symbols.rs`

`★ Insight ─────────────────────────────────────`
- `git_head_hash()` shells out to `git rev-parse HEAD` instead of using `git2` — adding a libgit2 binding for one query inflates compile times disproportionately. We accept a ~3-5ms shell-out cost per turn (Distiller already runs async, and Phase A's <50ms budget covers this).
- The fact-builder writes `anchored_symbols` and `git_hash` into `metadata` JSON — this matches the Phase 1 metadata convention exactly, no new top-level columns.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

```rust
// crates/coding-memory/tests/distiller_anchored_symbols.rs
use coding_memory::distiller::test_helpers::*;
use coding_memory::symbols::Language;

#[tokio::test]
async fn phase_a_attaches_anchored_symbols_for_rust_edits() {
    let harness = DistillerHarness::new_in_memory().await;
    harness.feed_synthetic_turn_with_file_edit(
        "src/lib.rs",
        "pub fn new_function() {}\n",
        Language::Rust,
        "abc123",
    ).await;
    harness.run_phase_a().await.expect("phase a");
    let trace = harness.last_turn_trace().await;
    let metadata = trace.metadata_json();
    assert_eq!(metadata["git_hash"], "abc123");
    let symbols = metadata["anchored_symbols"].as_array().expect("array");
    assert!(symbols.iter().any(|s| s["symbol"] == "new_function"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p coding-memory --test distiller_anchored_symbols`
Expected: FAIL — `test_helpers::DistillerHarness::feed_synthetic_turn_with_file_edit` does not exist or does not yet inject symbols.

- [ ] **Step 3: Add `git_status.rs`**

`crates/coding-memory/src/symbols/git_status.rs`:

```rust
use std::path::Path;
use std::process::Command;

pub fn git_head_hash(repo_root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

pub fn changed_files(repo_root: &Path, parent: &str, head: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["diff", "--name-only", parent, head])
        .current_dir(repo_root)
        .output();
    let Ok(out) = out else { return Vec::new(); };
    if !out.status.success() { return Vec::new(); }
    String::from_utf8_lossy(&out.stdout)
        .lines().map(|l| l.to_string()).collect()
}
```

Append `pub mod git_status;` to `crates/coding-memory/src/symbols/mod.rs`.

- [ ] **Step 4: Update `phase_a.rs` to call extractor on FileEdit**

In `crates/coding-memory/src/distiller/phase_a.rs`, locate the `FileEdit` branch in the event loop and add:

```rust
EventKind::FileEdit { ref path, ref new_content, .. } => {
    let lang = crate::symbols::Language::from_path(path);
    let mut extractor = self.symbol_extractor.lock().await;
    let symbols = extractor
        .extract(path, new_content, lang)
        .unwrap_or_default();
    drop(extractor);
    let git_hash = self.repo_root.as_ref()
        .and_then(|r| crate::symbols::git_status::git_head_hash(r))
        .unwrap_or_default();
    fact_builder.attach_symbols(&symbols, &git_hash);
    // ... existing FileEdit handling ...
}
```

The `Distiller` struct must hold `symbol_extractor: Arc<tokio::sync::Mutex<TreeSitterSymbolExtractor>>` and `repo_root: Option<PathBuf>` — add these fields and initialize them in `Distiller::new(...)`.

- [ ] **Step 5: Update `fact_builder.rs`**

In `crates/coding-memory/src/distiller/fact_builder.rs`, add:

```rust
impl FactBuilder {
    pub fn attach_symbols(
        &mut self,
        symbols: &[crate::symbols::ExtractedSymbol],
        git_hash: &str,
    ) {
        self.pending_metadata
            .entry("anchored_symbols".into())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("array")
            .extend(symbols.iter().map(|s| serde_json::to_value(s).unwrap()));
        if !git_hash.is_empty() {
            self.pending_metadata.insert("git_hash".into(), git_hash.into());
        }
    }
}
```

(Where `pending_metadata: serde_json::Map<String, serde_json::Value>` is already a field — if not, add it.)

- [ ] **Step 6: Run tests to verify pass**

Run: `cargo nextest run -p coding-memory --test distiller_anchored_symbols`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/symbols/git_status.rs \
        crates/coding-memory/src/symbols/mod.rs \
        crates/coding-memory/src/distiller/phase_a.rs \
        crates/coding-memory/src/distiller/fact_builder.rs \
        crates/coding-memory/tests/distiller_anchored_symbols.rs
git commit -m "feat(coding-memory): Phase 6 — Distiller attaches anchored_symbols + git_hash"
```

---

## Task 5: Causal `BrokeTest` Detector (within-turn)

**Files:**
- Create: `crates/coding-memory/src/causal/mod.rs`
- Create: `crates/coding-memory/src/causal/detector.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/causal_detector_brokentest.rs`

`★ Insight ─────────────────────────────────────`
- "Within-turn" here means: inside a single `TurnTrace` event vector — i.e., a `FileEdit` followed in the same vector by a `TestRun{failed>0}`. This is much cheaper than scanning across `episodic_memories`; we run the detection at the same place we already build the trace.
- Confidence is fixed at 0.8 per the spec § Phase 6 boundary; we don't tune it dynamically yet — that's autotuner territory deferred to Phase 8.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

```rust
// crates/coding-memory/tests/causal_detector_brokentest.rs
use coding_memory::causal::{CausalDetector, DetectedEdge, EdgeKind};
use coding_ingest::event::EventKind;

#[test]
fn file_edit_followed_by_failing_test_emits_broketest_edge() {
    let edits_id = uuid::Uuid::new_v4();
    let test_id = uuid::Uuid::new_v4();
    let events = vec![
        (edits_id, EventKind::FileEdit {
            path: "src/lib.rs".into(),
            new_content: "fn x() {}".into(),
            old_content: "".into(),
        }),
        (test_id, EventKind::TestRun {
            command: "cargo test".into(),
            passed: 5, failed: 2, output: "FAIL".into(),
        }),
    ];
    let edges = CausalDetector::within_turn(&events, "phash_xyz");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::BrokeTest);
    assert_eq!(edges[0].from_id, edits_id);
    assert_eq!(edges[0].to_id, test_id);
    assert!((edges[0].confidence - 0.8).abs() < 1e-6);
    assert_eq!(edges[0].metadata["problem_hash"], "phash_xyz");
}

#[test]
fn passing_tests_emit_no_edge() {
    let events = vec![
        (uuid::Uuid::new_v4(), EventKind::FileEdit {
            path: "src/lib.rs".into(), new_content: "".into(), old_content: "".into(),
        }),
        (uuid::Uuid::new_v4(), EventKind::TestRun {
            command: "cargo test".into(), passed: 5, failed: 0, output: "ok".into(),
        }),
    ];
    assert!(CausalDetector::within_turn(&events, "ph").is_empty());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p coding-memory --test causal_detector_brokentest`
Expected: FAIL — module `causal` does not exist.

- [ ] **Step 3: Implement `CausalDetector`**

`crates/coding-memory/src/causal/mod.rs`:

```rust
//! Causal edge auto-detection (C1 game-changer).

mod detector;
pub use detector::{CausalDetector, DetectedEdge, EdgeKind};

pub mod chain_counter;
pub mod graph_walker;
```

`crates/coding-memory/src/causal/detector.rs`:

```rust
use coding_ingest::event::EventKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind { BrokeTest, FixedBy, RegressionOf }

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedEdge {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub kind: EdgeKind,
    pub confidence: f32,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

pub struct CausalDetector;

impl CausalDetector {
    /// Scan a single turn's event vector for FileEdit→TestRun{failed>0} transitions.
    /// Confidence locked at 0.8 per spec.
    pub fn within_turn(events: &[(Uuid, EventKind)], problem_hash: &str) -> Vec<DetectedEdge> {
        let mut last_edit: Option<Uuid> = None;
        let mut out = Vec::new();
        for (id, kind) in events {
            match kind {
                EventKind::FileEdit { .. } => last_edit = Some(*id),
                EventKind::TestRun { failed, .. } if *failed > 0 => {
                    if let Some(from) = last_edit {
                        let mut md = serde_json::Map::new();
                        md.insert("problem_hash".into(), problem_hash.into());
                        out.push(DetectedEdge {
                            from_id: from, to_id: *id,
                            kind: EdgeKind::BrokeTest, confidence: 0.8, metadata: md,
                        });
                    }
                }
                _ => {}
            }
        }
        out
    }
}
```

Add `pub mod causal;` to `crates/coding-memory/src/lib.rs`.

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test causal_detector_brokentest`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/causal/ \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/causal_detector_brokentest.rs
git commit -m "feat(coding-memory): Phase 6 — CausalDetector::within_turn (BrokeTest)"
```

---

## Task 6: Causal `FixedBy` Detector (cross-turn) + Repo persistence

**Files:**
- Modify: `crates/coding-memory/src/causal/detector.rs`
- Modify: `crates/coding-memory/src/causal/mod.rs` — add `CausalEdgeRepo`
- Test: `crates/coding-memory/tests/causal_detector_fixedby.rs`
- Test: `crates/coding-memory/tests/causal_edge_repo.rs`

`★ Insight ─────────────────────────────────────`
- "Cross-turn" requires a database query (we need historical FixAttempts). We scope the lookup to `problem_hash` matches within a 24h window — both bounds protect against runaway scans on long-lived projects.
- Persistence goes in `CausalEdgeRepo` so the Phase 1 `memory_causal_edges` table gets a typed surface, mirroring `EpisodicMemoryRepo` / `SemanticFactRepo`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/coding-memory/tests/causal_detector_fixedby.rs
use coding_memory::causal::{CausalDetector, EdgeKind};

#[tokio::test]
async fn success_after_failure_same_problem_hash_emits_fixedby() {
    let pool = test_pool().await;
    let failure_id = seed_fix_attempt(&pool, "ph_42", "Failure", -3600).await; // 1h ago
    let success_id = seed_fix_attempt(&pool, "ph_42", "Success", 0).await;
    let edges = CausalDetector::cross_turn_scan(&pool, "ph_42").await.unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::FixedBy);
    assert_eq!(edges[0].from_id, failure_id);
    assert_eq!(edges[0].to_id, success_id);
    assert!((edges[0].confidence - 0.7).abs() < 1e-6);
}

#[tokio::test]
async fn outside_24h_window_yields_nothing() { /* analogous, -100h */ }

#[tokio::test]
async fn different_problem_hash_yields_nothing() { /* analogous */ }

// + helper `test_pool()` and `seed_fix_attempt` in module scope
```

```rust
// crates/coding-memory/tests/causal_edge_repo.rs
use coding_memory::causal::CausalEdgeRepo;

#[tokio::test]
async fn insert_then_list_by_subject() {
    let pool = test_pool().await;
    let repo = CausalEdgeRepo::new(pool.clone());
    let edge = sample_edge();
    repo.insert(&edge).await.unwrap();
    let listed = repo.list_from(edge.from_id, 10).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].to_id, edge.to_id);
}

#[tokio::test]
async fn duplicate_insert_is_idempotent() { /* uses ON CONFLICT key */ }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo nextest run -p coding-memory --test causal_detector_fixedby --test causal_edge_repo`
Expected: FAIL — `cross_turn_scan` and `CausalEdgeRepo` don't exist.

- [ ] **Step 3: Implement `cross_turn_scan` and `CausalEdgeRepo`**

Append to `detector.rs`:

```rust
use storage::StoragePool;

const FIXEDBY_WINDOW_SECS: i64 = 24 * 3600;
const FIXEDBY_CONFIDENCE: f32 = 0.7;

impl CausalDetector {
    pub async fn cross_turn_scan(
        pool: &StoragePool,
        problem_hash: &str,
    ) -> crate::error::Result<Vec<DetectedEdge>> {
        // Pull last fix-attempt episodics matching the problem_hash within window.
        let rows: Vec<(uuid::Uuid, String, i64)> = sqlx::query_as(
            r#"
            SELECT id, json_extract(metadata, '$.outcome') as outcome,
                   strftime('%s', recorded_at) as ts
            FROM episodic_memories
            WHERE kind = 'fix_attempt'
              AND json_extract(metadata, '$.problem_hash') = ?
              AND strftime('%s', recorded_at) > strftime('%s','now') - ?
            ORDER BY recorded_at ASC
            "#,
        )
        .bind(problem_hash).bind(FIXEDBY_WINDOW_SECS)
        .fetch_all(pool.inner()).await
        .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;

        let mut out = Vec::new();
        let mut prev_failure: Option<uuid::Uuid> = None;
        for (id, outcome, _ts) in rows {
            match outcome.as_str() {
                "Failure" => prev_failure = Some(id),
                "Success" => if let Some(from) = prev_failure.take() {
                    let mut md = serde_json::Map::new();
                    md.insert("problem_hash".into(), problem_hash.into());
                    out.push(DetectedEdge {
                        from_id: from, to_id: id,
                        kind: EdgeKind::FixedBy,
                        confidence: FIXEDBY_CONFIDENCE,
                        metadata: md,
                    });
                },
                _ => {}
            }
        }
        Ok(out)
    }
}
```

Append to `mod.rs`:

```rust
use storage::StoragePool;

pub struct CausalEdgeRepo { pool: StoragePool }

impl CausalEdgeRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    pub async fn insert(&self, edge: &DetectedEdge) -> crate::error::Result<()> {
        let kind = serde_json::to_string(&edge.kind).unwrap().trim_matches('"').to_string();
        let md = serde_json::Value::Object(edge.metadata.clone()).to_string();
        sqlx::query(
            r#"INSERT INTO memory_causal_edges
               (id, from_id, to_id, edge_kind, confidence, inferred_at, metadata)
               VALUES (?, ?, ?, ?, ?, datetime('now'), ?)
               ON CONFLICT(from_id, to_id, edge_kind) DO NOTHING"#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(edge.from_id).bind(edge.to_id).bind(kind)
        .bind(edge.confidence).bind(md)
        .execute(self.pool.inner()).await
        .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn list_from(&self, from_id: uuid::Uuid, limit: i64)
        -> crate::error::Result<Vec<DetectedEdge>>
    { /* analogous SELECT WHERE from_id = ? LIMIT ? */ todo!("see body") }

    pub async fn list_by_problem_hash(&self, problem_hash: &str)
        -> crate::error::Result<Vec<DetectedEdge>>
    { /* SELECT WHERE json_extract(metadata,'$.problem_hash') = ? */ todo!() }
}
```

(Implement the two `todo!()` bodies fully — they're shown stubbed above for brevity but must be real `sqlx::query_as` calls returning rows mapped into `DetectedEdge`.)

Note: a unique index `(from_id, to_id, edge_kind)` must exist on `memory_causal_edges` — if not present from Phase 1, append it to `migrations/005_causal_metadata.sql`:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_causal_edges_unique
  ON memory_causal_edges(from_id, to_id, edge_kind);
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test causal_detector_fixedby --test causal_edge_repo`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/causal/ \
        crates/coding-memory/migrations/005_causal_metadata.sql \
        crates/coding-memory/tests/causal_detector_fixedby.rs \
        crates/coding-memory/tests/causal_edge_repo.rs
git commit -m "feat(coding-memory): Phase 6 — CausalDetector::cross_turn_scan + CausalEdgeRepo"
```

---

## Task 7: Wire `CausalDetector` into Distiller (BrokeTest emit) + emit `CausalEdgeInferred` event

**Files:**
- Modify: `crates/bus/src/domain_events.rs` — add `CausalEdgeInferred`
- Modify: `crates/coding-memory/src/distiller/phase_a.rs`
- Test: `crates/coding-memory/tests/distiller_emits_causal_edges.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/coding-memory/tests/distiller_emits_causal_edges.rs
#[tokio::test]
async fn distiller_persists_broketest_edge_after_failing_test() {
    let harness = DistillerHarness::new_in_memory().await;
    harness.feed_file_edit_then_failing_test().await;
    harness.run_phase_a().await.unwrap();
    let edges = harness.list_causal_edges().await;
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, coding_memory::causal::EdgeKind::BrokeTest);
}
```

- [ ] **Step 2: Run test (expect FAIL — Distiller does not yet call detector)**

Run: `cargo nextest run -p coding-memory --test distiller_emits_causal_edges`

- [ ] **Step 3: Add domain event**

In `crates/bus/src/domain_events.rs` `enum DomainEvent`:

```rust
CausalEdgeInferred {
    from_id: uuid::Uuid,
    to_id: uuid::Uuid,
    kind: String,
    confidence: f32,
},
```

- [ ] **Step 4: Wire detector into Phase A**

At the end of `phase_a.rs`'s turn-finalization (after writing the `turn_trace` episodic), add:

```rust
let edges = CausalDetector::within_turn(&self.event_buffer, &problem_hash);
for edge in &edges {
    self.causal_repo.insert(edge).await?;
    self.event_bus.publish(DomainEvent::CausalEdgeInferred {
        from_id: edge.from_id, to_id: edge.to_id,
        kind: format!("{:?}", edge.kind), confidence: edge.confidence,
    }).await;
}
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p coding-memory --test distiller_emits_causal_edges`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs \
        crates/coding-memory/src/distiller/phase_a.rs \
        crates/coding-memory/tests/distiller_emits_causal_edges.rs
git commit -m "feat(coding-memory): Phase 6 — Distiller persists BrokeTest edges + emits CausalEdgeInferred"
```

---

## Task 8: `ChainCounter` — flag ≥3-chain clusters at session end

**Files:**
- Create: `crates/coding-memory/src/causal/chain_counter.rs`
- Modify: `crates/coding-memory/src/reforge/session_end.rs`
- Test: `crates/coding-memory/tests/chain_counter.rs`

`★ Insight ─────────────────────────────────────`
- Promotion threshold is hard-coded at 3 per spec ("≥3-causal-chain promotion to ProblemSolutionPattern"). We expose it as a const for autotuner future tuning, but it's not yet a config field.
- Flagged clusters are written into a **transient** scratch table `causal_chain_flags` consumed by Phase 2.5 next nightly cycle. We don't persist them in `mirror_snippets` because they aren't user-visible; they're an internal handoff.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

```rust
// crates/coding-memory/tests/chain_counter.rs
use coding_memory::causal::chain_counter::ChainCounter;

#[tokio::test]
async fn three_chains_same_problem_hash_flag_cluster() {
    let pool = test_pool().await;
    seed_chain(&pool, "ph_a").await; // 3 BrokeTest+FixedBy chains
    seed_chain(&pool, "ph_a").await;
    seed_chain(&pool, "ph_a").await;
    let flags = ChainCounter::flag_clusters(&pool).await.unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].problem_hash, "ph_a");
    assert!(flags[0].chain_count >= 3);
}

#[tokio::test]
async fn two_chains_do_not_flag() { /* analogous */ }
```

- [ ] **Step 2: Run (expect FAIL)**

- [ ] **Step 3: Implement**

`crates/coding-memory/src/causal/chain_counter.rs`:

```rust
use storage::StoragePool;

pub const CHAIN_PROMOTION_THRESHOLD: i64 = 3;

#[derive(Debug, Clone)]
pub struct FlaggedCluster {
    pub problem_hash: String,
    pub chain_count: i64,
    pub representative_edge_ids: Vec<uuid::Uuid>,
}

pub struct ChainCounter;

impl ChainCounter {
    pub async fn flag_clusters(pool: &StoragePool)
        -> crate::error::Result<Vec<FlaggedCluster>>
    {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT json_extract(metadata,'$.problem_hash') as ph, COUNT(*) as cnt
            FROM memory_causal_edges
            WHERE json_extract(metadata,'$.promoted_at') IS NULL
            GROUP BY ph
            HAVING cnt >= ?
            "#,
        )
        .bind(CHAIN_PROMOTION_THRESHOLD)
        .fetch_all(pool.inner()).await
        .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
        let mut out = Vec::new();
        for (ph, cnt) in rows {
            let ids: Vec<(uuid::Uuid,)> = sqlx::query_as(
                "SELECT id FROM memory_causal_edges
                 WHERE json_extract(metadata,'$.problem_hash')=? LIMIT 10")
                .bind(&ph)
                .fetch_all(pool.inner()).await
                .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
            out.push(FlaggedCluster {
                problem_hash: ph,
                chain_count: cnt,
                representative_edge_ids: ids.into_iter().map(|(i,)| i).collect(),
            });
        }
        Ok(out)
    }
}
```

In `reforge/session_end.rs`, append at end of `SessionEndPass::run()`:

```rust
let clusters = crate::causal::chain_counter::ChainCounter::flag_clusters(&self.pool).await?;
self.flagged_clusters = clusters; // exposed for Phase 2.5 to consume next cycle
```

(Add a `pub flagged_clusters: Vec<FlaggedCluster>` field to `SessionEndPass`, plus an accessor.)

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test chain_counter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/causal/chain_counter.rs \
        crates/coding-memory/src/reforge/session_end.rs \
        crates/coding-memory/tests/chain_counter.rs
git commit -m "feat(coding-memory): Phase 6 — ChainCounter flags ≥3-chain clusters at session end"
```

---

## Task 9: Phase 2.5 promotion to `ProblemSolutionPattern` from flagged clusters

**Files:**
- Modify: `crates/coding-memory/src/reforge/coding_synthesis.rs`
- Test: `crates/coding-memory/tests/phase25_problem_solution_promotion.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/coding-memory/tests/phase25_problem_solution_promotion.rs
#[tokio::test]
async fn flagged_cluster_emits_problem_solution_promotion() {
    let harness = ReforgeHarness::new_in_memory().await;
    harness.seed_flagged_cluster("ph_demo", 3).await;
    let actions = harness.run_coding_synthesis().await.unwrap();
    let promo = actions.iter()
        .find(|a| matches!(a, PromoteAction::PromoteToProblemSolutionPattern { .. }))
        .expect("promotion missing");
    if let PromoteAction::PromoteToProblemSolutionPattern { problem_hash, supporting_chains, .. } = promo {
        assert_eq!(problem_hash, "ph_demo");
        assert!(supporting_chains.len() >= 3);
    }
    // verify ProceduralRule persisted
    let rule = harness.find_rule_by_problem_hash("ph_demo").await;
    assert!(rule.is_some());
    assert_eq!(rule.unwrap().metadata["supporting_chains"].as_array().unwrap().len(), 3);
}
```

- [ ] **Step 2: Run (expect FAIL)**

- [ ] **Step 3: Extend `coding_synthesis.rs`**

In `CodingSynthesisPhase::run`, after the existing LLM promotion logic, add:

```rust
let flagged = self.session_end_pass.flagged_clusters.clone();
for cluster in flagged {
    let action = PromoteAction::PromoteToProblemSolutionPattern {
        problem_hash: cluster.problem_hash.clone(),
        supporting_chains: cluster.representative_edge_ids.clone(),
        confidence: 0.85,
    };
    self.writer.promote(action.clone()).await?;
    actions.push(action);
    // mark edges as promoted to avoid re-promotion next cycle
    sqlx::query(
        "UPDATE memory_causal_edges
         SET metadata = json_set(coalesce(metadata,'{}'), '$.promoted_at', datetime('now'))
         WHERE json_extract(metadata,'$.problem_hash') = ?",
    )
    .bind(&cluster.problem_hash)
    .execute(self.pool.inner()).await?;
}
```

Add the new `PromoteAction` variant in `crates/coding-memory/src/reforge/types.rs`:

```rust
PromoteToProblemSolutionPattern {
    problem_hash: String,
    supporting_chains: Vec<uuid::Uuid>,
    confidence: f32,
},
```

(If this variant already exists from Phase 5 per the briefing, skip the enum edit and only update the writer to handle it.)

The writer impl must persist a `ProceduralRule` with `metadata.supporting_chains = supporting_chains` and `metadata.problem_hash = problem_hash`.

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test phase25_problem_solution_promotion`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/coding_synthesis.rs \
        crates/coding-memory/src/reforge/types.rs \
        crates/coding-memory/tests/phase25_problem_solution_promotion.rs
git commit -m "feat(coding-memory): Phase 6 — Phase 2.5 promotes ≥3-chain clusters to ProblemSolutionPattern"
```

---

## Task 10: `trace_causes` graph walker + MCP activation

**Files:**
- Create: `crates/coding-memory/src/causal/graph_walker.rs`
- Modify: `crates/coding-memory/src/mcp.rs`
- Test: `crates/coding-memory/tests/trace_causes.rs`

`★ Insight ─────────────────────────────────────`
- BFS with depth cap matches the spec's `depth=3` default. We track visited nodes to avoid cycles (causal graphs can loop in pathological cases — e.g. A FixedBy B and B BrokeTest A across long sessions).
- `subject` is a tagged enum so the MCP signature matches "fact UUID **or** problem_hash" without overloading a single string parameter.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

```rust
// crates/coding-memory/tests/trace_causes.rs
use coding_memory::causal::graph_walker::{TraceSubject, GraphWalker};

#[tokio::test]
async fn trace_returns_chain_within_depth_3() {
    let pool = test_pool().await;
    let (a, b, c, d) = seed_chain_a_b_c_d(&pool).await; // A→B→C→D
    let walker = GraphWalker::new(pool.clone());
    let resp = walker.trace(TraceSubject::FactId(a), 3).await.unwrap();
    assert!(resp.nodes.iter().any(|n| n.id == b));
    assert!(resp.nodes.iter().any(|n| n.id == c));
    assert!(resp.nodes.iter().any(|n| n.id == d));
    assert_eq!(resp.max_depth_reached, 3);
}

#[tokio::test]
async fn trace_handles_cycles_without_loop() {
    let pool = test_pool().await;
    seed_cycle_a_b_a(&pool).await;
    let walker = GraphWalker::new(pool.clone());
    let resp = walker.trace(TraceSubject::FactId(/* a */), 3).await.unwrap();
    assert!(resp.nodes.len() <= 2);
}

#[tokio::test]
async fn trace_by_problem_hash_returns_all_edges_in_cluster() { /* analogous */ }
```

- [ ] **Step 2: Run (expect FAIL)**

- [ ] **Step 3: Implement walker**

```rust
// crates/coding-memory/src/causal/graph_walker.rs
use std::collections::{HashSet, VecDeque};
use storage::StoragePool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TraceSubject {
    FactId(uuid::Uuid),
    ProblemHash(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalTraceResponse {
    pub root_id: Option<uuid::Uuid>,
    pub nodes: Vec<TraceNode>,
    pub edges: Vec<TraceEdge>,
    pub max_depth_reached: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceNode { pub id: uuid::Uuid, pub label: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEdge { pub from: uuid::Uuid, pub to: uuid::Uuid, pub kind: String, pub confidence: f32 }

pub struct GraphWalker { pool: StoragePool }

impl GraphWalker {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    pub async fn trace(&self, subject: TraceSubject, depth: u8)
        -> crate::error::Result<CausalTraceResponse>
    {
        let depth = depth.min(3);
        let starts: Vec<uuid::Uuid> = match &subject {
            TraceSubject::FactId(id) => vec![*id],
            TraceSubject::ProblemHash(ph) => sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT DISTINCT from_id FROM memory_causal_edges
                 WHERE json_extract(metadata,'$.problem_hash')=?")
                .bind(ph).fetch_all(self.pool.inner()).await
                .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?
                .into_iter().map(|(i,)| i).collect(),
        };
        let mut visited: HashSet<uuid::Uuid> = HashSet::new();
        let mut q: VecDeque<(uuid::Uuid, u8)> = VecDeque::new();
        let mut nodes: Vec<TraceNode> = Vec::new();
        let mut edges: Vec<TraceEdge> = Vec::new();
        let mut max_depth = 0u8;
        for s in &starts { q.push_back((*s, 0)); visited.insert(*s); }
        while let Some((cur, d)) = q.pop_front() {
            max_depth = max_depth.max(d);
            nodes.push(TraceNode { id: cur, label: self.label_for(cur).await });
            if d >= depth { continue; }
            let outs: Vec<(uuid::Uuid, String, f32)> = sqlx::query_as(
                "SELECT to_id, edge_kind, confidence FROM memory_causal_edges WHERE from_id = ?",
            ).bind(cur).fetch_all(self.pool.inner()).await
             .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
            for (to, kind, conf) in outs {
                edges.push(TraceEdge { from: cur, to, kind, confidence: conf });
                if visited.insert(to) { q.push_back((to, d + 1)); }
            }
        }
        Ok(CausalTraceResponse {
            root_id: starts.first().copied(),
            nodes, edges, max_depth_reached: max_depth,
        })
    }

    async fn label_for(&self, id: uuid::Uuid) -> String {
        // Try semantic_facts first, fallback to episodic_memories.
        sqlx::query_as::<_, (String,)>(
            "SELECT content FROM semantic_facts WHERE id=? UNION ALL
             SELECT content FROM episodic_memories WHERE id=? LIMIT 1")
            .bind(id).bind(id)
            .fetch_optional(self.pool.inner()).await.ok().flatten()
            .map(|(c,)| c).unwrap_or_else(|| format!("<{id}>"))
    }
}
```

In `crates/coding-memory/src/mcp.rs`, find the `trace_causes` stub and replace its body:

```rust
async fn trace_causes(&self, args: TraceCausesArgs) -> crate::error::Result<serde_json::Value> {
    let walker = crate::causal::graph_walker::GraphWalker::new(self.pool.clone());
    let resp = walker.trace(args.subject, args.depth.unwrap_or(3)).await?;
    Ok(serde_json::to_value(resp).unwrap())
}
```

The `TraceCausesArgs` struct (in `mcp.rs`) must hold `subject: TraceSubject` and `depth: Option<u8>`.

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test trace_causes`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/causal/graph_walker.rs \
        crates/coding-memory/src/mcp.rs \
        crates/coding-memory/tests/trace_causes.rs
git commit -m "feat(coding-memory): Phase 6 — trace_causes MCP tool live (BFS depth=3)"
```

---

## Task 11: `CausalContextExpander` retrieval skill — replace stub with live edge query

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/causal_context_expander.rs`
- Test: `crates/coding-memory/tests/causal_context_expander_live.rs`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn expander_pulls_neighbors_from_causal_graph() {
    let pool = test_pool().await;
    let (a, b, c) = seed_chain(&pool).await;
    let initial = vec![hit_with_id(a)];
    let expanded = CausalContextExpander::new(pool.clone())
        .expand(&query, initial).await.unwrap();
    let ids: Vec<_> = expanded.iter().map(|h| h.id).collect();
    assert!(ids.contains(&b));
    assert!(ids.contains(&c));
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Replace stub**

```rust
impl RetrievalSkill for CausalContextExpander {
    async fn expand(&self, _q: &Query, hits: Vec<Hit>) -> Result<Vec<Hit>> {
        let walker = crate::causal::graph_walker::GraphWalker::new(self.pool.clone());
        let mut out = hits.clone();
        let mut seen: HashSet<Uuid> = hits.iter().map(|h| h.id).collect();
        for h in hits {
            let trace = walker.trace(
                crate::causal::graph_walker::TraceSubject::FactId(h.id), 2,
            ).await?;
            for n in trace.nodes {
                if seen.insert(n.id) {
                    out.push(Hit { id: n.id, score: 0.5, content: n.label });
                }
            }
        }
        Ok(out)
    }
}
```

- [ ] **Step 4: Verify** — `cargo nextest run -p coding-memory --test causal_context_expander_live` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/retrieval_skills/causal_context_expander.rs \
        crates/coding-memory/tests/causal_context_expander_live.rs
git commit -m "feat(coding-memory): Phase 6 — CausalContextExpander pulls live edges"
```

---

## Task 12: `klyntbot-hook post-commit` subcommand

**Files:**
- Create: `crates/coding-ingest/src/bin/klyntbot_hook/git_post_commit_cmd.rs`
- Modify: `crates/coding-ingest/src/bin/klyntbot-hook.rs`
- Test: `crates/coding-ingest/tests/post_commit_cmd.rs`

`★ Insight ─────────────────────────────────────`
- Per spec: <5ms hook overhead + fire-and-forget. We send `EventKind::GitCommit` over the existing socket; if the socket isn't there, we fall back to the file buffer. Either way the hook returns 0 immediately so git never blocks.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn post_commit_cmd_sends_gitcommit_event_to_buffer_when_socket_down() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("KLYNTBOT_HOME", tmp.path());
    let repo = init_repo_with_two_commits(&tmp).await;
    klyntbot_hook::git_post_commit_cmd::run(&repo).await.unwrap();
    let buffer = read_buffer(&tmp).await;
    let events: Vec<AgentEvent> = buffer.lines()
        .map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(events.iter().any(|e| matches!(
        e, AgentEvent::V1(v) if matches!(v.kind, EventKind::GitCommit { .. })
    )));
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement**

```rust
// crates/coding-ingest/src/bin/klyntbot_hook/git_post_commit_cmd.rs
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use crate::hook_client::HookClient;
use std::path::Path;

pub async fn run(repo_root: &Path) -> anyhow::Result<()> {
    let head = run_git(repo_root, &["rev-parse", "HEAD"])?;
    let parent = run_git(repo_root, &["rev-parse", "HEAD~1"]).ok();
    let changed = run_git(repo_root, &["diff", "--name-only",
        parent.as_deref().unwrap_or("HEAD"), &head])?;
    let evt = AgentEvent::V1(AgentEventV1 {
        timestamp: jiff::Timestamp::now(),
        source: AgentSource::Git,
        repo_id: Some(scope_resolve::canonical(repo_root)?),
        session_id: None,
        kind: EventKind::GitCommit {
            changed_files: changed.lines().map(String::from).collect(),
            git_hash: head.trim().into(),
            parent_hash: parent.map(|s| s.trim().into()),
        },
    });
    HookClient::send_or_buffer(&evt).await?;
    Ok(())
}

fn run_git(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git").args(args).current_dir(repo).output()?;
    if !out.status.success() { anyhow::bail!("git {:?}", args); }
    Ok(String::from_utf8(out.stdout)?)
}
```

In `klyntbot-hook.rs`, register the subcommand:

```rust
match cli.command {
    // ... existing commands ...
    Commands::PostCommit { repo } => git_post_commit_cmd::run(&repo).await?,
}
```

Add `AgentSource::Git` variant to `crates/coding-ingest/src/event.rs` if not present.

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-ingest --test post_commit_cmd`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/bin/klyntbot_hook/git_post_commit_cmd.rs \
        crates/coding-ingest/src/bin/klyntbot-hook.rs \
        crates/coding-ingest/src/event.rs \
        crates/coding-ingest/tests/post_commit_cmd.rs
git commit -m "feat(coding-ingest): Phase 6 — klyntbot-hook post-commit subcommand"
```

---

## Task 13: `GitInvalidationHandler` — delete/stale-flag facts on GitCommit

**Files:**
- Create: `crates/coding-memory/src/reforge/git_invalidation.rs`
- Modify: `crates/coding-memory/src/sink/in_process.rs` (or wherever the Distiller dispatches `EventKind`) — route `GitCommit` to the handler.
- Test: `crates/coding-memory/tests/git_invalidation.rs`

`★ Insight ─────────────────────────────────────`
- The handler distinguishes three cases per spec error matrix: (a) symbol deleted → `valid_until = now` (bi-temporal invalidation); (b) symbol survives but file changed → `metadata.status = 'stale_candidate'`; (c) file unchanged → no-op. We only need to read the current file content for changed files (others are unchanged by definition).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn deleted_symbol_invalidates_anchored_fact() {
    let harness = GitInvalHarness::new().await;
    let fact_id = harness.seed_fact_anchored_to("src/lib.rs", "old_fn", "abc123").await;
    harness.write_file("src/lib.rs", "fn other_fn() {}\n").await;
    harness.handle_commit(vec!["src/lib.rs"], "def456", Some("abc123")).await.unwrap();
    let f = harness.fetch_fact(fact_id).await;
    assert!(f.metadata["valid_until"].is_string());
}

#[tokio::test]
async fn surviving_symbol_marked_stale_candidate() {
    let harness = GitInvalHarness::new().await;
    let fact_id = harness.seed_fact_anchored_to("src/lib.rs", "kept_fn", "abc123").await;
    harness.write_file("src/lib.rs", "fn kept_fn() { /* refactored */ }\n").await;
    harness.handle_commit(vec!["src/lib.rs"], "def456", Some("abc123")).await.unwrap();
    let f = harness.fetch_fact(fact_id).await;
    assert_eq!(f.metadata["status"], "stale_candidate");
}

#[tokio::test]
async fn unchanged_file_leaves_facts_alone() { /* analogous */ }
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement**

```rust
// crates/coding-memory/src/reforge/git_invalidation.rs
use std::path::Path;
use storage::StoragePool;
use crate::symbols::{Language, SymbolExtractor, TreeSitterSymbolExtractor};

pub struct GitInvalidationHandler {
    pool: StoragePool,
    extractor: tokio::sync::Mutex<TreeSitterSymbolExtractor>,
}

impl GitInvalidationHandler {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool, extractor: tokio::sync::Mutex::new(TreeSitterSymbolExtractor::new()) }
    }

    pub async fn handle(&self, repo_root: &Path, changed_files: &[String], head: &str)
        -> crate::error::Result<InvalidationReport>
    {
        let mut report = InvalidationReport::default();
        for file in changed_files {
            let contents = std::fs::read_to_string(repo_root.join(file)).unwrap_or_default();
            let lang = Language::from_path(file);
            let mut ex = self.extractor.lock().await;
            let live_symbols: std::collections::HashSet<String> = ex
                .extract(file, &contents, lang).unwrap_or_default()
                .into_iter().map(|s| s.symbol).collect();
            drop(ex);

            let rows: Vec<(uuid::Uuid, String)> = sqlx::query_as(
                r#"SELECT id, metadata FROM semantic_facts
                   WHERE json_extract(metadata,'$.anchored_symbols') IS NOT NULL"#,
            ).fetch_all(self.pool.inner()).await
             .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;

            for (id, md_json) in rows {
                let md: serde_json::Value = serde_json::from_str(&md_json).unwrap_or_default();
                let Some(syms) = md["anchored_symbols"].as_array() else { continue; };
                let referenced_in_file: Vec<&str> = syms.iter()
                    .filter(|s| s["file_path"].as_str() == Some(file.as_str()))
                    .filter_map(|s| s["symbol"].as_str()).collect();
                if referenced_in_file.is_empty() { continue; }
                let all_survive = referenced_in_file.iter().all(|n| live_symbols.contains(*n));
                let any_missing = referenced_in_file.iter().any(|n| !live_symbols.contains(*n));

                if any_missing {
                    sqlx::query(
                        "UPDATE semantic_facts SET metadata = json_set(metadata,
                         '$.valid_until', datetime('now'), '$.invalidated_by_commit', ?) WHERE id=?",
                    ).bind(head).bind(id).execute(self.pool.inner()).await
                     .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
                    report.invalidated += 1;
                } else if all_survive {
                    sqlx::query(
                        "UPDATE semantic_facts SET metadata = json_set(metadata,
                         '$.status','stale_candidate','$.last_seen_commit',?) WHERE id=?",
                    ).bind(head).bind(id).execute(self.pool.inner()).await
                     .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
                    report.stale_candidates += 1;
                }
            }
        }
        Ok(report)
    }
}

#[derive(Debug, Default)]
pub struct InvalidationReport { pub invalidated: usize, pub stale_candidates: usize }
```

In the daemon's event-dispatch path (search for the match on `EventKind` in the `IngestDaemon` consumer or `InProcessSink::dispatch`), add:

```rust
EventKind::GitCommit { changed_files, git_hash, .. } => {
    let repo_root = scope.resolve_root(&evt)?;
    self.git_invalidation.handle(&repo_root, &changed_files, &git_hash).await?;
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test git_invalidation`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/git_invalidation.rs \
        crates/coding-memory/src/sink/in_process.rs \
        crates/coding-memory/tests/git_invalidation.rs
git commit -m "feat(coding-memory): Phase 6 — GitInvalidationHandler (delete + stale-candidate)"
```

---

## Task 14: `GitHookInstaller` (per-repo) + Tauri commands + UI install button

**Files:**
- Create: `crates/app-core/src/coding_memory/git_hook.rs`
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`
- Modify: `desktop-ui/src/features/settings/pages/CodingCliSettings.tsx`
- Test: `crates/app-core/tests/git_hook_installer.rs`

`★ Insight ─────────────────────────────────────`
- We mirror `ClaudeCodeInstaller`'s shape: install / uninstall / diagnose. The atomic-write pattern (write to `post-commit.tmp`, fsync, rename) protects against half-installed hooks if the OS crashes mid-write.
- Existing hooks aren't overwritten silently — we back them up to `post-commit.klyntbot-backup`, matching the principle from spec §10 of "leave no broken state behind."
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn installs_atomic_post_commit_hook() {
    let repo = tempfile::tempdir().unwrap();
    init_git(&repo).await;
    let installer = GitHookInstaller::new(klyntbot_hook_binary_path());
    installer.install(repo.path()).await.unwrap();
    let hook_path = repo.path().join(".git/hooks/post-commit");
    assert!(hook_path.exists());
    let content = std::fs::read_to_string(&hook_path).unwrap();
    assert!(content.contains("klyntbot-hook post-commit"));
    let mode = std::fs::metadata(&hook_path).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "hook must be executable");
}

#[tokio::test]
async fn existing_hook_is_backed_up() {
    let repo = tempfile::tempdir().unwrap();
    init_git(&repo).await;
    std::fs::write(repo.path().join(".git/hooks/post-commit"), "#!/bin/sh\necho old\n").unwrap();
    let installer = GitHookInstaller::new(klyntbot_hook_binary_path());
    installer.install(repo.path()).await.unwrap();
    assert!(repo.path().join(".git/hooks/post-commit.klyntbot-backup").exists());
}

#[tokio::test]
async fn uninstall_restores_backup() { /* analogous */ }

#[tokio::test]
async fn diagnose_reports_missing_hook() { /* analogous */ }
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement**

```rust
// crates/app-core/src/coding_memory/git_hook.rs
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct GitHookInstaller { hook_binary: PathBuf }

impl GitHookInstaller {
    pub fn new(hook_binary: PathBuf) -> Self { Self { hook_binary } }

    pub async fn install(&self, repo_root: &Path) -> common::Result<()> {
        let hooks_dir = repo_root.join(".git/hooks");
        if !hooks_dir.exists() {
            return Err(common::KlyntbotError::Other(".git/hooks not found".into()));
        }
        let target = hooks_dir.join("post-commit");
        if target.exists() {
            let existing = std::fs::read_to_string(&target).unwrap_or_default();
            if !existing.contains("klyntbot-hook post-commit") {
                std::fs::rename(&target, hooks_dir.join("post-commit.klyntbot-backup"))?;
            }
        }
        let body = format!(
            "#!/bin/sh\n# klyntbot-managed post-commit hook\nexec {:?} post-commit --repo \"$(git rev-parse --show-toplevel)\" >/dev/null 2>&1 &\n",
            self.hook_binary,
        );
        let tmp = hooks_dir.join("post-commit.tmp");
        std::fs::write(&tmp, body)?;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
        std::fs::rename(tmp, target)?;
        Ok(())
    }

    pub async fn uninstall(&self, repo_root: &Path) -> common::Result<()> {
        let hooks_dir = repo_root.join(".git/hooks");
        let target = hooks_dir.join("post-commit");
        if target.exists() { std::fs::remove_file(&target)?; }
        let backup = hooks_dir.join("post-commit.klyntbot-backup");
        if backup.exists() { std::fs::rename(backup, target)?; }
        Ok(())
    }

    pub async fn diagnose(&self, repo_root: &Path) -> common::Result<HookStatus> {
        let target = repo_root.join(".git/hooks/post-commit");
        if !target.exists() { return Ok(HookStatus::Missing); }
        let body = std::fs::read_to_string(&target)?;
        if body.contains("klyntbot-hook post-commit") {
            Ok(HookStatus::Installed)
        } else {
            Ok(HookStatus::ConflictExternal)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HookStatus { Installed, Missing, ConflictExternal }
```

Tauri commands in `crates/desktop/src/commands/coding_memory.rs`:

```rust
#[tauri::command]
pub async fn coding_memory_install_git_hook(state: tauri::State<'_, AppCore>, repo_path: String)
    -> Result<(), String>
{ state.git_hook_installer().install(Path::new(&repo_path)).await.map_err(|e| e.to_string()) }

#[tauri::command]
pub async fn coding_memory_uninstall_git_hook(state: tauri::State<'_, AppCore>, repo_path: String)
    -> Result<(), String>
{ state.git_hook_installer().uninstall(Path::new(&repo_path)).await.map_err(|e| e.to_string()) }

#[tauri::command]
pub async fn coding_memory_diagnose_git_hook(state: tauri::State<'_, AppCore>, repo_path: String)
    -> Result<HookStatus, String>
{ state.git_hook_installer().diagnose(Path::new(&repo_path)).await.map_err(|e| e.to_string()) }
```

Append all three names to `DEV_COMMANDS` and to the dev-server route map. Add to `invoke_handler![...]` in `crates/desktop/src/lib.rs`.

UI section (append to `desktop-ui/src/features/settings/pages/CodingCliSettings.tsx`):

```tsx
function GitHookInstallSection() {
  const [repoPath, setRepoPath] = useState("");
  const { data: status, refetch } = useQuery("coding_memory_diagnose_git_hook", { repoPath }, { enabled: !!repoPath });
  const install = useMutation("coding_memory_install_git_hook");
  const uninstall = useMutation("coding_memory_uninstall_git_hook");
  return (
    <section className="space-y-3 rounded-md border border-border p-4">
      <h3 className="font-medium">Git post-commit hook</h3>
      <input value={repoPath} onChange={e => setRepoPath(e.target.value)}
             placeholder="/path/to/repo"
             className="w-full rounded border border-border bg-surface-base px-2 py-1" />
      <div className="text-sm text-muted">Status: {status ?? "—"}</div>
      <div className="flex gap-2">
        <button onClick={() => install.mutate({ repoPath }).then(() => refetch())}
                className="rounded bg-accent px-3 py-1 text-sm">Install</button>
        <button onClick={() => uninstall.mutate({ repoPath }).then(() => refetch())}
                className="rounded border border-border px-3 py-1 text-sm">Uninstall</button>
      </div>
    </section>
  );
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p app-core --test git_hook_installer && cd desktop-ui && bun run lint && bun run test --run`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/git_hook.rs \
        crates/desktop/src/commands/coding_memory.rs \
        crates/desktop-shared/src/commands/coding_memory.rs \
        crates/desktop/src/lib.rs \
        crates/app-core/tests/git_hook_installer.rs \
        desktop-ui/src/features/settings/pages/CodingCliSettings.tsx
git commit -m "feat(coding-memory): Phase 6 — GitHookInstaller + Tauri commands + Settings UI"
```

---

## Task 15: Reforge Phase 6.5 — deep symbol validation pass

**Files:**
- Create: `crates/coding-memory/src/reforge/deep_symbol_validation.rs`
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Test: `crates/coding-memory/tests/deep_symbol_validation.rs`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn deep_validation_marks_drifted_facts_needs_review() {
    let h = ReforgeHarness::new_in_memory().await;
    h.seed_fact_anchored_to("src/x.rs", "f", "old_hash").await;
    h.write_repo_file("src/x.rs", "fn f() { /* radically different */ }\n").await;
    h.run_deep_validation().await.unwrap();
    let fact = h.last_fact().await;
    assert_eq!(fact.metadata["status"], "needs_review");
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement**

```rust
// crates/coding-memory/src/reforge/deep_symbol_validation.rs
pub struct DeepSymbolValidationPass { /* pool, extractor, repo_roots */ }

impl DeepSymbolValidationPass {
    pub async fn run(&self, repo_roots: &[std::path::PathBuf]) -> crate::error::Result<()> {
        for root in repo_roots {
            let head = crate::symbols::git_status::git_head_hash(root).unwrap_or_default();
            let rows: Vec<(uuid::Uuid, String)> = sqlx::query_as(
                "SELECT id, metadata FROM semantic_facts
                 WHERE json_extract(metadata,'$.git_hash') IS NOT NULL
                   AND json_extract(metadata,'$.git_hash') <> ?",
            ).bind(&head).fetch_all(self.pool.inner()).await
             .map_err(|e| crate::error::CodingMemoryError::Db(e.to_string()))?;
            for (id, md_json) in rows {
                // re-extract symbols at current HEAD; if symbol survives, mark needs_review.
                // (Implementation: same as GitInvalidationHandler but always needs_review on survival.)
                self.flag_needs_review_if_symbols_drifted(id, &md_json, root).await?;
            }
        }
        Ok(())
    }
}
```

In `crates/cognitive/src/services/reforge/service.rs`, in `run_reforge()`, after Phase 5 (Apply) and before nightly close, add:

```rust
if let Some(handlers) = coding_phase_handlers {
    handlers.deep_symbol_validation.run(&self.repo_roots).await?;
}
```

(Phase 6.5 is a sibling of Phase 5 in the existing reforge sequence, not a wholesale insertion — and it's a no-op when `coding_phase_handlers` is None.)

- [ ] **Step 4: Verify** — `cargo nextest run -p coding-memory --test deep_symbol_validation` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/deep_symbol_validation.rs \
        crates/cognitive/src/services/reforge/service.rs \
        crates/coding-memory/tests/deep_symbol_validation.rs
git commit -m "feat(coding-memory): Phase 6.5 — deep tree-sitter validation flags drifted facts"
```

---

## Task 16: Inject `coverage_threshold` into `RetrievalQualityProbe` + emit `RetrievalThresholdEvaluated`

**Files:**
- Modify: `crates/autotuner/src/trial.rs`
- Modify: `crates/coding-memory/src/recall/probe.rs`
- Modify: `crates/bus/src/domain_events.rs`
- Test: `crates/coding-memory/tests/probe_threshold_injection.rs`

`★ Insight ─────────────────────────────────────`
- The training loop is deferred — Phase 6 only ensures the threshold is *injectable* and the *correct signal* is emitted. Phase 8 will read `RetrievalThresholdEvaluated` events and drive trial selection. This keeps Phase 6 scoped.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn probe_uses_injected_threshold() {
    let probe = RetrievalQualityProbe::with_threshold(0.6);
    let score = probe.score(&hits_with_mean_0_5_min_0_3());
    assert!(probe.is_below_threshold(score));
}

#[tokio::test]
async fn probe_emits_threshold_evaluated_event() {
    let bus = test_bus();
    let probe = RetrievalQualityProbe::new(0.5, bus.clone());
    probe.record_evaluation(0.5, /* correction_rate */ 0.2).await;
    let evts = bus.drained();
    assert!(evts.iter().any(|e| matches!(e, DomainEvent::RetrievalThresholdEvaluated { .. })));
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement**

In `crates/bus/src/domain_events.rs`:

```rust
RetrievalThresholdEvaluated { threshold: f32, correction_rate: f32, sample_size: u32 },
```

In `crates/autotuner/src/trial.rs`, add to `TrialParams`:

```rust
pub coverage_threshold: Option<f32>,
```

Update `Default` impl + serde `#[serde(default)]`.

In `crates/coding-memory/src/recall/probe.rs`:

```rust
pub struct RetrievalQualityProbe { threshold: f32, bus: Arc<DomainEventBus> }

impl RetrievalQualityProbe {
    pub fn new(threshold: f32, bus: Arc<DomainEventBus>) -> Self { Self { threshold, bus } }
    pub fn with_threshold(threshold: f32) -> Self {
        Self::new(threshold, Arc::new(DomainEventBus::noop()))
    }
    pub fn score(&self, hits: &[Hit]) -> f32 {
        if hits.is_empty() { return 0.0; }
        let mean: f32 = hits.iter().map(|h| h.score).sum::<f32>() / hits.len() as f32;
        let min: f32 = hits.iter().map(|h| h.score).fold(f32::INFINITY, f32::min);
        mean - min
    }
    pub fn is_below_threshold(&self, s: f32) -> bool { s < self.threshold }
    pub async fn record_evaluation(&self, score: f32, correction_rate: f32) {
        self.bus.publish(DomainEvent::RetrievalThresholdEvaluated {
            threshold: self.threshold, correction_rate, sample_size: 1,
        }).await;
    }
}
```

In `CodingRecallService::new`, read `champion.coverage_threshold.unwrap_or(0.5)` from the autotuner.

- [ ] **Step 4: Verify** — `cargo nextest run -p coding-memory --test probe_threshold_injection` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/autotuner/src/trial.rs \
        crates/coding-memory/src/recall/probe.rs \
        crates/bus/src/domain_events.rs \
        crates/coding-memory/tests/probe_threshold_injection.rs
git commit -m "feat(coding-memory): Phase 6 — coverage_threshold injectable + RetrievalThresholdEvaluated event"
```

---

## Task 17: Causal Graph Viewer panel (handler + Tauri command + UI)

**Files:**
- Create: `crates/app-core/src/coding_memory/causal_panel.rs`
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`
- Create: `desktop-ui/src/features/coding-memory/CausalGraphViewerPanel.tsx`
- Test: `crates/app-core/tests/causal_panel.rs`
- Test: `desktop-ui/src/features/coding-memory/__tests__/CausalGraphViewerPanel.test.tsx`

`★ Insight ─────────────────────────────────────`
- We render the graph as a simple node/edge list with manual SVG edges (or a tiny D3 force layout) rather than pulling in `react-flow` — that keeps the Phase 6 dependency surface tighter. Auto-collapse to summary at >200 nodes preserves the 60fps rendering target.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Failing handler test**

```rust
#[tokio::test]
async fn causal_panel_returns_graph_view() {
    let core = test_app_core().await;
    seed_chain(&core.pool(), "ph_a", 3).await;
    let view = core.causal_graph_view(CausalGraphArgs {
        repo: None, problem_hash: Some("ph_a".into()), depth: Some(3),
    }).await.unwrap();
    assert!(view.nodes.len() >= 3);
    assert!(!view.edges.is_empty());
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement** — handler delegates to `GraphWalker::trace`. DTOs in `desktop-shared`. Tauri command `coding_memory_causal_graph(args)`. Add to `DEV_COMMANDS`. UI panel calls `useQuery("coding_memory_causal_graph", ...)` and renders nodes positioned by problem_hash buckets, with `<line>` SVG edges colored by `edge_kind`. Implement >200 nodes summary fallback.

- [ ] **Step 4: Verify** — `cargo nextest run -p app-core --test causal_panel && cd desktop-ui && bun run test --run` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/causal_panel.rs \
        crates/desktop/src/commands/coding_memory.rs \
        crates/desktop-shared/src/commands/coding_memory.rs \
        crates/app-core/tests/causal_panel.rs \
        desktop-ui/src/features/coding-memory/CausalGraphViewerPanel.tsx \
        desktop-ui/src/features/coding-memory/__tests__/CausalGraphViewerPanel.test.tsx
git commit -m "feat(coding-memory): Phase 6 — Causal Graph Viewer workbench panel"
```

---

## Task 18: Stale Candidates panel

**Files:**
- Create: `crates/app-core/src/coding_memory/stale_panel.rs`
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Create: `desktop-ui/src/features/coding-memory/StaleCandidatesPanel.tsx`
- Test: `crates/app-core/tests/stale_panel.rs`
- Test: `desktop-ui/src/features/coding-memory/__tests__/StaleCandidatesPanel.test.tsx`

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn stale_panel_lists_candidates_only() {
    let core = test_app_core().await;
    seed_fact(&core.pool(), "fact-stale", "stale_candidate").await;
    seed_fact(&core.pool(), "fact-ok", "active").await;
    let view = core.stale_candidates(StaleArgs { repo: None }).await.unwrap();
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].fact_id_label, "fact-stale");
}

#[tokio::test]
async fn invalidate_now_action_marks_fact_invalid() { /* analogous */ }

#[tokio::test]
async fn keep_action_clears_stale_status() { /* analogous */ }
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement** — handler queries `WHERE json_extract(metadata,'$.status')='stale_candidate'`. Three actions: `invalidate_now` (sets `valid_until = now`), `keep` (clears `status` field), `review_later` (no-op, just snoozes). UI panel: list with three buttons per row.

- [ ] **Step 4: Verify** — `cargo nextest run -p app-core --test stale_panel && bun run test --run` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/stale_panel.rs \
        crates/desktop/src/commands/coding_memory.rs \
        crates/app-core/tests/stale_panel.rs \
        desktop-ui/src/features/coding-memory/StaleCandidatesPanel.tsx \
        desktop-ui/src/features/coding-memory/__tests__/StaleCandidatesPanel.test.tsx
git commit -m "feat(coding-memory): Phase 6 — Stale Candidates workbench panel"
```

---

## Task 19: Property tests (Invariant 7 + Invariant 8)

**Files:**
- Create: `crates/coding-memory/tests/prop_no_dangling_causal_edges.rs`
- Create: `crates/coding-ingest/tests/prop_post_commit_roundtrip.rs`

`★ Insight ─────────────────────────────────────`
- Invariant 8 ("no dangling causal edges") is the most important Phase 6 property — we built two heuristics that write to `memory_causal_edges` and any of them dropping a referent (e.g., the originating episodic ID being garbage-collected) would silently corrupt the trace.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing properties**

```rust
// crates/coding-memory/tests/prop_no_dangling_causal_edges.rs
proptest! {
    #[test]
    fn every_edge_references_existing_rows(events in arb_event_sequence()) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let pool = test_pool().await;
            let harness = DistillerHarness::with_pool(pool.clone()).await;
            harness.feed_events(events).await;
            harness.run_phase_a().await.unwrap();
            let dangling: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM memory_causal_edges e
                   WHERE NOT EXISTS (SELECT 1 FROM episodic_memories WHERE id = e.from_id)
                      OR NOT EXISTS (SELECT 1 FROM episodic_memories WHERE id = e.to_id)"#,
            ).fetch_one(pool.inner()).await.unwrap();
            prop_assert_eq!(dangling, 0);
            Ok(())
        }).unwrap();
    }
}
```

```rust
// crates/coding-ingest/tests/prop_post_commit_roundtrip.rs
proptest! {
    #[test]
    fn post_commit_event_serde_roundtrip(
        files in proptest::collection::vec("[a-z/]{1,40}\\.rs", 0..10),
        head in "[0-9a-f]{40}",
        parent in proptest::option::of("[0-9a-f]{40}"),
    ) {
        let evt = AgentEvent::V1(AgentEventV1 {
            timestamp: jiff::Timestamp::now(),
            source: AgentSource::Git,
            repo_id: Some("r".into()), session_id: None,
            kind: EventKind::GitCommit { changed_files: files, git_hash: head, parent_hash: parent },
        });
        let s = serde_json::to_string(&evt).unwrap();
        let back: AgentEvent = serde_json::from_str(&s).unwrap();
        prop_assert_eq!(back, evt);
    }
}
```

- [ ] **Step 2: Run** — both should already pass given prior tasks; this confirms the invariants empirically over many cases.

Run: `cargo nextest run -p coding-memory --test prop_no_dangling_causal_edges && cargo nextest run -p coding-ingest --test prop_post_commit_roundtrip`
Expected: PASS (proptest 256 cases each).

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/tests/prop_no_dangling_causal_edges.rs \
        crates/coding-ingest/tests/prop_post_commit_roundtrip.rs
git commit -m "test(coding-memory): Phase 6 — property tests Invariant 7 + Invariant 8"
```

---

## Task 20: Integration scenarios + exit-gate scenario

**Files:**
- Create: `tests/integration/coding_memory_phase6_scenario.rs` (in root facade)
- Create: `tests/fixtures/coding/repo_evolution.git.tar` (tarball of a tiny git repo with 3 commits — pre-built)

- [ ] **Step 1: Write failing scenario tests**

```rust
// tests/integration/coding_memory_phase6_scenario.rs

/// Spec exit gate: "commit deleting a function invalidates anchored facts within 1 minute"
#[tokio::test]
async fn exit_gate_commit_invalidates_within_one_minute() {
    let env = TestEnv::new_with_seeded_repo("repo_evolution.git.tar").await;
    let fact_id = env.distill_session_anchored_to("foo_fn").await;
    env.commit_deleting_symbol("foo_fn").await;
    env.invoke_post_commit_hook().await;
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let f = env.fetch_fact(fact_id).await;
            if f.metadata["valid_until"].is_string() { break; }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }).await.expect("invalidation within 60s");
}

/// Spec exit gate: "trace_causes returns meaningful chain on seeded data"
#[tokio::test]
async fn exit_gate_trace_causes_returns_meaningful_chain() {
    let env = TestEnv::new().await;
    env.seed_chain_a_b_c_d().await;
    let resp = env.mcp_call("trace_causes", json!({
        "subject": {"kind":"factId","value": env.root_id().to_string()},
        "depth": 3
    })).await;
    let parsed: CausalTraceResponse = serde_json::from_value(resp).unwrap();
    assert!(parsed.nodes.len() >= 4);
    assert!(parsed.edges.iter().any(|e| e.kind == "BrokeTest"));
}

#[tokio::test]
async fn end_to_end_session_to_promotion() {
    let env = TestEnv::new().await;
    // simulate 3 distinct fix sessions on the same problem_hash
    for _ in 0..3 { env.run_synthetic_fix_session("ph_demo").await; }
    env.run_session_end_pass().await;
    env.run_nightly_reforge().await;
    let rules = env.list_procedural_rules().await;
    assert!(rules.iter().any(|r| r.metadata["problem_hash"] == "ph_demo"));
}
```

- [ ] **Step 2: Run (FAIL initially if helper API missing; iterate to make tests compile and pass)**

Run: `cargo nextest run -E 'test(coding_memory_phase6_scenario)'`
Expected: PASS after helper bring-up.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/coding_memory_phase6_scenario.rs \
        tests/fixtures/coding/repo_evolution.git.tar
git commit -m "test(coding-memory): Phase 6 — exit-gate scenarios + e2e promotion"
```

---

## Task 21: Final verification

- [ ] **Step 1: Workspace build clean**

Run: `cargo build --workspace`
Expected: builds with no errors.

- [ ] **Step 2: All tests pass**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: 0 failures.

- [ ] **Step 3: Clippy clean**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 5: Frontend lint + tests**

Run: `cd desktop-ui && bun run lint && bun run test --run`
Expected: clean.

- [ ] **Step 6: Manual exit-gate smoke**

In a real terminal, in a scratch git repo:
```bash
KLYNTBOT_HOME=$(mktemp -d) cargo run --bin klyntbot -- &
cargo run --bin klyntbot-mcp -- tools --schema trace_causes
# verify schema is non-stub
```

- [ ] **Step 7: Tag commit**

```bash
git commit --allow-empty -m "chore(coding-memory): Phase 6 complete — game-changers (C1+C2+C3) live"
```

---

## Spec coverage map

| Spec § Phase 6 deliverable | Task |
|---|---|
| C1 causal edge auto-detection (test pass→fail) | 5, 7 |
| C1 FixAttempt↔TestRun correlation | 6, 7 |
| C2 tree-sitter symbol extraction at Distiller | 3, 4 |
| C2 git post-commit hook via desktop UI install | 12, 14 |
| C2 Reforge deep symbol validation pass | 15 |
| `trace_causes` non-stub | 10 |
| `anchored_symbols` populated on every write | 4 |
| ≥3-causal-chain promotion to ProblemSolutionPattern | 8, 9 |
| Exit gate: commit→invalidation <1 min | 20 |
| Exit gate: `trace_causes` meaningful chain | 20 |
| Workbench: Causal Graph Viewer | 17 |
| Workbench: Stale Candidates | 18 |
| Autotuner coverage-threshold (Phase 6 scope) | 16 |
| Property: Invariant 7 (event roundtrip) | 19 |
| Property: Invariant 8 (no dangling edges) | 19 |
| Integration: ~20 unit + 5 integration + 2 property + 1 scenario | 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 19, 20 |

## Out-of-scope (deferred)

- Full autotuner shadow-eval training loop for `coverage_threshold` → **Phase 8 hardening**.
- Multi-CLI git hook installation (codex, kimi-cli, opencode workflows) → **Phase 7**.
- Performance benchmarks (<200ms per 1KB Rust file extraction) → **Phase 8 hardening**.
