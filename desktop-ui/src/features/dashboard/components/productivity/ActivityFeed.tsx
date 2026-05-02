import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { ActivityTimelineResponse } from "@/bindings";
import { productivityActivityFeedQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatTime } from "@/utils/dashboardDates";
import { AppIcon, getAppColor } from "../../lib/productivity";

const BROWSER_RE =
  /\s*[-–—]\s*(?:Google Chrome|Safari|Firefox|Arc|Brave Browser|Microsoft Edge|Orion|Vivaldi|Opera|Chromium|Zen Browser)(?:\s*[-–—]\s*.+)?$/i;

function stripBrowserSuffix(title: string): string {
  return title.replace(BROWSER_RE, "").trim();
}

function resolveDisplayName(e: ActivityTimelineResponse): { name: string; subtitle?: string } {
  if (e.isIdle) return { name: "Idle" };
  if (e.siteName && e.windowTitle) {
    const pageTitle = stripBrowserSuffix(e.windowTitle);
    if (pageTitle && pageTitle.toLowerCase() !== e.siteName.toLowerCase()) {
      return { name: e.siteName, subtitle: pageTitle };
    }
  }
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
  const client = useQueryClient();
  const { data: events } = useTauriQuery<ActivityTimelineResponse[]>({
    queryKey: qk.productivity.activityFeed(30),
    queryFn: () => productivityActivityFeedQuery(30),
    fallback: [],
  });

  const prevKeysRef = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());
  const scrollRef = useRef<HTMLDivElement>(null);

  // Periodic poll every 30s
  useEffect(() => {
    const id = setInterval(() => {
      void client.invalidateQueries({ queryKey: qk.productivity.activityFeed(30) });
    }, 30_000);
    return () => clearInterval(id);
  }, [client]);

  // Periodic re-render for relative-time labels
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 30_000);
    return () => clearInterval(id);
  }, []);

  // Detect new entries and animate
  useEffect(() => {
    const currentKeys = new Set(events.map((e, i) => `${e.startedAt}-${e.appName}-${i}`));
    const fresh = new Set<string>();
    for (const k of currentKeys) {
      if (!prevKeysRef.current.has(k)) fresh.add(k);
    }
    prevKeysRef.current = currentKeys;

    if (fresh.size > 0) {
      setNewKeys(fresh);
      if (typeof scrollRef.current?.scrollTo === "function") {
        scrollRef.current.scrollTo({ top: 0, behavior: "smooth" });
      }
      const timer = setTimeout(() => setNewKeys(new Set()), 600);
      return () => clearTimeout(timer);
    }
  }, [events]);

  if (events.length === 0) {
    return (
      <div className="dashboard__activity-feed">
        <h2 className="dashboard__activity-feed-header">Activity</h2>
        <p className="dashboard__activity-feed-empty">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="dashboard__activity-feed">
      <div className="dashboard__activity-feed-header">
        <h2>Activity</h2>
        <div>
          <span className="dashboard__activity-feed-live-dot" />
          <span>Live</span>
        </div>
      </div>
      <div ref={scrollRef} className="dashboard__activity-feed-list">
        {events.map((e, i) => {
          const { name, subtitle } = resolveDisplayName(e);
          const color = getAppColor(name, e.categoryId);
          const isFirst = i === 0;
          const key = `${e.startedAt}-${e.appName}-${i}`;
          const isNew = newKeys.has(key);
          const age = ageSecs(e.startedAt);
          const tag = relativeTag(age);
          const isRecent = age < 60;

          const rowClass = [
            "dashboard__activity-feed-row",
            isFirst && "dashboard__activity-feed-row--first",
            isNew && "dashboard__activity-feed-row--new",
          ]
            .filter(Boolean)
            .join(" ");

          return (
            <div key={key} className={rowClass}>
              <div className="dashboard__activity-feed-icon">
                {e.isIdle ? <span /> : <AppIcon appName={name} color={color} />}
              </div>

              <span className="dashboard__activity-feed-time">{formatTime(e.startedAt)}</span>
              {tag && (
                <span
                  className={
                    isRecent
                      ? "dashboard__activity-feed-tag dashboard__activity-feed-tag--recent"
                      : "dashboard__activity-feed-tag"
                  }
                >
                  {tag}
                </span>
              )}

              <div>
                <span
                  className={
                    e.isIdle
                      ? "dashboard__activity-feed-name dashboard__activity-feed-name--idle"
                      : isFirst
                        ? "dashboard__activity-feed-name dashboard__activity-feed-name--first"
                        : "dashboard__activity-feed-name"
                  }
                >
                  {name}
                </span>
                {subtitle && !e.isIdle && (
                  <p className="dashboard__activity-feed-subtitle">{subtitle}</p>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
