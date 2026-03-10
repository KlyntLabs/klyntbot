# Phase 3: Progressive Context Loading

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace one-shot context assembly with two-phase loading: lightweight initial assembly + on-demand expansion via a new `context_request` tool. The agent can request more context mid-execution.

**Architecture:** Extends `AssembledContext` with a `ContextInventory` that tracks loaded vs. deferred sources. A new `context_request` tool lets the agent expand context mid-execution. The `ContextEngine` gains an `expand()` method. The `ReactiveEngine` loop checks for context version changes between iterations.

**Tech Stack:** tokio (RwLock), async-trait, serde

**Depends on:** Phase 1 (annotations context source), Phase 2 (tool metadata for context_request tool registration)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/context_engine/src/inventory.rs` | Create | ContextInventory, ContextInventoryItem, ContextItemStatus types |
| `crates/context_engine/src/assembler.rs` | Modify | Add inventory to AssembledContext, add expand() method |
| `crates/context_engine/src/budget.rs` | Modify | Add remaining budget tracking |
| `crates/context_engine/src/source.rs` | Modify | Add `estimated_tokens()` to ContextSource trait |
| `crates/context_engine/src/lib.rs` | Modify | Add inventory module |
| `crates/tools/src/context_request.rs` | Create | context_request tool |
| `crates/tools/src/lib.rs` | Modify | Add context_request module |
| `crates/agent/src/context_sources/*.rs` | Modify | Add `estimated_tokens()` to each source |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Modify | Check context version per iteration |

---

## Chunk 1: Context Inventory + Engine Extension

### Task 1: ContextInventory Types

**Files:**
- Create: `crates/context_engine/src/inventory.rs`

- [ ] **Step 1: Write inventory types**

```rust
// crates/context_engine/src/inventory.rs

//! Tracks what context is loaded vs. available but not yet loaded.

use serde::{Deserialize, Serialize};

/// Status of a context source in the inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContextItemStatus {
    /// Source content has been loaded into the prompt.
    Loaded { tokens_used: usize },
    /// Source exists but was deferred due to budget constraints.
    Deferred { reason: String },
    /// Source is available but wasn't queried in this assembly.
    Available { description: String },
}

/// A single entry in the context inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextInventoryItem {
    pub source_name: String,
    pub priority: u8,
    pub status: ContextItemStatus,
    pub token_estimate: usize,
    pub summary: Option<String>,
}

/// Tracks all context sources and their load status.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextInventory {
    pub items: Vec<ContextInventoryItem>,
}

impl ContextInventory {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add or update an inventory item.
    pub fn upsert(&mut self, item: ContextInventoryItem) {
        if let Some(existing) = self.items.iter_mut().find(|i| i.source_name == item.source_name) {
            *existing = item;
        } else {
            self.items.push(item);
        }
    }

    /// Get total tokens used by loaded sources.
    pub fn tokens_loaded(&self) -> usize {
        self.items.iter().map(|i| match &i.status {
            ContextItemStatus::Loaded { tokens_used } => *tokens_used,
            _ => 0,
        }).sum()
    }

    /// Get names of deferred sources.
    pub fn deferred_sources(&self) -> Vec<&str> {
        self.items.iter().filter_map(|i| match &i.status {
            ContextItemStatus::Deferred { .. } => Some(i.source_name.as_str()),
            _ => None,
        }).collect()
    }

    /// Mark a source as loaded.
    pub fn mark_loaded(&mut self, source_name: &str, tokens_used: usize) {
        if let Some(item) = self.items.iter_mut().find(|i| i.source_name == source_name) {
            item.status = ContextItemStatus::Loaded { tokens_used };
        }
    }

    /// Format as a human-readable summary for the system prompt.
    pub fn format_for_prompt(&self, budget_total: usize, budget_remaining: usize) -> String {
        let mut lines = vec![format!(
            "[Available Context — request via context_request tool if needed]"
        )];

        for item in &self.items {
            let status_icon = match &item.status {
                ContextItemStatus::Loaded { tokens_used } => {
                    format!("loaded — {:.1}k tokens", *tokens_used as f64 / 1000.0)
                }
                ContextItemStatus::Deferred { reason } => {
                    format!("deferred — {} ({:.1}k est.)", reason, item.token_estimate as f64 / 1000.0)
                }
                ContextItemStatus::Available { description } => {
                    format!("available — {}", description)
                }
            };
            lines.push(format!("- {} ({})", item.source_name, status_icon));
        }

        lines.push(format!(
            "Budget: {:.1}k / {:.1}k tokens remaining",
            budget_remaining as f64 / 1000.0,
            budget_total as f64 / 1000.0,
        ));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_upsert_and_tokens() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "memories".into(),
            priority: 70,
            status: ContextItemStatus::Loaded { tokens_used: 1800 },
            token_estimate: 2000,
            summary: Some("Episodic memories".into()),
        });
        inv.upsert(ContextInventoryItem {
            source_name: "project".into(),
            priority: 50,
            status: ContextItemStatus::Deferred { reason: "budget insufficient".into() },
            token_estimate: 2100,
            summary: Some("Project details".into()),
        });

        assert_eq!(inv.tokens_loaded(), 1800);
        assert_eq!(inv.deferred_sources(), vec!["project"]);
    }

    #[test]
    fn test_mark_loaded() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "project".into(),
            priority: 50,
            status: ContextItemStatus::Deferred { reason: "budget".into() },
            token_estimate: 2100,
            summary: None,
        });

        inv.mark_loaded("project", 1900);
        assert_eq!(inv.tokens_loaded(), 1900);
        assert!(inv.deferred_sources().is_empty());
    }

    #[test]
    fn test_format_for_prompt() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "memories".into(),
            priority: 70,
            status: ContextItemStatus::Loaded { tokens_used: 1800 },
            token_estimate: 2000,
            summary: None,
        });

        let prompt = inv.format_for_prompt(12000, 8200);
        assert!(prompt.contains("memories"));
        assert!(prompt.contains("loaded"));
        assert!(prompt.contains("Budget"));
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod inventory;` to `crates/context_engine/src/lib.rs` and re-export types.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -E 'test(inventory)'`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/inventory.rs crates/context_engine/src/lib.rs
git commit -m "feat(context_engine): add ContextInventory for tracking loaded vs deferred sources"
```

---

### Task 2: Extend ContextSource Trait with Token Estimation

**Files:**
- Modify: `crates/context_engine/src/source.rs`

- [ ] **Step 1: Add `estimated_tokens()` with default impl**

```rust
#[async_trait]
pub trait ContextSource: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u8;
    async fn provide(&self, ctx: &SourceContext) -> Option<String>;

    /// Estimated token count for this source's output.
    /// Used for budget planning before actually loading.
    /// Default: 500 tokens (conservative estimate).
    fn estimated_tokens(&self) -> usize {
        500
    }
}
```

- [ ] **Step 2: Run context_engine tests**

Run: `cargo nextest run -p context_engine`
Expected: All PASS (default impl is backward compatible)

- [ ] **Step 3: Commit**

```bash
git add crates/context_engine/src/source.rs
git commit -m "feat(context_engine): add estimated_tokens() to ContextSource trait"
```

---

### Task 3: Extend AssembledContext with Inventory + Version

**Files:**
- Modify: `crates/context_engine/src/assembler.rs`

- [ ] **Step 1: Add inventory fields to AssembledContext**

```rust
use crate::inventory::ContextInventory;

#[derive(Clone)]
pub struct AssembledContext {
    pub messages: Vec<Message>,
    pub token_count: usize,
    pub budget_report: BudgetReport,
    pub inventory: ContextInventory,      // NEW
    pub budget_remaining: usize,          // NEW
    pub version: u32,                     // NEW — incremented on expand
}
```

- [ ] **Step 2: Update `assemble_uncached()` to build inventory**

In the assembly pipeline, after collecting sources, build inventory items for each source:

```rust
// After building messages, create inventory
let mut inventory = ContextInventory::new();
for source in &self.sources {
    let status = if /* source was included in messages */ {
        ContextItemStatus::Loaded { tokens_used: /* actual tokens */ }
    } else {
        ContextItemStatus::Deferred { reason: "budget insufficient".into() }
    };
    inventory.upsert(ContextInventoryItem {
        source_name: source.name().to_string(),
        priority: source.priority(),
        status,
        token_estimate: source.estimated_tokens(),
        summary: None,
    });
}
```

- [ ] **Step 3: Fix all test compilation errors**

Update test assertions that construct `AssembledContext` to include the new fields.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p context_engine`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "feat(context_engine): add inventory and version tracking to AssembledContext"
```

---

### Task 4: Context Expansion Method

**Files:**
- Modify: `crates/context_engine/src/assembler.rs`

- [ ] **Step 1: Write test for expand()**

```rust
#[tokio::test]
async fn test_expand_loads_deferred_source() {
    use crate::inventory::*;
    use crate::source::ContextSource;

    struct DeferredSource;

    #[async_trait]
    impl ContextSource for DeferredSource {
        fn name(&self) -> &str { "deferred_test" }
        fn priority(&self) -> u8 { 40 }
        async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
            Some("[Deferred Content]\nProject details here.".into())
        }
        fn estimated_tokens(&self) -> usize { 200 }
    }

    let engine = ContextEngine::new()
        .with_sources(vec![Box::new(DeferredSource)]);

    // Create initial context with deferred source
    let mut initial = AssembledContext {
        messages: vec![Message::system("System prompt.")],
        token_count: 100,
        budget_report: BudgetReport::default(), // simplified for test
        inventory: ContextInventory::new(),
        budget_remaining: 5000,
        version: 1,
    };
    initial.inventory.upsert(ContextInventoryItem {
        source_name: "deferred_test".into(),
        priority: 40,
        status: ContextItemStatus::Deferred { reason: "budget".into() },
        token_estimate: 200,
        summary: None,
    });

    let expanded = engine.expand(&initial, "deferred_test", "cli", "test").await.unwrap();
    assert!(expanded.version > initial.version);
    assert!(expanded.messages.len() > initial.messages.len());
    assert!(expanded.inventory.deferred_sources().is_empty());
}
```

- [ ] **Step 2: Implement `expand()`**

```rust
impl ContextEngine {
    /// Expand context by loading a deferred source.
    pub async fn expand(
        &self,
        current: &AssembledContext,
        source_name: &str,
        channel: &str,
        chat_id: &str,
    ) -> common::Result<AssembledContext> {
        // Find the source by name
        let source = self.sources.iter()
            .find(|s| s.name() == source_name)
            .ok_or_else(|| common::KlyntbotError::Generic(
                format!("Context source '{}' not found", source_name)
            ))?;

        let ctx = SourceContext {
            channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            message: None,
            intent_summary: None,
            project_id: None,
        };

        let content = source.provide(&ctx).await;

        let mut result = current.clone();
        result.version += 1;

        if let Some(text) = content {
            let tokens = self.estimate_text(&text);
            if tokens <= result.budget_remaining {
                // Insert as system message at the right priority position
                result.messages.push(Message::system(&text));
                result.token_count += tokens;
                result.budget_remaining -= tokens;
                result.inventory.mark_loaded(source_name, tokens);
            } else {
                return Err(common::KlyntbotError::Generic(
                    format!("Insufficient budget for '{}': needs {} tokens, {} remaining",
                        source_name, tokens, result.budget_remaining)
                ));
            }
        }

        Ok(result)
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -E 'test(expand)'`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "feat(context_engine): add expand() for mid-execution context loading"
```

---

## Chunk 2: context_request Tool + Engine Integration

### Task 5: context_request Tool

**Files:**
- Create: `crates/tools/src/context_request.rs`
- Modify: `crates/tools/src/lib.rs`

- [ ] **Step 1: Write the context_request tool**

This is a **user contribution point** — the tool design determines how agents interact with progressive context.

```rust
// crates/tools/src/context_request.rs

//! Tool for requesting additional context mid-execution.

use tools_core::{Tool, ToolParams, ToolExecute, RoutingContext};
use tools_core_macros::{Tool, ToolParams};

/// Request additional context mid-execution.
#[derive(Tool)]
#[tool(
    name = "context_request",
    description = "Request additional context mid-execution. Use when you need more information from a specific context source (e.g., project details, additional memories, user history) to complete the current task.",
    category = "System",
    tags = "context,memory,expand,load",
    cost = "Free",
)]
pub struct ContextRequestTool {
    // Will need Arc<RwLock<AssembledContext>> + Arc<ContextEngine> injected
    // Exact wiring depends on how the agent runtime passes these
}

#[derive(ToolParams)]
pub struct ContextRequestParams {
    /// Name of the context source to load (from the inventory list).
    #[param(required)]
    pub source: String,

    /// Optional query to filter the context (e.g., for memory retrieval).
    pub query: Option<String>,
}
```

The exact implementation depends on how `Arc<RwLock<AssembledContext>>` is passed to tools. This needs to be wired through the `RoutingContext` or via a separate injection mechanism.

- [ ] **Step 2: Register in tools/lib.rs**

Add `pub mod context_request;` to `crates/tools/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p tools`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/context_request.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add context_request tool for mid-execution context expansion"
```

---

### Task 6: Add `estimated_tokens()` to Existing Context Sources

**Files:**
- Modify: `crates/agent/src/context_sources/*.rs`

- [ ] **Step 1: Add estimates to each context source**

For each context source in `crates/agent/src/context_sources/`, add an `estimated_tokens()` override with a reasonable estimate based on typical output:

| Source | Estimated Tokens |
|--------|-----------------|
| identity | 300 |
| persona | 200 |
| productivity | 800 |
| todo | 600 |
| project | 1500 |
| area | 400 |
| confidence | 100 |
| agent | 500 |
| bootstrap | 200 |
| page_context | 300 |
| annotation | 400 |
| cognitive (from Phase 1) | 1000 |

- [ ] **Step 2: Run agent tests**

Run: `cargo nextest run -p agent`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/context_sources/
git commit -m "feat(agent): add estimated_tokens() to all context sources"
```

---

### Task 7: Inject Inventory into System Prompt

**Files:**
- Modify: `crates/context_engine/src/assembler.rs`

- [ ] **Step 1: Add inventory summary to assembled context**

At the end of `assemble_uncached()`, append the inventory summary as a system message:

```rust
// After building all messages and inventory:
let inventory_text = inventory.format_for_prompt(
    allocator.total_budget(),
    allocator.remaining(),
);
messages.push(Message::system(&inventory_text));
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p context_engine`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "feat(context_engine): inject context inventory summary into system prompt"
```

---

### Task 8: Final Integration + Verification

- [ ] **Step 1: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Format**

Run: `cargo fmt --all --check`
Expected: Clean

- [ ] **Step 4: Commit fixes**

```bash
git commit -m "fix: address clippy and formatting from Phase 3"
```
