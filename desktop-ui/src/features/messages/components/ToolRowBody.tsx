import { memo } from "react";
import { PierreDiffBlock } from "@/features/git/components/PierreDiffBlock";
import type { ConversationItem } from "@/types";
import { Markdown } from "./Markdown";
import { SubagentChip } from "./SubagentChip";

type ToolRowBodyProps = {
  item: Extract<ConversationItem, { kind: "tool" }>;
};

function tryParseSubagentOutput(
  output: string,
): { agentId?: string; sessionId?: string; description?: string } | null {
  try {
    const parsed = JSON.parse(output);
    if (typeof parsed.agentId === "string" && typeof parsed.sessionId === "string") {
      return {
        agentId: parsed.agentId,
        sessionId: parsed.sessionId,
        description: typeof parsed.summary === "string" ? parsed.summary : undefined,
      };
    }
  } catch {
    // not JSON or missing fields
  }
  return null;
}

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
  const subagentInfo = item.output ? tryParseSubagentOutput(item.output) : null;

  return (
    <>
      {output ? (
        <div className="tool-row__body tool-row__body--code">{output}</div>
      ) : item.detail ? (
        <div className="tool-row__body tool-row__body--code">{item.detail}</div>
      ) : null}
      {subagentInfo && (
        <SubagentChip
          agentId={subagentInfo.agentId!}
          sessionId={subagentInfo.sessionId!}
          description={subagentInfo.description}
        />
      )}
    </>
  );
});
