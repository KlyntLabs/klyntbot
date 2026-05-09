# Coding Background Bash — Execution Intelligence Design (Phase 2.3b)

**Date:** 2026-05-09
**Status:** Spec — ready for implementation plan
**Phase:** 2.3b of the long-running-task roadmap (`docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`)
**Companion docs:**
- `docs/superpowers/specs/2026-05-08-coding-background-bash-design.md` (Phase 2.3a — the foundation this builds on)
- `docs/superpowers/specs/2026-05-08-coding-plan-mode-design.md` (Phase 2.2 — `DynamicInjector` scaffold reused)
- `docs/superpowers/specs/2026-05-07-coding-todowrite-design.md` (Phase 2.1 — `TodoRepo`, `TodoSignalSource` patterns reused)
- `docs/superpowers/specs/2026-04-29-klynt-cognitive-architecture-design.md` (KCA — `EpisodicMemory`, `MirrorSignalSource`)

---

## 1. Goal & Scope

Phase 2.3a shipped "the LLM can run things in the background and get notified on completion." Phase 2.3b closes the cognitive loop: the LLM should *learn from* what those background runs produce, *recognize* that a TodoItem looks like a verification step, and *remember* what failed last time. None of this adds new tool surface — every new behavior enriches what 2.3a already publishes.

**2.3b explicit scope:**

1. **Plan-mode auto-affordance** — new `ExecutionIntelligenceInjector : DynamicInjector` (third injector in `InjectorRegistry`, alongside `PlanModeInjector` and `BackgroundJobsInjector`) that, when plan mode is active, scans active TodoItems for verification verbs (Run/Test/Check/Verify/Build) and renders a one-line affordance per match: `"TodoItem 'Run integration tests' looks like a background-bash candidate after /plan-exit."`
2. **Output diffing across runs** — `JobCompleted` looks up the most recent prior completion of the same normalized command (via new `command_key TEXT` column + sha256 + composite index) and embeds a structured diff into the existing completion `<system-reminder>` body. Diff covers `failure_kind` transitions, structured `failure_extracted` deltas, and (when applicable) per-test-name new/still-failing/resolved sets.
3. **Episodic memory of past runs** — new `BackgroundJobSignalSource : MirrorSignalSource` subscribes to `DomainEvent::BashJob(_)` (via the currently-`None` arm in `app-core::init::ai_pipeline::translate`), writes one `EpisodicMemory` row per terminal-state job, with `kind="bash_job"`, `domain="coding"`, importance scaled by failure interest. Searchable via the existing `MemoryRetriever::fetch_episodes` BM25 path.

**Enabling extension:**

- `GateClassifier` is extended to capture **all** failed test names (not just the first), enabling rich diffs. The `failure_extracted` JSON shape for `TestFailure` becomes `{ failed_test_names: Vec<String>, n_failed, n_passed, n_ignored }`. Other `FailureKind` shapes are unchanged.

**2.3b explicit non-goals:**

- LLM-driven cross-job pattern distillation (e.g. "test X fails when commit-set Y is present") — deferred to a later phase. Raw episodes are searchable; pattern recognition happens at retrieval time, not write time.
- Output diff prose summarization via LLM — purely structural diff in 2.3b.
- Trend dashboards / aggregate snapshots in `mirror_*` tables — `BackgroundJobSignalSource` writes only to `episodic_memories`. There is no `mirror_bash_job_snapshots` table; `flush_interval_secs: None`.
- New tool surface — no new `coding_task_*` tools. The LLM consumes everything via the existing reminder/completion-update channels.
- Cross-session episodic correlation — episodes carry `scope_id = session_id` like all coding episodes; the cognitive layer's existing scope rules govern visibility.
- Frontend / `JobsPanel` changes — the UI already shows everything it needs to from 2.3a.

---

## 2. Architecture Overview

The 2.3b changes touch **two new wires** in the agent runtime: register `ExecutionIntelligenceInjector` in `InjectorRegistry`, and register `BackgroundJobSignalSource` in `MirrorEngine`. Everything else is internal to `feature-coding-bash` or to the cognitive layer. The single existing wiring gap that lets the cognitive layer in is `app-core/src/init/ai_pipeline.rs::translate()`, which currently returns `None` for `DomainEvent::BashJob(_)` (it falls through to the `_ => None` arm).

```
L4 (extended)  feature-coding-bash/
                   src/lib.rs                       # exports ExecutionIntelligenceInjector
                   src/intelligence/                # NEW submodule
                       mod.rs
                       injector.rs                  # ExecutionIntelligenceInjector
                       normalize.rs                 # command_key (trim + collapse + strip-env, sha256)
                       diff.rs                      # JobDiff + diff_against_prior
                       verification_match.rs        # is_verification_verb(title) classifier
                   src/gate.rs                      # EXTENDED: captures all failed test names
                   src/supervisor.rs                # EXTENDED: finalize_job queries prior + embeds diff
                   src/render.rs                    # EXTENDED: completion body renders diff section
                   src/migrations.rs                # EXTENDED: adds command_key column + index
                   tests/intel_*.rs                 # 10 new integration tests

L4 (NEW source)  cognitive/src/mirror/sources/coding_bash.rs
                   BackgroundJobSignalSource : MirrorSignalSource
                   subscribed_kinds: ["BashJob.Completed", "BashJob.Failed",
                                      "BashJob.Cancelled", "BashJob.Lost"]

L4 (modified)  app-core/src/init/ai_pipeline.rs    # translate() arm for BashJob → AiSignal
L4 (modified)  app-core/src/init/mod.rs            # register both new components

L2 (extended)  storage/src/repos/coding_background_jobs.rs
                   BashJobRepo::find_prior_by_command_key
                   BashJobRow gains command_key: String

L1 (small additive)  bus/src/domain_events.rs
                   BashJobEvent gains 3 accessor methods (job_id, thread_id, agent_id)
                   No new variants or fields — read-only API extension

L1 (no change) tools-core — surface stable from 2.3a
```

**Key design choices:**

- **Single `intelligence` submodule, not a sibling crate.** Mirrors the 2.1/2.2/2.3a "feature crate owns its layer" convention. Splitting would break the established pattern for ~600 LOC of net-new code.
- **`ExecutionIntelligenceInjector` is a new third injector**, not an extension of `BackgroundJobsInjector`. Separates concerns: the existing injector renders running jobs, the new one renders cross-feature affordances. Both can be reasoned about and tested independently.
- **Per-job episodic write, no aggregation.** `BackgroundJobSignalSource` skips the `MirrorSignalSource::flush` loop (`flush_interval_secs: None`). Each terminal `BashJobEvent` produces exactly one `EpisodicMemory` insert. Searchable individually via BM25.
- **Diff is computed on demand, never persisted.** `JobDiff` exists only in the renderer's stack frame. The episodic memory carries the raw current state; if the cognitive layer wants to compare, it queries prior episodes.
- **Translator arm uses the bus's existing `BashJobEvent` shape.** No new `DomainEvent` variants. The `AiSignal` carries only `job_id`, `thread_id`, `agent_id`; `BackgroundJobSignalSource::accumulate` re-reads the row to get `failure_extracted`. Keeps the bus message lean.

---

## 3. Tool Surface

**No new tools in 2.3b.** All consumer-facing behavior is delivered through:
- The existing `<system-reminder>` body emitted by `JobSupervisor::finalize_job` — now with diff section appended when prior runs exist.
- The existing per-iteration injector pull — `ExecutionIntelligenceInjector::collect` joins the registry alongside `PlanModeInjector` and `BackgroundJobsInjector`.
- The existing cognitive retrieval path — `MemoryRetriever::fetch_episodes` returns bash-job episodes via BM25 like any other episode.

The four 2.3a tools (`bash`, `coding_task_list`, `coding_task_output`, `coding_task_stop`) are unchanged in signature, schema, and approval class.

---

## 4. Data Model

### 4.1 SQLite schema extension

Single addition to `coding_background_jobs` (pre-release; per `CLAUDE.md` we extend the existing `FeatureMigration` in-place rather than adding migration #2):

```sql
-- crates/feature-coding-bash/src/migrations.rs (FeatureMigration { version: 2, ... })

CREATE TABLE coding_background_jobs (
    id                    TEXT PRIMARY KEY,
    session_id            TEXT NOT NULL,
    agent_id              TEXT NOT NULL,
    description           TEXT NOT NULL,
    command               TEXT NOT NULL,
    command_key           TEXT NOT NULL,            -- NEW: sha256 hex of normalized command
    cwd                   TEXT NOT NULL,
    timeout_ms            INTEGER NOT NULL,
    silent_completion     INTEGER NOT NULL DEFAULT 0,
    status                TEXT NOT NULL,
    exit_code             INTEGER,
    failure_kind          TEXT,
    failure_detail        TEXT,
    failure_extracted     TEXT,
    started_at            TEXT NOT NULL,
    finished_at           TEXT,
    total_bytes_emitted   INTEGER NOT NULL DEFAULT 0,
    bisect_count          INTEGER NOT NULL DEFAULT 0,
    log_path              TEXT NOT NULL,
    final_path            TEXT,
    last_polled_at        TEXT,
    last_seen_offset      INTEGER NOT NULL DEFAULT 0,

    CHECK (status IN ('Starting','Running','Completed','Failed','Cancelled','Lost')),
    CHECK (failure_kind IS NULL OR status IN ('Failed','Cancelled','Lost')),
    FOREIGN KEY (session_id) REFERENCES coding_sessions(id) ON DELETE CASCADE
);

CREATE INDEX idx_cbj_session_status ON coding_background_jobs(session_id, status);
CREATE INDEX idx_cbj_active        ON coding_background_jobs(status) WHERE status IN ('Starting','Running');
CREATE INDEX idx_cbj_session_command_key                                             -- NEW
    ON coding_background_jobs(session_id, command_key, started_at DESC);
```

The new index is composite — the diff-lookup query is

```sql
SELECT * FROM coding_background_jobs
WHERE session_id = ? AND command_key = ? AND id != ?
  AND status IN ('Completed','Failed','Cancelled')
ORDER BY started_at DESC
LIMIT 1;
```

So `(session_id, command_key, started_at DESC)` is a covering index for that exact shape.

`command_key` is non-nullable. For pre-release, dropping and recreating the table in the new migration is fine. Post-release this becomes `ALTER TABLE coding_background_jobs ADD COLUMN command_key TEXT NOT NULL DEFAULT ''` plus a backfill script — out of scope here.

### 4.2 `BashJobRow` extension

```rust
// crates/storage/src/repos/coding_background_jobs.rs (extended)

pub struct BashJobRow {
    // ... all existing 2.3a fields ...
    pub command_key: String,    // NEW; sha256 hex (64 chars)
}

impl BashJobRepo {
    /// Most recent terminal-state job in this session with the same command_key,
    /// excluding `exclude_id`. Returns None if no prior run exists. Excludes Lost
    /// status — Lost runs lack reliable final output to diff against.
    pub async fn find_prior_by_command_key(
        &self,
        session_id: &str,
        command_key: &str,
        exclude_id: &str,
    ) -> Result<Option<BashJobRow>>;
}
```

All other `BashJobRepo` methods (`upsert`, `get`, `list_for_session`, etc.) keep their 2.3a signatures; their bodies extend to include `command_key` in the row mapping.

### 4.3 Extended `failure_extracted` schema for `TestFailure`

The 2.3a shape captured only the headline test name (`test_name: String`). 2.3b extends:

```jsonc
// FailureKind::TestFailure (cargo / nextest)
{
    "failed_test_names": [
        "tests::session_persistence::reload_active_thread",
        "tests::session_persistence::reload_orphan",
        "tests::concurrent_writes::write_then_read"
    ],
    "n_failed": 3,
    "n_passed": 17,
    "n_ignored": 1
}

// FailureKind::TestFailure (vitest / jest)
{
    "failed_test_names": [
        "JobsPanel > renders empty state",
        "JobsPanel > updates row on tauri event"
    ],
    "n_failed": 2,
    "n_passed": 11
}

// FailureKind::TestFailure (pytest)
{
    "failed_test_names": [
        "test_session::test_reload_active",
        "test_session::test_reload_orphan"
    ],
    "n_failed": 2,
    "n_passed": 5
}
```

The headline `test_name` field is dropped — `failed_test_names[0]` replaces it. (Pre-release; no consumers to migrate.) Other `FailureKind` shapes (`CompileError`, `LintFailure`, `NetworkBindFailure`, `Timeout`, `Cancelled`, `Lost`, `Other`) are unchanged.

### 4.4 New types in `feature-coding-bash::intelligence`

```rust
// crates/feature-coding-bash/src/intelligence/diff.rs

pub struct JobDiff {
    pub kind_transition:   KindTransition,
    pub extracted_diff:    ExtractedDiff,
    pub elapsed_delta_ms:  i64,                          // signed: negative = faster
}

pub enum KindTransition {
    StillPassing,                                        // both Passed
    StillFailing { kind: FailureKind },                  // both Failed with same kind
    Regressed { from: FailureKind, to: FailureKind },    // from passed or different failure
    Recovered { prior_kind: FailureKind },               // prior Failed → current Passed
    Changed { from: FailureKind, to: FailureKind },      // both failed, different kinds
}

pub enum ExtractedDiff {
    None,                                                // both Passed or no extracted JSON
    TestSet {
        new_failures:  Vec<String>,                      // in current but not prior
        still_failing: Vec<String>,                      // intersection
        resolved:      Vec<String>,                      // in prior but not current
    },
    Compile { same_location: bool, prior_loc: Option<Location>, curr_loc: Option<Location> },
    Bind    { same_port: bool, prior_port: Option<u16>, curr_port: Option<u16> },
    Lint    { delta_n_errors: i64 },
    Timeout { prior_ms: u64, curr_ms: u64 },
    OtherExitTransition { from: Option<i32>, to: Option<i32> },
}

pub struct Location { pub file: String, pub line: u32 }
```

`JobDiff` is constructed exclusively by `diff::diff_against_prior(prior: &BashJobRow, curr: &BashJobRow) -> JobDiff` and handed to `render::completion_body`. Never persisted.

### 4.5 New types in `intelligence::verification_match`

```rust
pub struct VerificationCandidate {
    pub todo_id:       String,
    pub title:         String,
    pub matched_verb:  VerificationVerb,
}

pub enum VerificationVerb { Run, Test, Check, Verify, Build }

pub fn classify(title: &str) -> Option<VerificationVerb>;
```

Implementation: case-insensitive match against the title's leading word. One verb per item; first match wins. No-op for titles shorter than 3 chars.

### 4.6 Episodic memory write shape

For each terminal-state `BashJobEvent`, `BackgroundJobSignalSource::accumulate` constructs:

```rust
EpisodicMemory {
    id:             ulid_new(),
    domain:         "coding".to_string(),
    kind:           Some("bash_job".to_string()),
    content:        serde_json::json!({
        "job_id":            job_id,
        "command":           command,
        "command_key":       command_key,
        "description":       description,
        "status":            status,                // "Completed"|"Failed"|"Cancelled"|"Lost"
        "exit_code":         exit_code,
        "elapsed_ms":        elapsed_ms,
        "failure_kind":      failure_kind,         // null when Passed
        "failure_extracted": extracted,            // the JSON object from gate.rs
    }).to_string(),
    summary:        Some(render_episode_summary(...)),  // one sentence; ≤ 160 chars
    importance:     importance_for(status, failure_kind),
    occurred_at:    finished_at,
    recorded_at:    now(),
    stability:      1.0,
    last_accessed:  None,
    access_count:   0,
    project_id:     None,
    scope_type:     "session".to_string(),
    scope_id:       Some(session_id),
    scope_repo_id:  None,                          // populated by coding-memory if cwd is in a repo
    metadata:       Some(serde_json::json!({
        "agent_id":  agent_id,
        "thread_id": thread_id,
    }).to_string()),
    actor_id:       Some(agent_id),
    tier:           "raw".to_string(),
    parent_id:      None,
    child_count:    0,
    rolled_up_at:   None,
}
```

`importance_for`:

| Terminal status | importance |
|---|---|
| `Failed` | 0.7 |
| `Lost`   | 0.6 |
| `Cancelled` | 0.5 |
| `Completed` (exit_code 0) | 0.3 |

The 0.3 floor for passes is intentional — enough that "this command ran 50 times and passed" stays retrievable, low enough that BM25 ranking pushes failures to the top.

`summary` examples:
- `"TestFailure in cargo nextest run -p agent — 3 failed (reload_active_thread, reload_orphan, concurrent_writes)"`
- `"Passed bun run test -- JobsPanel in 4.2s"`
- `"Cancelled bun run dev after 1h 8m (reason: stale build)"`

---

## 5. Components

### 5.1 `ExecutionIntelligenceInjector`

```rust
// crates/feature-coding-bash/src/intelligence/injector.rs

pub struct ExecutionIntelligenceInjector {
    todo_repo:  Arc<TodoRepo>,
    bash_repo:  Arc<BashJobRepo>,                    // for future "recent failure recall" body
    supervisor: Arc<dyn JobSupervisorHandle>,        // to skip already-running candidates
}

#[async_trait::async_trait]
impl DynamicInjector for ExecutionIntelligenceInjector {
    fn name(&self) -> &str { "execution-intelligence" }

    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        let mut sections: Vec<String> = Vec::new();

        // Section A: verification affordance (only when plan mode is active)
        if ctx.plan_mode_active() {
            if let Some(items) = self.todo_repo.list_for_thread_blocking(
                ctx.thread_id(), ctx.agent_id()
            ) {
                let active_jobs = self.supervisor.list(
                    ctx.thread_id(), ctx.agent_chain(), true
                );
                let candidates = items.iter()
                    .filter(|i| matches!(i.status, TodoStatus::Pending | TodoStatus::InProgress))
                    .filter_map(|i| classify(&i.title).map(|v| (i, v)))
                    .filter(|(i, _)| !active_jobs.iter()
                        .any(|j| j.description.contains(&i.title)))
                    .collect::<Vec<_>>();
                if !candidates.is_empty() {
                    sections.push(render_verification_affordance(&candidates));
                }
            }
        }

        // No section B (recent-failure recall) in 2.3b — episodic memory carries this.
        // The cognitive retrieval pipeline surfaces relevant past failures during
        // context assembly. Future phase if signal proves weak.

        if sections.is_empty() { return vec![]; }
        let body = wrap_in_system_reminder(sections.join("\n\n"));
        vec![ContextUpdate {
            reason:   ContextUpdateReason::CodingJobsChanged,
            priority: ContextUpdatePriority::Standard,
            content:  Some(body),
            ..Default::default()
        }]
    }
}
```

Rendered `<system-reminder>` body for verification affordance:

```xml
<system-reminder>
Plan mode active — the following pending TodoItems look like background-bash candidates after `/plan-exit`:
- "Run integration tests" → bash(command=…, run_in_background=true) [verb: Run]
- "Verify migration safety" → bash(command=…, run_in_background=true) [verb: Verify]
Background jobs cannot be spawned while plan mode is active. After ratification, you may launch these as background jobs.
</system-reminder>
```

The section-B comment in the code is intentional documentation: we considered a sticky "recent failure" body and rejected it in favor of routing recall through the cognitive retrieval pipeline. This keeps the injector cheap (one `<system-reminder>` per iteration max) and avoids token bloat on long sessions.

### 5.2 `command_key` normalization

```rust
// crates/feature-coding-bash/src/intelligence/normalize.rs

/// Trim, collapse internal whitespace, strip leading `KEY=VAL` env vars,
/// then sha256-hex the result. Output is a stable 64-char identifier.
pub fn command_key(raw: &str) -> String {
    let trimmed = raw.trim();
    let no_env  = strip_leading_env_vars(trimmed);
    let collapsed = collapse_whitespace(no_env);
    sha256_hex(&collapsed)
}

fn strip_leading_env_vars(s: &str) -> &str {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^([A-Z_][A-Z0-9_]*=\S+\s+)+").unwrap()
    });
    match RE.find(s) {
        Some(m) => &s[m.end()..],
        None => s,
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

Examples (key abbreviated):

| Input | command_key |
|---|---|
| `cargo nextest run -p agent` | `e3a1...` |
| `  cargo nextest run  -p agent` | `e3a1...` (same: collapsed) |
| `RUST_LOG=debug cargo nextest run -p agent` | `e3a1...` (same: env stripped) |
| `cargo nextest run -p agent --nocapture` | `f72b...` (different: real flag change) |

### 5.3 `diff_against_prior`

```rust
// crates/feature-coding-bash/src/intelligence/diff.rs

pub fn diff_against_prior(prior: &BashJobRow, curr: &BashJobRow) -> JobDiff {
    let kind_transition = classify_transition(prior, curr);
    let extracted_diff = match (parse_extracted(&prior.failure_extracted),
                                parse_extracted(&curr.failure_extracted)) {
        (Some(p), Some(c)) => diff_extracted(&p, &c, &prior.failure_kind, &curr.failure_kind),
        _ => ExtractedDiff::None,
    };
    let elapsed_delta_ms = elapsed_ms(curr) as i64 - elapsed_ms(prior) as i64;
    JobDiff { kind_transition, extracted_diff, elapsed_delta_ms }
}

fn diff_extracted(
    prior: &serde_json::Value,
    curr:  &serde_json::Value,
    prior_kind: &Option<String>,
    curr_kind:  &Option<String>,
) -> ExtractedDiff {
    use ExtractedDiff::*;
    if prior_kind.as_deref() == Some("TestFailure") && curr_kind.as_deref() == Some("TestFailure") {
        let p_set = string_array_set(prior, "failed_test_names");
        let c_set = string_array_set(curr,  "failed_test_names");
        return TestSet {
            new_failures:  c_set.difference(&p_set).cloned().collect(),
            still_failing: c_set.intersection(&p_set).cloned().collect(),
            resolved:      p_set.difference(&c_set).cloned().collect(),
        };
    }
    if prior_kind.as_deref() == Some("CompileError") && curr_kind.as_deref() == Some("CompileError") {
        let pl = location_from(prior);
        let cl = location_from(curr);
        return Compile {
            same_location: pl.is_some() && pl == cl,
            prior_loc: pl,
            curr_loc:  cl,
        };
    }
    if prior_kind.as_deref() == Some("NetworkBindFailure") && curr_kind.as_deref() == Some("NetworkBindFailure") {
        return Bind { /* parse port from each */ };
    }
    if prior_kind.as_deref() == Some("LintFailure") && curr_kind.as_deref() == Some("LintFailure") {
        return Lint { /* delta_n_errors */ };
    }
    if prior_kind.as_deref() == Some("Timeout") && curr_kind.as_deref() == Some("Timeout") {
        return Timeout { /* prior_ms, curr_ms */ };
    }
    if curr_kind.is_none() && prior_kind.is_none() {
        return None;                                                  // both Passed
    }
    OtherExitTransition { from: extract_exit_code(prior), to: extract_exit_code(curr) }
}
```

Pure functions, fully testable without I/O. ~120 LOC including helpers.

### 5.4 `verification_match::classify`

```rust
// crates/feature-coding-bash/src/intelligence/verification_match.rs

pub fn classify(title: &str) -> Option<VerificationVerb> {
    let trimmed = title.trim();
    if trimmed.len() < 3 { return None; }
    let first_token = trimmed.split_whitespace().next()?;
    let lower = first_token.to_ascii_lowercase();
    let cleaned = lower.trim_end_matches(|c: char| !c.is_alphanumeric());
    match cleaned {
        "run" | "running"                   => Some(VerificationVerb::Run),
        "test" | "tests"                    => Some(VerificationVerb::Test),
        "check" | "checking"                => Some(VerificationVerb::Check),
        "verify" | "verifies" | "verifying" => Some(VerificationVerb::Verify),
        "build" | "rebuild" | "compile"     => Some(VerificationVerb::Build),
        _ => None,
    }
}
```

~25 LOC. Tested against fixtures: positive ("Run integration tests"), negative ("Refactor the supervisor"), edge ("verify."→Verify, ""→None).

### 5.5 `GateClassifier` extension

The 2.3a cargo-test detector captured only the headline test name. 2.3b extends to multi-capture:

```rust
// crates/feature-coding-bash/src/gate.rs (extended)

static CARGO_TEST_NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"---- (?P<name>[\w:]+) stdout ----").unwrap()
});

static CARGO_TEST_TOTALS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"test result: FAILED\.\s+(?P<passed>\d+)\s+passed;\s+(?P<failed>\d+)\s+failed(?:;\s+(?P<ignored>\d+)\s+ignored)?").unwrap()
});

fn extract_cargo_test_failure(stdout: &str, stderr: &str) -> Option<serde_json::Value> {
    let combined = format!("{stdout}\n{stderr}");
    let totals = CARGO_TEST_TOTALS_RE.captures(&combined)?;
    let names: Vec<String> = CARGO_TEST_NAME_RE
        .captures_iter(&combined)
        .map(|c| c["name"].to_string())
        .take(50)                                    // defensive cap
        .collect();
    Some(json!({
        "failed_test_names": names,
        "n_passed":  totals["passed"].parse::<u32>().unwrap_or(0),
        "n_failed":  totals["failed"].parse::<u32>().unwrap_or(0),
        "n_ignored": totals.name("ignored")
            .map(|m| m.as_str().parse::<u32>().unwrap_or(0)).unwrap_or(0),
    }))
}
```

Same pattern for vitest / jest / pytest — each detector returns `failed_test_names: Vec<String>` instead of `test_name: String`. Lint/compile/bind/timeout extractors are unchanged.

### 5.6 Extended `supervisor::finalize_job`

```rust
// crates/feature-coding-bash/src/supervisor.rs (existing, extended)

async fn finalize_job(&self, live: &Arc<LiveJob>, exit: ExitStatus) -> Result<()> {
    // ... existing: cancel readers, finalize ring, run gate classifier ...
    let gate = self.gate.classify(...);

    // NEW: command_key for diff lookup + episodic recall
    let command_key = command_key(&live.spec.command);

    // NEW: query prior run for diff
    let diff = match self.repo
        .find_prior_by_command_key(&live.spec.session_id, &command_key, live.id.as_str())
        .await
    {
        Ok(Some(prior)) => Some(diff_against_prior(&prior, &curr_row)),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = ?e, "prior lookup failed; skipping diff");
            None
        }
    };

    let curr_row = build_row(live, gate, command_key, exit);

    // existing: UPSERT (now includes command_key)
    self.repo.upsert(&curr_row).await?;

    // existing: publish DomainEvent::BashJob(...)
    self.bus.publish_bash_job(/* event */);

    // existing: enqueue completion ContextUpdate — NOW with diff
    if !live.spec.silent_completion {
        let body = render::completion_body(&curr_row, diff.as_ref());
        self.update_queue.enqueue(ContextUpdate {
            reason:   ContextUpdateReason::CodingJobsChanged,
            priority: ContextUpdatePriority::High,
            content:  Some(body),
            ..Default::default()
        });
    }

    // existing: emit Tauri event, remove from DashMap
    Ok(())
}
```

The diff lookup is best-effort: a DB error or missing prior never blocks the completion notification.

### 5.7 Extended `render::completion_body`

```rust
// crates/feature-coding-bash/src/render.rs (existing, extended)

pub fn completion_body(curr: &BashJobRow, diff: Option<&JobDiff>) -> String {
    let mut s = String::new();
    s.push_str("<system-reminder>\n");
    s.push_str(&format_header(curr));            // existing 2.3a
    s.push_str(&format_status_line(curr));       // existing
    s.push_str(&format_failure_section(curr));   // existing — uses failure_extracted

    if let Some(d) = diff {
        s.push_str("\nCompared to last run of this command:\n");
        s.push_str(&format_kind_transition(&d.kind_transition));
        s.push_str(&format_extracted_diff(&d.extracted_diff));
        s.push_str(&format_elapsed_delta(d.elapsed_delta_ms));
    }

    s.push_str(&format_tail_lines_section(curr)); // existing — last 80 lines
    s.push_str("</system-reminder>\n");
    s
}
```

Rendered example (test failure regression):

```xml
<system-reminder>
Background job bash-aB3kF7c2qR completed.
Description: cargo nextest run --workspace
Status: Failed  Exit: 101  Ran: 4m 38s  Output: 3.2 MB

Failure kind: TestFailure
Detail: "test result: FAILED. 3 failed; 17 passed; 1 ignored"
Extracted:
  failed_test_names: [reload_active_thread, reload_orphan, concurrent_writes]
  n_failed: 3, n_passed: 17, n_ignored: 1

Compared to last run of this command:
  Transition: StillFailing (TestFailure → TestFailure)
  Test diff:
    new failures:  concurrent_writes
    still failing: reload_active_thread
    resolved:      <prior had: reload_orphan, reload_partial>
  Wall-clock: +12.4s (slower)

Last 80 lines of output below.
For more, call coding_task_output("bash-aB3kF7c2qR", since_offset=…).

[--- last 80 lines verbatim ---]
</system-reminder>
```

**Token-budget guard:** if the rendered diff section exceeds 4 KB (rare — happens only for huge test-set diffs), the renderer truncates each set to 50 names with `+ N more` suffix. Implementation in `format_extracted_diff`.

### 5.8 `BackgroundJobSignalSource`

```rust
// crates/cognitive/src/mirror/sources/coding_bash.rs

pub struct BackgroundJobSignalSource {
    episodic_repo: EpisodicMemoryRepo,
    bash_repo:     Arc<BashJobRepo>,
}

impl BackgroundJobSignalSource {
    pub fn new(episodic_repo: EpisodicMemoryRepo, bash_repo: Arc<BashJobRepo>) -> Self {
        Self { episodic_repo, bash_repo }
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for BackgroundJobSignalSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_bash",
            subscribed_kinds: &[
                "BashJob.Completed",
                "BashJob.Failed",
                "BashJob.Cancelled",
                "BashJob.Lost",
            ],
            flush_interval_secs: None,                   // per-event write; no flush loop
        }
    }

    fn name(&self) -> &'static str { "coding_bash" }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        let job_id = signal.field_str("job_id")?;
        let row = match self.bash_repo.get(job_id).await? {
            Some(r) => r,
            None => {
                tracing::debug!(job_id, "row missing at episodic write; skipping");
                return Ok(());
            }
        };
        let mem = build_episodic_memory(&row);
        self.episodic_repo.insert(&mem).await?;
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> { Ok(()) }
}

fn build_episodic_memory(row: &BashJobRow) -> EpisodicMemory { /* §4.6 shape */ }
```

`flush_interval_secs: None` skips the periodic flush loop entirely — there's nothing to aggregate.

### 5.9 Translator arm in `ai_pipeline::translate`

```rust
// crates/app-core/src/init/ai_pipeline.rs (extended translate function)

fn translate(event: &DomainEvent) -> Option<AiSignal> {
    match event {
        // ... existing arms (Skill, Task, Todo, etc.) ...

        DomainEvent::BashJob(inner) => {
            let kind = match inner {
                BashJobEvent::Started   { .. } => "BashJob.Started",
                BashJobEvent::Completed { .. } => "BashJob.Completed",
                BashJobEvent::Failed    { .. } => "BashJob.Failed",
                BashJobEvent::Cancelled { .. } => "BashJob.Cancelled",
                BashJobEvent::Lost      { .. } => "BashJob.Lost",
            };
            Some(AiSignal::builder(kind, RecallDomain::CodingMemory)
                .with_field("job_id",    inner.job_id())
                .with_field("thread_id", inner.thread_id())
                .with_field("agent_id",  inner.agent_id())
                .build())
        }

        _ => None,
    }
}
```

`BashJobEvent::job_id() / .thread_id() / .agent_id()` are convenience methods we add on the existing enum (~30 LOC extension to `crates/bus/src/domain_events.rs`).

`BackgroundJobSignalSource::accumulate` reads the bash row instead of unpacking the signal because (a) `BashJob.Started` doesn't carry `failure_extracted`, (b) keeping bus messages lean preserves the 2.3a invariant, (c) the bash row is already-persisted by the time the signal fires (the supervisor publishes the event *after* the SQL update in `finalize_job`).

### 5.10 Registration in `app-core::init`

```rust
// crates/app-core/src/init/mod.rs (extended)

let exec_intel_injector = Arc::new(ExecutionIntelligenceInjector::new(
    todo_repo.clone(),
    bash_repo.clone(),
    job_supervisor.clone() as Arc<dyn JobSupervisorHandle>,
));

let injector_registry = bus::InjectorRegistry::new(vec![
    plan_mode_injector       as Arc<dyn DynamicInjector>,
    background_jobs_injector as Arc<dyn DynamicInjector>,
    exec_intel_injector      as Arc<dyn DynamicInjector>,    // NEW
]);

// MirrorEngine registration: pass the new source via the existing builder
let bg_job_source = Arc::new(BackgroundJobSignalSource::new(
    episodic_repo.clone(),
    bash_repo.clone(),
));
mirror_engine_builder.add_source(bg_job_source);                // NEW
```

(Exact builder shape depends on whether `MirrorEngine::start` gains an `extra_sources` parameter or whether sources are pushed via a new builder method. The implementation plan will pick one based on `cognitive/src/mirror/engine.rs` at the time of writing.)

---

## 6. Data Flow

### 6.1 Plan-mode iteration boundary — affordance flow

```
LLM iteration N completes inside coding mode + plan mode active
   │
   ▼
LiveContextRefresher::inject_pending_with_ctx(messages, ctx)
   │
   ├── drain ContextUpdateQueue (existing)
   ├── InjectorRegistry::collect_all(ctx)
   │     ├── PlanModeInjector             → "Plan mode active. Edit/Write only the plan file."
   │     ├── BackgroundJobsInjector       → 0 updates (bg bash unavailable in plan mode)
   │     └── ExecutionIntelligenceInjector::collect(ctx)                    ← NEW
   │           ├── ctx.plan_mode_active() → true; proceed
   │           ├── todo_repo.list_for_thread_blocking(thread_id, agent_id)
   │           ├── filter status ∈ {Pending, InProgress}
   │           ├── verification_match::classify(title) per item
   │           ├── filter out items already covered by an active job (description match)
   │           ├── render_verification_affordance(matches) → <system-reminder>
   │           └── enqueue 1 ContextUpdate (priority=Standard, reason=CodingJobsChanged)
   │
   └── merge into iteration N+1 prompt as Message::ContextUpdate
```

Edge cases:
- No matching todos → injector returns `vec![]` (silent).
- All matches covered by active jobs → injector returns `vec![]` (silent — would be redundant).
- Plan mode inactive → injector returns `vec![]` immediately (early exit before any DB call).

### 6.2 Completion with diff — push flow

```
child process exits (or is killed by stop / thread-delete / timeout)
   │
   ▼
wait_task observes exit_status
   │
   ▼
JobSupervisor::finalize_job(live_job, exit)
   │
   ├── existing: cancel reader tasks
   ├── existing: RingFile::finalize() → {id}.final
   ├── existing: gate_classifier.classify(...) → GateResult
   │             (NOW returns failed_test_names: Vec<String> for TestFailure — §5.5)
   │
   ├── NEW: command_key = normalize::command_key(&spec.command)
   │
   ├── NEW: prior = bash_repo.find_prior_by_command_key(session_id, command_key, this_id)
   │         → Option<BashJobRow>
   │
   ├── existing: build BashJobRow (NOW includes command_key + extended failure_extracted)
   ├── existing: bash_repo.upsert(&row).await
   │
   ├── NEW: diff = prior.map(|p| diff::diff_against_prior(&p, &row))
   │         → Option<JobDiff>
   │
   ├── existing: bus.publish_bash_job(BashJobEvent::Completed | Failed | Cancelled)
   │
   ├── if !silent_completion:                        ← NEW: render now takes diff
   │     body = render::completion_body(&row, diff.as_ref())
   │     update_queue.enqueue(ContextUpdate { priority=High, content=body, ... })
   │
   ├── existing: emit Tauri event "coding:job_event"
   └── existing: DashMap.remove(job_id) — LiveJob dropped
```

If `find_prior_by_command_key` errors: log a warning, continue without diff. If the prior row's `failure_extracted` is malformed JSON: `parse_extracted` returns `None`, body falls back to `ExtractedDiff::None`; the kind transition still renders.

### 6.3 Episodic write — cognitive flow

```
JobSupervisor.finalize_job publishes BashJob event (above)
   │
   ▼
DomainEventBus dispatches DomainEvent::BashJob(BashJobEvent::Failed { ... })
   │
   ▼
SignalRouter (existing) calls translate(&event) per consumer
   │
   ├── ai_pipeline::translate(event) — NEW arm                       ← §5.9
   │     match BashJob(inner) →
   │         AiSignal { event_kind: "BashJob.Failed",
   │                    domain: RecallDomain::CodingMemory,
   │                    fields: { job_id, thread_id, agent_id } }
   │
   ▼
SignalRouter dispatches AiSignal to all subscribed consumers
   │
   ├── BackgroundJobSignalSource (subscribed_kinds matches "BashJob.Failed")  ← NEW
   │     │
   │     └── accumulate(signal):
   │           job_id = signal.field_str("job_id")
   │           row    = bash_repo.get(job_id).await
   │           mem    = build_episodic_memory(&row)            ← §4.6 shape
   │           episodic_repo.insert(&mem).await
   │
   ├── (other existing consumers unaffected — none subscribe to "BashJob.*")
   └── done; no flush loop (flush_interval_secs: None)
```

Race window: the supervisor's order is `bash_repo.upsert → bus.publish_bash_job → enqueue ContextUpdate`. The signal source re-reads the row via `bash_repo.get`. The upsert happens *before* the publish, so the row is guaranteed visible when `accumulate` runs. (`StoragePool` is WAL-mode SQLite; readers see committed writes.)

If `bash_repo.get(job_id)` returns `None` (concurrent thread delete): `accumulate` logs at `tracing::debug!` and returns `Ok(())`. Episodic memory for a deleted thread serves no recall purpose.

### 6.4 Restart recovery — extended Lost flow

```
AppCore::init() → JobSupervisor::reconcile_on_startup()
   │
   ├── existing: SELECT id, log_path, final_path, started_at, command_key
   │             FROM coding_background_jobs WHERE status IN ('Starting','Running')
   │
   ├── for each orphan row:
   │     │
   │     ├── existing: classify as Lost / Completed-late based on .final presence
   │     ├── existing: status=Lost; failure_kind=Lost
   │     │
   │     ├── NEW: even Lost rows keep their command_key (set on insert at spawn time)
   │     │
   │     ├── existing: enqueue ContextUpdate(CodingJobsChanged, body=lost_notification)
   │     │
   │     └── NEW: publish BashJob.Lost on the bus
   │             → BackgroundJobSignalSource writes a Lost episode (importance 0.6)
   │             → Lost runs DO produce episodes, just no diff
   │
   └── existing: orphan-file sweep
```

The Lost episode lets the LLM remember "we tried this command, it ran for X minutes, then we restarted before it finished" — useful context if the user asks the LLM to re-run it.

### 6.5 Subagent inheritance — unchanged from 2.3a

`ExecutionIntelligenceInjector` calls `ctx.agent_chain()` exactly like `BackgroundJobsInjector`. TodoItems are scoped per `(thread_id, agent_id)` (Phase 2.1), so a subagent's affordance reflects only the subagent's own todos.

`BackgroundJobSignalSource` writes episodic memories with `actor_id = agent_id`. The cognitive retrieval path can already filter by `actor_id`; default behavior pools across all agents in the session.

### 6.6 Thread cleanup — unchanged from 2.3a

`reap_session` already kills processes, finalizes rows, and the SQLite cascade deletes them. Episodic memories themselves are independent rows in the cognitive DB and are not cascaded — the user's deleted thread leaves behind episodes labelled with that thread's session_id; they remain searchable but unreachable via the bash row. Consistent with how todo events behave.

(Future cleanup: a periodic `INVALIDATE FROM episodic_memories WHERE scope_id NOT IN (SELECT id FROM coding_sessions) AND domain='coding' AND kind='bash_job'` job. Out of scope for 2.3b.)

---

## 7. Approval & Concurrency

**Unchanged from 2.3a.**

- No new tools → no new approval-class decisions.
- The `bash` tool's `Destructive` class still gates spawn (background or foreground).
- `ExecutionIntelligenceInjector` runs synchronously inside the iteration boundary like every other `DynamicInjector`; no concurrency considerations.
- `BackgroundJobSignalSource::accumulate` is `async fn`; the SignalRouter's existing back-pressure model applies (queue-based, no supervisor blocking).
- The cap = 6 active jobs per `(session_id, agent_chain)` is unchanged. Affordance suppression checks the active job list to avoid suggesting jobs that would hit the cap immediately.

---

## 8. Subagent Inheritance

**Unchanged from 2.3a.**

- `ExecutionIntelligenceInjector` reads `ctx.agent_chain()` identically to `BackgroundJobsInjector`.
- TodoItems remain scoped per `(thread_id, agent_id)` — subagents have their own todo lists.
- Episodic memories carry `actor_id = agent_id`; cognitive retrieval can filter or pool as needed.

---

## 9. Recovery & Restart

The 2.3a recovery flow gains exactly two changes (§6.4):

1. The recovery row's `command_key` is preserved (it was set at spawn time, persists across restart).
2. `JobSupervisor::reconcile_on_startup` publishes `BashJob.Lost` on the bus for each orphan, so `BackgroundJobSignalSource` writes a Lost episode. Importance 0.6.

Lost rows are still excluded from `find_prior_by_command_key` (they have no reliable final output to diff against). The Lost episode is for retrieval, not diff source.

---

## 10. Error Handling

The principle from 2.3a continues: **errors surface as tool results or as logs, never as Rust panics.**

| Failure | Symptom | Response |
|---|---|---|
| `find_prior_by_command_key` errors | DB transient or migration-skew | log `tracing::warn!`, set `diff = None`, completion notification still fires |
| Prior row's `failure_extracted` is malformed JSON | Hand-edited DB or pre-2.3b row | `parse_extracted` returns `None`, body falls back to `ExtractedDiff::None`, kind transition still rendered |
| `bash_repo.get(job_id)` returns None in `accumulate` | Concurrent `coding_thread_delete` | `tracing::debug!`, `Ok(())` — no episode written, no error propagated |
| `episodic_repo.insert` errors | Cognitive DB corrupt or full | `tracing::warn!`, `Ok(())` — bash supervisor unaffected; missing episode is acceptable degradation |
| TodoRepo blocking-read returns None | Cache miss at iteration time | `ExecutionIntelligenceInjector::collect` returns `vec![]` silently — affordance is best-effort |
| Verification verb regex matches a noisy title | "Run away from this code" | False-positive affordance is acceptable; LLM ignores irrelevant suggestions |
| Plan-mode todos and active jobs go out of sync (substring heuristic) | description match misses or mis-matches | False negative produces redundant affordance (LLM tolerates); false positive suppresses a valid affordance (LLM still has the active-jobs reminder) |
| Episodic write happens before row durable-commit (WAL fsync) | Very low probability | `bash_repo.upsert` awaits the SQLite commit before publishing; SignalSource reads via `bash_repo.get` after publish — read-your-writes guaranteed by SQLite WAL within the same pool |
| `MirrorEngine::start` signature drifts and source not registered | Compile-time | Compile error if registration is omitted — `register!` macro statically requires the source struct |
| `command_key` collision (sha256) | 2^-256 probability | Diff lookup returns wrong prior — practically impossible. Documented; not defensively handled |

---

## 11. Testing Strategy

### 11.1 Unit tests (inline `#[cfg(test)] mod tests`)

| File | Coverage |
|---|---|
| `intelligence/normalize.rs` | command_key idempotence; whitespace collapsing; env-var stripping (`RUST_LOG=…`, multiple `K=V`); env-var-shaped-but-not-leading is preserved; sha256 stable across runs; differing flags produce different keys |
| `intelligence/verification_match.rs` | each verb maps; case-insensitive; conjugations (running/tests/verifying); short titles return None; punctuation-trailing first word ("verify."→Verify); pure refactor titles ("Refactor X") return None |
| `intelligence/diff.rs` | `KindTransition` per pair (Pass→Pass, Pass→Fail, Fail→Pass, Fail-same-kind, Fail-different-kind); `ExtractedDiff::TestSet` set-arithmetic correctness (overlapping, disjoint, empty); `Compile::same_location` bool; `Bind`, `Lint`, `Timeout` variants; missing/malformed `failure_extracted` JSON → `ExtractedDiff::None`; signed `elapsed_delta_ms` |
| `intelligence/injector.rs` | empty when plan_mode_active=false; empty when no Pending/InProgress todos; empty when all matches covered by active jobs; rendered body contains every match; respects `ctx.agent_chain` for active-job dedup; ≤1 ContextUpdate |
| `gate.rs` extension | cargo: extracts ALL failed test names (multi-failure fixture); vitest: extracts all `> failure name`; pytest: extracts all `FAILED test/path::test_name`; cap at 50; `failed_test_names: []` when totals show 0 failed |
| `mirror/sources/coding_bash.rs` | `spec()` returns the 4 BashJob.* kinds; `accumulate` writes one EpisodicMemory with correct kind/domain/importance per status; missing bash_repo row is non-fatal (debug log + Ok); `flush()` is a no-op |
| `storage/repos/coding_background_jobs.rs` | `find_prior_by_command_key` returns most recent terminal-state row; excludes `exclude_id`; excludes Lost; returns None when no prior; uses the composite index (verify via EXPLAIN QUERY PLAN) |

Total: ~40 test cases, all sub-second, no I/O beyond `connect_in_memory()`.

### 11.2 Integration tests (`crates/feature-coding-bash/tests/`)

| File | Scenario |
|---|---|
| `intel_diff_basic.rs` | Spawn `false` (immediate Failed). Spawn `false` again with same command. Second completion's body contains "Compared to last run" + "StillFailing" |
| `intel_diff_test_set.rs` | Spawn fixture command producing TestFailure with names [A,B]. Spawn again with output [B,C]. Diff body shows new=[C], still=[B], resolved=[A] |
| `intel_diff_recovered.rs` | Spawn failing command, then a passing command of same key. Body shows "Recovered (TestFailure → Passed)" |
| `intel_affordance_in_plan.rs` | Open thread, enter plan mode, write todos ["Run integration tests", "Refactor X"]. Trigger an iteration. Captured prompt contains affordance for "Run integration tests" only |
| `intel_affordance_dedup.rs` | Same as above but spawn an active background job whose description contains "Run integration tests". Affordance section is suppressed |
| `intel_episodic_write.rs` | Spawn `false`. Wait for finalize. Query `episodic_memories` for `kind='bash_job'`. Exactly one row, importance≈0.7, content JSON contains failure_extracted |
| `intel_episodic_passed.rs` | Spawn `true`. Episode written, importance≈0.3, status=Completed |
| `intel_episodic_lost_on_restart.rs` | Insert fake Running row + .log file; call `reconcile_on_startup`. Verify a Lost episode appears with importance≈0.6 |
| `intel_command_key_normalization.rs` | Spawn ` cargo test  -p agent` then `RUST_LOG=debug cargo test -p agent`. Second completion finds the first as prior (diff body present) |
| `intel_subagent_episodic_actor_id.rs` | Subagent spawns a job; episode's `actor_id` matches subagent's agent_id, not the parent's |

All use `StoragePool::connect_in_memory()`; the cognitive `episodic_memories` table is created via the standard cognitive migration pre-test. Fakes for `BashJobEvent` are constructed via the existing `bus.publish_bash_job` helper.

### 11.3 Frontend tests

**None.** The `JobsPanel` already shows everything it needs to from 2.3a. The diff section is rendered server-side as part of the LLM-facing `<system-reminder>` body and never touches the UI.

### 11.4 Manual smoke checklist

Run before merge:

1. Open a coding thread. Run `bash run_in_background=true command="echo hi; false"`. After completion, observe the `<system-reminder>` body — no diff section (no prior).
2. Re-run the same command. Observe diff section: "StillFailing (Other → Other)".
3. Run `bash run_in_background=true command="cargo nextest run -p agent"`. Wait. Re-run with a code change that breaks one more test. Observe `new failures: [...]` in the diff.
4. Enter plan mode. Add todos via TodoWrite: "Run integration tests" and "Refactor supervisor". Continue chatting. Verify the LLM sees the affordance for "Run integration tests" in its system reminders.
5. Query the cognitive memory inspector or `~/.klyntbot-dev/data.db`: `SELECT id, kind, importance, summary FROM episodic_memories WHERE kind='bash_job' ORDER BY recorded_at DESC LIMIT 5`. Confirm rows match the spawns.
6. Force-kill Tauri while a job is running. Restart. Confirm a Lost episode appears in `episodic_memories`.
7. In a long session (≥5 bash spawns, ≥3 distinct commands), trigger a chat that asks "what failed last time we ran the test suite?" — verify the LLM cites the past episode (the cognitive-retrieval acceptance criterion).

### 11.5 Migration / rollout

Pre-release status (per `CLAUDE.md`): bump `FeatureMigration::version` from 1 → 2; SQL drops + recreates `coding_background_jobs` with the new column. No backfill script. Existing dev databases lose their bash job history on first run with the new binary — acceptable for pre-release. Document this in the PR description.

After release this approach changes — but that's the next major-feature concern, not this spec's.

---

## 12. Future Phases

### Phase 2.3c — "Interactive Compute" (~3–4 days, demand-driven)

Unchanged from the 2.3a spec:
- PTY support: `tty: bool` flag on `bash` activates `klynt-pty::ChildHandle::Pty`
- Interactive stdin: `coding_task_stdin(task_id, data)` (`Sensitive` approval class)
- TTY resize: `coding_task_resize(task_id, rows, cols)`
- Trigger criterion: at least one user request demands `npm init`-style flows or visible TTY-aware command output

### Beyond 2.3c — opportunistically, not roadmap-bound

- **LLM-driven cross-job pattern distillation.** A nightly cognitive job reads the last 7 days of `kind='bash_job'` episodes, asks the cognitive provider to find patterns, writes a distilled `kind='bash_pattern'` episode with importance 0.9. Trigger: when simple BM25 retrieval starts producing low-quality matches.
- **Bash command auto-suggestion in plan mode.** Currently the affordance shows what *kind* of bash a TodoItem looks like; future could propose the actual command string by matching against past episodes. Trigger: when the user reports the LLM keeps writing the wrong command for a recurring TodoItem.
- **Diff prose summarization via LLM.** Currently the diff is structured `<system-reminder>` body. Future could call the cognitive provider for "you've now broken `concurrent_writes` on top of the still-failing `reload_active_thread`" prose. Trigger: user feedback that the structured diff is too cryptic.
- **Episodic GC for deleted threads.** Periodic `INVALIDATE FROM episodic_memories WHERE scope_id NOT IN (SELECT id FROM coding_sessions) AND domain='coding' AND kind='bash_job'`. Trigger: episodic table size exceeds N MB.

---

## 13. Game-Changer Scorecard

| Pillar | 2.3a | **2.3b** | 2.3c |
|---|---|---|---|
| Never forgets (per-turn injector) | ✅ | ✅ | ✅ |
| Push-on-completion (no polling required) | ✅ | ✅ | ✅ |
| Structured failure extraction (file/line/test_name/port) | ✅ | ✅ | ✅ |
| Tauri-restart recovery (honest Lost status) | ✅ | ✅ | ✅ |
| Subagent inheritance | ✅ | ✅ | ✅ |
| Cross-run output diffing | ❌ | **✅** | ✅ |
| Plan-mode auto-affordance | ❌ | **✅** | ✅ |
| Episodic memory of past failures | ❌ | **✅** | ✅ |
| Interactive (PTY/stdin/resize) | ❌ | ❌ | ✅ |

**8 of 9 pillars after 2.3b.** Only interactive shell remains, and it was always demand-driven rather than roadmap-driven.

---

## Appendix A — Verification commands

```bash
# Tests
cargo nextest run -p feature-coding-bash
cargo nextest run -p storage -E 'test(coding_background_jobs)'
cargo nextest run -p cognitive -E 'test(coding_bash)'
cargo nextest run -E 'test(intel_)'

# Lint + format
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check

# Doctest
cargo test --workspace --doc

# Manual
cargo tauri dev
# Then run the §11.4 smoke checklist
```

---

## Appendix B — Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Diff body inflates token budget for chatty completions | Medium | Diff section bounded: ≤ 50 test names rendered (rest summarized as `+ N more`); ≤ 3 lines for non-test-set diffs; falls back to "no diff" if body would exceed 4 KB |
| `command_key` index gets bloated in long-lived sessions | Low | Index is per-session; sessions bounded by user interaction; CASCADE DELETE on session-deletion clears it |
| `failed_test_names` extraction regex misses a test framework's format | Medium | Detector tested per fixture; new framework adds a new detector + fixture; detector returning `failed_test_names: []` is safe (TestFailure still classified) |
| `BackgroundJobSignalSource::accumulate` falls behind under bash-spawn burst | Low | SignalRouter is async + queue-based; no back-pressure on supervisor. Worst case: episodic write lags completion notification by a few hundred ms |
| Verification-verb classifier produces too many false positives | Low | Affordance is silent for the user (only LLM sees it); LLM ignores irrelevant suggestions; cost is a few prompt tokens per iteration |
| Plan-mode todos and active jobs go out of sync (description-match heuristic) | Medium | Dedup is best-effort by substring match; false negatives produce a redundant affordance (LLM tolerates); false positives suppress a valid affordance (LLM still has active-jobs reminder) |
| Episodic write happens before row is durably committed (WAL fsync) | Very low | `bash_repo.upsert` awaits SQLite commit before publishing; SignalSource reads via `bash_repo.get` after publish — read-your-writes guaranteed by SQLite WAL within the same pool |
| `MirrorEngine::start` signature drifts and source not registered | Low | Compile error if registration is omitted — `register!` macro statically requires source struct |
| Backfill needed post-release | Pre-release N/A | Documented as known limitation; first post-release migration must add `command_key TEXT NOT NULL DEFAULT ''` + a backfill that recomputes keys for existing rows |
| `command_key` sha256 collision | 2^-256 | Practically impossible. Documented; not defensively handled |

---

## Appendix C — Out-of-scope confirmations

For clarity, these are explicitly NOT part of 2.3b (so reviewers don't ask):

- New tools (no `coding_task_history`, no `coding_task_diff`)
- Frontend changes — `JobsPanel` already shows everything it needs to from 2.3a
- Cognitive provider routing changes (`BackgroundJobSignalSource` makes no LLM calls)
- Agent runtime extensions beyond the one translator arm and one injector registration
- Tests in `desktop-ui/` — no UI surface added or changed
- `mirror_bash_job_snapshots` table — episodic-only; aggregation deferred
- Cross-session episodic correlation — episodes stay scoped to their session
