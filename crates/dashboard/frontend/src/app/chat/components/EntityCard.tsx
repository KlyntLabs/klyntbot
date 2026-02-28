import { useNavigate } from 'react-router';
import { motion } from 'motion/react';
import {
  CheckSquare,
  FolderKanban,
  Target,
  DollarSign,
  Clock,
  ChevronRight,
} from 'lucide-react';
import type { EntityCardData } from '../../../lib/types';

const ICON_MAP: Record<string, React.ComponentType<{ className?: string; strokeWidth?: number; style?: React.CSSProperties }>> = {
  task: CheckSquare,
  project: FolderKanban,
  target: Target,
  dollar: DollarSign,
  clock: Clock,
};

export function EntityCard({ card }: { card: EntityCardData }) {
  const navigate = useNavigate();
  const Icon = ICON_MAP[card.iconHint] ?? CheckSquare;

  const handleClick = () => {
    if (card.route) {
      navigate(card.route);
    }
  };

  return (
    <motion.button
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      onClick={handleClick}
      className="flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-left transition-colors cursor-pointer"
      style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        border: '1px solid var(--codex-border)',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.borderColor = 'var(--codex-accent)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.borderColor = 'var(--codex-border)';
      }}
    >
      <div
        className="flex items-center justify-center w-8 h-8 rounded-md flex-shrink-0"
        style={{ backgroundColor: 'var(--codex-bg-tertiary)' }}
      >
        <Icon className="w-4 h-4" strokeWidth={1.5} style={{ color: 'var(--codex-accent)' }} />
      </div>
      <div className="flex-1 min-w-0">
        <div
          className="text-[13px] font-medium truncate"
          style={{ color: 'var(--codex-fg)' }}
        >
          {card.title}
        </div>
        {card.subtitle && (
          <div
            className="text-[11px] truncate mt-0.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {card.subtitle}
          </div>
        )}
      </div>
      <ChevronRight
        className="w-4 h-4 flex-shrink-0"
        strokeWidth={1.5}
        style={{ color: 'var(--codex-fg-subtle)' }}
      />
    </motion.button>
  );
}
