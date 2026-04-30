import type { SessionSummary } from "./types";

interface Props {
  summary: SessionSummary;
  onClick: () => void;
  selected?: boolean;
}

export function SessionCard({ summary, onClick, selected }: Props) {
  const title = summary.customTitle?.trim() || "Untitled Session";
  const sizeKB = (summary.sizeBytes / 1024).toFixed(1);
  const durationMs = Date.parse(summary.lastEventAt) - Date.parse(summary.startedAt);
  const duration = formatDurationMs(durationMs);
  return (
    <button
      type="button"
      className={"tracing-card" + (selected ? " tracing-card--selected" : "")}
      onClick={onClick}
    >
      <div className="tracing-card__row">
        <span className="tracing-card__id">{summary.sessionId.slice(0, 8)}</span>
        <span className="tracing-card__time">{relativeTime(summary.lastEventAt)}</span>
      </div>
      <div className="tracing-card__title" title={title}>{title}</div>
      <div className="tracing-card__row">
        {summary.hasWire && <span className="tracing-badge tracing-badge--wire">wire</span>}
        {summary.hasContext && <span className="tracing-badge tracing-badge--context">context</span>}
        <span className="tracing-card__size">{sizeKB} KB</span>
      </div>
      <div className="tracing-card__counts">
        <span className="tracing-count tracing-count--turn">{summary.turnCount} turn</span>
        <span className="tracing-count-sep">·</span>
        <span className="tracing-count tracing-count--step">{summary.stepCount} step</span>
        <span className="tracing-count-sep">·</span>
        <span className="tracing-count tracing-count--tool">{summary.toolCallCount} tool</span>
      </div>
      <div className="tracing-card__row">
        <span className="tracing-card__duration">⏱ {duration}</span>
        {summary.errorCount > 0 && (
          <span className="tracing-card__error">ⓘ {summary.errorCount} error</span>
        )}
      </div>
    </button>
  );
}

function relativeTime(iso: string): string {
  const ms = Date.now() - Date.parse(iso);
  const days = ms / 86_400_000;
  if (days < 1) return "today";
  if (days < 2) return "1d ago";
  if (days < 30) return `${Math.floor(days)}d ago`;
  return new Date(iso).toLocaleDateString();
}

function formatDurationMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
