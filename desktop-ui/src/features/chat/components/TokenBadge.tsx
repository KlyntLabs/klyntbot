import { formatCost, formatDuration, formatTokens } from "@shared/lib/utils";
import type { TransparencyData } from "@shared/types";
import { ChevronDown } from "lucide-react";
import { useState } from "react";

interface TokenBadgeProps {
  transparency: TransparencyData;
}

export function TokenBadge({ transparency }: TokenBadgeProps) {
  const [expanded, setExpanded] = useState(false);
  const { usage, cost, timing } = transparency;

  // Nothing to show until usage data arrives
  if (!usage) return null;

  return (
    <div className="mt-1.5">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="ml-auto flex items-center gap-1.5 text-ui-xs font-light text-fg-dim hover:text-fg-secondary transition-colors"
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
        <div className="mt-1.5 p-2.5 island text-ui-xs font-light space-y-1">
          <div className="flex justify-between text-fg-secondary">
            <span>Input tokens</span>
            <span className="text-fg-secondary">{usage.promptTokens.toLocaleString()}</span>
          </div>
          <div className="flex justify-between text-fg-secondary">
            <span>Output tokens</span>
            <span className="text-fg-secondary">{usage.completionTokens.toLocaleString()}</span>
          </div>
          {usage.cacheReadTokens > 0 && (
            <div className="flex justify-between text-fg-secondary">
              <span>Cache read</span>
              <span className="text-fg-secondary">
                {usage.cacheReadTokens.toLocaleString()}
              </span>
            </div>
          )}
          {usage.cacheWriteTokens > 0 && (
            <div className="flex justify-between text-fg-secondary">
              <span>Cache write</span>
              <span className="text-fg-secondary">
                {usage.cacheWriteTokens.toLocaleString()}
              </span>
            </div>
          )}
          {transparency.toolTokensTotal && transparency.toolTokensTotal > 0 && (
            <div className="flex justify-between text-fg-secondary">
              <span>Tool I/O (est.)</span>
              <span className="text-fg-secondary">
                ~{formatTokens(transparency.toolTokensTotal)}
              </span>
            </div>
          )}
          {cost && (
            <>
              <div className="border-t border-separator my-1" />
              <div className="flex justify-between text-fg-secondary">
                <span>Model</span>
                <span className="text-fg-secondary">{cost.model}</span>
              </div>
              <div className="flex justify-between text-fg-secondary">
                <span>Cost</span>
                <span className="text-fg-secondary">{formatCost(cost.estimatedUsd)}</span>
              </div>
            </>
          )}
          {timing?.totalMs && timing.totalMs > 0 && (
            <div className="flex justify-between text-fg-secondary">
              <span>Latency</span>
              <span className="text-fg-secondary">{formatDuration(timing.totalMs)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
