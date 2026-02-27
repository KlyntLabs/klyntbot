import { useState } from 'react';
import {
  CheckSquare,
  FileText,
  Calendar,
  DollarSign,
  Zap,
  Clock,
  FolderKanban,
  Globe,
  File,
  MessageSquare,
  GitBranch,
} from 'lucide-react';
import type { ToolCategory, ToolActivityEntry } from '../../../lib/types';

const TOOL_CATEGORIES: { category: ToolCategory; icon: typeof CheckSquare }[] = [
  { category: 'Tasks', icon: CheckSquare },
  { category: 'Plans', icon: FileText },
  { category: 'Calendar', icon: Calendar },
  { category: 'Finance', icon: DollarSign },
  { category: 'Skills', icon: Zap },
  { category: 'Cron', icon: Clock },
  { category: 'Projects', icon: FolderKanban },
  { category: 'Web', icon: Globe },
  { category: 'Files', icon: File },
  { category: 'Message', icon: MessageSquare },
  { category: 'Spawn', icon: GitBranch },
];

interface ToolActivityPanelProps {
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

function getToolState(
  category: ToolCategory,
  activeTools: Set<string>,
  toolHistory: ToolActivityEntry[],
): 'inactive' | 'active' | 'used' {
  if (activeTools.has(category)) return 'active';
  if (toolHistory.some((e) => e.category === category)) return 'used';
  return 'inactive';
}

function getLastOperation(category: ToolCategory, toolHistory: ToolActivityEntry[]): string | null {
  for (let i = toolHistory.length - 1; i >= 0; i--) {
    if (toolHistory[i].category === category) {
      const entry = toolHistory[i];
      const argsStr = entry.args
        ? Object.entries(entry.args)
            .map(([k, v]) => `${k}: ${JSON.stringify(v)}`)
            .join(', ')
        : '';
      const status = entry.status === 'failed' ? ' (failed)' : '';
      return `${entry.toolName}${argsStr ? ` — ${argsStr}` : ''}${status}`;
    }
  }
  return null;
}

export function ToolActivityPanel({ activeTools, toolHistory }: ToolActivityPanelProps) {
  const [hoveredCategory, setHoveredCategory] = useState<string | null>(null);

  return (
    <div className="px-4 py-3">
      <div
        className="text-[10px] uppercase tracking-wider mb-2.5"
        style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
      >
        Systems
      </div>
      <div className="flex flex-wrap gap-1.5">
        {TOOL_CATEGORIES.map(({ category, icon: Icon }) => {
          const state = getToolState(category, activeTools, toolHistory);
          const lastOp = getLastOperation(category, toolHistory);

          return (
            <div
              key={category}
              className="relative"
              onMouseEnter={() => setHoveredCategory(category)}
              onMouseLeave={() => setHoveredCategory(null)}
            >
              <div
                className={`flex items-center gap-1 px-2 py-1 rounded-md text-[10px] border transition-all ${
                  state === 'active' ? 'animate-pulse' : ''
                }`}
                style={{
                  opacity: state === 'inactive' ? 0.3 : 1,
                  borderColor:
                    state === 'active'
                      ? 'var(--codex-accent)'
                      : state === 'used'
                        ? 'var(--codex-border)'
                        : 'var(--codex-border-subtle)',
                  backgroundColor:
                    state === 'active'
                      ? 'var(--codex-accent-dim)'
                      : 'transparent',
                  color:
                    state === 'active'
                      ? 'var(--codex-accent)'
                      : state === 'used'
                        ? 'var(--codex-fg-muted)'
                        : 'var(--codex-fg-subtle)',
                }}
              >
                <Icon className="w-3 h-3" strokeWidth={1.5} />
                <span style={{ fontFamily: 'var(--font-mono)' }}>{category}</span>
              </div>

              {/* Tooltip */}
              {hoveredCategory === category && lastOp && (
                <div
                  className="absolute left-0 top-full mt-1 px-2 py-1 rounded text-[10px] whitespace-nowrap z-50"
                  style={{
                    backgroundColor: 'var(--codex-bg-tertiary)',
                    border: '1px solid var(--codex-border)',
                    color: 'var(--codex-fg-muted)',
                    fontFamily: 'var(--font-mono)',
                    maxWidth: '220px',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {lastOp}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
