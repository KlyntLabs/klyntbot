import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
  onOpenSubagent?: (agentId: string) => void;
}

export function SubagentEventCard({ event, onOpenSubagent }: Props) {
  const p = event.payload as any;
  const agentId = p?.agent_id;
  const wrapped = p?.event;
  const description = p?.subagent_type ?? agentId;
  return (
    <div className="tracing-evcard tracing-evcard--subagent">
      <button
        type="button"
        className="tracing-evcard__sub-link"
        onClick={() => agentId && onOpenSubagent?.(agentId)}
      >
        ↳ {description}
      </button>
      <pre className="tracing-detail-pane__pre">
        {JSON.stringify(wrapped, null, 2).slice(0, 240)}
      </pre>
    </div>
  );
}
