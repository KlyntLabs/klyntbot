import type { SubagentInfo } from "@/tracing/lib/api";

interface Props {
  agents: SubagentInfo[];
  onSelect: (agentId: string) => void;
}

export function ClaudeCodeAgentsPanel({ agents, onSelect }: Props) {
  if (agents.length === 0) {
    return <div className="cm-state cm-state--empty">No subagents in this session.</div>;
  }
  return (
    <ul className="cc-agents-list">
      {agents.map((a) => (
        <li key={a.agent_id} className="cc-agents-list__item">
          <button type="button" onClick={() => onSelect(a.agent_id)}>
            <span className="cc-agents-list__type">{a.subagent_type}</span>
            <span className="cc-agents-list__desc">{a.description}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
