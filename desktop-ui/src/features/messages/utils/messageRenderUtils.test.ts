import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { buildToolSummary, statusToneFromText, toolRowDescriptor } from "./messageRenderUtils";

function makeToolItem(
  overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>,
): Extract<ConversationItem, { kind: "tool" }> {
  return {
    id: "tool-1",
    kind: "tool",
    toolType: "webSearch",
    title: "Web search",
    detail: "klynt",
    status: "completed",
    output: "",
    ...overrides,
  };
}

describe("messageRenderUtils", () => {
  it("renders web search as searching while in progress", () => {
    const summary = buildToolSummary(makeToolItem({ status: "inProgress" }), "");
    expect(summary.label).toBe("searching");
    expect(summary.value).toBe("klynt");
  });

  it("renders mcp search calls as searching while in progress", () => {
    const summary = buildToolSummary(
      makeToolItem({
        toolType: "mcpToolCall",
        title: "Tool: web / search_query",
        detail: '{\n  "query": "klynt"\n}',
        status: "inProgress",
      }),
      "",
    );
    expect(summary.label).toBe("searching");
    expect(summary.value).toBe("klynt");
  });

  it("classifies camelCase inProgress as processing", () => {
    expect(statusToneFromText("inProgress")).toBe("processing");
  });

  it("renders collab tool calls with nickname and role", () => {
    const summary = buildToolSummary(
      makeToolItem({
        toolType: "collabToolCall",
        title: "Collab: wait",
        detail: "From thread-parent → thread-child",
        status: "completed",
        output: "Robie [explorer]: completed",
        collabReceivers: [
          {
            threadId: "thread-child",
            nickname: "Robie",
            role: "explorer",
          },
        ],
      }),
      "",
    );
    expect(summary.label).toBe("waited for");
    expect(summary.value).toBe("Robie [explorer]");
    expect(summary.output).toContain("Robie [explorer]: completed");
  });
});

function toolItem(
  overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>,
): Extract<ConversationItem, { kind: "tool" }> {
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
});
