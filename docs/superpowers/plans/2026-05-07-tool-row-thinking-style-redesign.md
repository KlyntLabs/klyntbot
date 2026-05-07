# Tool-Row Thinking-Style Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the rounded-card `.tool-inline` rendering of assistant tool calls with a thinking-block-style row (flat neutral bar, 2px family-coloured left border, status-only leading icon, `Action: argument` label, click-to-expand body, live bash tail, auto-expand on error, burst-grouping for ≥3 consecutive same-tool calls).

**Architecture:** Presentation-only rebuild. The wire model (`ConversationItem.kind: "tool"`), the `outputDelta` streaming pipeline (`threadItemsSlice.ts:314`), and the `expandedItems` Set state (`useMessagesViewState.ts:50`) are unchanged. We add a pure descriptor (`toolRowDescriptor`) and a pure grouper (`groupBursts`); rewrite `ToolRow` end-to-end into smaller files (`ToolRow.tsx`, `ToolRowBody.tsx`, `BashTail.tsx`, `BurstRow.tsx`); add `--tool-family-*` design tokens; delete the old `.tool-inline*` CSS block; and add a single new effect in `useMessagesViewState.ts` for auto-expand-on-error.

**Tech Stack:** React 19, TypeScript (strict), Vitest + @testing-library/react, plain CSS (no Tailwind), BEM-ish class naming. Existing path aliases: `@/`, `@app/`, `@services/`, `@utils/`. Lint via `bun run lint` (ESLint).

**Spec:** `docs/superpowers/specs/2026-05-07-tool-row-thinking-style-redesign.md`

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `desktop-ui/src/styles/ds-tokens.css` | Modify | Add nine `--tool-family-*` tokens at `:root` aliasing existing `--cm-color-*-fg` values |
| `desktop-ui/src/styles/themes.dark.css` | Modify | Theme-specific overrides for the new family tokens (dark) |
| `desktop-ui/src/styles/themes.light.css` | Modify | Theme-specific overrides for the new family tokens (light) |
| `desktop-ui/src/styles/themes.dim.css` | Modify | Theme-specific overrides for the new family tokens (dim) |
| `desktop-ui/src/styles/messages.css` | Modify | Delete old `.tool-inline*` (lines 532–910) + `.tool-group*` (912–1020); add new `.tool-row*` block |
| `desktop-ui/src/features/messages/utils/messageRenderUtils.ts` | Modify | Add `toolRowDescriptor`, `ToolFamily`, `ToolRowDescriptor` types |
| `desktop-ui/src/features/messages/utils/messageRenderUtils.test.ts` | Modify | New table-driven tests for `toolRowDescriptor` |
| `desktop-ui/src/features/messages/utils/groupBursts.ts` | Create | Pure function: collapse ≥3 consecutive same-family + same-name `kind: "tool"` items into `BurstGroup` virtuals |
| `desktop-ui/src/features/messages/utils/groupBursts.test.ts` | Create | Tests for grouper |
| `desktop-ui/src/features/messages/components/BashTail.tsx` | Create | 3-line live stdout strip below a running shell row |
| `desktop-ui/src/features/messages/components/BashTail.test.tsx` | Create | Tests for tail |
| `desktop-ui/src/features/messages/components/ToolRowBody.tsx` | Create | Per-`toolType` expanded body dispatcher (file content, diff, terminal output, plan markdown, etc.) |
| `desktop-ui/src/features/messages/components/ToolRowBody.test.tsx` | Create | Tests for body dispatcher |
| `desktop-ui/src/features/messages/components/BurstRow.tsx` | Create | Wrapper that renders a `BurstGroup` with header + on-expand sub-rows |
| `desktop-ui/src/features/messages/components/BurstRow.test.tsx` | Create | Tests for burst |
| `desktop-ui/src/features/messages/components/MessageRows.tsx` | Modify | Replace `ToolRow` body (lines 668–861); update `toolIconForSummary`; refactor `ExploreRow` (867–900) onto new shell |
| `desktop-ui/src/features/messages/components/MessageRows.test.tsx` (or `Messages.test.tsx`) | Modify | Add tests for `ToolRow` family colour, spinner, error auto-expand, AskUser persistence |
| `desktop-ui/src/features/messages/components/Messages.tsx` | Modify | Run `groupBursts` over visible items before dispatch; render `BurstRow` for `BurstGroup` virtuals (lines 213–229 dispatch case) |
| `desktop-ui/src/features/messages/components/useMessagesViewState.ts` | Modify | Add auto-expand-on-error effect mirroring the plan effect at lines 202–224 |
| `desktop-ui/src/features/messages/components/useMessagesViewState.test.ts` (new if not present) | Create | Test for the new auto-expand effect |

**Existing surfaces we DO NOT modify** (intentionally — wire compat):
- `desktop-ui/src/utils/threadItems.conversion.ts` — keeps emitting `title: "Tool: <server>/<tool>"`. Parsing happens in `toolRowDescriptor`.
- `desktop-ui/src/features/threads/hooks/threadReducer/threadItemsSlice.ts:314` — `appendToolOutput` reducer is correct already.
- `desktop-ui/src/utils/appServerEvents.ts` — wire event names are unchanged.
- `desktop-ui/src/types.ts` — `ConversationItem` shape is unchanged.

---

## Reference: tool family colour palette

Tokens added to `ds-tokens.css` `:root`:

```css
--tool-family-filesystem: var(--cm-color-blue-fg);
--tool-family-shell: var(--cm-color-orange-fg);
--tool-family-search: var(--cm-color-green-fg);
--tool-family-web: var(--cm-color-purple-fg);
--tool-family-domain: var(--cm-color-indigo-fg);
--tool-family-agent: var(--cm-color-pink-fg);
--tool-family-mcp: var(--cm-color-teal-fg);
--tool-family-system: var(--text-subtle);
--tool-family-approval: var(--cm-color-amber-fg);
```

(These already exist in `themes.dark.css:91–101`. The token aliases give us a stable layer to override per theme.)

---

## Task 0: Branch + worktree setup

**Files:** none (env)

- [ ] **Step 1: Verify clean working tree**

Run: `git status`
Expected: `nothing to commit, working tree clean`

- [ ] **Step 2: Create feature branch**

Run: `git checkout -b feature/tool-row-thinking-style`
Expected: `Switched to a new branch 'feature/tool-row-thinking-style'`

- [ ] **Step 3: Verify spec exists**

Run: `ls docs/superpowers/specs/2026-05-07-tool-row-thinking-style-redesign.md`
Expected: file path printed (no error)

---

## Task 1: Add `--tool-family-*` design tokens

**Files:**
- Modify: `desktop-ui/src/styles/ds-tokens.css`

- [ ] **Step 1: Read current `:root` block in ds-tokens.css to find insertion point**

Run: `grep -n "status-error\|status-warning" desktop-ui/src/styles/ds-tokens.css`
Expected: lines around 81–83 (the existing status tokens). Insert family tokens immediately after.

- [ ] **Step 2: Add family tokens after `--status-unknown` line**

Insert after the line containing `--status-unknown:`:

```css
  /* Tool-row family colours — referenced by .tool-row left-border.
     Each family aliases a CodeMirror color FG token so themes only
     need to override the --cm-color-*-fg values. */
  --tool-family-filesystem: var(--cm-color-blue-fg);
  --tool-family-shell: var(--cm-color-orange-fg);
  --tool-family-search: var(--cm-color-green-fg);
  --tool-family-web: var(--cm-color-purple-fg);
  --tool-family-domain: var(--cm-color-indigo-fg);
  --tool-family-agent: var(--cm-color-pink-fg);
  --tool-family-mcp: var(--cm-color-teal-fg);
  --tool-family-system: var(--text-subtle);
  --tool-family-approval: var(--cm-color-amber-fg);
```

- [ ] **Step 3: Verify Vite picks up the change**

Run: `cd desktop-ui && bun run typecheck`
Expected: no errors (CSS-only change; typecheck unaffected but acts as a smoke test)

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/ds-tokens.css
git commit -m "$(cat <<'EOF'
feat(ui): add --tool-family-* design tokens

Nine new tokens aliasing existing --cm-color-*-fg values so the
tool-row redesign can reference family colours without coupling to
specific hex values. Themes already define the underlying CM colours
in dark/light/dim, so no theme overrides are required yet.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit succeeds.

---

## Task 2: Define `ToolFamily` + `ToolRowDescriptor` types

**Files:**
- Modify: `desktop-ui/src/features/messages/utils/messageRenderUtils.ts` (after line 23, before `SCROLL_THRESHOLD_PX`)

- [ ] **Step 1: Add type definitions**

Insert near the top of the file (after the existing `MessageImage` type at line 23):

```ts
export type ToolFamily =
  | "filesystem"
  | "shell"
  | "search"
  | "web"
  | "domain"
  | "agent"
  | "mcp"
  | "system"
  | "approval";

export type ToolRowDescriptor = {
  family: ToolFamily;
  /** Display name in the header — capitalised, no trailing colon. */
  name: string;
  /** Primary argument shown after the name (path, command, query, action verb). */
  arg: string;
  /** Optional right-side meta fragments joined with " · " when rendered. */
  meta: string[];
};
```

- [ ] **Step 2: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: 0 errors. Types are unused at this point — that's fine; subsequent tasks consume them.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/messages/utils/messageRenderUtils.ts
git commit -m "feat(ui): add ToolFamily and ToolRowDescriptor types

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: TDD `toolRowDescriptor` — base cases (commandExecution, fileChange)

**Files:**
- Modify: `desktop-ui/src/features/messages/utils/messageRenderUtils.test.ts`
- Modify: `desktop-ui/src/features/messages/utils/messageRenderUtils.ts`

- [ ] **Step 1: Write failing test for `commandExecution` → shell family**

Append to `messageRenderUtils.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { toolRowDescriptor } from "./messageRenderUtils";
import type { ConversationItem } from "@/types";

function toolItem(overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>): Extract<
  ConversationItem,
  { kind: "tool" }
> {
  return {
    id: "t1",
    kind: "tool",
    toolType: "commandExecution",
    title: "",
    detail: "",
    status: "completed",
    output: "",
    ...overrides,
  };
}

describe("toolRowDescriptor", () => {
  it("maps commandExecution to shell family with command in arg", () => {
    const item = toolItem({
      toolType: "commandExecution",
      title: "Command: cargo nextest run -p agent",
      durationMs: 2400,
      status: "completed",
    });
    expect(toolRowDescriptor(item)).toEqual({
      family: "shell",
      name: "Bash",
      arg: "cargo nextest run -p agent",
      meta: ["2.4s"],
    });
  });

  it("classifies grep/glob/rg as search family", () => {
    const item = toolItem({
      toolType: "commandExecution",
      title: "Command: rg --type ts AgentEvent",
      status: "completed",
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("search");
    expect(desc.name).toBe("Grep");
  });
});
```

- [ ] **Step 2: Run test — expect import failure**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: fails with `toolRowDescriptor is not a function` or `not exported`.

- [ ] **Step 3: Implement minimal `toolRowDescriptor` for shell + search**

Append to `messageRenderUtils.ts`:

```ts
const SHELL_SEARCH_TOOLS = ["grep", "glob", "rg", "ripgrep", "fd", "find"];

function classifyShellCommand(command: string): "search" | "shell" {
  const head = command.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  return SHELL_SEARCH_TOOLS.includes(head) ? "search" : "shell";
}

function formatDurationCompact(ms: number | null | undefined): string | null {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return null;
  if (ms < 1000) return `${ms}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  const minutes = Math.floor(seconds / 60);
  const rem = Math.round(seconds % 60);
  return `${minutes}m${rem.toString().padStart(2, "0")}s`;
}

export function toolRowDescriptor(
  item: Extract<ConversationItem, { kind: "tool" }>,
): ToolRowDescriptor {
  if (item.toolType === "commandExecution") {
    const command = item.title.replace(/^Command:\s*/i, "").trim() || "command";
    const family = classifyShellCommand(command);
    const meta: string[] = [];
    const dur = formatDurationCompact(item.durationMs ?? null);
    if (dur) meta.push(dur);
    return {
      family,
      name: family === "search" ? "Grep" : "Bash",
      arg: command,
      meta,
    };
  }
  // Stub for other types — filled in subsequent tasks.
  return { family: "system", name: item.title || "Tool", arg: "", meta: [] };
}
```

- [ ] **Step 4: Run test — expect pass**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: 2 passing tests for `toolRowDescriptor`.

- [ ] **Step 5: Add fileChange test**

Append to `messageRenderUtils.test.ts`:

```ts
  it("maps fileChange (single edit) to filesystem · Edit with diff stats", () => {
    const item = toolItem({
      toolType: "fileChange",
      title: "File changes",
      detail: "src/lib.rs",
      status: "completed",
      changes: [
        {
          path: "src/lib.rs",
          kind: "edit",
          diff: "@@ -1,2 +1,3 @@\n a\n-b\n+B\n+c",
        },
      ],
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("filesystem");
    expect(desc.name).toBe("Edit");
    expect(desc.arg).toBe("src/lib.rs");
    expect(desc.meta).toContain("+2 −1");
  });

  it("maps fileChange (write) to Write with line count", () => {
    const item = toolItem({
      toolType: "fileChange",
      changes: [
        {
          path: "new.ts",
          kind: "add",
          diff: "@@ -0,0 +1,3 @@\n+a\n+b\n+c",
        },
      ],
    });
    const desc = toolRowDescriptor(item);
    expect(desc.name).toBe("Write");
    expect(desc.meta).toContain("+3");
  });

  it("maps fileChange (read) to Read with optional range meta", () => {
    const item = toolItem({
      toolType: "fileChange",
      changes: [{ path: "src/a.ts", kind: "read", diff: "" }],
    });
    expect(toolRowDescriptor(item).name).toBe("Read");
  });
```

- [ ] **Step 6: Run — expect failure (fileChange branch missing)**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: 3 new tests fail.

- [ ] **Step 7: Implement fileChange branch**

Replace the placeholder return in `toolRowDescriptor` with logic; add helper `summarizeDiffStats` above:

```ts
function summarizeDiffStats(diff: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of diff.split("\n")) {
    if (line.startsWith("+") && !line.startsWith("+++")) added += 1;
    else if (line.startsWith("-") && !line.startsWith("---")) removed += 1;
  }
  return { added, removed };
}

function fileChangeName(kind: string | undefined): "Read" | "Write" | "Edit" | "Patch" {
  switch (kind) {
    case "read":
      return "Read";
    case "add":
    case "write":
      return "Write";
    case "apply_patch":
    case "notebook_edit":
      return "Patch";
    default:
      return "Edit";
  }
}
```

Then add this branch inside `toolRowDescriptor` after the `commandExecution` block:

```ts
  if (item.toolType === "fileChange") {
    const change = item.changes?.[0];
    const path = change?.path ?? item.detail ?? "";
    const name = fileChangeName(change?.kind);
    const meta: string[] = [];
    if (change?.diff) {
      const { added, removed } = summarizeDiffStats(change.diff);
      if (name === "Write" && added > 0) meta.push(`+${added}`);
      else if (name !== "Read" && (added > 0 || removed > 0))
        meta.push(`+${added} −${removed}`);
    }
    if ((item.changes?.length ?? 0) > 1) {
      meta.push(`${item.changes!.length} files`);
    }
    return { family: "filesystem", name, arg: path, meta };
  }
```

- [ ] **Step 8: Run — expect pass**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: all 5 `toolRowDescriptor` tests pass.

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src/features/messages/utils/messageRenderUtils.ts desktop-ui/src/features/messages/utils/messageRenderUtils.test.ts
git commit -m "feat(ui): toolRowDescriptor for commandExecution and fileChange

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: TDD `toolRowDescriptor` — webSearch, mcpToolCall, collabToolCall, hook, contextCompaction, imageView, plan

**Files:**
- Modify: `desktop-ui/src/features/messages/utils/messageRenderUtils.test.ts`
- Modify: `desktop-ui/src/features/messages/utils/messageRenderUtils.ts`

- [ ] **Step 1: Add tests for remaining toolTypes**

Append to `messageRenderUtils.test.ts`:

```ts
  it("maps webSearch to web family", () => {
    const item = toolItem({
      toolType: "webSearch",
      title: "Web search",
      detail: "anthropic computer use",
      status: "completed",
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("web");
    expect(desc.name).toBe("WebSearch");
    expect(desc.arg).toBe("anthropic computer use");
  });

  it("maps mcpToolCall klyntbot/* to domain family", () => {
    const item = toolItem({
      toolType: "mcpToolCall",
      title: "Tool: klyntbot / tasks",
      detail: '{"action":"create","title":"Ship redesign"}',
      status: "completed",
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("domain");
    expect(desc.name).toBe("Tasks");
    expect(desc.arg).toBe("create");
    expect(desc.meta.join(" ")).toContain("Ship redesign");
  });

  it("maps mcpToolCall non-klyntbot to mcp family", () => {
    const item = toolItem({
      toolType: "mcpToolCall",
      title: "Tool: github / list_pull_requests",
      detail: '{"owner":"anthropics","repo":"claude-code"}',
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("mcp");
    expect(desc.name).toBe("github");
    expect(desc.arg).toBe("list_pull_requests");
  });

  it("maps collabToolCall to agent family with subagent type as arg", () => {
    const item = toolItem({
      toolType: "collabToolCall",
      title: "collab: spawn",
      detail: "Explore",
      status: "in_progress",
      durationMs: 18000,
    });
    const desc = toolRowDescriptor(item);
    expect(desc.family).toBe("agent");
    expect(desc.name).toBe("Agent");
  });

  it("maps hook to system family", () => {
    const item = toolItem({
      toolType: "hook",
      title: "Hook: PreToolUse",
      detail: "block",
      status: "completed",
    });
    expect(toolRowDescriptor(item).family).toBe("system");
  });

  it("maps contextCompaction to system family", () => {
    const item = toolItem({
      toolType: "contextCompaction",
      title: "Context compaction",
      status: "completed",
    });
    expect(toolRowDescriptor(item).family).toBe("system");
  });

  it("maps imageView to system Image", () => {
    const item = toolItem({
      toolType: "imageView",
      title: "Image view",
      detail: "/tmp/screenshot.png",
    });
    const desc = toolRowDescriptor(item);
    expect(desc.name).toBe("Image");
    expect(desc.family).toBe("system");
  });

  it("maps plan to domain family", () => {
    const item = toolItem({
      toolType: "plan",
      title: "Plan",
      output: "Step 1…",
    });
    expect(toolRowDescriptor(item).family).toBe("domain");
    expect(toolRowDescriptor(item).name).toBe("Plan");
  });

  it("returns system fallback for unknown toolType", () => {
    const item = toolItem({ toolType: "weird_unknown", title: "Some tool" });
    expect(toolRowDescriptor(item).family).toBe("system");
  });
```

- [ ] **Step 2: Run — expect failures**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: 9 new tests fail.

- [ ] **Step 3: Implement remaining branches**

Add helpers above `toolRowDescriptor`:

```ts
function parseMcpTitle(title: string): { server: string; tool: string } {
  // Format produced by threadItems.conversion.ts:146 — "Tool: server / tool"
  const match = title.match(/^Tool:\s*([^\s/]+)\s*(?:\/\s*(.+))?$/i);
  if (!match) return { server: "", tool: title };
  return { server: match[1] ?? "", tool: (match[2] ?? "").trim() };
}

const KLYNTBOT_MCP_SERVERS = new Set(["klyntbot", "klynt", "klyntcoach"]);

function capitalize(input: string): string {
  if (!input) return input;
  return input[0].toUpperCase() + input.slice(1);
}

function summarizeMcpArgs(detail: string): string[] {
  const args = parseToolArgs(detail);
  if (!args) return [];
  const interesting: string[] = [];
  for (const key of ["title", "name", "query", "path", "id"]) {
    const v = args[key];
    if (typeof v === "string" && v.trim()) {
      interesting.push(`${key}=${v.length > 40 ? `${v.slice(0, 40)}…` : v}`);
      break;
    }
  }
  return interesting;
}
```

Add branches to `toolRowDescriptor` (insert before the final fallback):

```ts
  if (item.toolType === "webSearch") {
    return {
      family: "web",
      name: "WebSearch",
      arg: item.detail || "",
      meta: [],
    };
  }
  if (item.toolType === "mcpToolCall") {
    const { server, tool } = parseMcpTitle(item.title);
    const isKlyntbot = KLYNTBOT_MCP_SERVERS.has(server.toLowerCase());
    const args = parseToolArgs(item.detail);
    if (isKlyntbot) {
      const action = (args && typeof args.action === "string" ? args.action : "") || tool || "";
      return {
        family: "domain",
        name: capitalize(tool || "Tool"),
        arg: action,
        meta: summarizeMcpArgs(item.detail),
      };
    }
    return {
      family: "mcp",
      name: server || "mcp",
      arg: tool || "",
      meta: summarizeMcpArgs(item.detail),
    };
  }
  if (item.toolType === "collabToolCall" || item.toolType === "collabAgentToolCall") {
    return {
      family: "agent",
      name: "Agent",
      arg: item.detail || item.title || "",
      meta: formatDurationCompact(item.durationMs ?? null) ? [formatDurationCompact(item.durationMs ?? null)!] : [],
    };
  }
  if (item.toolType === "hook") {
    return {
      family: "system",
      name: "Hook",
      arg: item.title.replace(/^Hook:\s*/i, "").trim() || "",
      meta: item.detail ? [item.detail] : [],
    };
  }
  if (item.toolType === "contextCompaction") {
    return { family: "system", name: "Context", arg: "compacted", meta: [] };
  }
  if (item.toolType === "imageView") {
    return { family: "system", name: "Image", arg: item.detail || "", meta: [] };
  }
  if (item.toolType === "plan") {
    return { family: "domain", name: "Plan", arg: "", meta: [] };
  }
```

- [ ] **Step 4: Run — expect pass**

Run: `cd desktop-ui && bun run test -- messageRenderUtils`
Expected: all 14 `toolRowDescriptor` tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/utils/messageRenderUtils.ts desktop-ui/src/features/messages/utils/messageRenderUtils.test.ts
git commit -m "feat(ui): toolRowDescriptor handles all 9 toolTypes

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: TDD `groupBursts` pure function

**Files:**
- Create: `desktop-ui/src/features/messages/utils/groupBursts.ts`
- Create: `desktop-ui/src/features/messages/utils/groupBursts.test.ts`

- [ ] **Step 1: Write failing tests**

Create `groupBursts.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { groupBursts, type BurstGroup } from "./groupBursts";

function tool(
  id: string,
  toolType: string,
  title: string,
  status: string = "completed",
  detail: string = "",
): Extract<ConversationItem, { kind: "tool" }> {
  return { id, kind: "tool", toolType, title, detail, status, output: "" };
}

function read(id: string, path: string) {
  return tool(id, "fileChange", "File changes", "completed", path);
}

describe("groupBursts", () => {
  it("returns input unchanged when no group of 3+ exists", () => {
    const items: ConversationItem[] = [read("a", "x.ts"), read("b", "y.ts")];
    expect(groupBursts(items)).toEqual(items);
  });

  it("collapses 3 consecutive same-family same-name reads", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      read("c", "z.ts"),
    ];
    const out = groupBursts(items);
    expect(out).toHaveLength(1);
    const burst = out[0] as BurstGroup;
    expect(burst.kind).toBe("burst");
    expect(burst.items).toHaveLength(3);
    expect(burst.family).toBe("filesystem");
    expect(burst.name).toBe("Edit"); // default fileChange name
  });

  it("breaks a group when a failed tool appears in the middle", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      tool("c", "fileChange", "File changes", "failed", "z.ts"),
      read("d", "p.ts"),
      read("e", "q.ts"),
      read("f", "r.ts"),
    ];
    const out = groupBursts(items);
    // a,b are <3 so kept, failed escapes, d,e,f group
    expect(out.map((x) => ("kind" in x && x.kind === "burst" ? "burst" : x.id))).toEqual([
      "a",
      "b",
      "c",
      "burst",
    ]);
  });

  it("does not group across different families", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      tool("c", "commandExecution", "Command: ls"),
      read("d", "z.ts"),
    ];
    const out = groupBursts(items);
    expect(out).toHaveLength(4);
  });

  it("does not group different names within same family", () => {
    // A read followed by edits — same filesystem family but different `name`
    const reads = [read("a", "x.ts"), read("b", "y.ts")];
    const writes: ConversationItem[] = [
      {
        id: "c",
        kind: "tool",
        toolType: "fileChange",
        title: "File changes",
        detail: "",
        status: "completed",
        output: "",
        changes: [{ path: "n.ts", kind: "add", diff: "@@ +1 @@\n+x" }],
      },
    ];
    expect(groupBursts([...reads, ...writes])).toHaveLength(3);
  });
});
```

- [ ] **Step 2: Run — expect import failure**

Run: `cd desktop-ui && bun run test -- groupBursts`
Expected: failures (file does not exist yet).

- [ ] **Step 3: Implement `groupBursts.ts`**

```ts
import type { ConversationItem } from "@/types";
import { toolRowDescriptor, type ToolFamily } from "./messageRenderUtils";

export type BurstGroup = {
  id: string;
  kind: "burst";
  family: ToolFamily;
  name: string;
  items: Array<Extract<ConversationItem, { kind: "tool" }>>;
};

export type GroupedItem = ConversationItem | BurstGroup;

const MIN_BURST_SIZE = 3;

function isFailed(item: Extract<ConversationItem, { kind: "tool" }>): boolean {
  return /(fail|error)/i.test(item.status ?? "");
}

export function groupBursts(items: ConversationItem[]): GroupedItem[] {
  const out: GroupedItem[] = [];
  let i = 0;
  while (i < items.length) {
    const item = items[i];
    if (item.kind !== "tool" || isFailed(item)) {
      out.push(item);
      i += 1;
      continue;
    }
    const baseDesc = toolRowDescriptor(item);
    let j = i;
    while (j < items.length) {
      const candidate = items[j];
      if (candidate.kind !== "tool") break;
      if (isFailed(candidate)) break;
      const desc = toolRowDescriptor(candidate);
      if (desc.family !== baseDesc.family || desc.name !== baseDesc.name) break;
      j += 1;
    }
    const runLength = j - i;
    if (runLength >= MIN_BURST_SIZE) {
      const groupItems = items.slice(i, j) as Array<
        Extract<ConversationItem, { kind: "tool" }>
      >;
      out.push({
        id: `burst-${groupItems[0].id}`,
        kind: "burst",
        family: baseDesc.family,
        name: baseDesc.name,
        items: groupItems,
      });
    } else {
      for (let k = i; k < j; k += 1) out.push(items[k]);
    }
    i = j;
  }
  return out;
}
```

- [ ] **Step 4: Run — expect pass**

Run: `cd desktop-ui && bun run test -- groupBursts`
Expected: 5 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/utils/groupBursts.ts desktop-ui/src/features/messages/utils/groupBursts.test.ts
git commit -m "feat(ui): pure groupBursts collapses >=3 same-tool calls

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Add new `.tool-row*` CSS

**Files:**
- Modify: `desktop-ui/src/styles/messages.css`

> Strategy: add the new block at the end of the file. Old `.tool-inline*` is removed in Task 13 once consumers are migrated. This keeps the UI working between commits.

- [ ] **Step 1: Append the new CSS block**

Append to `desktop-ui/src/styles/messages.css`:

```css
/* ─────────────────────────────────────────────────────────────
   tool-row — thinking-style inline tool call rendering.
   See docs/superpowers/specs/2026-05-07-tool-row-thinking-style-redesign.md
   ───────────────────────────────────────────────────────────── */

.tool-row {
  display: flex;
  align-items: stretch;
  gap: 0;
  margin: 4px 0;
  border-radius: 0 6px 6px 0;
  background: var(--surface-raised);
  border-left: 2px solid var(--tool-row-bar, var(--border-subtle));
  min-width: 0;
}

.tool-row--filesystem { --tool-row-bar: var(--tool-family-filesystem); }
.tool-row--shell      { --tool-row-bar: var(--tool-family-shell); }
.tool-row--search     { --tool-row-bar: var(--tool-family-search); }
.tool-row--web        { --tool-row-bar: var(--tool-family-web); }
.tool-row--domain     { --tool-row-bar: var(--tool-family-domain); }
.tool-row--agent      { --tool-row-bar: var(--tool-family-agent); }
.tool-row--mcp        { --tool-row-bar: var(--tool-family-mcp); }
.tool-row--system     { --tool-row-bar: var(--tool-family-system); }
.tool-row--approval   { --tool-row-bar: var(--tool-family-approval); }
.tool-row--failed     { --tool-row-bar: var(--status-error); }

.tool-row__toggle {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 10px;
  padding: 8px 12px 8px 14px;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  font: inherit;
  color: var(--text-stronger);
  min-width: 0;
}

.tool-row__toggle:disabled {
  cursor: default;
}

.tool-row__toggle:focus-visible {
  outline: 1px solid var(--border-strong);
  outline-offset: -1px;
  border-radius: 0 6px 6px 0;
}

.tool-row__icon {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--tool-row-bar);
}

.tool-row__name {
  font-weight: 600;
  font-size: var(--fs-sm);
  color: var(--text-strong);
  flex: 0 0 auto;
}

.tool-row__arg {
  font-family: var(--code-font-family);
  font-size: var(--code-font-size, 11px);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1 1 auto;
}

.tool-row__meta {
  margin-left: auto;
  font-size: var(--fs-xs);
  color: var(--text-faint);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}

.tool-row__meta-sep {
  opacity: 0.5;
}

.tool-row__chevron {
  flex: 0 0 auto;
  color: var(--text-subtle);
  font-size: 10px;
  transition: transform 0.15s ease;
}

.tool-row.is-expanded .tool-row__chevron {
  transform: rotate(90deg);
}

.tool-row.is-running .tool-row__chevron {
  visibility: hidden;
}

.tool-row__spinner {
  width: 11px;
  height: 11px;
  border: 1.5px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: tool-row-spin 0.7s linear infinite;
}

@keyframes tool-row-spin {
  to { transform: rotate(360deg); }
}

@keyframes tool-row-pulse-border {
  0%, 100% { border-left-color: var(--tool-family-approval); }
  50%      { border-left-color: var(--cm-color-amber-fg); opacity: 0.7; }
}

.tool-row.is-askuser {
  animation: tool-row-pulse-border 1.4s infinite;
}

/* Body shell — appears only when expanded. Inherits family colour
   on its own left edge to anchor it to the parent row. */
.tool-row__body {
  margin: 0 0 4px 14px;
  padding: 8px 12px;
  background: var(--surface-command);
  border-left: 2px solid var(--tool-row-bar, var(--border-subtle));
  border-radius: 0 6px 6px 0;
  font-size: var(--fs-xs);
  color: var(--text-quiet);
  max-height: 360px;
  overflow: auto;
}

.tool-row__body--code {
  font-family: var(--code-font-family);
  font-size: var(--code-font-size, 11px);
  white-space: pre-wrap;
  line-height: 1.55;
}

.tool-row__body--diff {
  padding: 0;
}

.tool-row--failed .tool-row__body {
  background: color-mix(in srgb, var(--status-error) 10%, var(--surface-command));
  color: var(--cm-color-danger-fg);
}

/* Live tail — 3-line stdout strip below the row while a shell call
   is running. Bound to the row visually via the same family border. */
.tool-row__tail {
  margin: -2px 0 4px 14px;
  padding: 6px 12px;
  background: var(--cm-surface-command-panel);
  border-left: 2px solid var(--tool-row-bar, var(--border-subtle));
  border-radius: 0 6px 6px 0;
  font-family: var(--code-font-family);
  font-size: var(--code-font-size, 11px);
  color: var(--text-quiet);
  line-height: 1.45;
  max-height: calc(1.45em * 3 + 12px);
  overflow: hidden;
}

.tool-row__tail-line {
  white-space: pre-wrap;
  word-break: break-word;
}

.tool-row__tail-line--dim {
  color: var(--text-faint);
}

/* Burst grouping — header row + indented sub-rows when expanded. */
.tool-row.is-burst .tool-row__name {
  font-weight: 600;
}

.tool-row__burst-children {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: -2px 0 4px 26px;
  padding: 4px 0 4px 12px;
  border-left: 2px solid var(--tool-row-bar, var(--border-subtle));
}

.tool-row__burst-children .tool-row {
  background: transparent;
  border-left: none;
  border-radius: 0;
  margin: 0;
}
```

- [ ] **Step 2: Verify CSS imported**

Run: `grep -n "messages.css" desktop-ui/src/styles/index.css`
Expected: line containing `@import "./messages.css"` exists.

- [ ] **Step 3: Boot the dev server briefly to confirm CSS parses**

Run: `cd desktop-ui && bun run typecheck`
Expected: 0 errors. (Vite would catch CSS syntax errors at runtime; typecheck doesn't, but it confirms no TS regressions.)

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/messages.css
git commit -m "feat(ui): add .tool-row CSS block (thinking-style)

Adds the new presentational rules without removing the legacy
.tool-inline* block. Old + new co-exist until ToolRow is migrated;
old block is deleted in a later commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: TDD `BashTail` component

**Files:**
- Create: `desktop-ui/src/features/messages/components/BashTail.tsx`
- Create: `desktop-ui/src/features/messages/components/BashTail.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BashTail } from "./BashTail";

describe("BashTail", () => {
  it("renders the last 3 non-empty lines", () => {
    const output = ["line 1", "line 2", "line 3", "line 4", "line 5"].join("\n");
    render(<BashTail output={output} />);
    expect(screen.queryByText("line 1")).toBeNull();
    expect(screen.queryByText("line 2")).toBeNull();
    expect(screen.getByText("line 3")).toBeInTheDocument();
    expect(screen.getByText("line 4")).toBeInTheDocument();
    expect(screen.getByText("line 5")).toBeInTheDocument();
  });

  it("renders nothing when output is empty", () => {
    const { container } = render(<BashTail output="" />);
    expect(container.firstChild).toBeNull();
  });

  it("preserves whitespace on each line", () => {
    render(<BashTail output={"  indented\nstart\n"} />);
    expect(screen.getByText(/indented/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run — expect failure**

Run: `cd desktop-ui && bun run test -- BashTail`
Expected: import error.

- [ ] **Step 3: Implement `BashTail.tsx`**

```tsx
import { memo, useMemo } from "react";

type BashTailProps = {
  output: string;
};

const TAIL_LINES = 3;

export const BashTail = memo(function BashTail({ output }: BashTailProps) {
  const lastLines = useMemo(() => {
    if (!output) return [];
    const lines = output.split(/\r?\n/);
    const trimmed = lines.length > 0 && lines[lines.length - 1] === "" ? lines.slice(0, -1) : lines;
    return trimmed.slice(-TAIL_LINES);
  }, [output]);

  if (lastLines.length === 0) return null;

  return (
    <div className="tool-row__tail" role="log" aria-live="polite">
      {lastLines.map((line, index) => (
        <div
          key={`tail-${index}-${line.slice(0, 16)}`}
          className={`tool-row__tail-line${index < lastLines.length - 1 ? " tool-row__tail-line--dim" : ""}`}
        >
          {line || " "}
        </div>
      ))}
    </div>
  );
});
```

- [ ] **Step 4: Run — expect pass**

Run: `cd desktop-ui && bun run test -- BashTail`
Expected: 3 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/BashTail.tsx desktop-ui/src/features/messages/components/BashTail.test.tsx
git commit -m "feat(ui): BashTail renders last 3 stdout lines

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: TDD `ToolRowBody` dispatcher

**Files:**
- Create: `desktop-ui/src/features/messages/components/ToolRowBody.tsx`
- Create: `desktop-ui/src/features/messages/components/ToolRowBody.test.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { ToolRowBody } from "./ToolRowBody";

function tool(overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>) {
  return {
    id: "t1",
    kind: "tool" as const,
    toolType: "commandExecution",
    title: "",
    detail: "",
    status: "completed",
    output: "",
    ...overrides,
  };
}

describe("ToolRowBody", () => {
  it("renders command output in code style for commandExecution", () => {
    const item = tool({
      toolType: "commandExecution",
      title: "Command: echo hi",
      output: "hi\n",
    });
    render(<ToolRowBody item={item} />);
    expect(screen.getByText(/hi/)).toBeInTheDocument();
  });

  it("renders 'No output' placeholder when output is empty", () => {
    const item = tool({ toolType: "commandExecution", title: "Command: true", output: "" });
    render(<ToolRowBody item={item} />);
    expect(screen.getByText(/No output/i)).toBeInTheDocument();
  });

  it("renders fileChange diff via PierreDiffBlock when changes present", () => {
    const item = tool({
      toolType: "fileChange",
      title: "File changes",
      changes: [
        { path: "a.ts", kind: "edit", diff: "@@ -1 +1 @@\n-a\n+b" },
      ],
    });
    render(<ToolRowBody item={item} />);
    // PierreDiffBlock renders diff content; we just confirm something containing the path renders
    expect(screen.getByText(/a\.ts/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run — expect import failure**

Run: `cd desktop-ui && bun run test -- ToolRowBody`
Expected: failure.

- [ ] **Step 3: Implement `ToolRowBody.tsx`**

```tsx
import { memo } from "react";
import { PierreDiffBlock } from "@/features/git/components/PierreDiffBlock";
import type { ConversationItem } from "@/types";
import { Markdown } from "./Markdown";

type ToolRowBodyProps = {
  item: Extract<ConversationItem, { kind: "tool" }>;
};

export const ToolRowBody = memo(function ToolRowBody({ item }: ToolRowBodyProps) {
  if (item.toolType === "commandExecution") {
    const output = item.output ?? "";
    if (!output.trim()) {
      return <div className="tool-row__body">No output.</div>;
    }
    return <div className="tool-row__body tool-row__body--code">{output}</div>;
  }

  if (item.toolType === "fileChange") {
    const changes = item.changes ?? [];
    if (changes.length === 0) {
      return item.detail ? (
        <div className="tool-row__body tool-row__body--code">{item.detail}</div>
      ) : null;
    }
    return (
      <div className="tool-row__body tool-row__body--diff">
        {changes.map((c) => (
          <div key={`${c.path}-${c.kind ?? ""}`}>
            <div className="tool-row__body--diff-path">{c.path}</div>
            {c.diff && <PierreDiffBlock diff={c.diff} displayPath={c.path} />}
          </div>
        ))}
      </div>
    );
  }

  if (item.toolType === "plan") {
    const text = (item.output ?? "").trim();
    if (!text) return null;
    return (
      <div className="tool-row__body">
        <Markdown value={text} className="markdown" />
      </div>
    );
  }

  // mcpToolCall / collabToolCall / hook / contextCompaction / imageView /
  // webSearch — render output (string) when present.
  const output = (item.output ?? "").trim();
  if (!output) {
    return item.detail ? (
      <div className="tool-row__body tool-row__body--code">{item.detail}</div>
    ) : null;
  }
  return <div className="tool-row__body tool-row__body--code">{output}</div>;
});
```

- [ ] **Step 4: Run — expect pass**

Run: `cd desktop-ui && bun run test -- ToolRowBody`
Expected: 3 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/ToolRowBody.tsx desktop-ui/src/features/messages/components/ToolRowBody.test.tsx
git commit -m "feat(ui): ToolRowBody dispatcher per toolType

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: TDD `BurstRow` component

**Files:**
- Create: `desktop-ui/src/features/messages/components/BurstRow.tsx`
- Create: `desktop-ui/src/features/messages/components/BurstRow.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConversationItem } from "@/types";
import type { BurstGroup } from "../utils/groupBursts";
import { BurstRow } from "./BurstRow";

function read(id: string, path: string): Extract<ConversationItem, { kind: "tool" }> {
  return {
    id,
    kind: "tool",
    toolType: "fileChange",
    title: "File changes",
    detail: path,
    status: "completed",
    output: "",
    changes: [{ path, kind: "read", diff: "" }],
  };
}

function makeBurst(): BurstGroup {
  return {
    id: "burst-a",
    kind: "burst",
    family: "filesystem",
    name: "Read",
    items: [read("a", "x.ts"), read("b", "y.ts"), read("c", "z.ts"), read("d", "p.ts"), read("e", "q.ts")],
  };
}

describe("BurstRow", () => {
  it("renders header showing first 3 paths plus +N more", () => {
    render(
      <BurstRow group={makeBurst()} expandedItems={new Set()} onToggle={vi.fn()} />,
    );
    expect(screen.getByText(/Read:/)).toBeInTheDocument();
    expect(screen.getByText(/x\.ts/)).toBeInTheDocument();
    expect(screen.getByText(/\+2 more/)).toBeInTheDocument();
  });

  it("expands sub-rows when group id is in expandedItems", () => {
    render(
      <BurstRow group={makeBurst()} expandedItems={new Set(["burst-a"])} onToggle={vi.fn()} />,
    );
    expect(screen.getByText(/p\.ts/)).toBeInTheDocument();
    expect(screen.getByText(/q\.ts/)).toBeInTheDocument();
  });

  it("calls onToggle with group id on click", () => {
    const onToggle = vi.fn();
    render(<BurstRow group={makeBurst()} expandedItems={new Set()} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole("button", { name: /toggle/i }));
    expect(onToggle).toHaveBeenCalledWith("burst-a");
  });
});
```

- [ ] **Step 2: Run — expect failure**

Run: `cd desktop-ui && bun run test -- BurstRow`
Expected: failure.

- [ ] **Step 3: Implement `BurstRow.tsx`**

```tsx
import { memo } from "react";
import type { BurstGroup } from "../utils/groupBursts";
import { ToolRow } from "./MessageRows";
import { toolRowDescriptor } from "../utils/messageRenderUtils";

const PREVIEW_PATHS = 3;

type BurstRowProps = {
  group: BurstGroup;
  expandedItems: Set<string>;
  onToggle: (id: string) => void;
};

function basenameOf(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export const BurstRow = memo(function BurstRow({ group, expandedItems, onToggle }: BurstRowProps) {
  const expanded = expandedItems.has(group.id);
  const previewArgs = group.items
    .slice(0, PREVIEW_PATHS)
    .map((item) => basenameOf(toolRowDescriptor(item).arg))
    .filter(Boolean);
  const remaining = group.items.length - previewArgs.length;
  const preview = previewArgs.join(" · ") + (remaining > 0 ? ` · +${remaining} more` : "");

  return (
    <>
      <div
        className={`tool-row tool-row--${group.family} is-burst${expanded ? " is-expanded" : ""}`}
      >
        <button
          type="button"
          className="tool-row__toggle"
          aria-label={`Toggle ${group.name} burst`}
          aria-expanded={expanded}
          onClick={() => onToggle(group.id)}
        >
          <span className="tool-row__icon" aria-hidden />
          <span className="tool-row__name">{group.name}:</span>
          <span className="tool-row__arg">
            {group.items.length} {group.name === "Read" ? "files" : "items"}
          </span>
          <span className="tool-row__meta">{preview}</span>
          <span className="tool-row__chevron" aria-hidden>
            ▸
          </span>
        </button>
      </div>
      {expanded && (
        <div className="tool-row__burst-children">
          {group.items.map((item) => (
            <ToolRow
              key={item.id}
              item={item}
              isExpanded={expandedItems.has(item.id)}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </>
  );
});
```

- [ ] **Step 4: Run — expect pass (or compile failure if `ToolRow` not yet refactored)**

Run: `cd desktop-ui && bun run test -- BurstRow`
Expected: import resolves; test runs. If `ToolRow` signature differs, defer the test until after Task 10 — comment-out the `expand sub-rows` and `onToggle` tests temporarily and revisit. **Preferred:** complete Task 10 first, then run this test set.

If running before Task 10, skip Step 4 and instead:

Run: `cd desktop-ui && bun run typecheck`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/BurstRow.tsx desktop-ui/src/features/messages/components/BurstRow.test.tsx
git commit -m "feat(ui): BurstRow renders >=3 same-tool calls as a single header

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Rewrite `ToolRow` end-to-end

**Files:**
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx` (lines 668–861)

- [ ] **Step 1: Add imports at top of `MessageRows.tsx`**

Insert after the existing imports block (after line 38):

```ts
import { BashTail } from "./BashTail";
import { ToolRowBody } from "./ToolRowBody";
import { toolRowDescriptor } from "../utils/messageRenderUtils";
```

- [ ] **Step 2: Replace the entire `ToolRow` definition (lines 668–861)**

Delete lines 668–861 inclusive. Insert in their place:

```tsx
const ASKUSER_TOOLTYPES = new Set(["mcpToolCall"]);

function isAskUser(item: Extract<ConversationItem, { kind: "tool" }>): boolean {
  if (!ASKUSER_TOOLTYPES.has(item.toolType)) return false;
  return /\bask_user\b/i.test(item.title);
}

export const ToolRow = memo(function ToolRow({
  item,
  isExpanded,
  onToggle,
  onRequestAutoScroll,
}: ToolRowProps) {
  const desc = toolRowDescriptor(item);
  const tone = toolStatusTone(item, (item.changes?.length ?? 0) > 0);
  const askUser = isAskUser(item);
  const isRunning = tone === "processing";
  const isFailed = tone === "failed";

  // Bash live tail — gated on existing 600ms warm-up + 1.2s long-running.
  const isCommand = item.toolType === "commandExecution";
  const durationMs = typeof item.durationMs === "number" ? item.durationMs : null;
  const isLongRunning = durationMs !== null && durationMs >= 1200;
  const [tailWarm, setTailWarm] = useState(false);
  useEffect(() => {
    if (!isRunning || !isCommand) {
      setTailWarm(false);
      return;
    }
    const handle = window.setTimeout(() => setTailWarm(true), 600);
    return () => window.clearTimeout(handle);
  }, [isCommand, isRunning]);

  const showTail =
    isCommand &&
    isRunning &&
    (item.output ?? "").length > 0 &&
    (tailWarm || isLongRunning) &&
    !isExpanded;

  useEffect(() => {
    if (showTail) onRequestAutoScroll?.();
  }, [showTail, onRequestAutoScroll]);

  const family = isFailed ? "failed" : desc.family;
  const className = [
    "tool-row",
    `tool-row--${family}`,
    isExpanded ? "is-expanded" : "",
    isRunning ? "is-running" : "",
    askUser ? "is-askuser" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const handleClick = () => {
    if (askUser || isRunning) return;
    onToggle(item.id);
  };

  return (
    <>
      <div className={className}>
        <button
          type="button"
          className="tool-row__toggle"
          aria-label={`Toggle ${desc.name} details`}
          aria-expanded={isExpanded}
          disabled={askUser || isRunning}
          onClick={handleClick}
        >
          <span className="tool-row__icon" aria-hidden>
            {isRunning ? (
              <span className="tool-row__spinner" />
            ) : isFailed ? (
              <X size={11} aria-hidden />
            ) : null}
          </span>
          <span className="tool-row__name">{desc.name}:</span>
          {desc.arg && <span className="tool-row__arg">{desc.arg}</span>}
          {desc.meta.length > 0 && (
            <span className="tool-row__meta">
              {desc.meta.map((fragment, idx) => (
                <span key={`meta-${idx}`}>
                  {idx > 0 && <span className="tool-row__meta-sep">·</span>}
                  {fragment}
                </span>
              ))}
            </span>
          )}
          <span className="tool-row__chevron" aria-hidden>
            ▸
          </span>
        </button>
      </div>
      {showTail && <BashTail output={item.output ?? ""} />}
      {isExpanded && <ToolRowBody item={item} />}
    </>
  );
});
```

- [ ] **Step 3: Remove now-unused imports**

`toolIconForSummary`, `Wrench`, `FileDiffIcon`, `FileText`, `Image`, `Search`, `Terminal`, `Users`, `Diff`, `buildToolSummary`, `cleanCommandText`, `MAX_COMMAND_OUTPUT_LINES`, `formatToolStatusLabel` are no longer used by `ToolRow`. Audit which are still referenced by `ExploreRow`, `CommandOutput`, `MessageRow`, `UserInputRow`. Keep:
- `Check`, `Copy`, `Quote`, `X` — used elsewhere
- `Terminal` — still used by `ExploreRow` (for now; refactored in Task 12)
- `MAX_COMMAND_OUTPUT_LINES` — used by `CommandOutput` (kept until Task 13)
- `formatToolStatusLabel` — only used by old `ToolRow`; remove
- `buildToolSummary` — only used by old `ToolRow`; remove
- `cleanCommandText` — only used by old `ToolRow`; remove
- `Wrench`, `FileDiffIcon`, `FileText`, `Image`, `Search`, `Users`, `Diff` — only used by old `toolIconForSummary`; remove
- `toolIconForSummary` function (lines 260–295) — delete entirely

Also delete the unused `buildPlanExportFileName` helper (lines 297–308) and `exportMarkdownFile`, `pushErrorToast`, `ToolSummary` imports — verify with grep before deleting.

Run after each removal: `cd desktop-ui && bun run typecheck`
Expected: 0 errors after each step.

- [ ] **Step 4: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: 0 errors.

- [ ] **Step 5: Run all unit tests**

Run: `cd desktop-ui && bun run test -- messages`
Expected: pre-existing tests pass; `BurstRow` tests now pass.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/messages/components/MessageRows.tsx
git commit -m "feat(ui): rewrite ToolRow as thinking-style row

Replaces the rounded-card .tool-inline rendering with a flat bar +
2px family-coloured left border + Action: arg label + status-only
leading icon + click-to-expand body. AskUser and running rows are
non-toggleable. Bash 600ms warm-up + 1.2s long-running thresholds
are preserved; long output collapses to a 3-line tail via BashTail.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Wire `Messages.tsx` to use `groupBursts` + `BurstRow`

**Files:**
- Modify: `desktop-ui/src/features/messages/components/Messages.tsx`

- [ ] **Step 1: Add imports**

Insert near existing imports:

```ts
import { groupBursts } from "../utils/groupBursts";
import { BurstRow } from "./BurstRow";
```

- [ ] **Step 2: Apply grouping inside the render loop**

Find `{visibleItems.map(renderItem)}` (line 263) and replace with:

```tsx
{groupBursts(visibleItems).map((entry) => {
  if ("kind" in entry && entry.kind === "burst") {
    return (
      <BurstRow
        key={entry.id}
        group={entry}
        expandedItems={expandedItems}
        onToggle={toggleExpanded}
      />
    );
  }
  return renderItem(entry);
})}
```

- [ ] **Step 3: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: 0 errors.

- [ ] **Step 4: Run unit tests**

Run: `cd desktop-ui && bun run test -- messages`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/Messages.tsx
git commit -m "feat(ui): apply burst grouping in Messages render loop

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Auto-expand-on-error effect

**Files:**
- Modify: `desktop-ui/src/features/messages/components/useMessagesViewState.ts`

- [ ] **Step 1: Write failing test for the effect**

Append to `useMessagesViewState.test.ts` (create the file if it does not exist):

```tsx
import { renderHook, act } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { useMessagesViewState } from "./useMessagesViewState";

function makeFailedTool(id: string): ConversationItem {
  return {
    id,
    kind: "tool",
    toolType: "commandExecution",
    title: "Command: tsc",
    detail: "",
    status: "failed",
    output: "error TS2322",
  };
}

describe("useMessagesViewState — auto-expand on error", () => {
  it("auto-expands a failed tool on first appearance", () => {
    const initial: ConversationItem[] = [];
    const { result, rerender } = renderHook(
      ({ items }: { items: ConversationItem[] }) =>
        useMessagesViewState({
          items,
          threadId: "t1",
          isThinking: false,
          activeUserInputRequestId: null,
          hasVisibleUserInputRequest: false,
        }),
      { initialProps: { items: initial } },
    );

    expect(result.current.expandedItems.has("err-1")).toBe(false);
    rerender({ items: [makeFailedTool("err-1")] });
    expect(result.current.expandedItems.has("err-1")).toBe(true);
  });

  it("does not re-expand after a manual collapse", () => {
    const failed = makeFailedTool("err-2");
    const { result, rerender } = renderHook(
      ({ items }: { items: ConversationItem[] }) =>
        useMessagesViewState({
          items,
          threadId: "t1",
          isThinking: false,
          activeUserInputRequestId: null,
          hasVisibleUserInputRequest: false,
        }),
      { initialProps: { items: [failed] } },
    );

    expect(result.current.expandedItems.has("err-2")).toBe(true);
    act(() => result.current.toggleExpanded("err-2"));
    expect(result.current.expandedItems.has("err-2")).toBe(false);
    rerender({ items: [failed] });
    expect(result.current.expandedItems.has("err-2")).toBe(false);
  });
});
```

- [ ] **Step 2: Run test — expect failure**

Run: `cd desktop-ui && bun run test -- useMessagesViewState`
Expected: failure (effect not yet in place).

- [ ] **Step 3: Add the effect to `useMessagesViewState.ts`**

Insert after the existing plan-auto-expand effect (after line 224):

```ts
  // Auto-expand-on-error: when a tool transitions to a failed status, add its
  // id to expandedItems on first observation. Mirrors the plan-expand effect
  // above and respects manual user collapse via manuallyToggledExpandedRef.
  useEffect(() => {
    setExpandedItems((prev) => {
      let next: Set<string> | null = null;
      for (const item of visibleItems) {
        if (item.kind !== "tool") continue;
        const status = (item.status ?? "").toLowerCase();
        if (!/(fail|error)/.test(status)) continue;
        if (manuallyToggledExpandedRef.current.has(item.id)) continue;
        if (prev.has(item.id)) continue;
        if (!next) next = new Set(prev);
        next.add(item.id);
      }
      return next ?? prev;
    });
  }, [visibleItems]);
```

- [ ] **Step 4: Run test — expect pass**

Run: `cd desktop-ui && bun run test -- useMessagesViewState`
Expected: 2 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/useMessagesViewState.ts desktop-ui/src/features/messages/components/useMessagesViewState.test.ts
git commit -m "feat(ui): auto-expand failed tool rows on first render

Mirrors the existing plan-auto-expand effect, gated on the
manuallyToggledExpandedRef so a user collapse is sticky.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Refactor `ExploreRow` onto the new shell

**Files:**
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx` (lines ~867–900)
- Modify: `desktop-ui/src/styles/messages.css` (add `.tool-row__explore-list*`)

- [ ] **Step 1: Replace `ExploreRow` body**

Replace the existing `ExploreRow` (lines 867–900):

```tsx
export const ExploreRow = memo(function ExploreRow({ item }: ExploreRowProps) {
  const isProcessing = item.status === "exploring";
  return (
    <div className={`tool-row tool-row--search${isProcessing ? " is-running" : ""}`}>
      <div className="tool-row__toggle" aria-disabled="true">
        <span className="tool-row__icon" aria-hidden>
          {isProcessing ? <span className="tool-row__spinner" /> : null}
        </span>
        <span className="tool-row__name">{isProcessing ? "Exploring" : "Explored"}:</span>
        <span className="tool-row__arg tool-row__explore-list">
          {item.entries.map((entry) => (
            <span
              key={`${entry.kind}-${entry.label}-${entry.detail ?? ""}`}
              className="tool-row__explore-item"
            >
              <span className="tool-row__explore-kind">{exploreKindLabel(entry.kind)}</span>
              <span className="tool-row__explore-label">{entry.label}</span>
            </span>
          ))}
        </span>
      </div>
    </div>
  );
});
```

- [ ] **Step 2: Add helper styles to `messages.css`**

Append to `.tool-row*` block in `messages.css`:

```css
.tool-row__explore-list {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  white-space: normal;
  font-family: inherit;
}
.tool-row__explore-item {
  display: inline-flex;
  align-items: baseline;
  gap: 4px;
}
.tool-row__explore-kind {
  font-weight: 600;
  color: var(--text-subtle);
}
.tool-row__explore-label {
  color: var(--text-stronger);
  overflow-wrap: anywhere;
}
```

- [ ] **Step 3: Run typecheck + tests**

Run: `cd desktop-ui && bun run typecheck && bun run test -- messages`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/messages/components/MessageRows.tsx desktop-ui/src/styles/messages.css
git commit -m "feat(ui): port ExploreRow onto .tool-row shell

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 14: Delete legacy `.tool-inline*` and `CommandOutput`

**Files:**
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx`
- Modify: `desktop-ui/src/styles/messages.css`

- [ ] **Step 1: Verify nothing references the old classes/component**

Run: `grep -rn "tool-inline\|CommandOutput" desktop-ui/src --include='*.ts' --include='*.tsx' --include='*.css'`
Expected: only the definitions remain (no consumers). `UserInputRow` still uses `.tool-inline*`; that's intentionally NOT migrated in this plan.

If `UserInputRow` is the only consumer:
- Confirm the class names it uses (`tool-inline`, `user-input-inline`, `tool-inline-bar-toggle`, `tool-inline-content`, `tool-inline-summary`, `tool-inline-toggle`, `tool-inline-icon completed`, `tool-inline-label`, `tool-inline-value`, `user-input-inline-preview`, `user-input-inline-details`, `user-input-inline-entry`, `user-input-inline-question`, `user-input-inline-answers`, `user-input-inline-answer`, `user-input-inline-empty-answer`).
- Keep ONLY those rules. Delete every other `.tool-inline*`, `.tool-group*`, `.tool-inline-terminal*` rule (lines ~532–910 + 912–1020).

- [ ] **Step 2: Delete `CommandOutput` from `MessageRows.tsx`**

Delete the `CommandOutput` definition (lines 199–258) and its `CommandOutputProps` type (lines 101–103). Remove the now-unused `MAX_COMMAND_OUTPUT_LINES` import.

- [ ] **Step 3: Delete obsolete CSS**

Edit `messages.css`. Remove all `.tool-inline*` rules NOT used by `UserInputRow`. Concretely:
- Keep rules whose selectors involve `user-input-inline`.
- Keep `.tool-inline { … }` and the modifier `.tool-inline.user-input-inline` (rename if needed) so `UserInputRow` still has its baseline card style — OR leave the entire `.tool-inline*` block untouched since only `UserInputRow` consumes it.

Recommended minimal cut: delete only `.tool-inline-terminal*`, `.tool-inline-row`, `.tool-inline.explore-inline`, `.tool-inline-command*`, `.tool-inline-bar-toggle`, `.tool-inline-icon` modifiers, `.tool-inline-status`, `.tool-inline-detail`, `.tool-inline-muted`, `.tool-inline-clamp`, `.tool-inline-expanded` modifier, `.tool-inline-change*`, `.tool-inline-output`, `.tool-inline-actions`, `.tool-inline-action`, `.tool-group*`, `.explore-inline*` rules. Keep everything `user-input-inline*` references.

- [ ] **Step 4: Run typecheck + lint + all tests**

Run: `cd desktop-ui && bun run typecheck && bun run lint && bun run test`
Expected: 0 errors, all green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/messages/components/MessageRows.tsx desktop-ui/src/styles/messages.css
git commit -m "chore(ui): drop legacy .tool-inline rendering surface

Removes CommandOutput, the .tool-inline-row card style, .tool-group*,
.explore-inline*, and related CSS. UserInputRow still uses the
.user-input-inline* subset.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 15: Snapshot/component tests for the new ToolRow

**Files:**
- Modify: `desktop-ui/src/features/messages/components/Messages.test.tsx` (or new `MessageRows.test.tsx`)

- [ ] **Step 1: Write focused tests**

Append (or create new file alongside `Messages.test.tsx`):

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConversationItem } from "@/types";
import { ToolRow } from "./MessageRows";

function tool(overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>) {
  return {
    id: "t1",
    kind: "tool" as const,
    toolType: "commandExecution",
    title: "Command: ls",
    detail: "",
    status: "completed",
    output: "",
    durationMs: 100,
    ...overrides,
  };
}

describe("ToolRow (new)", () => {
  it("applies the family modifier class", () => {
    const { container } = render(
      <ToolRow item={tool({})} isExpanded={false} onToggle={vi.fn()} />,
    );
    expect(container.querySelector(".tool-row--shell")).not.toBeNull();
  });

  it("shows spinner when running", () => {
    const { container } = render(
      <ToolRow
        item={tool({ status: "in_progress", durationMs: null as unknown as number })}
        isExpanded={false}
        onToggle={vi.fn()}
      />,
    );
    expect(container.querySelector(".tool-row__spinner")).not.toBeNull();
  });

  it("shows X icon and failed modifier on failure", () => {
    const { container } = render(
      <ToolRow item={tool({ status: "failed" })} isExpanded={true} onToggle={vi.fn()} />,
    );
    expect(container.querySelector(".tool-row--failed")).not.toBeNull();
  });

  it("does not call onToggle on click while running", () => {
    const onToggle = vi.fn();
    const { container } = render(
      <ToolRow item={tool({ status: "in_progress" })} isExpanded={false} onToggle={onToggle} />,
    );
    fireEvent.click(container.querySelector(".tool-row__toggle")!);
    expect(onToggle).not.toHaveBeenCalled();
  });

  it("calls onToggle on click when completed", () => {
    const onToggle = vi.fn();
    const { container } = render(
      <ToolRow item={tool({})} isExpanded={false} onToggle={onToggle} />,
    );
    fireEvent.click(container.querySelector(".tool-row__toggle")!);
    expect(onToggle).toHaveBeenCalledWith("t1");
  });
});
```

- [ ] **Step 2: Run tests — expect pass**

Run: `cd desktop-ui && bun run test -- Messages`
Expected: 5 new tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/messages/components/Messages.test.tsx
git commit -m "test(ui): ToolRow family + status + click behaviour

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 16: Manual visual smoke test (user-driven)

**Files:** none

- [ ] **Step 1: Start dev server**

Run (terminal 1): `cd desktop-ui && bun run dev`
Expected: Vite serves on `:1420`.

Run (terminal 2): `cargo tauri dev`
Expected: Tauri shell opens. Dev/prod isolation: ensure `KLYNTBOT_HOME=~/.klyntbot-dev` is set in `.env` so production data is untouched.

- [ ] **Step 2: Trigger each toolType in a real session**

In the desktop UI, run prompts that exercise each path. Suggestions:

1. Coding mode: "List the files in this folder" → exercises `commandExecution` (`ls`) → expect orange shell bar.
2. "Read CLAUDE.md and summarise it" → `fileChange` read → blue filesystem bar.
3. "Edit CLAUDE.md and add a heading" → `fileChange` edit → blue bar with `+1 −0` meta.
4. "Search for TODO" → `commandExecution` with `grep` head → green search bar.
5. "Run `cargo nextest run -p agent`" → long-running shell → expect 3-line tail to appear after 1.2s.
6. Trigger a failure: "Run `tsc --noEmit`" while there's a known type error → red border, auto-expanded body.
7. Klyntbot domain tool: "Add a task to ship the redesign" → indigo domain bar.

- [ ] **Step 3: Verify each row renders correctly**

Confirm visually:
- Family colours match the spec.
- Spinner mounts during run, ✕ on failure.
- Click toggles expand for completed rows; running rows are not clickable.
- Bash tail appears for long-running commands and disappears on completion (unless expanded).
- Auto-expanded failed row collapses on user click and stays collapsed.
- A burst of 3+ reads renders one row with `+N more`; expanding shows sub-rows.

- [ ] **Step 4: Capture before/after screenshots (optional)**

If a screenshot tool is set up, capture a coding-mode turn for the PR description.

- [ ] **Step 5: No commit needed for manual test**

---

## Task 17: Final verification + clippy/fmt

**Files:** none

- [ ] **Step 1: Full FE checks**

Run:
```bash
cd desktop-ui && bun run lint && bun run typecheck && bun run test
```
Expected: 0 errors, all green.

- [ ] **Step 2: Format pass**

Run: `cd desktop-ui && bun run lint -- --fix` (if supported by the project's lint script)
Expected: no changes, or only fmt fixups committed.

If changes are produced, commit:
```bash
git add desktop-ui/src
git commit -m "chore(ui): formatter pass after tool-row redesign

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 3: Workspace-level Rust checks (sanity — no Rust changes expected)**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep -E '^error|^warning' | head -10`
Expected: 0 new warnings introduced (none expected since no Rust files were touched).

- [ ] **Step 4: Push branch and open PR**

Run:
```bash
git push -u origin feature/tool-row-thinking-style
gh pr create --title "feat(ui): tool-row thinking-style redesign" --body "$(cat <<'EOF'
## Summary

- Replaces `.tool-inline` rounded-card rendering with a flat bar + 2px family-coloured left border
- Adds `Action: argument` header + status-only leading icon + click-to-expand body
- Auto-expands failed tool calls; bash live-tail when running ≥1.2s; burst-grouping for ≥3 consecutive same-tool calls
- Spec: `docs/superpowers/specs/2026-05-07-tool-row-thinking-style-redesign.md`
- Plan: `docs/superpowers/plans/2026-05-07-tool-row-thinking-style-redesign.md`

## Test plan

- [x] `bun run test` (all FE tests green)
- [x] Manual smoke: each toolType rendered correctly in dev (`cargo tauri dev`)
- [x] Failed call auto-expands; user collapse is sticky
- [x] Bash tail appears at 1.2s; collapses on completion
- [x] Burst grouping fires for 3+ same-family-same-name reads

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-review notes

**Spec coverage check:**
- Goals 1–5 ✓ — covered by Tasks 1, 6, 10–13 (visual model), 11 (groupBursts), 12 (auto-expand), 7 (BashTail), 8 (body), 9 (BurstRow).
- Per-toolType mapping ✓ — Tasks 3–4 (`toolRowDescriptor` table-driven tests).
- All 8 interaction states ✓ — explicitly tested in Tasks 7, 12, 15.
- Family palette ✓ — Task 1 + Task 6.
- Migration "clean cut" with no compat shim ✓ — Task 14 deletes legacy CSS in same branch.
- Out-of-scope items (image-bearing tool results, keyboard shortcuts, persistence across thread reopen) ✓ — none implemented.

**Type consistency:**
- `ToolFamily` defined in Task 2, consumed by Tasks 3, 4, 5 — names match.
- `BurstGroup.id` shape (`burst-{itemId}`) used identically in Task 5 (definition) and Task 9 (test).
- `toolRowDescriptor` signature unchanged across tasks.

**Placeholder scan:** no "TBD"/"TODO"/"add error handling"/"similar to Task N" left in the document.
