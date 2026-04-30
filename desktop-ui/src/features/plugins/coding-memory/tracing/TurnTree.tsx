import type { TraceEvent } from "./types";

interface Props {
  events: TraceEvent[];
}

export function TurnTree({ events }: Props) {
  const turns = groupByTurn(events);
  return (
    <div className="tracing-turn-tree">
      {turns.map((turn, i) => (
        <div key={i} className="tracing-turn">
          <div className="tracing-turn__label">Turn {turn.turnIndex}</div>
          <div className="tracing-turn__children">
            {turn.events.map((e) => (
              <div key={e.seq} className="tracing-turn__node" data-kind={e.rawKind}>
                {e.rawKind}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function groupByTurn(events: TraceEvent[]) {
  const map = new Map<number, TraceEvent[]>();
  for (const e of events) {
    const key = e.turnIndex ?? -1;
    const list = map.get(key) ?? [];
    list.push(e);
    map.set(key, list);
  }
  return Array.from(map.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([turnIndex, evs]) => ({ turnIndex, events: evs }));
}
