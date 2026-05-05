import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import { useMemo, useState } from "react";

export function ToolCallPart({
  callId,
  name,
  args,
}: {
  callId: string;
  name: string;
  args: unknown;
}) {
  const [expanded, setExpanded] = useState(false);
  const argsStr = useMemo(
    () => (typeof args === "string" ? args : JSON.stringify(args, null, 2)),
    [args],
  );

  return (
    <div className="part-tool-call">
      <button
        type="button"
        className="part-tool-call__header"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <ChevronRight
          size={14}
          className={`part-tool-call__chevron ${expanded ? "expanded" : ""}`}
        />
        <span className="part-tool-call__name">{name}</span>
        <span className="part-tool-call__id">{callId.slice(0, 8)}</span>
      </button>
      {expanded && <pre className="part-tool-call__args">{argsStr}</pre>}
    </div>
  );
}
