const SHORT_MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/**
 * Format an ISO date string ("2026-03-04") as "Mar 4".
 * Returns the input unchanged if it can't be parsed.
 */
export function formatDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return iso;
  return `${SHORT_MONTHS[m - 1]} ${d}`;
}

/** Format seconds as "Xh Ym" or "Ym" for human-readable durations. */
export function formatHumanDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Format seconds as "HH:MM:SS" or "MM:SS" for live timers. */
export function formatElapsed(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Format an ISO timestamp to locale time (e.g. "2:30 PM"). */
export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/** Format an ISO date string's weekday (e.g. "Mon"). */
export function formatDayLabel(dateStr: string): string {
  const d = new Date(`${dateStr}T00:00:00`);
  return d.toLocaleDateString("en-US", { weekday: "short" });
}

const LONG_MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Format as "Thursday, March 5, 2026" */
export function formatFullDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  return `${WEEKDAYS[d.getDay()]}, ${LONG_MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

/** Format as "Mar 3 - Mar 9, 2026" */
export function formatWeekRange(weekStart: string): string {
  const start = new Date(`${weekStart}T00:00:00`);
  const end = new Date(start);
  end.setDate(end.getDate() + 6);
  const sy = start.getFullYear();
  const ey = end.getFullYear();
  const s = `${SHORT_MONTHS[start.getMonth()]} ${start.getDate()}`;
  const e = `${SHORT_MONTHS[end.getMonth()]} ${end.getDate()}, ${ey}`;
  if (sy !== ey) return `${s}, ${sy} - ${e}`;
  return `${s} - ${e}`;
}

/** Format as "March 2026" */
export function formatMonthLabel(yearMonth: string): string {
  const [y, m] = yearMonth.split("-").map(Number);
  return `${LONG_MONTHS[m - 1]} ${y}`;
}

/** Format a Date as YYYY-MM-DD using local timezone (NOT UTC). */
export function toLocalISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Format a Date as YYYY-MM-DDTHH:mm using local timezone (for datetime-local inputs). */
export function toLocalDateTime(d: Date): string {
  const date = toLocalISO(d);
  const h = String(d.getHours()).padStart(2, "0");
  const m = String(d.getMinutes()).padStart(2, "0");
  return `${date}T${h}:${m}`;
}

/** Get today as YYYY-MM-DD in local timezone. */
export function todayISO(): string {
  return toLocalISO(new Date());
}

/**
 * JS-style timezone offset in minutes (e.g. -420 for UTC+7).
 * Pass to backend APIs so they query the correct UTC range for a local date.
 */
export const TZ_OFFSET_MINS = new Date().getTimezoneOffset();

/** Get the Monday of the week containing the given date */
export function weekStartISO(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  const day = d.getDay();
  const diff = d.getDate() - day + (day === 0 ? -6 : 1);
  d.setDate(diff);
  return toLocalISO(d);
}

/** Get YYYY-MM from a date */
export function monthISO(iso: string): string {
  return iso.slice(0, 7);
}

/** Navigate a date by offset: +1 day, -1 day, etc. */
export function shiftDate(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00`);
  d.setDate(d.getDate() + days);
  return toLocalISO(d);
}

/** Navigate a month by offset: +1 month, -1 month */
export function shiftMonth(yearMonth: string, months: number): string {
  const [y, m] = yearMonth.split("-").map(Number);
  const d = new Date(y, m - 1 + months, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** Get the last day of a month as YYYY-MM-DD */
export function monthEndISO(yearMonth: string): string {
  const [y, m] = yearMonth.split("-").map(Number);
  const d = new Date(y, m, 0);
  return toLocalISO(d);
}

/** Format seconds as "Xh Ym" with large text style (e.g. "7 hr 33 min") */
export function formatLongDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0 && m > 0) return `${h} hr ${m} min`;
  if (h > 0) return `${h} hr`;
  return `${m} min`;
}

/** Minutes elapsed since midnight for an ISO timestamp (local timezone). */
export function minutesSinceMidnight(isoStr: string): number {
  const d = new Date(isoStr);
  return d.getHours() * 60 + d.getMinutes();
}

/** Format an ISO timestamp as a compact relative time (e.g. "now", "5m", "3h", "2d", "1w", "3mo"). */
export function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}mo`;
}
