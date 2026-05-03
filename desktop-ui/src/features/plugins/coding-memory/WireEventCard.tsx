import { AlertCircle, Check, ChevronDown, ChevronRight, Copy } from "lucide-react";
import { memo, useState } from "react";
import {
  eventChipColor,
  formatTimeDelta,
  formatTimestamp,
  isErrorEvent,
  summarizeEvent,
  timeDeltaSeverity,
} from "./eventHelpers";
import type { WireEventDto } from "./types";

export interface WireEventCardProps {
  event: WireEventDto;
  expanded: boolean;
  onToggle: () => void;
  onSelect?: () => void;
  selected?: boolean;
  prevEvent?: WireEventDto;
  nestLevel?: number;
  linkedToolName?: string;
  linkedToolCallId?: string;
  searchMatch?: boolean;
}

export const WireEventCard = memo(function WireEventCard(props: WireEventCardProps) {
  const {
    event,
    expanded,
    onToggle,
    onSelect,
    selected,
    prevEvent,
    nestLevel = 0,
    searchMatch,
  } = props;
  const color = eventChipColor(event.kind);
  const isError = isErrorEvent(event);
  const [copied, setCopied] = useState(false);

  const gap = prevEvent ? renderGap(event, prevEvent) : null;

  return (
    <>
      {gap}
      <div
        className={
          "cm-event-card" +
          (selected ? " cm-event-card--selected" : "") +
          (searchMatch ? " cm-event-card--search-hit" : "") +
          (isError ? " cm-event-card--error" : "")
        }
        style={nestLevel ? { paddingLeft: `${20 + nestLevel * 16}px` } : undefined}
        onClick={onSelect}
      >
        <button
          type="button"
          className="cm-event-card__chevron"
          onClick={(e) => {
            e.stopPropagation();
            onToggle();
          }}
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>
        <span className="cm-event-card__time">{formatTimestamp(event.occurredAt)}</span>
        <span className={`cm-event-chip cm-event-chip--${color}`}>{event.kind}</span>
        {isError && <AlertCircle size={12} className="cm-event-card__error-icon" aria-hidden />}
        <span className="cm-event-card__summary">{summarizeEvent(event)}</span>
        <button
          type="button"
          className="cm-event-card__copy"
          onClick={async (e) => {
            e.stopPropagation();
            await navigator.clipboard.writeText(event.rawJson);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          }}
          aria-label="Copy raw JSON"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
        </button>
      </div>
      {expanded && <pre className="cm-event-card__payload">{prettyPrint(event.rawJson)}</pre>}
    </>
  );
});

function renderGap(curr: WireEventDto, prev: WireEventDto) {
  const ms = new Date(curr.occurredAt).getTime() - new Date(prev.occurredAt).getTime();
  if (ms < 1000) return null;
  const sev = timeDeltaSeverity(ms);
  return (
    <div
      className={`cm-gap${sev === "warn" ? " cm-gap--warn" : sev === "danger" ? " cm-gap--danger" : ""}`}
    >
      <span className="cm-gap__line" />
      <span>{formatTimeDelta(new Date(curr.occurredAt), new Date(prev.occurredAt))}</span>
      <span className="cm-gap__line" />
    </div>
  );
}

function prettyPrint(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}
