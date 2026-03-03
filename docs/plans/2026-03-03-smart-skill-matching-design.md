# Smart Skill Matching Design

## Problem

The current skill system has no intelligent matching. All available skills are dumped into the transparency panel regardless of relevance. When a user asks "What tasks do we have?", the panel shows `cron` and `test-skill` instead of `todo`. The `triggers` field in SKILL.md frontmatter is parsed but never used at runtime — it's only advisory metadata in the XML summary.

## Requirements

- **Only matched skills in transparency** — show skills relevant to the current message, not all loaded skills
- **Keyword-based trigger matching** — fast, zero API cost, deterministic
- **Match-based content loading** — matched skills get full SKILL.md content injected into system prompt, even if `always=false`
- **Integrated into IntentAnalyzer** — skill matching is part of intent analysis, enabling future tool-group-aware skill routing

## Architecture

### 1. IntentAnalysis Extension

Add `matched_skills: Vec<String>` to `IntentAnalysis` in `crates/agent/src/intent_pipeline/types.rs`.

### 2. Trigger Matching in IntentAnalyzer

New function in `analysis.rs`:

```rust
fn match_skill_triggers(msg: &str, skills: &[Skill]) -> Vec<String> {
    let lower = msg.to_lowercase();
    let mut matched: Vec<String> = skills.iter()
        .filter(|s| s.available)
        .filter(|s| {
            s.always || s.triggers.iter().any(|t| lower.contains(&t.to_lowercase()))
        })
        .map(|s| s.name.clone())
        .collect();
    matched.dedup();
    matched
}
```

`IntentAnalyzer` gains `Arc<SkillManager>` at construction (injected in `AgentLoopBuilder::build()`).

Called in both `analyze_heuristic()` and `analyze()` (LLM path) to populate `matched_skills`.

### 3. Two-Phase Content Loading

- **Phase 1 (pre-analysis):** `SkillContentSource` still loads `always: true` skills into the system prompt as before. No change.
- **Phase 2 (post-analysis, pre-LLM):** In `pipeline.rs`, after `IntentAnalyzer::analyze()` returns, if `matched_skills` contains skills that are NOT `always: true`, their full content is appended to the assembled messages as `Message::system(...)` before the LLM call.

New `SkillManager` methods:
- `get_skill_content(name: &str) -> Option<String>` — returns a skill's full body
- `is_always_loaded(name: &str) -> bool` — checks if a skill has `always: true`

### 4. Transparency Events — Only Matched Skills

Move `SkillLoaded` event emission from BEFORE `process_message()` to AFTER it returns. Only emit for skills in `matched_skills`:

```rust
// In run_pipeline(), AFTER pipeline.process_message():
if let Some(ref tx) = event_tx {
    for skill_name in &result.matched_skills {
        if let Some(skill) = self.skill_manager.get(skill_name) {
            let trigger = if skill.always {
                "always".to_string()
            } else {
                skill.triggers.join(", ")
            };
            let _ = tx.send(AgentEvent::SkillLoaded {
                name: skill_name.clone(),
                trigger,
            }).await;
        }
    }
}
```

The `PipelineResult` (or equivalent return type from `process_message()`) needs to carry `matched_skills: Vec<String>`.

### 5. Enriched Trigger Keywords

Expand skill trigger lists for better coverage:

**todo:**
```
["todo", "task", "tasks", "focus", "create a task", "add a task", "my tasks", "task list", "what tasks", "check tasks", "list tasks"]
```

**cron:**
```
["cron", "schedule", "reminder", "recurring", "every day", "every hour", "every minute", "remind me"]
```

**daily-planning:**
```
["daily plan", "plan", "morning plan", "focus", "what should I focus on", "plan my day", "today's plan"]
```

## Files to Modify

| File | Change |
|------|--------|
| `crates/agent/src/intent_pipeline/types.rs` | Add `matched_skills: Vec<String>` to `IntentAnalysis` |
| `crates/agent/src/intent_pipeline/analysis.rs` | Add `match_skill_triggers()`, inject `Arc<SkillManager>` into `IntentAnalyzer`, populate `matched_skills` |
| `crates/agent/src/intent_pipeline/pipeline.rs` | Inject matched skill content post-analysis, return `matched_skills` in result |
| `crates/agent/src/skills.rs` | Add `get_skill_content()`, `is_always_loaded()`, `get()` methods |
| `crates/agent/src/agent_loop/mod.rs` | Move `SkillLoaded` events after pipeline, filter to matched only |
| `crates/agent/src/agent_loop/builder.rs` | Pass `Arc<SkillManager>` to `IntentAnalyzer` constructor |
| `skills/todo/SKILL.md` | Expand triggers list |
| `skills/cron/SKILL.md` | Add triggers in metadata |
| `skills/daily-planning/SKILL.md` | Expand triggers list |

## UI Impact

No changes to `TransparencyPanel.tsx` or `useAgentStream.ts` — they already render whatever `SkillLoaded` events arrive. The filtering happens server-side.

## Testing

- Unit test `match_skill_triggers()` with various message patterns
- Unit test that `always: true` skills always appear in matched_skills
- Integration test: send task message, verify only `todo` in transparency events
- Integration test: send schedule message, verify only `cron` in transparency events
- Integration test: send greeting, verify no skills matched (or empty)
