/** RRULE preset mapping and human-readable label conversion. */

export const RECURRENCE_PRESETS = [
  { label: 'None', value: '' },
  { label: 'Daily', value: 'FREQ=DAILY;INTERVAL=1' },
  { label: 'Weekdays', value: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR' },
  { label: 'Weekly', value: 'FREQ=WEEKLY;INTERVAL=1' },
  { label: 'Biweekly', value: 'FREQ=WEEKLY;INTERVAL=2' },
  { label: 'Monthly', value: 'FREQ=MONTHLY;INTERVAL=1' },
  { label: 'Quarterly', value: 'FREQ=MONTHLY;INTERVAL=3' },
] as const;

const LABEL_MAP = new Map(
  RECURRENCE_PRESETS.filter(p => p.value).map(p => [p.value, p.label]),
);

/** Convert an RRULE string to a human-readable label. Falls back to raw rule. */
export function rruleToLabel(rule: string | null): string {
  if (!rule) return 'None';
  return LABEL_MAP.get(rule) ?? rule;
}
