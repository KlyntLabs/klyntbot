import { marked } from 'marked';
import type { ThinkingState } from '../../lib/hooks/useAgent';

// Configure marked for safe rendering
marked.setOptions({
  gfm: true,
  breaks: true,
});

/** Render markdown to sanitized HTML */
export function renderMarkdown(content: string): string {
  const html = marked.parse(content, { async: false }) as string;
  return html
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '')
    .replace(/on\w+="[^"]*"/gi, '')
    .replace(/on\w+='[^']*'/gi, '');
}

/** Parse Rust debug format strategy into a clean label */
export function strategyLabel(raw: string): string {
  if (raw.startsWith('Direct')) return 'Direct';
  if (raw.startsWith('Reactive')) return 'Reactive';
  if (raw.startsWith('Planned')) return 'Planned';
  return raw;
}

/** Format relative time like "in 2h", "tomorrow", "in 3d" */
export function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  const target = new Date(dateStr).getTime();
  const diffMs = target - now;
  if (diffMs < 0) return 'past';
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 60) return `in ${diffMin}m`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `in ${diffHours}h`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays === 1) return 'tomorrow';
  return `in ${diffDays}d`;
}

/** Priority label + color */
export function priorityDisplay(p: number | null): { label: string; color: string } {
  switch (p) {
    case 1: return { label: 'P1', color: '#ef4444' };
    case 2: return { label: 'P2', color: '#f59e0b' };
    case 3: return { label: 'P3', color: 'var(--codex-accent)' };
    case 4: return { label: 'P4', color: 'var(--codex-fg-subtle)' };
    default: return { label: '--', color: 'var(--codex-fg-subtle)' };
  }
}

/** Map thinking phase to a human-readable label */
export function phaseLabel(phase: ThinkingState['phase']): string {
  switch (phase) {
    case 'classifying':
      return 'Classifying';
    case 'buildingContext':
      return 'Building context';
    case 'thinking':
      return 'Thinking';
    case 'idle':
      return 'Idle';
  }
}

/** Format a Date to a short time string like "10:32 AM" */
export function formatTime(d: Date): string {
  return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

/** Compute a human-readable duration between two ISO date strings */
export function formatDuration(createdAt: string, updatedAt: string): string {
  const start = new Date(createdAt).getTime();
  const end = new Date(updatedAt).getTime();
  const diffMs = Math.max(0, end - start);
  const totalMinutes = Math.floor(diffMs / 60000);
  if (totalMinutes < 1) return '<1m';
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes}m`;
}

export type SuggestionCard = {
  id: string;
  icon: React.ComponentType<{ className?: string; strokeWidth?: number; style?: React.CSSProperties }>;
  title: string;
  description: string;
};
