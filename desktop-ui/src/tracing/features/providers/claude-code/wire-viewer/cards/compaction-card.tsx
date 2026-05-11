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
      <strong>Conversation compacted</strong>
      <span className="cc-card__compact-trigger">{meta.trigger ?? "auto"}</span>
      <span>
        {(meta.preTokens ?? 0).toLocaleString()} → {(meta.postTokens ?? 0).toLocaleString()} tokens
      </span>
      <span>{Math.round((meta.durationMs ?? 0) / 1000)}s</span>
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
