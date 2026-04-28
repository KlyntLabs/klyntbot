# Launcher Search: Performance Upgrade + Attention-Driven Personalization

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take launcher file search from ~270 µs (worst case) at 50k entries to **<300 µs at 200k+ entries**, fix the short-prefix scaling cliff, and turn the launcher's empty-query / personalization layer into a *true second brain* by ingesting `productivity::activity_events` (continuous attention data) instead of click-only frecency.

**Architecture:** Two parallel tracks land independently:
- **Track A (perf):** Phases 1–3, 6 — keep public API of `InvertedFileIndex` stable; rewrite internals (pre-lowercased fields → trie + Roaring bitmaps). Bench harness already exists at `crates/feature-launcher/benches/inverted_index.rs` with a saved `initial` baseline.
- **Track B (personalization):** Phases 4–5 — new `entity_attention` table + nightly aggregator + `AttentionSource`. No churn to existing search source contracts.

**Tech Stack:** Rust (sqlx, tokio, jiff, smol_str, parking_lot, criterion). New deps: `rustc-hash` (FxHashMap), `fst` (Map+MapBuilder), `roaring` (RoaringBitmap). Frontend: TanStack Query + Vitest + Testing Library.

**Hard targets (gates per phase):**
- Phase 1 ships only if `cargo bench --baseline initial` shows ≥40% improvement on `inverted_index_search_50000/re`.
- Phase 3 ships only if `inverted_index_search_50000/re` < 100 µs and a synthesized 200k bench < 300 µs.
- Phase 4 ships only if attention-rebuild on 90 days × 50k events completes in < 2 s.
- All phases keep `cargo nextest run -p feature-launcher` green.

**Dependency notes:**
- Phases 1, 2 are sequential (Phase 2 reuses Phase 1's pre-lowercased fields).
- Phase 3 depends on Phase 2 (BM25 weights are reused inside the trie scorer).
- Phases 4 + 5 depend on each other (4 ships schema + repos, 5 ships UI/source). Independent of perf phases.
- Phase 6 (build perf, 200k+ scaling) is opportunistic — only if profiling shows we need it.

---

## Table of Contents

- **Phase 1** — Allocation & sort hot-path elimination *(target: −50% short-prefix latency)*
- **Phase 2** — IDF / BM25 ranking quality
- **Phase 3** — Trie + Roaring bitmap intersection *(target: <100 µs short-prefix @ 50k, <300 µs @ 200k)*
- **Phase 4** — `entity_attention` schema + nightly aggregator
- **Phase 5** — `AttentionSource` + index-level personalization
- **Phase 6** — Window-title content recall (opt-in) **and** parallel build / generation tombstoning *(opportunistic)*

---

## Pre-flight checklist (do once before Phase 1)

- [ ] Verify saved baseline exists at `target/criterion/inverted_index_*/initial/` — if not, run `cargo bench -p feature-launcher --bench inverted_index -- --save-baseline initial`.
- [ ] Add a bench fixture for 200k-entry corpus to detect regressions early. Append to `crates/feature-launcher/benches/inverted_index.rs`:

  ```rust
  // Add 200_000 to both the build group and the search group.
  // Skip the build@200k case in CI (it's 1+ second) — gate behind an env var.
  for &corpus in &[5_000usize, 50_000, 200_000] { … }
  ```

- [ ] Add a release-checklist note to `crates/feature-launcher/README.md` (create if absent) describing how to run the bench against `initial` and how to update the baseline.
- [ ] Snapshot today's results to `docs/launcher-bench-baseline-2026-04-28.md` (committed) so future regressions have a portable reference.

---

## Phase 1 — Allocation & sort hot-path elimination

**Effort:** 1–2 days. **Target:** `inverted_index_search_50000/re` ≤ 140 µs (from 274 µs); `…/report` ≤ 70 µs (from 128 µs); zero allocations in the score-loop hot path.

### Background

Today's hot path runs once per scored candidate:

```rust
let name_lower = entry.name.to_lowercase();           // alloc 1
let compact: String = name_lower.chars()              // alloc 2
    .filter(|c| c.is_alphanumeric()).collect();
```

For a 2-char query against 50k entries this typically scores ~5,000 candidates — **10,000 small heap allocations on the keystroke path**. Plus the final `sort_by` walks the entire candidate array even though we only return 20 items.

### File structure

| File | Change |
|---|---|
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: extend `IndexEntry` with `name_lc: SmolStr` and `name_compact: SmolStr`; populate at build + `apply_event_create`; rewrite `score_entry_match` to take `&str` slices |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: replace tail `sort_by` in `search()` with a bounded min-heap (`BinaryHeap<Reverse<(i32, u32)>>` size = `limit + 1`) |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: replace `HashMap<SmolStr, Vec<u32>>` with `FxHashMap<SmolStr, Vec<u32>>` |
| `crates/feature-launcher/Cargo.toml` | Modify: add `rustc-hash = "2"` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Add: `#[bench]` fixture verifying scoring is allocation-free under `dhat` |

### Tasks

- [ ] **1.1** Add `name_lc: SmolStr` to `IndexEntry`. Populate at build:
      ```rust
      let name_lc: SmolStr = if name.bytes().all(|b| !b.is_ascii_uppercase()) {
          SmolStr::new(&name)        // already lowercase — borrow
      } else {
          SmolStr::new(name.to_lowercase())
      };
      ```
      The fast-path check matters: most filenames are already lowercase, so this avoids the `to_lowercase()` allocation entirely.

- [ ] **1.2** Add `name_compact: SmolStr` (alphanumeric-only). Populate similarly with a fast-path: if `name_lc.bytes().all(|b| b.is_ascii_alphanumeric())`, reuse `name_lc.clone()`.

- [ ] **1.3** Extend `apply_event_create` to populate the same two fields. Add unit test: `apply_event_create_caches_lc_and_compact`.

- [ ] **1.4** Rewrite `score_entry_match` to take `&IndexEntry` and read the cached fields:
      ```rust
      fn score_entry_match(query_tokens: &[String], entry: &IndexEntry) -> i32 {
          let name_lc = entry.name_lc.as_str();
          let name_cmp = entry.name_compact.as_str();
          let mut score = 0i32;
          for q in query_tokens {
              let s = if name_lc == *q                   { 140 }
                      else if name_lc.starts_with(q)     { 118 }
                      else if name_cmp.starts_with(q)    { 106 }
                      else if name_lc.contains(q.as_str()){ 60 }
                      else if is_subsequence(q, name_lc) { 44 }
                      else                               { 0 };
              score += s;
          }
          score - (entry.depth as i32) * 2
      }
      ```

- [ ] **1.5** Replace tail sort with bounded min-heap. Skeleton:
      ```rust
      use std::cmp::Reverse;
      use std::collections::BinaryHeap;

      let mut heap: BinaryHeap<Reverse<(i32, u32)>> = BinaryHeap::with_capacity(limit + 1);
      let worst = |h: &BinaryHeap<Reverse<(i32, u32)>>| h.peek().map(|r| r.0.0);

      for &idx in &candidates {
          let s = score_entry_match(&tokens, &self.entries[idx as usize]);
          if heap.len() < limit {
              heap.push(Reverse((s, idx)));
          } else if Some(s) > worst(&heap) {
              heap.pop();
              heap.push(Reverse((s, idx)));
          }
      }

      let mut out: Vec<ScoredEntry> = heap.into_sorted_vec().into_iter()
          .map(|Reverse((score, entry_idx))| ScoredEntry { entry_idx, score })
          .collect();
      // BinaryHeap returns ascending; reverse for descending.
      out.reverse();
      ```
      Tie-break by name length identical to current behaviour — encode it into the tuple key: `(score, -(name_len as i32), idx)`.

- [ ] **1.6** Swap `HashMap<SmolStr, Vec<u32>>` → `rustc_hash::FxHashMap<SmolStr, Vec<u32>>`. One-line change for the type alias; verify no API leak (the type is `pub(crate)`, so safe).

- [ ] **1.7** Bench against `initial`. Acceptance: `inverted_index_search_50000/re` shows `change: [-45%, -55%]` with statistical significance.

- [ ] **1.8** Add a "no-alloc in score loop" assertion. Use `dhat`-flavoured assertion via a feature-gated test:
      ```rust
      #[cfg(feature = "dhat-test")]
      #[test]
      fn score_loop_does_not_allocate() {
          let _profiler = dhat::Profiler::builder().testing().build();
          let stats_before = dhat::HeapStats::get();
          let _ = idx.search("re", 20);
          let stats_after = dhat::HeapStats::get();
          // Allow up to one allocation for the candidates Vec.
          assert!(stats_after.curr_blocks - stats_before.curr_blocks <= 1);
      }
      ```
      Optional — skip if it adds ceremony, but it's a cheap regression guard.

### Acceptance criteria
- Bench `inverted_index_search_50000/re` ≤ 140 µs.
- Bench `inverted_index_search_50000/report` ≤ 70 µs.
- All existing `feature-launcher` unit tests still green.
- No new clippy warnings.

---

## Phase 2 — IDF / BM25-style ranking quality

**Effort:** 2–3 days. **Target:** subjective ranking improvement (rare tokens beat common ones), no perf regression vs. Phase 1.

### Background

Common tokens (`md`, `js`, `test`, `index`) currently contribute the same tier scores as rare ones. A user typing `parser test` should see test-files for the parser module ranked above generic `test.js` files. IDF weighting accomplishes this with one extra multiply per token.

### File structure

| File | Change |
|---|---|
| `crates/feature-launcher/src/search/inverted_index.rs` | Add: `idf: FxHashMap<SmolStr, f32>` field on `InvertedFileIndex`; populate after posting-list build |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: extend `score_entry_match` signature to accept `&[(token, idf)]` instead of `&[String]` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: `apply_event_create` rebuilds IDF for the affected token (or marks IDF stale; recompute at next search) |
| `crates/feature-launcher/src/search/inverted_index.rs` | Add: tests `idf_downweights_common_tokens`, `idf_recomputed_on_incremental_add` |
| `crates/feature-launcher/benches/inverted_index.rs` | Add: a "ranking quality" bench fixture that asserts ordering on a known corpus (use `criterion`'s `bench_with_input` with `assert_eq!` on returned top-1) |

### Tasks

- [ ] **2.1** Compute IDF after posting-list dedupe in `build()`:
      ```rust
      let n = entries.len() as f32;
      let idf: FxHashMap<SmolStr, f32> = postings.iter().map(|(k, list)| {
          let df = list.len() as f32;
          let w = ((n - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.05);
          (k.clone(), w)
      }).collect();
      ```

- [ ] **2.2** Change `score_entry_match` to scale tier scores by IDF:
      ```rust
      let weighted = (tier as f32 * idf_for(q)) as i32;
      score += weighted;
      ```

- [ ] **2.3** On `apply_event_create` and `apply_event_remove`: recompute IDF for affected tokens only, or set a `dirty: AtomicBool` flag and recompute at next search if dirty (cheaper). Pick the dirty-flag approach — incremental IDF recomputation is bug-prone and the recompute cost is tens of µs.

- [ ] **2.4** Tests:
      - `idf_downweights_common_tokens`: build a corpus where `test` appears in 80% of files and `parser` in 5%; query `parser` and assert it returns the parser file as #1.
      - `idf_recomputed_on_incremental_add`: build small index, add many files with the same token, then assert IDF for that token has dropped.

- [ ] **2.5** Re-bench. Acceptance: no regression on perf benches (allow ±5%); ranking-fixture bench passes.

### Acceptance criteria
- IDF values computed & cached in `idf` field.
- `score_entry_match` reads from cache, no re-allocation.
- Ranking fixtures pass.

---

## Phase 3 — Trie + Roaring bitmap intersection

**Effort:** 1 week. **Target:** `inverted_index_search_50000/re` ≤ 80 µs; synthetic 200k bench `re` ≤ 280 µs (the project goal).

### Background

The `re@50k → re@200k` extrapolation says today's design hits ~1.1 ms at 200k. Two structural changes get us under 300 µs:

1. **Stop pre-exploding prefixes.** Replace `HashMap<SmolStr-prefix, Vec<u32>>` with a `fst::Map` keyed by *full tokens*. Short-prefix queries walk an FST `range(prefix..)` and union the matching posting lists. Long-prefix queries do an exact-key lookup as today.
2. **Roaring bitmaps for postings.** SIMD-vectorised ANDs and unions, plus 5–10× memory reduction for the dense ranges.

### File structure

| File | Change |
|---|---|
| `crates/feature-launcher/Cargo.toml` | Modify: add `fst = "0.4"`, `roaring = "0.10"` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: replace `postings: FxHashMap<SmolStr, Vec<u32>>` with new internal `TokenIndex` |
| `crates/feature-launcher/src/search/token_index.rs` | Add: new module with `TokenIndex { fst: fst::Map<Vec<u8>>, postings: Vec<RoaringBitmap> }` |
| `crates/feature-launcher/src/search/token_index.rs` | Add: `fn candidates_for(&self, q: &str) -> Option<RoaringBitmap>` — short query → trie range walk + union; long query → exact-key lookup |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: `search()` swaps `Vec<u32>` candidate intersect for `RoaringBitmap & RoaringBitmap` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: `apply_event_create` uses a "delta builder" (`FxHashMap<SmolStr, Vec<u32>>` of pending insertions); compacted into the FST every N inserts or every M seconds |
| `crates/feature-launcher/src/search/inverted_index.rs` | Add: tests for trie prefix walk, Roaring intersect correctness, delta-then-compact roundtrip |
| `crates/feature-launcher/benches/inverted_index.rs` | Modify: add 200k case (gated behind `BENCH_LARGE=1` env var so CI stays cheap) |

### Tasks

- [ ] **3.1** Add `fst` and `roaring` deps. Lock versions in `Cargo.toml`.

- [ ] **3.2** Define `TokenIndex`:
      ```rust
      // crates/feature-launcher/src/search/token_index.rs
      use fst::{IntoStreamer, Map, MapBuilder, Streamer};
      use roaring::RoaringBitmap;

      pub(crate) struct TokenIndex {
          /// Token bytes → posting list id.
          fst: Map<Vec<u8>>,
          /// One bitmap per terminal node in the FST.
          postings: Vec<RoaringBitmap>,
          /// IDF weight per token id (parallel to `postings`).
          idf: Vec<f32>,
          /// Pending insertions waiting for compaction.
          delta: FxHashMap<SmolStr, RoaringBitmap>,
          delta_pending: usize,
      }

      impl TokenIndex {
          pub fn build(tokens: Vec<(SmolStr, RoaringBitmap, f32)>) -> Self { … }

          /// Returns a bitmap of doc ids whose tokens match the given query.
          /// Short queries (< 3 chars) do a trie range walk + union.
          /// Longer queries first try exact match, falling back to range walk.
          pub fn candidates_for(&self, q: &str) -> Option<(RoaringBitmap, f32)> {
              if q.len() >= 3 {
                  if let Some(id) = self.fst.get(q.as_bytes()) {
                      let i = id as usize;
                      return Some((self.postings[i].clone(), self.idf[i]));
                  }
              }
              // Range walk: union all postings under `q..` until first key
              // that no longer starts with q.
              let mut union = RoaringBitmap::new();
              let mut idf_min = f32::MAX;
              let stream = self.fst
                  .range()
                  .ge(q.as_bytes())
                  .into_stream();
              let mut s = stream;
              while let Some((key, id)) = s.next() {
                  if !key.starts_with(q.as_bytes()) { break; }
                  let i = id as usize;
                  union |= &self.postings[i];
                  idf_min = idf_min.min(self.idf[i]);
              }
              if union.is_empty() { None } else { Some((union, idf_min)) }
          }

          pub fn insert_pending(&mut self, token: SmolStr, doc: u32) { … }
          pub fn compact(&mut self) { … }   // merge delta into a new FST
      }
      ```

- [ ] **3.3** Migrate `InvertedFileIndex::build` to populate the new `TokenIndex`. Steps:
      1. Walk the corpus exactly as today, collecting `FxHashMap<SmolStr, RoaringBitmap>` for *full tokens only* (no prefix explosion).
      2. Compute IDF per token.
      3. Sort the token map by key, feed into `MapBuilder::insert(key, idx)`.
      4. Move `RoaringBitmap`s into `postings: Vec<RoaringBitmap>` indexed by `idx`.
      5. Finalise `Map` from the builder bytes.

- [ ] **3.4** Migrate `search()` to:
      ```rust
      let per_token: Vec<(RoaringBitmap, f32)> = tokens.iter()
          .map(|t| index.candidates_for(t))
          .collect::<Option<Vec<_>>>()?;
      // Smallest first.
      let mut sorted = per_token; sorted.sort_by_key(|(b, _)| b.len());
      let mut cand = sorted[0].0.clone();
      for (b, _) in &sorted[1..] {
          cand &= b;
          if cand.is_empty() { return vec![]; }
      }
      // Score with min-heap (Phase 1's logic, just iterating the bitmap).
      ```

- [ ] **3.5** Incremental updates: keep a "delta" `FxHashMap<SmolStr, RoaringBitmap>` collected in `apply_event_create`. After 256 inserts (configurable) or 30 s, compact: rebuild the FST from `current_tokens ∪ delta_keys`, merge bitmaps, swap in. **Compaction is O(n) but we batch.**
      - For removals, mark `tombstones: RoaringBitmap`. Search filters `cand -= &self.tombstones`. Compact during the same flush.
      - Tests: `delta_compaction_roundtrip` builds, deletes, adds, confirms search post-compact matches a fresh build.

- [ ] **3.6** 200k-entry bench. Gate behind `BENCH_LARGE=1` env so default `cargo bench` stays fast. Update README to mention.

- [ ] **3.7** Memory regression check: log `idx.size_bytes()` before/after; expected drop from ~12 MB → ~2 MB at 50k.

### Acceptance criteria
- `cargo bench --baseline initial` shows ≥70% speedup on `…/re` at 50k.
- `BENCH_LARGE=1 cargo bench` shows `inverted_index_search_200000/re` ≤ 280 µs.
- All existing search tests pass.
- New tests cover delta compaction, range walk for short prefixes, and tombstone correctness.

### Risks & mitigations
- **FST is read-only.** Incremental adds must go through the delta + compact cycle. Mitigate with a small per-entry "delta posting list" that search merges in. Watch the worst-case (delta size before compaction) — we cap at 1024 deltas so search-side merge stays ≤ a few µs.
- **`fst::Map<Vec<u8>>` keys must be inserted sorted.** Build path must sort the token list before feeding the builder; this is O(T log T) where T = unique tokens (~10× fewer than today's prefix-exploded keys).
- **Range walk on degenerate corpora.** A 1-char query like `r` could match 10k+ tokens; cap the union by streaming an early-exit bitmap of size ≤ N (where N = limit × 50). Bench worst-case 1-char query.

---

## Phase 4 — `entity_attention` schema + nightly aggregator

**Effort:** 1 week. **Target:** Klynt knows which apps & sites the user actually lives in, blending live `productivity::activity_events` with `launcher_usage_log`.

### Background

`launcher_usage_log` records *click* frecency. `productivity::activity_events` records *attention seconds* with bundle_id, site_name, url, duration, category — far richer signal. The aggregator condenses 90 days × 14-day-decayed activity into a denormalized `entity_attention` table that the launcher can query in O(log n).

### File structure

| File | Change |
|---|---|
| `crates/feature-launcher/migrations/002_entity_attention.sql` | Add: new migration with `entity_attention` table, indexes, FTS5 mirror, sync triggers |
| `crates/feature-launcher/src/lib.rs` | Modify: append migration to `launcher_migrations()` |
| `crates/feature-launcher/src/repos/entity_attention.rs` | Add: `EntityAttentionRepo` with `top_by_attention`, `fts_search`, `upsert`, `purge_stale` |
| `crates/feature-launcher/src/repos/mod.rs` | Modify: re-export the new repo |
| `crates/feature-launcher/src/services/attention_aggregator.rs` | Add: `AttentionAggregator` with `rebuild_from_activity(window: Duration)` |
| `crates/feature-launcher/src/services/mod.rs` | Add (if absent) and register the new module |
| `crates/app-core/src/init/launcher.rs` | Modify: instantiate `AttentionAggregator` when productivity_repos is `Some`; spawn nightly cron via `CronExecutor` (or use `TemporalScheduler` if simpler) |
| `crates/app-core/src/handlers/launcher/handlers.rs` | Add: `launcher_rebuild_attention` handler (manual trigger from settings UI) |
| `crates/desktop/src/commands/launcher.rs` | Add: `launcher_rebuild_attention` IPC |
| `crates/desktop/src/specta_builder.rs` | Modify: register the new command |
| `crates/feature-launcher/src/repos/entity_attention.rs` | Add: tests for upsert, decayed sum SQL, FTS sync triggers |

### Schema (verbatim — paste into migration)

```sql
-- crates/feature-launcher/migrations/002_entity_attention.sql

CREATE TABLE IF NOT EXISTS entity_attention (
    canonical_id   TEXT NOT NULL,
    kind           TEXT NOT NULL,            -- 'app' | 'site' | 'file' | 'note' | 'task'
    display_name   TEXT NOT NULL,
    -- Decay-weighted attention seconds (14-day half-life).
    attention_secs INTEGER NOT NULL DEFAULT 0,
    last_used_at   TEXT NOT NULL,            -- ISO 8601 UTC
    icon_hint      TEXT,
    category       TEXT,
    PRIMARY KEY (canonical_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_attention_kind_score
    ON entity_attention(kind, attention_secs DESC);
CREATE INDEX IF NOT EXISTS idx_attention_last_used
    ON entity_attention(last_used_at DESC);

-- FTS5 mirror keyed on canonical_id+kind.
CREATE VIRTUAL TABLE IF NOT EXISTS entity_attention_fts USING fts5(
    canonical_id UNINDEXED,
    kind UNINDEXED,
    display_name,
    title_corpus,                            -- populated by Phase 6 (window titles)
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS entity_attention_ai AFTER INSERT ON entity_attention BEGIN
    INSERT INTO entity_attention_fts (canonical_id, kind, display_name)
    VALUES (NEW.canonical_id, NEW.kind, NEW.display_name);
END;

CREATE TRIGGER IF NOT EXISTS entity_attention_au AFTER UPDATE OF display_name ON entity_attention BEGIN
    DELETE FROM entity_attention_fts
        WHERE canonical_id = OLD.canonical_id AND kind = OLD.kind;
    INSERT INTO entity_attention_fts (canonical_id, kind, display_name)
        VALUES (NEW.canonical_id, NEW.kind, NEW.display_name);
END;

CREATE TRIGGER IF NOT EXISTS entity_attention_ad AFTER DELETE ON entity_attention BEGIN
    DELETE FROM entity_attention_fts
        WHERE canonical_id = OLD.canonical_id AND kind = OLD.kind;
END;
```

### Tasks

- [ ] **4.1** Land migration `002_entity_attention.sql`. Pre-release, so direct schema changes are fine; bump `feature_name="launcher"` migration version to `2`.

- [ ] **4.2** Implement `EntityAttentionRepo`:
      ```rust
      pub struct EntityAttentionRepo { pool: SqlitePool }

      #[derive(Debug, Clone, sqlx::FromRow, specta::Type, Serialize)]
      pub struct EntityAttentionRow {
          pub canonical_id: String,
          pub kind: String,
          pub display_name: String,
          pub attention_secs: i64,
          pub last_used_at: String,
          pub icon_hint: Option<String>,
          pub category: Option<String>,
      }

      impl EntityAttentionRepo {
          pub async fn top_by_attention(&self, kind: Option<&str>, limit: i64) -> Result<Vec<EntityAttentionRow>>;
          pub async fn fts_search(&self, q: &str, limit: i64) -> Result<Vec<EntityAttentionRow>>;
          pub async fn upsert(&self, row: &EntityAttentionRow) -> Result<()>;
          pub async fn purge_older_than(&self, days: i64) -> Result<u64>;
          /// Returns the personal score for a given file path on a 0..1 scale.
          /// Used by Phase 5 task 5.3 to populate `IndexEntry.personal`.
          pub async fn personal_for_paths(&self, paths: &[String]) -> Result<HashMap<String, f32>>;
      }
      ```
      The FTS query uses `bm25(entity_attention_fts) * 0.4 + (attention_secs / max_attention) * 0.6` as the ranking expression — this hands the user "the IDE that I actually use" not "the IDE that ranks well by name match alone".

- [ ] **4.3** Implement `AttentionAggregator::rebuild_from_activity` with the SQL from the design doc. Performance target: 90 days × 50k events → < 2 s on Apple Silicon.

      ```rust
      impl AttentionAggregator {
          pub async fn rebuild_from_activity(&self, window_days: i64) -> Result<u64> {
              let n = sqlx::query(
                  r#"
                  INSERT INTO entity_attention
                       (canonical_id, kind, display_name, attention_secs, last_used_at, category)
                  SELECT
                      COALESCE(NULLIF(e.bundle_id,''), lower(NULLIF(e.site_name,'')), lower(e.app_name)) AS canonical_id,
                      CASE WHEN e.site_name IS NOT NULL THEN 'site' ELSE 'app' END AS kind,
                      COALESCE(e.site_name, e.app_name) AS display_name,
                      CAST(SUM(e.duration_secs * exp(-(julianday('now') - julianday(e.started_at)) / 14.0)) AS INTEGER) AS attention_secs,
                      MAX(e.started_at) AS last_used_at,
                      MAX(c.name) AS category
                  FROM activity_events e
                  LEFT JOIN activity_categories c ON e.category_id = c.id
                  WHERE e.started_at > datetime('now', ?1)
                    AND e.is_idle = FALSE
                    AND COALESCE(e.bundle_id, e.site_name, e.app_name) IS NOT NULL
                  GROUP BY canonical_id, kind
                  HAVING attention_secs > 0
                  ON CONFLICT(canonical_id, kind) DO UPDATE SET
                      attention_secs = excluded.attention_secs,
                      last_used_at   = excluded.last_used_at,
                      display_name   = excluded.display_name,
                      category       = excluded.category;
                  "#,
              )
              .bind(format!("-{} days", window_days))
              .execute(&self.pool)
              .await?
              .rows_affected();
              Ok(n)
          }
      }
      ```
      Note: SQLite's `exp()` is not built-in by default. Either (a) load the math extension, or (b) implement decay in Rust by streaming events. **Pick (b)** — avoids bundling extensions and is portable.

- [ ] **4.4** Wire the aggregator into `init_launcher`:
      - If `productivity_repos.is_some()`, instantiate `AttentionAggregator { pool, productivity_pool }`.
      - Schedule a daily cron: `0 3 * * *` (3am local) via `CronExecutor`. Failure = log + skip; never crash the app.
      - Run once eagerly on first launch so brand-new installs don't see an empty attention table.

- [ ] **4.5** Settings UI (`desktop-ui`): "Rebuild attention now" button calling `launcher_rebuild_attention`, with toast on completion. Already covered by the typed-command-macros plan.

- [ ] **4.6** Tests:
      - `aggregator_rebuild_from_seeded_events_yields_expected_attention`
      - `aggregator_idempotent_on_repeated_rebuild`
      - `repo_fts_search_ranks_by_combined_bm25_and_attention`
      - `repo_top_by_attention_filters_by_kind`

### Acceptance criteria
- Migration applies cleanly on fresh + existing DBs.
- Aggregator rebuilds 90d × 50k events in < 2 s.
- FTS5 mirror returns results for prefix and full-token queries.
- All triggers verified by an integration test that mutates `entity_attention` and reads `entity_attention_fts`.

### Privacy / safety
- Aggregator only reads `app_name`, `site_name`, `bundle_id`, `url` host, and `duration_secs`. Window titles + URL paths are NOT touched in this phase.
- A user-facing setting `launcher.attention.enabled` defaults to `true` but documented in `~/.klyntbot/config.json` so it can be turned off.

---

## Phase 5 — `AttentionSource` + index-level personalization

**Effort:** 4–5 days. **Target:** the launcher's empty-query / app-search experience uses real attention, and file search results are personalized.

### File structure

| File | Change |
|---|---|
| `crates/feature-launcher/src/search/attention.rs` | Add: `AttentionSource` impl `SearchSource`; queries `EntityAttentionRepo` |
| `crates/feature-launcher/src/search/mod.rs` | Modify: add `pub mod attention;` and re-export |
| `crates/feature-launcher/src/types.rs` | Modify: extend `LauncherItemKind` with `Attention { canonical_id, kind, attention_secs }` (or reuse existing `Application`/`Bookmark` kinds with a `personal_score` field) |
| `crates/app-core/src/init/launcher.rs` | Modify: register `AttentionSource` when `entity_attention` is populated |
| `crates/app-core/src/handlers/launcher/search_engine.rs` | Modify: `top_frecency`-driven empty default replaced with `entity_attention.top_by_attention` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Add: optional `personal: Vec<f32>` parallel to `entries`; populated by builder via injected `PersonalScorer` trait |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: scorer applies personal boost when `personal[i] > 0` |
| `desktop-ui/src/features/launcher/components/EmptyState.tsx` | Modify: render attention-ranked apps & sites instead of frecency stub |
| `desktop-ui/src/features/launcher/components/EmptyState.test.tsx` | Modify: assert "Apps you live in" header + at least one row |

### Tasks

- [ ] **5.1** `AttentionSource`:
      ```rust
      pub struct AttentionSource { repo: Arc<EntityAttentionRepo> }

      #[async_trait]
      impl SearchSource for AttentionSource {
          fn name(&self) -> &'static str { "attention" }
          fn cache_ttl(&self) -> Option<Duration> { Some(Duration::from_secs(30)) }

          async fn search(&self, q: &str, limit: usize) -> Vec<LauncherItem> {
              let rows = if q.is_empty() {
                  self.repo.top_by_attention(None, limit as i64).await.unwrap_or_default()
              } else {
                  self.repo.fts_search(q, limit as i64).await.unwrap_or_default()
              };
              rows.into_iter().map(into_launcher_item).collect()
          }
      }
      ```

- [ ] **5.2** Map `EntityAttentionRow` → `LauncherItem` (`into_launcher_item` helper). Site rows become `LauncherItemKind::UrlNavigation { url }` with the canonical_id as the host; app rows become `Application { path, running }` if we can resolve the bundle path, otherwise a new `LauncherItemKind::AttentionApp { bundle_id }` for cases where the app isn't on disk anymore.

- [ ] **5.3** Index-level personalization:
      ```rust
      // In InvertedFileIndex::build, after entries are populated:
      if let Some(scorer) = personal_scorer {
          let paths: Vec<String> = entries.iter()
              .map(|e| e.path.to_string_lossy().to_string()).collect();
          let scores = scorer.scores_for(&paths).await.unwrap_or_default();
          let max = scores.values().copied().fold(0.0_f32, f32::max).max(1.0);
          self.personal = entries.iter().map(|e| {
              scores.get(&e.path.to_string_lossy().to_string())
                    .copied().unwrap_or(0.0) / max
          }).collect();
      }
      ```
      Use a trait so tests can inject a mock:
      ```rust
      #[async_trait]
      pub trait PersonalScorer: Send + Sync {
          async fn scores_for(&self, paths: &[String]) -> Result<HashMap<String, f32>>;
      }
      ```
      Default impl wraps `EntityAttentionRepo::personal_for_paths`.

- [ ] **5.4** Scorer integration: scoring path adds `score += (entry.personal * PERSONAL_BOOST) as i32` where `PERSONAL_BOOST = 60` (cap). Cap is **lower than the lowest tier (44)** so personal score breaks ties without overriding strong name matches.

- [ ] **5.5** Empty-state UI: replace the current `top_frecency` 8-row default in `LauncherSearchEngine::search` with a call to `entity_attention.top_by_attention(kind=None, 8)`. Frontend: `EmptyState` renders kind-grouped sections ("Apps you use", "Sites you visit").

- [ ] **5.6** Tests:
      - `attention_source_returns_top_apps_when_query_empty`
      - `attention_source_fts_ranks_by_combined_score`
      - `inverted_index_personal_score_breaks_ties`
      - `inverted_index_personal_score_respects_cap`
      - `empty_state_renders_attention_sections` (frontend)

### Acceptance criteria
- Empty launcher view shows attention-ranked apps/sites.
- Typing partial app name returns it ranked by combined name+attention score.
- File-search ranking respects personal score as a tie-breaker (verified in unit test on a fixture corpus).

---

## Phase 6 — Window-title content recall + build/memory perf (opportunistic)

**Effort:** 3–5 days for window titles; 2–3 days for parallel build & generation tombstoning. **Target:** "the Q3 deck I edited yesterday" → returns the actual file/window, opt-in.

### Background

`activity_events.window_title` captures the active window every few seconds — a continuous, content-rich signal. Tokenized and indexed into `entity_attention_fts.title_corpus`, it gives Klynt a *cognitive recall* mode: search by what you were working on, not by filename.

This is gated behind privacy controls because window titles can include sensitive data. Default off; opt-in via `launcher.attention.indexTitles = true` plus an allowlist/blocklist of bundle IDs.

### File structure

| File | Change |
|---|---|
| `crates/config/src/schema/launcher.rs` | Modify: add `attention: AttentionSettings { enabled: bool, index_titles: bool, redact_bundle_ids: Vec<String> }` |
| `crates/feature-launcher/src/services/title_corpus_builder.rs` | Add: nightly job that tokenizes + redacts window titles, populates `entity_attention_fts.title_corpus` |
| `crates/feature-launcher/src/services/title_redactor.rs` | Add: redaction rules — strip URL paths, mask password-like tokens, drop entries from blocklisted bundles |
| `crates/feature-launcher/src/types.rs` | Modify: add `LauncherItemKind::AttentionTitle { event_id, app_name }` |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: add generation-counter tombstoning (`gen: Vec<u32>`), search filters dead entries; `apply_event_remove` becomes O(1) |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: optional `rayon` parallel walk, gated behind `parallel-build` feature flag |
| `desktop-ui/src/features/settings/components/AttentionSettings.tsx` | Add: opt-in toggle + redaction list editor |

### Tasks

- [ ] **6.1** Config schema. Default `index_titles=false`, `redact_bundle_ids=["com.1password.*", "com.lastpass.*", "com.apple.keychainaccess"]`.

- [ ] **6.2** `TitleRedactor`:
      ```rust
      pub fn redact(title: &str, bundle: &str, blocklist: &[String]) -> Option<String> {
          if blocklist.iter().any(|p| glob_match(p, bundle)) { return None; }
          let mut t = title.to_owned();
          // Strip URL paths beyond the host (keep "github.com" but drop "/owner/repo/issues/42")
          // Mask 16+ char alphanumerics that look like tokens.
          // Drop trailing app suffixes ("— Visual Studio Code").
          Some(t)
      }
      ```
      Tests: cover all the redaction rules.

- [ ] **6.3** `TitleCorpusBuilder` runs nightly after the attention aggregator. SQL pattern:
      ```sql
      UPDATE entity_attention
      SET title_corpus_pending = (
          SELECT group_concat(window_title, ' ')
          FROM activity_events
          WHERE bundle_id = entity_attention.canonical_id
            AND started_at > datetime('now', '-30 days')
      )
      WHERE kind = 'app';
      ```
      Then: stream rows, redact in Rust, update `entity_attention_fts`.

- [ ] **6.4** Generation-counter tombstoning in `InvertedFileIndex`:
      - Add `gen: u32` to `IndexEntry`; add `current_gen: u32` to the index.
      - `apply_event_remove` flips `gen` to a sentinel — no posting-list mutation.
      - `search()` filters `entry.gen != tombstone`.
      - Periodic compaction (`compact_if_needed`) rebuilds the FST and posting lists when tombstone count exceeds 5% of total.

- [ ] **6.5** Parallel build (gated): use `rayon` to parallelize `WalkBuilder` per-root; aggregate via channel. Feature-gate so default builds stay simple.

- [ ] **6.6** Tests:
      - `title_redactor_strips_url_paths`
      - `title_redactor_drops_blocklisted_bundles`
      - `tombstoning_o1_remove`
      - `compact_after_tombstone_threshold_rebuilds_fst`
      - `parallel_build_matches_serial_build` (snapshot-based)

### Acceptance criteria
- Default config keeps title indexing off.
- Opt-in flow is one toggle in settings + restart.
- `entity_attention_fts.title_corpus` populated correctly for opt-in users.
- Tombstoned `apply_event_remove` is O(1) by bench (compare 1k removals on a 50k index — should drop from ~seconds to ms).

### Risks
- **Privacy**: Audit the redactor with adversarial fixture titles before shipping. Document explicitly that the user owns this feature flag.
- **FTS5 tokenizer**: Default `unicode61` is fine for English/code names; for Japanese/Chinese filenames look at `trigram` or `icu`. Out of scope for v1.

---

## Cross-phase test plan

- [ ] All `cargo nextest run -p feature-launcher` tests green at every phase.
- [ ] After Phase 1: `cargo bench --baseline initial` shows ≥40% speedup on `…/re@50k`.
- [ ] After Phase 2: ranking-quality fixture passes; perf within ±5%.
- [ ] After Phase 3: `BENCH_LARGE=1 cargo bench` shows `…/re@200k` ≤ 280 µs.
- [ ] After Phase 4: aggregator integration test seeds 50k synthetic events and rebuilds in < 2 s.
- [ ] After Phase 5: empty launcher renders attention sections; manual smoke matches user expectations.
- [ ] After Phase 6 (if shipped): redaction regression suite passes; opt-in flow documented.

## Roll-out plan

1. Phase 1 + 2 ship as one PR (single bench run, single ranking change). Tag commit.
2. Phase 3 ships as a separate PR with the saved baseline diff in the description.
3. Phase 4 + 5 ship together (schema + UX wire-up are coupled).
4. Phase 6 ships as an opt-in followup PR with explicit privacy doc.

## Test-plan checklist (PR template)

- [ ] `cargo nextest run --workspace`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo bench -p feature-launcher -- --baseline initial` results pasted in PR description
- [ ] `bun run test && bun run typecheck && bun run lint`
- [ ] Manual smoke: type a 1-char query, 2-char query, multi-word query; verify <16 ms perceived latency
- [ ] Manual smoke: open empty launcher; verify attention-ranked default view
- [ ] If Phase 4: verify aggregator runs successfully on first launch
- [ ] If Phase 6: verify default-off + opt-in toggle works

---

## Future work (not in this plan)

- **Cross-device attention**: sync `entity_attention` over an end-to-end-encrypted channel so the launcher knows what you use *across* machines.
- **Project-aware ranking**: when a focus session is active, boost files within `focus_session.project_id` first.
- **Embedding-based recall**: vectorize `display_name + title_corpus` into LanceDB; let users search by intent ("the doc about latency"). Synergistic with the existing cognitive memory layer.
- **Suggested actions**: when `entity_attention_fts.bm25() > threshold`, prefetch a "Resume" launcher row that re-opens the last window/tab — Raycast-style continuity.
