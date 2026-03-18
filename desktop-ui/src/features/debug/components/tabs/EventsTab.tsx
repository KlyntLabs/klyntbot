import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { Pause, Play, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

interface DomainEventPayload {
  eventType: string;
  salience: string;
  domain: string;
  timestamp: string;
  payload: unknown;
}

/** Shape returned by the `cognitive_event_log` backend command. */
interface DomainEventRow {
  id: string;
  event_type: string;
  domain: string;
  salience: string;
  payload: string;
  timestamp: string;
}

const salienceColors: Record<string, string> = {
  extract: "bg-success/20 text-success",
  accumulate: "bg-warning/20 text-warning",
  discard: "bg-accent text-muted-foreground",
};

const salienceBorders: Record<string, string> = {
  extract: "border-l-green-500",
  accumulate: "border-l-yellow-500",
  discard: "border-l-white/20",
};

const MAX_EVENTS = 200;

export function EventsTab() {
  const [events, setEvents] = useState<DomainEventPayload[]>([]);
  const [paused, setPaused] = useState(false);
  const [filters, setFilters] = useState({ extract: true, accumulate: true, discard: false });
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Load historical events on mount
  useEffect(() => {
    ipc<DomainEventRow[]>("cognitive_event_log", { limit: 100 })
      .then((rows) => {
        const historical: DomainEventPayload[] = rows.map((r) => ({
          eventType: r.event_type,
          salience: r.salience,
          domain: r.domain,
          timestamp: r.timestamp,
          payload: (() => {
            try {
              return JSON.parse(r.payload);
            } catch {
              return {};
            }
          })(),
        }));
        setEvents(historical);
      })
      .catch(() => {
        // Endpoint may not exist on older backends — silently ignore.
      });
  }, []);

  useEvent<DomainEventPayload>(
    "cognitive:domain_event",
    useCallback((event: DomainEventPayload) => {
      if (pausedRef.current) return;
      setEvents((prev) => [event, ...prev.slice(0, MAX_EVENTS - 1)]);
    }, []),
  );

  const visibleEvents = useMemo(
    () => events.filter((e) => filters[e.salience as keyof typeof filters]),
    [events, filters],
  );

  return (
    <div className="space-y-4">
      {/* Filter Bar */}
      <div className="flex items-center gap-3">
        {(["extract", "accumulate", "discard"] as const).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setFilters((f) => ({ ...f, [s]: !f[s] }))}
            className={`text-[11px] px-2 py-1 rounded transition-all ${
              filters[s] ? salienceColors[s] : "bg-card text-muted-foreground"
            }`}
          >
            {s.charAt(0).toUpperCase() + s.slice(1)}
          </button>
        ))}
        <div className="flex-1" />
        <button
          type="button"
          onClick={() => setPaused(!paused)}
          className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground"
        >
          {paused ? <Play className="w-3 h-3" /> : <Pause className="w-3 h-3" />}
          {paused ? "Resume" : "Pause"}
        </button>
        <button
          type="button"
          onClick={() => setEvents([])}
          className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground"
        >
          <Trash2 className="w-3 h-3" /> Clear
        </button>
        <span className="text-[11px] text-muted-foreground">{events.length} events</span>
      </div>

      {/* Event Stream */}
      <div className="space-y-1">
        {visibleEvents.map((e, i) => {
          const color = salienceColors[e.salience] ?? salienceColors.discard;
          const border = salienceBorders[e.salience] ?? salienceBorders.discard;
          const key = `${e.timestamp}-${e.eventType}-${i}`;
          const isExpanded = expandedKey === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => setExpandedKey(isExpanded ? null : key)}
              className={`w-full text-left p-2 rounded border-l-2 transition-all ${color} ${border}`}
            >
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted-foreground font-mono w-20 shrink-0">
                  {new Date(e.timestamp).toLocaleTimeString(undefined, {
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                    hour12: false,
                  })}
                </span>
                <span className="text-[11px] text-muted-foreground">{e.eventType}</span>
                <span className={`text-[9px] px-1 py-0.5 rounded ${color}`}>{e.salience}</span>
                <span className="text-[10px] text-muted-foreground">{e.domain}</span>
              </div>
              {isExpanded && (
                <pre className="mt-2 text-[10px] text-muted-foreground font-mono whitespace-pre-wrap">
                  {JSON.stringify(e.payload, null, 2)}
                </pre>
              )}
            </button>
          );
        })}
        {visibleEvents.length === 0 && (
          <p className="text-[12px] text-muted-foreground text-center py-8">
            {paused ? "Stream paused" : "Waiting for domain events..."}
          </p>
        )}
      </div>
    </div>
  );
}
