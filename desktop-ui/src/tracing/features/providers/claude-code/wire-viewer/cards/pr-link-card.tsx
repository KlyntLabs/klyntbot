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
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-pink-500">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(236,72,153,0.14)] text-[rgb(157,23,77)] dark:bg-[rgba(236,72,153,0.18)] dark:text-[rgb(249,168,212)]">
          <GitPullRequest size={11} aria-hidden />
          PR
        </span>
        <span className="font-code font-semibold text-text-primary text-ui-xs shrink-0">{p.prRepository ?? ""}</span>
        <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">
          {p.prUrl ? (
            <a href={p.prUrl} target="_blank" rel="noopener noreferrer" className="text-text-accent-cyan no-underline hover:underline">
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
