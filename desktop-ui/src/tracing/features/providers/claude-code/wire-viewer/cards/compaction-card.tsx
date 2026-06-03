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
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-orange-500 bg-[rgba(249,115,22,0.05)]">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(249,115,22,0.14)] text-[rgb(154,52,18)] dark:bg-[rgba(249,115,22,0.18)] dark:text-[rgb(253,186,116)]">
          <RefreshCw size={11} aria-hidden />
          Compacted
        </span>
        <span className="font-code text-ui-2xs text-[rgb(154,52,18)] dark:text-[rgb(253,186,116)]">{meta.trigger ?? "auto"}</span>
        <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">{Math.round((meta.durationMs ?? 0) / 1000)}s</span>
      </div>
      <div className="flex flex-wrap items-center gap-2 px-3.5 pb-2.5 text-ui-xs text-text-muted">
        <span>
          {(meta.preTokens ?? 0).toLocaleString()} →{" "}
          {(meta.postTokens ?? 0).toLocaleString()} tokens
        </span>
      </div>
      {meta.preCompactDiscoveredTools && meta.preCompactDiscoveredTools.length > 0 && (
        <ul className="m-0 px-3.5 pb-2.5 flex flex-wrap gap-1 list-none">
          {meta.preCompactDiscoveredTools.map((t) => (
            <li key={t} className="py-px px-2 rounded-full bg-surface-card-muted text-ui-2xs text-text-muted font-code">{t}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
