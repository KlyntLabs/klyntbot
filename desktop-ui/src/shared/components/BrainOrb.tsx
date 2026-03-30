import { useAmbientSignals } from "@shared/hooks/useAmbientSignals";
import { Shield } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";

// ── BrainOrb ─────────────────────────────────────────────────────────────────
//
// Persistent ambient signal indicator. Sits in the AppShell and surfaces
// brain events (memory connections, focus signals) as visual states with
// glass-panel tooltips on hover.

export function BrainOrb() {
  const { current, badgeCount, isPulsing, isFocusDeferred, badgeSignals, acknowledge, clearBadge } =
    useAmbientSignals();
  const navigate = useNavigate();

  const [tooltipVisible, setTooltipVisible] = useState(false);
  const [badgeTooltipVisible, setBadgeTooltipVisible] = useState(false);
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

  // Close tooltips when pulse ends
  useEffect(() => {
    if (!isPulsing) {
      setTooltipVisible(false);
    }
  }, [isPulsing]);

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
    orbClasses = `${orbBase} bg-warning animate-[orb-pulse_600ms_ease-out]`;
  } else if (isFocusDeferred) {
    orbClasses = `${orbBase} bg-muted-foreground/30 animate-[orb-breathe_3s_ease-in-out_infinite]`;
  } else {
    orbClasses = `${orbBase} bg-muted-foreground/30 animate-[orb-breathe_3s_ease-in-out_infinite]`;
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
            className="absolute inset-0 m-auto text-muted-foreground/50"
            strokeWidth={2.5}
          />
        )}
      </div>

      {/* ── Badge counter ── */}
      {badgeCount > 0 && (
        <span
          className="absolute -top-1.5 -right-1.5 flex items-center justify-center
            w-3.5 h-3.5 rounded-full bg-warning text-warning-foreground text-[8px] font-bold
            leading-none select-none"
        >
          {badgeCount > 9 ? "9+" : badgeCount}
        </span>
      )}

      {/* ── Pulse tooltip ── */}
      {tooltipVisible && isPulsing && current?.tooltip && (
        // biome-ignore lint/a11y/noStaticElementInteractions: tooltip needs hover to stay open
        <div
          className="absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50
            glass-panel rounded-xl p-3 max-w-[320px] w-max shadow-xl
            animate-[fade-in_0.2s_ease-out]"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <p className="text-xs text-foreground leading-relaxed">{current.tooltip}</p>
          {current.detailRoute && (
            <button
              type="button"
              onClick={handleDetailRouteClick}
              className="mt-2 text-xs text-warning hover:text-warning/80 transition-colors font-medium"
            >
              See more →
            </button>
          )}
        </div>
      )}

      {/* ── Badge summary tooltip ── */}
      {badgeTooltipVisible && badgeCount > 0 && (
        // biome-ignore lint/a11y/noStaticElementInteractions: tooltip needs hover to stay open
        <div
          className="absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50
            glass-panel rounded-xl p-3 max-w-[320px] w-max shadow-xl
            animate-[fade-in_0.2s_ease-out]"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <p className="text-xs text-muted-foreground mb-2">
            {badgeCount} new {badgeCount === 1 ? "connection" : "connections"}
          </p>
          <ul className="space-y-1">
            {badgeSignals.slice(0, 5).map((sig, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: badge signals are ephemeral display-only list
              <li key={i} className="text-xs text-foreground leading-snug">
                {sig.headline}
              </li>
            ))}
            {badgeSignals.length > 5 && (
              <li className="text-xs text-muted-foreground">+{badgeSignals.length - 5} more</li>
            )}
          </ul>
          <button
            type="button"
            onClick={handleSeeAllClick}
            className="mt-2 text-xs text-warning hover:text-warning/80 transition-colors font-medium"
          >
            See all →
          </button>
        </div>
      )}
    </div>
  );
}
