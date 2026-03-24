import { formatCost, formatDuration, formatTokens } from "@shared/lib/utils";
import type { TransparencyData } from "@shared/types";
import { ChevronDown, Loader2 } from "lucide-react";
import { useState } from "react";

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
        <div className="flex items-center gap-1 text-2xs font-light text-dim">
          <Loader2 className="size-2.5 animate-spin" strokeWidth={1.5} />
        </div>
      </div>
    );
  }

  return (
    <div className="mt-1.5">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="ml-auto flex items-center gap-1.5 text-2xs font-light text-dim hover:text-muted-foreground transition-colors"
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
          className={`size-2.5 transition-transform ${expanded ? "rotate-180" : ""}`}
          strokeWidth={1.5}
        />
      </button>

      {expanded && (
        <div className="mt-1.5 p-2.5 rounded-lg bg-accent border border-border text-2xs font-light space-y-1">
          <div className="flex justify-between text-muted-foreground">
            <span>Input tokens</span>
            <span className="text-muted-foreground">{usage.promptTokens.toLocaleString()}</span>
          </div>
          <div className="flex justify-between text-muted-foreground">
            <span>Output tokens</span>
            <span className="text-muted-foreground">{usage.completionTokens.toLocaleString()}</span>
          </div>
          {usage.cacheReadTokens > 0 && (
            <div className="flex justify-between text-muted-foreground">
              <span>Cache read</span>
              <span className="text-muted-foreground">
                {usage.cacheReadTokens.toLocaleString()}
              </span>
            </div>
          )}
          {usage.cacheWriteTokens > 0 && (
            <div className="flex justify-between text-muted-foreground">
              <span>Cache write</span>
              <span className="text-muted-foreground">
                {usage.cacheWriteTokens.toLocaleString()}
              </span>
            </div>
          )}
          {transparency.toolTokensTotal && transparency.toolTokensTotal > 0 && (
            <div className="flex justify-between text-muted-foreground">
              <span>Tool I/O (est.)</span>
              <span className="text-muted-foreground">
                ~{formatTokens(transparency.toolTokensTotal)}
              </span>
            </div>
          )}
          {cost && (
            <>
              <div className="border-t border-border my-1" />
              <div className="flex justify-between text-muted-foreground">
                <span>Model</span>
                <span className="text-muted-foreground">{cost.model}</span>
              </div>
              <div className="flex justify-between text-muted-foreground">
                <span>Cost</span>
                <span className="text-muted-foreground">{formatCost(cost.estimatedUsd)}</span>
              </div>
            </>
          )}
          {timing?.totalMs && timing.totalMs > 0 && (
            <div className="flex justify-between text-muted-foreground">
              <span>Latency</span>
              <span className="text-muted-foreground">{formatDuration(timing.totalMs)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
