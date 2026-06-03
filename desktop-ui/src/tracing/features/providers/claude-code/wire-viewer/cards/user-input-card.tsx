import { Markdown } from "@/tracing/components/markdown";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function UserInputCard({ event }: Props) {
  const p = event.payload as { type?: string; text?: string };
  const isImage = p.type === "image";
  return (
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-blue-500 bg-[rgba(59,130,246,0.05)]">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(59,130,246,0.14)] text-[rgb(29,78,216)] dark:bg-[rgba(59,130,246,0.18)] dark:text-[rgb(147,197,253)]">User</span>
        {isImage && <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">image</span>}
      </div>
      <div className="px-3.5 pb-2.5 text-ui-sm leading-[1.55] text-text-primary">
        {isImage ? <span>[image]</span> : <Markdown>{p.text ?? ""}</Markdown>}
      </div>
    </div>
  );
}
