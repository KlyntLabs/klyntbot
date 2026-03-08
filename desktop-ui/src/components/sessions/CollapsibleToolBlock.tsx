import { ChevronDown, FileText, Terminal, Wrench } from "lucide-react";
import { useState } from "react";
import type { ContentBlock } from "../../lib/session-types";

interface CollapsibleToolBlockProps {
  tool: ContentBlock;
  result?: ContentBlock;
}

const TOOL_ICONS: Record<string, typeof Wrench> = {
  Read: FileText,
  Edit: FileText,
  Write: FileText,
  Bash: Terminal,
  Grep: Terminal,
  Glob: Terminal,
};

function toolSummary(tool: ContentBlock): string {
  const name = tool.name ?? "Tool";
  const input = tool.input as Record<string, unknown> | undefined;
  if (!input) return name;

  if (input.file_path) return `${name} ${input.file_path}`;
  if (input.command) {
    const cmd = String(input.command);
    return `${name}: ${cmd.length > 60 ? `${cmd.slice(0, 60)}...` : cmd}`;
  }
  if (input.pattern) return `${name}: ${input.pattern}`;
  return name;
}

export function CollapsibleToolBlock({ tool, result }: CollapsibleToolBlockProps) {
  const [expanded, setExpanded] = useState(false);
  const Icon = TOOL_ICONS[tool.name ?? ""] ?? Wrench;

  return (
    <div className="glass-card my-1">
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="w-full flex items-center gap-2 px-3 py-2 text-[12px] font-light text-muted hover:text-secondary transition-colors"
      >
        <Icon className="w-3.5 h-3.5 shrink-0" strokeWidth={1.5} />
        <span className="flex-1 text-left truncate">{toolSummary(tool)}</span>
        {result?.isError && <span className="text-[10px] text-destructive shrink-0">error</span>}
        <ChevronDown
          className={`w-3.5 h-3.5 shrink-0 transition-transform ${expanded ? "rotate-0" : "-rotate-90"}`}
          strokeWidth={1.5}
        />
      </button>
      {expanded && (
        <div className="px-3 pb-2 space-y-2">
          {tool.input != null && (
            <pre className="text-[11px] text-dim font-light overflow-x-auto whitespace-pre-wrap break-all">
              {typeof tool.input === "string" ? tool.input : JSON.stringify(tool.input, null, 2)}
            </pre>
          )}
          {result?.content != null && (
            <>
              <div className="glass-divider" />
              <pre
                className={`text-[11px] font-light overflow-x-auto whitespace-pre-wrap break-all ${
                  result.isError ? "text-destructive" : "text-dim"
                }`}
              >
                {typeof result.content === "string"
                  ? result.content
                  : JSON.stringify(result.content, null, 2)}
              </pre>
            </>
          )}
        </div>
      )}
    </div>
  );
}
