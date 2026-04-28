# Computer-Use Phase 2 — Tool Surface and Cloud Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the `feature-computer-use` crate with a `ComputerUseTool`, extend the providers crate so Anthropic can carry image-bearing tool results and the `computer_20251124` tool block, ship the orchestrator skill, and prove an end-to-end "agent screenshots its own screen" path with a mock-driven integration test.

**Architecture:** A new L4 crate (`feature-computer-use`) wraps the Phase 1 `PlatformInput`/`PlatformCapture` traits and exposes them to the agent via `#[derive(Tool)]` + `#[tool_actions]`. The providers crate gains `ContentPart::ImageData`, a `ToolResultContent` enum (replacing `Message::Tool.content: String`), `ProviderCapabilities::{computer_use, computer_use_version}`, and an Anthropic adapter that emits the `computer_20251124` tool block plus the `anthropic-beta: computer-use-2025-11-24` header. A sentinel-JSON wrapper (`klynt_tool_result_multipart`) lets a tool's `Result<String>` carry image data without changing the `Tool::execute` signature. `RoutingContext` gains a `screenshot_tx` sidecar channel (mirrors `entity_tx`) for future HUD consumers.

**Tech Stack:** Rust 2024, `tools-core` derive macros, `serde_json`, `base64`, Anthropic native API, existing `MockInput`/`MockCapture` from Phase 1, `cargo nextest`, `chrono`/`jiff` for timestamps.

---

## Phase 2 Scope Boundary

Phase 2 explicitly **DOES** include:

- A working `ComputerUseTool` that exposes every action variant from Phase 1's `ComputerUseAction` enum.
- Anthropic adapter changes for image-bearing tool results, the `computer_20251124` tool block, and the beta header.
- Image-aware `MidLoopCompressor` exception (latest screenshot preserved, older multipart variants dropped with a placeholder).
- `RoutingContext::screenshot_tx` sidecar channel + `AgentEvent::ScreenshotCaptured` event (no consumer wired; Phase 3 adds HUD).
- New orchestrator skill at `skills/computer-use/`.
- Mock-driven integration test proving the full pipeline: chat → agent → tool → screenshot → multipart `Message::Tool` → next iteration receives image.

Phase 2 explicitly **DOES NOT** include:

- Risk-tier classifier, scope locks, sensitive-surface patterns (Phase 3).
- HUD window, cursor overlay, action callouts, voice narration (Phase 3).
- Emergency-stop hotkey, `agent_action_log` table, screenshot blob storage (Phase 3).
- `Tool::execution_constraints()` / serial-only invariant (Phase 3 — does not exist on the trait today).
- Hybrid perception cascade, local VLM, AX-tree-first router (Phase 4).
- OpenAI / Gemini computer-use translations (deferred — Anthropic-only path in v1).
- Browser-control (Phase 5), procedural memory (Phase 6), reforge integration (Phase 7).
- Real screenshot downsampling for old multipart variants — we drop with placeholder text in Phase 2 (real downsample is a Phase 4 polish step).

---

## File Structure

### New crate: `crates/feature-computer-use/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace member, depends on `tools-core`, `platform-input`, `platform-capture`, `providers` (for `ContentPart`), `common`, `serde`, `serde_json`, `base64`, `async-trait`, `tracing`. |
| `src/lib.rs` | `ComputerUseFeature` (`FeaturePackage` impl), `ComputerUseToolDeps`, re-exports. |
| `src/tool/mod.rs` | `ComputerUseTool` struct + `#[tool_actions]` impl block dispatching every action variant. |
| `src/tool/actions.rs` | `ActionParams` structs (`ScreenshotParams`, `LeftClickParams`, `TypeParams`, etc.) — one per action variant. |
| `src/tool/result.rs` | `multipart_payload(parts)` → sentinel JSON string; `text_summary(...)` helper; `MULTIPART_SENTINEL_KEY` constant. |
| `tests/tool_smoke.rs` | Headless tests using `MockInput`/`MockCapture` to exercise every action method. |

### New skill: `skills/computer-use/`

| File | Responsibility |
|---|---|
| `SKILL.md` | Orchestrator-style skill body. YAML frontmatter (`name`, `description`, `whenToUse`, `metadata.klyntbot.{tools, mcp_tools, triggers}`). |
| `references/action-vocabulary.md` | The 16-action vocabulary reference (demand-loaded via `skill_reference` tool). |

### New config module

| File | Responsibility |
|---|---|
| `crates/config/src/schema/computer_use.rs` | `ComputerUseConfig` struct (`providers.cloud` field; other tier configs deferred to Phase 4). |

### Modified existing files

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `crates/feature-computer-use` to `members`. |
| `crates/providers/src/types.rs` | Add `ContentPart::ImageData`, `ToolResultContent` enum, change `Message::Tool.content: String` → `content: ToolResultContent`, add `ProviderCapabilities::{computer_use, computer_use_version}`. |
| `crates/providers/src/adapters/anthropic_native.rs` | Add `anthropic-beta` header, `ContentPart::ImageData` serialization, multipart `Message::Tool` serialization, `convert_tools` special case for `"type": "computer_use"`, update `capabilities()` literal. |
| `crates/agent/src/execution/core.rs` | Replace `sanitize_tool_result(&str) -> String` with `process_tool_result(&str) -> ToolResultContent`; add `screenshot_tx`/`screenshot_rx` channel init + drain; emit `AgentEvent::ScreenshotCaptured`. |
| `crates/agent/src/execution/mid_loop_compressor.rs` | Image-aware branch: latest multipart `Message::Tool` preserved verbatim; older multiparts replaced with `ToolResultContent::Text("[older screenshot dropped to save tokens]")`. |
| `crates/agent/src/events.rs` | Add `AgentEvent::ScreenshotCaptured(ScreenshotEvent)` variant. |
| `crates/tools-core/src/routing.rs` | Add `pub screenshot_tx: Option<mpsc::Sender<common::ScreenshotEvent>>` field. |
| `crates/common/src/lib.rs` (or wherever shared types live) | Add `ScreenshotEvent` struct. |
| `crates/skill-system/src/defaults.rs` | Add `computer-use` block to `compiled_skill_defaults()`. |
| `crates/config/src/schema/mod.rs` | Add `mod computer_use;` + `pub use self::computer_use::*;`. |
| `crates/config/src/schema/core.rs` | Add `computer_use: ComputerUseConfig` field to `Config`. |
| `crates/app-core/src/state.rs` | Add `platform_input` and `platform_capture` `Option<Arc<dyn …>>` fields to `AppCore`. |
| `crates/app-core/src/init/mod.rs` | Construct `MacInput`/`MacCapture` singletons (with `MockInput`/`MockCapture` fallback off-macOS); register `ComputerUseTool` after `LauncherTool` block. |
| `crates/app-core/Cargo.toml` | Add `platform-input`, `platform-capture`, `feature-computer-use` deps. |
| `crates/agent/src/agent_loop/builder.rs` | (only if `RoutingContext` defaults need updating to include `screenshot_tx: None`). |

### Test files

| File | Responsibility |
|---|---|
| `crates/feature-computer-use/tests/tool_smoke.rs` | Mock-driven action smoke tests (one per action method). |
| `crates/agent/tests/screenshot_pipeline.rs` | Integration test: agent loop with mocked tool returning sentinel-multipart → assert `Message::Tool.content` becomes `Multipart(...)` and `AgentEvent::ScreenshotCaptured` fires. |
| `crates/agent/tests/midloop_compressor_image_aware.rs` | Image-aware compressor test: 3 screenshots over a long history → newest preserved, older two dropped with placeholder. |
| `crates/providers/tests/anthropic_computer_use.rs` | Golden test: `convert_tools` for a computer-use tool produces the exact `computer_20251124` JSON block; `Message::Tool` with `Multipart` serializes to the documented Anthropic shape. |

---

## Tasks

### Task 1: Add `ContentPart::ImageData` variant

**Files:**
- Modify: `crates/providers/src/types.rs:406-417`
- Test: `crates/providers/src/types.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing test for new variant serialization**

Append at the bottom of `crates/providers/src/types.rs`:

```rust
#[cfg(test)]
mod content_part_tests {
    use super::*;

    #[test]
    fn image_data_serializes_with_snake_case_tag() {
        let part = ContentPart::ImageData {
            media_type: "image/png".to_string(),
            data: "AAAA".to_string(),
        };
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "image_data");
        assert_eq!(json["media_type"], "image/png");
        assert_eq!(json["data"], "AAAA");
    }

    #[test]
    fn image_data_round_trips() {
        let part = ContentPart::ImageData {
            media_type: "image/jpeg".to_string(),
            data: "ZGVhZGJlZWY=".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        match back {
            ContentPart::ImageData { media_type, data } => {
                assert_eq!(media_type, "image/jpeg");
                assert_eq!(data, "ZGVhZGJlZWY=");
            }
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p providers content_part_tests`
Expected: FAIL with "no variant named `ImageData`".

- [ ] **Step 3: Add the variant**

In `crates/providers/src/types.rs`, replace the existing `ContentPart` enum with:

```rust
/// Content part for multipart messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    /// Base64-encoded image, sent inline. Used for tool-returned screenshots.
    ImageData { media_type: String, data: String },
}
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p providers content_part_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/types.rs
git commit -m "feat(providers): add ContentPart::ImageData variant for inline base64 images"
```

---

### Task 2: Add `ToolResultContent` enum

**Files:**
- Modify: `crates/providers/src/types.rs` (add new enum, update `Message::Tool` later in Task 3)
- Test: inline.

- [ ] **Step 1: Write failing test**

Append to `crates/providers/src/types.rs`:

```rust
#[cfg(test)]
mod tool_result_content_tests {
    use super::*;

    #[test]
    fn text_serializes_as_bare_string() {
        let trc = ToolResultContent::Text("hello".to_string());
        let json = serde_json::to_value(&trc).unwrap();
        assert_eq!(json, serde_json::json!("hello"));
    }

    #[test]
    fn multipart_serializes_as_array() {
        let trc = ToolResultContent::Multipart(vec![
            ContentPart::Text { text: "screenshot".to_string() },
            ContentPart::ImageData {
                media_type: "image/png".to_string(),
                data: "AAAA".to_string(),
            },
        ]);
        let json = serde_json::to_value(&trc).unwrap();
        assert!(json.is_array(), "expected array, got {json}");
        assert_eq!(json.as_array().unwrap().len(), 2);
    }

    #[test]
    fn from_string_yields_text() {
        let trc: ToolResultContent = "hi".to_string().into();
        match trc {
            ToolResultContent::Text(s) => assert_eq!(s, "hi"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn round_trips_text() {
        let trc = ToolResultContent::Text("plain result".to_string());
        let json = serde_json::to_string(&trc).unwrap();
        let back: ToolResultContent = serde_json::from_str(&json).unwrap();
        matches!(back, ToolResultContent::Text(_));
    }

    #[test]
    fn round_trips_multipart() {
        let trc = ToolResultContent::Multipart(vec![ContentPart::Text {
            text: "x".to_string(),
        }]);
        let json = serde_json::to_string(&trc).unwrap();
        let back: ToolResultContent = serde_json::from_str(&json).unwrap();
        matches!(back, ToolResultContent::Multipart(_));
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p providers tool_result_content_tests`
Expected: FAIL — `ToolResultContent` does not exist.

- [ ] **Step 3: Add the enum + impls**

In `crates/providers/src/types.rs`, add immediately after the `ContentPart`/`ImageUrl` block:

```rust
/// Content of a tool result message — either plain text or a multipart vector
/// (text + images). Constructed via `From<String>` for backwards compatibility
/// with existing tools, or built explicitly by tools that return images.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Multipart(Vec<ContentPart>),
}

impl From<String> for ToolResultContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for ToolResultContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl ToolResultContent {
    /// Returns true if any part is an image.
    pub fn has_image(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Multipart(parts) => parts
                .iter()
                .any(|p| matches!(p, ContentPart::ImageData { .. } | ContentPart::ImageUrl { .. })),
        }
    }

    /// Returns a short text representation suitable for logs/compression summaries.
    pub fn as_text_preview(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Multipart(parts) => {
                let text: Vec<&str> = parts
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if text.is_empty() {
                    "[multipart tool result, no text]".to_string()
                } else {
                    text.join(" ")
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p providers tool_result_content_tests`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/types.rs
git commit -m "feat(providers): add ToolResultContent enum (Text|Multipart) with From<String>"
```

---

### Task 3: Refactor `Message::Tool.content` to `ToolResultContent`

**Files:**
- Modify: `crates/providers/src/types.rs:368-372` (Tool variant) and `464-475` (constructor)
- Modify: every call site of `Message::tool(...)` and pattern matches on `Message::Tool { content, .. }` across the workspace.
- Test: inline.

- [ ] **Step 1: Find all call sites**

Run: `rg -n 'Message::Tool|Message::tool\(' crates/ tests/ | head -100`
Expected: locations include constructions and pattern matches in `agent`, `providers`, `mcp`. Note them down for Step 4.

- [ ] **Step 2: Write failing test**

Append to `crates/providers/src/types.rs`:

```rust
#[cfg(test)]
mod message_tool_content_tests {
    use super::*;

    #[test]
    fn tool_constructor_accepts_string() {
        let m = Message::tool("call_1", "task", "ok".to_string());
        match m {
            Message::Tool { content: ToolResultContent::Text(s), .. } => {
                assert_eq!(s, "ok");
            }
            _ => panic!("wrong shape"),
        }
    }

    #[test]
    fn tool_constructor_accepts_multipart() {
        let parts = vec![
            ContentPart::Text { text: "took screenshot".to_string() },
            ContentPart::ImageData {
                media_type: "image/png".to_string(),
                data: "AAAA".to_string(),
            },
        ];
        let m = Message::tool(
            "call_1",
            "computer_use",
            ToolResultContent::Multipart(parts),
        );
        match m {
            Message::Tool { content: ToolResultContent::Multipart(p), .. } => {
                assert_eq!(p.len(), 2);
            }
            _ => panic!("wrong shape"),
        }
    }
}
```

- [ ] **Step 3: Run failing test**

Run: `cargo nextest run -p providers message_tool_content_tests`
Expected: FAIL — `Message::tool(... String)` may still compile due to `Into`, but the pattern match on `ToolResultContent::Text` fails because `content` is still `String`.

- [ ] **Step 4: Update the variant + constructor**

In `crates/providers/src/types.rs`, change the `Message::Tool` variant (around line 368):

```rust
    Tool {
        tool_call_id: String,
        name: String,
        content: ToolResultContent,
    },
```

And the constructor (around line 464):

```rust
pub fn tool(
    tool_call_id: impl Into<String>,
    name: impl Into<String>,
    content: impl Into<ToolResultContent>,
) -> Self {
    Self::Tool {
        tool_call_id: tool_call_id.into(),
        name: name.into(),
        content: content.into(),
    }
}
```

- [ ] **Step 5: Fix call sites**

For each call site noted in Step 1 that pattern-matches `Message::Tool { content, .. }` and uses `content` as `&str` / `String`, change to `content.as_text_preview()` (read-only) or destructure as `content: ToolResultContent`. Likely sites:

- `crates/agent/src/execution/core.rs` (around the `messages.push(Message::tool(...))` site at line 713 and `MidLoopCompressor` consumers).
- `crates/agent/src/execution/mid_loop_compressor.rs` (line 82 `if let Message::Tool { content, name, .. } = msg`).
- `crates/agent/src/agent_loop/streaming.rs` and `crates/agent/src/agent_loop/synthesis.rs` if they read `Message::Tool` content.
- `crates/mcp/` if any.

For each: where the code does `&str` ops on `content`, replace with:

```rust
let text = content.as_text_preview();
// then operate on `text`
```

Where the code mutates `*content = "...".to_string();`, replace with:
```rust
*content = ToolResultContent::Text("...".to_string());
```

Pattern matches on `content: String` change to `content: ToolResultContent`.

- [ ] **Step 6: Run new tests + workspace build**

Run: `cargo nextest run -p providers message_tool_content_tests`
Expected: PASS.

Run: `cargo build --workspace`
Expected: success — every call site updated.

- [ ] **Step 7: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: all existing tests pass (call-site changes are mechanical and behavior-preserving).

- [ ] **Step 8: Commit**

```bash
git add -u
git commit -m "refactor(providers): change Message::Tool.content to ToolResultContent

Existing call sites use the From<String> impl so the constructor signature
is preserved. Pattern matches updated to handle the new enum."
```

---

### Task 4: Add `ProviderCapabilities::{computer_use, computer_use_version}`

**Files:**
- Modify: `crates/providers/src/types.rs:304-329`
- Test: inline.

- [ ] **Step 1: Write failing test**

Append:

```rust
#[cfg(test)]
mod capabilities_computer_use_tests {
    use super::*;

    #[test]
    fn default_has_no_computer_use() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.computer_use);
        assert!(caps.computer_use_version.is_none());
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p providers capabilities_computer_use`
Expected: FAIL — fields do not exist.

- [ ] **Step 3: Add the fields**

In `crates/providers/src/types.rs`, update `ProviderCapabilities`:

```rust
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub extended_thinking: bool,
    pub structured_outputs: bool,
    pub prompt_caching: bool,
    pub native_token_counting: bool,
    pub vision: bool,
    pub streaming: bool,
    pub tool_choice_required: bool,
    pub parallel_tool_calls: bool,
    /// Whether the provider supports the `computer_use` tool block.
    pub computer_use: bool,
    /// e.g. `Some("computer_20251124")`. None if `computer_use` is false.
    pub computer_use_version: Option<String>,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            extended_thinking: false,
            structured_outputs: false,
            prompt_caching: false,
            native_token_counting: false,
            vision: true,
            streaming: true,
            tool_choice_required: false,
            parallel_tool_calls: true,
            computer_use: false,
            computer_use_version: None,
        }
    }
}
```

- [ ] **Step 4: Fix all hand-built `ProviderCapabilities { ... }` literals**

Run: `rg -n 'ProviderCapabilities\s*\{' crates/`. For each location (notably `anthropic_native.rs::capabilities`, `openai_compat.rs::capabilities`, `noop.rs`), add the two new fields. For Anthropic, set `computer_use: true, computer_use_version: Some("computer_20251124".to_string())`. For all others: `false` / `None`.

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p providers capabilities_computer_use`
Expected: PASS.

- [ ] **Step 6: Run workspace build**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "feat(providers): add computer_use + computer_use_version to ProviderCapabilities"
```

---

### Task 5: Anthropic adapter — `ContentPart::ImageData` serialization

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (around lines 100–125 where `ContentPart` is converted)
- Test: golden test.

- [ ] **Step 1: Read existing serialization**

Run: `rg -n 'ContentPart::ImageUrl' crates/providers/src/adapters/anthropic_native.rs`
Note the exact closure/match arm so the new arm slots in cleanly.

- [ ] **Step 2: Write failing golden test**

Create `crates/providers/tests/anthropic_image_data.rs`:

```rust
use providers::adapters::anthropic_native::AnthropicProvider;
use providers::types::{ContentPart, ImageUrl, Message, ToolResultContent, UserContent};
use serde_json::json;

#[test]
fn user_message_with_image_data_serializes_to_base64_block() {
    let provider = AnthropicProvider::for_test();
    let parts = vec![
        ContentPart::Text { text: "what is this?".to_string() },
        ContentPart::ImageData {
            media_type: "image/png".to_string(),
            data: "AAAA".to_string(),
        },
    ];
    let m = Message::User { content: UserContent::Parts(parts) };
    let serialized = provider.serialize_message_for_test(&m).unwrap();
    let user = serialized;
    let content = user["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["type"], "base64");
    assert_eq!(content[1]["source"]["media_type"], "image/png");
    assert_eq!(content[1]["source"]["data"], "AAAA");
}
```

> **Note:** `AnthropicProvider::for_test()` and `serialize_message_for_test(...)` are test-only constructors. If they don't exist, add them in this task as `#[cfg(test)]` items inside `anthropic_native.rs`. Their bodies just construct a default-config `AnthropicProvider` and call into the existing private message serializer. If the existing serializer is private, expose it under `#[cfg(test)] pub`.

- [ ] **Step 3: Run failing test**

Run: `cargo nextest run -p providers --test anthropic_image_data`
Expected: FAIL.

- [ ] **Step 4: Add the serialization arm**

Locate the `match part { ContentPart::Text { .. } => ..., ContentPart::ImageUrl { .. } => ..., }` block in `anthropic_native.rs` (~line 110) and add the new arm:

```rust
crate::types::ContentPart::ImageData { media_type, data } => {
    json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media_type,
            "data": data,
        }
    })
}
```

If the existing match is exhaustive without a wildcard arm, this is sufficient. If a wildcard arm exists, replace it with the explicit arm.

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p providers --test anthropic_image_data`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(providers/anthropic): serialize ContentPart::ImageData as base64 image source"
```

---

### Task 6: Anthropic adapter — multipart `Message::Tool` serialization

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (the `Message::Tool` serialization path)
- Test: golden.

- [ ] **Step 1: Locate the Tool message serialization**

Run: `rg -n 'Message::Tool' crates/providers/src/adapters/anthropic_native.rs`
Expected: a match arm or branch that produces the `tool_result` Anthropic block from `tool_call_id`/`name`/`content`. Today it serializes `content: String` → `content: "<text>"`. We need to handle both `Text` and `Multipart`.

- [ ] **Step 2: Write failing golden test**

Add to `crates/providers/tests/anthropic_image_data.rs`:

```rust
use providers::types::ToolResultContent;

#[test]
fn tool_message_with_multipart_serializes_to_array_content() {
    let provider = AnthropicProvider::for_test();
    let m = Message::Tool {
        tool_call_id: "toolu_abc".to_string(),
        name: "computer_use".to_string(),
        content: ToolResultContent::Multipart(vec![
            ContentPart::Text { text: "screenshot done".to_string() },
            ContentPart::ImageData {
                media_type: "image/png".to_string(),
                data: "ZGVhZGJlZWY=".to_string(),
            },
        ]),
    };
    let serialized = provider.serialize_message_for_test(&m).unwrap();
    // Anthropic's tool_result with multipart content has content as an array.
    let content = &serialized["content"];
    assert!(content.is_array(), "expected array, got {content}");
    let arr = content.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[1]["type"], "image");
    assert_eq!(arr[1]["source"]["type"], "base64");
}

#[test]
fn tool_message_with_text_serializes_to_string_content() {
    let provider = AnthropicProvider::for_test();
    let m = Message::Tool {
        tool_call_id: "toolu_abc".to_string(),
        name: "echo".to_string(),
        content: ToolResultContent::Text("ok".to_string()),
    };
    let serialized = provider.serialize_message_for_test(&m).unwrap();
    assert_eq!(serialized["content"], "ok");
}
```

- [ ] **Step 3: Run failing tests**

Run: `cargo nextest run -p providers --test anthropic_image_data`
Expected: 2 new tests FAIL (multipart path missing).

- [ ] **Step 4: Implement the multipart serialization**

In `anthropic_native.rs`, change the `Message::Tool` arm. Pseudo-code (adapt to real call shape):

```rust
crate::types::Message::Tool { tool_call_id, name: _, content } => {
    let serialized_content = match content {
        crate::types::ToolResultContent::Text(s) => json!(s),
        crate::types::ToolResultContent::Multipart(parts) => {
            let blocks: Vec<serde_json::Value> = parts.iter().map(|part| {
                match part {
                    crate::types::ContentPart::Text { text } => json!({"type": "text", "text": text}),
                    crate::types::ContentPart::ImageUrl { image_url } => json!({
                        "type": "image",
                        "source": {"type": "url", "url": image_url.url}
                    }),
                    crate::types::ContentPart::ImageData { media_type, data } => json!({
                        "type": "image",
                        "source": {"type": "base64", "media_type": media_type, "data": data}
                    }),
                }
            }).collect();
            json!(blocks)
        }
    };
    // Anthropic native shape: a "user" role message with one tool_result block.
    json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": serialized_content,
        }]
    })
}
```

> **Important:** Whatever the existing message serializer's exact return shape is (a single message object vs. a wrapped pair), match that shape — only the `content` field's structure changes. Do NOT restructure the message envelope in this task.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p providers --test anthropic_image_data`
Expected: all 4 tests PASS.

- [ ] **Step 6: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: zero regressions.

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "feat(providers/anthropic): serialize multipart tool results as array content blocks"
```

---

### Task 7: Anthropic adapter — `convert_tools` special case for `computer_use`

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs:187-208`
- Test: golden.

- [ ] **Step 1: Write failing golden test**

Add to `crates/providers/tests/anthropic_image_data.rs`:

```rust
#[test]
fn convert_tools_emits_computer_20251124_block() {
    let provider = AnthropicProvider::for_test();
    let openai_tool = json!({
        "type": "computer_use",
        "function": {
            "name": "computer",
            "description": "Control the computer",
            "parameters": {
                "display_width_px": 1280,
                "display_height_px": 800,
                "display_number": 0
            }
        }
    });
    let converted = provider.convert_tools(&[openai_tool]);
    assert_eq!(converted.len(), 1);
    let t = &converted[0];
    assert_eq!(t["type"], "computer_20251124");
    assert_eq!(t["name"], "computer");
    assert_eq!(t["display_width_px"], 1280);
    assert_eq!(t["display_height_px"], 800);
    assert!(t.get("input_schema").is_none(), "computer_use tools have no input_schema");
}

#[test]
fn convert_tools_passes_through_normal_function_tools() {
    let provider = AnthropicProvider::for_test();
    let openai_tool = json!({
        "type": "function",
        "function": {
            "name": "echo",
            "description": "echo input",
            "parameters": { "type": "object" }
        }
    });
    let converted = provider.convert_tools(&[openai_tool]);
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0]["name"], "echo");
    assert!(converted[0]["input_schema"].is_object());
    assert!(converted[0].get("type").is_none());
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p providers --test anthropic_image_data convert_tools`
Expected: FAIL.

- [ ] **Step 3: Update `convert_tools`**

Replace the existing function in `anthropic_native.rs` (around line 187):

```rust
pub fn convert_tools(&self, openai_tools: &[Value]) -> Vec<Value> {
    openai_tools
        .iter()
        .filter_map(|tool| {
            let tool_type = tool.get("type").and_then(|v| v.as_str()).unwrap_or("function");
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?;
            let description = func.get("description").and_then(|d| d.as_str());
            let parameters = func.get("parameters").cloned().unwrap_or(json!({}));

            if tool_type == "computer_use" {
                let mut block = json!({
                    "type": "computer_20251124",
                    "name": name,
                });
                if let Some(w) = parameters.get("display_width_px") {
                    block["display_width_px"] = w.clone();
                }
                if let Some(h) = parameters.get("display_height_px") {
                    block["display_height_px"] = h.clone();
                }
                if let Some(n) = parameters.get("display_number") {
                    block["display_number"] = n.clone();
                }
                return Some(block);
            }

            let mut anthropic_tool = json!({
                "name": name,
                "input_schema": parameters,
            });
            if let Some(desc) = description {
                anthropic_tool["description"] = json!(desc);
            }
            Some(anthropic_tool)
        })
        .collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p providers --test anthropic_image_data`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(providers/anthropic): emit computer_20251124 tool block for type=computer_use tools"
```

---

### Task 8: Anthropic adapter — `anthropic-beta` header

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (request header construction, around lines 583–632)
- Test: integration.

- [ ] **Step 1: Find the request-building site**

Run: `rg -n '"anthropic-version"' crates/providers/src/adapters/anthropic_native.rs`
Note both the request-building blocks (chat + chat_stream).

- [ ] **Step 2: Write failing test**

Add to `crates/providers/tests/anthropic_image_data.rs`:

```rust
#[test]
fn beta_header_added_when_tools_contain_computer_use() {
    let provider = AnthropicProvider::for_test();
    let tools = vec![json!({"type": "computer_use", "function": {"name": "computer"}})];
    let headers = provider.build_headers_for_test(&tools);
    let beta = headers.get("anthropic-beta").map(|v| v.to_str().unwrap().to_string());
    assert_eq!(beta.as_deref(), Some("computer-use-2025-11-24"));
}

#[test]
fn beta_header_omitted_for_normal_tools() {
    let provider = AnthropicProvider::for_test();
    let tools = vec![json!({"type": "function", "function": {"name": "echo"}})];
    let headers = provider.build_headers_for_test(&tools);
    assert!(headers.get("anthropic-beta").is_none());
}
```

> If `build_headers_for_test` does not exist, add it as `#[cfg(test)] pub` in `anthropic_native.rs` — a small helper that returns the `HeaderMap` it would attach to a request given the input tools, by extracting the existing header construction into a helper function.

- [ ] **Step 3: Run failing test**

Run: `cargo nextest run -p providers --test anthropic_image_data beta_header`
Expected: FAIL.

- [ ] **Step 4: Add the beta header**

Extract the header-building logic into a helper:

```rust
const COMPUTER_USE_BETA: &str = "computer-use-2025-11-24";

fn requires_computer_use_beta(tools: &[Value]) -> bool {
    tools.iter().any(|t| t.get("type").and_then(|v| v.as_str()) == Some("computer_use"))
}

fn build_headers(api_version: &str, api_key: &str, tools: &[Value]) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("anthropic-version", HeaderValue::from_str(api_version).unwrap());
    h.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    if requires_computer_use_beta(tools) {
        h.insert("anthropic-beta", HeaderValue::from_static(COMPUTER_USE_BETA));
    }
    h
}
```

Update both `chat()` and `chat_stream()` to call `build_headers(...)` instead of inlining headers. (Adjust to whatever HTTP client is in use; the public `reqwest::header::HeaderMap` API works for `reqwest`.)

Add the `#[cfg(test)] pub fn build_headers_for_test(...)` shim that calls `build_headers` with the provider's stored API version + a dummy API key.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p providers --test anthropic_image_data beta_header`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(providers/anthropic): emit anthropic-beta: computer-use-2025-11-24 when tools include computer_use"
```

---

### Task 9: Add `ScreenshotEvent` to `common`

**Files:**
- Modify: `crates/common/src/lib.rs` (or `crates/common/src/events.rs`, wherever existing structs like `EntityCard` live).
- Test: inline.

- [ ] **Step 1: Find where `EntityCard` is defined**

Run: `rg -n 'pub struct EntityCard' crates/common/`
Note the file. We add `ScreenshotEvent` next to it for symmetry.

- [ ] **Step 2: Write failing test**

In the same file, add `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod screenshot_event_tests {
    use super::*;

    #[test]
    fn screenshot_event_round_trips() {
        let evt = ScreenshotEvent {
            tool_call_id: "toolu_x".to_string(),
            captured_at: jiff::Timestamp::UNIX_EPOCH,
            width: 1280,
            height: 800,
            format: "png".to_string(),
            data: "AAAA".to_string(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: ScreenshotEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.width, 1280);
        assert_eq!(back.format, "png");
    }
}
```

- [ ] **Step 3: Run failing test**

Run: `cargo nextest run -p common screenshot_event`
Expected: FAIL.

- [ ] **Step 4: Add the struct**

In the same file as `EntityCard`:

```rust
/// A screenshot captured by a tool, emitted via the agent's screenshot sidecar
/// channel for UI consumers (HUD/side-panel) without polluting the agent's
/// message history. The image is always also embedded into the corresponding
/// `Message::Tool` so the model can reason over it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotEvent {
    pub tool_call_id: String,
    pub captured_at: jiff::Timestamp,
    pub width: u32,
    pub height: u32,
    /// Encoding format: "png" or "jpeg".
    pub format: String,
    /// Base64-encoded image bytes.
    pub data: String,
}
```

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p common screenshot_event`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(common): add ScreenshotEvent struct for sidecar channel payloads"
```

---

### Task 10: Add `RoutingContext::screenshot_tx` field

**Files:**
- Modify: `crates/tools-core/src/routing.rs:59-82`
- Test: inline.

- [ ] **Step 1: Write failing test**

Append to `crates/tools-core/src/routing.rs`:

```rust
#[cfg(test)]
mod screenshot_tx_tests {
    use super::*;

    #[test]
    fn default_routing_context_has_no_screenshot_tx() {
        let ctx = RoutingContext::default();
        assert!(ctx.screenshot_tx.is_none());
    }

    #[tokio::test]
    async fn screenshot_tx_clones_and_sends() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<common::ScreenshotEvent>(4);
        let mut ctx = RoutingContext::default();
        ctx.screenshot_tx = Some(tx);
        let cloned = ctx.clone();
        let evt = common::ScreenshotEvent {
            tool_call_id: "x".to_string(),
            captured_at: jiff::Timestamp::UNIX_EPOCH,
            width: 1, height: 1, format: "png".to_string(), data: String::new(),
        };
        cloned.screenshot_tx.unwrap().send(evt).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.tool_call_id, "x");
    }
}
```

> Make sure `RoutingContext` already has a `Default` impl. If it does not, ensure the new field's absence is constructible by adding `Default`. Otherwise, build the context manually with the existing constructor.

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p tools-core screenshot_tx`
Expected: FAIL — field does not exist.

- [ ] **Step 3: Add the field**

In `crates/tools-core/src/routing.rs`, update `RoutingContext`:

```rust
#[derive(Clone, Default)]
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    pub is_direct_mode: bool,
    pub delegation_depth: u32,
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
    pub squad_id: Option<String>,
    pub champion_params: Option<common::TrialParams>,
    /// Sidecar channel: tools that capture screenshots send `ScreenshotEvent`s
    /// here for UI consumers (HUD/side-panel). The image data is also embedded
    /// in the tool's textual result via the multipart sentinel.
    pub screenshot_tx: Option<mpsc::Sender<common::ScreenshotEvent>>,
    /// The id of the in-flight tool call this context belongs to. Set per-call
    /// by the executor before the tool runs.
    pub tool_call_id: Option<String>,
}
```

If `Default` is not derivable because of one of the other field types, add a `Default` impl manually with `None`/empty defaults for everything.

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p tools-core screenshot_tx`
Expected: PASS.

- [ ] **Step 5: Run workspace build**

Run: `cargo build --workspace`
Expected: success — every place that constructs `RoutingContext { ... }` needs the new fields. Fix any errors by adding `screenshot_tx: None, tool_call_id: None,` (or the `..Default::default()` shorthand if Default is derivable).

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(tools-core): add screenshot_tx + tool_call_id fields to RoutingContext"
```

---

### Task 11: Replace `sanitize_tool_result` with `process_tool_result`

**Files:**
- Modify: `crates/agent/src/execution/core.rs:31-55, 713`
- Test: new integration test.

- [ ] **Step 1: Write failing test**

Create `crates/agent/tests/process_tool_result.rs`:

```rust
use agent::execution::core::process_tool_result;
use providers::types::{ContentPart, ToolResultContent};

#[test]
fn plain_text_passes_through_as_text() {
    let raw = "ok";
    match process_tool_result(raw) {
        ToolResultContent::Text(s) => assert_eq!(s, "ok"),
        _ => panic!("expected text"),
    }
}

#[test]
fn control_chars_filtered_in_text() {
    let raw = "hello\x07world";
    match process_tool_result(raw) {
        ToolResultContent::Text(s) => assert_eq!(s, "helloworld"),
        _ => panic!("expected text"),
    }
}

#[test]
fn long_text_truncated_at_50kb() {
    let raw: String = "x".repeat(60_000);
    match process_tool_result(&raw) {
        ToolResultContent::Text(s) => {
            assert!(s.len() < 51_000);
            assert!(s.ends_with("[truncated - result exceeded 50KB]"));
        }
        _ => panic!("expected text"),
    }
}

#[test]
fn multipart_sentinel_yields_multipart() {
    let raw = serde_json::json!({
        "klynt_tool_result_multipart": [
            {"type": "text", "text": "screenshot taken"},
            {"type": "image_data", "media_type": "image/png", "data": "AAAA"}
        ]
    }).to_string();
    match process_tool_result(&raw) {
        ToolResultContent::Multipart(parts) => {
            assert_eq!(parts.len(), 2);
            matches!(parts[0], ContentPart::Text { .. });
            matches!(parts[1], ContentPart::ImageData { .. });
        }
        _ => panic!("expected multipart"),
    }
}

#[test]
fn malformed_sentinel_falls_back_to_text() {
    let raw = r#"{"klynt_tool_result_multipart": "not an array"}"#;
    match process_tool_result(raw) {
        ToolResultContent::Text(_) => {}
        _ => panic!("expected fallback to text"),
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p agent --test process_tool_result`
Expected: FAIL — `process_tool_result` does not exist (and `pub` visibility not yet exported).

- [ ] **Step 3: Implement `process_tool_result`**

Replace the existing `sanitize_tool_result` block in `crates/agent/src/execution/core.rs`:

```rust
pub const MAX_TOOL_RESULT_LENGTH: usize = 50_000;
const MULTIPART_SENTINEL_KEY: &str = "klynt_tool_result_multipart";

/// Process a raw tool-result string into a `ToolResultContent`.
/// - If the string parses as JSON with a top-level `klynt_tool_result_multipart`
///   array, each element is mapped to a `ContentPart` (preserving images verbatim).
/// - Otherwise, the string is sanitized (control chars stripped, truncated at 50KB)
///   and returned as `ToolResultContent::Text`.
pub fn process_tool_result(input: &str) -> providers::types::ToolResultContent {
    use providers::types::{ContentPart, ToolResultContent};

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(input) {
        if let Some(arr) = val.get(MULTIPART_SENTINEL_KEY).and_then(|v| v.as_array()) {
            let mut parts: Vec<ContentPart> = Vec::with_capacity(arr.len());
            for elem in arr {
                let kind = elem.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match kind {
                    "text" => {
                        if let Some(t) = elem.get("text").and_then(|v| v.as_str()) {
                            parts.push(ContentPart::Text { text: t.to_string() });
                        }
                    }
                    "image_data" => {
                        let media_type = elem.get("media_type").and_then(|v| v.as_str()).unwrap_or("image/png").to_string();
                        let data = elem.get("data").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !data.is_empty() {
                            parts.push(ContentPart::ImageData { media_type, data });
                        }
                    }
                    _ => {}
                }
            }
            if !parts.is_empty() {
                return ToolResultContent::Multipart(parts);
            }
        }
    }
    ToolResultContent::Text(sanitize_text(input))
}

fn sanitize_text(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();
    if cleaned.len() > MAX_TOOL_RESULT_LENGTH {
        let mut truncate_at = MAX_TOOL_RESULT_LENGTH;
        while truncate_at > 0 && !cleaned.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        let mut truncated = cleaned[..truncate_at].to_string();
        truncated.push_str("\n[truncated - result exceeded 50KB]");
        truncated
    } else {
        cleaned
    }
}
```

- [ ] **Step 4: Update the call site**

In `crates/agent/src/execution/core.rs:713`, replace:

```rust
messages.push(Message::tool(
    r.tool_call_id.clone(),
    r.tool_name.clone(),
    sanitize_tool_result(&r.result),
));
```

with:

```rust
messages.push(Message::tool(
    r.tool_call_id.clone(),
    r.tool_name.clone(),
    process_tool_result(&r.result),
));
```

If `sanitize_tool_result` is exported elsewhere (e.g. `pub use`), update or remove that re-export.

- [ ] **Step 5: Make `process_tool_result` available to tests**

In `crates/agent/src/execution/mod.rs`, ensure `pub use core::{process_tool_result, MAX_TOOL_RESULT_LENGTH};` is exported, and the `agent` crate's `lib.rs` re-exports the `execution` module (it likely already does).

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent --test process_tool_result`
Expected: 5 tests PASS.

- [ ] **Step 7: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: zero regressions.

- [ ] **Step 8: Commit**

```bash
git add -u
git commit -m "refactor(agent): replace sanitize_tool_result with process_tool_result (multipart-aware)"
```

---

### Task 12: Image-aware `MidLoopCompressor`

**Files:**
- Modify: `crates/agent/src/execution/mid_loop_compressor.rs:82-95`
- Test: integration.

- [ ] **Step 1: Write failing test**

Create `crates/agent/tests/midloop_compressor_image_aware.rs`:

```rust
use agent::execution::mid_loop_compressor::MidLoopCompressor;
use agent::tokens::EstimateTokenCounter;
use providers::types::{ContentPart, Message, ToolResultContent};
use std::sync::Arc;

fn long_text_msg(role_name: &str) -> Message {
    Message::tool(
        format!("call_{role_name}"),
        "echo",
        ToolResultContent::Text("x".repeat(20_000)),
    )
}

fn screenshot_msg(id: &str) -> Message {
    Message::tool(
        id,
        "computer_use",
        ToolResultContent::Multipart(vec![
            ContentPart::Text { text: format!("screenshot {id}") },
            ContentPart::ImageData {
                media_type: "image/png".to_string(),
                data: "A".repeat(50_000),
            },
        ]),
    )
}

#[test]
fn newest_screenshot_preserved_older_dropped() {
    let counter: Arc<dyn agent::tokens::TokenCounter> = Arc::new(EstimateTokenCounter::new());
    // Tiny context window to force compression on every multi-turn case.
    let compressor = MidLoopCompressor::new(counter, 4_000);

    let mut msgs = vec![
        Message::system("system".to_string()),
        // Older screenshots (will be replaced):
        screenshot_msg("ss_1"),
        long_text_msg("a"),
        screenshot_msg("ss_2"),
        long_text_msg("b"),
        // Padding to push compression past the threshold:
        long_text_msg("c"), long_text_msg("d"), long_text_msg("e"),
        long_text_msg("f"), long_text_msg("g"), long_text_msg("h"),
        // Newest screenshot — must be preserved:
        screenshot_msg("ss_latest"),
    ];

    let result = compressor.compress_if_needed(&mut msgs);
    assert!(result.is_some(), "compressor should have fired");

    // Latest screenshot is in the recent window; verify it is intact.
    let latest = msgs.iter().find_map(|m| match m {
        Message::Tool { content: ToolResultContent::Multipart(parts), tool_call_id, .. }
            if tool_call_id == "ss_latest" => Some(parts),
        _ => None,
    }).expect("latest screenshot must remain multipart");
    assert!(latest.iter().any(|p| matches!(p, ContentPart::ImageData { .. })));

    // Older screenshots should be dropped to placeholder text.
    let ss_1 = msgs.iter().find_map(|m| match m {
        Message::Tool { content, tool_call_id, .. } if tool_call_id == "ss_1" => Some(content),
        _ => None,
    }).unwrap();
    match ss_1 {
        ToolResultContent::Text(s) => {
            assert!(s.contains("[older screenshot dropped"), "got: {s}");
        }
        _ => panic!("ss_1 should have been dropped to text placeholder"),
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p agent --test midloop_compressor_image_aware`
Expected: FAIL.

- [ ] **Step 3: Update the compressor**

In `crates/agent/src/execution/mid_loop_compressor.rs`, change the inner loop (around line 82). Replace it with a two-pass strategy:

```rust
// Pass 1: identify the index of the most recent multipart-image tool message
// in the *compressible* range [system_count, recent_start). Only this one
// needs to remain multipart; all other multipart variants are dropped.
let mut latest_image_idx: Option<usize> = None;
for (i, msg) in messages[system_count..recent_start].iter().enumerate() {
    if let Message::Tool { content, .. } = msg {
        if content.has_image() {
            latest_image_idx = Some(i + system_count);
        }
    }
}

// Pass 2: compress text + drop older images.
for (i, msg) in messages[system_count..recent_start].iter_mut().enumerate() {
    let abs_i = i + system_count;
    if let Message::Tool { content, name, .. } = msg {
        match content {
            ToolResultContent::Multipart(_) => {
                if Some(abs_i) != latest_image_idx {
                    let placeholder = ToolResultContent::Text(
                        format!("[older screenshot dropped to save tokens; tool={name}]"),
                    );
                    let original_tokens = self.token_counter.estimate_text(&content.as_text_preview());
                    let new_tokens = self.token_counter.estimate_text("[older screenshot dropped to save tokens]");
                    saved_tokens += original_tokens.saturating_sub(new_tokens);
                    *content = placeholder;
                }
                // else: latest image — leave intact.
            }
            ToolResultContent::Text(text) => {
                let original_tokens = self.token_counter.estimate_text(text);
                if original_tokens > MIN_COMPRESSIBLE_TOKENS {
                    let summary = format!(
                        "{}... [compressed {name} result, originally {} chars]",
                        context_engine::first_snippet(text, SUMMARY_SNIPPET_LENGTH),
                        text.len()
                    );
                    let new_tokens = self.token_counter.estimate_text(&summary);
                    saved_tokens += original_tokens.saturating_sub(new_tokens);
                    *content = ToolResultContent::Text(summary);
                }
            }
        }
    }
}
```

> Notice messages in `[recent_start..]` (the most recent N) are untouched — Phase 1 of the spec invariant says "the latest screenshot is always preserved verbatim" because it is in the recent window. The Pass-1 latest-image search is an extra guard for the case where the latest image is *also* in the compressible range (e.g. an old session resuming).

- [ ] **Step 4: Add `ToolResultContent` import**

At the top of `mid_loop_compressor.rs`:
```rust
use providers::types::{Message, ToolResultContent};
```
(Adjust to the existing import style.)

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p agent --test midloop_compressor_image_aware`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(agent): image-aware MidLoopCompressor — preserve latest screenshot, drop older multiparts"
```

---

### Task 13: Drain `screenshot_rx` in `run_cycle` + emit `AgentEvent::ScreenshotCaptured`

**Files:**
- Modify: `crates/agent/src/events.rs` (add variant)
- Modify: `crates/agent/src/execution/core.rs` (init channel, drain after `join_all`)
- Test: integration.

- [ ] **Step 1: Add `AgentEvent` variant**

In `crates/agent/src/events.rs`, add to the `AgentEvent` enum (mirror the pattern of `EntityCreated`):

```rust
/// A tool captured a screenshot. Emitted before the corresponding tool result
/// is added to the message history. UI consumers (HUD, side-panel) display
/// the image; the agent's reasoning over the image happens via the multipart
/// `Message::Tool` content.
#[serde(rename_all = "camelCase")]
ScreenshotCaptured(common::ScreenshotEvent),
```

> Place it next to `EntityCreated` so the variants stay grouped by purpose. If `AgentEvent` already uses `#[serde(tag = "type", rename_all = "camelCase")]` at the enum level, the per-variant `#[serde(...)]` line above is unnecessary.

- [ ] **Step 2: Wire the channel in `run_cycle`**

In `crates/agent/src/execution/core.rs`, near the existing `entity_tx`/`entity_rx` setup (around line 533), add a parallel channel:

```rust
let (entity_tx, mut entity_rx) = tokio::sync::mpsc::channel::<common::EntityCard>(16);
let (screenshot_tx, mut screenshot_rx) = tokio::sync::mpsc::channel::<common::ScreenshotEvent>(8);
```

In the per-tool-call clone of `RoutingContext` (around line 549), add:

```rust
let mut ctx = routing_ctx.clone();
ctx.entity_tx = Some(entity_tx.clone());
ctx.screenshot_tx = Some(screenshot_tx.clone());
ctx.tool_call_id = Some(tool_call.id.clone());
```

> If the per-tool tool_call isn't in scope as `tool_call.id` at that point, replace with whichever local binding holds the tool call's ID — typically the `id` field of the `ToolCall` struct being iterated.

After the `drop(entity_tx);` line (around 644), add:

```rust
drop(screenshot_tx);
```

After the existing entity_rx draining loop (around 649–655), add the screenshot drain:

```rust
if let Some(tx) = event_tx {
    while let Ok(evt) = screenshot_rx.try_recv() {
        let _ = tx.send(crate::events::AgentEvent::ScreenshotCaptured(evt)).await;
    }
}
```

- [ ] **Step 3: Write failing integration test**

Create `crates/agent/tests/screenshot_event_emission.rs`:

```rust
// Use a stub Tool that emits a ScreenshotEvent + returns a multipart sentinel string.
// Drive run_cycle and verify both: (a) AgentEvent::ScreenshotCaptured fires,
// (b) the resulting Message::Tool has Multipart content with the image.

// Skip if the test infrastructure for run_cycle is not directly accessible.
// Mark this test with #[ignore] if needed and expand in Task 30 (full smoke).
```

> If exposing `run_cycle` for unit-testing is too invasive, defer this to Task 30's end-to-end smoke. In that case, the only mechanical assertion here is that `AgentEvent::ScreenshotCaptured` exists and is constructible:

```rust
use agent::events::AgentEvent;
use common::ScreenshotEvent;

#[test]
fn screenshot_captured_variant_constructible() {
    let evt = ScreenshotEvent {
        tool_call_id: "x".to_string(),
        captured_at: jiff::Timestamp::UNIX_EPOCH,
        width: 1, height: 1, format: "png".to_string(), data: String::new(),
    };
    let _e = AgentEvent::ScreenshotCaptured(evt);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent --test screenshot_event_emission`
Expected: PASS.

- [ ] **Step 5: Run workspace build**

Run: `cargo build --workspace`
Expected: success — no unhandled enum arms anywhere (search for `match … AgentEvent::` if compile fails on a non-exhaustive arm and add the new variant).

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(agent): drain screenshot_rx + emit AgentEvent::ScreenshotCaptured"
```

---

### Task 14: Workspace scaffold for `feature-computer-use`

**Files:**
- Create: `crates/feature-computer-use/Cargo.toml`
- Create: `crates/feature-computer-use/src/lib.rs` (placeholder)
- Modify: workspace root `Cargo.toml` — add member.
- Test: minimal compile.

- [ ] **Step 1: Add to workspace**

Modify root `Cargo.toml` `[workspace] members = [...]`:

```toml
"crates/feature-computer-use",
```

(Place alphabetically next to `crates/feature-coaching` etc.)

- [ ] **Step 2: Create `crates/feature-computer-use/Cargo.toml`**

```toml
[package]
name = "feature-computer-use"
version = "0.1.0"
edition = "2024"

[dependencies]
common.workspace = true
tools-core.workspace = true
tools-core-macros.workspace = true
platform-input.workspace = true
platform-capture.workspace = true
providers.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
base64 = "0.22"
jiff.workspace = true
tracing.workspace = true
thiserror.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt"] }
```

> Validate against root `Cargo.toml` `[workspace.dependencies]` to confirm `platform-input`, `platform-capture`, `providers`, `jiff` are listed there. If any are missing, add them under `[workspace.dependencies]` (path-style for the platform-* crates).

- [ ] **Step 3: Placeholder `src/lib.rs`**

```rust
//! Computer-use feature crate (Phase 2 — see docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p feature-computer-use`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/feature-computer-use/
git commit -m "feat(feature-computer-use): scaffold L4 crate with workspace deps"
```

---

### Task 15: `ActionParams` structs

**Files:**
- Create: `crates/feature-computer-use/src/tool/actions.rs`
- Test: inline.

- [ ] **Step 1: Write failing tests**

Create the file with tests at the bottom:

```rust
//! Per-action parameter structs for the ComputerUseTool. Each struct uses
//! `#[derive(ActionParams)]` so the `#[tool_actions]` macro can dispatch
//! incoming JSON to the correct method.

use serde::{Deserialize, Serialize};
use tools_core_macros::ActionParams;

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct ScreenshotParams {
    /// Optional region as [x, y, width, height]. If omitted, captures the active app's window.
    #[param(required = false)]
    pub region: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct PointParams {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct ClickParams {
    pub x: i32,
    pub y: i32,
    /// Modifier keys held during click. Each is one of: "cmd", "ctrl", "alt", "shift", "fn".
    #[param(required = false)]
    pub modifiers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct TypeParams {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct KeyParams {
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct HoldKeyParams {
    pub keys: Vec<String>,
    #[param(min = 1)]
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct ScrollParams {
    pub x: i32,
    pub y: i32,
    /// One of "up", "down", "left", "right".
    pub direction: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct DragParams {
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
    #[param(required = false)]
    pub modifiers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct WaitParams {
    #[param(min = 1)]
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams)]
pub struct ZoomParams {
    /// Region [x, y, width, height].
    pub region: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_core::ActionParams as _;

    #[test]
    fn screenshot_params_no_region() {
        let v = serde_json::json!({});
        let p = ScreenshotParams::from_value(&v).expect("ok");
        assert!(p.region.is_none());
    }

    #[test]
    fn screenshot_params_with_region() {
        let v = serde_json::json!({"region": [0, 0, 1280, 800]});
        let p = ScreenshotParams::from_value(&v).expect("ok");
        assert_eq!(p.region.unwrap().len(), 4);
    }

    #[test]
    fn click_params_with_modifiers() {
        let v = serde_json::json!({"x": 100, "y": 200, "modifiers": ["cmd"]});
        let p = ClickParams::from_value(&v).expect("ok");
        assert_eq!(p.x, 100);
        assert_eq!(p.modifiers.unwrap(), vec!["cmd"]);
    }

    #[test]
    fn key_params_validation() {
        let v = serde_json::json!({"keys": ["enter"]});
        let p = KeyParams::from_value(&v).expect("ok");
        assert_eq!(p.keys, vec!["enter"]);
    }
}
```

- [ ] **Step 2: Wire module into `lib.rs`**

```rust
// crates/feature-computer-use/src/lib.rs
pub mod tool;
```

```rust
// crates/feature-computer-use/src/tool/mod.rs (placeholder)
pub mod actions;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-computer-use`
Expected: 4 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-computer-use/src/
git commit -m "feat(feature-computer-use): action params structs for all 16 ComputerUseAction variants"
```

---

### Task 16: `multipart_payload` helper + sentinel constant

**Files:**
- Create: `crates/feature-computer-use/src/tool/result.rs`
- Test: inline.

- [ ] **Step 1: Write failing test**

```rust
//! Helpers for building tool result strings that the agent's
//! `process_tool_result` will decode into `ToolResultContent::Multipart`.

use serde::Serialize;

pub const MULTIPART_SENTINEL_KEY: &str = "klynt_tool_result_multipart";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MultipartElem<'a> {
    Text { text: &'a str },
    ImageData { media_type: &'a str, data: &'a str },
}

/// Build the sentinel JSON string that `process_tool_result` decodes into
/// `ToolResultContent::Multipart`.
pub fn multipart_payload(parts: &[MultipartElem<'_>]) -> String {
    let v = serde_json::json!({ MULTIPART_SENTINEL_KEY: parts });
    serde_json::to_string(&v).expect("serialize multipart sentinel")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_contains_sentinel_key() {
        let s = multipart_payload(&[MultipartElem::Text { text: "hi" }]);
        assert!(s.contains("klynt_tool_result_multipart"));
        assert!(s.contains("\"type\":\"text\""));
    }

    #[test]
    fn payload_image_data_serializes_with_snake_case() {
        let s = multipart_payload(&[
            MultipartElem::Text { text: "captured" },
            MultipartElem::ImageData { media_type: "image/png", data: "AAAA" },
        ]);
        assert!(s.contains("\"type\":\"image_data\""));
        assert!(s.contains("\"media_type\":\"image/png\""));
    }
}
```

- [ ] **Step 2: Wire module into `tool/mod.rs`**

```rust
pub mod actions;
pub mod result;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-computer-use result::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): multipart_payload helper for sentinel-encoded image results"
```

---

### Task 17: `ComputerUseTool` skeleton + screenshot action

**Files:**
- Create/replace: `crates/feature-computer-use/src/tool/mod.rs`
- Test: integration.

- [ ] **Step 1: Write failing test**

Create `crates/feature-computer-use/tests/tool_smoke.rs`:

```rust
use feature_computer_use::ComputerUseTool;
use platform_capture::mock::MockCapture;
use platform_input::mock::MockInput;
use providers::types::{ContentPart, ToolResultContent};
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

fn make_tool() -> ComputerUseTool {
    ComputerUseTool::new(
        Arc::new(MockInput::new()),
        Arc::new(MockCapture::checkerboard()),
    )
}

#[tokio::test]
async fn screenshot_returns_multipart_sentinel() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "screenshot"});
    let result = tool.execute(args, &ctx).await.expect("ok");
    // Should be sentinel-encoded.
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let arr = parsed["klynt_tool_result_multipart"].as_array().expect("array");
    assert!(arr.iter().any(|e| e["type"] == "image_data"));
}

#[tokio::test]
async fn screenshot_emits_screenshot_event_on_sidecar() {
    let tool = make_tool();
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut ctx = RoutingContext::default();
    ctx.screenshot_tx = Some(tx);
    ctx.tool_call_id = Some("toolu_xyz".to_string());
    let args = serde_json::json!({"action": "screenshot"});
    let _ = tool.execute(args, &ctx).await.expect("ok");
    let evt = rx.recv().await.expect("received");
    assert_eq!(evt.tool_call_id, "toolu_xyz");
    assert!(evt.width > 0);
    assert!(!evt.data.is_empty());
}
```

> If `MockCapture::checkerboard()` returns the test fixture from Phase 1, great. If the constructor name differs, use whatever Phase 1 actually exposed (e.g. `MockCapture::new()`). The fixture should yield a small frame (e.g. 4×4 px).

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke`
Expected: FAIL — `ComputerUseTool` does not exist.

- [ ] **Step 3: Implement the tool skeleton + screenshot**

Replace `crates/feature-computer-use/src/tool/mod.rs`:

```rust
pub mod actions;
pub mod result;

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use common::Result;
use platform_capture::{AxScope, PlatformCapture};
use platform_input::{ComputerUseAction, KeyMods, PlatformInput, Point, Rect, ScrollDir};
use tools_core::{ActionParams as _, RoutingContext};
use tools_core_macros::tool_actions;
use tracing::{debug, instrument};

use self::actions::*;
use self::result::{multipart_payload, MultipartElem};

pub struct ComputerUseTool {
    input: Arc<dyn PlatformInput>,
    capture: Arc<dyn PlatformCapture>,
}

impl ComputerUseTool {
    pub fn new(input: Arc<dyn PlatformInput>, capture: Arc<dyn PlatformCapture>) -> Self {
        Self { input, capture }
    }

    fn parse_modifiers(raw: &Option<Vec<String>>) -> KeyMods {
        let mut m = KeyMods::default();
        if let Some(list) = raw {
            for k in list {
                match k.to_lowercase().as_str() {
                    "cmd" | "command" | "meta" => m.cmd = true,
                    "ctrl" | "control" => m.ctrl = true,
                    "alt" | "option" | "opt" => m.alt = true,
                    "shift" => m.shift = true,
                    "fn" => m.r#fn = true,
                    _ => {}
                }
            }
        }
        m
    }

    fn parse_scroll_dir(raw: &str) -> ScrollDir {
        match raw.to_lowercase().as_str() {
            "up" => ScrollDir::Up,
            "down" => ScrollDir::Down,
            "left" => ScrollDir::Left,
            "right" => ScrollDir::Right,
            _ => ScrollDir::Down,
        }
    }
}

#[tool_actions(
    name = "computer_use",
    description = "Control the screen, keyboard, and mouse. Use this for any task that requires interacting with on-screen UI elements not addressable through more specific tools (launcher, tasks, etc.).",
    category = "System",
    tags = "computer,screen,mouse,keyboard,automation",
    cost = "Variable"
)]
impl ComputerUseTool {
    #[action(name = "screenshot")]
    #[instrument(skip(self, ctx), err)]
    async fn screenshot(&self, params: ScreenshotParams, ctx: &RoutingContext) -> Result<String> {
        let region = params.region.as_ref().and_then(|r| {
            if r.len() == 4 { Some(Rect { x: r[0], y: r[1], width: r[2] as u32, height: r[3] as u32 }) } else { None }
        });
        let frame = self
            .capture
            .capture_screen(region)
            .map_err(|e| common::KlyntbotError::other(format!("capture failed: {e}")))?;

        let png = encode_png(&frame)
            .map_err(|e| common::KlyntbotError::other(format!("encode failed: {e}")))?;
        let b64 = STANDARD.encode(&png);

        // Sidecar emit (HUD/UI consumer).
        if let Some(tx) = &ctx.screenshot_tx {
            let evt = common::ScreenshotEvent {
                tool_call_id: ctx.tool_call_id.clone().unwrap_or_default(),
                captured_at: jiff::Timestamp::now(),
                width: frame.width,
                height: frame.height,
                format: "png".to_string(),
                data: b64.clone(),
            };
            let _ = tx.send(evt).await;
        }

        let summary = format!("Captured screen: {}x{} px", frame.width, frame.height);
        Ok(multipart_payload(&[
            MultipartElem::Text { text: &summary },
            MultipartElem::ImageData { media_type: "image/png", data: &b64 },
        ]))
    }

    // Other actions added in subsequent tasks.
}

/// Encode a `Frame`'s raw pixels into PNG bytes.
fn encode_png(frame: &platform_capture::Frame) -> std::io::Result<Vec<u8>> {
    use std::io::Cursor;
    // Phase 2: assume frame.format == BGRA or RGBA. Use the `image` crate for correctness.
    // If you don't want to add `image` as a dep, hand-roll a minimal PNG via `png` crate.
    let mut out: Vec<u8> = Vec::with_capacity(frame.data.len() / 4);
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| std::io::Error::other(e.to_string()))?;

        // Convert BGRA (ScreenCaptureKit default) → RGBA.
        let pixel_count = (frame.width * frame.height) as usize;
        let mut rgba = Vec::with_capacity(pixel_count * 4);
        let stride = 4;
        for chunk in frame.data.chunks_exact(stride) {
            // BGRA: chunk[0]=B, [1]=G, [2]=R, [3]=A → RGBA: R,G,B,A
            rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
        }
        writer.write_image_data(&rgba).map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    Ok(out)
}
```

Add `png = "0.17"` to `crates/feature-computer-use/Cargo.toml` `[dependencies]`.

> If `MockCapture::checkerboard()` returns RGBA data instead of BGRA, the BGRA→RGBA conversion still works (it just swaps R/B, which is acceptable for a mock that doesn't care about color fidelity).

- [ ] **Step 4: Re-export from `lib.rs`**

```rust
pub mod tool;

pub use tool::ComputerUseTool;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke`
Expected: 2 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): ComputerUseTool with screenshot action via PlatformCapture"
```

---

### Task 18: Click + mouse-move + manual mouse button actions

**Files:**
- Modify: `crates/feature-computer-use/src/tool/mod.rs`
- Test: extend `tests/tool_smoke.rs`.

- [ ] **Step 1: Write failing tests**

Append to `crates/feature-computer-use/tests/tool_smoke.rs`:

```rust
#[tokio::test]
async fn left_click_dispatches_action() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "left_click", "x": 100, "y": 200});
    let result = tool.execute(args, &ctx).await.expect("ok");
    assert!(result.contains("clicked"));
}

#[tokio::test]
async fn mouse_move_dispatches_action() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "mouse_move", "x": 50, "y": 60});
    tool.execute(args, &ctx).await.expect("ok");
}

#[tokio::test]
async fn invalid_action_returns_error() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "no_such_thing"});
    let err = tool.execute(args, &ctx).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("invalid") || err.to_string().to_lowercase().contains("unknown"));
}
```

- [ ] **Step 2: Run failing tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke left_click`
Expected: FAIL — actions don't exist.

- [ ] **Step 3: Add the actions inside `#[tool_actions]` impl**

Add the methods inside the `impl ComputerUseTool` block from Task 17:

```rust
    #[action(name = "left_click")]
    #[instrument(skip(self, _ctx), err)]
    async fn left_click(&self, params: ClickParams, _ctx: &RoutingContext) -> Result<String> {
        let action = ComputerUseAction::LeftClick {
            x: params.x,
            y: params.y,
            modifiers: Self::parse_modifiers(&params.modifiers),
        };
        self.input.perform_action(action).map_err(|e| common::KlyntbotError::other(format!("left_click: {e}")))?;
        Ok(format!("clicked at ({}, {})", params.x, params.y))
    }

    #[action(name = "double_click")]
    #[instrument(skip(self, _ctx), err)]
    async fn double_click(&self, params: ClickParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::DoubleClick {
            x: params.x, y: params.y,
            modifiers: Self::parse_modifiers(&params.modifiers),
        }).map_err(|e| common::KlyntbotError::other(format!("double_click: {e}")))?;
        Ok(format!("double-clicked at ({}, {})", params.x, params.y))
    }

    #[action(name = "triple_click")]
    #[instrument(skip(self, _ctx), err)]
    async fn triple_click(&self, params: ClickParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::TripleClick {
            x: params.x, y: params.y,
            modifiers: Self::parse_modifiers(&params.modifiers),
        }).map_err(|e| common::KlyntbotError::other(format!("triple_click: {e}")))?;
        Ok(format!("triple-clicked at ({}, {})", params.x, params.y))
    }

    #[action(name = "right_click")]
    #[instrument(skip(self, _ctx), err)]
    async fn right_click(&self, params: PointParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::RightClick { x: params.x, y: params.y })
            .map_err(|e| common::KlyntbotError::other(format!("right_click: {e}")))?;
        Ok(format!("right-clicked at ({}, {})", params.x, params.y))
    }

    #[action(name = "middle_click")]
    #[instrument(skip(self, _ctx), err)]
    async fn middle_click(&self, params: PointParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::MiddleClick { x: params.x, y: params.y })
            .map_err(|e| common::KlyntbotError::other(format!("middle_click: {e}")))?;
        Ok(format!("middle-clicked at ({}, {})", params.x, params.y))
    }

    #[action(name = "mouse_move")]
    #[instrument(skip(self, _ctx), err)]
    async fn mouse_move(&self, params: PointParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::MouseMove { x: params.x, y: params.y })
            .map_err(|e| common::KlyntbotError::other(format!("mouse_move: {e}")))?;
        Ok(format!("moved cursor to ({}, {})", params.x, params.y))
    }

    #[action(name = "left_mouse_down")]
    async fn left_mouse_down(&self, params: PointParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::LeftMouseDown { x: params.x, y: params.y })
            .map_err(|e| common::KlyntbotError::other(format!("left_mouse_down: {e}")))?;
        Ok(format!("mouse down at ({}, {})", params.x, params.y))
    }

    #[action(name = "left_mouse_up")]
    async fn left_mouse_up(&self, params: PointParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::LeftMouseUp { x: params.x, y: params.y })
            .map_err(|e| common::KlyntbotError::other(format!("left_mouse_up: {e}")))?;
        Ok(format!("mouse up at ({}, {})", params.x, params.y))
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke`
Expected: 5+ tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): click + mouse-move + manual mouse button actions"
```

---

### Task 19: Type, key, hold_key actions

**Files:**
- Modify: `crates/feature-computer-use/src/tool/mod.rs`
- Test: append.

- [ ] **Step 1: Write failing test**

Append to `tests/tool_smoke.rs`:

```rust
#[tokio::test]
async fn type_text_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "type", "text": "hello"});
    let result = tool.execute(args, &ctx).await.expect("ok");
    assert!(result.contains("typed"));
}

#[tokio::test]
async fn key_combo_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "key", "keys": ["cmd", "l"]});
    let result = tool.execute(args, &ctx).await.expect("ok");
    assert!(result.contains("pressed"));
}

#[tokio::test]
async fn hold_key_validates_duration() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "hold_key", "keys": ["shift"], "duration_ms": 100});
    tool.execute(args, &ctx).await.expect("ok");
}
```

- [ ] **Step 2: Run failing tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke type_text`
Expected: FAIL.

- [ ] **Step 3: Add actions**

```rust
    #[action(name = "type")]
    #[instrument(skip(self, _ctx, params), err, fields(len = params.text.len()))]
    async fn type_text(&self, params: TypeParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::Type { text: params.text.clone() })
            .map_err(|e| common::KlyntbotError::other(format!("type: {e}")))?;
        Ok(format!("typed {} chars", params.text.len()))
    }

    #[action(name = "key")]
    #[instrument(skip(self, _ctx), err)]
    async fn key(&self, params: KeyParams, _ctx: &RoutingContext) -> Result<String> {
        let combo = params.keys.join("+");
        self.input.perform_action(ComputerUseAction::Key { keys: params.keys })
            .map_err(|e| common::KlyntbotError::other(format!("key: {e}")))?;
        Ok(format!("pressed {combo}"))
    }

    #[action(name = "hold_key")]
    #[instrument(skip(self, _ctx), err)]
    async fn hold_key(&self, params: HoldKeyParams, _ctx: &RoutingContext) -> Result<String> {
        let combo = params.keys.join("+");
        self.input.perform_action(ComputerUseAction::HoldKey { keys: params.keys, duration_ms: params.duration_ms })
            .map_err(|e| common::KlyntbotError::other(format!("hold_key: {e}")))?;
        Ok(format!("held {combo} for {}ms", params.duration_ms))
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): type, key, hold_key actions"
```

---

### Task 20: Scroll, drag, wait, zoom actions

**Files:**
- Modify: `crates/feature-computer-use/src/tool/mod.rs`
- Test: append.

- [ ] **Step 1: Write failing tests**

Append to `tests/tool_smoke.rs`:

```rust
#[tokio::test]
async fn scroll_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "scroll", "x": 100, "y": 200, "direction": "down", "amount": 3});
    tool.execute(args, &ctx).await.expect("ok");
}

#[tokio::test]
async fn drag_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({
        "action": "left_click_drag",
        "from_x": 10, "from_y": 20, "to_x": 30, "to_y": 40
    });
    tool.execute(args, &ctx).await.expect("ok");
}

#[tokio::test]
async fn wait_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "wait", "duration_ms": 5});
    tool.execute(args, &ctx).await.expect("ok");
}

#[tokio::test]
async fn zoom_dispatches() {
    let tool = make_tool();
    let ctx = RoutingContext::default();
    let args = serde_json::json!({"action": "zoom", "region": [0, 0, 100, 100]});
    let result = tool.execute(args, &ctx).await;
    // MacInput returns NotImplemented for zoom in Phase 1; mock should pass through.
    // Either Ok or a graceful "not_implemented" error are acceptable.
    let _ = result;
}
```

- [ ] **Step 2: Run failing tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke scroll_dispatches`
Expected: FAIL.

- [ ] **Step 3: Add actions**

```rust
    #[action(name = "scroll")]
    #[instrument(skip(self, _ctx), err)]
    async fn scroll(&self, params: ScrollParams, _ctx: &RoutingContext) -> Result<String> {
        let direction = Self::parse_scroll_dir(&params.direction);
        self.input.perform_action(ComputerUseAction::Scroll {
            x: params.x, y: params.y, direction, amount: params.amount,
        }).map_err(|e| common::KlyntbotError::other(format!("scroll: {e}")))?;
        Ok(format!("scrolled {} {} ticks at ({}, {})", params.direction, params.amount, params.x, params.y))
    }

    #[action(name = "left_click_drag")]
    #[instrument(skip(self, _ctx), err)]
    async fn left_click_drag(&self, params: DragParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::LeftClickDrag {
            from: Point { x: params.from_x, y: params.from_y },
            to: Point { x: params.to_x, y: params.to_y },
            hold_modifiers: Self::parse_modifiers(&params.modifiers),
        }).map_err(|e| common::KlyntbotError::other(format!("drag: {e}")))?;
        Ok(format!("dragged from ({}, {}) to ({}, {})", params.from_x, params.from_y, params.to_x, params.to_y))
    }

    #[action(name = "wait")]
    async fn wait(&self, params: WaitParams, _ctx: &RoutingContext) -> Result<String> {
        self.input.perform_action(ComputerUseAction::Wait { duration_ms: params.duration_ms })
            .map_err(|e| common::KlyntbotError::other(format!("wait: {e}")))?;
        Ok(format!("waited {}ms", params.duration_ms))
    }

    #[action(name = "zoom")]
    async fn zoom(&self, params: ZoomParams, _ctx: &RoutingContext) -> Result<String> {
        if params.region.len() != 4 {
            return Err(common::KlyntbotError::other("zoom region must have 4 elements"));
        }
        let rect = Rect {
            x: params.region[0],
            y: params.region[1],
            width: params.region[2] as u32,
            height: params.region[3] as u32,
        };
        // Phase 1 MacInput returns NotImplemented for Zoom; that surfaces here.
        match self.input.perform_action(ComputerUseAction::Zoom { region: rect }) {
            Ok(()) => Ok(format!("zoomed to region {:?}", params.region)),
            Err(e) => Ok(format!("zoom not implemented yet: {e}")),
        }
    }
```

> Zoom returns `Ok` even when `NotImplemented` is the underlying error. This matches the spec — the action is part of the vocabulary but the platform impl is deferred to Phase 4. Returning success-with-message lets the agent reason that the action was acknowledged but the platform did nothing.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke`
Expected: all action tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): scroll, drag, wait, zoom actions"
```

---

### Task 21: `ComputerUseFeature` (FeaturePackage impl)

**Files:**
- Modify: `crates/feature-computer-use/src/lib.rs`
- Test: inline.

- [ ] **Step 1: Write failing test**

Append to `crates/feature-computer-use/tests/tool_smoke.rs`:

```rust
#[test]
fn feature_package_returns_one_tool_when_deps_present() {
    use tools_core::FeaturePackage;
    let f = feature_computer_use::ComputerUseFeature::with_tool_deps(
        feature_computer_use::ComputerUseToolDeps {
            input: Arc::new(MockInput::new()),
            capture: Arc::new(MockCapture::checkerboard()),
        },
    );
    assert_eq!(f.tools().len(), 1);
    assert!(f.migrations().is_empty());
}

#[test]
fn feature_package_returns_no_tools_without_deps() {
    use tools_core::FeaturePackage;
    let f = feature_computer_use::ComputerUseFeature::new();
    assert_eq!(f.tools().len(), 0);
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke feature_package`
Expected: FAIL — types don't exist.

- [ ] **Step 3: Implement `ComputerUseFeature`**

Update `crates/feature-computer-use/src/lib.rs`:

```rust
//! Computer-use feature crate (Phase 2).
//!
//! Wraps the platform-input + platform-capture traits in a `ComputerUseTool`
//! exposed to the agent runtime via the `tools-core::FeaturePackage` trait.

pub mod tool;

pub use tool::ComputerUseTool;

use std::sync::Arc;

use async_trait::async_trait;
use platform_capture::PlatformCapture;
use platform_input::PlatformInput;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct ComputerUseToolDeps {
    pub input: Arc<dyn PlatformInput>,
    pub capture: Arc<dyn PlatformCapture>,
}

pub struct ComputerUseFeature {
    deps: Option<ComputerUseToolDeps>,
}

impl ComputerUseFeature {
    pub fn new() -> Self {
        Self { deps: None }
    }

    pub fn with_tool_deps(deps: ComputerUseToolDeps) -> Self {
        Self { deps: Some(deps) }
    }
}

impl Default for ComputerUseFeature {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeaturePackage for ComputerUseFeature {
    fn name(&self) -> &str {
        "computer-use"
    }

    fn tools(&self) -> Vec<DynTool> {
        match &self.deps {
            Some(d) => vec![Arc::new(ComputerUseTool::new(
                Arc::clone(&d.input),
                Arc::clone(&d.capture),
            ))],
            None => vec![],
        }
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![]
    }

    async fn health_check(&self) -> common::Result<HealthStatus> {
        Ok(if self.deps.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded("no platform deps wired".to_string())
        })
    }
}
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p feature-computer-use --test tool_smoke feature_package`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(feature-computer-use): ComputerUseFeature (FeaturePackage impl)"
```

---

### Task 22: `ComputerUseConfig` schema

**Files:**
- Create: `crates/config/src/schema/computer_use.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`
- Test: inline.

- [ ] **Step 1: Write failing test**

In `crates/config/src/schema/computer_use.rs` (will create in step 3), tests will live inline. For now, add a workspace-level config-deserialization test in `crates/config/tests/` (or wherever existing tests live):

```rust
// crates/config/tests/computer_use_config_test.rs (new file)
use config::Config;

#[test]
fn config_deserializes_with_default_computer_use() {
    let json = r#"{}"#;
    let cfg: Config = serde_json::from_str(json).expect("default ok");
    // Just assert the field exists.
    let _ = &cfg.computer_use;
}

#[test]
fn config_deserializes_explicit_computer_use_block() {
    let json = r#"{
        "computerUse": {
            "providers": { "cloud": "anthropic" }
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("explicit ok");
    assert_eq!(cfg.computer_use.providers.cloud, Some("anthropic".to_string()));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p config --test computer_use_config_test`
Expected: FAIL — `computer_use` field doesn't exist on `Config`.

- [ ] **Step 3: Create the schema module**

Create `crates/config/src/schema/computer_use.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseConfig {
    #[serde(default)]
    pub providers: ComputerUseProvidersConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseProvidersConfig {
    /// Provider key for the cloud VLM tier (Phase 2: only this is wired).
    /// Defaults to `None` → use the default agent provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud: Option<String>,
    /// Provider key for the local VLM tier (Phase 4 — accepted but unused in Phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    /// Provider key for the embedding model (Phase 6 — accepted but unused in Phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<String>,
}
```

- [ ] **Step 4: Wire into `mod.rs`**

In `crates/config/src/schema/mod.rs`, add (alphabetical order):

```rust
mod computer_use;
pub use self::computer_use::*;
```

- [ ] **Step 5: Add to `Config` root**

In `crates/config/src/schema/core.rs`, near the top of the `Config` struct's fields (alphabetical with other sub-configs):

```rust
    #[serde(default)]
    pub computer_use: ComputerUseConfig,
```

If `core.rs` doesn't already `use super::ComputerUseConfig;` or rely on glob import, add:

```rust
use super::computer_use::ComputerUseConfig;
```

- [ ] **Step 6: Run test**

Run: `cargo nextest run -p config --test computer_use_config_test`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "feat(config): ComputerUseConfig schema with cloud/local/embedding provider keys"
```

---

### Task 23: `AppCore` fields for platform singletons

**Files:**
- Modify: `crates/app-core/src/state.rs:40-188`
- Modify: `crates/app-core/Cargo.toml`
- Test: defer to Task 24.

- [ ] **Step 1: Add deps**

Update `crates/app-core/Cargo.toml` `[dependencies]`:

```toml
platform-input.workspace = true
platform-capture.workspace = true
feature-computer-use.workspace = true
```

(Confirm those three are listed under `[workspace.dependencies]` in the root `Cargo.toml`; add path-style entries if missing.)

- [ ] **Step 2: Add fields to `AppCore`**

In `crates/app-core/src/state.rs`, append to the struct (before the closing `}`):

```rust
    /// Phase-2 input injection backend. `None` if the platform does not support it
    /// or permissions have not been granted.
    pub platform_input: Option<std::sync::Arc<dyn platform_input::PlatformInput>>,
    /// Phase-2 screen capture backend. `None` if the platform does not support it
    /// or screen-recording permission has not been granted.
    pub platform_capture: Option<std::sync::Arc<dyn platform_capture::PlatformCapture>>,
```

- [ ] **Step 3: Verify compile**

Run: `cargo check -p app-core`
Expected: failure — every `AppCore { ... }` literal needs the new fields. Note their locations.

- [ ] **Step 4: Update every constructor site**

Find all `AppCore {` literals in `crates/app-core/src/`. Add `platform_input: None, platform_capture: None,` to each. (They will be set non-`None` in Task 24.)

- [ ] **Step 5: Verify compile**

Run: `cargo check -p app-core`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(app-core): add platform_input + platform_capture singleton fields to AppCore"
```

---

### Task 24: Instantiate `MacInput`/`MacCapture` in `AppCore::init_with_sender`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:89, 1667-1675, struct literal`
- Test: workspace build.

- [ ] **Step 1: Find the struct literal**

Run: `rg -n 'AppCore\s*\{' crates/app-core/src/init/`
Note the line of the `AppCore { ... }` literal at the end of `init_with_sender`. The new fields go in there.

- [ ] **Step 2: Insert platform singleton construction**

Immediately before the `AppCore { ... }` literal, add:

```rust
    // ── Phase-2 platform singletons ──
    #[cfg(target_os = "macos")]
    let platform_input: Option<Arc<dyn platform_input::PlatformInput>> =
        Some(Arc::new(platform_macos::computer_use::MacInput::new()));
    #[cfg(not(target_os = "macos"))]
    let platform_input: Option<Arc<dyn platform_input::PlatformInput>> =
        Some(Arc::new(platform_input::mock::MockInput::new()));

    #[cfg(target_os = "macos")]
    let platform_capture: Option<Arc<dyn platform_capture::PlatformCapture>> =
        Some(Arc::new(platform_macos::computer_use::MacCapture::new()));
    #[cfg(not(target_os = "macos"))]
    let platform_capture: Option<Arc<dyn platform_capture::PlatformCapture>> =
        Some(Arc::new(platform_capture::mock::MockCapture::new()));
```

In the struct literal at the end, replace the `platform_input: None, platform_capture: None,` from Task 23 with:

```rust
        platform_input,
        platform_capture,
```

- [ ] **Step 3: Verify compile**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 4: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "feat(app-core): wire MacInput/MacCapture singletons (Mock fallback off-macOS)"
```

---

### Task 25: Register `ComputerUseTool` in agent registry

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` (right after the `LauncherTool` registration block, ~lines 1019-1035)
- Test: integration test verifying registration.

- [ ] **Step 1: Write failing test**

Create `crates/app-core/tests/computer_use_registered.rs`:

```rust
use app_core::AppCore;

#[tokio::test]
async fn computer_use_tool_registered_in_agent_registry() {
    let core = AppCore::init_in_memory_for_test().await.expect("init ok");
    let registry = core.agent.tool_registry();
    let r = registry.read().await;
    assert!(r.has_tool("computer_use"), "computer_use must be registered");
}
```

> If `AppCore::init_in_memory_for_test()` does not exist, search for the existing in-memory test helper (likely `init_with_in_memory_storage` or similar) and use that instead. If none exists, this test moves to `crates/agent/tests/` and verifies through `AgentLoop::tool_registry()` against a manually constructed `ComputerUseFeature`.

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p app-core --test computer_use_registered`
Expected: FAIL.

- [ ] **Step 3: Add the registration block**

In `crates/app-core/src/init/mod.rs`, immediately after the launcher registration block (around line 1035, after `info!("Launcher tools registered ...")`):

```rust
    // ── Register computer-use tool in agent's tool registry ──
    if let (Some(input), Some(capture)) = (&platform_input, &platform_capture) {
        use tools_core::FeaturePackage;
        let reg = agent.tool_registry();
        let mut registry = reg.write().await;
        let cu = feature_computer_use::ComputerUseFeature::with_tool_deps(
            feature_computer_use::ComputerUseToolDeps {
                input: Arc::clone(input),
                capture: Arc::clone(capture),
            },
        );
        for tool in cu.tools() {
            registry.register_dyn(tool);
        }
        info!("ComputerUseTool registered in agent registry");
    }
```

> The block intentionally moves AFTER `platform_input`/`platform_capture` are constructed (Task 24) but BEFORE the `AppCore { ... }` literal. If the order is wrong, hoist the construction earlier.

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p app-core --test computer_use_registered`
Expected: PASS.

- [ ] **Step 5: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(app-core): register ComputerUseTool alongside LauncherTool in agent registry"
```

---

### Task 26: `skills/computer-use/SKILL.md`

**Files:**
- Create: `skills/computer-use/SKILL.md`
- Test: skill-loading round-trip in Task 28.

- [ ] **Step 1: Author the skill**

```markdown
---
name: computer-use
description: Drive on-screen UI — open apps, click, type, scroll, take screenshots, and reason over what you see
whenToUse: When the user asks to perform an action on the screen that isn't addressable through a more specific tool. Examples — "open Chrome and play my favorite song", "screenshot the current window", "click the Send button", "fill in this form", "scroll down to find X".
metadata:
  klyntbot:
    skill_type: orchestrator
    tools: ["computer_use", "launcher", "ask_user", "skill_reference"]
    mcp_tools: []
    triggers:
      - "open app"
      - "click"
      - "screenshot"
      - "take a screenshot"
      - "use my computer"
      - "control the computer"
      - "automate ui"
      - "fill in"
      - "scroll"
    summary: "Multi-step UI automation via screen perception + input injection"
---

You are the computer-use agent. You drive the user's screen via the `computer_use` tool. The tool lets you take screenshots, click, type, press keys, scroll, drag, and wait. You also have the `launcher` tool for known apps/files/URLs/window operations and `ask_user` for any clarification you need.

## Tool selection rule

- **Use `launcher` first** for any known app/file/URL/window operation. It is fast, deterministic, and free.
- **Use `computer_use` only** when you need to interact with unknown or dynamic UI elements on screen.
- **Always perceive first**: if you are uncertain about screen state, take a `screenshot` and reason over it before acting.

## Action vocabulary

The full vocabulary of `computer_use` actions is described in `references/action-vocabulary.md`. Call `skill_reference` with `action: "get_reference", skill_name: "computer-use", reference_name: "action-vocabulary"` to load it on demand.

The most common ones:

- `screenshot` — capture the current screen. Returns an image you can reason over.
- `left_click { x, y }` — click at pixel coordinates.
- `type { text }` — type text wherever the focus currently is.
- `key { keys: ["cmd", "l"] }` — press a key combination.
- `scroll { x, y, direction, amount }` — scroll at a location.

## Workflow pattern

1. Take a `screenshot` to see current state (or use `launcher` to deterministically open the target).
2. Locate the target element by reasoning over the image.
3. Issue the action (`left_click`, `type`, `key`, etc.).
4. Take another `screenshot` to verify the action had the intended effect.
5. Repeat until the user's goal is met.

## Coordinates

All coordinates are pixel-based, measured from the top-left of the active display in logical points (Retina displays are described in points, not physical pixels — the `screenshot` action accounts for the backing scale factor automatically). Use the screenshot's reported width/height to bound your coordinate choices.

## Be concise and visible

Narrate what you are doing in one short sentence per step. The user is watching. Do not silently take actions.

## Error handling

If an action fails or the screen does not change as expected:
1. Take a fresh `screenshot`.
2. Reason about why the previous step did not work (modal blocking, focus elsewhere, dynamic content not yet loaded).
3. Adjust and retry, or call `ask_user` if you are stuck.

Never repeat the same failing action more than twice in a row.
```

- [ ] **Step 2: Verify file exists**

Run: `ls -la skills/computer-use/SKILL.md`
Expected: file exists.

- [ ] **Step 3: Commit**

```bash
git add skills/computer-use/SKILL.md
git commit -m "feat(skills): computer-use orchestrator skill body"
```

---

### Task 27: `skills/computer-use/references/action-vocabulary.md`

**Files:**
- Create: `skills/computer-use/references/action-vocabulary.md`

- [ ] **Step 1: Author the reference**

```markdown
# Computer-use action vocabulary

The `computer_use` tool dispatches to one of these 16 actions via the `action` field. Each action is described below with its parameters and typical usage.

## Read-only / perception

- `screenshot { region?: [x, y, w, h] }` — capture the current screen. With no `region`, captures the active app's window. Returns a multipart result containing an image you can reason over.
- `mouse_move { x, y }` — move the cursor without clicking. Useful to hover over hover-only menus.

## Mouse clicks

- `left_click { x, y, modifiers? }` — primary click. `modifiers` is an array of any of `["cmd", "ctrl", "alt", "shift", "fn"]`.
- `double_click { x, y, modifiers? }`
- `triple_click { x, y, modifiers? }`
- `right_click { x, y }`
- `middle_click { x, y }`

## Manual mouse-button control

For when you need to hold the mouse button across multiple coordinates (e.g. drawing or fine-grained selection):

- `left_mouse_down { x, y }` — press and hold.
- `left_mouse_up { x, y }` — release.

For a complete drag, prefer `left_click_drag` (atomic).

## Keyboard

- `type { text }` — type literal characters. Modifiers are NOT applied — use `key` for shortcuts.
- `key { keys: ["cmd", "l"] }` — press a key combination once. Each entry is a key name: letters/digits, `enter`, `escape`, `tab`, `space`, `delete`, arrows (`up`/`down`/`left`/`right`), `f1`–`f12`, modifier names (`cmd`/`ctrl`/`alt`/`shift`/`fn`).
- `hold_key { keys: [...], duration_ms }` — press and hold for a duration.

## Drag

- `left_click_drag { from_x, from_y, to_x, to_y, modifiers? }` — atomic press → move → release.

## Scroll

- `scroll { x, y, direction, amount }` — `direction` is `"up"`/`"down"`/`"left"`/`"right"`; `amount` is in scroll ticks.

## Timing

- `wait { duration_ms }` — pause execution. Use to let animations or network requests complete before acting again.

## Zoom (deferred)

- `zoom { region: [x, y, w, h] }` — zoom into a region. Phase-1/2 platforms return a "not implemented" message; the agent should treat zoom as a hint that does nothing today.

## Coordinate conventions

- Origin is top-left of the active display.
- Coordinates are in logical points; the platform layer translates to physical pixels.
- A `screenshot` result reports `width × height` — clamp click coordinates within that range.

## Tips

- Always `screenshot` before clicking on dynamic UI.
- Prefer `key { keys: ["cmd", "l"] }` then `type` then `key { keys: ["enter"] }` over clicking the URL bar of a browser.
- For text fields you can't see clearly, `triple_click` selects existing content so a subsequent `type` replaces it.
```

- [ ] **Step 2: Verify file exists**

Run: `ls -la skills/computer-use/references/action-vocabulary.md`
Expected: file exists.

- [ ] **Step 3: Commit**

```bash
git add skills/computer-use/references/action-vocabulary.md
git commit -m "docs(skills): computer-use action-vocabulary reference"
```

---

### Task 28: Register `computer-use` in `compiled_skill_defaults`

**Files:**
- Modify: `crates/skill-system/src/defaults.rs:16-33`
- Test: integration.

- [ ] **Step 1: Write failing test**

Create `crates/skill-system/tests/computer_use_skill_seed.rs`:

```rust
use skill_system::compiled_skill_defaults;

#[test]
fn computer_use_skill_is_compiled_in() {
    let map = compiled_skill_defaults();
    let entries = map.get("computer-use").expect("computer-use must be registered");
    let names: Vec<&str> = entries.iter().map(|(p, _)| *p).collect();
    assert!(names.contains(&"SKILL.md"));
    assert!(names.contains(&"references/action-vocabulary.md"));
}

#[test]
fn computer_use_skill_md_has_required_frontmatter_fields() {
    let map = compiled_skill_defaults();
    let entries = map.get("computer-use").unwrap();
    let body = entries.iter().find(|(p, _)| *p == "SKILL.md").unwrap().1;
    assert!(body.contains("name: computer-use"));
    assert!(body.contains("description:"));
    assert!(body.contains("whenToUse:"));
    assert!(body.contains("skill_type: orchestrator"));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo nextest run -p skill-system --test computer_use_skill_seed`
Expected: FAIL.

- [ ] **Step 3: Add the registration**

In `crates/skill-system/src/defaults.rs`, append to `compiled_skill_defaults()` (before the final `map`):

```rust
    map.insert(
        "computer-use".to_string(),
        vec![
            ("SKILL.md", include_str!("../../../skills/computer-use/SKILL.md")),
            (
                "references/action-vocabulary.md",
                include_str!("../../../skills/computer-use/references/action-vocabulary.md"),
            ),
        ],
    );
```

If `store.rs` also has a `DEFAULT_SKILLS` flat array (the legacy parallel structure), add the same `("computer-use", include_str!("../../../skills/computer-use/SKILL.md"))` tuple there to keep both seed paths in sync.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p skill-system --test computer_use_skill_seed`
Expected: PASS.

- [ ] **Step 5: Run workspace build (verifies `include_str!` paths resolve)**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(skill-system): seed computer-use skill into compiled_skill_defaults"
```

---

### Task 29: End-to-end smoke — agent screenshots itself

**Files:**
- Create: `crates/agent/tests/screenshot_pipeline.rs`
- Test: gated behind `KLYNT_E2E_COMPUTER_USE` env (off by default).

- [ ] **Step 1: Write the smoke test**

```rust
//! Phase 2 smoke test — verifies the full pipeline:
//!   chat input → agent loop → ComputerUseTool::screenshot → multipart
//!   Message::Tool → next iteration sees the image.
//!
//! Mock-driven so it runs on any CI. The MockCapture returns a deterministic
//! 4x4 checkerboard frame; we assert the multipart sentinel decoded into a
//! Multipart Message::Tool with one ImageData part and one Text part.

use agent::events::AgentEvent;
use feature_computer_use::{ComputerUseFeature, ComputerUseToolDeps};
use platform_capture::mock::MockCapture;
use platform_input::mock::MockInput;
use providers::types::{ContentPart, Message, ToolResultContent};
use std::sync::Arc;

#[tokio::test]
async fn screenshot_action_round_trips_through_process_tool_result() {
    use tools_core::{FeaturePackage, RoutingContext, Tool};

    // 1. Build the tool from the feature package.
    let f = ComputerUseFeature::with_tool_deps(ComputerUseToolDeps {
        input: Arc::new(MockInput::new()),
        capture: Arc::new(MockCapture::checkerboard()),
    });
    let tool = f.tools().into_iter().next().expect("one tool");

    // 2. Build a routing context with a screenshot sidecar.
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut ctx = RoutingContext::default();
    ctx.screenshot_tx = Some(tx);
    ctx.tool_call_id = Some("toolu_test".to_string());

    // 3. Dispatch screenshot.
    let raw = tool
        .execute(serde_json::json!({"action": "screenshot"}), &ctx)
        .await
        .expect("screenshot ok");

    // 4. Convert via process_tool_result (the same path the agent uses).
    let content = agent::execution::core::process_tool_result(&raw);
    match content {
        ToolResultContent::Multipart(parts) => {
            assert_eq!(parts.len(), 2, "want text + image part");
            assert!(parts.iter().any(|p| matches!(p, ContentPart::Text { .. })));
            assert!(parts.iter().any(|p| matches!(p, ContentPart::ImageData { .. })));
        }
        ToolResultContent::Text(t) => panic!("got text-only: {t}"),
    }

    // 5. Verify sidecar emitted the screenshot.
    let evt = rx.recv().await.expect("sidecar event");
    assert_eq!(evt.tool_call_id, "toolu_test");
    assert_eq!(evt.format, "png");
    assert!(evt.width > 0);
    assert!(!evt.data.is_empty());

    // 6. Build a Message::Tool with the multipart and verify it serializes
    //    via the Anthropic adapter into an array-content tool_result.
    let msg = Message::tool("toolu_test", "computer_use", content);
    let provider = providers::adapters::anthropic_native::AnthropicProvider::for_test();
    let serialized = provider.serialize_message_for_test(&msg).expect("serialize");
    assert!(serialized["content"].is_array(), "tool result content must be array, got {serialized}");
}

#[tokio::test]
async fn agent_event_screenshot_captured_constructible() {
    let evt = common::ScreenshotEvent {
        tool_call_id: "x".to_string(),
        captured_at: jiff::Timestamp::UNIX_EPOCH,
        width: 1, height: 1, format: "png".to_string(), data: String::new(),
    };
    let _e = AgentEvent::ScreenshotCaptured(evt);
}
```

- [ ] **Step 2: Run smoke test**

Run: `cargo nextest run -p agent --test screenshot_pipeline`
Expected: PASS — both tests.

- [ ] **Step 3: Commit**

```bash
git add crates/agent/tests/screenshot_pipeline.rs
git commit -m "test(agent): Phase 2 screenshot pipeline smoke test"
```

---

### Task 30: Run full quality gates

**Files:** none (verification only).

- [ ] **Step 1: Workspace build**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 2: Clippy zero-warning**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: success.

- [ ] **Step 3: Workspace tests**

Run: `cargo nextest run --workspace`
Expected: all PASS.

- [ ] **Step 4: Doc tests**

Run: `cargo test --workspace --doc`
Expected: success.

- [ ] **Step 5: Format check**

Run: `cargo fmt --all --check`
Expected: clean. If dirty, run `cargo fmt --all` and commit:

```bash
git add -u
git commit -m "style: cargo fmt"
```

- [ ] **Step 6: Phase-1 invariants still hold**

Run: `cargo test -p desktop -E 'test(registration_drift) | test(bindings_are_current) | test(no_raw_tauri_command_outside_macros)'`
Expected: PASS.

> Phase 2 does not add new Tauri commands (the smoke test is purely Rust-side), so these tests should be unaffected. If they fail, check whether `RoutingContext`/`AppCore` field changes accidentally surfaced through `desktop_macros::klynt_collect_commands!`.

- [ ] **Step 7: Phase-1 macOS smoke still passes**

Run: `cargo test -p platform-macos --test computer_use_smoke`
Expected: PASS (skip path).

---

### Task 31: Phase 2 sign-off — update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (the design-status note for computer-use)

- [ ] **Step 1: Update the status note**

Locate the paragraph in `CLAUDE.md` that begins with `**Computer Use & Procedural Memory** (in design …)`. Replace it with:

```markdown
**Computer Use & Procedural Memory** (Phase 1 + Phase 2 landed; see [`docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md`](docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md)): Full-OS automation feature with hybrid AVR perception cascade (planned: Accessibility tree → local VLM → cloud VLM, all routed through `ProviderManager`), risk-tier safety gates with NSAlert + `AskUserTool` confirmation (planned for Phase 3), HUD overlay + side panel UI (planned for Phase 3), time-bound `ComputerUseSession` for background automation, and procedural memory (Intent → Stage → Action trajectories distilled into `web_tree_memories` for replay). Phase 1 shipped: `platform-input`/`platform-capture` trait crates at L0 with `MacInput`/`MacCapture` impls and an AX-tree walker. Phase 2 shipped: `feature-computer-use` crate with `ComputerUseTool` exposing all 16 ComputerUseAction variants; Anthropic adapter gained `ContentPart::ImageData`, `ToolResultContent`, `convert_tools` special-case for `computer_20251124`, and the `anthropic-beta: computer-use-2025-11-24` header; `MidLoopCompressor` is image-aware (latest screenshot preserved, older multiparts dropped); `RoutingContext::screenshot_tx` sidecar channel + `AgentEvent::ScreenshotCaptured` are wired (HUD consumer comes in Phase 3). Phase 3+ pending in `docs/superpowers/plans/`.
```

- [ ] **Step 2: Bump crate count**

Find the workspace summary section that names "(39 crates, 9 layers)". Phase 2 added one new crate (`feature-computer-use`). Update to "(40 crates, 9 layers)" and amend the L4 listing to include `feature-computer-use`.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude.md): bump crate count + record Phase 2 sign-off (computer-use tool surface + cloud path)"
```

---

## Phase 2 acceptance criteria

Phase 2 is complete when **all** of the following are true:

1. `cargo build --workspace` succeeds.
2. `cargo nextest run --workspace` succeeds.
3. `cargo clippy --workspace --all-targets -- -D warnings` reports zero warnings.
4. `cargo fmt --all --check` is clean.
5. `cargo test -p desktop -E 'test(registration_drift) | test(bindings_are_current) | test(no_raw_tauri_command_outside_macros)'` passes.
6. `feature-computer-use` exists, registers `ComputerUseTool` via `FeaturePackage::tools()`, and exposes every `ComputerUseAction` variant (16 actions including `screenshot` and a graceful `zoom`-not-implemented fallback).
7. `providers::types::ContentPart` has an `ImageData` variant; `Message::Tool.content` is `ToolResultContent`; `ProviderCapabilities::{computer_use, computer_use_version}` exist.
8. The Anthropic adapter:
   - emits `anthropic-beta: computer-use-2025-11-24` when any tool has `type: "computer_use"`,
   - converts `type: "computer_use"` tools into the `computer_20251124` block,
   - serializes `ContentPart::ImageData` as a base64 image source,
   - serializes multipart `Message::Tool` content as an array of blocks.
9. `MidLoopCompressor` preserves the latest multipart `Message::Tool` (the latest screenshot) verbatim and replaces older multiparts with a placeholder text.
10. `RoutingContext` carries `screenshot_tx` and `tool_call_id`; `core.rs::run_cycle` initializes the channel, drains it after `join_all`, and emits `AgentEvent::ScreenshotCaptured`.
11. The orchestrator skill `skills/computer-use/SKILL.md` plus its `references/action-vocabulary.md` are seeded into `compiled_skill_defaults()`.
12. `crates/agent/tests/screenshot_pipeline.rs` passes — proving the round trip from tool dispatch → multipart sentinel → `process_tool_result` → multipart `Message::Tool` → Anthropic-shape serialization.
13. `AppCore` carries `platform_input` and `platform_capture` `Option<Arc<…>>`; on macOS they hold `MacInput`/`MacCapture`, off-macOS they fall back to `MockInput`/`MockCapture` so the workspace builds cross-platform.

## What Phase 2 deliberately does NOT include

- Risk-tier classifier, scope locks, sensitive-surface patterns (Phase 3).
- HUD window, cursor overlay, action callouts, voice narration (Phase 3).
- Emergency-stop hotkey, `agent_action_log` table, screenshot blob storage (Phase 3).
- `Tool::execution_constraints()` / serial-only invariant — does not exist on the trait today; Phase 3 introduces it.
- Real screenshot downsampling for older multipart variants — Phase 2 drops with placeholder; Phase 4 adds genuine downsampling.
- Hybrid perception cascade, AX-first router, local VLM (Phase 4).
- OpenAI / Gemini adapter computer-use translations.
- `feature-browser-control` + CDP integration (Phase 5).
- `web_tree_memories` + procedural memory + replay (Phase 6).
- `WorkflowInductionSignals` mirror source + reforge phase (Phase 7).
- Settings UI section + side panel + skill cookbook (Phase 8).

Each is a separate plan file in `docs/superpowers/plans/`.
