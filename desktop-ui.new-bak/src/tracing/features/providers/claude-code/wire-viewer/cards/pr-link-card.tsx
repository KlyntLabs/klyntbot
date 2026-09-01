import { GitPullRequest } from "lucide-react";
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
      <div className="cc-card__header">
        <span className="cc-card__role cc-card__role--pr">
          <GitPullRequest size={11} aria-hidden />
          PR
        </span>
        <span className="cc-card__tool-name">{p.prRepository ?? ""}</span>
        <span className="cc-card__meta">
          {p.prUrl ? (
            <a href={p.prUrl} target="_blank" rel="noopener noreferrer">
              #{p.prNumber}
            </a>
          ) : (
            <span>#{p.prNumber}</span>
          )}
        </span>
      </div>
    </div>
  );
}
