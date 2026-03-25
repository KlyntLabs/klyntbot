# Enhanced Loop Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the broken `detect_oscillation()` (generic string comparison) with a hash-based `LoopDetector` that identifies repeating tool call patterns and escalates progressively (warn@3, hard-stop@5).

**Architecture:** A new `LoopDetector` struct in `execution/loop_detector.rs` hashes sorted `(tool_name, args)` pairs per iteration and tracks them in a sliding window. The `ReactiveEngine` feeds tool signatures from `CycleOutcome::ToolsExecuted` into the detector after each iteration, and handles `LoopStatus::Warning` (inject steering message) and `LoopStatus::HardStop` (strip tools, force synthesis). New `AgentEvent` variants surface detection in the transparency panel.

**Tech Stack:** Rust (agent crate), `DefaultHasher`, `serde_json`

**Spec:** `docs/superpowers/specs/2026-03-25-enhanced-loop-detection-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/agent/src/execution/loop_detector.rs` | `LoopDetector` struct, `LoopStatus` enum, `IterationSignature`, hash computation, all unit tests |

### Modified files

| File | Change |
|------|--------|
| `crates/agent/src/execution/scratchpad.rs` | Remove `detect_oscillation()`, add `pub loop_detector: LoopDetector` field |
| `crates/agent/src/execution/mod.rs` | Add `pub mod loop_detector;`, re-export types |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Replace oscillation check (~line 334) with `loop_detector.record_iteration()`, handle Warning/HardStop |
| `crates/agent/src/events.rs` | Add `LoopDetected` and `LoopHardStop` variants |
| `crates/app-core/src/handlers/chat/streaming.rs` | Add match arms for new variants (~line 941) |

---

## Task 1: Create LoopDetector with Hash Computation + Tests

**Files:**
- Create: `crates/agent/src/execution/loop_detector.rs`
- Modify: `crates/agent/src/execution/mod.rs`

- [ ] **Step 1: Write tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_hash_consistency() {
        let calls = vec![
            ("search".to_string(), json!({"query": "hello"})),
            ("fetch".to_string(), json!({"url": "http://example.com"})),
        ];
        let h1 = compute_iteration_hash(&calls);
        let h2 = compute_iteration_hash(&calls);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_order_independence() {
        let calls_a = vec![
            ("search".to_string(), json!({"query": "hello"})),
            ("fetch".to_string(), json!({"url": "http://x.com"})),
        ];
        let calls_b = vec![
            ("fetch".to_string(), json!({"url": "http://x.com"})),
            ("search".to_string(), json!({"query": "hello"})),
        ];
        assert_eq!(compute_iteration_hash(&calls_a), compute_iteration_hash(&calls_b));
    }

    #[test]
    fn test_different_args_different_hash() {
        let calls_a = vec![("search".to_string(), json!({"query": "hello"}))];
        let calls_b = vec![("search".to_string(), json!({"query": "world"}))];
        assert_ne!(compute_iteration_hash(&calls_a), compute_iteration_hash(&calls_b));
    }

    #[test]
    fn test_no_loop_below_threshold() {
        let mut det = LoopDetector::new();
        let calls = vec![("search".to_string(), json!({"q": "a"}))];
        assert!(matches!(det.record_iteration(1, &calls), LoopStatus::NoLoop));
        assert!(matches!(det.record_iteration(2, &calls), LoopStatus::NoLoop));
    }

    #[test]
    fn test_warning_at_threshold() {
        let mut det = LoopDetector::new();
        let calls = vec![("search".to_string(), json!({"q": "a"}))];
        det.record_iteration(1, &calls);
        det.record_iteration(2, &calls);
        let status = det.record_iteration(3, &calls);
        assert!(matches!(status, LoopStatus::Warning { count: 3, .. }));
    }

    #[test]
    fn test_warning_once_per_hash() {
        let mut det = LoopDetector::new();
        let calls = vec![("search".to_string(), json!({"q": "a"}))];
        det.record_iteration(1, &calls);
        det.record_iteration(2, &calls);
        let s3 = det.record_iteration(3, &calls);
        assert!(matches!(s3, LoopStatus::Warning { .. }));
        // 4th identical → NoLoop (already warned for this hash)
        let s4 = det.record_iteration(4, &calls);
        assert!(matches!(s4, LoopStatus::NoLoop));
    }

    #[test]
    fn test_hard_stop_at_threshold() {
        let mut det = LoopDetector::new();
        let calls = vec![("search".to_string(), json!({"q": "a"}))];
        for i in 1..=4 { det.record_iteration(i, &calls); }
        let s5 = det.record_iteration(5, &calls);
        assert!(matches!(s5, LoopStatus::HardStop { count: 5, .. }));
    }

    #[test]
    fn test_different_hash_resets_count() {
        let mut det = LoopDetector::new();
        let calls_a = vec![("search".to_string(), json!({"q": "a"}))];
        let calls_b = vec![("fetch".to_string(), json!({"url": "x"}))];
        det.record_iteration(1, &calls_a);
        det.record_iteration(2, &calls_a);
        // Break the streak
        det.record_iteration(3, &calls_b);
        // Resume — count restarts from 1
        let s4 = det.record_iteration(4, &calls_a);
        assert!(matches!(s4, LoopStatus::NoLoop));
    }

    #[test]
    fn test_sliding_window_eviction() {
        let mut det = LoopDetector::with_config(5, 3, 5); // window=5
        let calls = vec![("search".to_string(), json!({"q": "a"}))];
        // Fill window with different hashes
        for i in 0..6 {
            det.record_iteration(i, &[
                (format!("tool_{i}"), json!({})),
            ]);
        }
        assert!(det.history.len() <= 5);
    }

    #[test]
    fn test_empty_tool_calls() {
        let mut det = LoopDetector::new();
        let empty: Vec<(String, serde_json::Value)> = vec![];
        let s = det.record_iteration(1, &empty);
        assert!(matches!(s, LoopStatus::NoLoop));
    }

    #[test]
    fn test_json_key_order_stability() {
        let calls_a = vec![("t".to_string(), json!({"b": 2, "a": 1}))];
        let calls_b = vec![("t".to_string(), json!({"a": 1, "b": 2}))];
        assert_eq!(compute_iteration_hash(&calls_a), compute_iteration_hash(&calls_b));
    }
}
```

- [ ] **Step 2: Implement LoopDetector**

```rust
//! Hash-based loop detection for the ReactiveEngine.
//!
//! Replaces the old `detect_oscillation()` which compared generic action labels.
//! Hashes sorted (tool_name, args) per iteration, tracks in a sliding window,
//! and escalates progressively: warn@3, hard-stop@5.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashSet, VecDeque};
use std::hash::{Hash, Hasher};

const DEFAULT_WINDOW_SIZE: usize = 20;
const DEFAULT_WARN_THRESHOLD: u32 = 3;
const DEFAULT_HARD_STOP_THRESHOLD: u32 = 5;

#[derive(Debug)]
pub struct IterationSignature {
    pub hash: u64,
    pub tools: Vec<String>,
    pub iteration: usize,
}

#[derive(Debug)]
pub enum LoopStatus {
    NoLoop,
    Warning {
        count: u32,
        hash: u64,
        tools_summary: String,
    },
    HardStop {
        count: u32,
        tools_summary: String,
    },
}

#[derive(Debug)]
pub struct LoopDetector {
    history: VecDeque<IterationSignature>,
    warned_hashes: HashSet<u64>,
    window_size: usize,
    warn_threshold: u32,
    hard_stop_threshold: u32,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            warned_hashes: HashSet::new(),
            window_size: DEFAULT_WINDOW_SIZE,
            warn_threshold: DEFAULT_WARN_THRESHOLD,
            hard_stop_threshold: DEFAULT_HARD_STOP_THRESHOLD,
        }
    }

    pub fn with_config(window_size: usize, warn_threshold: u32, hard_stop_threshold: u32) -> Self {
        Self {
            history: VecDeque::new(),
            warned_hashes: HashSet::new(),
            window_size,
            warn_threshold,
            hard_stop_threshold,
        }
    }

    pub fn record_iteration(
        &mut self,
        iteration: usize,
        tool_calls: &[(String, serde_json::Value)],
    ) -> LoopStatus {
        let hash = compute_iteration_hash(tool_calls);
        let tools: Vec<String> = tool_calls.iter().map(|(n, _)| n.clone()).collect();
        let tools_summary = tools.join(", ");

        self.history.push_back(IterationSignature {
            hash,
            tools: tools.clone(),
            iteration,
        });
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // Count consecutive identical hashes at the tail
        let consecutive = self.history.iter().rev()
            .take_while(|sig| sig.hash == hash)
            .count() as u32;

        if consecutive >= self.hard_stop_threshold {
            return LoopStatus::HardStop { count: consecutive, tools_summary };
        }
        if consecutive >= self.warn_threshold {
            // Once-per-hash guard
            if self.warned_hashes.contains(&hash) {
                return LoopStatus::NoLoop;
            }
            self.warned_hashes.insert(hash);
            return LoopStatus::Warning { count: consecutive, hash, tools_summary };
        }
        LoopStatus::NoLoop
    }
}

/// Hash a set of tool calls order-independently.
/// Sorts by (name, key-sorted JSON args) before hashing.
pub fn compute_iteration_hash(tool_calls: &[(String, serde_json::Value)]) -> u64 {
    let mut sorted: Vec<(String, String)> = tool_calls
        .iter()
        .map(|(name, args)| (name.clone(), stable_json(args)))
        .collect();
    sorted.sort();

    let mut hasher = DefaultHasher::new();
    for (name, args_str) in &sorted {
        name.hash(&mut hasher);
        args_str.hash(&mut hasher);
    }
    hasher.finish()
}

/// Recursively serialize a JSON Value with sorted object keys.
fn stable_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let entries: Vec<String> = keys.iter()
                .map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), stable_json(&map[*k])))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(stable_json).collect();
            format!("[{}]", items.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
```

- [ ] **Step 3: Export from mod.rs**

In `crates/agent/src/execution/mod.rs`, add:
```rust
pub mod loop_detector;
```

And add to re-exports:
```rust
pub use loop_detector::{LoopDetector, LoopStatus};
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(loop_detector)'`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(agent): add LoopDetector with hash-based iteration signatures and tests"
```

---

## Task 2: Wire LoopDetector into Scratchpad

**Files:**
- Modify: `crates/agent/src/execution/scratchpad.rs`

- [ ] **Step 1: Add LoopDetector field to Scratchpad**

In the `Scratchpad` struct, add:
```rust
pub loop_detector: LoopDetector,
```

In `Scratchpad::new()`, initialize:
```rust
loop_detector: LoopDetector::new(),
```

Import `use super::loop_detector::LoopDetector;` at the top.

- [ ] **Step 2: Remove detect_oscillation()**

Delete the `detect_oscillation()` method (lines 199-215 in `scratchpad.rs`) and its test(s) if any exist in the test module.

- [ ] **Step 3: Build**

Run: `cargo build -p agent`

This may fail if `detect_oscillation()` is called from `reactive.rs` — that's expected and will be fixed in Task 4. Check if it compiles; if not, temporarily comment out the call site in `reactive.rs:334` with a `// TODO: replace with loop_detector` comment.

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor(agent): replace detect_oscillation with LoopDetector on Scratchpad"
```

---

## Task 3: Add AgentEvent Variants + Streaming Match Arms

**Files:**
- Modify: `crates/agent/src/events.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Add event variants**

In `crates/agent/src/events.rs`, add to the `AgentEvent` enum (before the closing `}`):

```rust
/// The agent detected a repeating tool call pattern (warning level).
LoopDetected {
    iteration: usize,
    #[serde(rename = "toolsSummary")]
    tools_summary: String,
    suggestion: String,
},

/// The agent hit the hard-stop threshold for repeated tool calls.
LoopHardStop {
    iteration: usize,
    #[serde(rename = "toolsSummary")]
    tools_summary: String,
},
```

Note: The serialized type tags are `"loopDetected"` and `"loopHardStop"` (from `rename_all = "camelCase"` on the enum).

- [ ] **Step 2: Add streaming match arms**

In `crates/app-core/src/handlers/chat/streaming.rs`, find the exhaustive match on `AgentEvent` (around line 941, after `ContextCompressed`). Add:

```rust
AgentEvent::LoopDetected { iteration, tools_summary, suggestion } => {
    tracing::info!(
        iteration,
        tools_summary = %tools_summary,
        suggestion = %suggestion,
        "loop detected: repeating tool pattern"
    );
}
AgentEvent::LoopHardStop { iteration, tools_summary } => {
    tracing::warn!(
        iteration,
        tools_summary = %tools_summary,
        "loop hard-stop: forcing synthesis"
    );
}
```

- [ ] **Step 3: Update events_tests.rs**

Find `crates/agent/src/events_tests.rs` (or the test file with `all_variants()` helper). Add the two new variants to the helper so serialization tests cover them.

- [ ] **Step 4: Build**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(agent): add LoopDetected and LoopHardStop AgentEvent variants"
```

---

## Task 4: Wire LoopDetector into ReactiveEngine

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

This is the core integration. Replace the oscillation check with the new loop detector.

- [ ] **Step 1: Read the current reactive engine loop**

Read `crates/agent/src/intent_pipeline/engines/reactive.rs` around lines 310-345 to understand the full context of the oscillation check and what data is available at that point.

- [ ] **Step 2: Replace oscillation check with LoopDetector**

The `outcome` is consumed by the `match outcome { ... }` block (~line 134), so it is NOT available at line 334. You must extract tool signatures INSIDE the `ToolsExecuted` match arm, then use them at the oscillation check point.

**Step 2a: Declare `last_tool_calls` before the loop:**

```rust
let mut last_tool_calls: Vec<(String, serde_json::Value)> = Vec::new();
```

**Step 2b: Inside the `CycleOutcome::ToolsExecuted { results }` match arm (around line 214-270), extract signatures:**

```rust
// At the start of the ToolsExecuted arm, before other processing:
last_tool_calls = results.iter()
    .map(|r| (r.tool_name.clone(), r.arguments.clone()))
    .collect();
```

**Step 2c: Replace the oscillation check (line 334) with the loop detector:**

Remove:
```rust
if scratchpad.detect_oscillation(3) {
    tracing::warn!(
        "ReactiveEngine: oscillation detected at iteration {} — breaking loop",
        iteration
    );
    break;
}
```

Replace with:
```rust
// Loop detection: hash-based iteration signature comparison
if !last_tool_calls.is_empty() {
    let steering_msg = format!(
        "I'm noticing I've been repeating the same set of tools ({}) for the last 3 steps \
         without finding new information. Would you like me to summarize what I've found so far, \
         try a different approach, or keep going?",
        last_tool_calls.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
    );
    match scratchpad.loop_detector.record_iteration(iteration, &last_tool_calls) {
        LoopStatus::NoLoop => {}
        LoopStatus::Warning { tools_summary, .. } => {
            tracing::warn!(
                iteration,
                tools_summary = %tools_summary,
                "ReactiveEngine: loop warning — repeating tool pattern"
            );
            emitter.emit(AgentEvent::LoopDetected {
                iteration,
                tools_summary: tools_summary.clone(),
                suggestion: steering_msg.clone(),
            });
            messages.push(common::Message::user(steering_msg));
        }
        LoopStatus::HardStop { tools_summary, .. } => {
            tracing::warn!(
                iteration,
                tools_summary = %tools_summary,
                "ReactiveEngine: loop hard-stop — forcing synthesis"
            );
            emitter.emit(AgentEvent::LoopHardStop {
                iteration,
                tools_summary,
            });
            break; // Synthesis happens in existing post-loop block
        }
    }
    last_tool_calls.clear();
}
```

Note: The existing post-loop synthesis code (line ~388) already passes `&[]` for tools, so HardStop's tool-stripping is handled automatically.

Also import `LoopStatus` at the top of the file:
```rust
use crate::execution::loop_detector::LoopStatus;
```

- [ ] **Step 3: Handle HardStop synthesis**

At HardStop, the loop breaks. The existing post-loop synthesis code (around line 356-389) already generates a synthesis prompt and makes a final tool-less LLM call. This handles the HardStop case naturally — the loop breaks and falls through to synthesis.

However, to strip tool schemas explicitly for the synthesis call, check if the existing post-loop code already passes empty tools. If it does (search for `&[]` in the synthesis call), no change needed. If it passes the full tool list, change to `&[]`.

- [ ] **Step 4: Build and run tests**

Run: `cargo build -p agent`
Run: `cargo nextest run -p agent`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(agent): wire LoopDetector into ReactiveEngine replacing oscillation check"
```

---

## Task 5: Episodic Memory on HardStop (Optional)

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

The spec says to write an episodic memory on hard-stop. This requires threading `Option<EpisodicMemoryRepo>` into the reactive engine. This is a nice-to-have — if the wiring is complex, defer it.

- [ ] **Step 1: Check how ReactiveEngine is constructed**

Read `crates/agent/src/intent_pipeline/engines/reactive.rs` constructor and `crates/agent/src/` builder to understand how to pass an optional repo.

- [ ] **Step 2: Add optional episodic repo field**

If straightforward (e.g., `ReactiveEngine` already has access to repos or a context struct), add the field and write the episodic memory on HardStop:

```rust
// In HardStop handler, before break:
if let Some(ref episodic) = self.episodic_repo {
    let repo = episodic.clone();
    // Include skill name if available from execution context; fall back to "unknown"
    let content = format!("Loop detected: repeated {} 5 times during task execution", tools_summary);
    tokio::spawn(async move {
        let mem = cognitive::types::EpisodicMemory {
            id: uuid::Uuid::new_v4().to_string(),
            domain: "meta".to_string(),
            content,
            summary: None,
            importance: 0.6,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
        };
        if let Err(e) = repo.insert(&mem).await {
            tracing::warn!("failed to write loop episodic memory: {e}");
        }
    });
}
```

If the wiring is complex (requires changing multiple builders), skip this step and add a `// TODO: Phase 2 — write episodic memory on loop hard-stop` comment instead.

- [ ] **Step 3: Build**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(agent): write episodic memory on loop hard-stop"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all agent tests:** `cargo nextest run -p agent`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Verify existing tests still pass:** `cargo nextest run --workspace` (full suite)
