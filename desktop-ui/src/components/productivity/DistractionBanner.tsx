import { useMemo, useState } from "react";
import type { ProductivitySummary } from "../../lib/types";

interface DistractionBannerProps {
  summary: ProductivitySummary | null;
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export function DistractionBanner({ summary }: DistractionBannerProps) {
  const [dismissed, setDismissed] = useState(false);

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
      .filter((c) => {
        const lower = c.category.toLowerCase();
        return lower === "entertainment" || lower === "social media" || lower === "gaming";
      })
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
      <div className="relative flex items-center gap-3 bg-surface-base/80 px-4 py-3">
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
              {formatDuration(distractingSecs)}
            </span>
            <span className="text-[12px] text-muted">
              on distracting apps
              {distractingPct > 0 && (
                <span className="text-dim"> · {distractingPct}% of active time</span>
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
            className="flex-shrink-0 text-[10px] font-medium px-2 py-0.5 rounded-full"
            style={{
              background:
                severity === "high" ? "rgba(212, 24, 61, 0.15)" : "rgba(212, 24, 61, 0.08)",
              color: "var(--destructive)",
              border: "1px solid rgba(212, 24, 61, 0.2)",
            }}
          >
            {severity === "high" ? "High" : "Moderate"}
          </span>
        )}

        {/* Dismiss */}
        <button
          onClick={() => setDismissed(true)}
          className="flex-shrink-0 p-1 rounded-md text-dim hover:text-muted hover:bg-surface-raised transition-colors"
          aria-label="Dismiss"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path
              d="M10.5 3.5L3.5 10.5M3.5 3.5L10.5 10.5"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
        </button>
      </div>

      <style>{`
        @keyframes distraction-pulse {
          0%, 100% { opacity: 0.07; }
          50% { opacity: 0.12; }
        }
      `}</style>
    </div>
  );
}
