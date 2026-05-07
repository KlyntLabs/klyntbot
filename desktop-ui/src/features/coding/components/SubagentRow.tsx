import type { SubagentActiveSummary } from "@/bindings";

type Props = {
  row: SubagentActiveSummary;
  onCancel: (agentId: string) => void;
};

export function SubagentRow({ row, onCancel }: Props) {
  return (
    <li className={`subagent-row subagent-row--${row.status}`}>
      <span className="subagent-row__profile">{row.profile}</span>
      <span className="subagent-row__label">{row.label}</span>
      <span className="subagent-row__iteration">iter {row.iteration}</span>
      {row.lastTool && <span className="subagent-row__last-tool">{row.lastTool}</span>}
      {row.status === "running" && (
        <button
          type="button"
          className="subagent-row__cancel"
          onClick={() => onCancel(row.agentId)}
          title="Cancel subagent"
        >
          Cancel
        </button>
      )}
    </li>
  );
}
