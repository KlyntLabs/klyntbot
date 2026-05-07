import { Markdown } from "@/tracing/components/markdown";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function AssistantTextCard({ event }: Props) {
  const text = (event.payload as { text?: string }).text ?? "";
  return (
    <div className="cc-card cc-card--assistant-text">
      <Markdown>{text}</Markdown>
    </div>
  );
}
