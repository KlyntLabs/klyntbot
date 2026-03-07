import * as ProgressPrimitive from "@radix-ui/react-progress";
import { cn } from "../../lib/utils";

interface ProgressProps {
  value: number;
  className?: string;
}

export function Progress({ value, className }: ProgressProps) {
  return (
    <ProgressPrimitive.Root
      value={value}
      className={cn("h-1.5 w-full bg-white/[0.08] rounded-full overflow-hidden", className)}
    >
      <ProgressPrimitive.Indicator
        className="h-full bg-brand rounded-full transition-[width] duration-500"
        style={{ width: `${value}%` }}
      />
    </ProgressPrimitive.Root>
  );
}
