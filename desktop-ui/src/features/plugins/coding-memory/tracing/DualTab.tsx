import { ContextMessagesTab } from "./ContextMessagesTab";
import type { Scope } from "./types";

interface Props {
  events?: import("./types").TraceEvent[];
  providerName?: string;
  scope?: Scope;
  providerId?: string;
  sessionId?: string;
}

export function DualTab({
  events,
  providerName: _providerName,
  scope,
  providerId,
  sessionId,
}: Props) {
  return (
    <div className="tracing-dual">
      <div className="tracing-dual__col">
        <h4 className="tracing-dual__title">Wire Events</h4>
        {events && events.length > 0 ? (
          <div className="tracing-state">{events.length} events available</div>
        ) : (
          <div className="tracing-state">No wire events.</div>
        )}
      </div>
      <div className="tracing-dual__col">
        <h4 className="tracing-dual__title">Context</h4>
        <ContextMessagesTab providerId={providerId} sessionId={sessionId} scope={scope} />
      </div>
    </div>
  );
}
