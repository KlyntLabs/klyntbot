import * as ProgressPrimitive from "@radix-ui/react-progress";
import { cn } from "@shared/lib/utils";
import { cva, type VariantProps } from "class-variance-authority";

const progressIndicatorVariants = cva("h-full rounded-full transition-[width] duration-500", {
  variants: {
    color: {
      brand: "bg-brand",
      success: "bg-success",
      warning: "bg-warning",
      destructive: "bg-destructive",
      info: "bg-info",
    },
  },
  defaultVariants: {
    color: "brand",
  },
});

export interface ProgressProps extends VariantProps<typeof progressIndicatorVariants> {
  value: number;
  className?: string;
}

export function Progress({ value, className, color }: ProgressProps) {
  return (
    <ProgressPrimitive.Root
      value={value}
      className={cn("h-1.5 w-full bg-muted rounded-full overflow-hidden", className)}
    >
      <ProgressPrimitive.Indicator
        className={cn(progressIndicatorVariants({ color }))}
        style={{ width: `${value}%` }}
      />
    </ProgressPrimitive.Root>
  );
}

export { progressIndicatorVariants };
