/** Format a millisecond duration as a human-readable string (e.g. "123ms", "1.2s"). */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** Format a token count as a compact string (e.g. "120", "1.2k", "2.5M"). */
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** Format a USD cost as a string (e.g. "$0.0012", "$0.053"). */
export function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  return `$${usd.toFixed(3)}`;
}

/** Build a qualified tool display name (e.g. "task:list" or just "calendar"). */
export function qualifiedToolName(name: string, action?: string): string {
  return action ? `${name}:${action}` : name;
}
