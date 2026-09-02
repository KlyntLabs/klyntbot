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

/** CSS variable value for inline styles (not Tailwind classes). */
export function retentionCssColor(pct: number): string {
  if (pct >= 0.8) return "var(--ds-status-success)";
  if (pct >= 0.5) return "var(--ds-status-warning)";
  return "var(--ds-accent)";
}

export function retentionLabel(pct: number): string {
  if (pct >= 0.9) return "Strong";
  if (pct >= 0.7) return "Good";
  if (pct >= 0.5) return "Fading";
  return "Weak";
}
