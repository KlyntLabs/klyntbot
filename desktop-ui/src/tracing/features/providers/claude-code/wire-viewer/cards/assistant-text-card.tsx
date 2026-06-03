import { Markdown } from "@/tracing/components/markdown";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function AssistantTextCard({ event }: Props) {
  const text = (event.payload as { text?: string }).text ?? "";
  return (
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-emerald-500">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(16,185,129,0.14)] text-[rgb(4,120,87)] dark:bg-[rgba(16,185,129,0.18)] dark:text-[rgb(110,231,183)]">Assistant</span>
      </div>
      <div className="px-3.5 pb-2.5 text-ui-sm leading-[1.55] text-text-primary">
        <Markdown>{text}</Markdown>
      </div>
    </div>
  );
}
