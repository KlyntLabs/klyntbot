import { useCallback, useMemo } from "react";
import { Virtuoso } from "react-virtuoso";
import type { TraceEvent, Scope } from "./types";
import { TurnBeginCard } from "./event-cards/TurnBeginCard";
import { TurnEndCard } from "./event-cards/TurnEndCard";
import { StepBeginCard } from "./event-cards/StepBeginCard";
import { StepInterruptedCard } from "./event-cards/StepInterruptedCard";
import { ThinkingCard } from "./event-cards/ThinkingCard";
import { AssistantTextCard } from "./event-cards/AssistantTextCard";
import { ToolCallCard } from "./event-cards/ToolCallCard";
import { ToolCallStreamCard } from "./event-cards/ToolCallStreamCard";
import { ToolResultCard } from "./event-cards/ToolResultCard";
import { StatusUpdateCard } from "./event-cards/StatusUpdateCard";
import { SubagentEventCard } from "./event-cards/SubagentEventCard";
import { CompactionMarkerCard } from "./event-cards/CompactionMarkerCard";
import { ErrorCard } from "./event-cards/ErrorCard";
import { OtherEventCard } from "./event-cards/OtherEventCard";
import { DetailPane } from "./DetailPane";
import { WireFilters } from "./WireFilters";

interface Props {
  events: TraceEvent[];
  providerName: string;
  scope: Scope;
  filterKind: string;
  onFilterKind: (k: string) => void;
  expandedSeq: number | null;
  onToggleEvent: (seq: number) => void;
  selectedToolId: string | null;
  onSelectTool: (id: string) => void;
  providerId?: string;
  sessionId?: string;
  onOpenSubagent?: (agentId: string) => void;
}

export function WireEventsTab(props: Props) {
  const {
    events, filterKind, onFilterKind,
    expandedSeq, onToggleEvent, selectedToolId, onSelectTool,
    onOpenSubagent,
  } = props;

  const filtered = useMemo(() => {
    if (!filterKind) return events;
    return events.filter((e) => e.rawKind === filterKind);
  }, [events, filterKind]);

  const kinds = useMemo(() => [...new Set(events.map((e) => e.rawKind))], [events]);

  const selectedToolEvent = useMemo(() => {
    if (!selectedToolId) return undefined;
    return events.find((e) => (e.payload as any)?.id === selectedToolId && e.rawKind === "ToolCall");
  }, [events, selectedToolId]);

  const itemContent = useCallback((index: number, event: TraceEvent) => {
    const prev = filtered[index - 1];
    const deltaMs = prev ? new Date(event.occurredAt).getTime() - new Date(prev.occurredAt).getTime() : undefined;
    return renderCard(event, {
      expanded: expandedSeq === event.seq,
      onToggle: () => onToggleEvent(event.seq),
      deltaMs,
      onSelectTool: () => onSelectTool((event.payload as any)?.id),
      selectedTool: selectedToolId === (event.payload as any)?.id,
      onOpenSubagent,
    });
  }, [filtered, expandedSeq, onToggleEvent, selectedToolId, onSelectTool, onOpenSubagent]);

  return (
    <div className="tracing-detail-pane">
      <div className="tracing-detail-pane__left">
        <WireFilters kinds={kinds} value={filterKind} onChange={onFilterKind} />
        <Virtuoso
          className="tracing-detail-pane__scroll"
          data={filtered}
          itemContent={itemContent}
        />
      </div>
      <DetailPane selectedToolEvent={selectedToolEvent} />
    </div>
  );
}

function renderCard(event: TraceEvent, ctx: CardCtx) {
  switch (event.category) {
    case "turnBegin": return <TurnBeginCard event={event} expanded={ctx.expanded} onToggle={ctx.onToggle} />;
    case "turnEnd": return <TurnEndCard event={event} />;
    case "stepBegin": return <StepBeginCard event={event} deltaMs={ctx.deltaMs} />;
    case "stepInterrupted": return <StepInterruptedCard event={event} />;
    case "thinking": return <ThinkingCard event={event} />;
    case "assistantText": return <AssistantTextCard event={event} />;
    case "toolCall": return <ToolCallCard event={event} onSelect={ctx.onSelectTool!} selected={ctx.selectedTool} />;
    case "toolCallStream": return <ToolCallStreamCard event={event} />;
    case "toolResult": return <ToolResultCard event={event} />;
    case "statusUpdate": return <StatusUpdateCard event={event} />;
    case "subagent": return <SubagentEventCard event={event} onOpenSubagent={ctx.onOpenSubagent} />;
    case "compactionBegin":
    case "compactionEnd": return <CompactionMarkerCard event={event} />;
    case "error": return <ErrorCard event={event} />;
    default: return <OtherEventCard event={event} />;
  }
}

interface CardCtx {
  expanded: boolean;
  onToggle: () => void;
  deltaMs?: number;
  onSelectTool?: () => void;
  selectedTool: boolean;
  onOpenSubagent?: (agentId: string) => void;
}
