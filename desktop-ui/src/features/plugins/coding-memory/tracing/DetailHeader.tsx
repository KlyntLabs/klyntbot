import type { HeaderStats } from "./types";

interface Props {
  providerDisplayName: string;
  sessionId: string;
  stats: HeaderStats;
  onBack: () => void;
  onOpenDir: () => void;
  onCopyDir: () => void;
  onDownload: () => void;
  onRefresh: () => void;
}

export function DetailHeader(p: Props) {
  return (
    <div className="tracing-detail-header">
      <div className="tracing-detail-header__top">
        <button type="button" className="tracing-detail-header__back" onClick={p.onBack}>
          ←
        </button>
        <span className="tracing-detail-header__title">{p.providerDisplayName} Agent Tracing</span>
      </div>
      <div className="tracing-detail-header__stats">
        <span className="tracing-detail-header__id" title={p.sessionId}>
          {p.sessionId.slice(0, 16)}…
        </span>
        <span className="tracing-stat">{p.stats.turnCount} turns</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.stepCount} steps</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.toolCallCount} tool calls</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat tracing-stat--err">{p.stats.errorCount} errors</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.compactionCount} compaction</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{p.stats.agentCount} agents</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{formatDurationMs(p.stats.totalDurationMs)}</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">
          {(p.stats.totalInputTokens / 1000).toFixed(1)}k in /{" "}
          {(p.stats.totalOutputTokens / 1000).toFixed(1)}k out
        </span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{p.stats.cacheHitPct.toFixed(0)}% cache</span>
        <div className="tracing-detail-header__actions">
          <button type="button" onClick={p.onOpenDir}>
            Open Dir
          </button>
          <button type="button" onClick={p.onCopyDir}>
            Copy DIR
          </button>
          <button type="button" onClick={p.onDownload}>
            Download
          </button>
          <button type="button" onClick={p.onRefresh}>
            Refresh
          </button>
        </div>
      </div>
    </div>
  );
}

function formatDurationMs(ms: number): string {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
