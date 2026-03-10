import { ChevronDown, Loader2 } from "lucide-react";
import { useState } from "react";
import type { TransparencyData } from "@shared/types";
import { formatCost, formatDuration, formatTokens } from "@shared/lib/utils";

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
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="ml-auto flex items-center gap-1.5 text-[10px] font-light text-dim hover:text-muted transition-colors"
      >
        <span>
          {"\u2191"}
          {formatTokens(usage.promptTokens)}
        </span>
        <span>
          {"\u2193"}
          {formatTokens(usage.completionTokens)}
        </span>
        {cost && (
          <span>
            {"\u00b7"} {formatCost(cost.estimatedUsd)}
          </span>
        )}
        <ChevronDown
          className={`w-2.5 h-2.5 transition-transform ${expanded ? "rotate-180" : ""}`}
          strokeWidth={1.5}
        />
      </button>

      {expanded && (
        <div className="mt-1.5 p-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] text-[10px] font-light space-y-1">
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
          {transparency.toolTokensTotal && transparency.toolTokensTotal > 0 && (
            <div className="flex justify-between text-muted">
              <span>Tool I/O (est.)</span>
              <span className="text-secondary">~{formatTokens(transparency.toolTokensTotal)}</span>
            </div>
          )}
          {cost && (
            <>
              <div className="border-t border-white/[0.08] my-1" />
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
