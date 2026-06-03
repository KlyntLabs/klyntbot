import type { SubagentInfo } from "@/tracing/lib/api";

interface Props {
  agents: SubagentInfo[];
  onSelect: (agentId: string) => void;
}

export function ClaudeCodeAgentsPanel({ agents, onSelect }: Props) {
  if (agents.length === 0) {
    return <div className="cm-state p-6 text-text-faint text-ui-xs text-center">No subagents in this session.</div>;
  }
  return (
    <ul className="cc-agents-list">
      {agents.map((a) => (
        <li key={a.agent_id} className="rounded-md border border-[var(--color-border)] p-3 hover:bg-[var(--color-accent)] cursor-pointer">
          <button type="button" onClick={() => onSelect(a.agent_id)}>
            <span className="text-xs font-mono text-[var(--color-muted-foreground)]">{a.subagent_type}</span>
            <span className="text-sm">{a.description}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
