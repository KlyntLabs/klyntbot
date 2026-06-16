import { RefreshCw } from "lucide-react";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function CompactionCard({ event }: Props) {
  const meta =
    (
      event.payload as {
        compactMetadata?: {
          trigger?: string;
          preTokens?: number;
          postTokens?: number;
          durationMs?: number;
          preCompactDiscoveredTools?: string[];
        };
      }
    ).compactMetadata ?? {};
  return (
    <div className="cc-card cc-card--compaction">
      <div className="cc-card__header">
        <span className="cc-card__role cc-card__role--compaction">
          <RefreshCw size={11} aria-hidden />
          Compacted
        </span>
        <span className="cc-card__compact-trigger">{meta.trigger ?? "auto"}</span>
        <span className="cc-card__meta">{Math.round((meta.durationMs ?? 0) / 1000)}s</span>
      </div>
      <div className="cc-card__details">
        <span>
          {(meta.preTokens ?? 0).toLocaleString()} → {(meta.postTokens ?? 0).toLocaleString()}{" "}
          tokens
        </span>
      </div>
      {meta.preCompactDiscoveredTools && meta.preCompactDiscoveredTools.length > 0 && (
        <ul className="cc-card__chip-list">
          {meta.preCompactDiscoveredTools.map((t) => (
            <li key={t}>{t}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
