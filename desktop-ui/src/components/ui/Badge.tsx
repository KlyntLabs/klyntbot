import { cn } from '../../lib/utils';

type BadgeVariant = 'priority' | 'status' | 'area' | 'tag';

const colorMaps: Record<string, Record<string, string>> = {
  priority: {
    P1: 'bg-red-500/10 text-red-400/80',
    P2: 'bg-orange-500/10 text-orange-400/80',
    P3: 'bg-yellow-500/10 text-yellow-400/80',
    P4: 'bg-blue-500/10 text-blue-400/80',
  },
  status: {
    Todo: 'bg-[rgba(139,148,158,0.1)] text-[#8B949E]',
    Doing: 'bg-[rgba(249,115,22,0.1)] text-[#F97316]',
    Done: 'bg-[rgba(34,197,94,0.1)] text-[#22C55E]',
  },
  area: {
    Work: 'bg-[rgba(59,130,246,0.1)] text-[#3B82F6]',
    Personal: 'bg-[rgba(139,92,246,0.1)] text-[#8B5CF6]',
  },
};

const defaultColor = 'bg-[rgba(255,255,255,0.04)] text-[#8B949E]';

interface BadgeProps {
  variant: BadgeVariant;
  value: string;
  className?: string;
}

export function Badge({ variant, value, className }: BadgeProps) {
  const colorClass = colorMaps[variant]?.[value] ?? defaultColor;

  return (
    <span className={cn('px-2 py-0.5 text-[11px] font-light rounded', colorClass, className)}>
      {value}
    </span>
  );
}
