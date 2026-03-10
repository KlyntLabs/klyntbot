import type { CronJob, CronOrigin, CronSchedule } from "./types";

/** Humanize a CronSchedule to a readable string */
export function humanizeSchedule(schedule: CronSchedule): string {
  switch (schedule.kind) {
    case "at":
      return new Date(schedule.atMs).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      });
    case "every": {
      const ms = schedule.everyMs;
      if (ms < 60_000) return `Every ${Math.round(ms / 1000)}s`;
      if (ms < 3_600_000) return `Every ${Math.round(ms / 60_000)} min`;
      if (ms < 86_400_000) {
        const h = Math.round(ms / 3_600_000);
        return h === 1 ? "Every hour" : `Every ${h} hours`;
      }
      const d = Math.round(ms / 86_400_000);
      return d === 1 ? "Every day" : `Every ${d} days`;
    }
    case "cron":
      return humanizeCronExpr(schedule.expr, schedule.tz);
  }
}

/** Best-effort humanization of common cron expressions */
function humanizeCronExpr(expr: string, tz?: string): string {
  const parts = expr.trim().split(/\s+/);
  // Handle both 5-field and 6-field (with seconds) cron
  const [min, hour, dom, _mon, dow] = parts.length === 6 ? parts.slice(1) : parts;

  const tzSuffix = tz && tz !== "UTC" ? ` (${tz})` : "";

  if (min !== undefined && hour !== undefined && dom === "*" && dow === "*") {
    const h = Number.parseInt(hour);
    const m = Number.parseInt(min);
    if (!Number.isNaN(h) && !Number.isNaN(m)) {
      const time = formatTime(h, m);
      return `Daily at ${time}${tzSuffix}`;
    }
  }

  if (min !== undefined && hour !== undefined && dom === "*" && dow !== undefined && dow !== "*") {
    const h = Number.parseInt(hour);
    const m = Number.parseInt(min);
    if (!Number.isNaN(h) && !Number.isNaN(m)) {
      const time = formatTime(h, m);
      const dayName = dayOfWeek(dow);
      if (!dayName) return `${expr}${tzSuffix}`;
      // Multi-day: "Mon, Wed, Fri at 10 AM"; single day: "Mondays at 10 AM"
      const dayLabel = dow.includes(",") ? dayName : `${dayName}s`;
      return `${dayLabel} at ${time}${tzSuffix}`;
    }
  }

  return `${expr}${tzSuffix}`;
}

function formatTime(h: number, m: number): string {
  const ampm = h >= 12 ? "PM" : "AM";
  const h12 = h % 12 || 12;
  return m === 0 ? `${h12} ${ampm}` : `${h12}:${String(m).padStart(2, "0")} ${ampm}`;
}

const DAY_NAMES: Record<string, string> = {
  "0": "Sun",
  "1": "Mon",
  "2": "Tue",
  "3": "Wed",
  "4": "Thu",
  "5": "Fri",
  "6": "Sat",
  "7": "Sun",
};

function dayOfWeek(dow: string): string | null {
  // Handle comma-separated days like "1,3,5"
  if (dow.includes(",")) {
    const names = dow.split(",").map((d) => DAY_NAMES[d.trim()]);
    if (names.every(Boolean)) return names.join(", ");
    return null;
  }
  const FULL_NAMES: Record<string, string> = {
    "0": "Sunday",
    "1": "Monday",
    "2": "Tuesday",
    "3": "Wednesday",
    "4": "Thursday",
    "5": "Friday",
    "6": "Saturday",
    "7": "Sunday",
  };
  return FULL_NAMES[dow] ?? null;
}

/** Humanize a job name by stripping prefixes and converting to title case */
export function humanizeJobName(name: string): string {
  return name
    .replace(/^__klyntbot_/, "")
    .replace(/^todo_/, "")
    .replace(/^plugin:.*?:/, "")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Format relative time: "in 28 min", "2h ago", etc. */
export function relativeTime(ms: number): string {
  const now = Date.now();
  const diff = ms - now;
  const abs = Math.abs(diff);
  const suffix = diff > 0 ? "" : " ago";
  const prefix = diff > 0 ? "in " : "";

  if (abs < 60_000) return "just now";
  if (abs < 3_600_000) return `${prefix}${Math.round(abs / 60_000)} min${suffix}`;
  if (abs < 86_400_000) return `${prefix}${Math.round(abs / 3_600_000)}h${suffix}`;
  return `${prefix}${Math.round(abs / 86_400_000)}d${suffix}`;
}

/** Origin badge config — uses CSS token variables from theme.css */
export const ORIGIN_STYLES: Record<CronOrigin, { label: string; className: string }> = {
  system: { label: "System", className: "bg-origin-system/20 text-origin-system" },
  ai: { label: "AI", className: "bg-origin-ai/20 text-origin-ai" },
  user: { label: "User", className: "bg-origin-user/20 text-origin-user" },
  plugin: { label: "Plugin", className: "bg-origin-plugin/20 text-origin-plugin" },
};
