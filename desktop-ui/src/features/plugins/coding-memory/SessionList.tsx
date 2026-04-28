import { eventChipColor } from "./eventHelpers";
import type { SessionSummaryDto } from "./types";

interface Props {
  sessions: SessionSummaryDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  loading?: boolean;
}

export function SessionList({ sessions, selectedId, onSelect, loading }: Props) {
  if (loading) return <div className="cm-state cm-state--loading">Loading sessions…</div>;
  if (sessions.length === 0) return <div className="cm-state cm-state--empty">No sessions in the last 14 days.</div>;
  return (
    <ul className="cm-session-list" role="listbox" aria-label="Coding memory sessions">
      {sessions.map((s) => (
        <li key={s.sessionId}>
          <button
            type="button"
            role="option"
            aria-selected={selectedId === s.sessionId}
            className={"cm-session-list__item" + (selectedId === s.sessionId ? " cm-session-list__item--active" : "")}
            onClick={() => onSelect(s.sessionId)}
          >
            <div className="cm-session-list__top">
              <span className={`cm-event-chip cm-event-chip--${providerColor(s.source)}`}>{s.source}</span>
              <span className="cm-session-list__id">{s.sessionId.slice(0, 8)}…</span>
              <span className="cm-session-list__when">{new Date(s.lastEventAt).toLocaleString()}</span>
            </div>
            <div className="cm-session-list__cwd">{s.cwd ?? "(no cwd)"}</div>
            <div className="cm-session-list__stats">
              {s.eventCount} events · {s.turnCount} turns · {s.toolCallCount} tools
              {s.errorCount > 0 && <span className="cm-session-list__errors"> · {s.errorCount} errors</span>}
            </div>
          </button>
        </li>
      ))}
    </ul>
  );
}

function providerColor(source: string): string {
  switch (source) {
    case "kimiCli": return "purple";
    case "claudeCode": return "blue";
    case "codex": return "green";
    case "openCode": return "cyan";
    default: return "neutral";
  }
}
