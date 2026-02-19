# Thinking Phase UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the dumb "Thinking..." spinner with a real-time pipeline stage + tool call display that collapses to a summary after completion.

**Architecture:** Thread an `event_tx: Option<Sender<AgentEvent>>` from the CLI through `AgentPipeline → EngineDispatch → ReactPlusEngine → ExecutionCore`. Each layer emits events as they happen. A new `ThinkingRenderer` component in `common` renders them in-place. After `Done`, it collapses the trace to a one-line summary.

**Tech Stack:** Rust, tokio mpsc channels, crossterm (already a dependency), serde_json

---

### Task 1: Add new event variants to AgentEvent

**Files:**
- Modify: `crates/agent/src/events.rs`

**Step 1: Add three new variants to `AgentEvent`**

In `crates/agent/src/events.rs`, add these variants after `IterationStart`:

```rust
/// Pipeline classification step completed.
ClassificationComplete {
    strategy: String,
    confidence: f32,
    source: String,
    duration_ms: u64,
},

/// Context assembly step completed.
ContextAssembled {
    total_tokens: usize,
    budget: usize,
    duration_ms: u64,
},

/// Execution engine selected and starting.
ExecutionStarted {
    engine: String,
    max_iterations: usize,
},
```

**Step 2: Verify it compiles**

Run: `cargo build -p agent 2>&1 | head -30`
Expected: Warnings about non-exhaustive match patterns (existing code doesn't handle new variants yet) but no errors. Fix any match exhaustiveness issues by adding wildcard arms where needed.

**Step 3: Update match arms in `cli/src/chat.rs`**

In the event loop at `chat.rs:314`, add arms for the three new variants. For now, just ignore them:

```rust
AgentEvent::ClassificationComplete { .. } => {}
AgentEvent::ContextAssembled { .. } => {}
AgentEvent::ExecutionStarted { .. } => {}
```

**Step 4: Verify clean compile**

Run: `cargo build --workspace 2>&1 | head -30`
Expected: Clean build (0 errors). Clippy warnings about unused fields are OK for now.

**Step 5: Commit**

```bash
git add crates/agent/src/events.rs crates/cli/src/chat.rs
git commit -m "feat(agent): add pipeline stage event variants to AgentEvent"
```

---

### Task 2: Add `--verbose` flag to CLI chat command

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/chat.rs`

**Step 1: Add verbose flag to Chat variant**

In `crates/cli/src/commands.rs`, add to the `Chat` variant:

```rust
/// Show detailed thinking trace (tool args, token counts, timing)
#[arg(short = 'V', long)]
verbose: bool,
```

**Step 2: Thread verbose through `handle_chat`**

In `crates/cli/src/chat.rs`:
- Change `handle_chat` signature to `pub async fn handle_chat(message: Option<String>, session: String, verbose: bool) -> Result<()>`
- Pass `verbose` to `run_with_streaming` calls
- Change `run_with_streaming` signature to accept `verbose: bool` parameter

For now, just accept the parameter — don't use it yet. Store it in a `let _verbose = verbose;` binding.

**Step 3: Update the caller in main.rs**

Find where `handle_chat` is called (likely `src/main.rs` or a match on `Commands::Chat`). Pass the new `verbose` field.

Run: `cargo build -p cli 2>&1 | head -20`
Expected: Compiles cleanly. The `_verbose` binding silences unused warnings.

**Step 4: Commit**

```bash
git add crates/cli/src/commands.rs crates/cli/src/chat.rs src/main.rs
git commit -m "feat(cli): add --verbose flag to chat command"
```

---

### Task 3: Create ThinkingRenderer component

**Files:**
- Create: `crates/common/src/utils/terminal/thinking_renderer.rs`
- Modify: `crates/common/src/utils/terminal/mod.rs`

**Step 1: Write tests first**

Create `crates/common/src/utils/terminal/thinking_renderer.rs` with these tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_renderer() -> ThinkingRenderer {
        ThinkingRenderer::new(false, false) // not verbose, not TTY (for test)
    }

    fn verbose_renderer() -> ThinkingRenderer {
        ThinkingRenderer::new(true, false) // verbose, not TTY
    }

    #[test]
    fn test_new_renderer_has_zero_lines() {
        let r = test_renderer();
        assert_eq!(r.rendered_lines, 0);
    }

    #[test]
    fn test_classification_complete() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 312);
        assert_eq!(r.rendered_lines, 1);
        assert_eq!(r.tool_count, 0);
        assert_eq!(r.iteration_count, 0);
    }

    #[test]
    fn test_context_assembled() {
        let mut r = test_renderer();
        r.on_context_assembled(2400, 8192, 48);
        assert_eq!(r.rendered_lines, 1);
    }

    #[test]
    fn test_execution_started() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        assert_eq!(r.rendered_lines, 1);
    }

    #[test]
    fn test_tool_start_increments_count() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        assert_eq!(r.tool_count, 1);
    }

    #[test]
    fn test_tool_end_updates_state() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 832);
        assert_eq!(r.tool_count, 1);
    }

    #[test]
    fn test_iteration_start() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_iteration_start(2, 5);
        assert_eq!(r.iteration_count, 2);
    }

    #[test]
    fn test_summary_line_format() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 100);
        r.on_context_assembled(2400, 8192, 50);
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 100);
        r.on_tool_start("todo_search");
        r.on_tool_end("todo_search", true, 200);
        r.on_iteration_start(2, 5);

        let summary = r.summary_line("o4-mini", 5.1);
        assert!(summary.contains("o4-mini"));
        assert!(summary.contains("5.1s"));
        assert!(summary.contains("2 tools"));
        assert!(summary.contains("2 iters"));
    }

    #[test]
    fn test_summary_with_single_tool() {
        let mut r = test_renderer();
        r.on_execution_started("Direct", 1);
        r.on_tool_start("echo");
        r.on_tool_end("echo", true, 50);

        let summary = r.summary_line("gpt-4o", 0.3);
        assert!(summary.contains("1 tool"));
        // Should not say "1 tools"
        assert!(!summary.contains("1 tools"));
    }

    #[test]
    fn test_summary_no_tools() {
        let r = test_renderer();
        let summary = r.summary_line("o4-mini", 1.2);
        assert!(summary.contains("o4-mini"));
        assert!(summary.contains("1.2s"));
        // No tools or iters mentioned
        assert!(!summary.contains("tool"));
        assert!(!summary.contains("iter"));
    }

    #[test]
    fn test_verbose_mode_flag() {
        let r = verbose_renderer();
        assert!(r.verbose);
    }
}
```

**Step 2: Write the ThinkingRenderer implementation**

Above the tests in the same file, implement:

```rust
//! Thinking phase renderer — shows pipeline stages and tool calls during AI processing.
//!
//! Normal mode: stage checkmarks + tool names with durations.
//! Verbose mode: adds confidence scores, token budgets, tool args.
//! Collapses to a one-line summary after completion.

use std::io::{self, Write};

use crossterm::{cursor, terminal::{self, ClearType}, ExecutableCommand};

use super::colors::{colorize, colors_enabled, DIM, SEPARATOR, SUCCESS, TOOL};
use super::status_success;

/// Renders pipeline thinking trace during agent execution.
pub struct ThinkingRenderer {
    pub verbose: bool,
    is_tty: bool,
    pub rendered_lines: u16,
    pub tool_count: usize,
    pub iteration_count: usize,
    max_iterations: usize,
}

impl ThinkingRenderer {
    pub fn new(verbose: bool, is_tty: bool) -> Self {
        Self {
            verbose,
            is_tty,
            rendered_lines: 0,
            tool_count: 0,
            iteration_count: 0,
            max_iterations: 0,
        }
    }

    /// Handle classification stage completing.
    pub fn on_classification_complete(
        &mut self,
        strategy: &str,
        confidence: f32,
        source: &str,
        duration_ms: u64,
    ) {
        if self.verbose {
            let line = format!(
                "  {} Classified: {} ({:.2})  {}",
                status_success(),
                colorize(strategy, TOOL),
                confidence,
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
            let detail = format!("    {}", colorize(&format!("method: {}", source), DIM));
            println!("{}", detail);
            self.rendered_lines += 1;
        } else {
            let line = format!(
                "  {} Classified → {}  {}",
                status_success(),
                colorize(strategy, TOOL),
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
        }
        let _ = io::stdout().flush();
    }

    /// Handle context assembly completing.
    pub fn on_context_assembled(&mut self, total_tokens: usize, budget: usize, duration_ms: u64) {
        if self.verbose {
            let line = format!(
                "  {} Context: {}/{} tokens  {}",
                status_success(),
                colorize(&total_tokens.to_string(), TOOL),
                budget,
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
        } else {
            let line = format!(
                "  {} Context assembled  {}",
                status_success(),
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
        }
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle execution engine starting.
    pub fn on_execution_started(&mut self, engine: &str, max_iterations: usize) {
        self.max_iterations = max_iterations;
        self.iteration_count = 1;
        let line = format!(
            "  {} Executing ({})",
            colorize("▸", TOOL),
            colorize(engine, DIM),
        );
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a new iteration starting.
    pub fn on_iteration_start(&mut self, iteration: usize, max: usize) {
        self.iteration_count = iteration;
        self.max_iterations = max;
        // Update the execution line in-place if TTY
        if self.is_tty && self.rendered_lines > 0 {
            let mut stdout = io::stdout();
            // Find how many lines back the "Executing" line is
            // It's rendered_lines minus the line where execution started,
            // but we'll just update a new line for simplicity
            let line = format!(
                "  {} Executing (iteration {}/{})",
                colorize("▸", TOOL),
                iteration,
                max,
            );
            // Print on new line — simpler and more reliable than cursor tricks for iteration updates
            println!("{}", line);
            self.rendered_lines += 1;
            let _ = stdout.flush();
        } else {
            let line = format!(
                "  {} Executing (iteration {}/{})",
                colorize("▸", TOOL),
                iteration,
                max,
            );
            println!("{}", line);
            self.rendered_lines += 1;
        }
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution starting.
    pub fn on_tool_start(&mut self, name: &str) {
        self.tool_count += 1;
        let line = format!(
            "    {} {}",
            colorize("⟳", TOOL),
            colorize(name, TOOL),
        );
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution completing.
    pub fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64) {
        if self.is_tty {
            // Move up to the tool's start line and overwrite
            let mut stdout = io::stdout();
            let _ = stdout.execute(cursor::MoveUp(1));
            let _ = stdout.execute(terminal::Clear(ClearType::CurrentLine));

            let indicator = if success {
                status_success()
            } else {
                colorize("✗", "\x1b[31m") // red
            };
            println!(
                "    {} {} {}",
                indicator,
                colorize(name, TOOL),
                colorize(&format_duration(duration_ms), DIM),
            );
            let _ = stdout.flush();
            // No change to rendered_lines — we overwrote in place
        } else {
            let indicator = if success { "✓" } else { "✗" };
            println!(
                "    {} {} {}",
                indicator,
                name,
                format_duration(duration_ms),
            );
            self.rendered_lines += 1;
        }
    }

    /// Collapse the thinking trace to a one-line summary.
    ///
    /// Uses crossterm to erase all thinking lines and replace with a summary.
    /// In non-TTY mode, just prints the summary on a new line.
    pub fn collapse(&self, model: &str, elapsed_secs: f64) {
        let summary = self.summary_line(model, elapsed_secs);

        if self.is_tty && self.rendered_lines > 0 {
            let mut stdout = io::stdout();
            // Move cursor up past all thinking lines
            let _ = stdout.execute(cursor::MoveUp(self.rendered_lines));
            let _ = stdout.execute(cursor::MoveToColumn(0));
            // Clear everything from cursor down
            let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
            // Print the summary
            println!("{}", summary);
            let _ = stdout.flush();
        } else {
            println!("{}", summary);
        }
    }

    /// Generate the summary line string.
    pub fn summary_line(&self, model: &str, elapsed_secs: f64) -> String {
        let width = terminal::size().map(|(w, _)| w as usize).unwrap_or(60);
        let sep_char = "\u{2500}"; // ─

        let mut parts = vec![format!("{} · {:.1}s", model, elapsed_secs)];

        if self.tool_count > 0 {
            let tool_word = if self.tool_count == 1 { "tool" } else { "tools" };
            parts.push(format!("{} {}", self.tool_count, tool_word));
        }

        if self.iteration_count > 1 {
            let iter_word = if self.iteration_count == 1 { "iter" } else { "iters" };
            parts.push(format!("{} {}", self.iteration_count, iter_word));
        }

        let label = parts.join(", ");
        let text_with_spaces = format!(" {} ", label);
        let remaining = width.saturating_sub(text_with_spaces.len() + 6);
        let left = sep_char.repeat(3);
        let right = sep_char.repeat(remaining);

        if colors_enabled() {
            colorize(&format!("{}{}{}", left, text_with_spaces, right), SEPARATOR)
        } else {
            format!("{}{}{}", left, text_with_spaces, right)
        }
    }
}

/// Format milliseconds into a human-readable duration string.
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}
```

**Step 3: Register the module**

In `crates/common/src/utils/terminal/mod.rs`, add:

```rust
pub mod thinking_renderer;
```

And add to the re-exports:

```rust
pub use thinking_renderer::*;
```

**Step 4: Run tests**

Run: `cargo nextest run -p common -E 'test(thinking_renderer)' --nocapture`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/common/src/utils/terminal/thinking_renderer.rs crates/common/src/utils/terminal/mod.rs
git commit -m "feat(common): add ThinkingRenderer component for pipeline trace display"
```

---

### Task 4: Thread event_tx through pipeline

**Files:**
- Modify: `crates/agent/src/pipeline.rs`

**Step 1: Write a test for event emission**

Add this test to the existing test module in `pipeline.rs`:

```rust
#[tokio::test]
async fn test_pipeline_emits_classification_event() {
    use crate::events::AgentEvent;
    use tokio::sync::mpsc;

    let provider = MockPipelineProvider::new(vec![text_response("Hi!")]);
    let pipeline = make_pipeline(provider);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    let _result = pipeline
        .process_message(
            "hello",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            Some(event_tx),
        )
        .await
        .unwrap();

    // Should have received ClassificationComplete event
    let mut found_classification = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event, AgentEvent::ClassificationComplete { .. }) {
            found_classification = true;
        }
    }
    assert!(found_classification, "Expected ClassificationComplete event");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_pipeline_emits_classification_event)' --nocapture`
Expected: Compile error — `process_message` doesn't accept `event_tx` yet.

**Step 3: Add event_tx parameter to process_message**

Change `process_message` signature to:

```rust
pub async fn process_message(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value],
    tool_names: &[&str],
    ctx: &RoutingContext,
    system_prompt: Option<&str>,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<PipelineResult> {
```

Add the import at the top of pipeline.rs:

```rust
use crate::events::AgentEvent;
use std::time::Instant;
```

**Step 4: Emit events at each pipeline stage**

After classification (around line 108):

```rust
let classification = self.orchestrator.classify(message, tool_names).await;
```

Add timing and event emission:

```rust
// Step 1: Classify (with timing)
let classify_start = Instant::now();
let classification = self.orchestrator.classify(message, tool_names).await;
let classify_ms = classify_start.elapsed().as_millis() as u64;

if let Some(ref tx) = event_tx {
    let _ = tx.send(AgentEvent::ClassificationComplete {
        strategy: format!("{:?}", classification.strategy),
        confidence: classification.confidence,
        source: format!("{:?}", classification.source),
        duration_ms: classify_ms,
    }).await;
}
```

After context assembly (around line 127):

```rust
let assemble_start = Instant::now();
let assembled = self.context_engine.assemble(context_request).await;
let assemble_ms = assemble_start.elapsed().as_millis() as u64;

if let Some(ref tx) = event_tx {
    let _ = tx.send(AgentEvent::ContextAssembled {
        total_tokens: assembled.token_count,
        budget: self.config.context_window,
        duration_ms: assemble_ms,
    }).await;
}
```

Before execution (around line 136):

```rust
if let Some(ref tx) = event_tx {
    let engine_name = format!("{:?}", classification.strategy);
    let max_iter = match &classification.strategy {
        context_engine::ExecutionStrategy::ToolAssisted { max_iterations } => *max_iterations as usize,
        context_engine::ExecutionStrategy::AutonomousTask { max_iterations } => *max_iterations as usize,
        _ => 1,
    };
    let _ = tx.send(AgentEvent::ExecutionStarted {
        engine: engine_name,
        max_iterations: max_iter,
    }).await;
}
```

**Step 5: Fix existing tests**

All existing `process_message` calls need the new `None` parameter appended:

```rust
// Change all existing test calls from:
.process_message("hello", vec![], &[], &[], &routing_ctx(), None)
// To:
.process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
```

**Step 6: Run all pipeline tests**

Run: `cargo nextest run -p agent -E 'test(pipeline)' --nocapture`
Expected: All tests pass, including the new event emission test.

**Step 7: Fix the caller in agent_loop.rs**

In `agent_loop.rs` `run_pipeline()` method (around line 952), update the call to pass `None`:

```rust
let result = self
    .pipeline
    .process_message(
        content,
        history_messages,
        &tool_defs,
        &tool_name_refs,
        routing_ctx,
        Some(&system_prompt),
        None,  // event_tx — wired in Task 6
    )
    .await?;
```

**Step 8: Verify workspace builds**

Run: `cargo build --workspace 2>&1 | head -20`
Expected: Clean build.

**Step 9: Commit**

```bash
git add crates/agent/src/pipeline.rs crates/agent/src/agent_loop.rs
git commit -m "feat(agent): thread event_tx through pipeline for real-time stage events"
```

---

### Task 5: Thread event_tx through execution engines

**Files:**
- Modify: `crates/agent/src/execution/dispatch.rs`
- Modify: `crates/agent/src/execution/react_plus.rs`
- Modify: `crates/agent/src/execution/core.rs`

**Step 1: Add event_tx to EngineDispatch::execute**

In `dispatch.rs`, change the `execute` method signature:

```rust
pub async fn execute(
    &self,
    strategy: ExecutionStrategy,
    messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<DispatchResult> {
```

Pass `event_tx.clone()` to engine execute calls. For ReactPlusEngine calls, pass it through.

**Step 2: Add event_tx to ReactPlusEngine::execute**

In `react_plus.rs`, change the `execute` method signature:

```rust
pub async fn execute(
    &self,
    mut messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<ReactOutcome> {
```

Emit `IterationStart` at the top of the iteration loop:

```rust
for iteration in 1..=self.max_iterations {
    if let Some(ref tx) = event_tx {
        let _ = tx.send(AgentEvent::IterationStart {
            iteration: iteration as usize,
            max: self.max_iterations as usize,
        }).await;
    }
    // ... existing code ...
```

**Step 3: Add event_tx to ExecutionCore::run_cycle**

In `core.rs`, change the `run_cycle` method signature:

```rust
pub async fn run_cycle(
    &self,
    messages: &mut Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    routing_ctx: &RoutingContext,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<(CycleOutcome, Usage)> {
```

Before executing each tool (inside the futures map), emit `ToolStart`. After each tool completes, emit `ToolEnd`:

In the tool execution section (after `let results = join_all(futures).await;`), emit events:

```rust
// Emit events for each tool result
if let Some(tx) = event_tx {
    for r in &results {
        // We can't emit ToolStart before parallel execution easily,
        // so emit both start and end after completion for now
        let _ = tx.send(AgentEvent::ToolStart {
            name: r.tool_name.clone(),
            args: serde_json::Value::Null,  // args not easily available here
        }).await;
        let _ = tx.send(AgentEvent::ToolEnd {
            name: r.tool_name.clone(),
            success: r.success,
            duration_ms: r.duration_ms,
        }).await;
    }
}
```

Note: For truly real-time tool events, we'd need to restructure the parallel execution. For now, emit events after the batch completes — tools still show individually with their actual durations.

**Step 4: Update all callers**

Update all `run_cycle` calls in `react_plus.rs` to pass `event_tx.as_ref()`:

```rust
let (outcome, cycle_usage) = self
    .core
    .run_cycle(&mut messages, tools, params, ctx, event_tx.as_ref())
    .await?;
```

Update all `engine.execute()` calls in `dispatch.rs` to pass `event_tx.clone()`:

```rust
let outcome = engine
    .execute(current_messages.clone(), params, ctx)
    .await?;
// becomes:
let outcome = engine
    .execute(current_messages.clone(), tools, params, ctx, event_tx.clone())
    .await?;
```

Note: `DirectEngine::execute` also needs the event_tx parameter added, or the Direct engine path in dispatch.rs can just not pass events (since direct responses don't have tool calls). Check the `DirectEngine` signature and add the parameter if needed, or pass `None`.

**Step 5: Update pipeline.rs to pass event_tx to dispatch**

In `pipeline.rs`, pass event_tx through to `engine_dispatch.execute()`:

```rust
let dispatch_result = self
    .engine_dispatch
    .execute(
        classification.strategy.clone(),
        assembled.messages,
        tool_definitions,
        &params,
        ctx,
        event_tx,  // pass through
    )
    .await?;
```

**Step 6: Fix all tests**

All test calls to `execute`, `run_cycle` need the new `None` parameter. Search for all call sites in the test modules and add `None` to each.

**Step 7: Run all execution tests**

Run: `cargo nextest run -p agent --nocapture 2>&1 | tail -30`
Expected: All tests pass.

**Step 8: Commit**

```bash
git add crates/agent/src/execution/dispatch.rs crates/agent/src/execution/react_plus.rs crates/agent/src/execution/core.rs crates/agent/src/pipeline.rs
git commit -m "feat(agent): thread event_tx through execution engines for real-time tool events"
```

---

### Task 6: Wire event_tx from CLI into the pipeline

**Files:**
- Modify: `crates/agent/src/agent_loop.rs`

**Step 1: Add event_tx parameter to run_pipeline**

Change `run_pipeline` signature:

```rust
async fn run_pipeline(
    &self,
    content: &str,
    history: Vec<session::SessionMessage>,
    routing_ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<String> {
```

Pass `event_tx` through to `self.pipeline.process_message(...)`.

**Step 2: Update process_direct to pass None**

In `process_direct` (non-streaming path), pass `None`:

```rust
let response_content = self.run_pipeline(&content, history, &routing_ctx, None).await?;
```

**Step 3: Update process_direct_streaming to pass event_tx**

In `process_direct_streaming`, the spawned task already has `event_tx`. Clone it and pass to `run_pipeline`:

```rust
let handle = tokio::spawn(async move {
    let pipeline_event_tx = event_tx.clone();
    let result = match agent.run_pipeline(&content, history, &routing_ctx, Some(pipeline_event_tx)).await {
```

**Step 4: Remove the old batch ContentChunk emission**

In the spawned task, the current code sends `ContentChunk(response.clone())` followed by `Done(response.clone())`. Since events now stream from the pipeline, we should still send `Done` but can remove the `ContentChunk` send (the content comes from the pipeline result now):

```rust
Ok(response) => {
    let _ = event_tx.send(AgentEvent::ContentChunk(response.clone())).await;
    let _ = event_tx.send(AgentEvent::Done(response.clone())).await;
    Ok(response)
}
```

Keep both for now — `ContentChunk` is how the final response text reaches the `StreamRenderer`. We'll keep this working.

**Step 5: Verify builds and existing tests pass**

Run: `cargo build --workspace && cargo nextest run -p agent --nocapture 2>&1 | tail -20`
Expected: Clean build and all tests pass.

**Step 6: Commit**

```bash
git add crates/agent/src/agent_loop.rs
git commit -m "feat(agent): wire event_tx from streaming path into pipeline"
```

---

### Task 7: Integrate ThinkingRenderer into CLI chat

**Files:**
- Modify: `crates/cli/src/chat.rs`

**Step 1: Replace Spinner with ThinkingRenderer in run_with_streaming**

In `run_with_streaming`, replace the spinner creation:

```rust
// OLD:
let mut spinner = if io::stdout().is_terminal() {
    let mut s = Spinner::new(colorize("Thinking...", DIM));
    s.start();
    Some(s)
} else {
    None
};

// NEW:
let is_tty = io::stdout().is_terminal();
let mut thinking = ThinkingRenderer::new(verbose, is_tty);
let mut thinking_active = true;
```

**Step 2: Route new events to ThinkingRenderer**

In the event loop, replace spinner stop logic with ThinkingRenderer calls:

```rust
AgentEvent::ClassificationComplete { strategy, confidence, source, duration_ms } => {
    thinking.on_classification_complete(&strategy, confidence, &source, duration_ms);
}
AgentEvent::ContextAssembled { total_tokens, budget, duration_ms } => {
    thinking.on_context_assembled(total_tokens, budget, duration_ms);
}
AgentEvent::ExecutionStarted { engine, max_iterations } => {
    thinking.on_execution_started(&engine, max_iterations);
}
AgentEvent::ToolStart { name, args: _ } => {
    thinking_active = false; // first real work happening
    thinking.on_tool_start(&name);
}
AgentEvent::ToolEnd { name, success, duration_ms } => {
    thinking.on_tool_end(&name, success, duration_ms);
}
AgentEvent::IterationStart { iteration, max } => {
    thinking.on_iteration_start(iteration, max);
}
AgentEvent::ContentChunk(chunk) => {
    if thinking_active {
        // Collapse thinking trace before showing content
        thinking.collapse(model, thinking.summary_elapsed());
        thinking_active = false;
    }
    renderer.on_content_chunk(&chunk);
}
```

Wait — `ThinkingRenderer` doesn't track elapsed time internally. We need to track the start time. Add `start_time: Instant` field and `elapsed_secs()` method to `ThinkingRenderer`, or just use `renderer.elapsed_secs()` since `StreamRenderer` already tracks time.

**Step 3: Collapse on Done**

When `AgentEvent::Done` is received:

```rust
AgentEvent::Done(_) => {
    clean_exit = true;
    break;
}
```

After the event loop, before finalizing the renderer, collapse the thinking:

```rust
// Collapse thinking trace
if thinking.rendered_lines > 0 {
    thinking.collapse(model, renderer.elapsed_secs());
}
```

**Step 4: Handle interaction pauses**

In the interaction_rx branch, when pausing for `ask_user`:

```rust
// Thinking is already done by this point (tool calls have started)
// No spinner to stop, ThinkingRenderer has already shown its trace
renderer.pause();
```

**Step 5: Remove the old spinner import**

Remove `Spinner` from imports if no longer used elsewhere in the file.

**Step 6: Thread verbose into the call chain**

Update the `run_with_streaming` signature to accept `verbose: bool`, and pass it from `handle_chat`.

**Step 7: Run manual test**

Run: `cargo build --workspace && echo "Does it compile? Yes."`
Then manually test: `cargo run -- chat "hello"` and `cargo run -- chat --verbose "hello"`

Verify:
- Normal mode shows: classification, context, execution stages, tool calls
- `--verbose` shows extra detail (confidence, token counts)
- After response, thinking trace collapses to summary line

**Step 8: Commit**

```bash
git add crates/cli/src/chat.rs
git commit -m "feat(cli): integrate ThinkingRenderer into chat streaming loop"
```

---

### Task 8: Un-suppress IterationStart in StreamRenderer

**Files:**
- Modify: `crates/common/src/utils/stream_renderer.rs`

**Step 1: Update the suppressed method**

The `on_iteration_start` in `StreamRenderer` is now unused — iterations are handled by `ThinkingRenderer`. We can leave it suppressed since `StreamRenderer` handles the response phase only, not the thinking phase.

Actually, since `ThinkingRenderer` handles iterations now and `StreamRenderer` handles content rendering, we don't need to change `StreamRenderer` at all. The event routing in `chat.rs` directs iteration events to `ThinkingRenderer` instead.

**Step 2: Update stream_renderer tests**

The existing tests that assert iteration suppression are still valid — `StreamRenderer.on_iteration_start` still does nothing. No changes needed.

**Step 3: Verify**

Run: `cargo nextest run -p common --nocapture 2>&1 | tail -20`
Expected: All tests pass.

**Step 4: Skip commit** — no changes needed for this task.

---

### Task 9: Final integration test and cleanup

**Files:**
- Modify: Various (clippy fixes)

**Step 1: Run full workspace build and test**

Run: `cargo build --workspace && cargo nextest run --workspace --nocapture 2>&1 | tail -40`
Expected: All tests pass.

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | head -40`
Expected: 0 warnings. Fix any issues.

**Step 3: Run fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 4: Manual smoke test**

Test with a real message:
```bash
cargo run -- chat "create task: test the thinking renderer"
cargo run -- chat --verbose "what tasks do I have?"
```

Verify:
1. Normal mode: stages appear with checkmarks, tool calls show with durations, collapses to summary
2. Verbose mode: extra detail (confidence, token counts, method)
3. Non-TTY: `cargo run -- chat "hello" | cat` — plain text output, no cursor control artifacts

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat(cli): complete thinking phase UX with pipeline trace display

Replaces the simple 'Thinking...' spinner with a real-time display of
pipeline stages (classification, context assembly, execution) and
tool calls during AI processing. Collapses to a one-line summary
after completion.

- Add ClassificationComplete, ContextAssembled, ExecutionStarted events
- Thread event_tx through pipeline → dispatch → engine → core
- New ThinkingRenderer component with normal/verbose modes
- --verbose flag shows confidence scores, token budgets, tool args
- Graceful non-TTY degradation (plain text, no cursor control)"
```
