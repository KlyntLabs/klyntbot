# Coding-in-Chat Phase 2 — Performance Pass Notes

**Date:** 2026-05-02
**Status:** Partial — Optimization A applied; B and C documented for future work.

## Baseline

Micro-benches in `crates/agent/benches/chat_send_to_first_token.rs` cover:
- `layer3_mirror_approval_eval` — approval evaluator hot path
- `tool_search_10_results` — lexical index search
- `layer3_args_hash_bash` / `layer3_args_hash_edit` — relevance hashing

Full e2e `chat_send_to_first_token` bench deferred (requires agent test-helper crate stabilisation).

## Optimisation A — SoulContextSource mtime memoisation ✅ Applied

**Problem:** `SoulContextSource::provide()` read `KLYNTBOT.md` from disk on *every* turn, even when the file had not changed. In a long coding session this adds unnecessary I/O latency to the first-token path.

**Fix:** Added `last_mtime: Arc<RwLock<Option<SystemTime>>>` to `SoulContextSource`. `provide()` now:
1. Checks `metadata().modified()` against the cached mtime.
2. Returns the in-memory cache immediately when mtime is unchanged.
3. Only re-reads from disk when the file has actually been modified.

**Commit:** `perf(skill-system): SoulContextSource mtime memoization`

## Optimisation B — ToolRegistry per-thread cache 📋 Documented

**Hypothesis:** `ToolKitBuilder::register_all()` rebuilds the full `ToolRegistry` on every turn. For the curated coding profile (24 tools) this is mostly register calls + Arc clones, but it still allocates.

**Approach:** Cache the built `Vec<Arc<dyn Tool>>` (or the entire `ToolRegistry`) per-thread, keyed by `(tool_profile, channel)`. Invalidate on `/power` toggle or mode flip.

**Status:** Not applied. Profiling required to confirm this is a bottleneck. The bench micro-tests show `tool_search` is fast; registry rebuild may be negligible compared to provider RTT.

## Optimisation C — SkillActivator LRU 📋 Documented

**Hypothesis:** `SkillActivator` re-evaluates conditional skills on every file touch, even for the same `(repo_id, file_paths)` combination within a session.

**Approach:** Add an LRU cache keyed by `(repo_id, file_paths_hash)` with a small TTL (e.g. 60s) so repeated touches of the same files in a single turn don't trigger redundant skill walks.

**Status:** Not applied. The plan's target is 30ms for 50 skills; if current profiling is already under that, this cache adds complexity for no gain.

## Next Steps

1. Run `cargo bench -p agent --bench chat_send_to_first_token` to establish baseline numbers.
2. If `layer3_mirror_approval_eval` or `tool_search_10_results` are > 1ms, investigate further.
3. Once the agent test-helper crate stabilises, land the full e2e bench and measure p95.
4. If p95 > 800ms, flamegraph the hot path and evaluate B and C.
