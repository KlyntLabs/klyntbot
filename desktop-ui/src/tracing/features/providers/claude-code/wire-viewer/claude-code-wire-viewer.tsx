import type { WireEvent } from "@/tracing/lib/api";
import { OtherCard } from "./cards/other-card";

interface Props {
  events: WireEvent[];
}

export function ClaudeCodeWireViewer({ events }: Props) {
  return (
    <div className="cc-wire-viewer">
      {events.map((e) => (
        <OtherCard key={e.index} event={e} />
      ))}
    </div>
  );
}
