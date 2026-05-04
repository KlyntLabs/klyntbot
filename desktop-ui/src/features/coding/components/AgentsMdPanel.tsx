import { useState } from "react";
import { useAgentsMd } from "../hooks/useAgentsMd";
import type { AgentsMdSource } from "@/bindings";

type Props = {
  threadId: string;
  initialSources: AgentsMdSource[];
};

export function AgentsMdPanel({ threadId, initialSources }: Props) {
  const { sources, refresh, refreshing, lastRefreshedAt } = useAgentsMd(threadId, initialSources);
  const [expanded, setExpanded] = useState(false);

  if (sources.length === 0) {
    return (
      <aside className="agents-md-panel agents-md-panel--empty">
        No AGENTS.md found in workspace ancestor chain.
      </aside>
    );
  }

  return (
    <aside className="agents-md-panel" aria-label="Loaded AGENTS.md context">
      <header className="agents-md-panel__header">
        <button
          type="button"
          className="agents-md-panel__toggle"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
        >
          Loaded context <span className="agents-md-panel__count">{sources.length}</span>
        </button>
        <button
          type="button"
          className="agents-md-panel__refresh"
          onClick={refresh}
          disabled={refreshing}
        >
          {refreshing ? "Refreshing…" : "Refresh"}
        </button>
      </header>
      {expanded && (
        <ol className="agents-md-panel__sources">
          {sources.map((src) => (
            <li key={src.path} className="agents-md-panel__source">
              <span className={`agents-md-panel__origin agents-md-panel__origin--${originKind(src)}`}>
                {originLabel(src)}
              </span>
              <code className="agents-md-panel__path">{shortenPath(src.path)}</code>
              <span className="agents-md-panel__bytes">{formatBytes(byteLength(src.contents))}</span>
            </li>
          ))}
        </ol>
      )}
      {lastRefreshedAt && (
        <footer className="agents-md-panel__footer">
          Last refreshed {lastRefreshedAt.toLocaleTimeString()}
        </footer>
      )}
    </aside>
  );
}

function originKind(src: AgentsMdSource): string {
  if (src.dir === "<global>") return "global";
  if (src.path.split("/").length <= 4) return "root";
  return "nested";
}

function originLabel(src: AgentsMdSource): string {
  return originKind(src);
}

function byteLength(s: string): number {
  return new TextEncoder().encode(s).length;
}

function shortenPath(p: string): string {
  const home = "/Users/";
  if (p.startsWith(home)) return p.replace(/^\/Users\/[^/]+/, "~");
  return p;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
