import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function PrLinkCard({ event }: Props) {
  const p = event.payload as {
    prNumber?: number;
    prUrl?: string;
    prRepository?: string;
  };
  return (
    <div className="cc-card cc-card--pr-link">
      <span className="cc-card__chip">PR</span>
      <span>{p.prRepository ?? ""}</span>
      {p.prUrl ? (
        <a href={p.prUrl} target="_blank" rel="noopener noreferrer">
          #{p.prNumber}
        </a>
      ) : (
        <span>#{p.prNumber}</span>
      )}
    </div>
  );
}
