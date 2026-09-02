import { useQuery } from "@shared/hooks/useQuery";
import { formatTime } from "@shared/lib/dates";
import { AppIcon, getAppColor } from "@shared/lib/productivity";
import type { ActivityTimeline } from "@shared/types";
import { useEffect, useRef, useState } from "react";

const BROWSER_RE =
  /\s*[-\u2013\u2014]\s*(?:Google Chrome|Safari|Firefox|Arc|Brave Browser|Microsoft Edge|Orion|Vivaldi|Opera|Chromium|Zen Browser)(?:\s*[-\u2013\u2014]\s*.+)?$/i;

function stripBrowserSuffix(title: string): string {
  return title.replace(BROWSER_RE, "").trim();
}

function resolveDisplayName(e: ActivityTimeline): { name: string; subtitle?: string } {
  if (e.isIdle) return { name: "Idle" };
  if (e.siteName && e.windowTitle) {
    const pageTitle = stripBrowserSuffix(e.windowTitle);
    if (pageTitle && pageTitle.toLowerCase() !== e.siteName.toLowerCase()) {
      return { name: e.siteName, subtitle: pageTitle };
    }
  }
  // Apps with a detected project: show app name primary, project as subtitle
  if (e.projectId) {
    return { name: e.appName, subtitle: e.projectId };
  }
  return { name: e.siteName ?? e.appName };
}

function ageSecs(dateStr: string): number {
  return Math.max(0, Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000));
}

function relativeTag(secs: number): string | null {
  if (secs < 10) return "now";
  if (secs < 60) return `${secs}s`;
  if (secs < 300) return `${Math.floor(secs / 60)}m`;
  return null;
}

export function ActivityFeed() {
  const { data: events } = useQuery<ActivityTimeline[]>(
    "productivity_activity_feed",
    { limit: 30 },
    [],
    {
      invalidateOn: ["entity:updated", "activity:switch"],
      invalidateFilter: (p) => {
        const kind = (p as { entityKind?: string })?.entityKind;
        return !kind || kind === "productivity";
      },
    },
  );

  // Track which keys are "new" for animation
  const prevKeysRef = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());

  const scrollRef = useRef<HTMLDivElement>(null);

  // Refresh relative time labels periodically (no data fetch, just re-render)
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 30_000);
    return () => clearInterval(id);
  }, []);

  // Detect new entries and animate them
  useEffect(() => {
    const currentKeys = new Set(events.map((e, i) => `${e.startedAt}-${e.appName}-${i}`));
    const fresh = new Set<string>();
    for (const k of currentKeys) {
      if (!prevKeysRef.current.has(k)) fresh.add(k);
    }
    prevKeysRef.current = currentKeys;

    if (fresh.size > 0) {
      setNewKeys(fresh);
      // Scroll to top to show new entries
      scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
      // Clear "new" state after animation
      const timer = setTimeout(() => setNewKeys(new Set()), 600);
      return () => clearTimeout(timer);
    }
  }, [events]);

  if (events.length === 0) {
    return (
      <div className="island p-4">
        <h2 className="text-ui font-medium text-fg-secondary mb-3">Activity</h2>
        <p className="text-ui-sm font-light text-fg-dim">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="island p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-ui font-medium text-fg-secondary">Activity</h2>
        <div className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-status-success animate-pulse" />
          <span className="text-ui-xs font-light text-fg-dim">Live</span>
        </div>
      </div>
      <div ref={scrollRef} className="flex flex-col gap-0 max-h-64 overflow-y-auto">
        {events.map((e, i) => {
          const { name, subtitle } = resolveDisplayName(e);
          const color = getAppColor(name, e.categoryId);
          const isFirst = i === 0;
          const key = `${e.startedAt}-${e.appName}-${i}`;
          const isNew = newKeys.has(key);
          const age = ageSecs(e.startedAt);
          const tag = relativeTag(age);
          const isRecent = age < 60;

          return (
            <div
              key={key}
              className={`flex items-center gap-2 py-1.5 ${isFirst ? "" : "border-t border-separator"}`}
              style={isNew ? { animation: "fade-in 0.4s ease-out" } : undefined}
            >
              {/* App icon */}
              <div className="flex-shrink-0">
                {e.isIdle ? (
                  <span className="size-3.5 rounded-full bg-control-hover block" />
                ) : (
                  <AppIcon appName={name} color={color} />
                )}
              </div>

              {/* Time */}
              <span className="text-ui-xs tabular-nums w-10 flex-shrink-0 font-light text-fg-dim">
                {formatTime(e.startedAt)}
              </span>
              {tag && (
                <span
                  className={`text-[9px] font-medium tabular-nums flex-shrink-0 ${isRecent ? "text-status-success" : "text-fg-secondary"}`}
                >
                  {tag}
                </span>
              )}

              {/* App/Site name */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span
                    className={`text-ui-xs truncate ${e.isIdle ? "text-fg-dim italic" : isFirst ? "font-normal text-fg" : "font-light text-fg-secondary"}`}
                  >
                    {name}
                  </span>
                </div>
                {subtitle && !e.isIdle && (
                  <p className="text-[9px] font-light text-fg-dim truncate leading-tight">
                    {subtitle}
                  </p>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
