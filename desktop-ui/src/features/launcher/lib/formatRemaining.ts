export function formatRemaining(endsAtISO: string): string {
  const diffMs = new Date(endsAtISO).getTime() - Date.now();
  if (diffMs <= 0) return "0s";
  const totalSecs = Math.floor(diffMs / 1000);
  const hrs = Math.floor(totalSecs / 3600);
  const mins = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;
  if (hrs > 0) return `${hrs}h ${mins}m`;
  if (mins > 0) return `${mins}m`;
  return `${secs}s`;
}
