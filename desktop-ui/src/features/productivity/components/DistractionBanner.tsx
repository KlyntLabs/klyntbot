import { ipc } from "@shared/hooks/useIpc";
import { formatHumanDuration } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { useEffect, useMemo, useState } from "react";

interface DistractionBannerProps {
  summary: ProductivitySummary | null;
}

export function DistractionBanner({ summary }: DistractionBannerProps) {
  const [dismissed, setDismissed] = useState(false);
  const [urgeSurfingCountdown, setUrgeSurfingCountdown] = useState(10);

  useEffect(() => {
    if (urgeSurfingCountdown <= 0 || dismissed) return;
    const timer = setTimeout(() => setUrgeSurfingCountdown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [urgeSurfingCountdown, dismissed]);

  const distractingSecs = summary?.distractingSecs ?? 0;
  const totalActive = summary?.totalActiveSecs ?? 0;

  const distractingPct = useMemo(
    () => (totalActive > 0 ? Math.round((distractingSecs / totalActive) * 100) : 0),
    [distractingSecs, totalActive],
  );

  // Find the "Entertainment" category duration from topCategories
  const distractingApps = useMemo(() => {
    if (!summary) return [];
    // topApps with heuristic: if the app contributed to entertainment, list it
    // We check topCategories for the distracting category name
    return summary.topCategories
      .filter((c) => c.categoryType === "distracting")
      .map((c) => c.category);
  }, [summary]);

  if (distractingSecs === 0 || dismissed) return null;

  // Severity levels drive visual intensity
  const severity: "low" | "medium" | "high" =
    distractingPct >= 30 ? "high" : distractingPct >= 15 ? "medium" : "low";

  return (
    <div className="col-span-3 relative overflow-hidden rounded-xl">
      {/* Animated gradient background */}
      <div
        className="absolute inset-0 opacity-[0.07]"
        style={{
          background:
            severity === "high"
              ? "linear-gradient(135deg, var(--destructive), transparent 60%), linear-gradient(225deg, var(--destructive), transparent 60%)"
              : "linear-gradient(135deg, var(--destructive), transparent 70%)",
          animation: severity === "high" ? "distraction-pulse 3s ease-in-out infinite" : undefined,
        }}
      />

      {/* Left accent bar */}
      <div
        className="absolute left-0 top-0 bottom-0 w-[3px] rounded-full"
        style={{ background: "var(--destructive)" }}
      />

      {/* Content */}
      <div className="relative flex items-center gap-3 bg-white/[0.04] px-4 py-3">
        {/* Pulsing dot */}
        <div className="relative flex-shrink-0">
          <span
            className="block w-2 h-2 rounded-full"
            style={{ background: "var(--destructive)" }}
          />
          {severity !== "low" && (
            <span
              className="absolute inset-0 w-2 h-2 rounded-full animate-ping"
              style={{ background: "var(--destructive)", opacity: 0.4 }}
            />
          )}
        </div>

        {/* Message */}
        <div className="flex-1 min-w-0">
          <div className="flex items-baseline gap-2">
            <span className="text-[13px] font-medium" style={{ color: "var(--destructive)" }}>
              {formatHumanDuration(distractingSecs)}
            </span>
            <span className="text-[12px] text-muted">
              on distracting apps
              {distractingPct > 0 && (
                <span className="text-dim tabular-nums"> · {distractingPct}% of active time</span>
              )}
            </span>
          </div>
          {distractingApps.length > 0 && (
            <div className="text-[11px] text-dim mt-0.5 truncate">{distractingApps.join(", ")}</div>
          )}
        </div>

        {/* Severity badge */}
        {severity !== "low" && (
          <span
            className={`flex-shrink-0 text-[10px] font-medium px-2 py-0.5 rounded-full text-destructive border border-destructive/20 ${
              severity === "high" ? "bg-destructive/15" : "bg-destructive/8"
            }`}
          >
            {severity === "high" ? "High" : "Moderate"}
          </span>
        )}

        {/* Dismiss with urge surfing */}
        <button
          type="button"
          disabled={urgeSurfingCountdown > 0}
          onClick={() => {
            setDismissed(true);
            ipc("distraction_dismiss", { app_name: distractingApps[0] ?? "unknown" }).catch(
              () => {},
            );
          }}
          className={`flex-shrink-0 px-2 py-1 rounded-md text-[10px] transition-colors ${
            urgeSurfingCountdown > 0
              ? "text-dim cursor-not-allowed animate-[breathe_4s_ease-in-out_infinite]"
              : "text-muted hover:text-secondary hover:bg-white/[0.08]"
          }`}
          aria-label={urgeSurfingCountdown > 0 ? "Pause and reflect" : "Dismiss"}
        >
          {urgeSurfingCountdown > 0 ? `Pause and reflect... (${urgeSurfingCountdown}s)` : "Dismiss"}
        </button>
      </div>
    </div>
  );
}
