# Requirements Validation Report: ask_user Tool

**Analyst:** Business Analyst
**Date:** 2026-02-13
**Plan:** `~/.claude/plans/iterative-inventing-beacon.md`

---

## 1. Plan Accuracy vs Codebase Reality

### Line References (All Verified Correct)

| Plan Claim | Actual Location | Status |
|---|---|---|
| Todo enrichment prompts at lines 167-278 | `todo.rs:167-278` | Correct |
| prompt_pending check at lines 517-545 | `agent_loop.rs:517-545` | Correct |
| format_user_response at lines 938-978 | `agent_loop.rs:936-978` | Close (off by 2 lines for fn signature) |
| handle_interactive_prompt at lines 342-399 | `chat.rs:342-399` | Correct |

### Type/Structure Mapping (All Verified Correct)

| Plan Claim | Codebase Location | Verified |
|---|---|---|
| PromptRequest, PromptType, PromptOption, PromptOptionWithInput, UserResponse | `common/src/prompts.rs:1-68` | Yes |
| RoutingContext.prompt_tx: Option<Sender<PromptRequest>> | `tools/src/lib.rs:46` | Yes |
| StreamingChannels {event_tx, user_rx, prompt_pending} | `agent_loop.rs:54-58` | Yes |
| AgentEvent::PromptUser(PromptRequest) | `events.rs:38` | Yes |
| process_direct_streaming returns (event_rx, user_tx, cancel_token, handle) | `agent_loop.rs:849-854` | Yes |
| agent/lib.rs re-exports PromptRequest, PromptType, UserResponse | `agent/src/lib.rs:24` | Yes |
| common/lib.rs re-exports all prompt types | `common/src/lib.rs:16` | Yes |

### Import Chain (All Verified)

- `chat.rs:3` — `agent::{AgentEvent, AgentLoop, UserResponse}`
- `chat.rs:6` — `common::prompts::PromptType`
- `todo.rs:14` — `common::{PromptOptionWithInput, PromptRequest, PromptType, Result, ToolError}`
- `agent_loop.rs:13` — `common::{PromptRequest, Result, UserResponse}`

**Conclusion:** The plan's understanding of the codebase is accurate. All references match.

---

## 2. Gaps, Ambiguities & Missing Requirements

### GAP-1: Parallel Tool Execution Conflict (MEDIUM RISK)

**Issue:** The `ask_user` tool blocks inside `execute()` on `response_rx.recv()`. However, tools execute in parallel via `join_all(tool_futures)` (agent_loop.rs:586-623, 736-772). If the LLM calls `ask_user` alongside other tools, `join_all` will block until ALL futures complete — meaning other tools finish while `ask_user` waits for user input, but the LLM won't see any results until the user answers.

**Impact:** Functionally correct (join_all naturally waits), but the LLM should be instructed to never call `ask_user` alongside other tools.

**Recommendation:** Add to the system prompt (Phase 8):
```
- IMPORTANT: Never call ask_user alongside other tools in the same turn.
  ask_user blocks until the user responds, so co-occurring tools would
  complete but their results would be delayed.
```

### GAP-2: Non-TTY Detection Underspecified (LOW RISK)

**Issue:** The plan says "Non-TTY fallback: return text-based questions as tool output." But the detection mechanism isn't specified.

**Recommendation:** When `prompt_tx` is `None` (no CLI prompt channel available), the tool should:
1. Format questions as a structured text block in the tool result string
2. Return immediately (no blocking)
3. Include an instruction telling the LLM to present the questions conversationally
4. The user's response arrives as normal text in the next turn

This is implied by the architecture (prompt_tx is None in bus mode) but should be explicitly documented in the ask_user tool implementation.

### GAP-3: cli/src/todo.rs Incorrectly Listed as Modified (NO RISK)

**Issue:** The plan lists `crates/cli/src/todo.rs` as needing updates for "direct CLI mode (klyntbot todo add)."

**Reality:** This file imports `prompt_select_with_input` from `crate::wizard::prompts` directly — it does NOT use the agent prompt system at all. No imports of PromptRequest/PromptType/UserResponse exist in this file. The wizard prompts (`crates/cli/src/wizard/prompts.rs`) are correctly listed as "NOT changed."

**Recommendation:** Remove `crates/cli/src/todo.rs` from the "Files to Modify" table and move it to "Files NOT Changed" with reason: "Uses wizard prompts directly, not the agent prompt system."

### GAP-4: ask_user Tool Parameter Schema Not Fully Specified (MEDIUM RISK)

**Issue:** The plan describes parameters as "title (string) and questions array (1-4 items)" but doesn't provide the complete JSON Schema. Since LLMs are sensitive to parameter schemas (they're passed as tool definitions), the schema needs to be precise.

**Recommendation:** The schema should be explicitly defined during implementation. Key structural decision: the `answer_type` field uses a tagged union. The JSON Schema should use `oneOf` with discriminator patterns. Example:

```json
{
  "type": "object",
  "properties": {
    "title": { "type": "string" },
    "questions": {
      "type": "array",
      "minItems": 1,
      "maxItems": 4,
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "title": { "type": "string", "maxLength": 20 },
          "text": { "type": "string" },
          "type": { "type": "string", "enum": ["single_select", "multi_select", "yes_no", "free_text"] },
          "options": { "type": "array", "items": { ... } },
          "default": {},
          "placeholder": { "type": "string" }
        },
        "required": ["id", "title", "text", "type"]
      }
    }
  },
  "required": ["title", "questions"]
}
```

### GAP-5: Affected Test Enumeration Missing (LOW RISK)

**Issue:** The plan says "some existing tests updated for new types" but doesn't enumerate which.

**Analysis of affected tests:**
- `crates/tools/src/todo.rs` tests (lines 608-970): Create `RoutingContext::new(...)` which sets `prompt_tx: None`. After changes, RoutingContext gains `response_rx` field — but `RoutingContext::new()` sets both to None. **These tests should compile and pass without modification.**
- Tests importing old prompt types (PromptRequest, etc.) from common will break at compile time — but these are being replaced, not updated in other test files.
- No other test files in the workspace import from `common::prompts` or `agent::UserResponse` based on the grep of imports.

**Recommendation:** Add explicit note: "Existing todo.rs tests should pass without changes since RoutingContext::new() defaults new fields to None."

### GAP-6: Channel Renderers Are Out of Scope (Clarification Needed)

**Issue:** The plan defines `InteractionRenderer` trait but only implements CLI renderer. The plan says "Design for all channels from the start" as a user decision.

**Recommendation:** The plan should explicitly state: "Channel-specific renderers (Telegram inline keyboards, Discord button components, etc.) are out of scope for this implementation. The InteractionRenderer trait is defined now so channels can implement it later without architectural changes."

---

## 3. Breaking Changes Validation

### Pre-Production Status: CONFIRMED

The git log shows this is an active development project with no release tags. The README and project structure confirm pre-production status. Breaking changes are acceptable.

### Breaking Change Inventory

| Change | Old | New | Impact |
|---|---|---|---|
| Prompt request type | `PromptRequest` | `InteractionRequest` | All prompt producers/consumers |
| User response type | `UserResponse` | `FormResponse` | Agent loop, CLI event handler |
| Event variant | `PromptUser(PromptRequest)` | `InteractionRequest(InteractionRequest)` | Event handlers |
| RoutingContext fields | `prompt_tx: Sender<PromptRequest>` | `prompt_tx: Sender<InteractionRequest>`, `response_rx: Arc<Mutex<Receiver<FormResponse>>>` | All tool contexts |
| Streaming return | 4-tuple | `StreamingHandle` struct | CLI chat handler |
| StreamingChannels | 3 fields | 1 field (event_tx only) | Internal to agent loop |
| Agent re-exports | PromptRequest, PromptType, UserResponse | StreamingHandle (others removed) | External consumers of agent crate |

**All consumers are internal to the workspace.** No external API surface is affected. **APPROVED.**

---

## 4. Test Scenarios

### TS-1: LLM Generates Dynamic Questions via ask_user

**Scenario:** LLM calls `ask_user` with a multi-question form
**Input:** JSON arguments with title, 2 questions (single_select + free_text)
**Expected:**
1. Tool parses arguments into `InteractionRequest`
2. Request sent via `prompt_tx`
3. Tool blocks on `response_rx`
4. After user responds, tool returns semantic string: `"User answered your questions:\n- Question1 → SelectedValue\n- Question2 → free text input"`
5. LLM receives this as a normal tool result

### TS-2: Semantic Response Formatting

**Scenario:** Various answer types produce correct semantic strings
**Cases:**
- `SingleSelect` → `"Priority → High: important tasks with deadline"`
- `MultiSelect` → `"Features → [Dark mode, Notifications]: selected two features"`
- `YesNo(true)` → `"Confirm → Yes"`
- `YesNo(false)` → `"Confirm → No"`
- `FreeText("custom input")` → `"Details → custom input"`
- `Skipped` → `"Priority → (skipped)"`
- `FormResponse::Cancelled` → `"User cancelled the form."`

### TS-3: Non-TTY Mode Fallback

**Scenario:** ask_user called when `prompt_tx` is None (non-interactive mode)
**Expected:**
1. Tool detects `prompt_tx` is None
2. Tool returns formatted text block as tool result:
   ```
   I need your input on the following questions. Please respond with your choices:

   1. Priority (single select): High, Medium, Low
   2. Details (free text): describe the task
   ```
3. LLM presents these conversationally to the user
4. User responds with text in next message
5. LLM interprets the response (no ask_user call needed)

### TS-4: Todo Enrichment Migration

**Scenario:** User creates a low-confidence todo in chat mode
**Before (old):** Todo tool directly emits PromptRequest with 5 hardcoded options, agent_loop waits on prompt_pending flag
**After (new):**
1. Todo tool returns confidence breakdown + instruction: "Use ask_user to help improve this task"
2. LLM reads the instruction and calls `ask_user` with dynamically generated questions based on missing fields
3. User answers via tabbed UI
4. LLM receives semantic answers and calls `todo.update` to improve the task

### TS-5: Tabbed UI Navigation

**Scenario:** User navigates a 3-question form with arrow keys
**Steps:**
1. Form renders with tabs: `[☐ Priority] [☐ Timeline] [☐ Tags] [✓ Submit]`
2. Left/Right arrow switches tabs
3. Up/Down navigates options within current question
4. Enter selects and auto-advances to next unanswered tab
5. After all answered, auto-advances to Submit tab
6. Enter on Submit tab returns `FormResponse::Completed`
7. Esc at any point returns `FormResponse::Cancelled`

### TS-6: Single Question Edge Case

**Scenario:** LLM sends ask_user with exactly 1 question
**Expected:** Tabbed UI still works (1 tab + Submit tab). No visual glitches.

### TS-7: Cancellation During ask_user

**Scenario:** User presses Ctrl+C while tabbed UI is active
**Expected:**
1. UI returns `FormResponse::Cancelled`
2. Tool returns `"User cancelled the form."`
3. LLM receives cancellation and acknowledges gracefully
4. Agent loop continues (not terminated)

### TS-8: ask_user Validation Errors

**Scenario:** LLM sends malformed ask_user parameters
**Cases:**
- Missing `title` → tool returns error
- Empty `questions` array → tool returns error
- More than 4 questions → tool returns error
- Question missing `id` → tool returns error
- `single_select` with no `options` → tool returns error

### TS-9: Direct CLI Todo Add Still Works

**Scenario:** `klyntbot todo add "write a story"` from command line
**Expected:**
1. `cli/src/todo.rs` handles this directly using `wizard::prompts` functions
2. NOT affected by ask_user changes at all
3. Same enrichment flow with 5 hardcoded options via `prompt_select_with_input`
4. Task created successfully with confidence scoring

### TS-10: Agent Loop Simplification Regression

**Scenario:** Bus-driven message processing (non-CLI channels)
**Expected:**
1. `StreamingChannels::none()` passed (only event_tx: None)
2. No prompt_pending, no user_rx — simplified struct compiles
3. Tool calls execute normally; ask_user returns non-TTY fallback text
4. No behavioral regression in Telegram/Discord/etc. message processing

### TS-11: Concurrent ask_user Prevention

**Scenario:** LLM attempts to call ask_user AND another tool in the same turn
**Expected:**
1. Both tools start executing in parallel
2. ask_user blocks on response_rx; other tool completes
3. join_all waits for both
4. User sees the form, responds
5. ask_user unblocks, join_all completes
6. Agent loop continues with both results
7. **Functionally correct** but suboptimal (other tool result delayed). System prompt should discourage this pattern.

---

## 5. Summary & Recommendations

### Overall Assessment: APPROVED WITH MINOR AMENDMENTS

The plan is thorough, architecturally sound, and accurately reflects the codebase. The core design — making `ask_user` a blocking tool that handles its own prompt/response cycle — elegantly eliminates the `prompt_pending`/`user_rx` complexity in the agent loop.

### Required Amendments

1. **Add system prompt instruction** about not calling ask_user with other tools (GAP-1)
2. **Remove `cli/src/todo.rs`** from "Files to Modify" (GAP-3)
3. **Document non-TTY fallback behavior** explicitly in the ask_user tool implementation (GAP-2)
4. **Add explicit note** that channel renderers are out of scope (GAP-6)

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| LLM calls ask_user with other tools | Medium | Low (works, just delayed) | System prompt instruction |
| ask_user schema doesn't produce good LLM outputs | Medium | Medium | Iterate on schema + description |
| Tabbed UI edge cases (terminal sizes, non-UTF8) | Low | Low | Non-TTY fallback exists |
| Existing tests break from type changes | High | Low | Compile-time errors, easy to fix |
| Todo enrichment quality degrades | Low | Medium | LLM now generates contextual questions, likely improvement |
