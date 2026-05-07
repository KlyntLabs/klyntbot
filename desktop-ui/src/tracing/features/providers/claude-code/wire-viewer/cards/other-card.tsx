import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function OtherCard({ event }: Props) {
  return (
    <div className="cc-card cc-card--other">
      <span className="cc-card__type">{event.type}</span>
      <pre className="cc-card__payload">
        {JSON.stringify(event.payload, null, 2).slice(0, 2000)}
      </pre>
    </div>
  );
}
