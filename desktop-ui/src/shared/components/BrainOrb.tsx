import { FocusDebrief } from "@features/productivity/components/FocusDebrief";
import { useAmbientSignals } from "@shared/hooks/useAmbientSignals";
import { useJourney } from "@shared/hooks/useJourney";
import { useQuery } from "@shared/hooks/useQuery";
import { Shield } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";

// ── BrainOrb ─────────────────────────────────────────────────────────────────
//
// Persistent ambient signal indicator. Sits in the AppShell and surfaces
// brain events (memory connections, focus signals) as visual states with
// liquid-glass tooltips on hover.

export function BrainOrb() {
  const { current, badgeCount, isPulsing, isFocusDeferred, badgeSignals, acknowledge, clearBadge } =
    useAmbientSignals();
  const { isComplete, markComplete } = useJourney();
  const { data: itemCount } = useQuery<number>("journey_item_count", undefined);
  const navigate = useNavigate();

  const [tooltipVisible, setTooltipVisible] = useState(false);
  const [badgeTooltipVisible, setBadgeTooltipVisible] = useState(false);
  const [hasRecentSignals, setHasRecentSignals] = useState(false);
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-dismiss pulse tooltip after 8s when cursor leaves
  const handleMouseEnter = () => {
    if (hoverTimerRef.current !== null) {
      clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    if (isPulsing && current?.tooltip) {
      setTooltipVisible(true);
    }
    if (badgeCount > 0) {
      setBadgeTooltipVisible(true);
    }
  };

  const handleMouseLeave = () => {
    hoverTimerRef.current = setTimeout(() => {
      setTooltipVisible(false);
      setBadgeTooltipVisible(false);
      hoverTimerRef.current = null;
    }, 8000);
  };

  // Close pulse tooltip when pulse ends, but keep debrief panel open
  useEffect(() => {
    if (!isPulsing && current?.mode !== "deferred") {
      setTooltipVisible(false);
    }
  }, [isPulsing, current?.mode]);

  // Close badge tooltip when badge is cleared
  useEffect(() => {
    if (badgeCount === 0) {
      setBadgeTooltipVisible(false);
    }
  }, [badgeCount]);

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current !== null) {
        clearTimeout(hoverTimerRef.current);
      }
    };
  }, []);

  // Track whether a signal arrived recently (within 5 minutes) for the idle breathing state
  useEffect(() => {
    if (current) {
      setHasRecentSignals(true);
      const timer = setTimeout(() => setHasRecentSignals(false), 5 * 60 * 1000);
      return () => clearTimeout(timer);
    }
  }, [current]);

  // Wire: QuietDay journey milestone — fires when orb is idle for 10s after mount
  useEffect(() => {
    if (isComplete("quiet_day")) return;
    const timer = setTimeout(() => {
      // Orb is idle: no active pulse, no badge, no deferred, no recent signals
      if (!isPulsing && badgeCount === 0 && !isFocusDeferred && !current) {
        markComplete("quiet_day");
      }
    }, 10_000);
    return () => clearTimeout(timer);
    // Only run on mount + when idle conditions change
  }, [isPulsing, badgeCount, isFocusDeferred, current, isComplete, markComplete]);

  const handleClick = () => {
    if (isPulsing && current?.detailRoute) {
      acknowledge();
      navigate(current.detailRoute);
      return;
    }
    if (badgeCount > 0) {
      setBadgeTooltipVisible((v) => !v);
      return;
    }
    navigate("/brain");
  };

  const handleSeeAllClick = () => {
    clearBadge();
    setBadgeTooltipVisible(false);
    navigate("/brain");
  };

  const handleDetailRouteClick = () => {
    if (current?.detailRoute) {
      // Wire: FirstDotAccepted journey milestone when user clicks through a cross-domain dot
      if (
        !isComplete("first_dot_accepted") &&
        current.signals.some((s) => s.signalType === "cross_domain_dot")
      ) {
        markComplete("first_dot_accepted");
      }
      acknowledge();
      setTooltipVisible(false);
      navigate(current.detailRoute);
    }
  };

  // ── Orb dot classes ───────────────────────────────────────────────────────

  const orbBase =
    "relative flex items-center justify-center w-2.5 h-2.5 rounded-full transition-all duration-300 cursor-pointer";

  let orbClasses = orbBase;
  if (isPulsing) {
    orbClasses = `${orbBase} bg-status-warning animate-[orb-pulse_600ms_ease-out]`;
  } else if (isFocusDeferred) {
    orbClasses = `${orbBase} bg-fg-secondary/30 animate-[orb-breathe_3s_ease-in-out_infinite]`;
  } else if (hasRecentSignals && badgeCount === 0) {
    // Slow, slightly brighter breathing — signals arrived recently but orb is now idle
    orbClasses = `${orbBase} bg-fg-secondary/50 animate-[orb-breathe_8s_ease-in-out_infinite]`;
  } else {
    orbClasses = `${orbBase} bg-fg-secondary/20`;
  }

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: orb is an ambient visual indicator with click shortcut
    <div
      ref={containerRef}
      className="relative flex items-center justify-center"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onClick={handleClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") handleClick();
      }}
    >
      {/* ── Orb dot ── */}
      <div className={orbClasses}>
        {/* Focus-deferred shield overlay */}
        {isFocusDeferred && (
          <Shield
            size={8}
            className="absolute inset-0 m-auto text-fg-secondary/50"
            strokeWidth={2.5}
          />
        )}
      </div>

      {/* ── Badge counter ── */}
      {badgeCount > 0 && (
        <span
          className="absolute -top-1.5 -right-1.5 flex items-center justify-center
            w-3.5 h-3.5 rounded-full bg-status-warning text-brand-foreground text-[8px] font-bold
            leading-none select-none"
        >
          {badgeCount > 9 ? "9+" : badgeCount}
        </span>
      )}

      {/* ── Pulse tooltip / Focus debrief / Guided onboarding ── */}
      {tooltipVisible && current?.mode === "deferred" ? (
        <div className="absolute top-full right-0 mt-2 z-50">
          <FocusDebrief
            signals={current.signals}
            tooltip={current.tooltip}
            onClose={() => {
              setTooltipVisible(false);
              acknowledge();
            }}
          />
        </div>
      ) : tooltipVisible &&
        isPulsing &&
        current &&
        !isComplete("orb_awakening") &&
        (itemCount ?? 0) >= 3 ? (
        // biome-ignore lint/a11y/noStaticElementInteractions: guided tooltip needs hover to stay open
        <div
          className="absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50
            liquid-glass rounded-panel w-80 p-4 space-y-3 shadow-xl
            animate-[fade-in_0.2s_ease-out]"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <p className="text-ui text-fg">
            Hey — I'm your second brain's heartbeat. I only light up when I've connected something
            worth your attention.
          </p>
          <p className="text-ui-sm text-fg-secondary">{current.tooltip}</p>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => {
                markComplete("orb_awakening");
                if (current.detailRoute) navigate(current.detailRoute);
              }}
              className="text-ui-sm px-3 py-1.5 rounded-control bg-status-warning text-brand-foreground hover:bg-status-warning/90"
            >
              Show me →
            </button>
            <button
              type="button"
              onClick={() => {
                markComplete("orb_awakening");
                setTooltipVisible(false);
              }}
              className="text-ui-sm px-3 py-1.5 rounded-control bg-control-hover text-fg hover:bg-control-hover/80"
            >
              Got it — keep whispering
            </button>
          </div>
        </div>
      ) : tooltipVisible && isPulsing && current?.tooltip ? (
        // biome-ignore lint/a11y/noStaticElementInteractions: tooltip needs hover to stay open
        <div
          className="absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50
            liquid-glass rounded-panel p-3 max-w-[320px] w-max shadow-xl
            animate-[fade-in_0.2s_ease-out]"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <p className="text-ui-sm text-fg leading-relaxed">{current.tooltip}</p>
          {current.detailRoute && (
            <button
              type="button"
              onClick={handleDetailRouteClick}
              className="mt-2 text-ui-sm text-status-warning hover:text-status-warning/80 transition-colors font-medium"
            >
              See more →
            </button>
          )}
        </div>
      ) : null}

      {/* ── Badge summary tooltip ── */}
      {badgeTooltipVisible && badgeCount > 0 && (
        // biome-ignore lint/a11y/noStaticElementInteractions: tooltip needs hover to stay open
        <div
          className="absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50
            liquid-glass rounded-panel p-3 max-w-[320px] w-max shadow-xl
            animate-[fade-in_0.2s_ease-out]"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <p className="text-ui-sm text-fg-secondary mb-2">
            {badgeCount} new {badgeCount === 1 ? "connection" : "connections"}
          </p>
          <ul className="space-y-1">
            {badgeSignals.slice(0, 5).map((sig, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: badge signals are ephemeral display-only list
              <li key={i} className="text-ui-sm text-fg leading-snug">
                {sig.headline}
              </li>
            ))}
            {badgeSignals.length > 5 && (
              <li className="text-ui-sm text-fg-secondary">+{badgeSignals.length - 5} more</li>
            )}
          </ul>
          <button
            type="button"
            onClick={handleSeeAllClick}
            className="mt-2 text-ui-sm text-status-warning hover:text-status-warning/80 transition-colors font-medium"
          >
            See all →
          </button>
        </div>
      )}
    </div>
  );
}
