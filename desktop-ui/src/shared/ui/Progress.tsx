import * as ProgressPrimitive from "@radix-ui/react-progress";
import { cn } from "@shared/lib/utils";

export interface ProgressProps {
  value: number;
  className?: string;
  color?: "brand" | "success" | "warning" | "destructive" | "info";
}

const colors = {
  brand: "bg-brand",
  success: "bg-success",
  warning: "bg-warning",
  destructive: "bg-destructive",
  info: "bg-info",
};

export function Progress({ value, className, color = "brand" }: ProgressProps) {
  return (
    <ProgressPrimitive.Root
      value={value}
      className={cn("h-1.5 w-full bg-muted rounded-full overflow-hidden", className)}
    >
      <ProgressPrimitive.Indicator
        className={cn(colors[color], "h-full rounded-full transition-[width] duration-500")}
        style={{ width: `${value}%` }}
      />
    </ProgressPrimitive.Root>
  );
}
