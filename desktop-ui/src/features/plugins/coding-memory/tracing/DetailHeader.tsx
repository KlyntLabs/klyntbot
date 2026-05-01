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
  const s = p.stats;
  return (
    <div className="tracing-detail-header">
      <div className="tracing-detail-header__top">
        <button type="button" className="tracing-detail-header__back" onClick={p.onBack}>←</button>
        <span className="tracing-detail-header__title">{p.providerDisplayName} Agent Tracing</span>
      </div>
      <div className="tracing-detail-header__stats">
        <span className="tracing-detail-header__id" title={p.sessionId}>{p.sessionId.slice(0, 16)}…</span>
        <span className="tracing-stat">{fmtNum(s.turnCount)} turns</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{fmtNum(s.stepCount)} steps</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{fmtNum(s.toolCallCount)} tool calls</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat tracing-stat--err">{fmtNum(s.errorCount)} errors</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{fmtNum(s.compactionCount)} compaction</span>
        <span className="tracing-stat">·</span>
        <span className="tracing-stat">{fmtNum(s.agentCount)} agents</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{formatDurationMs(s.totalDurationMs)}</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{fmtTokens(s.totalInputTokens)} in / {fmtTokens(s.totalOutputTokens)} out</span>
        <span className="tracing-stat">|</span>
        <span className="tracing-stat">{fmtPct(s.cacheHitPct)}% cache</span>
        <div className="tracing-detail-header__actions">
          <button type="button" onClick={p.onOpenDir}>Open Dir</button>
          <button type="button" onClick={p.onCopyDir}>Copy DIR</button>
          <button type="button" onClick={p.onDownload}>Download</button>
          <button type="button" onClick={p.onRefresh}>Refresh</button>
        </div>
      </div>
    </div>
  );
}

function fmtNum(n: number | null | undefined): number | string {
  return n ?? 0;
}

function fmtTokens(n: number | null | undefined): string {
  if (n == null) return "0.0";
  return (n / 1000).toFixed(1);
}

function fmtPct(n: number | null | undefined): string {
  if (n == null) return "0";
  return n.toFixed(0);
}

function formatDurationMs(ms: number | null | undefined): string {
  if (ms == null) return "0s";
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
