# Tool Usage Reference

## bash

Run shell commands. Always pass `cwd` explicitly.

```
bash(command="cargo test -p storage", cwd="/Users/me/project")
```

**Best practices:**
- Use `-n` / `--non-interactive` flags to avoid interactive prompts
- Pipe large output through `head` or `tail` if overwhelming
- Chain commands with `&&` for dependent steps
- Use `2>&1` to capture stderr

**Timeout:** Default 30s. Long-running commands (builds, tests) may need explicit timeout.

## read

Read file contents. Always `read` before `edit` — the edit tool requires exact string matching.

```
read(filePath="/path/to/file.rs", offset=100, limit=50)
```

## write

Create new files. Refuses to overwrite existing files without explicit user confirmation.

```
write(filePath="/path/to/new.ts", content="export function foo() {}")
```

## edit

Replace exact strings in existing files. Requires the `oldString` to match exactly.

```
edit(filePath="/path/to/file.rs", oldString="fn old_name()", newString="fn new_name()")
```

**Safety:** The edit fails if `oldString` is not found or matches multiple locations. Use more context to disambiguate.

## apply_patch

Apply unified diffs. Validates hunks before applying.

```
apply_patch(patch="--- a/file.rs\n+++ b/file.rs\n@@ -1,3 +1,3 @@\n-old\n+new")
```

## glob + grep

Search before reading. Find files by pattern, then search content.

```
glob(pattern="**/*.rs", path="/project")
grep(pattern="fn handle_event", path="/project", include="*.rs")
```

## recall_turns

Look up prior coding turns to avoid duplicating work or re-discovering context.

```
recall_turns(query="implemented auth middleware", limit=5)
```

## enter_plan_mode / exit_plan_mode

For multi-step work, present a plan before executing.

```
enter_plan_mode(plan="1. Add User struct\n2. Add DB migration\n3. Add handler")
# ... get user approval ...
exit_plan_mode()
```
