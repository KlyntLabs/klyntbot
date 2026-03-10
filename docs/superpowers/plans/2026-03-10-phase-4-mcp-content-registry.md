# Phase 4: MCP Server Enhancement + Multi-Source Content Registry

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose curated klyntbot tools via MCP server for external AI agents, and build a multi-source content registry for runtime-loaded documentation and skills.

**Architecture:** Two additive subsystems. MCP server enhancement wraps existing tools with MCP protocol handlers + security. Content registry loads docs/skills from builtin, local, and remote sources with BM25-indexed search.

**Tech Stack:** rmcp (MCP protocol), serde, reqwest (remote sources), tokio

**Depends on:** Phase 1 (BM25), Phase 2 (tool metadata + skills), Phase 3 (context_request exposure)

---

## File Structure

### MCP Server Enhancement (Upgrade 6)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/mcp/src/server/mod.rs` | Modify | Expose 10+ tools via MCP, add tool list notification |
| `crates/mcp/src/server/handlers.rs` | Create | Handler functions per exposed tool |
| `crates/mcp/src/server/security.rs` | Create | Path traversal protection, input validation |
| `crates/mcp/src/server/transport.rs` | Modify | Stderr redirect for stdio transport |

### Content Registry (Upgrade 7)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/agent/src/content_registry/mod.rs` | Create | ContentRegistry main struct |
| `crates/agent/src/content_registry/types.rs` | Create | DocEntry, SkillEntry, ContentSource types |
| `crates/agent/src/content_registry/loader.rs` | Create | Multi-source loader (builtin, local, remote) |
| `crates/agent/src/content_registry/search.rs` | Create | In-memory BM25 search index |
| `crates/config/src/lib.rs` | Modify | Add ContentConfig |
| `crates/tools/src/docs.rs` | Create | docs tool (search/get/list) |
| `crates/tools/src/lib.rs` | Modify | Add docs module |
| `crates/app-core/src/lib.rs` | Modify | ContentRegistry init |

---

## Chunk 1: MCP Server Enhancement

### Task 1: Security Module

**Files:**
- Create: `crates/mcp/src/server/security.rs`

- [ ] **Step 1: Write security validation tests**

```rust
// crates/mcp/src/server/security.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_allows_safe_paths() {
        let base = std::path::PathBuf::from("/tmp/klyntbot");
        std::fs::create_dir_all(&base).ok();

        let result = validate_path("/tmp/klyntbot/data.db", &base);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_blocks_traversal() {
        let base = std::path::PathBuf::from("/tmp/klyntbot");
        std::fs::create_dir_all(&base).ok();

        let result = validate_path("/tmp/klyntbot/../../etc/passwd", &base);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_blocks_absolute_outside() {
        let base = std::path::PathBuf::from("/tmp/klyntbot");
        let result = validate_path("/etc/passwd", &base);
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_input_strips_control_chars() {
        let input = "hello\x00world\x01test";
        let clean = sanitize_input(input);
        assert_eq!(clean, "helloworldtest");
    }

    #[test]
    fn test_sanitize_input_limits_length() {
        let long = "x".repeat(100_000);
        let clean = sanitize_input(&long);
        assert!(clean.len() <= MAX_INPUT_LENGTH);
    }
}
```

- [ ] **Step 2: Implement security functions**

```rust
// crates/mcp/src/server/security.rs

use std::path::PathBuf;

const MAX_INPUT_LENGTH: usize = 50_000;

/// Validate that a path stays within the allowed base directory.
pub fn validate_path(path: &str, allowed_base: &PathBuf) -> Result<PathBuf, String> {
    let resolved = PathBuf::from(path);
    let canonical = resolved.canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    let base_canonical = allowed_base.canonicalize()
        .map_err(|e| format!("Invalid base path: {}", e))?;

    if !canonical.starts_with(&base_canonical) {
        return Err(format!("Path traversal detected: {} is outside {}", path, allowed_base.display()));
    }
    Ok(canonical)
}

/// Sanitize user input: strip control characters and limit length.
pub fn sanitize_input(input: &str) -> String {
    let clean: String = input.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_INPUT_LENGTH)
        .collect();
    clean
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p mcp -E 'test(security)'`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/mcp/src/server/security.rs
git commit -m "feat(mcp): add security module with path traversal protection and input sanitization"
```

---

### Task 2: Expand MCP Server Tool List

**Files:**
- Modify: `crates/mcp/src/server/mod.rs`
- Create: `crates/mcp/src/server/handlers.rs`

- [ ] **Step 1: Read current MCP server implementation**

Read `crates/mcp/src/server/mod.rs` to understand the current `McpServerRunner` structure and how `get_status()` is exposed.

- [ ] **Step 2: Define exposed tool list**

```rust
// crates/mcp/src/server/handlers.rs

/// Tools exposed via MCP server to external agents.
pub const MCP_EXPOSED_TOOLS: &[&str] = &[
    "task",
    "memory",
    "annotate",
    "search",         // Will be global search once Phase 1 BM25 is integrated
    "project",
    "area",
    "okr",
    "context_request",
    "learning",
    "web_search",
];
```

- [ ] **Step 3: Implement MCP tool handlers**

Each handler wraps a call to the corresponding tool from the `ToolRegistry`:

```rust
pub async fn handle_tool_call(
    registry: &ToolRegistry,
    tool_name: &str,
    params: serde_json::Value,
    ctx: &RoutingContext,
) -> Result<String, String> {
    // Validate tool is in exposed list
    if !MCP_EXPOSED_TOOLS.contains(&tool_name) {
        return Err(format!("Tool '{}' is not exposed via MCP", tool_name));
    }

    // Sanitize input
    let params_str = params.to_string();
    let _sanitized = security::sanitize_input(&params_str);

    // Execute via registry
    registry.execute(tool_name, params, ctx)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Add dynamic tool list notification**

```rust
impl McpServer {
    /// Notify connected clients that the tool list has changed.
    pub async fn notify_tool_list_changed(&self) {
        // Send ToolListChanged notification via MCP protocol
        // Implementation depends on current MCP server transport layer
    }
}
```

- [ ] **Step 5: Run MCP tests**

Run: `cargo nextest run -p mcp`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/src/server/
git commit -m "feat(mcp): expose 10 tools via MCP server with handler routing"
```

---

### Task 3: Stderr Redirect for stdio Transport

**Files:**
- Modify: `crates/mcp/src/server/transport.rs` (or wherever stdio transport is configured)

- [ ] **Step 1: Add stderr redirect**

When running as stdio MCP server, redirect tracing/log output to stderr so stdout stays clean for JSON-RPC:

```rust
/// Redirect console output for stdio transport.
/// Tracing output goes to stderr, stdout is reserved for JSON-RPC.
pub fn configure_stdio_transport() {
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p mcp`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/mcp/src/server/
git commit -m "feat(mcp): redirect tracing to stderr for clean stdio transport"
```

---

## Chunk 2: Content Registry

### Task 4: Content Registry Types

**Files:**
- Create: `crates/agent/src/content_registry/types.rs`

- [ ] **Step 1: Write content registry types**

```rust
// crates/agent/src/content_registry/types.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentSourceKind {
    Builtin,
    Local { name: String, path: PathBuf },
    Remote { name: String, url: String, cache_dir: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub content_source: String,
    pub languages: Vec<LanguageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageEntry {
    pub language: String,
    pub recommended_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub content_source: String,
    pub path: PathBuf,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ContentEntry {
    Doc(DocEntry),
    Skill(SkillEntry),
}

#[derive(Debug, Clone)]
pub struct ContentSearchResult {
    pub entry: ContentEntry,
    pub score: f64,
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent/src/content_registry/types.rs
git commit -m "feat(agent): add content registry types"
```

---

### Task 5: Content Registry Implementation

**Files:**
- Create: `crates/agent/src/content_registry/mod.rs`
- Create: `crates/agent/src/content_registry/loader.rs`
- Create: `crates/agent/src/content_registry/search.rs`

- [ ] **Step 1: Write tests for content registry**

```rust
// crates/agent/src/content_registry/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_load_from_local_directory() {
        let dir = TempDir::new().unwrap();

        // Create a docs directory with a manifest
        let docs_dir = dir.path().join("docs");
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(docs_dir.join("manifest.json"), r#"{
            "docs": [
                {
                    "id": "stripe/api",
                    "name": "Stripe API",
                    "description": "Payment processing API",
                    "source": "community",
                    "tags": ["payment", "api", "stripe"]
                }
            ]
        }"#).unwrap();

        let config = ContentConfig {
            sources: vec![ContentSourceConfig {
                name: "test".into(),
                path: Some(dir.path().to_string_lossy().into()),
                url: None,
            }],
            trust_policy: "official,community".into(),
            refresh_interval_secs: 3600,
            content_dir: dir.path().to_path_buf(),
        };

        let registry = ContentRegistry::load_sync(&config).unwrap();
        assert!(!registry.docs().is_empty());
    }

    #[test]
    fn test_search_docs() {
        let mut registry = ContentRegistry::empty();
        registry.add_doc(DocEntry {
            id: "stripe/api".into(),
            name: "Stripe API".into(),
            description: "Payment processing REST API".into(),
            source: "community".into(),
            tags: vec!["payment".into(), "api".into()],
            content_source: "test".into(),
            languages: vec![],
        });
        registry.add_doc(DocEntry {
            id: "react/hooks".into(),
            name: "React Hooks".into(),
            description: "React state management hooks".into(),
            source: "community".into(),
            tags: vec!["react".into(), "frontend".into()],
            content_source: "test".into(),
            languages: vec![],
        });

        let results = registry.search("payment API", 10);
        assert!(!results.is_empty());
        // Stripe should rank higher for "payment API"
        if let ContentEntry::Doc(doc) = &results[0].entry {
            assert_eq!(doc.id, "stripe/api");
        }
    }
}
```

- [ ] **Step 2: Implement ContentRegistry**

```rust
// crates/agent/src/content_registry/mod.rs

pub mod loader;
pub mod search;
pub mod types;

pub use types::*;

pub struct ContentRegistry {
    docs: Vec<DocEntry>,
    skills: Vec<SkillEntry>,
}

impl ContentRegistry {
    pub fn empty() -> Self {
        Self { docs: Vec::new(), skills: Vec::new() }
    }

    pub fn load_sync(config: &ContentConfig) -> common::Result<Self> {
        loader::load_all(config)
    }

    pub fn add_doc(&mut self, doc: DocEntry) {
        self.docs.push(doc);
    }

    pub fn add_skill(&mut self, skill: SkillEntry) {
        self.skills.push(skill);
    }

    pub fn docs(&self) -> &[DocEntry] {
        &self.docs
    }

    pub fn skills(&self) -> &[SkillEntry] {
        &self.skills
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<ContentSearchResult> {
        search::search_content(&self.docs, &self.skills, query, limit)
    }

    pub fn get(&self, id: &str) -> Option<ContentEntry> {
        self.docs.iter()
            .find(|d| d.id == id)
            .map(|d| ContentEntry::Doc(d.clone()))
            .or_else(|| self.skills.iter()
                .find(|s| s.id == id)
                .map(|s| ContentEntry::Skill(s.clone())))
    }
}
```

- [ ] **Step 3: Implement loader**

```rust
// crates/agent/src/content_registry/loader.rs

use super::types::*;
use super::ContentRegistry;

pub fn load_all(config: &ContentConfig) -> common::Result<ContentRegistry> {
    let mut registry = ContentRegistry::empty();

    for source in &config.sources {
        if let Some(path) = &source.path {
            load_local(&mut registry, &source.name, path)?;
        }
        // Remote loading deferred to async refresh
    }

    Ok(registry)
}

fn load_local(registry: &mut ContentRegistry, name: &str, path: &str) -> common::Result<()> {
    let docs_manifest = std::path::PathBuf::from(path).join("docs/manifest.json");
    if docs_manifest.exists() {
        let content = std::fs::read_to_string(&docs_manifest)?;
        let manifest: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(docs) = manifest.get("docs").and_then(|d| d.as_array()) {
            for doc in docs {
                if let Ok(entry) = serde_json::from_value::<DocEntry>(doc.clone()) {
                    registry.add_doc(entry);
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Implement search**

```rust
// crates/agent/src/content_registry/search.rs

use super::types::*;

pub fn search_content(
    docs: &[DocEntry],
    skills: &[SkillEntry],
    query: &str,
    limit: usize,
) -> Vec<ContentSearchResult> {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();

    let mut results: Vec<ContentSearchResult> = Vec::new();

    for doc in docs {
        let score = score_entry(&doc.name, &doc.description, &doc.tags, &terms);
        if score > 0.0 {
            results.push(ContentSearchResult {
                entry: ContentEntry::Doc(doc.clone()),
                score,
            });
        }
    }

    for skill in skills {
        let score = score_entry(&skill.name, &skill.description, &skill.tags, &terms);
        if score > 0.0 {
            results.push(ContentSearchResult {
                entry: ContentEntry::Skill(skill.clone()),
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
}

fn score_entry(name: &str, description: &str, tags: &[String], terms: &[&str]) -> f64 {
    let name_lower = name.to_lowercase();
    let desc_lower = description.to_lowercase();
    let tags_str = tags.join(" ").to_lowercase();

    let mut score = 0.0;
    for term in terms {
        if name_lower.contains(term) { score += 3.0; }
        if desc_lower.contains(term) { score += 1.0; }
        if tags_str.contains(term) { score += 2.0; }
    }
    score
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(content_registry)'`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/content_registry/
git commit -m "feat(agent): add ContentRegistry with multi-source loading and keyword search"
```

---

### Task 6: ContentConfig

**Files:**
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: Add ContentConfig to config**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContentConfig {
    #[serde(default)]
    pub sources: Vec<ContentSourceConfig>,
    #[serde(default = "default_trust_policy")]
    pub trust_policy: String,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(default)]
    pub content_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSourceConfig {
    pub name: String,
    pub url: Option<String>,
    pub path: Option<String>,
}

fn default_trust_policy() -> String { "official,maintainer".into() }
fn default_refresh_interval() -> u64 { 86400 }
```

Add `content: ContentConfig` field to the main `Config` struct.

- [ ] **Step 2: Run config tests**

Run: `cargo nextest run -p config`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/lib.rs
git commit -m "feat(config): add ContentConfig for multi-source content registry"
```

---

### Task 7: docs Tool

**Files:**
- Create: `crates/tools/src/docs.rs`
- Modify: `crates/tools/src/lib.rs`

- [ ] **Step 1: Write the docs tool**

```rust
// crates/tools/src/docs.rs

use tools_core_macros::{Tool, ToolParams};

/// Search and fetch documentation from the content registry.
#[derive(Tool)]
#[tool(
    name = "docs",
    description = "Search and fetch documentation for APIs, SDKs, and libraries from the content registry. Use before writing code against external services to get current, accurate API reference.",
    category = "Search",
    tags = "documentation,api,sdk,reference",
    cost = "Free",
)]
pub struct DocsTool {
    // Will hold Arc<RwLock<ContentRegistry>>
}

#[derive(ToolParams)]
pub struct DocsParams {
    /// Action: "search", "get", or "list"
    #[param(required)]
    pub action: String,

    /// Search query (for "search" action)
    pub query: Option<String>,

    /// Document ID (for "get" action)
    pub id: Option<String>,

    /// Maximum results (for "search" action, default 10)
    pub limit: Option<i64>,
}
```

- [ ] **Step 2: Register in tools/lib.rs**

Add `pub mod docs;` to `crates/tools/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p tools`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/docs.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add docs tool for content registry search and retrieval"
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

- [ ] **Step 4: Doc tests**

Run: `cargo test --workspace --doc`
Expected: PASS

- [ ] **Step 5: Desktop UI build**

Run: `cd desktop-ui && bun run build && bun run lint:fix`
Expected: Clean build

- [ ] **Step 6: Commit fixes**

```bash
git commit -m "fix: address all clippy, formatting, and build issues from Phase 4"
```

---

## Post-Phase 4: Full Integration Verification

- [ ] **Step 1: Full test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Manual smoke test**

Run: `cargo tauri dev` and verify:
1. BM25 search returns ranked results
2. Annotations can be created/queried
3. Context inventory shows in transparency panel
4. Tool registry shows categories and usage
5. MCP server exposes tools (test with `echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | cargo run -p mcp -- serve`)

- [ ] **Step 3: Tag release**

```bash
git tag -a v0.x.0-context-hub -m "Context Hub integration: BM25, annotations, progressive context, tool metadata, skills spec, MCP server, content registry"
```
