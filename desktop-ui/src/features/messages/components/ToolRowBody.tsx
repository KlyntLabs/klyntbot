import { memo } from "react";
import { cn } from "@/utils/cn";
import { PierreDiffBlock } from "@/features/git/components/PierreDiffBlock";
import type { ConversationItem } from "@/types";
import { Markdown } from "./Markdown";
import { SubagentChip } from "./SubagentChip";

type ToolRowBodyProps = {
  item: Extract<ConversationItem, { kind: "tool" }>;
  isFailed?: boolean;
};

function tryParseSubagentOutput(output: string): { agentId?: string; sessionId?: string; description?: string } | null {
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

const bodyBaseClass =
  "mb-1 ml-3.5 py-2 px-3 bg-surface-command rounded-r-[6px] text-ui-xs text-text-quiet max-h-[360px] overflow-auto";

export const ToolRowBody = memo(function ToolRowBody({ item, isFailed }: ToolRowBodyProps) {
  if (item.toolType === "commandExecution") {
    const output = item.output ?? "";
    if (!output.trim()) {
      return (
        <div
          className={cn(bodyBaseClass, isFailed && "text-[var(--cm-color-danger-fg)]")}
          data-testid="tool-row-body"

        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
        >
          No output.
        </div>
      );
    }
    return (
      <div
        className={cn(bodyBaseClass, "font-code text-[11px] whitespace-pre-wrap leading-[1.55]", isFailed && "text-[var(--cm-color-danger-fg)]")}
        data-testid="tool-row-body"
        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
      >
        {output}
      </div>
    );
  }

  if (item.toolType === "fileChange") {
    const changes = item.changes ?? [];
    if (changes.length === 0) {
      return item.detail ? (
        <div
          className={cn(bodyBaseClass, "font-code text-[11px] whitespace-pre-wrap leading-[1.55]", isFailed && "text-[var(--cm-color-danger-fg)]")}
          data-testid="tool-row-body"

        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
        >
          {item.detail}
        </div>
      ) : null;
    }
    return (
      <div
        className={cn(bodyBaseClass, "p-0", isFailed && "text-[var(--cm-color-danger-fg)]")}
        data-testid="tool-row-body"
        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
      >
        {changes.map((c) => (
          <div key={`${c.path}-${c.kind ?? ""}`}>
            <div className="text-ui-xs font-semibold text-text-strong px-3 pt-2 pb-1">{c.path}</div>
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
      <div
        className={cn(bodyBaseClass, isFailed && "text-[var(--cm-color-danger-fg)]")}
        data-testid="tool-row-body"
        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
      >
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
        <div
          className={cn(bodyBaseClass, "font-code text-[11px] whitespace-pre-wrap leading-[1.55]", isFailed && "text-[var(--cm-color-danger-fg)]")}
          data-testid="tool-row-body"

        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
        >
          {output}
        </div>
      ) : item.detail ? (
        <div
          className={cn(bodyBaseClass, "font-code text-[11px] whitespace-pre-wrap leading-[1.55]", isFailed && "text-[var(--cm-color-danger-fg)]")}
          data-testid="tool-row-body"

        style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
        >
          {item.detail}
        </div>
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
