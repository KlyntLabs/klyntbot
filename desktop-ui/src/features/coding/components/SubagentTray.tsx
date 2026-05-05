import { useSubagents } from "../hooks/useSubagents";
import { SubagentRow } from "./SubagentRow";

type Props = { threadId: string };

export function SubagentTray({ threadId }: Props) {
  const { active, cancel } = useSubagents(threadId);
  if (active.length === 0) return null;

  return (
    <aside className="subagent-tray" aria-label="Active subagents">
      <header className="subagent-tray__header">
        Subagents <span className="subagent-tray__count">{active.length}</span>
      </header>
      <ol className="subagent-tray__list">
        {active.map((row) => (
          <SubagentRow key={row.agentId} row={row} onCancel={cancel} />
        ))}
      </ol>
    </aside>
  );
}
