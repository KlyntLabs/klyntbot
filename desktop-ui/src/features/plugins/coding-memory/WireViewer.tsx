import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { WireEventCard } from "./WireEventCard";
import { WireFilters } from "./WireFilters";
import { TurnTree } from "./TurnTree";
import { ToolCallDetail } from "./ToolCallDetail";
import { RecallOverlay } from "./RecallOverlay";
import { MirrorAlertSidecar } from "./MirrorAlertSidecar";
import { buildToolGrouping } from "./buildToolGrouping";
import { isErrorEvent } from "./eventHelpers";
import { fetchSessionWire } from "@/api/endpoints/codingMemory";
import type { WireEventDto } from "./types";

interface WireViewerProps {
  sessionId: string;
  refreshKey?: number;
}

export function WireViewer({ sessionId, refreshKey = 0 }: WireViewerProps) {
  const [events, setEvents] = useState<WireEventDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedTypes, setSelectedTypes] = useState<Set<string>>(new Set());
  const [expandedSet, setExpandedSet] = useState<Set<number>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [selectedToolEvent, setSelectedToolEvent] = useState<WireEventDto | null>(null);
  const [showRecall, setShowRecall] = useState(false);
  const [showMirror, setShowMirror] = useState(false);
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    if (refreshKey === 0) {
      setExpandedSet(new Set());
      setSearchQuery("");
      setErrorsOnly(false);
    }
    fetchSessionWire(sessionId, 1000, 0)
      .then((rows) => {
        if (!cancelled) setEvents(rows);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err.message ?? err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [sessionId, refreshKey]);

  const allTypes = useMemo(() => {
    const types = new Set<string>();
    for (const e of events) {
      types.add(e.kind);
    }
    return Array.from(types).sort();
  }, [events]);

  const toolGrouping = useMemo(() => buildToolGrouping(events), [events]);

  const searchMatchSet = useMemo(() => {
    const matches = new Set<number>();
    if (!searchQuery) return matches;
    const q = searchQuery.toLowerCase();
    events.forEach((e, i) => {
      const haystack = JSON.stringify(e.payloadDecoded ?? e.rawJson).toLowerCase();
      if (haystack.includes(q) || e.kind.toLowerCase().includes(q)) {
        matches.add(i);
      }
    });
    return matches;
  }, [events, searchQuery]);

  const errorIndices = useMemo(() => {
    const indices: number[] = [];
    events.forEach((e, i) => { if (isErrorEvent(e)) indices.push(i); });
    return indices;
  }, [events]);

  const filtered = useMemo(() => {
    let result = events.map((e, i) => ({ event: e, index: i }));
    if (selectedTypes.size > 0) {
      result = result.filter(({ event }) => selectedTypes.has(event.kind));
    }
    if (errorsOnly) {
      result = result.filter(({ event }) => isErrorEvent(event));
    }
    if (searchQuery) {
      result = result.filter(({ index }) => searchMatchSet.has(index));
    }
    return result;
  }, [events, selectedTypes, errorsOnly, searchQuery, searchMatchSet]);

  const toggleExpand = useCallback((index: number) => {
    setExpandedSet((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }, []);

  const allExpanded = filtered.length > 0 && filtered.every(({ index }) => expandedSet.has(index));

  const expandAll = useCallback(() => {
    setExpandedSet((prev) => {
      const next = new Set(prev);
      for (const { index } of filtered) {
        next.add(index);
      }
      return next;
    });
  }, [filtered]);

  const collapseAll = useCallback(() => {
    setExpandedSet((prev) => {
      const next = new Set(prev);
      for (const { index } of filtered) {
        next.delete(index);
      }
      return next;
    });
  }, [filtered]);

  const applyPreset = useCallback((types: Set<string>, newErrorsOnly: boolean) => {
    setSelectedTypes(types);
    setErrorsOnly(newErrorsOnly);
  }, []);

  const scrollToEventIndex = useCallback((eventIndex: number) => {
    const pos = filtered.findIndex(({ index }) => index === eventIndex);
    if (pos >= 0 && virtuosoRef.current) {
      virtuosoRef.current.scrollToIndex({ index: pos, align: "center", behavior: "smooth" });
    }
  }, [filtered]);

  const handleEventSelect = useCallback((event: WireEventDto, index: number) => {
    if (event.kind === "toolCall" || event.kind === "toolResult") {
      setSelectedToolEvent((prev) => (prev === event ? null : event));
    }
  }, []);

  const errorNavRef = useRef(0);
  const navigateError = useCallback((direction: "next" | "prev") => {
    if (errorIndices.length === 0) return;
    if (direction === "next") {
      errorNavRef.current = (errorNavRef.current + 1) % errorIndices.length;
    } else {
      errorNavRef.current = (errorNavRef.current - 1 + errorIndices.length) % errorIndices.length;
    }
    const targetIndex = errorIndices[errorNavRef.current];
    const pos = filtered.findIndex(({ index }) => index === targetIndex);
    if (pos >= 0 && virtuosoRef.current) {
      virtuosoRef.current.scrollToIndex({ index: pos, align: "center", behavior: "smooth" });
      setExpandedSet((prev) => {
        const next = new Set(prev);
        next.add(targetIndex);
        return next;
      });
    }
  }, [errorIndices, filtered]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "j") {
        e.preventDefault();
        // focus down handled by virtuoso
      } else if (e.key === "k") {
        e.preventDefault();
      } else if (e.key === "Enter") {
        e.preventDefault();
      } else if (e.key === "e") {
        e.preventDefault();
        navigateError("next");
      } else if (e.key === "/") {
        e.preventDefault();
        const input = document.querySelector("[data-wire-search]") as HTMLInputElement | null;
        input?.focus();
      } else if (e.key === "Escape") {
        setSelectedToolEvent(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigateError]);

  if (loading) {
    return <div className="cm-state cm-state--loading">Loading wire events…</div>;
  }
  if (error) {
    return <div className="cm-state cm-state--error">Error: {error}</div>;
  }
  if (events.length === 0) {
    return <div className="cm-state cm-state--empty">No wire events found</div>;
  }

  return (
    <div className="cm-wire-viewer">
      <WireFilters
        allTypes={allTypes}
        selectedTypes={selectedTypes}
        onToggle={(type) => {
          setSelectedTypes((prev) => {
            const next = new Set(prev);
            if (next.has(type)) next.delete(type);
            else next.add(type);
            return next;
          });
        }}
        total={events.length}
        filteredCount={filtered.length}
        allExpanded={allExpanded}
        onExpandAll={expandAll}
        onCollapseAll={collapseAll}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchMatchCount={searchMatchSet.size}
        errorCount={errorIndices.length}
        errorsOnly={errorsOnly}
        onToggleErrorsOnly={() => setErrorsOnly((v) => !v)}
        onNextError={() => navigateError("next")}
        onPrevError={() => navigateError("prev")}
        onApplyPreset={applyPreset}
      />
      <div className="cm-wire-viewer__toggles">
        <button
          type="button"
          className={"cm-wire-viewer__toggle" + (showRecall ? " cm-wire-viewer__toggle--active" : "")}
          onClick={() => setShowRecall((v) => !v)}
        >
          Recall
        </button>
        <button
          type="button"
          className={"cm-wire-viewer__toggle" + (showMirror ? " cm-wire-viewer__toggle--active" : "")}
          onClick={() => setShowMirror((v) => !v)}
        >
          Mirror
        </button>
      </div>
      <div className="cm-wire-viewer__body">
        <TurnTree events={events} onScrollToIndex={scrollToEventIndex} />
        <div className="cm-wire-viewer__main">
          <div className="cm-wire-viewer__list">
            <Virtuoso
              ref={virtuosoRef}
              data={filtered}
              itemContent={(_, { event, index }) => {
                const meta = toolGrouping.get(index);
                return (
                  <WireEventCard
                    event={event}
                    expanded={expandedSet.has(index)}
                    onToggle={() => toggleExpand(index)}
                    onSelect={() => handleEventSelect(event, index)}
                    selected={selectedToolEvent === event}
                    prevEvent={index > 0 ? events[index - 1] : undefined}
                    nestLevel={meta?.nestLevel}
                    linkedToolName={meta?.linkedToolName}
                    linkedToolCallId={meta?.linkedToolCallId}
                    searchMatch={searchQuery ? searchMatchSet.has(index) : undefined}
                  />
                );
              }}
            />
          </div>
          {selectedToolEvent && (
            <ToolCallDetail
              selectedEvent={selectedToolEvent}
              allEvents={events}
              onClose={() => setSelectedToolEvent(null)}
            />
          )}
          {showRecall && <RecallOverlay sessionId={sessionId} />}
          {showMirror && <MirrorAlertSidecar />}
        </div>
      </div>
    </div>
  );
}
