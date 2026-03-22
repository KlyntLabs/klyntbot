// tools/simulator/utils/dates.ts

/** Add days to a date, returning a new Date. */
export function addDays(date: Date, days: number): Date {
    const result = new Date(date);
    result.setDate(result.getDate() + days);
    return result;
}

/** Format date as YYYY-MM-DD for display. */
export function formatDate(date: Date): string {
    return date.toISOString().split("T")[0];
}

/** Format date as ISO 8601 string for API params. */
export function toISO(date: Date): string {
    return date.toISOString();
}

/** Add hours + minutes to a date, returning a new Date. */
export function addTime(date: Date, hours: number, minutes = 0): Date {
    const result = new Date(date);
    result.setHours(result.getHours() + hours, result.getMinutes() + minutes);
    return result;
}

const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/** Get day name from simulator ordinal (0=Monday). */
export function dayName(dayOfWeek: number): string {
    return DAY_NAMES[dayOfWeek] ?? "Unknown";
}
