import { Markdown } from "@/tracing/components/markdown";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function UserInputCard({ event }: Props) {
  const p = event.payload as { type?: string; text?: string };
  const isImage = p.type === "image";
  return (
    <div className="cc-card cc-card--user">
      {isImage ? <span>[image]</span> : <Markdown>{p.text ?? ""}</Markdown>}
    </div>
  );
}
