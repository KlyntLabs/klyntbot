import { X } from "lucide-react";
import type { PinnedMessage } from "../../lib/session-types";

interface PinnedContextStripProps {
  pins: PinnedMessage[];
  onUnpin: (pinId: number) => void;
}

export function PinnedContextStrip({ pins, onUnpin }: PinnedContextStripProps) {
  if (pins.length === 0) return null;

  return (
    <div className="flex gap-2 overflow-x-auto px-4 py-2">
      {pins.map((pin) => (
        <div
          key={pin.id}
          className="glass-badge flex items-center gap-1.5 px-2.5 py-1.5 shrink-0 max-w-[200px]"
        >
          <span className="text-[10px] text-dim uppercase">{pin.messageRole}</span>
          <span className="text-[11px] text-muted font-light truncate">{pin.messageContent}</span>
          <button
            type="button"
            onClick={() => onUnpin(pin.id)}
            className="w-4 h-4 rounded flex items-center justify-center text-dim hover:text-muted shrink-0"
          >
            <X className="w-3 h-3" strokeWidth={1.5} />
          </button>
        </div>
      ))}
    </div>
  );
}
