import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function OtherCard({ event }: Props) {
  return (
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-border-subtle opacity-70">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="font-code text-ui-2xs text-text-muted uppercase tracking-[0.04em]">{event.type}</span>
      </div>
      <pre className="m-0 py-2 px-3.5 font-code text-ui-2xs overflow-auto max-h-[30vh] bg-[rgba(0,0,0,0.03)] dark:bg-[rgba(0,0,0,0.2)] border-t border-border-subtle">
        {JSON.stringify(event.payload, null, 2).slice(0, 2000)}
      </pre>
    </div>
  );
}
