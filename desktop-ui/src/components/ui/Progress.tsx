import * as ProgressPrimitive from '@radix-ui/react-progress';
import { cn } from '../../lib/utils';

interface ProgressProps {
  value: number;
  className?: string;
}

export function Progress({ value, className }: ProgressProps) {
  return (
    <ProgressPrimitive.Root
      value={value}
      className={cn('h-1.5 w-full bg-[rgba(255,255,255,0.06)] rounded-full overflow-hidden', className)}
    >
      <ProgressPrimitive.Indicator
        className="h-full bg-[#F97316] rounded-full transition-all"
        style={{ width: `${value}%` }}
      />
    </ProgressPrimitive.Root>
  );
}
