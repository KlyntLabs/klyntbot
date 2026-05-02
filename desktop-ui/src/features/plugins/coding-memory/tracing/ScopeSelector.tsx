import type { Scope, SubagentSummary } from "./types";

interface Props {
  scope: Scope;
  onScopeChange: (s: Scope) => void;
  subagents: SubagentSummary[];
}

export function ScopeSelector({ scope, onScopeChange, subagents }: Props) {
  const isMain = scope.kind === "main";
  return (
    <div className="tracing-scope">
      <span className="tracing-scope__label">SCOPE</span>
      <button
        type="button"
        className={"tracing-scope__chip" + (isMain ? " tracing-scope__chip--active" : "")}
        onClick={() => onScopeChange({ kind: "main" })}
      >
        Main Agent
      </button>
      {subagents.map((s) => {
        const active = scope.kind === "subagent" && scope.agentId === s.agentId;
        const color = stableColor(s.agentId);
        return (
          <button
            key={s.agentId}
            type="button"
            className={"tracing-scope__chip" + (active ? " tracing-scope__chip--active" : "")}
            style={{ borderLeft: `3px solid ${color}` }}
            onClick={() => onScopeChange({ kind: "subagent", agentId: s.agentId })}
            title={s.description ?? s.agentId}
          >
            {s.description ?? s.agentId}
          </button>
        );
      })}
    </div>
  );
}

function stableColor(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) % 360;
  return `hsl(${h}, 60%, 60%)`;
}
