import { useDismiss } from "@shared/hooks";
import { Activity, ArrowRight, Radio } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface FabricPulseBadgeProps {
  lastActivityTimestamp: string;
  livePulseActive: boolean;
  onViewUpdates: () => void;
  onSwitchToSemantic: () => void;
}

function formatTimeAgo(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  if (diff < 0) return "just now";
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

export function FabricPulseBadge({
  lastActivityTimestamp,
  livePulseActive,
  onViewUpdates,
  onSwitchToSemantic,
}: FabricPulseBadgeProps) {
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [timeAgo, setTimeAgo] = useState(() => formatTimeAgo(lastActivityTimestamp));
  const badgeRef = useRef<HTMLDivElement>(null);

  // Update time-ago text periodically
  useEffect(() => {
    setTimeAgo(formatTimeAgo(lastActivityTimestamp));
    const interval = setInterval(() => {
      setTimeAgo(formatTimeAgo(lastActivityTimestamp));
    }, 10_000);
    return () => clearInterval(interval);
  }, [lastActivityTimestamp]);

  useDismiss(badgeRef, () => setPopoverOpen(false), popoverOpen);

  return (
    <div ref={badgeRef} className="relative">
      {/* Badge pill */}
      <button
        type="button"
        onClick={() => setPopoverOpen(!popoverOpen)}
        className="flex items-center gap-1.5 px-2.5 py-1 rounded-full
          glass-panel text-2xs text-muted-foreground
          hover:text-foreground transition-colors"
      >
        <span className="relative flex size-2">
          <span
            className={`absolute inset-0 rounded-full bg-green-400 ${
              livePulseActive ? "animate-ping opacity-75" : ""
            }`}
          />
          <span className="relative inline-flex rounded-full size-2 bg-green-500" />
        </span>
        <span>Fabric updated {timeAgo}</span>
      </button>

      {/* Popover */}
      {popoverOpen && (
        <div className="absolute bottom-full left-0 mb-2 glass-panel rounded-lg py-1 min-w-[200px] shadow-xl">
          <button
            type="button"
            className="flex items-center gap-2 w-full px-3 py-2 text-xs text-foreground
              hover:bg-surface-raised/50 transition-colors text-left"
            onClick={() => {
              onViewUpdates();
              setPopoverOpen(false);
            }}
          >
            <Activity size={12} className="text-muted-foreground" />
            View latest updates
          </button>
          <button
            type="button"
            className="flex items-center gap-2 w-full px-3 py-2 text-xs text-foreground
              hover:bg-surface-raised/50 transition-colors text-left"
            onClick={() => {
              onSwitchToSemantic();
              setPopoverOpen(false);
            }}
          >
            <ArrowRight size={12} className="text-muted-foreground" />
            Switch to Semantic
          </button>
        </div>
      )}
    </div>
  );
}

// ── Multi-Select Batch Action Bar ────────────────────────────────────────

interface BatchActionBarProps {
  selectedCount: number;
  onExpandAllTrees: () => void;
  onCreateBridgeNote: () => void;
  onCompareInChat: () => void;
}

export function BatchActionBar({
  selectedCount,
  onExpandAllTrees,
  onCreateBridgeNote,
  onCompareInChat,
}: BatchActionBarProps) {
  if (selectedCount < 2) return null;

  return (
    <div
      className="absolute bottom-4 left-1/2 -translate-x-1/2 z-20
        glass-panel rounded-full px-2 py-1.5 flex items-center gap-1.5 shadow-xl"
    >
      <span className="text-2xs text-muted-foreground px-2">{selectedCount} selected</span>
      <div className="w-px h-4 bg-border-subtle" />
      <button
        type="button"
        onClick={onExpandAllTrees}
        className="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-2xs
          text-foreground hover:bg-surface-raised/50 transition-colors"
      >
        <Radio size={10} />
        Expand all trees
      </button>
      <button
        type="button"
        onClick={onCreateBridgeNote}
        className="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-2xs
          text-foreground hover:bg-surface-raised/50 transition-colors"
      >
        <Activity size={10} />
        Create bridge note
      </button>
      <button
        type="button"
        onClick={onCompareInChat}
        className="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-2xs
          text-foreground hover:bg-surface-raised/50 transition-colors"
      >
        <ArrowRight size={10} />
        Compare in chat
      </button>
    </div>
  );
}
