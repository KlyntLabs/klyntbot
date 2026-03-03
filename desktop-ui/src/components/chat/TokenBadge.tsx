import { useState } from 'react';
import { ChevronDown, Loader2 } from 'lucide-react';
import { formatDuration } from '../../lib/utils';
import type { TransparencyData } from '../../lib/types';

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  return `$${usd.toFixed(3)}`;
}

interface TokenBadgeProps {
  transparency: TransparencyData;
  isStreaming?: boolean;
}

export function TokenBadge({ transparency, isStreaming }: TokenBadgeProps) {
  const [expanded, setExpanded] = useState(false);
  const { usage, cost, timing } = transparency;

  // During streaming, show spinner until usage arrives
  if (!usage) {
    if (!isStreaming) return null;
    return (
      <div className="flex justify-end mt-1">
        <div className="flex items-center gap-1 text-[10px] font-light text-dim">
          <Loader2 className="w-2.5 h-2.5 animate-spin" strokeWidth={1.5} />
        </div>
      </div>
    );
  }

  return (
    <div className="mt-1.5">
      <button
        onClick={() => setExpanded(!expanded)}
        className="ml-auto flex items-center gap-1.5 text-[10px] font-light text-dim hover:text-muted transition-colors"
      >
        <span>{'\u2191'}{formatTokens(usage.promptTokens)}</span>
        <span>{'\u2193'}{formatTokens(usage.completionTokens)}</span>
        {cost && <span>{'\u00b7'} {formatCost(cost.estimatedUsd)}</span>}
        <ChevronDown
          className={`w-2.5 h-2.5 transition-transform ${expanded ? 'rotate-180' : ''}`}
          strokeWidth={1.5}
        />
      </button>

      {expanded && (
        <div className="mt-1.5 p-2.5 rounded-lg bg-surface-base border border-border text-[10px] font-light space-y-1">
          <div className="flex justify-between text-muted">
            <span>Input tokens</span>
            <span className="text-secondary">{usage.promptTokens.toLocaleString()}</span>
          </div>
          <div className="flex justify-between text-muted">
            <span>Output tokens</span>
            <span className="text-secondary">{usage.completionTokens.toLocaleString()}</span>
          </div>
          {usage.cacheReadTokens > 0 && (
            <div className="flex justify-between text-muted">
              <span>Cache read</span>
              <span className="text-secondary">{usage.cacheReadTokens.toLocaleString()}</span>
            </div>
          )}
          {usage.cacheWriteTokens > 0 && (
            <div className="flex justify-between text-muted">
              <span>Cache write</span>
              <span className="text-secondary">{usage.cacheWriteTokens.toLocaleString()}</span>
            </div>
          )}
          {cost && (
            <>
              <div className="border-t border-border my-1" />
              <div className="flex justify-between text-muted">
                <span>Model</span>
                <span className="text-secondary">{cost.model}</span>
              </div>
              <div className="flex justify-between text-muted">
                <span>Cost</span>
                <span className="text-secondary">{formatCost(cost.estimatedUsd)}</span>
              </div>
            </>
          )}
          {timing?.totalMs && timing.totalMs > 0 && (
            <div className="flex justify-between text-muted">
              <span>Latency</span>
              <span className="text-secondary">{formatDuration(timing.totalMs)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
