# Feature Package Architecture Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure klyntbot so Todo and Finance features live in self-contained crates with derive macros, making it trivial to add new feature packages.

**Architecture:** Extract a `tools-core` crate (traits, macros, registry), create `feature-todo` and `feature-finance` crates that own their tool impl, storage, config, and types. Keep `tools` for non-feature utility tools. Add performance optimizations (pagination, parallel queries).

**Tech Stack:** Rust, proc-macros (syn/quote/proc-macro2), sqlx, pgvector, serde, async-trait, tokio

---

## Phase 1: Create `tools-core` Crate (Foundation)

The `tools-core` crate is the foundation everything else depends on. It holds the `Tool` trait, `FeaturePackage` trait, `ToolRegistry`, `ParamExtractor`, permissions, and derive macros.

### Task 1.1: Scaffold `tools-core` crate

**Files:**
- Create: `crates/tools-core/Cargo.toml`
- Create: `crates/tools-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace root — add member + workspace dep)

**Step 1: Create crate directory**

Run: `mkdir -p crates/tools-core/src`

**Step 2: Write Cargo.toml**

```toml
[package]
name = "tools-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
async-trait.workspace = true
serde_json.workspace = true
serde.workspace = true
tokio.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
```

**Step 3: Write initial lib.rs with module stubs**

```rust
//! Core tool framework for klyntbot feature packages.
//!
//! Provides the `Tool` trait, `FeaturePackage` trait, `ToolRegistry`,
//! `ParamExtractor`, and derive macros that eliminate boilerplate
//! when building feature packages.

pub mod feature;
pub mod params;
pub mod permissions;
pub mod registry;

use async_trait::async_trait;
use common::{ChannelName, ChatId, InteractionBundle, Result};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

// --- Tool trait (moved from tools/src/lib.rs) ---

/// Core trait that all tools must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;

    fn permission_level(&self) -> permissions::PermissionLevel {
        permissions::PermissionLevel::Standard
    }

    fn to_schema(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.parameters()
            }
        })
    }

    fn validate_params(&self, params: &Value) -> Vec<String> {
        let schema = self.parameters();
        let mut errors = Vec::new();
        // ... validation logic (copy from tools/src/lib.rs)
        errors
    }
}

pub type DynTool = Arc<dyn Tool>;

/// Execution context passed to every tool invocation.
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
}

impl RoutingContext {
    pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
        Self { channel, chat_id, interaction_tx: None }
    }

    pub fn with_interaction(
        channel: ChannelName,
        chat_id: ChatId,
        tx: mpsc::Sender<InteractionBundle>,
    ) -> Self {
        Self { channel, chat_id, interaction_tx: Some(tx) }
    }
}
```

**Step 4: Add to workspace root Cargo.toml**

Add `"crates/tools-core"` to `[workspace].members` array.
Add `tools-core = { path = "crates/tools-core" }` to `[workspace.dependencies]`.

**Step 5: Verify it compiles**

Run: `cargo build -p tools-core`
Expected: SUCCESS (no errors)

**Step 6: Commit**

```bash
git add crates/tools-core/ Cargo.toml
git commit -m "feat: scaffold tools-core crate with Tool trait and RoutingContext"
```

---

### Task 1.2: Move ParamExtractor to tools-core

**Files:**
- Copy from: `crates/tools/src/params.rs` (517 lines)
- Create: `crates/tools-core/src/params.rs`
- Modify: `crates/tools-core/src/lib.rs` (add `pub use params::ParamExtractor;`)

**Step 1: Copy params.rs to tools-core**

Copy `crates/tools/src/params.rs` → `crates/tools-core/src/params.rs`

Update imports: change `use common::` to match tools-core's deps. The ParamExtractor depends only on `serde_json::Value` and `common::KlyntbotError` (via `ToolError`), so this should be straightforward.

**Step 2: Re-export from lib.rs**

Add to `crates/tools-core/src/lib.rs`:
```rust
pub use params::ParamExtractor;
```

**Step 3: Verify it compiles**

Run: `cargo build -p tools-core`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add crates/tools-core/
git commit -m "feat(tools-core): move ParamExtractor from tools crate"
```

---

### Task 1.3: Move ToolRegistry to tools-core

**Files:**
- Copy from: `crates/tools/src/registry.rs` (264 lines)
- Create: `crates/tools-core/src/registry.rs`
- Modify: `crates/tools-core/src/lib.rs` (re-export)

**Step 1: Copy registry.rs to tools-core**

Copy `crates/tools/src/registry.rs` → `crates/tools-core/src/registry.rs`

Update imports to use `crate::Tool`, `crate::DynTool`, `crate::RoutingContext`, `crate::permissions::*`.

**Step 2: Add `register_dyn` method for Arc<dyn Tool>**

Add this method to ToolRegistry:
```rust
/// Register a pre-wrapped dynamic tool (used by FeaturePackage).
pub fn register_dyn(&mut self, tool: DynTool) {
    let name = tool.name().to_string();
    self.tools.insert(name, tool);
    self.invalidate_cache();
}
```

**Step 3: Re-export from lib.rs**

```rust
pub use registry::ToolRegistry;
```

**Step 4: Verify**

Run: `cargo build -p tools-core`

**Step 5: Commit**

```bash
git add crates/tools-core/
git commit -m "feat(tools-core): move ToolRegistry with register_dyn support"
```

---

### Task 1.4: Move permissions to tools-core

**Files:**
- Copy from: `crates/tools/src/permissions.rs` (112 lines)
- Create: `crates/tools-core/src/permissions.rs`

**Step 1: Copy permissions.rs**

Copy and update imports. `PermissionLevel` and `ToolPermissions` have no external deps beyond `serde`.

**Step 2: Re-export from lib.rs**

```rust
pub use permissions::{PermissionLevel, ToolPermissions};
```

**Step 3: Verify and commit**

Run: `cargo build -p tools-core`

```bash
git add crates/tools-core/
git commit -m "feat(tools-core): move PermissionLevel and ToolPermissions"
```

---

### Task 1.5: Add FeaturePackage trait and migration types

**Files:**
- Create: `crates/tools-core/src/feature.rs`
- Modify: `crates/tools-core/src/lib.rs`

**Step 1: Write the FeaturePackage trait**

```rust
//! Feature package abstraction for self-contained klyntbot features.

use crate::DynTool;
use async_trait::async_trait;
use common::Result;
use serde_json::Value;

/// A SQL migration owned by a feature.
#[derive(Debug, Clone)]
pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub description: String,
    pub sql: String,
}

/// Health status for a feature.
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Trait that all feature packages must implement.
///
/// Each feature crate exports a struct implementing this trait.
/// The agent discovers features and registers their tools automatically.
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    /// Unique feature name (e.g., "todo", "finance").
    fn name(&self) -> &str;

    /// The tool(s) this feature provides.
    fn tools(&self) -> Vec<DynTool>;

    /// SQL migrations owned by this feature, in order.
    fn migrations(&self) -> Vec<FeatureMigration>;

    /// Config section key (e.g., "todo", "finance").
    fn config_key(&self) -> &str;

    /// Default config value (merged if section is missing).
    fn default_config(&self) -> Value;

    /// Health check (default: healthy).
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

**Step 2: Re-export from lib.rs**

```rust
pub use feature::{FeatureMigration, FeaturePackage, HealthStatus};
```

**Step 3: Verify and commit**

Run: `cargo build -p tools-core`

```bash
git add crates/tools-core/
git commit -m "feat(tools-core): add FeaturePackage trait and FeatureMigration type"
```

---

### Task 1.6: Add Page<T> pagination type

**Files:**
- Create: `crates/tools-core/src/pagination.rs`
- Modify: `crates/tools-core/src/lib.rs`

**Step 1: Write cursor-based pagination type**

```rust
//! Cursor-based pagination for tool list operations.

use serde::Serialize;

/// A page of results with an opaque cursor for fetching the next page.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub has_more: bool,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, cursor: Option<String>, has_more: bool) -> Self {
        Self { items, cursor, has_more }
    }

    pub fn empty() -> Self {
        Self { items: Vec::new(), cursor: None, has_more: false }
    }

    pub fn single_page(items: Vec<T>) -> Self {
        Self { items, cursor: None, has_more: false }
    }
}
```

**Step 2: Re-export and verify**

```rust
pub use pagination::Page;
```

Run: `cargo build -p tools-core`

**Step 3: Commit**

```bash
git add crates/tools-core/
git commit -m "feat(tools-core): add Page<T> cursor-based pagination type"
```

---

### Task 1.7: Wire tools crate to re-export from tools-core

This is the bridge step. The existing `tools` crate starts depending on `tools-core` and re-exporting its types, so all existing consumers keep working without changes yet.

**Files:**
- Modify: `crates/tools/Cargo.toml` (add `tools-core` dependency)
- Modify: `crates/tools/src/lib.rs` (re-export from tools-core)

**Step 1: Add tools-core dependency to tools/Cargo.toml**

Add `tools-core.workspace = true` to `[dependencies]`.

**Step 2: Re-export tools-core types from tools/src/lib.rs**

Add at the top of `crates/tools/src/lib.rs`:
```rust
// Re-export from tools-core for backward compatibility.
// Consumers should gradually migrate to importing from tools-core directly.
pub use tools_core::{
    DynTool, FeatureMigration, FeaturePackage, HealthStatus,
    Page, ParamExtractor, PermissionLevel, RoutingContext,
    Tool, ToolPermissions, ToolRegistry,
};
```

Keep the existing `Tool` trait, `RoutingContext`, `ParamExtractor`, etc. in the tools crate for now. We'll remove the duplicates after all consumers migrate.

**Important:** This step might cause "duplicate definition" issues. If so, remove the local definitions from tools/src/lib.rs and keep only the re-exports. All tools in the tools crate should import from `crate::` which will resolve to the re-exported types.

**Step 3: Verify the full workspace compiles**

Run: `cargo build --workspace`
Expected: SUCCESS (everything still works)

**Step 4: Commit**

```bash
git add crates/tools/ Cargo.toml
git commit -m "feat(tools): re-export tools-core types for backward compatibility"
```

---

## Phase 2: Derive Macros

### Task 2.1: Scaffold proc-macro crate

**Files:**
- Create: `crates/tools-core-macros/Cargo.toml`
- Create: `crates/tools-core-macros/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/tools-core/Cargo.toml` (add macros dep)

**Step 1: Create crate**

```bash
mkdir -p crates/tools-core-macros/src
```

**Step 2: Write Cargo.toml**

```toml
[package]
name = "tools-core-macros"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full", "extra-traits"] }
quote = "1"
proc-macro2 = "1"
```

**Step 3: Write lib.rs stub**

```rust
//! Proc macros for klyntbot tool framework.
//!
//! - `#[derive(DomainEnum)]` — generates from_str_loose, as_str, Display
//! - `#[derive(ActionParams)]` — generates JSON Schema + from_value
//! - `#[tool_actions]` — generates Tool::execute dispatch

use proc_macro::TokenStream;

mod domain_enum;
mod action_params;
mod tool_actions;

#[proc_macro_derive(DomainEnum, attributes(aliases, canonical))]
pub fn derive_domain_enum(input: TokenStream) -> TokenStream {
    domain_enum::derive(input)
}

#[proc_macro_derive(ActionParams, attributes(param))]
pub fn derive_action_params(input: TokenStream) -> TokenStream {
    action_params::derive(input)
}

#[proc_macro_attribute]
pub fn tool_actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool_actions::expand(attr, item)
}
```

**Step 4: Create stub module files**

Create `crates/tools-core-macros/src/domain_enum.rs`:
```rust
use proc_macro::TokenStream;

pub fn derive(input: TokenStream) -> TokenStream {
    // Placeholder — implemented in Task 2.2
    TokenStream::new()
}
```

Create similar stubs for `action_params.rs` and `tool_actions.rs`.

**Step 5: Add to workspace and wire to tools-core**

Add `"crates/tools-core-macros"` to workspace members.
Add `tools-core-macros = { path = "crates/tools-core-macros" }` to workspace deps.
Add `tools-core-macros.workspace = true` to `crates/tools-core/Cargo.toml`.
Add `pub use tools_core_macros::*;` to `crates/tools-core/src/lib.rs`.

**Step 6: Verify and commit**

Run: `cargo build -p tools-core-macros && cargo build -p tools-core`

```bash
git add crates/tools-core-macros/ crates/tools-core/ Cargo.toml
git commit -m "feat: scaffold tools-core-macros proc-macro crate"
```

---

### Task 2.2: Implement DomainEnum derive macro

**Files:**
- Modify: `crates/tools-core-macros/src/domain_enum.rs`
- Create: `crates/tools-core-macros/tests/domain_enum_tests.rs`

**Step 1: Write the failing test**

Create `crates/tools-core-macros/tests/domain_enum_tests.rs`:
```rust
use tools_core_macros::DomainEnum;

#[derive(Debug, Clone, PartialEq, DomainEnum)]
pub enum TestStatus {
    #[aliases("pending", "open")]
    Todo,
    #[aliases("in_progress", "active")]
    Doing,
    #[aliases("completed", "closed")]
    Done,
    Archived,
}

#[test]
fn test_as_str() {
    assert_eq!(TestStatus::Todo.as_str(), "todo");
    assert_eq!(TestStatus::Doing.as_str(), "doing");
    assert_eq!(TestStatus::Done.as_str(), "done");
    assert_eq!(TestStatus::Archived.as_str(), "archived");
}

#[test]
fn test_from_str_loose_canonical() {
    assert_eq!(TestStatus::from_str_loose("todo"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("doing"), Some(TestStatus::Doing));
    assert_eq!(TestStatus::from_str_loose("done"), Some(TestStatus::Done));
    assert_eq!(TestStatus::from_str_loose("archived"), Some(TestStatus::Archived));
}

#[test]
fn test_from_str_loose_aliases() {
    assert_eq!(TestStatus::from_str_loose("pending"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("open"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("in_progress"), Some(TestStatus::Doing));
    assert_eq!(TestStatus::from_str_loose("active"), Some(TestStatus::Doing));
    assert_eq!(TestStatus::from_str_loose("completed"), Some(TestStatus::Done));
    assert_eq!(TestStatus::from_str_loose("closed"), Some(TestStatus::Done));
}

#[test]
fn test_from_str_loose_case_insensitive() {
    assert_eq!(TestStatus::from_str_loose("TODO"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("Doing"), Some(TestStatus::Doing));
    assert_eq!(TestStatus::from_str_loose("ARCHIVED"), Some(TestStatus::Archived));
}

#[test]
fn test_from_str_loose_unknown() {
    assert_eq!(TestStatus::from_str_loose("unknown"), None);
    assert_eq!(TestStatus::from_str_loose(""), None);
}

#[test]
fn test_display() {
    assert_eq!(format!("{}", TestStatus::Todo), "todo");
    assert_eq!(format!("{}", TestStatus::Doing), "doing");
}

#[test]
fn test_from_str() {
    assert_eq!("todo".parse::<TestStatus>(), Ok(TestStatus::Todo));
    assert!("unknown".parse::<TestStatus>().is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools-core-macros`
Expected: FAIL (macro is a stub)

**Step 3: Implement the macro**

In `crates/tools-core-macros/src/domain_enum.rs`:

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("DomainEnum can only be derived for enums"),
    };

    let mut as_str_arms = Vec::new();
    let mut from_str_arms = Vec::new();

    for variant in variants {
        assert!(
            matches!(variant.fields, Fields::Unit),
            "DomainEnum variants must be unit variants"
        );

        let variant_ident = &variant.ident;
        let canonical = variant.ident.to_string();
        let canonical_lower = canonical
            .chars()
            .enumerate()
            .fold(String::new(), |mut acc, (i, c)| {
                if c.is_uppercase() && i > 0 {
                    acc.push('_');
                }
                acc.push(c.to_ascii_lowercase());
                acc
            });

        // as_str arm
        as_str_arms.push(quote! {
            #name::#variant_ident => #canonical_lower,
        });

        // from_str: canonical name
        from_str_arms.push(quote! {
            #canonical_lower => Some(#name::#variant_ident),
        });

        // from_str: aliases
        for attr in &variant.attrs {
            if attr.path().is_ident("aliases") {
                if let Meta::List(list) = &attr.meta {
                    let tokens = list.tokens.clone();
                    let parser = syn::punctuated::Punctuated::<Lit, syn::Token![,]>::parse_terminated;
                    let aliases = parser.parse2(tokens).expect("Expected string literals in #[aliases(...)]");
                    for lit in aliases {
                        if let Lit::Str(s) = lit {
                            let alias = s.value().to_lowercase();
                            from_str_arms.push(quote! {
                                #alias => Some(#name::#variant_ident),
                            });
                        }
                    }
                }
            }
        }
    }

    let expanded = quote! {
        impl #name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms)*
                }
            }

            pub fn from_str_loose(s: &str) -> Option<Self> {
                match s.to_lowercase().as_str() {
                    #(#from_str_arms)*
                    _ => None,
                }
            }
        }

        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl ::std::str::FromStr for #name {
            type Err = String;

            fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                Self::from_str_loose(s)
                    .ok_or_else(|| format!("unknown {}: {}", stringify!(#name), s))
            }
        }
    };

    TokenStream::from(expanded)
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p tools-core-macros`
Expected: All 7 tests PASS

**Step 5: Commit**

```bash
git add crates/tools-core-macros/
git commit -m "feat(macros): implement DomainEnum derive macro with alias support"
```

---

### Task 2.3: Implement ActionParams derive macro

**Files:**
- Modify: `crates/tools-core-macros/src/action_params.rs`
- Create: `crates/tools-core-macros/tests/action_params_tests.rs`

**Step 1: Write the failing test**

Create `crates/tools-core-macros/tests/action_params_tests.rs`:
```rust
use tools_core_macros::ActionParams;
use serde_json::json;

#[derive(ActionParams)]
pub struct AddParams {
    /// Task title
    #[param(required)]
    pub title: String,

    /// Task priority (1-5)
    #[param(min = 1, max = 5)]
    pub priority: Option<u8>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Optional description
    pub description: Option<String>,
}

#[test]
fn test_json_schema_has_required_fields() {
    let schema = AddParams::json_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "title"));
}

#[test]
fn test_json_schema_has_properties() {
    let schema = AddParams::json_schema();
    assert!(schema["properties"]["title"]["type"] == "string");
    assert!(schema["properties"]["priority"]["type"] == "integer");
    assert!(schema["properties"]["tags"]["type"] == "array");
    assert!(schema["properties"]["description"]["type"] == "string");
}

#[test]
fn test_json_schema_constraints() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["priority"]["minimum"], 1);
    assert_eq!(schema["properties"]["priority"]["maximum"], 5);
}

#[test]
fn test_json_schema_descriptions() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["title"]["description"], "Task title");
    assert_eq!(schema["properties"]["priority"]["description"], "Task priority (1-5)");
}

#[test]
fn test_from_value_valid() {
    let args = json!({
        "title": "Buy groceries",
        "priority": 2,
        "tags": ["shopping", "personal"]
    });
    let params = AddParams::from_value(&args).unwrap();
    assert_eq!(params.title, "Buy groceries");
    assert_eq!(params.priority, Some(2));
    assert_eq!(params.tags, vec!["shopping", "personal"]);
    assert_eq!(params.description, None);
}

#[test]
fn test_from_value_missing_required() {
    let args = json!({ "priority": 2 });
    let result = AddParams::from_value(&args);
    assert!(result.is_err());
}

#[test]
fn test_from_value_empty_optional_vec() {
    let args = json!({ "title": "test" });
    let params = AddParams::from_value(&args).unwrap();
    assert!(params.tags.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools-core-macros -- action_params`
Expected: FAIL

**Step 3: Implement the macro**

In `crates/tools-core-macros/src/action_params.rs`, implement a proc macro that:
1. Iterates over struct fields
2. For `String` fields → `{ "type": "string" }`
3. For `Option<u8/u16/u32/u64/i8/i16/i32/i64>` → `{ "type": "integer" }` + min/max from `#[param]`
4. For `Option<f32/f64>` → `{ "type": "number" }`
5. For `Option<bool>` → `{ "type": "boolean" }`
6. For `Option<String>` → `{ "type": "string" }`
7. For `Vec<String>` → `{ "type": "array", "items": { "type": "string" } }`
8. Fields with `#[param(required)]` go into `required` array
9. Doc comments become `description`
10. Generates `fn json_schema() -> serde_json::Value`
11. Generates `fn from_value(args: &serde_json::Value) -> Result<Self, String>`

The `from_value` method:
- Required String: `args["field"].as_str().ok_or("missing field")?.to_string()`
- Option<T>: `args.get("field").and_then(|v| v.as_TYPE())`
- Vec<String>: `args.get("field").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()`

**Step 4: Run tests**

Run: `cargo nextest run -p tools-core-macros -- action_params`
Expected: All 7 tests PASS

**Step 5: Commit**

```bash
git add crates/tools-core-macros/
git commit -m "feat(macros): implement ActionParams derive macro with JSON Schema generation"
```

---

### Task 2.4: Implement tool_actions attribute macro

**Files:**
- Modify: `crates/tools-core-macros/src/tool_actions.rs`
- Create: `crates/tools-core-macros/tests/tool_actions_tests.rs`

This is the most complex macro. It:
1. Parses an `impl` block annotated with `#[tool_actions]`
2. Finds methods annotated with `#[action(name = "...")]`
3. Extracts the params type from each method signature
4. Generates `Tool::parameters()` that merges all action schemas with an `"action"` discriminator enum
5. Generates `Tool::execute()` that matches on `action` param and dispatches to the correct method with parsed params

**Step 1: Write test (compile-time test that macro expands)**

The full integration test for `tool_actions` requires the `Tool` trait from `tools-core`. Create a test that verifies the generated schema and dispatch logic. Since this is a proc-macro crate, the easiest way is an integration test in tools-core itself.

Create `crates/tools-core/tests/tool_actions_integration.rs`:
```rust
use tools_core::{ActionParams, tool_actions, Tool, RoutingContext};
use common::{ChannelName, ChatId, Result};
use serde_json::{json, Value};

#[derive(ActionParams)]
pub struct GreetParams {
    /// Name to greet
    #[param(required)]
    pub name: String,
}

#[derive(ActionParams)]
pub struct FarewellParams {
    /// Name to bid farewell
    #[param(required)]
    pub name: String,
    /// Include emoji
    pub emoji: Option<bool>,
}

pub struct TestTool;

#[tool_actions(name = "test_tool", description = "A test tool")]
impl TestTool {
    /// Say hello to someone
    #[action(name = "greet")]
    async fn handle_greet(&self, params: GreetParams, _ctx: &RoutingContext) -> Result<String> {
        Ok(format!("Hello, {}!", params.name))
    }

    /// Say goodbye to someone
    #[action(name = "farewell")]
    async fn handle_farewell(&self, params: FarewellParams, _ctx: &RoutingContext) -> Result<String> {
        let emoji = if params.emoji.unwrap_or(false) { " 👋" } else { "" };
        Ok(format!("Goodbye, {}!{}", params.name, emoji))
    }
}

#[test]
fn test_generated_name() {
    let tool = TestTool;
    assert_eq!(tool.name(), "test_tool");
}

#[test]
fn test_generated_description() {
    let tool = TestTool;
    assert_eq!(tool.description(), "A test tool");
}

#[test]
fn test_generated_parameters_has_action_enum() {
    let tool = TestTool;
    let params = tool.parameters();
    let action_enum = params["properties"]["action"]["enum"].as_array().unwrap();
    assert!(action_enum.iter().any(|v| v == "greet"));
    assert!(action_enum.iter().any(|v| v == "farewell"));
}

#[tokio::test]
async fn test_dispatch_greet() {
    let tool = TestTool;
    let ctx = RoutingContext::new(ChannelName::from("test"), ChatId::from("123"));
    let result = tool.execute(json!({"action": "greet", "name": "World"}), &ctx).await.unwrap();
    assert_eq!(result, "Hello, World!");
}

#[tokio::test]
async fn test_dispatch_farewell() {
    let tool = TestTool;
    let ctx = RoutingContext::new(ChannelName::from("test"), ChatId::from("123"));
    let result = tool.execute(json!({"action": "farewell", "name": "World", "emoji": true}), &ctx).await.unwrap();
    assert_eq!(result, "Goodbye, World! 👋");
}

#[tokio::test]
async fn test_dispatch_unknown_action() {
    let tool = TestTool;
    let ctx = RoutingContext::new(ChannelName::from("test"), ChatId::from("123"));
    let result = tool.execute(json!({"action": "dance"}), &ctx).await;
    assert!(result.is_err());
}
```

**Step 2: Implement the macro**

In `crates/tools-core-macros/src/tool_actions.rs`, the attribute macro:
1. Parses the `#[tool_actions(name = "...", description = "...")]` attributes
2. Iterates over methods with `#[action(name = "...")]`
3. Extracts the params type (second arg after `&self`)
4. Generates `impl Tool for Struct`:
   - `fn name()` → from attribute
   - `fn description()` → from attribute
   - `fn parameters()` → merge all action schemas, add `action` enum property
   - `async fn execute()` → match on `action` string, call `ActionParams::from_value()`, dispatch

**Step 3: Run tests**

Run: `cargo nextest run -p tools-core -- tool_actions`
Expected: All 6 tests PASS

**Step 4: Commit**

```bash
git add crates/tools-core-macros/ crates/tools-core/
git commit -m "feat(macros): implement tool_actions attribute macro for action dispatch"
```

---

## Phase 3: Storage Slimming

### Task 3.1: Add feature migration support to StoragePool

**Files:**
- Modify: `crates/storage/src/pool.rs`
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/migrations/20260220000000_feature_migration_tracking.sql`

**Step 1: Write failing test**

Add to storage tests:
```rust
#[tokio::test]
async fn test_feature_migration_tracking() {
    // Create pool, run feature migration, verify it was tracked
    let pool = test_pool().await;
    let migration = tools_core::FeatureMigration {
        feature_name: "test_feature".into(),
        version: 1,
        description: "create test table".into(),
        sql: "CREATE TABLE IF NOT EXISTS _test_feature_t (id TEXT PRIMARY KEY);".into(),
    };
    StoragePool::run_feature_migrations(pool.inner(), &[migration]).await.unwrap();
    // Verify migration was recorded
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _feature_migrations WHERE feature_name = 'test_feature'")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert_eq!(count.0, 1);
    // Cleanup
    sqlx::query("DROP TABLE IF EXISTS _test_feature_t").execute(pool.inner()).await.unwrap();
}
```

**Step 2: Create the tracking migration**

`20260220000000_feature_migration_tracking.sql`:
```sql
CREATE TABLE IF NOT EXISTS _feature_migrations (
    feature_name TEXT NOT NULL,
    version BIGINT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (feature_name, version)
);
```

**Step 3: Add `run_feature_migrations` to StoragePool**

```rust
impl StoragePool {
    /// Run feature-owned migrations that haven't been applied yet.
    pub async fn run_feature_migrations(
        pool: &sqlx::PgPool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError> {
        for m in migrations {
            // Check if already applied
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM _feature_migrations WHERE feature_name = $1 AND version = $2)"
            )
            .bind(&m.feature_name)
            .bind(m.version)
            .fetch_one(pool)
            .await?;

            if !exists {
                tracing::info!(
                    feature = %m.feature_name,
                    version = m.version,
                    description = %m.description,
                    "Running feature migration"
                );
                sqlx::query(&m.sql).execute(pool).await?;
                sqlx::query(
                    "INSERT INTO _feature_migrations (feature_name, version, description) VALUES ($1, $2, $3)"
                )
                .bind(&m.feature_name)
                .bind(m.version)
                .bind(&m.description)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}
```

**Step 4: Add tools-core dependency to storage**

Add `tools-core.workspace = true` to `crates/storage/Cargo.toml`.

**Step 5: Run tests**

Run: `cargo nextest run -p storage -- feature_migration`

**Step 6: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add feature migration tracking with _feature_migrations table"
```

---

## Phase 4: Create `feature-todo` Crate

This is the largest phase. We're creating a self-contained crate that owns the entire Todo feature.

### Task 4.1: Scaffold feature-todo crate

**Files:**
- Create: `crates/feature-todo/Cargo.toml`
- Create: `crates/feature-todo/src/lib.rs`
- Create directory structure:
  - `crates/feature-todo/src/tool/`
  - `crates/feature-todo/src/storage/`
  - `crates/feature-todo/migrations/`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Create directory structure**

```bash
mkdir -p crates/feature-todo/src/{tool/actions,storage}
mkdir -p crates/feature-todo/migrations
```

**Step 2: Write Cargo.toml**

```toml
[package]
name = "feature-todo"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
tools-core.workspace = true
storage.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
tokio.workspace = true
tracing.workspace = true
rrule.workspace = true
fastembed.workspace = true
pgvector.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
```

**Step 3: Write lib.rs stub**

```rust
//! Self-contained Todo feature package for klyntbot.

pub mod config;
pub mod enrichment;
pub mod embedding;
pub mod handler;
pub mod storage;
pub mod tool;
pub mod types;

use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};
use common::Result;
use serde_json::Value;
use std::sync::Arc;

pub struct TodoFeature {
    tool: Arc<tool::TodoTool>,
    config: config::TodoConfig,
}

impl TodoFeature {
    pub async fn new(pool: &sqlx::PgPool, raw_config: &Value) -> Result<Self> {
        let config: config::TodoConfig = serde_json::from_value(raw_config.clone())
            .unwrap_or_default();
        let repo = storage::TodoRepo::new(pool.clone());
        let tool = Arc::new(tool::TodoTool::new(repo, &config));
        Ok(Self { tool, config })
    }

    /// Get mutable access to the tool for handler injection.
    pub fn tool_mut(&mut self) -> &mut tool::TodoTool {
        Arc::get_mut(&mut self.tool)
            .expect("TodoTool has no other references during setup")
    }
}

#[async_trait::async_trait]
impl FeaturePackage for TodoFeature {
    fn name(&self) -> &str { "todo" }

    fn tools(&self) -> Vec<DynTool> {
        vec![self.tool.clone() as DynTool]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        // Will be populated when migrations are moved
        vec![]
    }

    fn config_key(&self) -> &str { "todo" }

    fn default_config(&self) -> Value {
        serde_json::to_value(config::TodoConfig::default()).unwrap()
    }
}
```

**Step 4: Add to workspace**

Add `"crates/feature-todo"` to workspace members.
Add `feature-todo = { path = "crates/feature-todo" }` to workspace deps.

**Step 5: Verify it compiles (will need stubs for modules)**

Create stub files for each module referenced in lib.rs.

Run: `cargo build -p feature-todo`

**Step 6: Commit**

```bash
git add crates/feature-todo/ Cargo.toml
git commit -m "feat: scaffold feature-todo crate with FeaturePackage impl"
```

---

### Task 4.2: Move Todo types to feature-todo

**Files:**
- Copy from: `crates/tools/src/todo_types.rs` (477 lines)
- Create: `crates/feature-todo/src/types.rs`

**Step 1: Copy todo_types.rs**

Copy `crates/tools/src/todo_types.rs` → `crates/feature-todo/src/types.rs`

Update imports to use `tools_core::DomainEnum` derive macro where applicable.

**Step 2: Apply DomainEnum macro to TodoStatus**

Replace the manual `impl` blocks for `TodoStatus` with:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DomainEnum)]
pub enum TodoStatus {
    #[aliases("pending", "open")]
    Todo,
    #[aliases("in_progress", "active")]
    Doing,
    #[aliases("completed", "closed")]
    Done,
    Archived,
}
```

Remove the manual `from_str_loose()`, `as_str()`, `Display` impls that were previously hand-written.

**Step 3: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move Todo types with DomainEnum macro"
```

---

### Task 4.3: Move Todo config to feature-todo

**Files:**
- Copy from: `crates/config/src/schema/todo.rs` (203 lines)
- Create: `crates/feature-todo/src/config.rs`

**Step 1: Copy todo.rs config**

Copy `crates/config/src/schema/todo.rs` → `crates/feature-todo/src/config.rs`

Remove the dependency on `super::core::default_true` and `super::core::default_semantic_threshold` — inline these as local functions or use serde defaults.

**Step 2: Apply DomainEnum to CreationMode**

```rust
#[derive(Debug, Clone, PartialEq, Eq, DomainEnum)]
pub enum CreationMode {
    #[aliases("ask_first")]
    AskFirst,
    Yolo,
    Party,
}
```

**Step 3: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move TodoConfig with DomainEnum for CreationMode"
```

---

### Task 4.4: Move Todo storage (repo + rows + migrations) to feature-todo

**Files:**
- Copy from: `crates/storage/src/repos/todo_repo.rs` (848 lines)
- Copy from: `crates/storage/src/rows/todo.rs` (64 lines)
- Copy from: `crates/storage/migrations/20240101000000_initial.sql` (todo-related DDL)
- Create: `crates/feature-todo/src/storage/repo.rs`
- Create: `crates/feature-todo/src/storage/rows.rs`
- Create: `crates/feature-todo/src/storage/mod.rs`
- Create: `crates/feature-todo/migrations/001_create_todos.sql`

**Step 1: Extract todo DDL from initial migration**

From `20240101000000_initial.sql`, extract the `CREATE TABLE todos`, `todo_attachments`, `todo_time_entries`, `todo_dependencies` statements into `crates/feature-todo/migrations/001_create_todos.sql`.

**Step 2: Copy row structs**

Copy `crates/storage/src/rows/todo.rs` → `crates/feature-todo/src/storage/rows.rs`

**Step 3: Copy repo**

Copy `crates/storage/src/repos/todo_repo.rs` → `crates/feature-todo/src/storage/repo.rs`

Update imports: `use crate::storage::rows::*` instead of `use crate::rows::todo::*`.

**Step 4: Write storage/mod.rs**

```rust
pub mod repo;
pub mod rows;

pub use repo::{TodoRepo, TodoFilter, TodoPatch, TodoSummary};
pub use rows::{TodoRow, TodoAttachmentRow, TodoTimeEntryRow, TodoDependencyRow};
```

**Step 5: Populate migrations in FeaturePackage impl**

Update `lib.rs` to return the migration SQL from `migrations/001_create_todos.sql` using `include_str!()`.

**Step 6: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move Todo storage (repo, rows, migrations)"
```

---

### Task 4.5: Move Todo handler traits to feature-todo

**Files:**
- Copy from: `crates/tools/src/enrichment.rs` (152 lines)
- Copy from: `crates/tools/src/embedding_engine.rs` (374 lines)
- Copy from: `crates/tools/src/learning_feedback.rs` (34 lines)
- Create: `crates/feature-todo/src/enrichment.rs`
- Create: `crates/feature-todo/src/embedding.rs`
- Create: `crates/feature-todo/src/handler.rs`

**Step 1: Copy enrichment handler trait**

Copy the `EnrichmentHandler` trait, `EnrichmentResult`, `EnrichmentSuggestion` from `enrichment.rs`.

**Step 2: Copy embedding handler trait**

Copy the `EmbeddingHandler` trait, `EmbeddingEngine`, `EmbeddingEngineImpl`, `EmbeddingRecord`, `EMBEDDING_DIM` from `embedding_engine.rs`.

Note: The `EmbeddingEngine` struct wraps `fastembed` and should stay in feature-todo since it's primarily used for todo embeddings. If other features need embeddings later, we can extract a shared `embedding-core` crate.

**Step 3: Create handler.rs with CalendarHandler + FeedbackHandler re-exports**

```rust
//! Handler traits for todo feature dependency inversion.
//! Implementations live in the agent crate.

pub use crate::enrichment::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion};
pub use crate::embedding::{EmbeddingHandler, EmbeddingRecord, EMBEDDING_DIM};

// CalendarHandler stays in tools/calendar_tool.rs since it's shared
// FeedbackHandler for enrichment learning
use async_trait::async_trait;
use common::Result;

#[async_trait]
pub trait EnrichmentFeedbackHandler: Send + Sync {
    async fn record_feedback(&self, entry: EnrichmentFeedbackEntry) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct EnrichmentFeedbackEntry {
    pub task_id: String,
    pub field: String,
    pub suggested_value: String,
    pub actual_value: String,
    pub accepted: bool,
    pub confidence: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

**Step 4: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move handler traits (enrichment, embedding, feedback)"
```

---

### Task 4.6: Move TodoTool implementation to feature-todo

**Files:**
- Copy from: `crates/tools/src/todo/mod.rs` (525 lines)
- Copy from: `crates/tools/src/todo/actions/*.rs` (6 files, ~1,350 lines)
- Create: `crates/feature-todo/src/tool/mod.rs`
- Create: `crates/feature-todo/src/tool/actions/` (6 files)

**Step 1: Copy TodoTool orchestrator**

Copy `crates/tools/src/todo/mod.rs` → `crates/feature-todo/src/tool/mod.rs`

Update all imports to use local types:
- `use crate::types::*`
- `use crate::storage::*`
- `use crate::config::*`
- `use crate::handler::*`
- `use tools_core::{Tool, RoutingContext, ParamExtractor, DynTool}`

**Step 2: Copy action handlers**

Copy all files from `crates/tools/src/todo/actions/` → `crates/feature-todo/src/tool/actions/`

Update imports in each file.

**Step 3: Apply ActionParams derive to param structs (if creating new ones)**

For this initial move, keep using `ParamExtractor` in the action handlers. We can migrate to `ActionParams` derive in a follow-up pass to keep the diff manageable.

**Step 4: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move TodoTool and all action handlers"
```

---

### Task 4.7: Move search utilities to feature-todo

**Files:**
- Copy from: `crates/tools/src/search_utils.rs` (234 lines)
- Copy from: `crates/tools/src/rrule_utils.rs` (576 lines)
- Copy from: `crates/tools/src/embedding_store.rs` (549 lines)
- Create corresponding files in `crates/feature-todo/src/`

**Step 1: Copy search utilities**

These are used exclusively by the todo feature's semantic/hybrid search.

**Step 2: Verify and commit**

Run: `cargo build -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "feat(feature-todo): move search utils, rrule utils, embedding store"
```

---

### Task 4.8: Integration test — TodoFeature as FeaturePackage

**Files:**
- Create: `crates/feature-todo/tests/feature_package_test.rs`

**Step 1: Write integration test**

```rust
use feature_todo::TodoFeature;
use tools_core::FeaturePackage;
use serde_json::json;

#[test]
fn test_todo_feature_package_basics() {
    // Can't create with real pool in unit test, but can test trait methods
    let default_config = TodoFeature::default_config_static();
    assert!(default_config.is_object());
    assert!(default_config["focus"]["maxSlots"].is_number());
}

#[test]
fn test_todo_feature_migrations_not_empty() {
    let migrations = TodoFeature::migrations_static();
    assert!(!migrations.is_empty());
    assert_eq!(migrations[0].feature_name, "todo");
}
```

Note: Full integration tests with a real database will be added in Phase 7.

**Step 2: Verify and commit**

Run: `cargo nextest run -p feature-todo`

```bash
git add crates/feature-todo/
git commit -m "test(feature-todo): add FeaturePackage integration tests"
```

---

## Phase 5: Create `feature-finance` Crate

Follows the same pattern as Phase 4 but for Finance.

### Task 5.1: Scaffold feature-finance crate

**Files:**
- Create: `crates/feature-finance/Cargo.toml`
- Create: `crates/feature-finance/src/lib.rs`
- Create directory structure
- Modify: `Cargo.toml` (workspace root)

Same pattern as Task 4.1 but for finance. The `FinanceFeature` struct implements `FeaturePackage`.

**Commit:** `feat: scaffold feature-finance crate with FeaturePackage impl`

---

### Task 5.2: Move Finance types to feature-finance

**Files:**
- Copy from: `crates/tools/src/finance_types.rs` (967 lines)
- Copy from: `crates/tools/src/finance_types_tests.rs` (553 lines)
- Create: `crates/feature-finance/src/types.rs`

Apply `DomainEnum` macro to all 10 finance enums:
- `AccountType`, `TransactionType`, `BudgetPeriod`, `BudgetMethod`, `JarType`, `AssetType`, `InvestmentTxType`, `GoalType`, `GoalStatus`, `LiabilityType`

This replaces ~180 lines of manual `from_str_loose()` + `as_str()` + `Display` impls with ~10 derive annotations.

**Commit:** `feat(feature-finance): move Finance types with DomainEnum macros`

---

### Task 5.3: Move Finance config to feature-finance

**Files:**
- Copy from: `crates/config/src/schema/finance.rs` (301 lines)
- Create: `crates/feature-finance/src/config.rs`

**Commit:** `feat(feature-finance): move FinanceConfig`

---

### Task 5.4: Move Finance storage to feature-finance

**Files:**
- Copy from: `crates/storage/src/repos/finance_*.rs` (6 repos, ~1,400 lines)
- Copy from: `crates/storage/src/rows/finance.rs` (288 lines)
- Copy from: `crates/storage/migrations/20260219100000_finance_tables.sql`
- Copy from: `crates/storage/src/repos/tests/finance_*.rs` (test files)
- Create: `crates/feature-finance/src/storage/`
- Create: `crates/feature-finance/migrations/`

**Commit:** `feat(feature-finance): move Finance storage (6 repos, rows, migrations, tests)`

---

### Task 5.5: Move Finance handler trait to feature-finance

**Files:**
- Copy from: `crates/tools/src/finance_handler.rs` (173 lines)
- Copy from: `crates/tools/src/price_service.rs` (595 lines)
- Create: `crates/feature-finance/src/handler.rs`
- Create: `crates/feature-finance/src/price_service.rs`

**Commit:** `feat(feature-finance): move FinanceHandler trait and PriceService`

---

### Task 5.6: Move FinanceTool implementation to feature-finance

**Files:**
- Copy from: `crates/tools/src/finance_tool/` (8 files, ~4,700 lines)
- Create: `crates/feature-finance/src/tool/`

**Commit:** `feat(feature-finance): move FinanceTool and all sub-modules`

---

### Task 5.7: Integration test — FinanceFeature as FeaturePackage

Same pattern as Task 4.8.

**Commit:** `test(feature-finance): add FeaturePackage integration tests`

---

## Phase 6: Slim Down Original Crates

### Task 6.1: Remove Todo code from tools crate

**Files:**
- Delete: `crates/tools/src/todo/` (entire directory)
- Delete: `crates/tools/src/todo_types.rs`
- Delete: `crates/tools/src/enrichment.rs`
- Delete: `crates/tools/src/embedding_engine.rs`
- Delete: `crates/tools/src/embedding_store.rs`
- Delete: `crates/tools/src/search_utils.rs`
- Delete: `crates/tools/src/rrule_utils.rs`
- Delete: `crates/tools/src/learning_feedback.rs`
- Modify: `crates/tools/src/lib.rs` (remove mod declarations and re-exports)
- Modify: `crates/tools/Cargo.toml` (remove fastembed, rrule, pgvector deps if no longer needed)

**Step 1: Remove files and mod declarations**

Remove all todo-related module declarations and re-exports from `crates/tools/src/lib.rs`.

**Step 2: Remove unused dependencies from Cargo.toml**

Check which deps were only used by todo code (fastembed, rrule, pgvector likely).

**Step 3: Verify workspace compiles**

Run: `cargo build --workspace`

**Step 4: Commit**

```bash
git add -A crates/tools/
git commit -m "refactor(tools): remove Todo code (moved to feature-todo)"
```

---

### Task 6.2: Remove Finance code from tools crate

**Files:**
- Delete: `crates/tools/src/finance_tool/` (entire directory)
- Delete: `crates/tools/src/finance_types.rs`
- Delete: `crates/tools/src/finance_types_tests.rs`
- Delete: `crates/tools/src/finance_handler.rs`
- Delete: `crates/tools/src/price_service.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/Cargo.toml`

Same pattern as Task 6.1.

**Commit:** `refactor(tools): remove Finance code (moved to feature-finance)`

---

### Task 6.3: Remove Todo and Finance from storage crate

**Files:**
- Delete: `crates/storage/src/repos/todo_repo.rs`
- Delete: `crates/storage/src/rows/todo.rs`
- Delete: `crates/storage/src/repos/finance_*.rs` (6 files)
- Delete: `crates/storage/src/rows/finance.rs`
- Delete: `crates/storage/src/repos/tests/finance_*.rs` (5 files)
- Delete: `crates/storage/src/repos/tests/todo_repo_tests.rs`
- Modify: `crates/storage/src/repos/mod.rs` (remove from Repos struct)
- Modify: `crates/storage/src/lib.rs` (remove re-exports)

**Step 1: Remove todo and finance fields from Repos struct**

Update `crates/storage/src/repos/mod.rs` to remove:
```rust
// Remove these fields from Repos:
pub todos: TodoRepo,
pub finance_accounts: FinanceAccountRepo,
pub finance_transactions: FinanceTransactionRepo,
pub finance_budgets: FinanceBudgetRepo,
pub finance_investments: FinanceInvestmentRepo,
pub finance_goals: FinanceGoalRepo,
pub finance_liabilities: FinanceLiabilityRepo,
```

**Step 2: Remove re-exports from lib.rs**

Remove all `TodoRepo`, `TodoRow`, `FinanceAccountRepo`, `FinanceAccountRow`, etc. re-exports.

**Step 3: Verify workspace compiles**

Run: `cargo build --workspace`

**Step 4: Commit**

```bash
git add -A crates/storage/
git commit -m "refactor(storage): remove Todo and Finance code (moved to feature crates)"
```

---

### Task 6.4: Remove Todo and Finance config from config crate

**Files:**
- Delete: `crates/config/src/schema/todo.rs`
- Delete: `crates/config/src/schema/finance.rs`
- Modify: `crates/config/src/schema/mod.rs` (remove mod/use declarations)
- Modify: `crates/config/src/schema/core.rs` (remove `todo` and `finance` fields from `Config`)

**Step 1: Remove todo and finance fields from Config struct**

In `core.rs`, remove:
```rust
pub todo: TodoConfig,
pub finance: FinanceConfig,
```

**Step 2: Remove mod declarations**

In `schema/mod.rs`, remove:
```rust
mod todo;
mod finance;
pub use self::todo::*;
pub use self::finance::*;
```

**Step 3: Fix tests in mod.rs that reference TodoConfig/FinanceConfig**

Remove or update tests that test `config.todo.*` or `config.finance.*`. These will now be tested in the feature crates.

**Step 4: Verify and commit**

Run: `cargo build --workspace`

```bash
git add -A crates/config/
git commit -m "refactor(config): remove Todo and Finance config (moved to feature crates)"
```

---

### Task 6.5: Rewrite agent builder to use FeaturePackage pattern

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/Cargo.toml` (add feature-todo, feature-finance deps)

This is the critical wiring change. The builder goes from 500+ lines of manual tool construction to a feature-discovery loop.

**Step 1: Add feature crate dependencies**

Add to `crates/agent/Cargo.toml`:
```toml
feature-todo.workspace = true
feature-finance.workspace = true
```

**Step 2: Rewrite the builder**

Replace the manual TodoTool/FinanceTool wiring blocks (lines 148-362 in builder.rs) with:

```rust
// --- Feature Packages ---
let raw_config = serde_json::to_value(&config)?;

// Todo feature
let mut todo_feature = feature_todo::TodoFeature::new(
    repos.pool(),
    &raw_config.get("todo").cloned().unwrap_or_default(),
).await?;

// Inject handlers (agent-layer implementations)
if config.calendar.is_any_enabled() {
    let adapter = Arc::new(CalendarSyncAdapter::new(/* ... */).await?);
    todo_feature.tool_mut().with_calendar_handler(adapter);
}
if config.todo.enrichment.enabled {
    let engine = Arc::new(EnrichmentEngine::new(/* ... */));
    todo_feature.tool_mut().with_enrichment_handler(engine);
}
// ... similar for embedding, feedback

// Register feature tools
for tool in todo_feature.tools() {
    tool_registry.register_dyn(tool);
}

// Finance feature (if enabled)
if config.finance.enabled {
    let mut finance_feature = feature_finance::FinanceFeature::new(
        repos.pool(),
        &raw_config.get("finance").cloned().unwrap_or_default(),
    ).await?;

    let handler = Arc::new(FinanceHandlerImpl::new(/* ... */));
    finance_feature.tool_mut().with_finance_handler(handler);

    for tool in finance_feature.tools() {
        tool_registry.register_dyn(tool);
    }
}
```

**Step 3: Run feature migrations at startup**

Before creating features, collect and run migrations:
```rust
let mut all_migrations = Vec::new();
all_migrations.extend(feature_todo::TodoFeature::migrations_static());
if config.finance.enabled {
    all_migrations.extend(feature_finance::FinanceFeature::migrations_static());
}
StoragePool::run_feature_migrations(pool.inner(), &all_migrations).await?;
```

**Step 4: Verify workspace compiles and tests pass**

Run: `cargo build --workspace && cargo nextest run --workspace`

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "refactor(agent): rewrite builder to use FeaturePackage pattern"
```

---

### Task 6.6: Update facade crate re-exports

**Files:**
- Modify: `src/lib.rs` (root klyntbot facade)
- Modify: `Cargo.toml` (root — add feature crate deps)

**Step 1: Add feature crate dependencies**

```toml
feature-todo.workspace = true
feature-finance.workspace = true
tools-core.workspace = true
```

**Step 2: Update re-exports**

```rust
pub use feature_todo;
pub use feature_finance;
pub use tools_core::{DynTool, FeaturePackage, Tool};
```

**Step 3: Verify and commit**

Run: `cargo build --workspace`

```bash
git add src/lib.rs Cargo.toml
git commit -m "refactor: update facade re-exports for feature crate architecture"
```

---

## Phase 7: Performance Optimizations

### Task 7.1: Add cursor-based pagination to TodoRepo

**Files:**
- Modify: `crates/feature-todo/src/storage/repo.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_list_with_pagination() {
    let pool = test_pool().await;
    let repo = TodoRepo::new(pool);
    // Create 5 todos
    for i in 0..5 {
        repo.add(&format!("Task {}", i), /* ... */).await.unwrap();
    }
    // Fetch page 1 (limit 2)
    let page1 = repo.list_paged(&TodoFilter::default(), 2, None).await.unwrap();
    assert_eq!(page1.items.len(), 2);
    assert!(page1.has_more);
    assert!(page1.cursor.is_some());
    // Fetch page 2
    let page2 = repo.list_paged(&TodoFilter::default(), 2, page1.cursor.as_deref()).await.unwrap();
    assert_eq!(page2.items.len(), 2);
}
```

**Step 2: Implement list_paged**

Add `list_paged` method to `TodoRepo` that uses `WHERE created_at < $cursor ORDER BY created_at DESC LIMIT $limit + 1` pattern.

**Step 3: Verify and commit**

```bash
git commit -m "feat(feature-todo): add cursor-based pagination to TodoRepo"
```

---

### Task 7.2: Add cursor-based pagination to Finance repos

Same pattern as Task 7.1 for `FinanceTransactionRepo`, `FinanceAccountRepo`, etc.

**Commit:** `feat(feature-finance): add cursor-based pagination to Finance repos`

---

### Task 7.3: Parallelize independent queries in action handlers

**Files:**
- Modify: `crates/feature-todo/src/tool/actions/list.rs`
- Modify: `crates/feature-finance/src/tool/reports.rs`

**Step 1: Identify sequential queries that can be parallelized**

Examples:
- `handle_show`: gets todo + attachments + time_entries + dependencies → `tokio::try_join!`
- `handle_report`: gets completed count + created count + time tracked → `tokio::try_join!`
- Finance `report_spending`: gets transactions + budgets → `tokio::try_join!`
- Finance `daily_review`: gets budgets + transactions + goals → `tokio::try_join!`

**Step 2: Replace sequential with parallel**

```rust
// Before
let todo = self.repo.get_or_err(id).await?;
let attachments = self.repo.list_attachments(id).await?;
let time_entries = self.repo.list_time_entries(id).await?;
let deps = self.repo.get_dependencies(id).await?;

// After
let (todo, attachments, time_entries, deps) = tokio::try_join!(
    self.repo.get_or_err(id),
    self.repo.list_attachments(id),
    self.repo.list_time_entries(id),
    self.repo.get_dependencies(id),
)?;
```

**Step 3: Verify and commit**

Run: `cargo nextest run --workspace`

```bash
git commit -m "perf: parallelize independent queries with tokio::try_join"
```

---

### Task 7.4: Add composite database indexes

**Files:**
- Create: `crates/feature-todo/migrations/002_add_indexes.sql`
- Create: `crates/feature-finance/migrations/002_add_indexes.sql`

**Step 1: Identify common query patterns and add indexes**

Todo:
```sql
-- Common filter: status + is_template (used by list, plan)
CREATE INDEX IF NOT EXISTS idx_todos_status_template ON todos (status, is_template) WHERE is_template = FALSE;

-- Common filter: focused tasks
CREATE INDEX IF NOT EXISTS idx_todos_focused ON todos (focused_at) WHERE focused_at IS NOT NULL;

-- Common filter: overdue tasks
CREATE INDEX IF NOT EXISTS idx_todos_due_date_status ON todos (due_date, status) WHERE due_date IS NOT NULL AND status != 'done';
```

Finance:
```sql
-- Common filter: transactions by account + date
CREATE INDEX IF NOT EXISTS idx_fin_tx_account_date ON finance_transactions (account_id, tx_date DESC);

-- Common filter: budget usage by category + period
CREATE INDEX IF NOT EXISTS idx_fin_budgets_category_active ON finance_budgets (category, is_active) WHERE is_active = TRUE;

-- Common filter: investments by portfolio
CREATE INDEX IF NOT EXISTS idx_fin_investments_portfolio ON finance_investments (portfolio_id);
```

**Step 2: Verify and commit**

```bash
git commit -m "perf: add composite indexes for common query patterns"
```

---

## Phase 8: Final Verification

### Task 8.1: Full workspace build and test

**Step 1: Clean build**

Run: `cargo clean && cargo build --workspace`
Expected: SUCCESS, 0 warnings

**Step 2: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Check formatting**

Run: `cargo fmt --all --check`
Expected: No formatting changes needed

**Step 5: Run doctests**

Run: `cargo test --workspace --doc`
Expected: All doctests PASS

---

### Task 8.2: Verify feature package DX

**Step 1: Document the "add a new feature" workflow**

Create a brief checklist in the design doc showing the steps to add a new feature package:

1. Create `crates/feature-<name>/` with Cargo.toml
2. Define types (with `#[derive(DomainEnum)]`)
3. Define config (serde)
4. Define storage (repo + rows + migrations)
5. Implement tool (with `#[tool_actions]` + `#[derive(ActionParams)]`)
6. Implement `FeaturePackage` trait
7. Add to workspace Cargo.toml
8. Add one line in agent builder

That's 8 steps, all in one crate (except the last two), compared to the current 10-15 steps across 5 crates.

**Step 2: Commit final state**

```bash
git add -A
git commit -m "feat: complete feature package architecture redesign

- tools-core crate with Tool, FeaturePackage, ToolRegistry, ParamExtractor
- tools-core-macros with DomainEnum, ActionParams, tool_actions macros
- feature-todo self-contained crate (types, config, storage, tool, handlers)
- feature-finance self-contained crate (types, config, storage, tool, handlers)
- Slimmed tools, storage, and config crates
- Agent builder uses FeaturePackage pattern
- Performance: cursor pagination, parallel queries, composite indexes
- Adding a new feature: 1 crate + 1 line in agent"
```

---

## Task Dependency Graph

```
Phase 1 (Foundation):
  1.1 → 1.2 → 1.3 → 1.4 → 1.5 → 1.6 → 1.7

Phase 2 (Macros) — depends on 1.7:
  2.1 → 2.2, 2.3 (parallel) → 2.4

Phase 3 (Storage) — depends on 1.5:
  3.1

Phase 4 (feature-todo) — depends on 2.4 + 3.1:
  4.1 → 4.2 → 4.3 → 4.4 → 4.5 → 4.6 → 4.7 → 4.8

Phase 5 (feature-finance) — depends on 2.4 + 3.1:
  5.1 → 5.2 → 5.3 → 5.4 → 5.5 → 5.6 → 5.7

Phase 6 (Cleanup) — depends on 4.8 + 5.7:
  6.1, 6.2, 6.3, 6.4 (parallel) → 6.5 → 6.6

Phase 7 (Performance) — depends on 6.6:
  7.1, 7.2 (parallel) → 7.3, 7.4 (parallel)

Phase 8 (Verification) — depends on all:
  8.1 → 8.2
```

**Phases 4 and 5 can run in parallel** if using separate worktrees or agents.

---

## Risk Mitigation

1. **Circular dependency risk**: The `tools` crate currently depends on `config`, `storage`, `bus`, `scheduling`, `calendar`, `goal`, `plan`. When splitting, ensure feature crates only depend on `tools-core` (not `tools`), and `tools-core` has minimal deps (only `common`).

2. **Migration ordering**: Feature migrations run after core migrations. The `_feature_migrations` table must exist before any feature migrations run. This is handled by the core migration in Task 3.1.

3. **Compile time regression**: Adding proc-macro crates can slow incremental builds. Mitigate by keeping macro crate small and using workspace-level caching.

4. **Test database requirement**: Storage tests need PostgreSQL. Ensure CI has pgvector. Feature crate tests that need DB should be gated behind a feature flag or test fixture.

5. **Re-export compatibility**: During the transition, both `tools::Tool` and `tools_core::Tool` exist. Use re-exports to avoid breaking consumers. Remove duplicates in Phase 6.
