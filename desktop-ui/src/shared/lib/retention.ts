export function retentionTextColor(pct: number): string {
  if (pct >= 0.8) return "text-green-400";
  if (pct >= 0.5) return "text-amber-400";
  return "text-red-400";
}

export function retentionBarColor(pct: number): string {
  if (pct >= 0.8) return "bg-green-500";
  if (pct >= 0.5) return "bg-amber-500";
  return "bg-red-500";
}
