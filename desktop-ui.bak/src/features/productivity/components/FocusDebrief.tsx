import type { SignalSummary } from "@shared/hooks/useAmbientSignals";
import { Brain, ChevronDown, MessageSquare, Sparkles, X } from "lucide-react";
import { useState } from "react";

// ── CollapsibleBox ────────────────────────────────────────────────────────────

interface CollapsibleBoxProps {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
  defaultOpen?: boolean;
}

function CollapsibleBox({ title, icon: Icon, children, defaultOpen = true }: CollapsibleBoxProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 rounded-t-[var(--radius-2xl)] bg-accent"
      >
        <Icon className="size-3 text-muted-foreground" strokeWidth={1.5} />
        <span className="flex-1 text-left text-[11px] font-medium text-muted-foreground">
          {title}
        </span>
        <ChevronDown
          className={`size-3 text-muted-foreground transition-transform ${open ? "rotate-0" : "-rotate-90"}`}
          strokeWidth={1.5}
        />
      </button>
      {open && <div className="px-3 py-2 space-y-1 text-2xs font-light">{children}</div>}
    </div>
  );
}

// ── FocusDebrief ──────────────────────────────────────────────────────────────

export interface FocusDebriefProps {
  signals: SignalSummary[];
  tooltip: string;
  onClose: () => void;
}

export function FocusDebrief({ signals, tooltip, onClose }: FocusDebriefProps) {
  const messageSignals = signals.filter((s) => s.signalType === "message_deferred");
  const brainSignals = signals.filter(
    (s) => s.signalType === "cross_domain_dot" || s.signalType === "memory_promoted",
  );
  const coachingSignals = signals.filter((s) => s.signalType === "coaching");

  const hasMessages = messageSignals.length > 0;
  const hasBrain = brainSignals.length > 0;
  const hasCoaching = coachingSignals.length > 0;

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: debrief panel stops propagation to prevent orb tooltip dismiss
    <div
      className="glass-panel rounded-xl shadow-xl w-72 animate-[glass-appear_0.2s_ease-out]"
      onMouseDown={(e) => e.stopPropagation()}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2.5 border-b border-border">
        <p className="text-xs font-medium text-foreground">Focus session ended</p>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close debrief"
          className="text-muted-foreground hover:text-foreground transition-colors"
        >
          <X className="size-3.5" strokeWidth={1.5} />
        </button>
      </div>

      {/* Summary tooltip */}
      {tooltip && (
        <p className="px-3 py-2 text-xs text-muted-foreground leading-relaxed border-b border-border">
          {tooltip}
        </p>
      )}

      {/* Sections */}
      <div className="space-y-1 p-1.5">
        {hasMessages && (
          <div className="overflow-hidden rounded-xl">
            <CollapsibleBox title={`Messages held (${messageSignals.length})`} icon={MessageSquare}>
              {messageSignals.map((sig) => (
                <div key={`msg-${sig.entityPair}`} className="text-muted-foreground leading-snug">
                  {sig.headline}
                </div>
              ))}
            </CollapsibleBox>
          </div>
        )}

        {hasBrain && (
          <div className="overflow-hidden rounded-xl">
            <CollapsibleBox title={`Brain activity (${brainSignals.length})`} icon={Brain}>
              {brainSignals.map((sig) => (
                <div key={`brain-${sig.entityPair}`} className="text-muted-foreground leading-snug">
                  {sig.headline}
                </div>
              ))}
            </CollapsibleBox>
          </div>
        )}

        {hasCoaching && (
          <div className="overflow-hidden rounded-xl">
            <CollapsibleBox title={`Coaching (${coachingSignals.length})`} icon={Sparkles}>
              {coachingSignals.map((sig) => (
                <div key={`coach-${sig.entityPair}`} className="text-muted-foreground leading-snug">
                  {sig.headline}
                </div>
              ))}
            </CollapsibleBox>
          </div>
        )}
      </div>
    </div>
  );
}
