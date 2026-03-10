import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { cn } from "@shared/lib/cn";
import { Check } from "lucide-react";

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  className?: string;
  disabled?: boolean;
}

export function Checkbox({ checked, onCheckedChange, className, disabled }: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      className={cn(
        "h-4 w-4 rounded border border-muted/40 flex items-center justify-center transition-colors",
        "data-[state=checked]:bg-brand data-[state=checked]:border-brand",
        "disabled:opacity-50 disabled:pointer-events-none",
        className,
      )}
    >
      <CheckboxPrimitive.Indicator>
        <Check className="h-3 w-3 text-white" strokeWidth={2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
