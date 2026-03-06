import { useEffect, useRef, useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { formatTime } from "../../lib/dates";
import type { ActivitySwitchPayload, ActivityTimeline } from "../../lib/types";
import { AppIcon, getAppColor } from "./shared";

const BROWSER_RE =
  /\s*[-–—]\s*(?:Google Chrome|Safari|Firefox|Arc|Brave Browser|Microsoft Edge|Orion|Vivaldi|Opera|Chromium|Zen Browser)(?:\s*[-–—]\s*.+)?$/i;

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
  const { data: events, refetch } = useQuery<ActivityTimeline[]>(
    "productivity_activity_feed",
    { limit: 30 },
    [],
  );

  // Track which keys are "new" for animation
  const prevKeysRef = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());

  const scrollRef = useRef<HTMLDivElement>(null);

  // Refetch on context switch events (replaces polling)
  useEvent<ActivitySwitchPayload>("activity:switch", () => refetch());

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

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
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Activity</h2>
        <p className="text-[12px] font-light text-dim">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Activity</h2>
        <div className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
          <span className="text-[10px] font-light text-dim">Live</span>
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
              className={`flex items-center gap-2 py-1.5 ${isFirst ? "" : "border-t border-white/[0.04]"}`}
              style={isNew ? { animation: "fade-in 0.4s ease-out" } : undefined}
            >
              {/* App icon */}
              <div className="flex-shrink-0">
                {e.isIdle ? (
                  <span className="w-3.5 h-3.5 rounded-full bg-white/[0.08] block" />
                ) : (
                  <AppIcon appName={name} color={color} />
                )}
              </div>

              {/* Time */}
              <span className="text-[10px] tabular-nums w-10 flex-shrink-0 font-light text-dim">
                {formatTime(e.startedAt)}
              </span>
              {tag && (
                <span
                  className={`text-[9px] font-medium tabular-nums flex-shrink-0 ${isRecent ? "text-success" : "text-muted"}`}
                >
                  {tag}
                </span>
              )}

              {/* App/Site name */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span
                    className={`text-[11px] truncate ${e.isIdle ? "text-dim italic" : isFirst ? "font-normal text-primary" : "font-light text-secondary"}`}
                  >
                    {name}
                  </span>
                </div>
                {subtitle && !e.isIdle && (
                  <p className="text-[9px] font-light text-dim truncate leading-tight">
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
