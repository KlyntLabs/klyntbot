import type React from 'react';

// ── Date formatting ──────────────────────────────────────────────────────────

/** Format a past ISO date as "Xm ago", "Xh ago", "Xd ago", etc. */
export function formatTimeAgo(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 7) return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  if (diffDays > 0) return `${diffDays}d ago`;
  if (diffHours > 0) return `${diffHours}h ago`;
  if (diffMins > 0) return `${diffMins}m ago`;
  return 'just now';
}

/** Format a past ISO date with full words: "2 days ago", "1 hour ago", etc. */
export function formatTimeAgoLong(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
  if (diffHours > 0) return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
  if (diffMins > 0) return `${diffMins} min${diffMins === 1 ? '' : 's'} ago`;
  return 'just now';
}

/** Format an ISO date as "Jan 15, 2026" */
export function formatShortDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

/** Format minutes into a compact duration like "2h 30m" */
export function formatMinutes(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

// ── Priority ─────────────────────────────────────────────────────────────────

/** Priority color for P1–P4. Consistent across all pages. */
export function getPriorityColor(priority: number | null): string {
  switch (priority) {
    case 1: return '#e55050';
    case 2: return '#d4a017';
    case 3: return '#e5c07b';
    case 4: return '#666';
    default: return '#666';
  }
}

// ── Project colors ───────────────────────────────────────────────────────────

export const PROJECT_COLORS: Record<string, string> = {
  red: '#ef4444',
  orange: '#f97316',
  yellow: '#eab308',
  green: '#22c55e',
  blue: '#3b82f6',
  purple: '#8b5cf6',
  gray: '#6b7280',
};

export function resolveProjectColor(color: string): string {
  return PROJECT_COLORS[color.toLowerCase()] ?? color;
}

// ── Plan helpers ─────────────────────────────────────────────────────────────

export function getPlanStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'executing': return 'var(--codex-accent)';
    case 'approved': return 'var(--codex-accent)';
    case 'draft': return '#fbbf24';
    case 'failed': return '#ef4444';
    case 'abandoned': return 'var(--codex-fg-subtle)';
    default: return 'var(--codex-fg-subtle)';
  }
}

export function isPlanActive(status: string): boolean {
  const s = status.toLowerCase();
  return s === 'executing' || s === 'approved';
}

// ── Form styles ──────────────────────────────────────────────────────────────

export const inputStyle = {
  backgroundColor: 'var(--codex-bg)',
  color: 'var(--codex-fg)',
  border: '1px solid var(--codex-border)',
};

type FocusableElement = HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;

export const focusHandler = (e: React.FocusEvent<FocusableElement>) =>
  e.currentTarget.style.borderColor = 'var(--codex-accent)';

export const blurHandler = (e: React.FocusEvent<FocusableElement>) =>
  e.currentTarget.style.borderColor = 'var(--codex-border)';
